use crate::recorder::types::Step;
use std::sync::mpsc;

/// Post-process PDF bytes via PDFKit to optimize images.
///
/// Applies `saveAllImagesAsJPEG` + `optimizeImagesForScreen` options.
/// Best-effort: returns original bytes on any failure.
fn optimize_pdf_bytes(pdf_bytes: &[u8]) -> Vec<u8> {
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::AnyThread;
    use objc2_foundation::{NSData, NSDictionary, NSNumber, NSString};
    use objc2_pdf_kit::{
        PDFDocument, PDFDocumentOptimizeImagesForScreenOption, PDFDocumentSaveImagesAsJPEGOption,
    };

    // Try to load as PDFDocument
    let ns_data = NSData::with_bytes(pdf_bytes);
    let Some(doc) = (unsafe { PDFDocument::initWithData(PDFDocument::alloc(), &ns_data) }) else {
        return pdf_bytes.to_vec();
    };

    // Build options dict via msg_send (bypass typed NSDictionary generics)
    let yes = NSNumber::new_bool(true);
    let keys: [&NSString; 2] = unsafe {
        [
            PDFDocumentSaveImagesAsJPEGOption,
            PDFDocumentOptimizeImagesForScreenOption,
        ]
    };
    let vals: [&NSNumber; 2] = [&yes, &yes];
    let options: Retained<NSDictionary> = unsafe {
        msg_send![
            NSDictionary::alloc(),
            initWithObjects: vals.as_ptr(),
            forKeys: keys.as_ptr(),
            count: 2usize,
        ]
    };

    // Re-encode with options; fall back to original on failure
    let Some(optimized) = (unsafe { doc.dataRepresentationWithOptions(&options) }) else {
        return pdf_bytes.to_vec();
    };

    optimized.to_vec()
}

/// Export steps as PDF using macOS WKWebView.createPDF() (macOS 11+).
pub fn write(
    title: &str,
    steps: &[Step],
    output_path: &str,
    app: &tauri::AppHandle,
) -> Result<(), String> {
    let html = super::html::generate_for(title, steps, super::helpers::ImageTarget::Pdf);
    let path = output_path.to_string();

    let (tx, rx) = mpsc::channel::<Result<(), String>>();

    app.run_on_main_thread(move || {
        render_pdf_on_main_thread(&html, &path, tx);
    })
    .map_err(|e| format!("Failed to dispatch to main thread: {e}"))?;

    rx.recv_timeout(std::time::Duration::from_secs(30))
        .map_err(|_| "PDF generation timed out (30s)".to_string())?
}

