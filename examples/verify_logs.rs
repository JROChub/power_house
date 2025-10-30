use power_house::verify_transcript_lines;
use std::fs;
use std::io::{self, BufRead};
use std::path::PathBuf;

fn read_lines(path: &PathBuf) -> io::Result<Vec<String>> {
    let file = fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;
    Ok(lines)
}

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let dir = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("power_house_ledger_logs"));
    println!("Verifying logs in {}", dir.display());
    let mut entries = fs::read_dir(&dir)?
        .filter_map(|res| res.ok())
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());
    if entries.is_empty() {
        println!("No log files found.");
        return Ok(());
    }
    for entry in entries {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("?");
        let lines = read_lines(&path)?;
        match verify_transcript_lines(lines.iter().map(|s| s.as_str())) {
            Ok(()) => println!("[ok]   {}", name),
            Err(err) => println!("[fail] {} -> {}", name, err),
        }
    }
    Ok(())
}
