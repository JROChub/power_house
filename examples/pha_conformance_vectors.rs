use power_house::provenance::{ExternalProofAttachment, PhaArtifact, Rootprint};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn write_json(path: &Path, value: &impl Serialize) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("serialize vector");
    bytes.push(b'\n');
    fs::write(path, &bytes).expect("write vector");
    bytes
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn artifact(stage: &str) -> PhaArtifact {
    PhaArtifact::new(
        json!({
            "producer": "power-house-conformance",
            "stage": stage,
        }),
        "power-house/rootprint-example/v1",
        json!({"claim": 70, "field": 1_000_000_007_u64}),
        json!({"accepted": true, "rounds": 70}),
    )
    .expect("valid artifact")
}

fn main() {
    let directory = Path::new("conformance/pha-v1");
    fs::create_dir_all(directory).expect("create conformance directory");

    let core = artifact("baseline");
    let mut attached = core.clone();
    attached.embedded_proof.external_proof_attachments = Some(vec![ExternalProofAttachment::new(
        "external-example-1",
        "example/external-proof/v1",
        json!({"proof": "opaque-example", "public_inputs": [1, 2, 3]}),
    )
    .expect("valid attachment")]);

    let mut rootprint = Rootprint::new("main", core.clone()).expect("valid root");
    let left = rootprint
        .fork("main", "candidate", attached.clone())
        .expect("valid fork");
    let right = rootprint
        .fork("main", "audit", artifact("audit"))
        .expect("valid fork");
    rootprint
        .merge(&left, &right, "accepted", artifact("accepted"))
        .expect("valid merge");
    rootprint.verify().expect("valid graph");

    let core_bytes = write_json(&directory.join("core-valid.pha"), &core);
    let attached_bytes = write_json(&directory.join("core-with-epa.pha"), &attached);
    let rootprint_bytes = write_json(&directory.join("rootprint-valid.json"), &rootprint);
    write_json(
        Path::new("publicpower/artifacts/rootprint-valid.json"),
        &rootprint,
    );

    let mut files = BTreeMap::new();
    files.insert("core-valid.pha", sha256(&core_bytes));
    files.insert("core-with-epa.pha", sha256(&attached_bytes));
    files.insert("rootprint-valid.json", sha256(&rootprint_bytes));
    let manifest: Value = json!({
        "schema": "power-house-pha-conformance-v1",
        "core_fingerprint": core.phx_fingerprint,
        "external_attachments_affect_core_identity": false,
        "files": files,
        "mutation_rules": {
            "core_field_mutation": "must fail core verification",
            "epa_payload_mutation": "must preserve core verification and fail explicit EPA integrity verification"
        }
    });
    write_json(&directory.join("manifest.json"), &manifest);
}