/// Must be called on the main thread. Creates an off-screen WKWebView,
/// loads the HTML, waits for navigation to finish, then calls createPDF.
fn render_pdf_on_main_thread(html: &str, output_path: &str, tx: mpsc::Sender<Result<(), String>>) {
    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::{NSObjectProtocol, ProtocolObject};
    use objc2::{define_class, msg_send, DefinedClass, MainThreadOnly};
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_foundation::{MainThreadMarker, NSData, NSError, NSObject, NSString};
    use objc2_web_kit::{
        WKNavigation, WKNavigationDelegate, WKPDFConfiguration, WKWebView, WKWebViewConfiguration,
    };

    // SAFETY: This function is only called from run_on_main_thread.
    let mtm = unsafe { MainThreadMarker::new_unchecked() };

    // ── Navigation delegate ────────────────────────────────────────────
    // When didFinishNavigation fires, we generate the PDF.

    struct DelegateIvars {
        output_path: String,
        tx: Option<mpsc::Sender<Result<(), String>>>,
        webview: Option<Retained<WKWebView>>,
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[ivars = DelegateIvars]
        struct NavDelegate;

        unsafe impl NSObjectProtocol for NavDelegate {}

        unsafe impl WKNavigationDelegate for NavDelegate {
            #[unsafe(method(webView:didFinishNavigation:))]
            #[allow(non_snake_case)]
            unsafe fn webView_didFinishNavigation(
                &self,
                web_view: &WKWebView,
                _navigation: Option<&WKNavigation>,
            ) {
                let ivars = self.ivars();
                let path = ivars.output_path.clone();

                // Take the sender so it's consumed exactly once.
                let tx: mpsc::Sender<Result<(), String>> = {
                    let ptr = ivars as *const DelegateIvars as *mut DelegateIvars;
                    match (*ptr).tx.take() {
                        Some(tx) => tx,
                        None => return, // already fired
                    }
                };

                // SAFETY: We are on the main thread (delegate is MainThreadOnly).
                let mtm = MainThreadMarker::new_unchecked();
                let pdf_config = WKPDFConfiguration::new(mtm);

                let block = RcBlock::new(move |data: *mut NSData, error: *mut NSError| {
                    let result = if !data.is_null() {
                        let raw_bytes = (*data).to_vec();
                        let bytes = optimize_pdf_bytes(&raw_bytes);
                        std::fs::write(&path, bytes)
                            .map_err(|e| super::friendly_write_error(&e, &path))
                    } else if !error.is_null() {
                        Err(format!("PDF generation failed: {}", *error))
                    } else {
                        Err("PDF generation failed: no data returned".into())
                    };
                    let _ = tx.send(result);
                });

                web_view.createPDFWithConfiguration_completionHandler(Some(&pdf_config), &block);
            }
        }
    );

    impl NavDelegate {
        fn new(
            mtm: MainThreadMarker,
            output_path: String,
            tx: mpsc::Sender<Result<(), String>>,
        ) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(DelegateIvars {
                output_path,
                tx: Some(tx),
                webview: None,
            });
            unsafe { msg_send![super(this), init] }
        }
    }

    // ── Build the webview ──────────────────────────────────────────────
    let config = unsafe { WKWebViewConfiguration::new(mtm) };

    let frame = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(800.0, 600.0));

    let webview =
        unsafe { WKWebView::initWithFrame_configuration(WKWebView::alloc(mtm), frame, &config) };

    let delegate = NavDelegate::new(mtm, output_path.to_string(), tx);

    // Store webview in delegate so it stays alive.
    {
        let ivars = delegate.ivars();
        let ptr = ivars as *const DelegateIvars as *mut DelegateIvars;
        unsafe { (*ptr).webview = Some(webview.clone()) };
    }

    unsafe {
        webview.setNavigationDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    }

    // Load HTML — this kicks off async loading, didFinishNavigation fires later.
    let ns_html = NSString::from_str(html);
    unsafe {
        webview.loadHTMLString_baseURL(&ns_html, None);
    }

    // The delegate must stay alive until the PDF callback fires.
    // Leak it intentionally; it will be reclaimed when the callback
    // consumes the sender (the only strong ref cycle is broken there).
    std::mem::forget(delegate);
}

#[cfg(test)]
mod tests {
    use super::optimize_pdf_bytes;

    #[test]
    fn optimize_pdf_bytes_returns_original_on_invalid_input() {
        let garbage = b"not a real pdf";
        let result = optimize_pdf_bytes(garbage);
        assert_eq!(
            result, garbage,
            "invalid input should return original bytes"
        );
    }

    #[test]
    fn optimize_pdf_bytes_returns_original_on_empty_input() {
        let empty: &[u8] = &[];
        let result = optimize_pdf_bytes(empty);
        assert_eq!(result, empty, "empty input should return empty bytes");
    }

    #[test]
    fn optimize_pdf_bytes_processes_valid_pdf() {
        // Minimal valid PDF (no images, so optimization is a no-op but should succeed)
        let minimal_pdf = b"%PDF-1.0\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n\
            2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n\
            3 0 obj<</Type/Page/MediaBox[0 0 612 792]/Parent 2 0 R>>endobj\n\
            xref\n0 4\n0000000000 65535 f \n0000000009 00000 n \n\
            0000000058 00000 n \n0000000115 00000 n \n\
            trailer<</Size 4/Root 1 0 R>>\nstartxref\n190\n%%EOF";
        let result = optimize_pdf_bytes(minimal_pdf);
        // Should return *some* valid bytes (possibly re-encoded, possibly original)
        assert!(
            !result.is_empty(),
            "valid PDF should produce non-empty output"
        );
    }

