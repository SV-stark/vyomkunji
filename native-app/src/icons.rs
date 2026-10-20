// Tray / window icon, drawn at runtime so no image decoder is needed.
use egui::Color32;

/// RGBA bytes of the Signer mark: a rounded square in the brand colour with
/// a white keyhole in it (a keyhole reads as "key" at 16 px).
pub fn app_icon_rgba(size: usize) -> Vec<u8> {
    let accent = Color32::from_rgb(0x4C, 0x8D, 0xFF);
    let mut out = vec![0u8; size * size * 4];
    let s = size as f32;
    let half = s / 2.0;
    let radius = s * 0.26;
    let circle_r = s * 0.11;
    let circle_c = (half, half - s * 0.09);
    let bar_w = s * 0.09;
    let bar_h = s * 0.18;

    for y in 0..size {
        for x in 0..size {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;

            // Rounded square.
            let d = rounded_rect_sdf(px - half, py - half, half - radius, half - radius, radius);
            let square = (0.5 - d).clamp(0.0, 1.0);
            if square <= 0.0 {
                continue;
            }

            // Keyhole: a circle over a short bar.
            let dc = ((px - circle_c.0).powi(2) + (py - circle_c.1).powi(2)).sqrt();
            let circle = (circle_r - dc).clamp(0.0, 1.0);
            let bar_top = circle_c.1 + circle_r * 0.45;
            let in_bar_x = (px - half).abs() <= bar_w * 0.5;
            let in_bar_y = py >= bar_top && py <= bar_top + bar_h;
            let bar = if in_bar_x && in_bar_y { 1.0 } else { 0.0 };
            let hole = circle.max(bar);

            let r = accent.r() as f32 * (1.0 - hole) + 255.0 * hole;
            let g = accent.g() as f32 * (1.0 - hole) + 255.0 * hole;
            let b = accent.b() as f32 * (1.0 - hole) + 255.0 * hole;
            let a = square;
            let i = (y * size + x) * 4;
            out[i] = r as u8;
            out[i + 1] = g as u8;
            out[i + 2] = b as u8;
            out[i + 3] = (a * 255.0) as u8;
        }
    }
    out
}

/// Signed distance to a rounded rectangle centred on the origin.
fn rounded_rect_sdf(x: f32, y: f32, half_w: f32, half_h: f32, r: f32) -> f32 {
    let qx = (x.abs() - (half_w - r)).max(0.0);
    let qy = (y.abs() - (half_h - r)).max(0.0);
    let outside = (qx * qx + qy * qy).sqrt();
    let inside = (x.abs() - half_w).max(y.abs() - half_h).max(0.0);
    outside + inside - r
}
