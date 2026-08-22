//! Command-line entry point for the separate MFENX contract-v1 verifier.

#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use mfenx_contract_v1_verifier::verify;

const USAGE: &str = "Usage: mfenx-contract-v1-verifier --image PATH --result PATH --store DIR [--report PATH]\n\
\n\
Validates and exactly replays an MFENX Local v2 contract-v1 result in this standalone binary.\n\
Omit --report to write the JSON report to stdout.";

#[derive(Default)]
struct Arguments {
    image: Option<PathBuf>,
    result: Option<PathBuf>,
    store: Option<PathBuf>,
    report: Option<PathBuf>,
}

fn parse_arguments() -> Result<Option<Arguments>, String> {
    let mut parsed = Arguments::default();
    let mut arguments = env::args_os();
    let _program = arguments.next();
    while let Some(flag) = arguments.next() {
        if flag == "--help" || flag == "-h" {
            return Ok(None);
        }
        let value = arguments
            .next()
            .ok_or_else(|| format!("missing value for {}", flag.to_string_lossy()))?;
        match flag.to_str() {
            Some("--image") if parsed.image.is_none() => parsed.image = Some(value.into()),
            Some("--result") if parsed.result.is_none() => parsed.result = Some(value.into()),
            Some("--store") if parsed.store.is_none() => parsed.store = Some(value.into()),
            Some("--report") if parsed.report.is_none() => parsed.report = Some(value.into()),
            _ => {
                return Err(format!(
                    "unknown or repeated argument {}",
                    flag.to_string_lossy()
                ));
            }
        }
    }
    if parsed.image.is_none() || parsed.result.is_none() || parsed.store.is_none() {
        return Err("--image, --result, and --store are required".into());
    }
    Ok(Some(parsed))
}

fn run() -> Result<(), String> {
    let Some(arguments) = parse_arguments()? else {
        println!("{USAGE}");
        return Ok(());
    };
    let report = verify(
        arguments.image.as_deref().expect("required image"),
        arguments.result.as_deref().expect("required result"),
        arguments.store.as_deref().expect("required store"),
    )
    .map_err(|error| error.to_string())?;
    let mut encoded = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("could not encode verification report: {error}"))?;
    encoded.push(b'\n');
    if let Some(path) = arguments.report {
        fs::write(&path, encoded)
            .map_err(|error| format!("could not write report {}: {error}", path.display()))?;
    } else {
        print!(
            "{}",
            String::from_utf8(encoded).expect("JSON report is UTF-8")
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("MFENX contract-v1 verification failed: {error}");
            eprintln!("{USAGE}");
            ExitCode::FAILURE
        }
    }
}
