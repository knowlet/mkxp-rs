/// Playback state for BGM, BGS, ME, and SE.
///
/// mkxp defines `pos` as seconds. This runtime keeps that clock and the fade
/// ramps, and records which file each channel was asked to play. Decoding and
/// device output are left to a host that has an audio backend; the script-facing
/// state (`bgm_pos`, volume, fade completion) does not depend on one.
#[derive(Clone, Debug)]
pub struct Channel {
    pub filename: String,
    pub volume: i32,
    pub pitch: i32,
    pub pos: f64,
    pub playing: bool,
    fade_from: i32,
    fade_ms: i32,
    fade_left: i32,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            filename: String::new(),
            volume: 100,
            pitch: 100,
            pos: 0.0,
            playing: false,
            fade_from: 100,
            fade_ms: 0,
            fade_left: 0,
        }
    }
}

impl Channel {
    fn play(&mut self, filename: &str, volume: i32, pitch: i32, pos: f64) {
        let volume = volume.clamp(0, 100);
        let pitch = pitch.clamp(50, 150);
        // Same file and pitch, and no fade in progress: keep the clock and
        // only replace the base volume. An in-progress fade is finished by
        // stopping it and starting from the requested position.
        if self.playing && self.fade_left <= 0 && self.filename == filename && self.pitch == pitch {
            self.volume = volume;
            return;
        }
        self.filename = filename.to_string();
        self.volume = volume;
        self.pitch = pitch;
        self.pos = pos.max(0.0);
        self.playing = !filename.is_empty();
        self.fade_ms = 0;
        self.fade_left = 0;
        self.fade_from = self.volume;
    }

    fn stop(&mut self) {
        self.playing = false;
        self.fade_ms = 0;
        self.fade_left = 0;
        self.pos = 0.0;
    }

    fn fade(&mut self, time_ms: i32) {
        if !self.playing || self.fade_left > 0 {
            return;
        }
        self.fade_from = self.volume;
        self.fade_ms = time_ms.max(0);
        self.fade_left = self.fade_ms;
        if self.fade_ms == 0 {
            self.stop();
        }
    }

    fn advance(&mut self, dt_ms: i32) {
        if !self.playing {
            return;
        }
        if dt_ms > 0 {
            let rate = self.pitch as f64 / 100.0;
            self.pos += (dt_ms as f64 / 1000.0) * rate;
        }
        if self.fade_ms > 0 {
            self.fade_left -= dt_ms;
            if self.fade_left <= 0 {
                // FadeOut returns to 1 after the stream stops. The base volume
                // stays at the value play() stored.
                self.stop();
            }
        }
    }

    /// `base * ratio * fade`, in percent. `external` is the ME duck, 0..=1.
    fn level(&self, ratio: i32, external: f32) -> i32 {
        if !self.playing {
            return 0;
        }
        let fade = if self.fade_ms > 0 {
            (self.fade_left as f32 / self.fade_ms as f32).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let value = self.volume as f32 * (ratio as f32 / 100.0) * fade * external;
        value.round().clamp(0.0, 100.0) as i32
    }
}

#[derive(Clone, Debug)]
struct SeSlot {
    channel: Channel,
}

/// RGSS `Audio` module.
#[derive(Clone, Debug)]
pub struct Audio {
    bgm: Vec<Channel>,
    bgs: Channel,
    me: Channel,
    se: Vec<SeSlot>,
    se_limit: usize,
    /// `AudioStream::BaseRatio`. `track == -127` changes this, not each track.
    bgm_ratio: i32,
    /// ME playback ducks BGM: 200ms of continued playback, then the clock holds.
    bgm_me_active: bool,
    bgm_me_fade_left: i32,
}

impl Audio {
    pub fn new(bgm_tracks: i32, se_sources: i32) -> Self {
        let tracks = bgm_tracks.clamp(1, 16) as usize;
        let se_limit = se_sources.clamp(1, 64) as usize;
        Self {
            bgm: vec![Channel::default(); tracks],
            bgs: Channel::default(),
            me: Channel::default(),
            se: Vec::new(),
            se_limit,
            bgm_ratio: 100,
            bgm_me_active: false,
            bgm_me_fade_left: 0,
        }
    }

