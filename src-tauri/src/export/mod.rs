pub mod helpers;
pub mod html;
pub mod markdown;
pub mod pdf;

use crate::recorder::types::Step;
use std::path::Path;

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

/// Turn an IO error into a user-friendly message.
fn friendly_write_error(e: &std::io::Error, path: &str) -> String {
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => {
            format!("Cannot save to \"{path}\" — permission denied. Is the file open in another app or the folder read-only?")
        }
        std::io::ErrorKind::NotFound => {
            format!("The folder for \"{path}\" does not exist.")
        }
        _ if e.raw_os_error() == Some(28) /* ENOSPC */ => {
            "Not enough disk space to save the file.".to_string()
        }
        _ => format!("Could not save file: {e}"),
    }
}

/// Pre-validate that we can write to `output_path` before doing expensive work.
///
/// Checks: parent dir writable (tempfile probe), existing file writable,
/// sufficient disk space. Total cost: ~3 syscalls, <1ms.
fn validate_write_access(output_path: &str, estimated_bytes: u64) -> Result<(), String> {
    let path = Path::new(output_path);

    let parent = path.parent().ok_or_else(|| {
        format!("Invalid output path: \"{output_path}\"")
    })?;

    if !parent.exists() {
        return Err(format!(
            "The folder \"{}\" does not exist.",
            parent.display()
        ));
    }

    // Probe writability: create a temp file in the same directory
    let probe_path = parent.join(format!(".stepcast_probe_{}", std::process::id()));
    match std::fs::File::create(&probe_path) {
        Ok(_) => { let _ = std::fs::remove_file(&probe_path); }
        Err(e) => {
            let _ = std::fs::remove_file(&probe_path);
            return Err(match e.kind() {
                std::io::ErrorKind::PermissionDenied => format!(
                    "Cannot write to folder \"{}\" — permission denied.",
                    parent.display()
                ),
                _ => format!("Cannot write to folder \"{}\": {e}", parent.display()),
            });
        }
    }

    // If target file exists, verify it is writable (opens without truncating)
    if path.exists() {
        if let Err(e) = std::fs::OpenOptions::new().write(true).open(path) {
            return Err(match e.kind() {
                std::io::ErrorKind::PermissionDenied => format!(
                    "Cannot overwrite \"{}\" — the file is read-only or locked.",
                    path.display()
                ),
                _ => format!("Cannot write to \"{}\": {e}", path.display()),
            });
        }
    }

    // Check available disk space via statvfs
    if let Some(dir_str) = parent.to_str() {
        if let Ok(avail) = available_disk_space(dir_str) {
            const MIN_BUFFER: u64 = 10 * 1024 * 1024; // 10 MB safety margin
            let needed = estimated_bytes + MIN_BUFFER;
            if avail < needed {
                let need_mb = needed / (1024 * 1024);
                let have_mb = avail / (1024 * 1024);
                return Err(format!(
                    "Not enough disk space. Need ~{need_mb} MB, but only {have_mb} MB available."
                ));
            }
        }
    }

    Ok(())
}

/// Returns available disk space in bytes for the filesystem containing `path`.
fn available_disk_space(path: &str) -> std::io::Result<u64> {
    let c_path = std::ffi::CString::new(path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(stat.f_bavail as u64 * stat.f_frsize)
}

/// Unified export: writes the given steps to output_path in the requested format.
pub fn export(title: &str, steps: &[Step], format: ExportFormat, output_path: &str, app: &tauri::AppHandle) -> Result<(), String> {
    // Pre-validate before expensive work (~500KB per step estimate)
    let estimated_bytes = (steps.len() as u64) * 500_000 + 100_000;
    validate_write_access(output_path, estimated_bytes)?;

    match format {
        ExportFormat::Html => {
            let content = html::generate(title, steps);
            std::fs::write(output_path, content)
                .map_err(|e| friendly_write_error(&e, output_path))
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

    #[test]
    fn validate_write_access_writable_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.html").to_str().unwrap().to_string();
        assert!(validate_write_access(&path, 1000).is_ok());
    }

    #[test]
    fn validate_write_access_nonexistent_parent() {
        let result = validate_write_access("/nonexistent/dir/file.html", 1000);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn available_disk_space_current_dir() {
        let space = available_disk_space(".").unwrap();
        assert!(space > 0);
    }
}