    /// Creates a PDFDocument with a single image page for size testing.
    fn make_pdf_with_image(w: usize, h: usize, pixels: &[u8]) -> Vec<u8> {
        use objc2::AnyThread;
        use objc2_app_kit::{NSBitmapImageRep, NSImage};
        use objc2_foundation::NSSize;
        use objc2_pdf_kit::{PDFDocument, PDFPage};

        let bitmap = unsafe {
            NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                w as isize,
                h as isize,
                8,
                4,
                true,
                false,
                objc2_app_kit::NSDeviceRGBColorSpace,
                (w * 4) as isize,
                32,
            )
        }.expect("bitmap alloc");

        let bitmap_data = bitmap.bitmapData();
        unsafe { std::ptr::copy_nonoverlapping(pixels.as_ptr(), bitmap_data, pixels.len()) };

        let ns_image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(w as f64, h as f64));
        ns_image.addRepresentation(&bitmap);

        let page = unsafe { PDFPage::initWithImage(PDFPage::alloc(), &ns_image) }
            .expect("PDFPage from image");
        let doc: objc2::rc::Retained<PDFDocument> =
            unsafe { objc2::msg_send![PDFDocument::alloc(), init] };
        unsafe { doc.insertPage_atIndex(&page, 0) };

        let raw = unsafe { doc.dataRepresentation() }.expect("PDF bytes");
        raw.to_vec()
    }

    /// Build a PDF with a raw uncompressed image stream (simulating WKWebView
    /// lossless output). Returns the PDF bytes.
    fn make_pdf_with_raw_image_stream(w: usize, h: usize, rgb_data: &[u8]) -> Vec<u8> {
        let stream_len = rgb_data.len();
        // Construct valid PDF with an uncompressed image XObject
        let mut pdf = Vec::new();
        let header = b"%PDF-1.4\n";
        pdf.extend_from_slice(header);

        // Object 1: Catalog
        let obj1_offset = pdf.len();
        let obj1 = b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n";
        pdf.extend_from_slice(obj1);

        // Object 2: Pages
        let obj2_offset = pdf.len();
        let obj2 = b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n";
        pdf.extend_from_slice(obj2);

        // Object 3: Page (references the image XObject)
        let obj3_offset = pdf.len();
        let obj3 = format!(
            "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w} {h}] \
             /Contents 5 0 R /Resources << /XObject << /Im0 4 0 R >> >> >>\nendobj\n"
        );
        pdf.extend_from_slice(obj3.as_bytes());

        // Object 4: Image XObject (raw RGB, no compression filter)
        let obj4_offset = pdf.len();
        let obj4_header = format!(
            "4 0 obj\n<< /Type /XObject /Subtype /Image /Width {w} /Height {h} \
             /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length {stream_len} >>\nstream\n"
        );
        pdf.extend_from_slice(obj4_header.as_bytes());
        pdf.extend_from_slice(rgb_data);
        pdf.extend_from_slice(b"\nendstream\nendobj\n");

        // Object 5: Content stream (draw image full page)
        let obj5_offset = pdf.len();
        let content = format!("{w} 0 0 {h} 0 0 cm /Im0 Do");
        let content_len = content.len();
        let obj5 =
            format!("5 0 obj\n<< /Length {content_len} >>\nstream\n{content}\nendstream\nendobj\n");
        pdf.extend_from_slice(obj5.as_bytes());

        // Cross-reference table
        let xref_offset = pdf.len();
        let xref = format!(
            "xref\n0 6\n\
             0000000000 65535 f \n\
             {obj1_offset:010} 00000 n \n\
             {obj2_offset:010} 00000 n \n\
             {obj3_offset:010} 00000 n \n\
             {obj4_offset:010} 00000 n \n\
             {obj5_offset:010} 00000 n \n"
        );
        pdf.extend_from_slice(xref.as_bytes());

        let trailer =
            format!("trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n");
        pdf.extend_from_slice(trailer.as_bytes());

        pdf
    }

    #[test]
    fn optimize_pdf_bytes_size_reduction() {
        // 1920x1080 "screenshot" — raw RGB pixels with realistic noise
        let (w, h) = (1920usize, 1080usize);
        let mut rgb_data = Vec::with_capacity(w * h * 3);
        let mut seed: u32 = 42;
        for y in 0..h {
            for x in 0..w {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let noise = (seed >> 16) as u8;
                let r = ((x * 255 / w) as u8).wrapping_add(noise >> 2);
                let g = ((y * 255 / h) as u8).wrapping_add(noise >> 3);
                let b = (((x + y) * 128 / (w + h)) as u8).wrapping_add(noise >> 1);
                rgb_data.extend_from_slice(&[r, g, b]);
            }
        }

        // Test 1: PDFKit-created PDF (already optimal)
        let mut rgba_data = Vec::with_capacity(w * h * 4);
        for chunk in rgb_data.chunks(3) {
            rgba_data.extend_from_slice(chunk);
            rgba_data.push(255);
        }
        let pdfkit_bytes = make_pdf_with_image(w, h, &rgba_data);
        let pdfkit_opt = optimize_pdf_bytes(&pdfkit_bytes);

        // Test 2: Raw-stream PDF (simulates WKWebView lossless output)
        let raw_bytes = make_pdf_with_raw_image_stream(w, h, &rgb_data);
        let raw_opt = optimize_pdf_bytes(&raw_bytes);

        let report = |label: &str, orig: &[u8], opt: &[u8]| {
            let o_kb = orig.len() / 1024;
            let n_kb = opt.len() / 1024;
            let pct = if !orig.is_empty() {
                100.0 - (opt.len() as f64 / orig.len() as f64 * 100.0)
            } else {
                0.0
            };
            let orig_str = String::from_utf8_lossy(orig);
            let dct = orig_str.contains("DCTDecode");
            let flate = orig_str.contains("FlateDecode");
            eprintln!("  {label}:");
            eprintln!("    Original:  {o_kb} KB  (DCT={dct}, Flate={flate})");
            eprintln!("    Optimized: {n_kb} KB");
            eprintln!("    Reduction: {pct:.1}%");
        };

        eprintln!("=== PDF Optimization Results (1920x1080 noisy screenshot) ===");
        report("PDFKit-created PDF", &pdfkit_bytes, &pdfkit_opt);
        report("Raw-stream PDF (simulated WKWebView)", &raw_bytes, &raw_opt);
        eprintln!("=============================================================");

        // PDFKit output is already optimal — no reduction expected
        assert!(pdfkit_opt.len() <= pdfkit_bytes.len());
        // Raw-stream PDF should see significant reduction
        assert!(
            raw_opt.len() < raw_bytes.len(),
            "raw-stream PDF ({} KB) should shrink after optimization ({} KB)",
            raw_bytes.len() / 1024,
            raw_opt.len() / 1024
        );
    }

    #[test]
    fn generate_delegates_to_html() {
        use crate::recorder::types::{ActionType, Step};
        let step = Step {
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
        };
        let result = super::super::html::generate("Test", &[step]);
        assert!(result.contains("<!doctype html>"));
    }

    #[test]
    fn pdf_html_uses_jpeg_not_webp() {
        use super::super::helpers::ImageTarget;
        use crate::recorder::types::{ActionType, Step};
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let img = image::RgbaImage::from_pixel(50, 50, image::Rgba([128, 128, 128, 255]));
        let img_path = tmp.path().join("shot.png");
        img.save(&img_path).unwrap();

        let step = Step {
            id: "s1".into(),
            ts: 0,
            action: ActionType::Click,
            x: 10,
            y: 20,
            click_x_percent: 50.0,
            click_y_percent: 50.0,
            app: "Finder".into(),
            window_title: "Downloads".into(),
            screenshot_path: Some(img_path.to_str().unwrap().to_string()),
            note: None,
            description: None,
            description_source: None,
            description_status: None,
            description_error: None,
            ax: None,
            capture_status: None,
            capture_error: None,
            crop_region: None,
        };

        let html = super::super::html::generate_for("Test", &[step], ImageTarget::Pdf);
        assert!(
            html.contains("data:image/jpeg;base64,"),
            "PDF path should use JPEG"
        );
        assert!(
            !html.contains("data:image/webp;base64,"),
            "PDF path should not use WebP"
        );
    }
}
