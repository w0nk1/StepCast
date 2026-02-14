use crate::recorder::types::{ActionType, BoundsPercent, Step};
use base64::Engine;
use std::fs;

/// Check if a step represents an authentication placeholder
pub fn is_auth_placeholder(step: &Step) -> bool {
    step.window_title == "Authentication dialog (secure)"
        || step.app.to_lowercase() == "authentication"
}

fn normalize_crop_region(crop_region: Option<&BoundsPercent>) -> Option<BoundsPercent> {
    let crop = crop_region?;
    let values = [
        crop.x_percent,
        crop.y_percent,
        crop.width_percent,
        crop.height_percent,
    ];
    if values.iter().any(|v| !v.is_finite()) {
        return None;
    }

    let x = crop.x_percent.clamp(0.0, 100.0);
    let y = crop.y_percent.clamp(0.0, 100.0);
    let mut w = crop.width_percent.clamp(0.0, 100.0);
    let mut h = crop.height_percent.clamp(0.0, 100.0);
    if x + w > 100.0 {
        w = (100.0 - x).max(0.0);
    }
    if y + h > 100.0 {
        h = (100.0 - y).max(0.0);
    }
    if w < 2.0 || h < 2.0 {
        return None;
    }

    Some(BoundsPercent {
        x_percent: x,
        y_percent: y,
        width_percent: w,
        height_percent: h,
    })
}

fn crop_rect_px(
    img_w: u32,
    img_h: u32,
    crop_region: Option<&BoundsPercent>,
) -> Option<(u32, u32, u32, u32)> {
    let crop = normalize_crop_region(crop_region)?;
    if crop.x_percent <= 0.0
        && crop.y_percent <= 0.0
        && crop.width_percent >= 99.99
        && crop.height_percent >= 99.99
    {
        return None;
    }

    let w_f = img_w as f64;
    let h_f = img_h as f64;
    let x = ((crop.x_percent as f64 / 100.0) * w_f).floor() as u32;
    let y = ((crop.y_percent as f64 / 100.0) * h_f).floor() as u32;
    let mut width = ((crop.width_percent as f64 / 100.0) * w_f).round() as u32;
    let mut height = ((crop.height_percent as f64 / 100.0) * h_f).round() as u32;

    if x >= img_w || y >= img_h {
        return None;
    }
    width = width.clamp(1, img_w.saturating_sub(x).max(1));
    height = height.clamp(1, img_h.saturating_sub(y).max(1));
    Some((x, y, width, height))
}

fn maybe_crop_image(raw: &[u8], crop_region: Option<&BoundsPercent>) -> Option<Vec<u8>> {
    let img = image::load_from_memory(raw).ok()?;
    let (x, y, width, height) = crop_rect_px(img.width(), img.height(), crop_region)?;
    let cropped = img.crop_imm(x, y, width, height);
    let mut out = std::io::Cursor::new(Vec::new());
    if cropped.write_to(&mut out, image::ImageFormat::Png).is_err() {
        return None;
    }
    Some(out.into_inner())
}

/// Map click marker into cropped image coordinate space.
/// Returns `None` when marker is outside the crop.
pub fn marker_position_percent(step: &Step) -> Option<(f32, f32)> {
    if step.screenshot_path.is_none() || is_auth_placeholder(step) {
        return None;
    }
    let click_x = step.click_x_percent.clamp(0.0, 100.0);
    let click_y = step.click_y_percent.clamp(0.0, 100.0);
    let Some(crop) = normalize_crop_region(step.crop_region.as_ref()) else {
        return Some((click_x, click_y));
    };

    let x = ((click_x - crop.x_percent) / crop.width_percent) * 100.0;
    let y = ((click_y - crop.y_percent) / crop.height_percent) * 100.0;
    if !(0.0..=100.0).contains(&x) || !(0.0..=100.0).contains(&y) {
        return None;
    }
    Some((x.clamp(0.0, 100.0), y.clamp(0.0, 100.0)))
}

/// Human-readable description of what happened in a step
pub fn action_description(step: &Step) -> String {
    if is_auth_placeholder(step) {
        return "Authenticate with Touch ID or enter your password to continue.".to_string();
    }

    match step.action {
        ActionType::Note => "Note".to_string(),
        _ => {
            let verb = match step.action {
                ActionType::DoubleClick => "Double-clicked in",
                ActionType::RightClick => "Right-clicked in",
                ActionType::Shortcut => "Used keyboard shortcut in",
                _ => "Clicked in",
            };
            format!("{} {} \u{2014} \"{}\"", verb, step.app, step.window_title)
        }
    }
}

