pub mod helpers;
pub mod html;
pub mod markdown;
pub mod pdf;

use crate::recorder::types::Step;

#[derive(Debug, Clone, Copy)]
pub enum ExportFormat {
    Html,
    Markdown,
    Pdf,
}

impl ExportFormat {
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "html" => Ok(Self::Html),
            "md" => Ok(Self::Markdown),
            "pdf" => Ok(Self::Pdf),
            other => Err(format!("Unknown export format: {other}")),
        }
    }
}

/// Unified export: writes the given steps to output_path in the requested format.
pub fn export(title: &str, steps: &[Step], format: ExportFormat, output_path: &str, app: &tauri::AppHandle) -> Result<(), String> {
    match format {
        ExportFormat::Html => {
            let content = html::generate(title, steps);
            std::fs::write(output_path, content)
                .map_err(|e| format!("Failed to write HTML: {e}"))
        }
        ExportFormat::Markdown => {
            markdown::write(title, steps, output_path)
        }
        ExportFormat::Pdf => {
            pdf::write(title, steps, output_path, app)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_from_str_valid() {
        assert!(matches!(ExportFormat::from_str("html"), Ok(ExportFormat::Html)));
        assert!(matches!(ExportFormat::from_str("md"), Ok(ExportFormat::Markdown)));
        assert!(matches!(ExportFormat::from_str("pdf"), Ok(ExportFormat::Pdf)));
    }

    #[test]
    fn format_from_str_invalid() {
        assert!(ExportFormat::from_str("docx").is_err());
    }
}
