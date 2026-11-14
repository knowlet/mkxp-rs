//! Bitmap blits use the RMXP DirectDraw equation from `shader/bitmapBlit.frag`.
//! Sprite compositing uses the OpenGL blend modes from `GLBlendMode::apply`.

fn clamp_u8(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

fn px(src: [u8; 4]) -> [f32; 4] {
    [src[0] as f32, src[1] as f32, src[2] as f32, src[3] as f32]
}

/// `Bitmap#blt` / `stretch_blt` normal blending.
pub fn bitmap_blend(dst: [u8; 4], src: [u8; 4], opacity: u8) -> [u8; 4] {
    let src = px(src);
    let dst = px(dst);
    let op = opacity as f32 / 255.0;
    let co1 = (src[3] / 255.0) * op;
    let co2 = (dst[3] / 255.0) * (1.0 - co1);
    let alpha = co1 + co2;
    if alpha == 0.0 {
        return [clamp_u8(src[0]), clamp_u8(src[1]), clamp_u8(src[2]), 0];
    }
    let mut out = [0u8; 4];
    for i in 0..3 {
        out[i] = clamp_u8((co1 * src[i] + co2 * dst[i]) / alpha);
    }
    out[3] = clamp_u8(alpha * 255.0);
    out
}

/// KGL subtract blit (`Bitmap::KGL_SUBTRACT`). Operates on a straight pixel
/// and forces the destination alpha to opaque, matching `bltFilter`.
pub fn bitmap_subtract(dst: [u8; 4], src: [u8; 4], opacity: u8) -> [u8; 4] {
    let norm = if opacity == 255 {
        1.0
    } else {
        opacity as f32 / 256.0
    };
    let mut out = dst;
    for i in 0..3 {
        let old = (norm * dst[i] as f32).round() as u8;
        out[i] = src[i].saturating_sub(old);
    }
    out[3] = 255;
    out
}

/// Sprite / plane / window compositing. `blend` is 0 normal, 1 add, 2 subtract.
/// `src` is a straight color whose alpha already includes sprite opacity.
pub fn sprite_blend(dst: [u8; 4], src: [u8; 4], blend: i32) -> [u8; 4] {
    let sa = src[3] as f32 / 255.0;
    let d = px(dst);
    let s = px(src);
    match blend {
        1 => {
            let mut out = [0u8; 4];
            for i in 0..3 {
                out[i] = clamp_u8(s[i] * sa + d[i]);
            }
            out[3] = clamp_u8(s[3] + d[3]);
            out
        }
        2 => {
            let mut out = [0u8; 4];
            for i in 0..3 {
                out[i] = clamp_u8(d[i] - s[i] * sa);
            }
            out[3] = dst[3];
            out
        }
        _ => {
            let mut out = [0u8; 4];
            for i in 0..3 {
                out[i] = clamp_u8(s[i] * sa + d[i] * (1.0 - sa));
            }
            out[3] = clamp_u8(s[3] + d[3] * (1.0 - sa));
            out
        }
    }
}

/// Apply the sprite tone and color uniforms from `shader/sprite.frag`.
/// Channels are straight `0..=1` RGB.
pub fn apply_tone_color(rgb: [f32; 3], tone: [f32; 4], color: [f32; 4]) -> [f32; 3] {
    let luma = rgb[0] * 0.299 + rgb[1] * 0.587 + rgb[2] * 0.114;
    let gray = tone[3].clamp(0.0, 1.0);
    let mut out = [
        rgb[0] + (luma - rgb[0]) * gray + tone[0],
        rgb[1] + (luma - rgb[1]) * gray + tone[1],
        rgb[2] + (luma - rgb[2]) * gray + tone[2],
    ];
    let ca = color[3].clamp(0.0, 1.0);
    for i in 0..3 {
        out[i] = out[i] * (1.0 - ca) + color[i].clamp(0.0, 1.0) * ca;
    }
    out
}

pub fn quantize_rgb(rgb: [f32; 3]) -> [u8; 3] {
    [
        clamp_u8(rgb[0] * 255.0),
        clamp_u8(rgb[1] * 255.0),
        clamp_u8(rgb[2] * 255.0),
    ]
}

fn rgb_to_hsv(c: [f32; 3]) -> [f32; 3] {
    // Direct port of the branchless mix/step form in shader/hue.frag.
    let p = if c[2] <= c[1] {
        [c[1], c[2], 0.0, -1.0 / 3.0]
    } else {
        [c[2], c[1], -1.0, 2.0 / 3.0]
    };
    let q = if p[0] <= c[0] {
        [c[0], p[1], p[2], p[0]]
    } else {
        [p[0], p[1], p[3], c[0]]
    };
    let d = q[0] - q[3].min(q[1]);
    let eps = 1.0e-10;
    [
        (q[2] + (q[3] - q[1]) / (6.0 * d + eps)).abs(),
        d / (q[0] + eps),
        q[0],
    ]
}

fn mix(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

/// Hue rotation from `shader/hue.frag`. `hue_turns` is the wrapped hue in turns
/// (`degrees / 360`).
pub fn hue_shift(rgb: [f32; 3], hue_turns: f32) -> [f32; 3] {
    let mut hsv = rgb_to_hsv(rgb);
    hsv[0] += hue_turns;
    hsv_to_rgb_exact(hsv)
}

fn hsv_to_rgb_exact(c: [f32; 3]) -> [f32; 3] {
    let kx = 1.0;
    let ky = 2.0 / 3.0;
    let kz = 1.0 / 3.0;
    let kw = 3.0;
    let p = [
        ((c[0] + kx).fract() * 6.0 - kw).abs(),
        ((c[0] + ky).fract() * 6.0 - kw).abs(),
        ((c[0] + kz).fract() * 6.0 - kw).abs(),
    ];
    [
        c[2] * mix(kx, (p[0] - kx).clamp(0.0, 1.0), c[1]),
        c[2] * mix(kx, (p[1] - kx).clamp(0.0, 1.0), c[1]),
        c[2] * mix(kx, (p[2] - kx).clamp(0.0, 1.0), c[1]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_bitmap_blend_covers_an_opaque_source() {
        let dst = [255, 0, 0, 255];
        let src = [0, 0, 255, 255];
        assert_eq!(bitmap_blend(dst, src, 255), [0, 0, 255, 255]);
    }

    #[test]
    fn sprite_add_clamps() {
        let dst = [200, 0, 0, 255];
        let src = [200, 0, 0, 255];
        let out = sprite_blend(dst, src, 1);
        assert_eq!(out[0], 255);
        assert_eq!(out[3], 255);
    }

    #[test]
    fn pure_red_rotates_toward_green() {
        let out = hue_shift([1.0, 0.0, 0.0], 120.0 / 360.0);
        assert!(out[1] > 0.95, "{out:?}");
        assert!(out[0] < 0.05, "{out:?}");
        assert!(out[2] < 0.05, "{out:?}");
    }
}
