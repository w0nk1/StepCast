use super::types::Step;
use std::{io, path::Path};

#[derive(Debug)]
pub enum StorageError {
    Io(io::Error),
    Json(serde_json::Error),
}

impl From<io::Error> for StorageError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

pub fn write_steps(dir: &Path, steps: &[Step]) -> Result<(), StorageError> {
    let json = serde_json::to_string_pretty(steps)?;
    let path = dir.join("steps.json");
    std::fs::write(path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn writes_steps_json() {
        let dir = tempdir().expect("tempdir");
        let steps = vec![Step::sample(), Step::sample()];

        write_steps(dir.path(), &steps).expect("write steps");

        let json_path = dir.path().join("steps.json");
        let contents = fs::read_to_string(json_path).expect("read steps.json");
        let parsed: Vec<Step> = serde_json::from_str(&contents).expect("parse steps");

        assert_eq!(steps, parsed);
    }
}
