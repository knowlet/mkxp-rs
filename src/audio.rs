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
        self.filename = filename.to_string();
        self.volume = volume.clamp(0, 100);
        self.pitch = pitch.max(1);
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
        if !self.playing {
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
        let rate = self.pitch as f64 / 100.0;
        self.pos += (dt_ms as f64 / 1000.0) * rate;
        if self.fade_ms > 0 {
            self.fade_left -= dt_ms;
            if self.fade_left <= 0 {
                self.volume = 0;
                self.stop();
            } else {
                let t = self.fade_left as f64 / self.fade_ms as f64;
                self.volume = (self.fade_from as f64 * t).round().clamp(0.0, 100.0) as i32;
            }
        }
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
        }
    }

    pub fn bgm_play(&mut self, filename: &str, volume: i32, pitch: i32, pos: f64, track: i32) {
        if let Some(channel) = self.bgm_mut(track) {
            channel.play(filename, volume, pitch, pos);
        }
    }

    pub fn bgm_stop(&mut self, track: i32) {
        if let Some(channel) = self.bgm_mut(track) {
            channel.stop();
        }
    }

    pub fn bgm_fade(&mut self, time_ms: i32, track: i32) {
        if let Some(channel) = self.bgm_mut(track) {
            channel.fade(time_ms);
        }
    }

    pub fn bgm_pos(&self, track: i32) -> f64 {
        self.bgm_ref(track).map(|c| c.pos).unwrap_or(0.0)
    }

    pub fn bgm_volume(&self, track: i32) -> i32 {
        self.bgm_ref(track).map(|c| c.volume).unwrap_or(0)
    }

    pub fn set_bgm_volume(&mut self, volume: i32, track: i32) {
        if let Some(channel) = self.bgm_mut(track) {
            channel.volume = volume.clamp(0, 100);
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
            *channel = Channel::default();
        }
        self.bgs = Channel::default();
        self.me = Channel::default();
        self.se.clear();
    }

    /// Advance playback clocks by `dt_ms`. Called once per presented frame.
    pub fn update(&mut self, dt_ms: i32) {
        for channel in &mut self.bgm {
            channel.advance(dt_ms);
        }
        self.bgs.advance(dt_ms);
        self.me.advance(dt_ms);
        for slot in &mut self.se {
            slot.channel.advance(dt_ms);
        }
    }

    fn track_index(&self, track: i32) -> usize {
        if track < 0 {
            0
        } else {
            (track as usize).min(self.bgm.len().saturating_sub(1))
        }
    }

    fn bgm_ref(&self, track: i32) -> Option<&Channel> {
        self.bgm.get(self.track_index(track))
    }

    fn bgm_mut(&mut self, track: i32) -> Option<&mut Channel> {
        let index = self.track_index(track);
        self.bgm.get_mut(index)
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
        assert!(audio.bgm_volume(0) < 100);
        assert!(!audio.bgm_filename(0).is_empty());
        audio.update(500);
        assert_eq!(audio.bgm_pos(0), 0.0);
        assert_eq!(audio.bgm_volume(0), 0);
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
