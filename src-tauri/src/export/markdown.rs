use super::helpers::{effective_description, load_screenshot_optimized_image, ImageTarget};
use crate::recorder::types::Step;
use std::fs;
use std::io::{Cursor, Write as _};
use std::path::Path;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// Derive the images directory name from a stem.
/// "My Guide" → "My Guide-images"
pub fn images_dir_name(output_path: &Path) -> String {
    let stem = output_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("export");
    format!("{stem}-images")
}

/// Generate markdown content. `images_dir` is the relative folder name for images.
/// `image_exts` maps step index (0-based) to file extension ("webp" or "png").
pub fn generate_content(
    title: &str,
    steps: &[Step],
    images_dir: &str,
    image_exts: &[&str],
) -> String {
    let mut md = format!(
        "# {title} — {count} step{plural}\n\n",
        count = steps.len(),
        plural = if steps.len() == 1 { "" } else { "s" },
    );

    for (i, step) in steps.iter().enumerate() {
        let num = i + 1;
        let desc = effective_description(step);

        md.push_str(&format!("## Step {num}\n\n"));

        md.push_str(&format!("**{desc}**\n\n"));

        // Image reference (relative path into images dir)
        if step.screenshot_path.is_some() {
            let ext = image_exts.get(i).unwrap_or(&"png");
            md.push_str(&format!(
                "![Step {num}](<./{images_dir}/step-{num}.{ext}>)\n\n"
            ));
        }

        if let Some(note) = &step.note {
            md.push_str(&format!("> {note}\n\n"));
        }
    }

    md
}

