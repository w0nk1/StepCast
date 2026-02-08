use crate::recorder::types::Step;
use std::sync::mpsc;

/// Export steps as PDF using macOS WKWebView.createPDF() (macOS 11+).
pub fn write(
    title: &str,
    steps: &[Step],
    output_path: &str,
    app: &tauri::AppHandle,
) -> Result<(), String> {
    let html = super::html::generate(title, steps);
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
        WKNavigationDelegate, WKNavigation, WKPDFConfiguration, WKWebView,
        WKWebViewConfiguration,
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
                        let bytes = (*data).to_vec();
                        std::fs::write(&path, bytes)
                            .map_err(|e| format!("Failed to write PDF file: {e}"))
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

    let webview = unsafe {
        WKWebView::initWithFrame_configuration(WKWebView::alloc(mtm), frame, &config)
    };

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
            capture_status: None,
            capture_error: None,
        };
        let result = super::super::html::generate("Test", &[step]);
        assert!(result.contains("<!doctype html>"));
    }
}
