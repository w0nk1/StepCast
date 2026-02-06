use crate::recorder::types::Step;
use super::helpers::action_description;
use std::fs;
use std::io::{Cursor, Write as _};
use std::path::Path;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

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
pub fn generate_content(title: &str, steps: &[Step], images_dir: &str) -> String {
    let mut md = format!(
        "# {title} — {count} step{plural}\n\n",
        count = steps.len(),
        plural = if steps.len() == 1 { "" } else { "s" },
    );

    for (i, step) in steps.iter().enumerate() {
        let num = i + 1;
        let desc = action_description(step);

        md.push_str(&format!("## Step {num}\n\n"));

        md.push_str(&format!("**{desc}**\n\n"));

        // Image reference (relative path into images dir)
        if step.screenshot_path.is_some() {
            md.push_str(&format!("![Step {num}](<./{images_dir}/step-{num}.png>)\n\n"));
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

    let content = generate_content(title, steps, &images_dir);

    let opts = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let buf: Vec<u8> = {
        let cursor = Cursor::new(Vec::new());
        let mut zip = ZipWriter::new(cursor);

        // Write the markdown file
        zip.start_file(&md_filename, opts)
            .map_err(|e| format!("Failed to create md entry in zip: {e}"))?;
        zip.write_all(content.as_bytes())
            .map_err(|e| format!("Failed to write md content: {e}"))?;

        // Write screenshot images
        for (i, step) in steps.iter().enumerate() {
            if let Some(src) = &step.screenshot_path {
                let entry_name = format!("{images_dir}/step-{}.png", i + 1);
                let img_data = fs::read(src)
                    .map_err(|e| format!("Failed to read screenshot {}: {e}", i + 1))?;
                zip.start_file(&entry_name, opts)
                    .map_err(|e| format!("Failed to create image entry in zip: {e}"))?;
                zip.write_all(&img_data)
                    .map_err(|e| format!("Failed to write image data: {e}"))?;
            }
        }

        zip.finish()
            .map_err(|e| format!("Failed to finalize zip: {e}"))?
            .into_inner()
    };

    fs::write(output_path, buf)
        .map_err(|e| format!("Failed to write zip file: {e}"))?;

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
            x: 10, y: 20,
            click_x_percent: 50.0,
            click_y_percent: 50.0,
            app: "Finder".into(),
            window_title: "Downloads".into(),
            screenshot_path: None,
            note: None,
        }
    }

    #[test]
    fn generate_contains_title() {
        let md = generate_content("Test Guide", &[sample_step()], "test-guide-images");
        assert!(md.starts_with("# Test Guide — "));
    }

    #[test]
    fn generate_contains_step_count() {
        let md = generate_content("G", &[sample_step(), sample_step()], "g-images");
        assert!(md.contains("2 steps"));
    }

    #[test]
    fn generate_contains_action_desc() {
        let md = generate_content("G", &[sample_step()], "g-images");
        assert!(md.contains("Clicked in Finder"));
    }

    #[test]
    fn generate_shortcut_text() {
        let mut s = sample_step();
        s.action = ActionType::Shortcut;
        let md = generate_content("G", &[s], "g-images");
        assert!(md.contains("Used keyboard shortcut in"));
    }

    #[test]
    fn generate_includes_note() {
        let mut s = sample_step();
        s.note = Some("Important!".into());
        let md = generate_content("G", &[s], "g-images");
        assert!(md.contains("> Important!"));
    }

    #[test]
    fn generate_image_references_folder() {
        let mut s = sample_step();
        s.screenshot_path = Some("/tmp/nonexistent-fake-file.png".into());
        let md = generate_content("G", &[s], "my-guide-images");
        // Image reference should be present even if file doesn't exist (generate_content doesn't check files)
        assert!(md.contains("![Step 1](<./my-guide-images/step-1.png>)"));
    }

    #[test]
    fn generate_no_image_when_no_screenshot() {
        let s = sample_step(); // screenshot_path is None
        let md = generate_content("G", &[s], "g-images");
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
        use tempfile::TempDir;
        use std::io::Cursor;
        use zip::ZipArchive;

        let tmp = TempDir::new().unwrap();

        // Create a fake screenshot file
        let img_path = tmp.path().join("fake-screenshot.png");
        std::fs::write(&img_path, b"PNG_DATA").unwrap();

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

        // Should contain the .md file and one image (only step 1 has a screenshot)
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();

        assert!(names.contains(&"My Guide.md".to_string()));
        assert!(names.contains(&"My Guide-images/step-1.png".to_string()));
        assert_eq!(names.len(), 2);

        // Verify markdown content
        let mut md_entry = archive.by_name("My Guide.md").unwrap();
        let mut md_content = String::new();
        std::io::Read::read_to_string(&mut md_entry, &mut md_content).unwrap();
        assert!(md_content.contains("# My Guide"));
        assert!(md_content.contains("2 steps"));
    }
}
