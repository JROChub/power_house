#![no_main]

use std::fs;

use libfuzzer_sys::fuzz_target;
use rarecomp_mfenx_ir::{ElementType, TensorType};
use rarecomp_mfenx_tensor_store::{LocalChunkStore, StoreLimits, StoredTensor};

const DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fuzz_target!(|data: &[u8]| {
    let Ok(directory) = tempfile::Builder::new().prefix("mfenx-store-fuzz-").tempdir() else {
        return;
    };
    let Ok(store) = LocalChunkStore::create(directory.path().join("store")) else {
        return;
    };
    let manifest_directory = store.root().join("manifests/00");
    if fs::create_dir_all(&manifest_directory).is_err()
        || fs::write(manifest_directory.join(format!("{DIGEST}.json")), data).is_err()
    {
        return;
    }
    let handle = StoredTensor {
        manifest_digest: DIGEST.to_owned(),
        ty: TensorType {
            element: ElementType::I32,
            shape: vec![1, 1],
        },
        byte_length: 4,
    };
    let _ = store.open(&handle, StoreLimits::default());
});
