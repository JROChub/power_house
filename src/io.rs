//! Minimal file helpers for writing ledger artifacts without external crates.

use std::fs::{create_dir_all, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

/// Writes a text file to `base_dir/prefix_index.txt` using the provided lines.
pub fn write_text_series(
    base_dir: impl AsRef<Path>,
    prefix: &str,
    index: usize,
    lines: &[String],
) -> io::Result<PathBuf> {
    let dir = base_dir.as_ref();
    create_dir_all(dir)?;
    let filename = format!("{}_{:04}.txt", prefix, index);
    let path = dir.join(filename);
    let file = File::create(&path)?;
    let mut writer = BufWriter::new(file);
    for line in lines {
        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::write_text_series;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_write_text_series() {
        let base = std::env::temp_dir();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let tmp_dir = base.join(format!("power_house_test_{}", unique));
        fs::create_dir_all(&tmp_dir).unwrap();
        let lines = vec!["hello".to_string(), "world".to_string()];
        let path = write_text_series(&tmp_dir, "test", 1, &lines).unwrap();
        assert!(path.ends_with(PathBuf::from("test_0001.txt")));
        let contents = fs::read_to_string(path).unwrap();
        assert_eq!(contents, "hello\nworld\n");
        fs::remove_dir_all(&tmp_dir).unwrap();
    }
}
