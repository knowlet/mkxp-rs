use mkxpz::{Color, Graphics, Rect, Table};

// https://github.com/mkxp-z/mkxp-z/issues/241
#[test]
fn xp_tilemap_wraps_both_axes_for_negative_and_multiple_map_offsets() {
    let mut graphics = Graphics::new(37, 35, 1).unwrap();
    let tileset = graphics.create_bitmap(128, 32).unwrap();
    let colors = [
        Color::rgb(255.0, 0.0, 0.0),
        Color::rgb(0.0, 255.0, 0.0),
        Color::rgb(0.0, 0.0, 255.0),
        Color::rgb(255.0, 255.0, 0.0),
    ];
    for (index, color) in colors.iter().enumerate() {
        graphics
            .bitmap_mut(tileset)
            .unwrap()
            .fill_rect(Rect::new(index as i32 * 32, 0, 32, 32), *color);
    }
    let mut map = Table::new(2, 2, 1).unwrap();
    for y in 0..2 {
        for x in 0..2 {
            map.set((384 + y * 2 + x) as i16, x, y, 0);
        }
    }
    let handle = graphics.create_tilemap(None);
    {
        let tilemap = graphics.tilemap_mut(handle).unwrap();
        tilemap.map_data = Some(map);
        tilemap.tileset = Some(tileset);
    }
    for ox in [-129, -33, -1, 0, 64, 97] {
        for oy in [-97, -1, 0, 64, 129] {
            let tilemap = graphics.tilemap_mut(handle).unwrap();
            tilemap.ox = ox;
            tilemap.oy = oy;
            graphics.update().unwrap();
            for y in 0..35 {
                for x in 0..37 {
                    let mx = (x + ox).div_euclid(32).rem_euclid(2);
                    let my = (y + oy).div_euclid(32).rem_euclid(2);
                    let expected = colors[(my * 2 + mx) as usize];
                    let actual = graphics.frame().get_pixel(x, y);
                    assert_eq!(
                        (actual.red, actual.green, actual.blue),
                        (expected.red, expected.green, expected.blue),
                        "offset ({ox}, {oy}), pixel ({x}, {y})"
                    );
                }
            }
        }
    }
}

// Related to #398 / #406, but covers the Rust XP skin renderer only.
// It is not evidence that the unimplemented VX window renderer is compatible.
#[test]
fn xp_window_keeps_edges_when_the_other_axis_is_small() {
    for (width, height) in [(200, 28), (200, 32), (28, 160), (32, 160)] {
        let mut graphics = Graphics::new(width + 2, height + 2, 1).unwrap();
        let skin = graphics.create_bitmap(192, 128).unwrap();
        // Transparent background, opaque border and corners.
        graphics
            .bitmap_mut(skin)
            .unwrap()
            .fill_rect(Rect::new(128, 0, 64, 64), Color::rgb(255.0, 255.0, 255.0));
        let handle = graphics.create_window(None);
        let window = graphics.window_mut(handle).unwrap();
        window.width = width;
        window.height = height;
        window.windowskin = Some(skin);
        graphics.update().unwrap();
        for (x, y) in [
            (width / 2, 0),
            (width / 2, height - 1),
            (0, height / 2),
            (width - 1, height / 2),
        ] {
            assert_eq!(
                graphics.frame().get_pixel(x, y).red,
                255.0,
                "window {width}x{height}, edge ({x}, {y})"
            );
        }
        assert_eq!(graphics.frame().get_pixel(width, height).red, 0.0);
    }
}