    pub fn bgm_play(&mut self, filename: &str, volume: i32, pitch: i32, pos: f64, track: i32) {
        if track == -127 {
            for channel in self.bgm.iter_mut().skip(1) {
                channel.stop();
            }
            if let Some(channel) = self.bgm.get_mut(0) {
                channel.play(filename, volume, pitch, pos);
            }
            return;
        }
        if let Some(channel) = self.bgm_mut(track) {
            channel.play(filename, volume, pitch, pos);
        }
    }

    pub fn bgm_stop(&mut self, track: i32) {
        self.each_bgm(track, Channel::stop);
    }

    pub fn bgm_fade(&mut self, time_ms: i32, track: i32) {
        self.each_bgm(track, |channel| channel.fade(time_ms));
    }

    pub fn bgm_pos(&self, track: i32) -> f64 {
        self.bgm_ref(track).map(|c| c.pos).unwrap_or(0.0)
    }

    pub fn bgm_volume(&self, track: i32) -> i32 {
        if track == -127 {
            return self.bgm_ratio;
        }
        self.bgm_ref(track).map(|c| c.volume).unwrap_or(0)
    }

    /// Effective output after the master ratio, the fade multiplier, and ME ducking.
    pub fn bgm_level(&self, track: i32) -> i32 {
        let Some(channel) = self.bgm_ref(track) else {
            return 0;
        };
        channel.level(self.bgm_ratio, self.me_factor())
    }

    pub fn set_bgm_volume(&mut self, volume: i32, track: i32) {
        let volume = volume.clamp(0, 100);
        if track == -127 {
            self.bgm_ratio = volume;
            return;
        }
        if let Some(channel) = self.bgm_mut(track) {
            channel.volume = volume;
        }
    }

    pub fn bgm_filename(&self, track: i32) -> &str {
        self.bgm_ref(track)
            .map(|c| c.filename.as_str())
            .unwrap_or("")
    }

    pub fn bgs_play(&mut self, filename: &str, volume: i32, pitch: i32, pos: f64) {
        self.bgs.play(filename, volume, pitch, pos);
    }
    pub fn bgs_stop(&mut self) {
        self.bgs.stop();
    }
    pub fn bgs_fade(&mut self, time_ms: i32) {
        self.bgs.fade(time_ms);
    }
    pub fn bgs_pos(&self) -> f64 {
        self.bgs.pos
    }

    pub fn me_play(&mut self, filename: &str, volume: i32, pitch: i32) {
        self.me.play(filename, volume, pitch, 0.0);
    }
    pub fn me_stop(&mut self) {
        self.me.stop();
    }
    pub fn me_fade(&mut self, time_ms: i32) {
        self.me.fade(time_ms);
    }

    pub fn se_play(&mut self, filename: &str, volume: i32, pitch: i32) {
        if filename.is_empty() {
            return;
        }
        if self.se.len() >= self.se_limit {
            self.se.remove(0);
        }
        let mut channel = Channel::default();
        channel.play(filename, volume, pitch, 0.0);
        self.se.push(SeSlot { channel });
    }

    pub fn se_stop(&mut self) {
        self.se.clear();
    }

    pub fn se_count(&self) -> usize {
        self.se.len()
    }

    pub fn reset(&mut self) {
        for channel in &mut self.bgm {
            channel.stop();
        }
        self.bgs.stop();
        self.me.stop();
        self.se.clear();
        self.bgm_me_active = false;
        self.bgm_me_fade_left = 0;
    }

    /// Advance playback clocks by `dt_ms`. Called once per presented frame.
    pub fn update(&mut self, dt_ms: i32) {
        let bgm_dt = self.bgm_dt(dt_ms);
        for channel in &mut self.bgm {
            channel.advance(bgm_dt);
        }
        self.bgs.advance(dt_ms);
        self.me.advance(dt_ms);
        for slot in &mut self.se {
            slot.channel.advance(dt_ms);
        }
    }

    /// While an ME is playing, BGM keeps moving for the 200ms duck, then holds.
    fn bgm_dt(&mut self, dt_ms: i32) -> i32 {
        if !self.me.playing {
            self.bgm_me_active = false;
            self.bgm_me_fade_left = 0;
            return dt_ms;
        }
        if !self.bgm_me_active {
            self.bgm_me_active = true;
            self.bgm_me_fade_left = 200;
        }
        let audible = self.bgm_me_fade_left.min(dt_ms).max(0);
        self.bgm_me_fade_left -= audible;
        audible
    }

