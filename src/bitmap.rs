use crate::blend::{bitmap_blend, bitmap_subtract, hue_shift};
use crate::color::Color;
use crate::error::{Error, Result};
use crate::font::{glyph_rows, Font, GLYPH_HEIGHT, GLYPH_WIDTH};
use crate::rect::Rect;

/// Software RGSS `Bitmap`. Pixels are straight RGBA8, row-major.
#[derive(Clone, Debug)]
pub struct Bitmap {
    width: i32,
    height: i32,
    pixels: Vec<u8>,
    pub font: Font,
    disposed: bool,
}

impl Bitmap {
    pub fn new(width: i32, height: i32) -> Result<Self> {
        Self::with_font(width, height, Font::default())
    }

    pub fn with_font(width: i32, height: i32, font: Font) -> Result<Self> {
        if width <= 0 || height <= 0 {
            return Err(Error::new("bitmap dimensions must be positive"));
        }
        if width > 8192 || height > 8192 {
            return Err(Error::new("bitmap exceeds the 8192 pixel dimension limit"));
        }
        let cells = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| Error::new("bitmap dimensions overflow"))?;
        Ok(Self {
            width,
            height,
            pixels: vec![0; cells],
            font,
            disposed: false,
        })
    }

    pub fn width(&self) -> i32 {
        self.width
    }
    pub fn height(&self) -> i32 {
        self.height
    }
    pub fn is_disposed(&self) -> bool {
        self.disposed
    }
    pub fn dispose(&mut self) {
        self.disposed = true;
    }
    pub fn rect(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn clear(&mut self) {
        if self.disposed {
            return;
        }
        self.pixels.fill(0);
    }

    pub fn fill_rect(&mut self, rect: Rect, color: Color) {
        if self.disposed {
            return;
        }
        let rect = rect.intersect(self.rect());
        let px = color.to_rgba8();
        for y in rect.y..rect.bottom() {
            for x in rect.x..rect.right() {
                self.put(x, y, px);
            }
        }
    }

    pub fn clear_rect(&mut self, rect: Rect) {
        self.fill_rect(rect, Color::default());
    }

    pub fn get_pixel(&self, x: i32, y: i32) -> Color {
        self.get(x, y).map(Color::from_rgba8).unwrap_or_default()
    }

    pub fn set_pixel(&mut self, x: i32, y: i32, color: Color) {
        if self.disposed {
            return;
        }
        self.put(x, y, color.to_rgba8());
    }

    pub fn blt(&mut self, x: i32, y: i32, source: &Bitmap, rect: Rect, opacity: i32) {
        let width = rect.width.abs();
        let height = rect.height.abs();
        self.stretch_blt(Rect::new(x, y, width, height), source, rect, opacity, false);
    }

    pub fn stretch_blt(
        &mut self,
        mut dest: Rect,
        source: &Bitmap,
        mut source_rect: Rect,
        opacity: i32,
        mut smooth: bool,
    ) {
        if self.disposed || source.disposed {
            return;
        }
        let opacity = opacity.clamp(0, 255);
        if opacity == 0 {
            return;
        }
        if shrink_rects(
            &mut source_rect.x,
            &mut source_rect.width,
            source.width,
            &mut dest.x,
            &mut dest.width,
            self.width,
        ) || shrink_rects(
            &mut source_rect.y,
            &mut source_rect.height,
            source.height,
            &mut dest.y,
            &mut dest.height,
            self.height,
        ) {
            return;
        }
        if source_rect.width == dest.width && source_rect.height == dest.height {
            smooth = false;
        }
        self.stretch_filtered(dest, source, source_rect, opacity, smooth, false);
    }

    /// KGL subtract blit. `smooth` selects bilinear sampling.
    pub fn subtract_blt(
        &mut self,
        mut dest: Rect,
        source: &Bitmap,
        mut source_rect: Rect,
        opacity: i32,
        smooth: bool,
    ) {
        if self.disposed || source.disposed {
            return;
        }
        let opacity = opacity.clamp(0, 255);
        if shrink_rects(
            &mut source_rect.x,
            &mut source_rect.width,
            source.width,
            &mut dest.x,
            &mut dest.width,
            self.width,
        ) || shrink_rects(
            &mut source_rect.y,
            &mut source_rect.height,
            source.height,
            &mut dest.y,
            &mut dest.height,
            self.height,
        ) {
            return;
        }
        self.stretch_filtered(dest, source, source_rect, opacity, smooth, true);
    }

    fn stretch_filtered(
        &mut self,
        dest: Rect,
        source: &Bitmap,
        source_rect: Rect,
        opacity: i32,
        smooth: bool,
        subtract: bool,
    ) {
        // A negative source rect and a negative destination rect each mirror.
        // Both together cancel, matching a reversed glBlitFramebuffer.
        let flip_x = (dest.width < 0) ^ (source_rect.width < 0);
        let flip_y = (dest.height < 0) ^ (source_rect.height < 0);
        let dest_w = dest.width.abs();
        let dest_h = dest.height.abs();
        if dest_w == 0 || dest_h == 0 {
            return;
        }
        let origin_x = if dest.width < 0 {
            dest.x + dest.width
        } else {
            dest.x
        };
        let origin_y = if dest.height < 0 {
            dest.y + dest.height
        } else {
            dest.y
        };
        for dy in 0..dest_h {
            for dx in 0..dest_w {
                let src_px = if smooth {
                    sample_bilinear(
                        source,
                        &source_rect,
                        BilinearAt {
                            dx,
                            dy,
                            dest_w,
                            dest_h,
                            flip_x,
                            flip_y,
                        },
                    )
                } else {
                    let sx = sample_nearest(source_rect.x, source_rect.width, dx, dest_w, flip_x);
                    let sy = sample_nearest(source_rect.y, source_rect.height, dy, dest_h, flip_y);
                    match source.get(sx, sy) {
                        Some(px) => px,
                        None => continue,
                    }
                };
                let x = origin_x + dx;
                let y = origin_y + dy;
                if let Some(dst) = self.get(x, y) {
                    let out = if subtract {
                        bitmap_subtract(dst, src_px, opacity as u8)
                    } else {
                        bitmap_blend(dst, src_px, opacity as u8)
                    };
                    self.put(x, y, out);
                }
            }
        }
    }

    pub fn gradient_fill_rect(&mut self, rect: Rect, start: Color, end: Color, vertical: bool) {
        if self.disposed || rect.width <= 0 || rect.height <= 0 {
            return;
        }
        let clip = rect.intersect(self.rect());
        if clip.is_empty() {
            return;
        }
        let span = if vertical { rect.height } else { rect.width };
        let a = start.to_rgba8();
        let b = end.to_rgba8();
        for y in clip.y..clip.bottom() {
            for x in clip.x..clip.right() {
                let index = if vertical { y - rect.y } else { x - rect.x };
                let t = if span <= 1 {
                    0.0
                } else {
                    index as f32 / (span - 1) as f32
                };
                let mut px = [0u8; 4];
                for c in 0..4 {
                    px[c] = (a[c] as f32 + (b[c] as f32 - a[c] as f32) * t).round() as u8;
                }
                self.put(x, y, px);
            }
        }
    }

    pub fn hue_change(&mut self, hue: i32) {
        if self.disposed || hue % 360 == 0 {
            return;
        }
        let turns = wrap_range(hue, 0, 360) as f32 / 360.0;
        let (pixels, _) = self.pixels.as_chunks_mut::<4>();
        for px in pixels {
            let rgb = [
                px[0] as f32 / 255.0,
                px[1] as f32 / 255.0,
                px[2] as f32 / 255.0,
            ];
            let out = hue_shift(rgb, turns);
            px[0] = (out[0] * 255.0).round().clamp(0.0, 255.0) as u8;
            px[1] = (out[1] * 255.0).round().clamp(0.0, 255.0) as u8;
            px[2] = (out[2] * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }

    /// Separable 3-tap blur matching `shader/blur.frag`: a horizontal pass and
    /// a vertical pass, each averaging the pixel with its two neighbors.
    /// Samples outside the bitmap repeat the edge pixel, and alpha is included.
    pub fn blur(&mut self) {
        if self.disposed || self.width <= 0 || self.height <= 0 {
            return;
        }
        self.blur_axis(true);
        self.blur_axis(false);
    }

    fn blur_axis(&mut self, horizontal: bool) {
        let original = self.pixels.clone();
        let width = self.width;
        let height = self.height;
        for y in 0..height {
            for x in 0..width {
                let mut acc = [0u32; 4];
                for delta in -1..=1 {
                    let sx = if horizontal {
                        (x + delta).clamp(0, width - 1)
                    } else {
                        x
                    };
                    let sy = if horizontal {
                        y
                    } else {
                        (y + delta).clamp(0, height - 1)
                    };
                    let px = self.sample_from(&original, sx, sy).unwrap();
                    for (channel, sample) in acc.iter_mut().zip(px) {
                        *channel += sample as u32;
                    }
                }
                let index = self.offset(x, y).unwrap();
                for (channel, sum) in acc.iter().enumerate() {
                    self.pixels[index + channel] = (sum / 3) as u8;
                }
            }
        }
    }

    pub fn text_size(&self, text: &str) -> Rect {
        let text = fixup_text(text);
        if text.is_empty() || text == " " {
            return Rect::default();
        }
        let scale = self.font.scale();
        let advance = (GLYPH_WIDTH + 1) * scale;
        let width = text.chars().count() as i32 * advance - scale;
        Rect::new(0, 0, width.max(0), GLYPH_HEIGHT * scale)
    }

    pub fn draw_text(&mut self, rect: Rect, text: &str, align: i32) {
        if self.disposed || rect.width <= 0 || rect.height <= 0 {
            return;
        }
        if rect.x >= self.width || rect.y >= self.height {
            return;
        }
        let text = fixup_text(text);
        if text.is_empty() || text == " " {
            return;
        }
        if Color::channel_u8(self.font.color.alpha) == 0 {
            return;
        }
        let size = self.text_size(&text);
        if size.width == 0 {
            return;
        }
        let scale = self.font.scale();
        let squeeze = (rect.width as f32 / size.width as f32).clamp(0.5, 1.0);
        let step = ((GLYPH_WIDTH + 1) as f32 * scale as f32 * squeeze).max(1.0);
        let drawn_width = (text.chars().count() as f32 * step - scale as f32 * squeeze)
            .round()
            .max(1.0) as i32;
        let mut x = rect.x;
        match align {
            1 => x += (rect.width - drawn_width) / 2,
            2 => x += rect.width - drawn_width,
            _ => {}
        }
        x = x.max(rect.x);
        let mut y = rect.y + (rect.height - size.height) / 2;
        y = y.max(rect.y);
        let clip = rect.intersect(self.rect());
        let outline = self.font.out_color;
        let ink = self.font.color;
        if self.font.shadow {
            self.paint_text(&TextPaint {
                text: &text,
                x: x + 1,
                y: y + 1,
                step,
                scale,
                squeeze,
                clip,
                color: outline,
            });
        }
        if self.font.outline {
            for (ox, oy) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
                self.paint_text(&TextPaint {
                    text: &text,
                    x: x + ox,
                    y: y + oy,
                    step,
                    scale,
                    squeeze,
                    clip,
                    color: outline,
                });
            }
        }
        self.paint_text(&TextPaint {
            text: &text,
            x,
            y,
            step,
            scale,
            squeeze,
            clip,
            color: ink,
        });
    }

    fn paint_text(&mut self, paint: &TextPaint) {
        let TextPaint {
            text,
            x,
            y,
            step,
            scale,
            squeeze,
            clip,
            color,
        } = *paint;
        let ink = color.to_rgba8();
        for (index, ch) in text.chars().enumerate() {
            let glyph_x = x + (index as f32 * step).round() as i32;
            let rows = glyph_rows(ch);
            for (row, bits) in rows.iter().enumerate() {
                for col in 0..GLYPH_WIDTH {
                    if bits & (1 << (GLYPH_WIDTH - 1 - col)) == 0 {
                        continue;
                    }
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let src_x = col * scale + sx;
                            let px = glyph_x + (src_x as f32 * squeeze).floor() as i32;
                            let py = y + row as i32 * scale + sy;
                            if !clip.contains(px, py) {
                                continue;
                            }
                            if let Some(dst) = self.get(px, py) {
                                self.put(px, py, bitmap_blend(dst, ink, 255));
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn to_ppm(&self) -> Vec<u8> {
        let mut out = format!("P6\n{} {}\n255\n", self.width, self.height).into_bytes();
        let (pixels, _) = self.pixels.as_chunks::<4>();
        for px in pixels {
            let a = px[3] as f32 / 255.0;
            for channel in &px[..3] {
                out.push((*channel as f32 * a).round().clamp(0.0, 255.0) as u8);
            }
        }
        out
    }

    fn offset(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 || x >= self.width || y >= self.height {
            None
        } else {
            Some(((y as usize) * (self.width as usize) + (x as usize)) * 4)
        }
    }

    fn get(&self, x: i32, y: i32) -> Option<[u8; 4]> {
        let i = self.offset(x, y)?;
        Some([
            self.pixels[i],
            self.pixels[i + 1],
            self.pixels[i + 2],
            self.pixels[i + 3],
        ])
    }

    fn put(&mut self, x: i32, y: i32, px: [u8; 4]) {
        if let Some(i) = self.offset(x, y) {
            self.pixels[i..i + 4].copy_from_slice(&px);
        }
    }

    fn sample_from(&self, pixels: &[u8], x: i32, y: i32) -> Option<[u8; 4]> {
        let i = self.offset(x, y)?;
        Some([pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]])
    }
}

#[derive(Clone, Copy)]
struct TextPaint<'a> {
    text: &'a str,
    x: i32,
    y: i32,
    step: f32,
    scale: i32,
    squeeze: f32,
    clip: Rect,
    color: Color,
}

fn fixup_text(text: &str) -> String {
    text.chars()
        .map(|ch| if ch == '\n' || ch == '\r' { ' ' } else { ch })
        .collect()
}

fn wrap_range(mut value: i32, min: i32, max: i32) -> i32 {
    let range = max - min;
    if range <= 0 {
        return min;
    }
    value -= min;
    value %= range;
    if value < 0 {
        value += range;
    }
    value + min
}

fn sample_nearest(origin: i32, length: i32, dest_index: i32, dest_len: i32, flip: bool) -> i32 {
    if dest_len == 0 || length == 0 {
        return origin;
    }
    let src_len = length.abs() as i64;
    let dest_len = dest_len as i64;
    // GL_NEAREST samples the source at the destination pixel center.
    let mut index = ((dest_index as i64 * 2 + 1) * src_len) / (dest_len * 2);
    if index >= src_len {
        index = src_len - 1;
    }
    if flip {
        index = src_len - 1 - index;
    }
    let base = if length >= 0 { origin } else { origin + length };
    base + index as i32
}

struct BilinearAt {
    dx: i32,
    dy: i32,
    dest_w: i32,
    dest_h: i32,
    flip_x: bool,
    flip_y: bool,
}

fn sample_bilinear(source: &Bitmap, source_rect: &Rect, at: BilinearAt) -> [u8; 4] {
    let BilinearAt {
        dx,
        dy,
        dest_w,
        dest_h,
        flip_x,
        flip_y,
    } = at;
    let w_ratio = source_rect.width.abs() as f32 / dest_w as f32;
    let h_ratio = source_rect.height.abs() as f32 / dest_h as f32;
    // Half a destination pixel, then back by half a source texel, so a 1:1
    // blit lands on texel centers instead of their top-left corners.
    let mut fx = w_ratio * (dx as f32 + 0.5) - 0.5;
    let mut fy = h_ratio * (dy as f32 + 0.5) - 0.5;
    if flip_x {
        fx = source_rect.width.abs() as f32 - 1.0 - fx;
    }
    if flip_y {
        fy = source_rect.height.abs() as f32 - 1.0 - fy;
    }
    let x0 = fx.floor() as i32;
    let y0 = fy.floor() as i32;
    let tx = fx - x0 as f32;
    let ty = fy - y0 as f32;
    let base_x = if source_rect.width >= 0 {
        source_rect.x
    } else {
        source_rect.x + source_rect.width
    };
    let base_y = if source_rect.height >= 0 {
        source_rect.y
    } else {
        source_rect.y + source_rect.height
    };
    let max_x = source_rect.width.abs() - 1;
    let max_y = source_rect.height.abs() - 1;
    let sample = |ox: i32, oy: i32| -> [f32; 4] {
        let sx = base_x + ox.clamp(0, max_x);
        let sy = base_y + oy.clamp(0, max_y);
        let px = source.get(sx, sy).unwrap_or([0, 0, 0, 0]);
        [px[0] as f32, px[1] as f32, px[2] as f32, px[3] as f32]
    };
    let c00 = sample(x0, y0);
    let c10 = sample(x0 + 1, y0);
    let c01 = sample(x0, y0 + 1);
    let c11 = sample(x0 + 1, y0 + 1);
    let mut out = [0u8; 4];
    for c in 0..4 {
        let v = c00[c] * (1.0 - tx) * (1.0 - ty)
            + c10[c] * tx * (1.0 - ty)
            + c01[c] * (1.0 - tx) * ty
            + c11[c] * tx * ty;
        out[c] = v.round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// Port of mkxp's `shrinkRects`. Returns true when the blit should be skipped.
fn shrink_rects(
    source_pos: &mut i32,
    source_len: &mut i32,
    source_bitmap: i32,
    dest_pos: &mut i32,
    dest_len: &mut i32,
    dest_bitmap: i32,
) -> bool {
    let mut f_source_pos = *source_pos as f32;
    let mut f_source_len = *source_len as f32;
    let mut f_dest_pos = *dest_pos as f32;
    let mut f_dest_len = *dest_len as f32;
    let mut skip = shrink_rects_f(
        &mut f_source_pos,
        &mut f_source_len,
        source_bitmap,
        &mut f_dest_pos,
        &mut f_dest_len,
        dest_bitmap,
        true,
    );
    if !skip {
        skip = shrink_rects_f(
            &mut f_dest_pos,
            &mut f_dest_len,
            dest_bitmap,
            &mut f_source_pos,
            &mut f_source_len,
            source_bitmap,
            false,
        );
    }
    *source_pos = f_source_pos.round() as i32;
    *source_len = f_source_len.round() as i32;
    *dest_pos = f_dest_pos.round() as i32;
    *dest_len = f_dest_len.round() as i32;
    skip || *source_len == 0 || *dest_len == 0
}

fn shrink_rects_f(
    source_pos: &mut f32,
    source_len: &mut f32,
    s_bitmap_len: i32,
    dest_pos: &mut f32,
    dest_len: &mut f32,
    _d_bitmap_len: i32,
    normalize: bool,
) -> bool {
    let s_bitmap_len = s_bitmap_len as f32;
    let mut s_start = if *source_len > 0.0 {
        *source_pos
    } else {
        *source_len + *source_pos
    };
    let mut s_end = if *source_len > 0.0 {
        *source_len + *source_pos
    } else {
        *source_pos
    };
    let mut s_length = s_end - s_start;
    if s_start >= 0.0 && s_end < s_bitmap_len {
        return false;
    }
    if s_start >= s_bitmap_len || s_end < 0.0 {
        return true;
    }
    let mut d_start = if *dest_len > 0.0 {
        *dest_pos
    } else {
        *dest_len + *dest_pos
    };
    let mut d_end = if *dest_len > 0.0 {
        *dest_len + *dest_pos
    } else {
        *dest_pos
    };
    let mut d_length = d_end - d_start;
    let delta = s_end - s_bitmap_len;
    if delta > 0.0 {
        let d_delta = (delta / s_length) * d_length;
        s_length -= delta;
        s_end = s_bitmap_len;
        d_end -= d_delta;
        d_length -= d_delta;
    }
    if s_start < 0.0 {
        let d_delta = (s_start / s_length) * d_length;
        s_length += s_start;
        s_start = 0.0;
        d_start -= d_delta;
        d_length += d_delta;
    }
    if !normalize {
        *source_pos = if *source_len > 0.0 { s_start } else { s_end };
        *source_len = if *source_len > 0.0 {
            s_length
        } else {
            -s_length
        };
        *dest_pos = if *dest_len > 0.0 { d_start } else { d_end };
        *dest_len = if *dest_len > 0.0 { d_length } else { -d_length };
    } else {
        let same_sign = (*dest_len > 0.0) == (*source_len > 0.0);
        *dest_pos = if same_sign { d_start } else { d_end };
        *dest_len = if same_sign { d_length } else { -d_length };
        *source_pos = s_start;
        *source_len = s_length;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stretch_nearest_samples_destination_pixel_centers() {
        let mut src = Bitmap::new(4, 1).unwrap();
        for (x, red) in [10u8, 50, 100, 200].into_iter().enumerate() {
            src.set_pixel(x as i32, 0, Color::new(red as f64, 0.0, 0.0, 255.0));
        }
        let mut dest = Bitmap::new(2, 1).unwrap();
        dest.stretch_blt(dest.rect(), &src, src.rect(), 255, false);
        assert_eq!(dest.get_pixel(0, 0).red, 50.0);
        assert_eq!(dest.get_pixel(1, 0).red, 200.0);

        let mut same = Bitmap::new(4, 1).unwrap();
        same.stretch_blt(same.rect(), &src, src.rect(), 255, false);
        for x in 0..4 {
            assert_eq!(same.get_pixel(x, 0).red, src.get_pixel(x, 0).red);
        }
    }

    #[test]
    fn negative_source_width_mirrors_a_positive_destination() {
        let mut src = Bitmap::new(4, 1).unwrap();
        src.set_pixel(1, 0, Color::new(10.0, 0.0, 0.0, 255.0));
        src.set_pixel(2, 0, Color::new(200.0, 0.0, 0.0, 255.0));
        let mut dest = Bitmap::new(2, 1).unwrap();
        dest.stretch_blt(
            Rect::new(0, 0, 2, 1),
            &src,
            Rect::new(3, 0, -2, 1),
            255,
            false,
        );
        assert_eq!(dest.get_pixel(0, 0).red, 200.0);
        assert_eq!(dest.get_pixel(1, 0).red, 10.0);
    }

    #[test]
    fn blit_clips_to_the_destination_and_respects_opacity() {
        let mut dest = Bitmap::new(4, 1).unwrap();
        dest.fill_rect(dest.rect(), Color::rgb(255.0, 0.0, 0.0));
        let mut src = Bitmap::new(4, 1).unwrap();
        src.fill_rect(src.rect(), Color::rgb(0.0, 0.0, 255.0));
        dest.blt(-1, 0, &src, Rect::new(0, 0, 2, 1), 255);
        assert_eq!(dest.get_pixel(0, 0).blue, 255.0);
        assert_eq!(dest.get_pixel(1, 0).red, 255.0);
        dest.blt(2, 0, &src, Rect::new(0, 0, 1, 1), 0);
        assert_eq!(dest.get_pixel(2, 0).red, 255.0);
    }

    #[test]
    fn hue_change_moves_red_toward_green() {
        let mut bmp = Bitmap::new(1, 1).unwrap();
        bmp.set_pixel(0, 0, Color::rgb(255.0, 0.0, 0.0));
        bmp.hue_change(120);
        let px = bmp.get_pixel(0, 0);
        assert!(px.green > 240.0, "{px:?}");
        assert!(px.red < 20.0, "{px:?}");
    }

    #[test]
    fn draw_text_centers_and_paints_ink() {
        let mut bmp = Bitmap::new(40, 20).unwrap();
        bmp.font.size = 5;
        bmp.font.outline = false;
        bmp.font.shadow = false;
        bmp.font.color = Color::rgb(255.0, 255.0, 255.0);
        bmp.draw_text(Rect::new(0, 0, 40, 20), "A", 1);
        assert!(bmp.pixels().contains(&255));
        let size = bmp.text_size("A");
        assert!(size.width > 0 && size.height > 0);
    }

    #[test]
    fn draw_text_squeeze_scales_glyph_pixels() {
        let mut bmp = Bitmap::new(6, 20).unwrap();
        bmp.font.size = 20;
        bmp.font.outline = false;
        bmp.font.shadow = false;
        bmp.font.color = Color::new(255.0, 255.0, 255.0, 255.0);
        assert_eq!(bmp.text_size("A").width, 12);
        bmp.draw_text(Rect::new(0, 0, 6, 20), "A", 0);
        // The top stroke of "A" is the middle column. At squeeze 0.5 it lands
        // on x=2, still on the first row. The right stroke of the second row
        // used to be clipped away (alpha 0) because only the advance shrank.
        assert_eq!(bmp.get_pixel(2, 0).alpha, 255.0);
        assert_eq!(bmp.get_pixel(4, 4).alpha, 255.0);
    }
}
