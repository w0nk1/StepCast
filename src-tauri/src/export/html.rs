use crate::recorder::types::{ActionType, Step};
use super::helpers::{action_description, html_escape, is_auth_placeholder, load_screenshot_optimized, ImageTarget};

/// Generate a self-contained HTML document from steps.
pub fn generate(title: &str, steps: &[Step]) -> String {
    generate_for(title, steps, ImageTarget::Web)
}

/// Generate HTML with a specific image target (Web = WebP, Pdf = JPEG).
pub fn generate_for(title: &str, steps: &[Step], target: ImageTarget) -> String {
    let steps_html: String = steps
        .iter()
        .enumerate()
        .map(|(i, step)| render_step(i + 1, step, target))
        .collect();

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title_esc}</title>
<style>
{css}
</style>
</head>
<body>
<div class="container">
<h1>{title_esc}</h1>
<p class="subtitle">{count} step{plural}</p>
<div class="timeline">
{steps_html}
</div>
</div>
</body>
</html>"#,
        title_esc = html_escape(title),
        css = CSS,
        count = steps.len(),
        plural = if steps.len() == 1 { "" } else { "s" },
        steps_html = steps_html,
    )
}

fn render_step(num: usize, step: &Step, target: ImageTarget) -> String {
    let desc = html_escape(&action_description(step));

    let image_html = step.screenshot_path.as_ref()
        .and_then(|p| load_screenshot_optimized(p, target))
        .map(|(b64, mime)| format!(r#"<img src="data:{mime};base64,{b64}" alt="Step {num}">"#))
        .unwrap_or_default();

    let marker_class = match step.action {
        ActionType::DoubleClick => "click-marker double-click",
        ActionType::RightClick => "click-marker right-click",
        _ => "click-marker",
    };

    let click_marker = if step.screenshot_path.is_some() && !is_auth_placeholder(step) {
        format!(
            r#"<div class="{}" style="left: {}%; top: {}%;"></div>"#,
            marker_class, step.click_x_percent, step.click_y_percent
        )
    } else {
        String::new()
    };

    let note_html = step.note.as_ref()
        .map(|n| format!(r#"<p class="step-note">{}</p>"#, escape_text(n)))
        .unwrap_or_default();

    format!(
        r#"
    <div class="timeline-item">
      <div class="timeline-badge">{num}</div>
      <article class="step">
        <div class="step-header">
          <span class="step-desc">{desc}</span>
        </div>
        <div class="step-image">
          <div class="image-wrapper">
            {image_html}
            {click_marker}
          </div>
        </div>
        {note_html}
      </article>
    </div>"#
    )
}

/// Escape text content (includes single quotes for note text)
fn escape_text(s: &str) -> String {
    html_escape(s).replace('\'', "&#x27;")
}

const CSS: &str = r#"* { box-sizing: border-box; margin: 0; padding: 0; }
body { font-family: -apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', sans-serif; background: #f5f5f7; color: #1d1d1f; line-height: 1.5; -webkit-font-smoothing: antialiased; -webkit-print-color-adjust: exact; print-color-adjust: exact; }
.container { max-width: 860px; margin: 0 auto; padding: 40px 32px 64px; }
h1 { font-size: 20px; font-weight: 700; letter-spacing: -0.01em; margin: 0 0 4px; }
.subtitle { font-size: 14px; color: #86868b; margin-bottom: 32px; }
.timeline { display: flex; flex-direction: column; position: relative; }
.timeline::before { content: ''; position: absolute; left: 15px; top: 16px; bottom: 16px; width: 2px; background: #d1d1d6; border-radius: 1px; }
.timeline-item { display: grid; grid-template-columns: 32px 1fr; gap: 16px; padding-bottom: 24px; position: relative; }
.timeline-item:last-child { padding-bottom: 0; }
.timeline-badge { width: 32px; height: 32px; border-radius: 50%; background: #7c5cfc; color: #fff; font-size: 13px; font-weight: 700; display: flex; align-items: center; justify-content: center; position: relative; z-index: 1; box-shadow: 0 0 0 4px #f5f5f7; flex-shrink: 0; }
.step { border: 1px solid #d1d1d6; border-radius: 14px; overflow: hidden; background: #fff; box-shadow: 0 1px 3px rgba(0,0,0,0.04), 0 4px 12px rgba(0,0,0,0.03); }
.step-header { display: flex; align-items: center; gap: 12px; padding: 14px 20px; }
.step-desc { font-size: 14px; font-weight: 600; color: #1d1d1f; }
.step-image { padding: 0 20px 16px; display: flex; align-items: center; justify-content: center; }
.image-wrapper { position: relative; display: inline-block; max-width: 100%; border-radius: 8px; overflow: hidden; box-shadow: 0 1px 2px rgba(0,0,0,0.06), 0 4px 16px rgba(0,0,0,0.08); border: 1px solid #d1d1d6; }
.image-wrapper img { display: block; max-width: 100%; height: auto; }
.step-note { margin: 0; padding: 12px 20px 16px; font-size: 13px; color: #1d1d1f; background: rgba(124,92,252,0.05); border-top: none; }
.click-marker { position: absolute; width: 24px; height: 24px; border-radius: 50%; background: transparent; border: 2.5px solid #ff3b30; box-shadow: 0 0 0 1.5px rgba(255,255,255,0.9), 0 2px 6px rgba(0,0,0,0.25); transform: translate(-50%, -50%); pointer-events: none; }
.click-marker.double-click { width: 18px; height: 18px; border-width: 2px; }
.click-marker.double-click::after { content: ''; position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); width: 30px; height: 30px; border-radius: 50%; border: 2px solid #ff3b30; box-shadow: 0 0 0 1.5px rgba(255,255,255,0.9); pointer-events: none; }
.click-marker.right-click { border-style: dashed; }
@media print {
  body { background: #fff !important; }
  .container { padding: 20px !important; }
  .timeline::before { background: #d1d1d6 !important; }
  .timeline-badge { box-shadow: 0 0 0 4px #fff !important; }
  .timeline-item { break-inside: avoid; }
  .step { box-shadow: none !important; border-color: #d1d1d6 !important; }
}
@media (prefers-color-scheme: dark) {
  body { background: #1c1c1e; color: #f5f5f7; }
  .subtitle { color: #98989d; }
  .timeline::before { background: #38383a; }
  .timeline-badge { box-shadow: 0 0 0 4px #1c1c1e; }
  .step { background: #2c2c2e; border-color: #38383a; box-shadow: inset 0 1px 0 rgba(255,255,255,0.04), 0 1px 3px rgba(0,0,0,0.2), 0 4px 12px rgba(0,0,0,0.15); }
  .step-desc { color: #f5f5f7; }
  .image-wrapper { border-color: #38383a; }
  .step-note { color: #f5f5f7; background: rgba(167,139,250,0.08); }
}"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::types::{ActionType, Step};

    fn sample_step() -> Step {
        Step {
            id: "s1".into(),
            ts: 0,
            action: ActionType::Click,
            x: 10, y: 20,
            click_x_percent: 50.0,
            click_y_percent: 50.0,
            app: "Finder".into(),
            window_title: "Downloads".into(),
            screenshot_path: None,
            note: None,
            capture_status: None,
            capture_error: None,
        }
    }

    #[test]
    fn generate_contains_title() {
        let html = generate("Test Guide", &[sample_step()]);
        assert!(html.contains("<title>Test Guide</title>"));
        assert!(html.contains("<h1>Test Guide</h1>"));
        assert!(html.contains("1 step"));
    }

    #[test]
    fn generate_contains_step_count() {
        let html = generate("G", &[sample_step(), sample_step()]);
        assert!(html.contains("2 steps"));
    }

    #[test]
    fn generate_contains_dark_mode() {
        let html = generate("G", &[sample_step()]);
        assert!(html.contains("prefers-color-scheme: dark"));
    }

    #[test]
    fn generate_contains_timeline_structure() {
        let html = generate("G", &[sample_step()]);
        assert!(html.contains("timeline-badge"));
        assert!(html.contains("timeline-item"));
        assert!(html.contains("Clicked in Finder"));
    }

    #[test]
    fn generate_includes_note_when_present() {
        let mut s = sample_step();
        s.note = Some("Don't forget this!".into());
        let html = generate("G", &[s]);
        assert!(html.contains("step-note"));
        assert!(html.contains("Don&#x27;t forget this!"));
    }

    #[test]
    fn generate_no_note_div_when_absent() {
        let html = generate("G", &[sample_step()]);
        assert!(!html.contains(r#"<p class="step-note">"#));
    }

    #[test]
    fn generate_shortcut_action_text() {
        let mut s = sample_step();
        s.action = ActionType::Shortcut;
        let html = generate("G", &[s]);
        assert!(html.contains("Used keyboard shortcut in"));
    }

    #[test]
    fn generate_click_marker_classes() {
        let mut dc = sample_step();
        dc.action = ActionType::DoubleClick;
        let html = generate("G", &[dc]);
        assert!(html.contains("double-click"));

        let mut rc = sample_step();
        rc.action = ActionType::RightClick;
        let html = generate("G", &[rc]);
        assert!(html.contains("right-click"));
    }

    #[test]
    fn html_escape_in_title() {
        let html = generate("<script>alert(1)</script>", &[]);
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    /// E2E: PDF target uses JPEG data URIs
    #[test]
    fn generate_for_pdf_uses_jpeg() {
        use super::super::helpers::ImageTarget;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let mut img = image::RgbaImage::new(100, 100);
        for pixel in img.pixels_mut() {
            *pixel = image::Rgba([200, 100, 50, 255]);
        }
        let img_path = tmp.path().join("screenshot.png");
        img.save(&img_path).unwrap();

        let mut step = sample_step();
        step.screenshot_path = Some(img_path.to_str().unwrap().to_string());

        let html = generate_for("Test", &[step], ImageTarget::Pdf);
        assert!(
            html.contains("data:image/jpeg;base64,"),
            "PDF target should use JPEG data URI"
        );
        assert!(
            !html.contains("data:image/webp;base64,"),
            "PDF target should not contain WebP"
        );
    }

    /// E2E: realistic screenshot → HTML with WebP data URI
    #[test]
    fn generate_uses_webp_for_real_screenshot() {
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();

        // Create a 1440x900 gradient screenshot
        let mut img = image::RgbaImage::new(1440, 900);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            *pixel = image::Rgba([
                ((x * 255) / 1440) as u8,
                ((y * 255) / 900) as u8,
                128, 255,
            ]);
        }
        let img_path = tmp.path().join("screenshot.png");
        img.save(&img_path).unwrap();

        let mut step = sample_step();
        step.screenshot_path = Some(img_path.to_str().unwrap().to_string());

        let html = generate("Test", &[step]);

        // Should embed as WebP, not PNG
        assert!(
            html.contains("data:image/webp;base64,"),
            "Expected WebP data URI in HTML output"
        );
        assert!(
            !html.contains("data:image/png;base64,"),
            "Should not contain PNG data URI when WebP is smaller"
        );
    }
}