/// Prefer enhanced description if present, otherwise fall back to `action_description`.
pub fn effective_description(step: &Step) -> String {
    let desc = step.description.as_deref().unwrap_or("").trim();
    if !desc.is_empty() {
        return desc.to_string();
    }
    action_description(step)
}

/// Image data with format metadata for export.
pub struct OptimizedImage {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
    pub ext: &'static str,
}

/// Convert raw PNG bytes to WebP. Falls back to PNG if conversion fails
/// or if the WebP output is not smaller.
pub fn to_webp_or_png(png_bytes: &[u8]) -> OptimizedImage {
    if let Ok(img) = image::load_from_memory(png_bytes) {
        let mut buf = std::io::Cursor::new(Vec::new());
        if img.write_to(&mut buf, image::ImageFormat::WebP).is_ok() {
            let webp_bytes = buf.into_inner();
            if webp_bytes.len() < png_bytes.len() {
                return OptimizedImage {
                    bytes: webp_bytes,
                    mime: "image/webp",
                    ext: "webp",
                };
            }
        }
    }
    OptimizedImage {
        bytes: png_bytes.to_vec(),
        mime: "image/png",
        ext: "png",
    }
}

/// Target format for image optimization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageTarget {
    /// WebP for standalone HTML / Markdown (smallest for web).
    Web,
    /// JPEG for PDF (PDF spec supports JPEG natively via DCTDecode).
    Pdf,
}

/// Load a screenshot and return optimized bytes + MIME/ext.
pub fn load_screenshot_optimized_image(
    path: &str,
    target: ImageTarget,
    crop_region: Option<&BoundsPercent>,
) -> Option<OptimizedImage> {
    let raw = fs::read(path).ok()?;
    let cropped = maybe_crop_image(&raw, crop_region);
    let source = cropped.as_deref().unwrap_or(&raw);
    let img = match target {
        ImageTarget::Web => to_webp_or_png(source),
        ImageTarget::Pdf => to_jpeg(source),
    };
    Some(img)
}

/// Load a screenshot, convert to optimized format, return base64 + MIME.
pub fn load_screenshot_optimized(
    path: &str,
    target: ImageTarget,
    crop_region: Option<&BoundsPercent>,
) -> Option<(String, &'static str)> {
    let img = load_screenshot_optimized_image(path, target, crop_region)?;
    Some((
        base64::engine::general_purpose::STANDARD.encode(&img.bytes),
        img.mime,
    ))
}

/// Convert raw PNG bytes to JPEG at quality 85. Falls back to PNG on failure.
pub fn to_jpeg(png_bytes: &[u8]) -> OptimizedImage {
    use image::ImageEncoder;
    if let Ok(img) = image::load_from_memory(png_bytes) {
        // JPEG doesn't support alpha — convert RGBA to RGB
        let rgb = img.to_rgb8();
        let mut buf = std::io::Cursor::new(Vec::new());
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 85);
        if encoder
            .write_image(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                image::ExtendedColorType::Rgb8,
            )
            .is_ok()
        {
            return OptimizedImage {
                bytes: buf.into_inner(),
                mime: "image/jpeg",
                ext: "jpg",
            };
        }
    }
    OptimizedImage {
        bytes: png_bytes.to_vec(),
        mime: "image/png",
        ext: "png",
    }
}

