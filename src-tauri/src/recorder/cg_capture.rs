use crate::recorder::capture::CaptureError;
use std::path::Path;

/// Capture a screen region using CoreGraphics (fast, in-process).
/// Falls back to CLI capture if the image cannot be converted.
pub fn capture_region_fast(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    output_path: &Path,
) -> Result<(), CaptureError> {
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};
    use core_graphics::window::{
        create_image, kCGNullWindowID, kCGWindowImageBestResolution,
        kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly,
    };
    use image::RgbaImage;

    if width <= 0 || height <= 0 {
        return Err(CaptureError::CgImage("invalid capture size".to_string()));
    }

    let rect = CGRect::new(
        &CGPoint::new(x as f64, y as f64),
        &CGSize::new(width as f64, height as f64),
    );

    let image = create_image(
        rect,
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
        kCGNullWindowID,
        kCGWindowImageBestResolution,
    )
    .ok_or_else(|| CaptureError::CgImage("CGWindowListCreateImage returned null".to_string()))?;

    let image_ref = image.as_ref();
    let w = image_ref.width() as usize;
    let h = image_ref.height() as usize;
    if w == 0 || h == 0 {
        return Err(CaptureError::CgImage("empty CGImage".to_string()));
    }

    let bytes_per_row = image_ref.bytes_per_row() as usize;
    let bytes_per_pixel = (image_ref.bits_per_pixel() / 8) as usize;
    if bytes_per_pixel < 4 {
        return Err(CaptureError::CgImage(
            "unsupported pixel format".to_string(),
        ));
    }

    let data = image_ref.data();
    let bytes = data.bytes();
    let needed = bytes_per_row.saturating_mul(h);
    if bytes.len() < needed {
        return Err(CaptureError::CgImage(
            "CGImage buffer too small".to_string(),
        ));
    }

    // CGWindowListCreateImage typically returns BGRA (premultiplied). Convert to RGBA.
    let mut out = vec![0u8; w * h * 4];
    for row in 0..h {
        let src_row = row * bytes_per_row;
        let dst_row = row * w * 4;
        let src = &bytes[src_row..src_row + w * bytes_per_pixel];
        let dst = &mut out[dst_row..dst_row + w * 4];
        for px in 0..w {
            let si = px * bytes_per_pixel;
            let di = px * 4;
            let b = src[si];
            let g = src[si + 1];
            let r = src[si + 2];
            let a = src[si + 3];
            dst[di] = r;
            dst[di + 1] = g;
            dst[di + 2] = b;
            dst[di + 3] = a;
        }
    }

    let img = RgbaImage::from_raw(w as u32, h as u32, out)
        .ok_or_else(|| CaptureError::CgImage("failed to build image buffer".to_string()))?;
    img.save(output_path)
        .map_err(|e| CaptureError::CgImage(format!("fast capture save failed: {e}")))?;

    Ok(())
}

/// Capture a specific window by its CGWindow ID using CoreGraphics.
/// This captures the window content even if it's partially obscured or closing,
/// avoiding race conditions where the window disappears before a region capture.
pub fn capture_window_cg(window_id: u32, output_path: &Path) -> Result<(), CaptureError> {
    use core_graphics::geometry::{CGPoint, CGRect, CGSize};
    use core_graphics::window::{
        create_image, kCGWindowImageBestResolution, kCGWindowImageBoundsIgnoreFraming,
    };
    use image::RgbaImage;

    // kCGWindowListOptionIncludingWindow = 1 << 3 = 8
    const K_CG_WINDOW_LIST_OPTION_INCLUDING_WINDOW: u32 = 1 << 3;

    // CGRectNull tells CGWindowListCreateImage to use the window's own bounds
    let null_rect = CGRect::new(
        &CGPoint::new(f64::INFINITY, f64::INFINITY),
        &CGSize::new(0.0, 0.0),
    );

    let image = create_image(
        null_rect,
        K_CG_WINDOW_LIST_OPTION_INCLUDING_WINDOW,
        window_id,
        kCGWindowImageBestResolution | kCGWindowImageBoundsIgnoreFraming,
    )
    .ok_or_else(|| {
        CaptureError::CgImage("CGWindowListCreateImage returned null for window ID".to_string())
    })?;

    let image_ref = image.as_ref();
    let w = image_ref.width() as usize;
    let h = image_ref.height() as usize;
    if w == 0 || h == 0 {
        return Err(CaptureError::CgImage(
            "empty CGImage for window capture".to_string(),
        ));
    }

    let bytes_per_row = image_ref.bytes_per_row() as usize;
    let bytes_per_pixel = (image_ref.bits_per_pixel() / 8) as usize;
    if bytes_per_pixel < 4 {
        return Err(CaptureError::CgImage(
            "unsupported pixel format".to_string(),
        ));
    }

    let data = image_ref.data();
    let bytes = data.bytes();
    let needed = bytes_per_row.saturating_mul(h);
    if bytes.len() < needed {
        return Err(CaptureError::CgImage(
            "CGImage buffer too small".to_string(),
        ));
    }

    let mut out = vec![0u8; w * h * 4];
    for row in 0..h {
        let src_row = row * bytes_per_row;
        let dst_row = row * w * 4;
        let src = &bytes[src_row..src_row + w * bytes_per_pixel];
        let dst = &mut out[dst_row..dst_row + w * 4];
        for px in 0..w {
            let si = px * bytes_per_pixel;
            let di = px * 4;
            let b = src[si];
            let g = src[si + 1];
            let r = src[si + 2];
            let a = src[si + 3];
            dst[di] = r;
            dst[di + 1] = g;
            dst[di + 2] = b;
            dst[di + 3] = a;
        }
    }

    let img = RgbaImage::from_raw(w as u32, h as u32, out)
        .ok_or_else(|| CaptureError::CgImage("failed to build image buffer".to_string()))?;
    img.save(output_path)
        .map_err(|e| CaptureError::CgImage(format!("window capture save failed: {e}")))?;

    Ok(())
}

/// Capture a screen region using macOS screencapture CLI.
/// This provides clean compositing of UI elements including menubar vibrancy effects.
pub fn capture_region_cg(
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    output_path: &Path,
) -> Result<(), CaptureError> {
    use std::process::Command;

    let region_arg = format!("{x},{y},{width},{height}");

    let status = Command::new("screencapture")
        .args(["-x", "-R", &region_arg, output_path.to_str().unwrap_or("")])
        .status()
        .map_err(|e| CaptureError::CgImage(format!("screencapture failed: {e}")))?;

    if !status.success() {
        return Err(CaptureError::CgImage(
            "screencapture returned non-zero".to_string(),
        ));
    }

    Ok(())
}
