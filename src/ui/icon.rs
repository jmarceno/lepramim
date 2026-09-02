//! Render the Lexaloud SVG icon at tray sizes with optional opacity tint.
//!
//! Returns straight RGBA pixels, plus ARGB32 pixmaps for StatusNotifierItem.

const ICON_SVG: &[u8] = include_bytes!("../lexaloud/icons/lexaloud.svg");
pub const TRAY_ICON_SIZE: u32 = 64;

/// Straight RGBA bytes, `TRAY_ICON_SIZE * TRAY_ICON_SIZE * 4` long.
pub fn render_tray_icon(running: bool) -> Option<Vec<u8>> {
    let opacity = if running { 1.0 } else { 0.35 };
    render_icon_rgba(TRAY_ICON_SIZE, opacity)
}

/// StatusNotifierItem pixmap: ARGB32, network byte order.
pub fn render_tray_icon_argb32(running: bool) -> Option<ksni::Icon> {
    let rgba = render_tray_icon(running)?;
    Some(rgba_to_argb32_icon(
        TRAY_ICON_SIZE as i32,
        TRAY_ICON_SIZE as i32,
        &rgba,
    ))
}

pub fn rgba_to_argb32_icon(width: i32, height: i32, rgba: &[u8]) -> ksni::Icon {
    let mut data = Vec::with_capacity(rgba.len());
    for px in rgba.chunks_exact(4) {
        data.extend_from_slice(&[px[3], px[0], px[1], px[2]]);
    }
    ksni::Icon {
        width,
        height,
        data,
    }
}

fn render_icon_rgba(size: u32, opacity: f32) -> Option<Vec<u8>> {
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(ICON_SVG, &opt).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)?;
    let scale = size as f32 / tree.size().width().max(tree.size().height());
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for px in pixmap.pixels() {
        let a = ((px.alpha() as f32) * opacity).round() as u8;
        let (r, g, b) = if px.alpha() == 0 || a == 0 {
            (0, 0, 0)
        } else {
            let unpremul = |c: u8| -> u8 {
                (((c as u32) * 255 + (px.alpha() as u32 / 2)) / px.alpha() as u32) as u8
            };
            let r = ((unpremul(px.red()) as f32) * opacity).round() as u8;
            let g = ((unpremul(px.green()) as f32) * opacity).round() as u8;
            let b = ((unpremul(px.blue()) as f32) * opacity).round() as u8;
            (r, g, b)
        };
        rgba.extend_from_slice(&[r, g, b, a]);
    }
    Some(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_icon_is_straight_rgba_64() {
        let bytes = render_tray_icon(true).expect("svg render");
        assert_eq!(bytes.len(), (TRAY_ICON_SIZE * TRAY_ICON_SIZE * 4) as usize);
        let dim = render_tray_icon(false).expect("svg render dim");
        assert_eq!(dim.len(), bytes.len());
        let icon = render_tray_icon_argb32(true).expect("argb");
        assert_eq!(icon.data.len(), bytes.len());
        assert_eq!(icon.data[0], bytes[3]);
    }
}
