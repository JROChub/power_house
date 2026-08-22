#![no_main]

use std::fs;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(directory) = tempfile::Builder::new().prefix("mfenx-contract-fuzz-").tempdir() else {
        return;
    };
    let image = directory.path().join("image.json");
    let result = directory.path().join("result.json");
    let store = directory.path().join("store");
    if fs::create_dir(&store).is_err() || fs::write(&image, data).is_err() {
        return;
    }
    let midpoint = data.len() / 2;
    if fs::write(&result, &data[midpoint..]).is_err() {
        return;
    }
    let _ = mfenx_contract_v1_verifier::verify(&image, &result, &store);
});