/// Write a zip archive containing the markdown file and screenshot images.
/// `output_path` should end in `.zip`. The inner `.md` file derives its name
/// from the zip stem: "My Guide.zip" → "My Guide.md".
pub fn write(title: &str, steps: &[Step], output_path: &str) -> Result<(), String> {
    let path = Path::new(output_path);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("export");
    let md_filename = format!("{stem}.md");
    let images_dir = images_dir_name(path);

    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // Convert images and collect (bytes, extension) per step
    let mut converted: Vec<Option<(Vec<u8>, &str)>> = Vec::with_capacity(steps.len());
    for (i, step) in steps.iter().enumerate() {
        if let Some(src) = &step.screenshot_path {
            let img =
                load_screenshot_optimized_image(src, ImageTarget::Web, step.crop_region.as_ref())
                    .ok_or_else(|| format!("Failed to read screenshot {}: {src}", i + 1))?;
            converted.push(Some((img.bytes, img.ext)));
        } else {
            converted.push(None);
        }
    }

    let image_exts: Vec<&str> = converted
        .iter()
        .map(|c| c.as_ref().map(|(_, ext)| *ext).unwrap_or("png"))
        .collect();
    let content = generate_content(title, steps, &images_dir, &image_exts);

    let buf: Vec<u8> = {
        let cursor = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(cursor);

        // Write the markdown file
        zip.start_file(&md_filename, opts)
            .map_err(|e| format!("Failed to create md entry in zip: {e}"))?;
        zip.write_all(content.as_bytes())
            .map_err(|e| format!("Failed to write md content: {e}"))?;

        // Write screenshot images
        for (i, conv) in converted.iter().enumerate() {
            if let Some((bytes, ext)) = conv {
                let entry_name = format!("{images_dir}/step-{}.{ext}", i + 1);
                zip.start_file(&entry_name, opts)
                    .map_err(|e| format!("Failed to create image entry in zip: {e}"))?;
                zip.write_all(bytes)
                    .map_err(|e| format!("Failed to write image data: {e}"))?;
            }
        }

        zip.finish()
            .map_err(|e| format!("Failed to finalize zip: {e}"))?
            .into_inner()
    };

    fs::write(output_path, buf).map_err(|e| super::friendly_write_error(&e, output_path))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::types::{ActionType, Step};
    use std::path::Path;

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
    fn generate_contains_title() {
        let md = generate_content(
            "Test Guide",
            &[sample_step()],
            "test-guide-images",
            &["png"],
        );
        assert!(md.starts_with("# Test Guide — "));
    }

    #[test]
    fn generate_contains_step_count() {
        let md = generate_content(
            "G",
            &[sample_step(), sample_step()],
            "g-images",
            &["png", "png"],
        );
        assert!(md.contains("2 steps"));
    }

    #[test]
    fn generate_contains_action_desc() {
        let md = generate_content("G", &[sample_step()], "g-images", &["png"]);
        assert!(md.contains("Clicked in Finder"));
    }

    #[test]
    fn generate_shortcut_text() {
        let mut s = sample_step();
        s.action = ActionType::Shortcut;
        let md = generate_content("G", &[s], "g-images", &["png"]);
        assert!(md.contains("Used keyboard shortcut in"));
    }

    #[test]
    fn generate_includes_note() {
        let mut s = sample_step();
        s.note = Some("Important!".into());
        let md = generate_content("G", &[s], "g-images", &["png"]);
        assert!(md.contains("> Important!"));
    }

    #[test]
    fn generate_image_references_webp() {
        let mut s = sample_step();
        s.screenshot_path = Some("/tmp/nonexistent-fake-file.png".into());
        let md = generate_content("G", &[s], "my-guide-images", &["webp"]);
        assert!(md.contains("![Step 1](<./my-guide-images/step-1.webp>)"));
    }

    #[test]
    fn generate_image_references_png_fallback() {
        let mut s = sample_step();
        s.screenshot_path = Some("/tmp/nonexistent-fake-file.png".into());
        let md = generate_content("G", &[s], "my-guide-images", &["png"]);
        assert!(md.contains("![Step 1](<./my-guide-images/step-1.png>)"));
    }

    #[test]
    fn generate_no_image_when_no_screenshot() {
        let s = sample_step(); // screenshot_path is None
        let md = generate_content("G", &[s], "g-images", &["png"]);
        assert!(!md.contains("!["));
    }

    #[test]
    fn images_dir_name_from_output_path() {
        let p = Path::new("/Users/me/docs/My Guide.md");
        assert_eq!(images_dir_name(p), "My Guide-images");
    }

    #[test]
    fn images_dir_name_no_extension() {
        let p = Path::new("/Users/me/docs/readme");
        assert_eq!(images_dir_name(p), "readme-images");
    }

    #[test]
    fn write_creates_valid_zip() {
        use std::io::Cursor;
        use tempfile::TempDir;
        use zip::ZipArchive;

        let tmp = TempDir::new().unwrap();

        // Create a real small PNG so WebP conversion can work
        let img = image::RgbaImage::from_pixel(4, 4, image::Rgba([0, 128, 255, 255]));
        let img_path = tmp.path().join("screenshot.png");
        img.save(&img_path).unwrap();

        let mut step_with_img = sample_step();
        step_with_img.screenshot_path = Some(img_path.to_str().unwrap().to_string());

        let step_no_img = sample_step();

        let zip_path = tmp.path().join("My Guide.zip");
        write(
            "My Guide",
            &[step_with_img, step_no_img],
            zip_path.to_str().unwrap(),
        )
        .unwrap();

        assert!(zip_path.exists());

        // Verify zip contents
        let data = std::fs::read(&zip_path).unwrap();
        let mut archive = ZipArchive::new(Cursor::new(data)).unwrap();

        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();

        assert!(names.contains(&"My Guide.md".to_string()));
        // Image could be webp or png depending on which is smaller
        let has_image = names.iter().any(|n| {
            n.starts_with("My Guide-images/step-1.")
                && (n.ends_with(".webp") || n.ends_with(".png"))
        });
        assert!(has_image, "Expected image file in zip, got: {names:?}");
        assert_eq!(names.len(), 2);

        // Verify markdown content references correct extension
        let mut md_entry = archive.by_name("My Guide.md").unwrap();
        let mut md_content = String::new();
        std::io::Read::read_to_string(&mut md_entry, &mut md_content).unwrap();
        assert!(md_content.contains("# My Guide"));
        assert!(md_content.contains("2 steps"));
        // The image reference should match the actual file in the zip
        let img_name = names
            .iter()
            .find(|n| n.starts_with("My Guide-images/step-1."))
            .unwrap();
        let ext = img_name.rsplit('.').next().unwrap();
        assert!(md_content.contains(&format!("step-1.{ext}")));
    }

    /// End-to-end: realistic 1440x900 screenshot → zip with WebP image + correct md reference
    #[test]
    fn write_zip_uses_webp_for_large_screenshot() {
        use std::io::Cursor;
        use tempfile::TempDir;
        use zip::ZipArchive;

        let tmp = TempDir::new().unwrap();

        // Create a realistic 1440x900 gradient image
        let mut img = image::RgbaImage::new(1440, 900);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            *pixel = image::Rgba([((x * 255) / 1440) as u8, ((y * 255) / 900) as u8, 128, 255]);
        }
        let img_path = tmp.path().join("screenshot.png");
        img.save(&img_path).unwrap();
        let png_size = std::fs::metadata(&img_path).unwrap().len();

        let mut step = sample_step();
        step.screenshot_path = Some(img_path.to_str().unwrap().to_string());

        let zip_path = tmp.path().join("Guide.zip");
        write("Guide", &[step], zip_path.to_str().unwrap()).unwrap();

        let data = std::fs::read(&zip_path).unwrap();
        let mut archive = ZipArchive::new(Cursor::new(data)).unwrap();

        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();

        // Must be WebP for a realistic screenshot
        assert!(
            names.iter().any(|n| n.ends_with(".webp")),
            "Expected .webp image in zip, got: {names:?}"
        );

        // Verify the WebP bytes are valid (start with RIFF...WEBP header)
        let webp_name = names.iter().find(|n| n.ends_with(".webp")).unwrap().clone();
        let webp_bytes = {
            let mut entry = archive.by_name(&webp_name).unwrap();
            let mut buf = Vec::new();
            std::io::Read::read_to_end(&mut entry, &mut buf).unwrap();
            buf
        };
        assert_eq!(
            &webp_bytes[..4],
            b"RIFF",
            "WebP must start with RIFF header"
        );
        assert_eq!(&webp_bytes[8..12], b"WEBP", "WebP must contain WEBP marker");

        // Verify markdown references .webp
        let md_content = {
            let mut entry = archive.by_name("Guide.md").unwrap();
            let mut buf = String::new();
            std::io::Read::read_to_string(&mut entry, &mut buf).unwrap();
            buf
        };
        assert!(md_content.contains("step-1.webp"));

        let webp_size = webp_bytes.len() as u64;
        eprintln!(
            "E2E: PNG={png_size}B → WebP={webp_size}B ({}% smaller)",
            100 - (webp_size * 100 / png_size)
        );
    }
}
