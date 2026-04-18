use qrencode::types::Color;
use qrencode::QrCode;

/// Generate a QR code as ASCII art (for serial console output).
///
/// Uses Unicode block characters for compact display — each character
/// represents 2 vertical modules.
pub fn generate_qr_ascii(data: &str) -> Result<String, String> {
    let code = QrCode::new(data.as_bytes()).map_err(|e| format!("QR encode failed: {e}"))?;
    let width = code.width();
    let mut result = String::new();
    // Each character covers 2 rows via half-block Unicode chars
    for y in (0..width).step_by(2) {
        for x in 0..width {
            let top = code[(x, y)] == Color::Dark;
            let bottom = if y + 1 < width {
                code[(x, y + 1)] == Color::Dark
            } else {
                false
            };
            result.push(match (top, bottom) {
                (true, true) => '\u{2588}',  // Full block
                (true, false) => '\u{2580}', // Upper half
                (false, true) => '\u{2584}', // Lower half
                (false, false) => ' ',
            });
        }
        result.push('\n');
    }
    Ok(result)
}

/// Generate QR code as inline SVG string (for web UI display).
pub fn generate_qr_svg(data: &str) -> Result<String, String> {
    let code = QrCode::new(data.as_bytes()).map_err(|e| format!("QR encode failed: {e}"))?;
    let width = code.width();
    let scale = 4;
    let margin = scale * 2;
    let size = width * scale + margin * 2;
    let mut svg =
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {size} {size}">"#);
    svg.push_str(&format!(
        r#"<rect width="{size}" height="{size}" fill="white"/>"#
    ));
    for y in 0..width {
        for x in 0..width {
            if code[(x, y)] == Color::Dark {
                svg.push_str(&format!(
                    r#"<rect x="{}" y="{}" width="{scale}" height="{scale}" fill="black"/>"#,
                    x * scale + margin,
                    y * scale + margin,
                ));
            }
        }
    }
    svg.push_str("</svg>");
    Ok(svg)
}
