use crate::autotile::autotile_piece;
use crate::bitmap::Bitmap;
use crate::blend::{apply_tone_color, quantize_rgb, sprite_blend};
use crate::color::{Color, Tone};
use crate::error::{Error, Result};
use crate::font::Font;
use crate::rect::Rect;
use crate::table::{wrap_index, Table};

/// Stable id for a bitmap, sprite, viewport, plane, window, or tilemap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Handle {
    index: u32,
    generation: u32,
}

struct Slot<T> {
    generation: u32,
    value: Option<T>,
}

impl<T> Slot<T> {
    fn get(&self, handle: Handle) -> Option<&T> {
        if self.generation == handle.generation {
            self.value.as_ref()
        } else {
            None
        }
    }

    fn get_mut(&mut self, handle: Handle) -> Option<&mut T> {
        if self.generation == handle.generation {
            self.value.as_mut()
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub struct Sprite {
    pub bitmap: Option<Handle>,
    pub src_rect: Rect,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub ox: i32,
    pub oy: i32,
    pub zoom_x: f32,
    pub zoom_y: f32,
    pub angle: f32,
    pub mirror: bool,
    pub opacity: i32,
    pub blend_type: i32,
    pub color: Color,
    pub tone: Tone,
    pub bush_depth: i32,
    pub bush_opacity: i32,
    pub visible: bool,
    pub wave_amp: f32,
    pub wave_length: i32,
    pub wave_speed: f32,
    pub wave_phase: f32,
    pub invert: bool,
    viewport: Option<Handle>,
    creation: u64,
}

impl Sprite {
    fn new(viewport: Option<Handle>, creation: u64) -> Self {
        Self {
            bitmap: None,
            src_rect: Rect::default(),
            x: 0,
            y: 0,
            z: 0,
            ox: 0,
            oy: 0,
            zoom_x: 1.0,
            zoom_y: 1.0,
            angle: 0.0,
            mirror: false,
            opacity: 255,
            blend_type: 0,
            color: Color::default(),
            tone: Tone::default(),
            bush_depth: 0,
            bush_opacity: 128,
            visible: true,
            wave_amp: 0.0,
            wave_length: 180,
            wave_speed: 360.0,
            wave_phase: 0.0,
            invert: false,
            viewport,
            creation,
        }
    }

    pub fn update(&mut self) {
        self.wave_phase += self.wave_speed / 180.0;
        self.wave_phase = wrap_f32(self.wave_phase, 360.0);
    }
}

#[derive(Clone, Debug)]
pub struct Viewport {
    pub rect: Rect,
    pub ox: i32,
    pub oy: i32,
    pub z: i32,
    pub visible: bool,
    pub color: Color,
    pub tone: Tone,
    flash_color: Color,
    flash_left: i32,
    flash_duration: i32,
    creation: u64,
}

impl Viewport {
    fn new(width: i32, height: i32, creation: u64) -> Self {
        Self {
            rect: Rect::new(0, 0, width, height),
            ox: 0,
            oy: 0,
            z: 0,
            visible: true,
            color: Color::default(),
            tone: Tone::default(),
            flash_color: Color::default(),
            flash_left: 0,
            flash_duration: 0,
            creation,
        }
    }

    pub fn flash(&mut self, color: Color, duration: i32) {
        if duration < 1 {
            return;
        }
        self.flash_color = color;
        self.flash_duration = duration;
        self.flash_left = duration;
    }

    pub fn update(&mut self) {
        if self.flash_left > 0 {
            self.flash_left -= 1;
        }
    }
}

#[derive(Clone, Debug)]
pub struct Plane {
    pub bitmap: Option<Handle>,
    pub ox: i32,
    pub oy: i32,
    pub zoom_x: f32,
    pub zoom_y: f32,
    pub opacity: i32,
    pub blend_type: i32,
    pub color: Color,
    pub tone: Tone,
    pub z: i32,
    pub visible: bool,
    viewport: Option<Handle>,
    creation: u64,
}

impl Plane {
    fn new(viewport: Option<Handle>, creation: u64) -> Self {
        Self {
            bitmap: None,
            ox: 0,
            oy: 0,
            zoom_x: 1.0,
            zoom_y: 1.0,
            opacity: 255,
            blend_type: 0,
            color: Color::default(),
            tone: Tone::default(),
            z: 0,
            visible: true,
            viewport,
            creation,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Window {
    pub windowskin: Option<Handle>,
    pub contents: Option<Handle>,
    pub cursor_rect: Rect,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub z: i32,
    pub ox: i32,
    pub oy: i32,
    pub opacity: i32,
    pub back_opacity: i32,
    pub contents_opacity: i32,
    pub stretch: bool,
    pub active: bool,
    pub pause: bool,
    pub visible: bool,
    cursor_count: i32,
    viewport: Option<Handle>,
    creation: u64,
}

impl Window {
    fn new(viewport: Option<Handle>, creation: u64) -> Self {
        Self {
            windowskin: None,
            contents: None,
            cursor_rect: Rect::default(),
            x: 0,
            y: 0,
            width: 100,
            height: 100,
            z: 100,
            ox: 0,
            oy: 0,
            opacity: 255,
            back_opacity: 255,
            contents_opacity: 255,
            stretch: true,
            active: true,
            pause: false,
            visible: true,
            cursor_count: 0,
            viewport,
            creation,
        }
    }

    pub fn update(&mut self) {
        self.cursor_count = self.cursor_count.wrapping_add(1);
    }
}

#[derive(Clone, Debug)]
pub struct Tilemap {
    pub tileset: Option<Handle>,
    pub autotiles: [Option<Handle>; 7],
    pub map_data: Option<Table>,
    pub priorities: Option<Table>,
    pub ox: i32,
    pub oy: i32,
    pub opacity: i32,
    pub blend_type: i32,
    pub color: Color,
    pub tone: Tone,
    pub z: i32,
    pub visible: bool,
    ani: u32,
    viewport: Option<Handle>,
    creation: u64,
}

impl Tilemap {
    fn new(viewport: Option<Handle>, creation: u64) -> Self {
        Self {
            tileset: None,
            autotiles: [None; 7],
            map_data: None,
            priorities: None,
            ox: 0,
            oy: 0,
            opacity: 255,
            blend_type: 0,
            color: Color::default(),
            tone: Tone::default(),
            z: 0,
            visible: true,
            ani: 0,
            viewport,
            creation,
        }
    }

    pub fn update(&mut self) {
        self.ani = self.ani.wrapping_add(1);
    }

    pub fn set_autotile(&mut self, index: usize, bitmap: Option<Handle>) {
        if let Some(slot) = self.autotiles.get_mut(index) {
            *slot = bitmap;
        }
    }
}

enum Kind {
    Sprite(usize),
    Plane(usize),
    Window(usize),
    Tilemap(usize),
    Viewport(usize),
}

struct Item {
    z: i32,
    y: i32,
    creation: u64,
    kind: Kind,
}

/// Software `Graphics` module: a frame buffer plus the RGSS scene graph.
pub struct Graphics {
    width: i32,
    height: i32,
    pub frame_rate: i32,
    pub frame_count: i32,
    pub brightness: i32,
    rgss_version: i32,
    screen: Bitmap,
    frozen: Option<Bitmap>,
    bitmaps: Vec<Slot<Bitmap>>,
    sprites: Vec<Slot<Sprite>>,
    viewports: Vec<Slot<Viewport>>,
    planes: Vec<Slot<Plane>>,
    windows: Vec<Slot<Window>>,
    tilemaps: Vec<Slot<Tilemap>>,
    creation: u64,
}

impl Graphics {
    pub fn new(width: i32, height: i32, rgss_version: i32) -> Result<Self> {
        let font = Font::for_version(rgss_version);
        let screen = Bitmap::with_font(width, height, font)?;
        let frame_rate = if rgss_version == 1 { 40 } else { 60 };
        Ok(Self {
            width,
            height,
            frame_rate,
            frame_count: 0,
            brightness: 255,
            rgss_version,
            screen,
            frozen: None,
            bitmaps: Vec::new(),
            sprites: Vec::new(),
            viewports: Vec::new(),
            planes: Vec::new(),
            windows: Vec::new(),
            tilemaps: Vec::new(),
            creation: 1,
        })
    }

    pub fn width(&self) -> i32 {
        self.width
    }
    pub fn height(&self) -> i32 {
        self.height
    }
    pub fn frame(&self) -> &Bitmap {
        &self.screen
    }

    pub fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        self.screen = Bitmap::with_font(width, height, Font::for_version(self.rgss_version))?;
        self.width = width;
        self.height = height;
        self.frozen = None;
        Ok(())
    }

    pub fn update(&mut self) -> Result<()> {
        if self.frozen.is_some() {
            return Ok(());
        }
        self.present_live()?;
        Ok(())
    }

    pub fn wait(&mut self, duration: i32) -> Result<()> {
        for _ in 0..duration.max(0) {
            self.update()?;
        }
        Ok(())
    }

    pub fn freeze(&mut self) -> Result<()> {
        // Brightness is part of the captured frame, as in `compositeToBuffer`.
        // Later fades present that frame as stored.
        let frame = self.composite()?;
        self.screen = frame.clone();
        self.frozen = Some(frame);
        Ok(())
    }

    pub fn transition(
        &mut self,
        duration: i32,
        trans_map: Option<Handle>,
        vague: i32,
    ) -> Result<()> {
        let Some(frozen) = self.frozen.clone() else {
            return Ok(());
        };
        // mkxp's Graphics::transition calls setBrightness(255) before the
        // new scene is captured, so a fadeout does not freeze the target black.
        self.brightness = 255;
        let current = self.composite()?;
        let vague = (vague.clamp(1, 256) as f32) / 256.0;
        let map = trans_map.and_then(|h| self.bitmap(h).ok()).cloned();
        let frames = duration.max(0);
        if frames == 0 {
            self.frozen = None;
            self.screen = current;
            return Ok(());
        }
        for i in 0..frames {
            let prog = i as f32 / frames as f32;
            self.screen = mix_transition(&frozen, &current, map.as_ref(), prog, vague)?;
            self.frame_count += 1;
        }
        self.frozen = None;
        Ok(())
    }

    pub fn fadeout(&mut self, duration: i32) -> Result<()> {
        self.fade(duration, true)
    }
    pub fn fadein(&mut self, duration: i32) -> Result<()> {
        self.fade(duration, false)
    }

    pub fn snap(&self) -> Result<Bitmap> {
        self.composite()
    }

    pub fn create_bitmap(&mut self, width: i32, height: i32) -> Result<Handle> {
        let bitmap = Bitmap::with_font(width, height, Font::for_version(self.rgss_version))?;
        Ok(self.insert_bitmap(bitmap))
    }

    pub fn bitmap(&self, handle: Handle) -> Result<&Bitmap> {
        self.bitmaps
            .get(handle.index as usize)
            .and_then(|slot| slot.get(handle))
            .ok_or_else(|| Error::new("disposed bitmap"))
    }

    pub fn bitmap_mut(&mut self, handle: Handle) -> Result<&mut Bitmap> {
        self.bitmaps
            .get_mut(handle.index as usize)
            .and_then(|slot| slot.get_mut(handle))
            .ok_or_else(|| Error::new("disposed bitmap"))
    }

    pub fn dispose_bitmap(&mut self, handle: Handle) {
        if let Some(slot) = self.bitmaps.get_mut(handle.index as usize) {
            if slot.generation == handle.generation {
                if let Some(bitmap) = slot.value.as_mut() {
                    bitmap.dispose();
                }
                slot.value = None;
                slot.generation = slot.generation.wrapping_add(1);
            }
        }
    }

    pub fn create_sprite(&mut self, viewport: Option<Handle>) -> Handle {
        let creation = self.bump();
        let sprite = Sprite::new(viewport, creation);
        push_slot(&mut self.sprites, sprite)
    }

    pub fn sprite(&self, handle: Handle) -> Result<&Sprite> {
        self.sprites
            .get(handle.index as usize)
            .and_then(|slot| slot.get(handle))
            .ok_or_else(|| Error::new("disposed sprite"))
    }

    pub fn sprite_mut(&mut self, handle: Handle) -> Result<&mut Sprite> {
        self.sprites
            .get_mut(handle.index as usize)
            .and_then(|slot| slot.get_mut(handle))
            .ok_or_else(|| Error::new("disposed sprite"))
    }

    pub fn set_sprite_bitmap(&mut self, sprite: Handle, bitmap: Option<Handle>) -> Result<()> {
        let rect = match bitmap {
            Some(bitmap) => self.bitmap(bitmap)?.rect(),
            None => Rect::default(),
        };
        let sprite = self.sprite_mut(sprite)?;
        sprite.bitmap = bitmap;
        sprite.src_rect = rect;
        Ok(())
    }

    pub fn create_viewport(&mut self, rect: Rect) -> Handle {
        let creation = self.bump();
        let mut viewport = Viewport::new(self.width, self.height, creation);
        viewport.rect = rect;
        push_slot(&mut self.viewports, viewport)
    }

    pub fn viewport_mut(&mut self, handle: Handle) -> Result<&mut Viewport> {
        self.viewports
            .get_mut(handle.index as usize)
            .and_then(|slot| slot.get_mut(handle))
            .ok_or_else(|| Error::new("disposed viewport"))
    }

    pub fn create_plane(&mut self, viewport: Option<Handle>) -> Handle {
        let creation = self.bump();
        let plane = Plane::new(viewport, creation);
        push_slot(&mut self.planes, plane)
    }

    pub fn plane_mut(&mut self, handle: Handle) -> Result<&mut Plane> {
        self.planes
            .get_mut(handle.index as usize)
            .and_then(|slot| slot.get_mut(handle))
            .ok_or_else(|| Error::new("disposed plane"))
    }

    pub fn create_window(&mut self, viewport: Option<Handle>) -> Handle {
        let creation = self.bump();
        let window = Window::new(viewport, creation);
        push_slot(&mut self.windows, window)
    }

    pub fn window_mut(&mut self, handle: Handle) -> Result<&mut Window> {
        self.windows
            .get_mut(handle.index as usize)
            .and_then(|slot| slot.get_mut(handle))
            .ok_or_else(|| Error::new("disposed window"))
    }

    pub fn create_tilemap(&mut self, viewport: Option<Handle>) -> Handle {
        let creation = self.bump();
        let tilemap = Tilemap::new(viewport, creation);
        push_slot(&mut self.tilemaps, tilemap)
    }

    pub fn tilemap_mut(&mut self, handle: Handle) -> Result<&mut Tilemap> {
        self.tilemaps
            .get_mut(handle.index as usize)
            .and_then(|slot| slot.get_mut(handle))
            .ok_or_else(|| Error::new("disposed tilemap"))
    }

    fn present_live(&mut self) -> Result<()> {
        let frame = self.composite()?;
        self.screen = frame;
        self.frame_count += 1;
        Ok(())
    }

    fn fade(&mut self, duration: i32, out: bool) -> Result<()> {
        let curr = self.brightness as f32;
        let diff = 255.0 - curr;
        if duration <= 0 {
            return Ok(());
        }
        if out {
            for i in (0..duration).rev() {
                let value = diff + (curr / duration as f32) * i as f32;
                self.brightness = (value as i32).clamp(0, 255);
                self.present_fade()?;
            }
        } else {
            for i in 1..=duration {
                let value = curr + (diff / duration as f32) * i as f32;
                self.brightness = (value as i32).clamp(0, 255);
                self.present_fade()?;
            }
        }
        Ok(())
    }

    fn present_fade(&mut self) -> Result<()> {
        if let Some(frozen) = self.frozen.clone() {
            self.screen = frozen;
            self.frame_count += 1;
        } else {
            self.present_live()?;
        }
        Ok(())
    }

    fn composite(&self) -> Result<Bitmap> {
        let mut frame = Bitmap::with_font(
            self.width,
            self.height,
            Font::for_version(self.rgss_version),
        )?;
        frame.fill_rect(frame.rect(), Color::new(0.0, 0.0, 0.0, 255.0));
        let clip = Rect::new(0, 0, self.width, self.height);
        self.draw_items(&mut frame, None, clip, 0, 0);
        apply_brightness(&mut frame, self.brightness);
        Ok(frame)
    }

    fn draw_items(
        &self,
        frame: &mut Bitmap,
        viewport: Option<Handle>,
        clip: Rect,
        origin_x: i32,
        origin_y: i32,
    ) {
        let mut items = Vec::new();
        for (index, slot) in self.sprites.iter().enumerate() {
            if let Some(sprite) = slot.value.as_ref() {
                if sprite.viewport == viewport {
                    items.push(Item {
                        z: sprite.z,
                        y: sprite.y,
                        creation: sprite.creation,
                        kind: Kind::Sprite(index),
                    });
                }
            }
        }
        for (index, slot) in self.planes.iter().enumerate() {
            if let Some(plane) = slot.value.as_ref() {
                if plane.viewport == viewport {
                    items.push(Item {
                        z: plane.z,
                        y: 0,
                        creation: plane.creation,
                        kind: Kind::Plane(index),
                    });
                }
            }
        }
        for (index, slot) in self.windows.iter().enumerate() {
            if let Some(window) = slot.value.as_ref() {
                if window.viewport == viewport {
                    items.push(Item {
                        z: window.z,
                        y: 0,
                        creation: window.creation,
                        kind: Kind::Window(index),
                    });
                }
            }
        }
        for (index, slot) in self.tilemaps.iter().enumerate() {
            if let Some(tilemap) = slot.value.as_ref() {
                if tilemap.viewport == viewport {
                    items.push(Item {
                        z: tilemap.z,
                        y: 0,
                        creation: tilemap.creation,
                        kind: Kind::Tilemap(index),
                    });
                }
            }
        }
        if viewport.is_none() {
            for (index, slot) in self.viewports.iter().enumerate() {
                if let Some(view) = slot.value.as_ref() {
                    items.push(Item {
                        z: view.z,
                        y: 0,
                        creation: view.creation,
                        kind: Kind::Viewport(index),
                    });
                }
            }
        }
        items.sort_by(|a, b| {
            let order = a.z.cmp(&b.z);
            if self.rgss_version >= 2 {
                order.then(a.y.cmp(&b.y))
            } else {
                order
            }
            .then(a.creation.cmp(&b.creation))
        });
        for item in items {
            match item.kind {
                Kind::Sprite(index) => {
                    if let Some(sprite) = self.sprites[index].value.as_ref() {
                        self.draw_sprite(frame, sprite, clip, origin_x, origin_y);
                    }
                }
                Kind::Plane(index) => {
                    if let Some(plane) = self.planes[index].value.as_ref() {
                        self.draw_plane(frame, plane, clip, origin_x, origin_y);
                    }
                }
                Kind::Window(index) => {
                    if let Some(window) = self.windows[index].value.as_ref() {
                        self.draw_window(frame, window, clip, origin_x, origin_y);
                    }
                }
                Kind::Tilemap(index) => {
                    if let Some(tilemap) = self.tilemaps[index].value.as_ref() {
                        self.draw_tilemap(frame, tilemap, clip, origin_x, origin_y);
                    }
                }
                Kind::Viewport(index) => {
                    let Some(view) = self.viewports[index].value.as_ref() else {
                        continue;
                    };
                    if !view.visible {
                        continue;
                    }
                    let handle = Handle {
                        index: index as u32,
                        generation: self.viewports[index].generation,
                    };
                    let child_clip = view.rect.intersect(clip);
                    let child_ox = view.rect.x - view.ox;
                    let child_oy = view.rect.y - view.oy;
                    self.draw_items(frame, Some(handle), child_clip, child_ox, child_oy);
                    let flash = if view.flash_left > 0 && view.flash_duration > 0 {
                        let scale = view.flash_left as f32 / view.flash_duration as f32;
                        Some((view.flash_color, scale))
                    } else {
                        None
                    };
                    apply_viewport_fx(frame, child_clip, view.tone, view.color, flash);
                }
            }
        }
    }

    fn draw_sprite(
        &self,
        frame: &mut Bitmap,
        sprite: &Sprite,
        clip: Rect,
        origin_x: i32,
        origin_y: i32,
    ) {
        if !sprite.visible || sprite.opacity <= 0 || sprite.zoom_x == 0.0 || sprite.zoom_y == 0.0 {
            return;
        }
        let Some(handle) = sprite.bitmap else { return };
        let Ok(bitmap) = self.bitmap(handle) else {
            return;
        };
        if bitmap.is_disposed() || sprite.src_rect.width == 0 || sprite.src_rect.height == 0 {
            return;
        }
        let xf = Xform::new(
            sprite.angle,
            sprite.zoom_x,
            sprite.zoom_y,
            sprite.ox,
            sprite.oy,
            origin_x + sprite.x,
            origin_y + sprite.y,
        );
        let src_w = sprite.src_rect.width.abs() as f32;
        let src_h = sprite.src_rect.height.abs() as f32;
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for (lx, ly) in [(0.0, 0.0), (src_w, 0.0), (0.0, src_h), (src_w, src_h)] {
            let (x, y) = xf.to_screen(lx, ly);
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
        let pad = sprite.wave_amp.abs() + 1.0;
        let left = (min_x - pad).floor();
        let top = (min_y - pad).floor();
        let right = left + ((max_x - min_x) + pad * 2.0).ceil() + 1.0;
        let bottom = top + ((max_y - min_y) + pad * 2.0).ceil() + 1.0;
        let Some((x0, x1)) = cover_pixels(left, right, clip.x, clip.right()) else {
            return;
        };
        let Some((y0, y1)) = cover_pixels(top, bottom, clip.y, clip.bottom()) else {
            return;
        };
        let tone = sprite.tone.norm();
        let color = sprite.color.norm();
        for y in y0..y1 {
            for x in x0..x1 {
                let Some((lx, ly)) = xf.to_local(x as f32 + 0.5, y as f32 + 0.5) else {
                    continue;
                };
                let shift = wave_shift(sprite, ly);
                let lx = lx - shift;
                if lx < 0.0 || ly < 0.0 || lx >= src_w || ly >= src_h {
                    continue;
                }
                let mut ix = lx.floor() as i32;
                let iy = ly.floor() as i32;
                if sprite.mirror {
                    ix = src_w as i32 - 1 - ix;
                }
                let sx = if sprite.src_rect.width >= 0 {
                    sprite.src_rect.x + ix
                } else {
                    sprite.src_rect.x + sprite.src_rect.width + ix
                };
                let sy = if sprite.src_rect.height >= 0 {
                    sprite.src_rect.y + iy
                } else {
                    sprite.src_rect.y + sprite.src_rect.height + iy
                };
                let Some(sample) = pixel_of(bitmap, sx, sy) else {
                    continue;
                };
                if sample[3] == 0 {
                    continue;
                }
                let mut rgb = [
                    sample[0] as f32 / 255.0,
                    sample[1] as f32 / 255.0,
                    sample[2] as f32 / 255.0,
                ];
                rgb = apply_tone_color(rgb, tone, color);
                if sprite.invert {
                    rgb = [1.0 - rgb[0], 1.0 - rgb[1], 1.0 - rgb[2]];
                }
                let mut alpha = sample[3] as f32 * (sprite.opacity.clamp(0, 255) as f32 / 255.0);
                if sprite.bush_depth > 0 && iy >= src_h as i32 - sprite.bush_depth {
                    alpha *= sprite.bush_opacity.clamp(0, 255) as f32 / 255.0;
                }
                let rgb = quantize_rgb(rgb);
                let src = [
                    rgb[0],
                    rgb[1],
                    rgb[2],
                    alpha.round().clamp(0.0, 255.0) as u8,
                ];
                if let Some(dst) = pixel_of(frame, x, y) {
                    put_pixel(frame, x, y, sprite_blend(dst, src, sprite.blend_type));
                }
            }
        }
    }

    fn draw_plane(
        &self,
        frame: &mut Bitmap,
        plane: &Plane,
        clip: Rect,
        origin_x: i32,
        origin_y: i32,
    ) {
        if !plane.visible || plane.opacity <= 0 || plane.zoom_x == 0.0 || plane.zoom_y == 0.0 {
            return;
        }
        let Some(handle) = plane.bitmap else { return };
        let Ok(bitmap) = self.bitmap(handle) else {
            return;
        };
        if bitmap.width() <= 0 || bitmap.height() <= 0 {
            return;
        }
        let tone = plane.tone.norm();
        let color = plane.color.norm();
        for y in clip.y..clip.bottom() {
            for x in clip.x..clip.right() {
                let lx = (x - origin_x) as f32 + plane.ox as f32;
                let ly = (y - origin_y) as f32 + plane.oy as f32;
                let sx = wrap_floor(lx / plane.zoom_x, bitmap.width());
                let sy = wrap_floor(ly / plane.zoom_y, bitmap.height());
                let Some(sample) = pixel_of(bitmap, sx, sy) else {
                    continue;
                };
                if sample[3] == 0 {
                    continue;
                }
                let rgb = quantize_rgb(apply_tone_color(
                    [
                        sample[0] as f32 / 255.0,
                        sample[1] as f32 / 255.0,
                        sample[2] as f32 / 255.0,
                    ],
                    tone,
                    color,
                ));
                let alpha = (sample[3] as f32 * (plane.opacity.clamp(0, 255) as f32 / 255.0))
                    .round()
                    .clamp(0.0, 255.0) as u8;
                if let Some(dst) = pixel_of(frame, x, y) {
                    put_pixel(
                        frame,
                        x,
                        y,
                        sprite_blend(dst, [rgb[0], rgb[1], rgb[2], alpha], plane.blend_type),
                    );
                }
            }
        }
    }

    fn draw_window(
        &self,
        frame: &mut Bitmap,
        window: &Window,
        clip: Rect,
        origin_x: i32,
        origin_y: i32,
    ) {
        if !window.visible || window.width <= 0 || window.height <= 0 {
            return;
        }
        let Ok(mut skin_target) = Bitmap::new(window.width, window.height) else {
            return;
        };
        if let Some(handle) = window.windowskin {
            if let Ok(skin) = self.bitmap(handle) {
                // Background keeps `back_opacity` only. The frame is composited
                // at full strength, and `opacity` is applied once to that result.
                let back = window.back_opacity.clamp(0, 255);
                let bg = Rect::new(2, 2, window.width - 4, window.height - 4);
                if window.stretch {
                    skin_target.stretch_blt(bg, skin, Rect::new(0, 0, 128, 128), back, false);
                } else {
                    blit_tiled(&mut skin_target, bg, skin, Rect::new(0, 0, 128, 128), back);
                }
                blit_tiled(
                    &mut skin_target,
                    Rect::new(8, 0, window.width - 16, 16),
                    skin,
                    Rect::new(144, 0, 32, 16),
                    255,
                );
                blit_tiled(
                    &mut skin_target,
                    Rect::new(8, window.height - 16, window.width - 16, 16),
                    skin,
                    Rect::new(144, 48, 32, 16),
                    255,
                );
                blit_tiled(
                    &mut skin_target,
                    Rect::new(0, 8, 16, window.height - 16),
                    skin,
                    Rect::new(128, 16, 16, 32),
                    255,
                );
                blit_tiled(
                    &mut skin_target,
                    Rect::new(window.width - 16, 8, 16, window.height - 16),
                    skin,
                    Rect::new(176, 16, 16, 32),
                    255,
                );
                let corners = [
                    (0, 0, 128, 0),
                    (window.width - 16, 0, 176, 0),
                    (0, window.height - 16, 128, 48),
                    (window.width - 16, window.height - 16, 176, 48),
                ];
                for (dx, dy, sx, sy) in corners {
                    skin_target.blt(dx, dy, skin, Rect::new(sx, sy, 16, 16), 255);
                }
            }
        }
        let window_x = origin_x + window.x;
        let window_y = origin_y + window.y;
        stamp(
            frame,
            &skin_target,
            window_x,
            window_y,
            clip,
            window.opacity,
        );
        if let Some(handle) = window.windowskin {
            if let Ok(skin) = self.bitmap(handle) {
                if window.cursor_rect.width > 0 && window.cursor_rect.height > 0 {
                    let alpha = CURSOR_ALPHA[window.cursor_count as usize % CURSOR_ALPHA.len()];
                    if let Ok(mut cursor) =
                        Bitmap::new(window.cursor_rect.width, window.cursor_rect.height)
                    {
                        cursor.stretch_blt(
                            cursor.rect(),
                            skin,
                            Rect::new(128, 64, 32, 32),
                            alpha as i32,
                            false,
                        );
                        stamp(
                            frame,
                            &cursor,
                            window_x + window.cursor_rect.x + 16,
                            window_y + window.cursor_rect.y + 16,
                            clip,
                            255,
                        );
                    }
                }
                if window.pause {
                    let frame_i = (window.cursor_count / 8) as usize % PAUSE_SRC.len();
                    let (sx, sy) = PAUSE_SRC[frame_i];
                    if let Ok(mut pause) = Bitmap::new(16, 16) {
                        pause.blt(0, 0, skin, Rect::new(sx, sy, 16, 16), 255);
                        stamp(
                            frame,
                            &pause,
                            window_x + (window.width - 16) / 2,
                            window_y + window.height - 16,
                            clip,
                            255,
                        );
                    }
                }
            }
        }
        if let Some(handle) = window.contents {
            if let Ok(contents) = self.bitmap(handle) {
                let dest = Rect::new(
                    window_x + 16,
                    window_y + 16,
                    window.width - 32,
                    window.height - 32,
                );
                let src = Rect::new(window.ox, window.oy, dest.width, dest.height);
                blit_clipped(
                    frame,
                    dest,
                    contents,
                    src,
                    window.contents_opacity.clamp(0, 255),
                    clip,
                );
            }
        }
    }

    fn draw_tilemap(
        &self,
        frame: &mut Bitmap,
        tilemap: &Tilemap,
        clip: Rect,
        origin_x: i32,
        origin_y: i32,
    ) {
        if !tilemap.visible || tilemap.opacity <= 0 {
            return;
        }
        let Some(map) = tilemap.map_data.as_ref() else {
            return;
        };
        if map.x_size() <= 0 || map.y_size() <= 0 || map.z_size() <= 0 {
            return;
        }
        let tiles_x = clip.width as i64 / 32 + 3;
        let tiles_y = clip.height as i64 / 32 + 3;
        // Viewport scroll lives in `origin_*`. The first visible column is
        // `(clip - origin + tilemap offset) / 32`, with one tile of margin.
        // The sum is wider than `i32` so extreme scroll values do not wrap.
        let start_x = tile_start(clip.x, origin_x, tilemap.ox);
        let start_y = tile_start(clip.y, origin_y, tilemap.oy);
        let mut ground = Vec::new();
        let mut above = Vec::new();
        for ty in start_y..start_y + tiles_y {
            for tx in start_x..start_x + tiles_x {
                let mx = wrap_index(tx, map.x_size());
                let my = wrap_index(ty, map.y_size());
                for z in 0..map.z_size() {
                    let Some(tile) = map.get(mx, my, z) else {
                        continue;
                    };
                    let tile = tile as i32;
                    if tile < 48 {
                        continue;
                    }
                    let prio = tilemap
                        .priorities
                        .as_ref()
                        .and_then(|table| table.get(tile, 0, 0))
                        .unwrap_or(0);
                    if prio > 5 {
                        continue;
                    }
                    let dest_x = origin_x as i64 + tx * 32 - tilemap.ox as i64;
                    let dest_y = origin_y as i64 + ty * 32 - tilemap.oy as i64;
                    if dest_x + 32 <= clip.x as i64
                        || dest_y + 32 <= clip.y as i64
                        || dest_x >= clip.right() as i64
                        || dest_y >= clip.bottom() as i64
                    {
                        continue;
                    }
                    let job = TileJob {
                        tile,
                        dest_x,
                        dest_y,
                        prio,
                    };
                    if prio <= 0 {
                        ground.push(job);
                    } else {
                        above.push(job);
                    }
                }
            }
        }
        above.sort_by_key(|job| job.dest_y + job.prio as i64 * 32);
        for job in ground.into_iter().chain(above) {
            self.draw_tile(frame, tilemap, clip, job);
        }
    }

    fn draw_tile(&self, frame: &mut Bitmap, tilemap: &Tilemap, clip: Rect, job: TileJob) {
        if job.tile < 48 * 8 {
            let index = job.tile / 48 - 1;
            if !(0..7).contains(&index) {
                return;
            }
            let Some(handle) = tilemap.autotiles[index as usize] else {
                return;
            };
            let Ok(bitmap) = self.bitmap(handle) else {
                return;
            };
            let (frame_w, frames) = if bitmap.height() <= 32 {
                (32, (bitmap.width() / 32).max(1))
            } else {
                (96, (bitmap.width() / 96).max(1))
            };
            let frame_i = if frames <= 1 {
                0
            } else {
                (tilemap.ani / 15) as i32 % frames
            };
            if bitmap.height() <= 32 {
                paint_tile(
                    frame,
                    bitmap,
                    Rect::new(frame_i * frame_w, 0, 32, 32),
                    (job.dest_x, job.dest_y),
                    clip,
                    tilemap,
                );
            } else {
                let pattern = (job.tile % 48) as usize;
                for corner in 0..4 {
                    let (sx, sy) = autotile_piece(pattern, corner);
                    let dx = job.dest_x + if corner % 2 == 1 { 16i64 } else { 0 };
                    let dy = job.dest_y + if corner >= 2 { 16i64 } else { 0 };
                    paint_tile(
                        frame,
                        bitmap,
                        Rect::new(sx + frame_i * frame_w, sy, 16, 16),
                        (dx, dy),
                        clip,
                        tilemap,
                    );
                }
            }
        } else if let Some(handle) = tilemap.tileset {
            if let Ok(bitmap) = self.bitmap(handle) {
                let ts = job.tile - 48 * 8;
                let tile_x = ts % 8;
                let tile_y = ts / 8;
                paint_tile(
                    frame,
                    bitmap,
                    Rect::new(tile_x * 32, tile_y * 32, 32, 32),
                    (job.dest_x, job.dest_y),
                    clip,
                    tilemap,
                );
            }
        }
    }

    fn insert_bitmap(&mut self, bitmap: Bitmap) -> Handle {
        push_slot(&mut self.bitmaps, bitmap)
    }

    fn bump(&mut self) -> u64 {
        let value = self.creation;
        self.creation += 1;
        value
    }
}

fn push_slot<T>(slots: &mut Vec<Slot<T>>, value: T) -> Handle {
    let index = slots.len() as u32;
    let generation = 1;
    slots.push(Slot {
        generation,
        value: Some(value),
    });
    Handle { index, generation }
}

struct TileJob {
    tile: i32,
    dest_x: i64,
    dest_y: i64,
    prio: i16,
}

struct Xform {
    cos: f32,
    sin: f32,
    zx: f32,
    zy: f32,
    tx: f32,
    ty: f32,
}

impl Xform {
    fn new(angle_deg: f32, zoom_x: f32, zoom_y: f32, ox: i32, oy: i32, x: i32, y: i32) -> Self {
        let angle = angle_deg.to_radians();
        let (sin, cos) = angle.sin_cos();
        let sxc = zoom_x * cos;
        let syc = zoom_y * cos;
        let sxs = zoom_x * sin;
        let sys = zoom_y * sin;
        let ox = ox as f32;
        let oy = oy as f32;
        Self {
            cos,
            sin,
            zx: zoom_x,
            zy: zoom_y,
            tx: -ox * sxc - oy * sys + x as f32,
            ty: ox * sxs - oy * syc + y as f32,
        }
    }

    fn to_screen(&self, lx: f32, ly: f32) -> (f32, f32) {
        let sxc = self.zx * self.cos;
        let syc = self.zy * self.cos;
        let sxs = self.zx * self.sin;
        let sys = self.zy * self.sin;
        (
            sxc * lx + sys * ly + self.tx,
            -sxs * lx + syc * ly + self.ty,
        )
    }

    fn to_local(&self, sx: f32, sy: f32) -> Option<(f32, f32)> {
        if self.zx == 0.0 || self.zy == 0.0 {
            return None;
        }
        let dx = sx - self.tx;
        let dy = sy - self.ty;
        let lx = (self.cos * dx - self.sin * dy) / self.zx;
        let ly = (self.sin * dx + self.cos * dy) / self.zy;
        Some((lx, ly))
    }
}

fn wave_shift(sprite: &Sprite, local_y: f32) -> f32 {
    if sprite.wave_amp == 0.0 || sprite.wave_length == 0 || sprite.zoom_x == 0.0 {
        return 0.0;
    }
    let phase = sprite.wave_phase.to_radians();
    let pos = phase + (local_y / sprite.wave_length as f32) * std::f32::consts::TAU;
    pos.sin() * sprite.wave_amp / sprite.zoom_x
}

fn wrap_f32(value: f32, range: f32) -> f32 {
    if range == 0.0 {
        return 0.0;
    }
    let mut v = value % range;
    if v < 0.0 {
        v += range;
    }
    v
}

fn wrap_floor(value: f32, range: i32) -> i32 {
    if range <= 0 {
        return 0;
    }
    let wrapped = value.rem_euclid(range as f32);
    wrapped.floor() as i32
}

fn pixel_of(bitmap: &Bitmap, x: i32, y: i32) -> Option<[u8; 4]> {
    if x < 0 || y < 0 || x >= bitmap.width() || y >= bitmap.height() {
        return None;
    }
    let i = ((y as usize) * (bitmap.width() as usize) + (x as usize)) * 4;
    let px = bitmap.pixels();
    Some([px[i], px[i + 1], px[i + 2], px[i + 3]])
}

fn put_pixel(bitmap: &mut Bitmap, x: i32, y: i32, px: [u8; 4]) {
    bitmap.set_pixel(x, y, Color::from_rgba8(px));
}

fn blit_tiled(
    dest: &mut Bitmap,
    dest_rect: Rect,
    source: &Bitmap,
    source_rect: Rect,
    opacity: i32,
) {
    if dest_rect.width <= 0
        || dest_rect.height <= 0
        || source_rect.width <= 0
        || source_rect.height <= 0
    {
        return;
    }
    let mut y = dest_rect.y;
    while y < dest_rect.bottom() {
        let mut x = dest_rect.x;
        let h = source_rect.height.min(dest_rect.bottom() - y);
        while x < dest_rect.right() {
            let w = source_rect.width.min(dest_rect.right() - x);
            dest.blt(
                x,
                y,
                source,
                Rect::new(source_rect.x, source_rect.y, w, h),
                opacity,
            );
            x += source_rect.width;
        }
        y += source_rect.height;
    }
}

fn blit_clipped(
    dest: &mut Bitmap,
    dest_rect: Rect,
    source: &Bitmap,
    source_rect: Rect,
    opacity: i32,
    clip: Rect,
) {
    if dest_rect.width <= 0 || dest_rect.height <= 0 {
        return;
    }
    let src = Rect::new(
        source_rect.x,
        source_rect.y,
        source_rect.width.min(source.width() - source_rect.x).max(0),
        source_rect
            .height
            .min(source.height() - source_rect.y)
            .max(0),
    );
    if src.is_empty() {
        return;
    }
    let mut temp = match Bitmap::new(dest_rect.width.max(1), dest_rect.height.max(1)) {
        Ok(bitmap) => bitmap,
        Err(_) => return,
    };
    temp.blt(0, 0, source, src, opacity);
    // `blt` keeps straight alpha. One sprite blend onto the framebuffer then
    // applies that alpha a single time.
    stamp(
        dest,
        &temp,
        dest_rect.x,
        dest_rect.y,
        clip.intersect(dest.rect()),
        255,
    );
}

fn stamp(dest: &mut Bitmap, source: &Bitmap, x: i32, y: i32, clip: Rect, opacity: i32) {
    let opacity = opacity.clamp(0, 255);
    if opacity == 0 {
        return;
    }
    let rect = Rect::new(x, y, source.width(), source.height()).intersect(clip);
    for py in rect.y..rect.bottom() {
        for px in rect.x..rect.right() {
            let Some(mut src) = pixel_of(source, px - x, py - y) else {
                continue;
            };
            if opacity != 255 {
                src[3] = ((src[3] as u16 * opacity as u16) / 255) as u8;
            }
            if src[3] == 0 {
                continue;
            }
            if let Some(dst) = pixel_of(dest, px, py) {
                put_pixel(dest, px, py, sprite_blend(dst, src, 0));
            }
        }
    }
}

struct TileBlit<'a> {
    frame: &'a mut Bitmap,
    source: &'a Bitmap,
    src: Rect,
    x: i64,
    y: i64,
    clip: Rect,
    opacity: i32,
    blend: i32,
    tone: Tone,
    color: Color,
}

fn paint_tile(
    frame: &mut Bitmap,
    source: &Bitmap,
    src: Rect,
    at: (i64, i64),
    clip: Rect,
    tilemap: &Tilemap,
) {
    blit_sprite_rect(TileBlit {
        frame,
        source,
        src,
        x: at.0,
        y: at.1,
        clip,
        opacity: tilemap.opacity,
        blend: tilemap.blend_type,
        tone: tilemap.tone,
        color: tilemap.color,
    });
}

fn blit_sprite_rect(args: TileBlit<'_>) {
    let TileBlit {
        frame,
        source,
        src,
        x,
        y,
        clip,
        opacity,
        blend,
        tone,
        color,
    } = args;
    let Some(rect) = clip_tile(x, y, src.width, src.height, clip) else {
        return;
    };
    let tone = tone.norm();
    let color = color.norm();
    for py in rect.y..rect.bottom() {
        for px in rect.x..rect.right() {
            let sx = src.x as i64 + (px as i64 - x);
            let sy = src.y as i64 + (py as i64 - y);
            let (Ok(sx), Ok(sy)) = (i32::try_from(sx), i32::try_from(sy)) else {
                continue;
            };
            let Some(sample) = pixel_of(source, sx, sy) else {
                continue;
            };
            if sample[3] == 0 {
                continue;
            }
            let rgb = quantize_rgb(apply_tone_color(
                [
                    sample[0] as f32 / 255.0,
                    sample[1] as f32 / 255.0,
                    sample[2] as f32 / 255.0,
                ],
                tone,
                color,
            ));
            let alpha = (sample[3] as f32 * (opacity.clamp(0, 255) as f32 / 255.0))
                .round()
                .clamp(0.0, 255.0) as u8;
            if let Some(dst) = pixel_of(frame, px, py) {
                put_pixel(
                    frame,
                    px,
                    py,
                    sprite_blend(dst, [rgb[0], rgb[1], rgb[2], alpha], blend),
                );
            }
        }
    }
}

/// Integer pixels of `[start, end)` that also lie in `[clip_lo, clip_hi)`.
///
/// The float box is clipped before it is converted, so a zoomed sprite whose
/// corners sit far outside the frame still covers the pixels it intersects.
fn cover_pixels(start: f32, end: f32, clip_lo: i32, clip_hi: i32) -> Option<(i32, i32)> {
    if clip_hi <= clip_lo || !start.is_finite() || !end.is_finite() || end <= start {
        return None;
    }
    let clip_lo = clip_lo as i64;
    let clip_hi = clip_hi as i64;
    let lo = if start <= i64::MIN as f32 {
        clip_lo
    } else {
        (start.ceil() as i64).max(clip_lo)
    };
    let hi = if end >= i64::MAX as f32 {
        clip_hi
    } else {
        (end.ceil() as i64).min(clip_hi)
    };
    if hi <= lo {
        None
    } else {
        Some((lo as i32, hi as i32))
    }
}

fn tile_start(clip: i32, origin: i32, offset: i32) -> i64 {
    (clip as i64 - origin as i64 + offset as i64).div_euclid(32) - 1
}

fn clip_tile(x: i64, y: i64, width: i32, height: i32, clip: Rect) -> Option<Rect> {
    if width <= 0 || height <= 0 {
        return None;
    }
    let left = x.max(clip.x as i64);
    let top = y.max(clip.y as i64);
    let right = x.saturating_add(width as i64).min(clip.right() as i64);
    let bottom = y.saturating_add(height as i64).min(clip.bottom() as i64);
    if right <= left || bottom <= top {
        return None;
    }
    Some(Rect::new(
        left as i32,
        top as i32,
        (right - left) as i32,
        (bottom - top) as i32,
    ))
}

fn apply_brightness(frame: &mut Bitmap, brightness: i32) {
    let scale = brightness.clamp(0, 255) as f32 / 255.0;
    if scale >= 1.0 {
        return;
    }
    for y in 0..frame.height() {
        for x in 0..frame.width() {
            if let Some(px) = pixel_of(frame, x, y) {
                let rgb = [
                    (px[0] as f32 * scale).round() as u8,
                    (px[1] as f32 * scale).round() as u8,
                    (px[2] as f32 * scale).round() as u8,
                    px[3],
                ];
                put_pixel(frame, x, y, rgb);
            }
        }
    }
}

fn apply_viewport_fx(
    frame: &mut Bitmap,
    clip: Rect,
    tone: Tone,
    color: Color,
    flash: Option<(Color, f32)>,
) {
    let tone = tone.norm();
    let color = color.norm();
    if tone == [0.0, 0.0, 0.0, 0.0] && color[3] <= 0.0 && flash.is_none() {
        return;
    }
    for y in clip.y..clip.bottom() {
        for x in clip.x..clip.right() {
            let Some(px) = pixel_of(frame, x, y) else {
                continue;
            };
            let mut rgb = [
                px[0] as f32 / 255.0,
                px[1] as f32 / 255.0,
                px[2] as f32 / 255.0,
            ];
            if tone[3] != 0.0 {
                let luma = rgb[0] * 0.299 + rgb[1] * 0.587 + rgb[2] * 0.114;
                for channel in &mut rgb {
                    *channel += (luma - *channel) * tone[3];
                }
            }
            for i in 0..3 {
                if tone[i] > 0.0 {
                    rgb[i] += tone[i];
                }
            }
            for i in 0..3 {
                if tone[i] < 0.0 {
                    rgb[i] += tone[i];
                }
            }
            if color[3] > 0.0 {
                for i in 0..3 {
                    rgb[i] = rgb[i] * (1.0 - color[3]) + color[i] * color[3];
                }
            }
            if let Some((flash_color, scale)) = flash {
                let flash = flash_color.norm();
                let alpha = (flash[3] * scale).clamp(0.0, 1.0);
                for i in 0..3 {
                    rgb[i] = rgb[i] * (1.0 - alpha) + flash[i] * alpha;
                }
            }
            let q = quantize_rgb(rgb);
            put_pixel(frame, x, y, [q[0], q[1], q[2], px[3]]);
        }
    }
}

fn mix_transition(
    frozen: &Bitmap,
    current: &Bitmap,
    map: Option<&Bitmap>,
    prog: f32,
    vague: f32,
) -> Result<Bitmap> {
    let mut out = Bitmap::new(current.width(), current.height())?;
    for y in 0..current.height() {
        for x in 0..current.width() {
            let old = pixel_of(frozen, x, y).unwrap_or([0, 0, 0, 255]);
            let new = pixel_of(current, x, y).unwrap_or([0, 0, 0, 255]);
            let alpha = if let Some(map) = map {
                let mx = if map.width() == 0 {
                    0
                } else {
                    x * map.width() / current.width()
                };
                let my = if map.height() == 0 {
                    0
                } else {
                    y * map.height() / current.height()
                };
                let trans_v = pixel_of(map, mx, my)
                    .map(|px| px[0] as f32 / 255.0)
                    .unwrap_or(0.0);
                let clamped = trans_v.clamp(prog, prog + vague);
                if vague == 0.0 {
                    0.0
                } else {
                    (clamped - prog) / vague
                }
            } else {
                1.0 - prog
            };
            let mut px = [0u8; 4];
            for c in 0..4 {
                let v = new[c] as f32 * (1.0 - alpha) + old[c] as f32 * alpha;
                px[c] = v.round().clamp(0.0, 255.0) as u8;
            }
            put_pixel(&mut out, x, y, px);
        }
    }
    Ok(out)
}

const CURSOR_ALPHA: [u8; 32] = [
    0xFF, 0xF7, 0xEF, 0xE7, 0xDF, 0xD7, 0xCF, 0xC7, 0xBF, 0xB7, 0xAF, 0xA7, 0x9F, 0x97, 0x8F, 0x87,
    0x7F, 0x87, 0x8F, 0x97, 0x9F, 0xA7, 0xAF, 0xB7, 0xBF, 0xC7, 0xCF, 0xD7, 0xDF, 0xE7, 0xEF, 0xF7,
];

const PAUSE_SRC: [(i32, i32); 4] = [(160, 64), (176, 64), (160, 80), (176, 80)];

/// A small built-in scene used by the `mkxpz demo` command.
pub struct Demo {
    pub graphics: Graphics,
    actor: Handle,
}

impl Demo {
    pub fn new() -> Result<Self> {
        let mut graphics = Graphics::new(96, 64, 1)?;
        let background = graphics.create_bitmap(96, 64)?;
        graphics
            .bitmap_mut(background)?
            .fill_rect(Rect::new(0, 0, 96, 64), Color::new(10.0, 20.0, 40.0, 255.0));
        let bg = graphics.create_sprite(None);
        graphics.set_sprite_bitmap(bg, Some(background))?;

        let actor_bmp = graphics.create_bitmap(8, 8)?;
        graphics
            .bitmap_mut(actor_bmp)?
            .fill_rect(Rect::new(0, 0, 8, 8), Color::rgb(255.0, 0.0, 0.0));
        let actor = graphics.create_sprite(None);
        graphics.set_sprite_bitmap(actor, Some(actor_bmp))?;
        {
            let sprite = graphics.sprite_mut(actor)?;
            sprite.x = 10;
            sprite.y = 10;
            sprite.z = 20;
        }

        let skin = graphics.create_bitmap(192, 128)?;
        graphics
            .bitmap_mut(skin)?
            .fill_rect(Rect::new(0, 0, 128, 128), Color::rgb(0.0, 90.0, 40.0));
        graphics
            .bitmap_mut(skin)?
            .fill_rect(Rect::new(128, 0, 64, 64), Color::rgb(220.0, 220.0, 180.0));
        let window = graphics.create_window(None);
        {
            let win = graphics.window_mut(window)?;
            win.windowskin = Some(skin);
            win.x = 40;
            win.y = 8;
            win.width = 48;
            win.height = 36;
            win.z = 10;
        }
        let text = graphics.create_bitmap(40, 16)?;
        {
            let bitmap = graphics.bitmap_mut(text)?;
            bitmap.font.size = 5;
            bitmap.font.outline = false;
            bitmap.font.shadow = false;
            bitmap.draw_text(Rect::new(0, 0, 40, 16), "MKXP", 0);
        }
        graphics.window_mut(window)?.contents = Some(text);

        graphics.update()?;
        Ok(Self { graphics, actor })
    }

    pub fn step(&mut self) -> Result<()> {
        let frame = self.graphics.frame_count;
        let sprite = self.graphics.sprite_mut(self.actor)?;
        sprite.x = 10 + frame % 30;
        self.graphics.update()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprites_composite_in_z_order() {
        let mut g = Graphics::new(16, 8, 1).unwrap();
        let red = g.create_bitmap(8, 8).unwrap();
        g.bitmap_mut(red)
            .unwrap()
            .fill_rect(Rect::new(0, 0, 8, 8), Color::rgb(255.0, 0.0, 0.0));
        let blue = g.create_bitmap(8, 8).unwrap();
        g.bitmap_mut(blue)
            .unwrap()
            .fill_rect(Rect::new(0, 0, 8, 8), Color::rgb(0.0, 0.0, 255.0));
        let low = g.create_sprite(None);
        g.set_sprite_bitmap(low, Some(red)).unwrap();
        g.sprite_mut(low).unwrap().z = 0;
        let high = g.create_sprite(None);
        g.set_sprite_bitmap(high, Some(blue)).unwrap();
        g.sprite_mut(high).unwrap().z = 5;
        g.sprite_mut(high).unwrap().x = 0;
        g.update().unwrap();
        let px = g.frame().get_pixel(1, 1);
        assert_eq!(px.blue, 255.0);
        assert_eq!(px.red, 0.0);
    }

    #[test]
    fn viewport_clips_and_tints() {
        let mut g = Graphics::new(20, 10, 1).unwrap();
        let view = g.create_viewport(Rect::new(0, 0, 4, 10));
        g.viewport_mut(view).unwrap().color = Color::new(0.0, 0.0, 0.0, 0.0);
        let bmp = g.create_bitmap(8, 4).unwrap();
        g.bitmap_mut(bmp)
            .unwrap()
            .fill_rect(Rect::new(0, 0, 8, 4), Color::rgb(255.0, 0.0, 0.0));
        let sprite = g.create_sprite(Some(view));
        g.set_sprite_bitmap(sprite, Some(bmp)).unwrap();
        g.update().unwrap();
        assert_eq!(g.frame().get_pixel(1, 1).red, 255.0);
        assert_eq!(g.frame().get_pixel(6, 1).red, 0.0);
    }

    #[test]
    fn tilemap_draws_tileset_and_autotile() {
        let mut g = Graphics::new(32, 32, 1).unwrap();
        let tileset = g.create_bitmap(32, 32).unwrap();
        g.bitmap_mut(tileset)
            .unwrap()
            .fill_rect(Rect::new(0, 0, 32, 32), Color::rgb(0.0, 255.0, 0.0));
        let auto = g.create_bitmap(96, 128).unwrap();
        g.bitmap_mut(auto)
            .unwrap()
            .fill_rect(Rect::new(0, 0, 96, 128), Color::rgb(255.0, 0.0, 0.0));
        let mut map = Table::new(2, 1, 1).unwrap();
        map.set(384, 0, 0, 0);
        map.set(48, 1, 0, 0);
        let tilemap = g.create_tilemap(None);
        {
            let tm = g.tilemap_mut(tilemap).unwrap();
            tm.tileset = Some(tileset);
            tm.set_autotile(0, Some(auto));
            tm.map_data = Some(map);
        }
        g.update().unwrap();
        assert_eq!(g.frame().get_pixel(1, 1).green, 255.0, "tileset");
        g.tilemap_mut(tilemap).unwrap().ox = 32;
        g.update().unwrap();
        assert_eq!(g.frame().get_pixel(1, 1).red, 255.0, "autotile");
    }

    #[test]
    fn tilemap_uses_the_viewport_scroll() {
        let mut g = Graphics::new(64, 32, 1).unwrap();
        let tileset = g.create_bitmap(32, 32).unwrap();
        g.bitmap_mut(tileset)
            .unwrap()
            .fill_rect(Rect::new(0, 0, 32, 32), Color::rgb(0.0, 255.0, 0.0));
        let mut map = Table::new(12, 1, 1).unwrap();
        map.set(384, 10, 0, 0);
        let view = g.create_viewport(Rect::new(0, 0, 64, 32));
        g.viewport_mut(view).unwrap().ox = 320;
        let tilemap = g.create_tilemap(Some(view));
        {
            let tm = g.tilemap_mut(tilemap).unwrap();
            tm.tileset = Some(tileset);
            tm.map_data = Some(map);
        }
        g.update().unwrap();
        assert_eq!(g.frame().get_pixel(1, 1).green, 255.0);
    }

    #[test]
    fn frozen_fade_does_not_darken_the_captured_frame_twice() {
        let mut g = Graphics::new(1, 1, 1).unwrap();
        let bmp = g.create_bitmap(1, 1).unwrap();
        g.bitmap_mut(bmp)
            .unwrap()
            .fill_rect(Rect::new(0, 0, 1, 1), Color::rgb(255.0, 255.0, 255.0));
        let sprite = g.create_sprite(None);
        g.set_sprite_bitmap(sprite, Some(bmp)).unwrap();
        g.brightness = 128;
        g.update().unwrap();
        let shown = g.frame().get_pixel(0, 0).red;
        assert!((shown - 128.0).abs() < 1.0, "{shown}");
        g.freeze().unwrap();
        g.fadeout(1).unwrap();
        let faded = g.frame().get_pixel(0, 0).red;
        assert!((faded - shown).abs() < 1.0, "{faded}");
    }

    #[test]
    fn transition_blends_the_frozen_frame() {
        let mut g = Graphics::new(2, 1, 1).unwrap();
        let bmp = g.create_bitmap(2, 1).unwrap();
        g.bitmap_mut(bmp)
            .unwrap()
            .fill_rect(Rect::new(0, 0, 2, 1), Color::rgb(0.0, 0.0, 0.0));
        let sprite = g.create_sprite(None);
        g.set_sprite_bitmap(sprite, Some(bmp)).unwrap();
        g.update().unwrap();
        g.freeze().unwrap();
        g.bitmap_mut(bmp)
            .unwrap()
            .fill_rect(Rect::new(0, 0, 2, 1), Color::rgb(255.0, 255.0, 255.0));
        g.transition(2, None, 40).unwrap();
        let mid = g.frame().get_pixel(0, 0).red;
        assert!(mid > 0.0 && mid < 255.0, "{mid}");
        g.update().unwrap();
        assert_eq!(g.frame().get_pixel(0, 0).red, 255.0);
    }

    #[test]
    fn transition_restores_brightness_after_fadeout() {
        let mut g = Graphics::new(1, 1, 1).unwrap();
        let bmp = g.create_bitmap(1, 1).unwrap();
        g.bitmap_mut(bmp)
            .unwrap()
            .fill_rect(Rect::new(0, 0, 1, 1), Color::rgb(255.0, 255.0, 255.0));
        let sprite = g.create_sprite(None);
        g.set_sprite_bitmap(sprite, Some(bmp)).unwrap();
        g.update().unwrap();
        g.freeze().unwrap();
        g.fadeout(0).unwrap();
        assert_eq!(g.brightness, 255);
        g.fadeout(1).unwrap();
        assert_eq!(g.brightness, 0);
        g.transition(0, None, 40).unwrap();
        g.update().unwrap();
        assert_eq!(g.brightness, 255);
        assert_eq!(g.frame().get_pixel(0, 0).red, 255.0);
    }

    #[test]
    fn demo_frame_keeps_the_background_and_actor() {
        let demo = Demo::new().unwrap();
        let bg = demo.graphics.frame().get_pixel(0, 0);
        assert_eq!((bg.red, bg.green, bg.blue), (10.0, 20.0, 40.0));
        let actor = demo.graphics.frame().get_pixel(12, 12);
        assert_eq!(actor.red, 255.0);
        let window = demo.graphics.frame().get_pixel(46, 14);
        assert!(window.green > 40.0, "{window:?}");
    }

    #[test]
    fn huge_zoom_still_covers_pixels_inside_the_frame() {
        let mut g = Graphics::new(2, 1, 1).unwrap();
        let bmp = g.create_bitmap(2, 1).unwrap();
        g.bitmap_mut(bmp)
            .unwrap()
            .fill_rect(Rect::new(0, 0, 2, 1), Color::rgb(255.0, 0.0, 0.0));
        let sprite = g.create_sprite(None);
        g.set_sprite_bitmap(sprite, Some(bmp)).unwrap();
        {
            let sprite = g.sprite_mut(sprite).unwrap();
            sprite.ox = 1;
            sprite.zoom_x = 30_000_000.0;
        }
        g.update().unwrap();
        assert_eq!(g.frame().get_pixel(0, 0).red, 255.0);
        assert_eq!(g.frame().get_pixel(1, 0).red, 255.0);
    }

    #[test]
    fn tile_scroll_uses_a_wide_sum() {
        assert_eq!(
            tile_start(i32::MAX, i32::MIN, i32::MAX),
            (i32::MAX as i64 - i32::MIN as i64 + i32::MAX as i64).div_euclid(32) - 1
        );
        let mut g = Graphics::new(32, 32, 1).unwrap();
        let tileset = g.create_bitmap(32, 32).unwrap();
        g.bitmap_mut(tileset)
            .unwrap()
            .fill_rect(Rect::new(0, 0, 32, 32), Color::rgb(0.0, 255.0, 0.0));
        let mut map = Table::new(1, 1, 1).unwrap();
        map.set(384, 0, 0, 0);
        let view = g.create_viewport(Rect::new(0, 0, 32, 32));
        g.viewport_mut(view).unwrap().ox = 100;
        let tilemap = g.create_tilemap(Some(view));
        {
            let tm = g.tilemap_mut(tilemap).unwrap();
            tm.tileset = Some(tileset);
            tm.map_data = Some(map);
            tm.ox = i32::MAX;
        }
        g.update().unwrap();
        assert_eq!(g.frame().get_pixel(1, 1).green, 255.0);
    }

    #[test]
    fn window_opacity_is_applied_once() {
        let mut g = Graphics::new(64, 64, 1).unwrap();
        let contents = g.create_bitmap(16, 16).unwrap();
        g.bitmap_mut(contents)
            .unwrap()
            .fill_rect(Rect::new(0, 0, 16, 16), Color::rgb(255.0, 255.0, 255.0));
        let window = g.create_window(None);
        {
            let window = g.window_mut(window).unwrap();
            window.width = 48;
            window.height = 48;
            window.contents = Some(contents);
            window.contents_opacity = 128;
        }
        g.update().unwrap();
        let red = g.frame().get_pixel(16, 16).red;
        assert!((red - 128.0).abs() < 1.0, "contents {red}");

        let skin = g.create_bitmap(192, 128).unwrap();
        g.bitmap_mut(skin)
            .unwrap()
            .fill_rect(Rect::new(0, 0, 192, 128), Color::rgb(255.0, 255.0, 255.0));
        {
            let window = g.window_mut(window).unwrap();
            window.contents = None;
            window.windowskin = Some(skin);
            window.width = 64;
            window.height = 64;
            window.opacity = 128;
            window.back_opacity = 255;
        }
        g.update().unwrap();
        let center = g.frame().get_pixel(32, 32).red;
        let edge = g.frame().get_pixel(10, 4).red;
        assert!((center - 128.0).abs() < 1.0, "center {center}");
        assert!((edge - 128.0).abs() < 1.0, "edge {edge}");
    }
}