/// Escape HTML special characters
pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Convert a title to a filesystem-safe slug
#[allow(dead_code)]
pub fn slugify_title(title: &str) -> String {
    slug::slugify(title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::types::{ActionType, Step};

    fn sample_step() -> Step {
        Step {
            id: "s1".into(),
            ts: 0,
            action: ActionType::Click,
            x: 10,
            y: 20,
            click_x_percent: 50.0,
            click_y_percent: 50.0,
            app: "Finder".into(),
            window_title: "Downloads".into(),
            screenshot_path: None,
            note: None,
            description: None,
            description_source: None,
            description_status: None,
            description_error: None,
            ax: None,
            capture_status: None,
            capture_error: None,
            crop_region: None,
        }
    }

    #[test]
    fn action_description_click() {
        let s = sample_step();
        assert_eq!(
            action_description(&s),
            "Clicked in Finder \u{2014} \"Downloads\""
        );
    }

    #[test]
    fn action_description_double_click() {
        let mut s = sample_step();
        s.action = ActionType::DoubleClick;
        assert_eq!(
            action_description(&s),
            "Double-clicked in Finder \u{2014} \"Downloads\""
        );
    }

    #[test]
    fn action_description_right_click() {
        let mut s = sample_step();
        s.action = ActionType::RightClick;
        assert_eq!(
            action_description(&s),
            "Right-clicked in Finder \u{2014} \"Downloads\""
        );
    }

    #[test]
    fn action_description_shortcut() {
        let mut s = sample_step();
        s.action = ActionType::Shortcut;
        assert_eq!(
            action_description(&s),
            "Used keyboard shortcut in Finder \u{2014} \"Downloads\""
        );
    }

    #[test]
    fn action_description_note() {
        let mut s = sample_step();
        s.action = ActionType::Note;
        s.note = Some("Remember to save".into());
        assert_eq!(action_description(&s), "Note");
    }

    #[test]
    fn action_description_auth_placeholder() {
        let mut s = sample_step();
        s.window_title = "Authentication dialog (secure)".into();
        assert_eq!(
            action_description(&s),
            "Authenticate with Touch ID or enter your password to continue."
        );
    }

    #[test]
    fn action_description_auth_by_app() {
        let mut s = sample_step();
        s.app = "Authentication".into();
        assert_eq!(
            action_description(&s),
            "Authenticate with Touch ID or enter your password to continue."
        );
    }

    #[test]
    fn effective_description_prefers_step_description() {
        let mut s = sample_step();
        s.description = Some("Click \"Share\"".into());
        assert_eq!(effective_description(&s), "Click \"Share\"");
    }

    #[test]
    fn effective_description_falls_back_when_empty() {
        let mut s = sample_step();
        s.description = Some("   ".into());
        assert!(effective_description(&s).contains("Clicked in Finder"));
    }

    #[test]
    fn html_escape_special_chars() {
        assert_eq!(html_escape("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }

    #[test]
    fn html_escape_quotes() {
        assert_eq!(html_escape(r#"say "hello""#), "say &quot;hello&quot;");
    }

    #[test]
    fn slugify_title_basic() {
        assert_eq!(slugify_title("My Guide Title"), "my-guide-title");
    }

    #[test]
    fn slugify_title_umlauts() {
        let result = slugify_title("\u{00c4}rger mit \u{00d6}lf\u{00f6}rderung");
        assert!(!result.contains('\u{00e4}'));
        assert!(!result.contains('\u{00f6}'));
        assert!(result
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-'));
    }

    #[test]
    fn slugify_title_special_chars() {
        assert_eq!(slugify_title("Hello World! (2026)"), "hello-world-2026");
    }

    #[test]
    fn is_auth_placeholder_checks() {
        let mut s = sample_step();
        assert!(!is_auth_placeholder(&s));

        s.window_title = "Authentication dialog (secure)".into();
        assert!(is_auth_placeholder(&s));

        s.window_title = "Normal".into();
        s.app = "Authentication".into();
        assert!(is_auth_placeholder(&s));
    }

    #[test]
    fn to_webp_or_png_converts_valid_png() {
        // Create a small 2x2 red PNG in memory
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
        let mut png_buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png_buf, image::ImageFormat::Png).unwrap();
        let png_bytes = png_buf.into_inner();

        let result = to_webp_or_png(&png_bytes);
        // Should produce either webp or png — both are valid
        assert!(result.mime == "image/webp" || result.mime == "image/png");
        assert!(result.ext == "webp" || result.ext == "png");
        assert!(!result.bytes.is_empty());
    }

    #[test]
    fn to_webp_or_png_falls_back_on_garbage() {
        let garbage = b"not an image at all";
        let result = to_webp_or_png(garbage);
        assert_eq!(result.mime, "image/png");
        assert_eq!(result.ext, "png");
        assert_eq!(result.bytes, garbage);
    }

    #[test]
    fn load_screenshot_optimized_missing_file() {
        assert!(
            load_screenshot_optimized("/nonexistent/file.png", ImageTarget::Web, None).is_none()
        );
    }

    #[test]
    fn marker_position_percent_without_crop() {
        let mut s = sample_step();
        s.screenshot_path = Some("/tmp/x.png".into());
        s.click_x_percent = 70.0;
        s.click_y_percent = 30.0;
        assert_eq!(marker_position_percent(&s), Some((70.0, 30.0)));
    }

    #[test]
    fn marker_position_percent_with_crop() {
        let mut s = sample_step();
        s.screenshot_path = Some("/tmp/x.png".into());
        s.click_x_percent = 50.0;
        s.click_y_percent = 50.0;
        s.crop_region = Some(BoundsPercent {
            x_percent: 25.0,
            y_percent: 25.0,
            width_percent: 50.0,
            height_percent: 50.0,
        });
        assert_eq!(marker_position_percent(&s), Some((50.0, 50.0)));
    }

    #[test]
    fn load_screenshot_optimized_image_applies_crop() {
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let img = image::RgbaImage::from_pixel(200, 100, image::Rgba([255, 0, 0, 255]));
        let img_path = tmp.path().join("shot.png");
        img.save(&img_path).unwrap();

        let out = load_screenshot_optimized_image(
            img_path.to_str().unwrap(),
            ImageTarget::Web,
            Some(&BoundsPercent {
                x_percent: 25.0,
                y_percent: 20.0,
                width_percent: 50.0,
                height_percent: 50.0,
            }),
        )
        .expect("optimized image");

        let decoded = image::load_from_memory(&out.bytes).expect("decode optimized image");
        assert_eq!(decoded.width(), 100);
        assert_eq!(decoded.height(), 50);
    }

    #[test]
    fn to_jpeg_converts_valid_png() {
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
        let mut png_buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png_buf, image::ImageFormat::Png).unwrap();
        let png_bytes = png_buf.into_inner();

        let result = to_jpeg(&png_bytes);
        assert_eq!(result.mime, "image/jpeg");
        assert_eq!(result.ext, "jpg");
        // JPEG starts with FF D8 FF
        assert_eq!(result.bytes[0], 0xFF);
        assert_eq!(result.bytes[1], 0xD8);
    }

    #[test]
    fn to_jpeg_falls_back_on_garbage() {
        let garbage = b"not an image";
        let result = to_jpeg(garbage);
        assert_eq!(result.mime, "image/png");
        assert_eq!(result.ext, "png");
    }

    #[test]
    fn jpeg_conversion_realistic_screenshot() {
        use image::{ImageFormat, Rgba, RgbaImage};
        let (w, h) = (1440u32, 900u32);
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let r = ((x * 255) / w) as u8;
                let g = ((y * 255) / h) as u8;
                let b = (((x + y) * 128) / (w + h)) as u8;
                img.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }

        let mut png_buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png_buf, ImageFormat::Png).unwrap();
        let png_bytes = png_buf.into_inner();

        let result = to_jpeg(&png_bytes);
        let png_kb = png_bytes.len() / 1024;
        let jpg_kb = result.bytes.len() / 1024;
        eprintln!(
            "Screenshot 1440x900: PNG={png_kb}KB → JPEG={jpg_kb}KB ({}% savings)",
            if png_kb > 0 {
                100 - (jpg_kb * 100 / png_kb)
            } else {
                0
            }
        );

        assert_eq!(result.mime, "image/jpeg");
        assert!(
            result.bytes.len() < png_bytes.len(),
            "JPEG should be smaller than PNG"
        );
    }

    #[test]
    fn webp_conversion_realistic_screenshot() {
        // Simulate a 1440x900 screenshot with varied content (gradient + noise)
        use image::{ImageFormat, Rgba, RgbaImage};
        let (w, h) = (1440u32, 900u32);
        let mut img = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let r = ((x * 255) / w) as u8;
                let g = ((y * 255) / h) as u8;
                let b = (((x + y) * 128) / (w + h)) as u8;
                img.put_pixel(x, y, Rgba([r, g, b, 255]));
            }
        }

        let mut png_buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut png_buf, ImageFormat::Png).unwrap();
        let png_bytes = png_buf.into_inner();

        let result = to_webp_or_png(&png_bytes);

        let png_kb = png_bytes.len() / 1024;
        let out_kb = result.bytes.len() / 1024;
        eprintln!(
            "Screenshot 1440x900: PNG={png_kb}KB → {}={out_kb}KB ({}% savings)",
            result.ext,
            if png_kb > 0 {
                100 - (out_kb * 100 / png_kb)
            } else {
                0
            }
        );

        assert_eq!(
            result.ext, "webp",
            "WebP should be smaller for a realistic screenshot"
        );
        assert_eq!(result.mime, "image/webp");
        // WebP should be meaningfully smaller (at least 20%)
        assert!(
            result.bytes.len() < png_bytes.len() * 80 / 100,
            "WebP ({out_kb}KB) should be at least 20% smaller than PNG ({png_kb}KB)"
        );
    }
}