    fn me_factor(&self) -> f32 {
        if !self.bgm_me_active {
            return 1.0;
        }
        if self.bgm_me_fade_left <= 0 {
            return 0.0;
        }
        self.bgm_me_fade_left as f32 / 200.0
    }

    fn track_slot(&self, track: i32) -> Option<usize> {
        if track == -127 {
            return Some(0);
        }
        if track < 0 || track as usize >= self.bgm.len() {
            return None;
        }
        Some(track as usize)
    }

    fn each_bgm(&mut self, track: i32, mut apply: impl FnMut(&mut Channel)) {
        if track == -127 {
            for channel in &mut self.bgm {
                apply(channel);
            }
            return;
        }
        if let Some(channel) = self.bgm_mut(track) {
            apply(channel);
        }
    }

    fn bgm_ref(&self, track: i32) -> Option<&Channel> {
        self.bgm.get(self.track_slot(track)?)
    }

    fn bgm_mut(&mut self, track: i32) -> Option<&mut Channel> {
        if track < 0 || track as usize >= self.bgm.len() {
            return None;
        }
        self.bgm.get_mut(track as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fade_reaches_silence_and_clears_the_channel() {
        let mut audio = Audio::new(1, 6);
        audio.bgm_play("Audio/BGM/Theme", 100, 100, 1.5, 0);
        audio.bgm_fade(1000, 0);
        audio.update(500);
        assert!(audio.bgm_pos(0) > 1.5);
        assert_eq!(audio.bgm_volume(0), 100);
        assert!(audio.bgm_level(0) < 100);
        assert!(!audio.bgm_filename(0).is_empty());
        audio.update(500);
        assert_eq!(audio.bgm_pos(0), 0.0);
        assert_eq!(audio.bgm_level(0), 0);
    }

    #[test]
    fn master_ratio_does_not_replace_track_base_volume() {
        let mut audio = Audio::new(2, 1);
        audio.bgm_play("quiet", 20, 100, 0.0, 0);
        audio.bgm_play("loud", 80, 100, 0.0, 1);
        audio.set_bgm_volume(50, -127);
        assert_eq!(audio.bgm_volume(0), 20);
        assert_eq!(audio.bgm_volume(1), 80);
        assert_eq!(audio.bgm_volume(-127), 50);
        assert_eq!(audio.bgm_level(0), 10);
        assert_eq!(audio.bgm_level(1), 40);
        audio.bgm_fade(1000, 1);
        audio.update(500);
        assert_eq!(audio.bgm_volume(1), 80);
        assert_eq!(audio.bgm_level(1), 20);
    }

    #[test]
    fn me_playback_pauses_bgm_and_resumes_the_same_position() {
        let mut audio = Audio::new(1, 1);
        audio.bgm_play("bgm", 100, 100, 10.0, 0);
        audio.me_play("me", 100, 100);
        audio.update(1000);
        audio.update(1000);
        let held = audio.bgm_pos(0);
        assert!(
            (held - 10.2).abs() < 1e-9,
            "BGM should advance only through the 200ms duck, got {held}"
        );
        audio.me_stop();
        audio.update(1000);
        let resumed = audio.bgm_pos(0);
        assert!(
            (resumed - 11.2).abs() < 1e-9,
            "BGM should resume from the held position, got {resumed}"
        );
    }

    #[test]
    fn replay_during_fade_uses_the_requested_position() {
        let mut audio = Audio::new(1, 1);
        audio.bgm_play("theme", 100, 100, 0.0, 0);
        audio.update(11_500);
        audio.bgm_fade(2000, 0);
        audio.bgm_play("theme", 100, 100, 0.0, 0);
        assert_eq!(audio.bgm_pos(0), 0.0);
        assert_eq!(audio.bgm_volume(0), 100);

        audio.update(1000);
        audio.bgm_play("theme", 40, 100, 0.0, 0);
        let pos = audio.bgm_pos(0);
        assert!(
            (pos - 1.0).abs() < 1e-9,
            "same file without a fade keeps position, got {pos}"
        );
        assert_eq!(audio.bgm_volume(0), 40);
    }

    #[test]
    fn sound_effects_respect_the_source_limit() {
        let mut audio = Audio::new(1, 2);
        audio.se_play("a", 80, 100);
        audio.se_play("b", 80, 100);
        audio.se_play("c", 80, 100);
        assert_eq!(audio.se_count(), 2);
        audio.se_stop();
        assert_eq!(audio.se_count(), 0);
    }
}
