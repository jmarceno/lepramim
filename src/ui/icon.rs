//! Render the Lepramim SVG icon at tray sizes with optional opacity tint.
//!
//! Returns straight RGBA pixels, plus ARGB32 pixmaps for StatusNotifierItem.

const ICON_SVG: &[u8] = include_bytes!("../lepramim/icons/lepramim.svg");
pub const TRAY_ICON_SIZE: u32 = 64;

const BLUE_LIGHT: &str = "#28aaa9";
const BLUE_DARK: &str = "#024e67";
const GREEN_LIGHT: &str = "#6edf8a";
const GREEN_DARK: &str = "#3db66a";

/// `mix` in `0.0..=1.0`: blue at 0, green at 1.
pub fn render_tray_icon_with_mix(running: bool, mix: f32) -> Option<Vec<u8>> {
    let opacity = if running { 1.0 } else { 0.35 };
    render_icon_rgba(TRAY_ICON_SIZE, opacity, mix)
}

pub fn render_tray_icon_argb32_with_mix(running: bool, mix: f32) -> Option<ksni::Icon> {
    let rgba = render_tray_icon_with_mix(running, mix)?;
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

fn lerp_hex_color(from: &str, to: &str, mix: f32) -> String {
    let parse = |s: &str| {
        let hex = s.trim_start_matches('#');
        u32::from_str_radix(hex, 16).unwrap_or(0)
    };
    let a = parse(from);
    let b = parse(to);
    let t = mix.clamp(0.0, 1.0);
    let lerp =
        |ca: u32, cb: u32| -> u32 { ((ca as f32) * (1.0 - t) + (cb as f32) * t).round() as u32 };
    let r = lerp((a >> 16) & 0xff, (b >> 16) & 0xff);
    let g = lerp((a >> 8) & 0xff, (b >> 8) & 0xff);
    let bl = lerp(a & 0xff, b & 0xff);
    format!("#{:02x}{:02x}{:02x}", r, g, bl)
}

fn icon_svg_for_mix(mix: f32) -> Vec<u8> {
    let svg = std::str::from_utf8(ICON_SVG).unwrap_or("");
    let light = lerp_hex_color(BLUE_LIGHT, GREEN_LIGHT, mix);
    let dark = lerp_hex_color(BLUE_DARK, GREEN_DARK, mix);
    svg.replace(BLUE_LIGHT, &light)
        .replace(BLUE_DARK, &dark)
        .into_bytes()
}

fn render_icon_rgba(size: u32, opacity: f32, mix: f32) -> Option<Vec<u8>> {
    let svg = icon_svg_for_mix(mix);
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(&svg, &opt).ok()?;
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

    fn avg_green_channel(rgba: &[u8]) -> f32 {
        let mut sum = 0u64;
        let mut count = 0u64;
        for px in rgba.chunks_exact(4) {
            if px[3] > 0 {
                sum += px[1] as u64;
                count += 1;
            }
        }
        if count == 0 {
            0.0
        } else {
            sum as f32 / count as f32
        }
    }

    fn avg_alpha(rgba: &[u8]) -> f32 {
        let mut sum = 0u64;
        let mut count = 0u64;
        for px in rgba.chunks_exact(4) {
            if px[3] > 0 {
                sum += px[3] as u64;
                count += 1;
            }
        }
        if count == 0 {
            0.0
        } else {
            sum as f32 / count as f32
        }
    }

    #[test]
    fn tray_icon_is_straight_rgba_64() {
        let bytes = render_tray_icon_with_mix(true, 0.0).expect("svg render");
        assert_eq!(bytes.len(), (TRAY_ICON_SIZE * TRAY_ICON_SIZE * 4) as usize);
        let dim = render_tray_icon_with_mix(false, 0.0).expect("svg render dim");
        assert_eq!(dim.len(), bytes.len());
        let icon = render_tray_icon_argb32_with_mix(true, 0.0).expect("argb");
        assert_eq!(icon.data.len(), bytes.len());
        assert_eq!(icon.data[0], bytes[3]);
    }

    #[test]
    fn mix_one_is_greener_than_mix_zero() {
        let blue = render_tray_icon_with_mix(true, 0.0).expect("svg render");
        let green = render_tray_icon_with_mix(true, 1.0).expect("svg render");
        assert!(
            avg_green_channel(&green) > avg_green_channel(&blue),
            "green mix should raise average green channel"
        );
    }

    #[test]
    fn stopped_icon_is_dimmer() {
        let bright = render_tray_icon_with_mix(true, 0.0).expect("svg render");
        let dim = render_tray_icon_with_mix(false, 0.0).expect("svg render");
        assert!(
            avg_alpha(&dim) < avg_alpha(&bright),
            "stopped tray icon should be more transparent"
        );
    }
}
