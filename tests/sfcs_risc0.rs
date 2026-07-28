#![cfg(feature = "sfcs-risc0")]

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use power_house::{
    provenance::Rootprint, verify_sfcs_risc0_private_vm_capsule,
    verify_sfcs_risc0_private_vm_embedding, MemoryCapsuleBuilder, MemoryVerificationPolicy,
    SfcsRisc0Error, SfcsRisc0PrivateVmProof,
};
use risc0_zkvm::{compute_image_id, FakeReceipt, InnerReceipt, Receipt, ReceiptClaim};
use std::sync::OnceLock;

const GUEST_PROGRAM: &[u8] = include_bytes!("../conformance/sfcs-risc0/private-sum-v1.bin");
const LEFT: u32 = 0x1357_9bdf;
const RIGHT: u32 = 0x2468_ace0;

fn private_input() -> Vec<u8> {
    [LEFT.to_le_bytes(), RIGHT.to_le_bytes()].concat()
}

fn real_proof() -> SfcsRisc0PrivateVmProof {
    static PROOF: OnceLock<SfcsRisc0PrivateVmProof> = OnceLock::new();
    PROOF
        .get_or_init(|| {
            SfcsRisc0PrivateVmProof::prove(GUEST_PROGRAM, &private_input())
                .expect("the conformance guest must produce a real receipt")
        })
        .clone()
}

#[test]
fn real_whole_program_receipt_verifies_and_keeps_private_input_out_of_transport() {
    let proof = real_proof();
    proof.verify(GUEST_PROGRAM).unwrap();

    let receipt = BASE64.decode(&proof.receipt_base64).unwrap();
    assert!(
        !receipt
            .windows(private_input().len())
            .any(|window| window == private_input()),
        "the serialized receipt disclosed the exact private input"
    );
    let public_journal = BASE64.decode(&proof.statement.journal_base64).unwrap();
    assert!(!public_journal.is_empty());
    assert_ne!(public_journal, private_input());
    assert!(proof.statement.receipt_claim_digest.starts_with("risc0:"));
}

#[test]
fn deterministic_core_identity_excludes_receipt_transport() {
    let proof = real_proof();
    let artifact = proof.to_pha_artifact("private-sum", GUEST_PROGRAM).unwrap();
    verify_sfcs_risc0_private_vm_embedding(&artifact).unwrap();

    let mut detached = artifact.clone();
    detached.embedded_proof.external_proof_attachments = None;
    assert_eq!(
        artifact.phx_fingerprint, detached.phx_fingerprint,
        "receipt transport must never alter the Power House v1 fingerprint"
    );
    detached.verify().unwrap();
    assert!(matches!(
        verify_sfcs_risc0_private_vm_embedding(&detached),
        Err(SfcsRisc0Error::InvalidEmbedding(_))
    ));

    let attached_rootprint = Rootprint::new("private-sum", artifact).unwrap();
    let detached_rootprint = Rootprint::new("private-sum", detached).unwrap();
    assert_eq!(
        attached_rootprint.root_branch, detached_rootprint.root_branch,
        "receipt transport must never alter Rootprint identity"
    );
}

#[test]
fn receipt_attachment_and_core_statement_mutations_are_rejected() {
    let proof = real_proof();
    let artifact = proof.to_pha_artifact("private-sum", GUEST_PROGRAM).unwrap();

    let mut attachment_mutation = artifact.clone();
    attachment_mutation
        .embedded_proof
        .external_proof_attachments
        .as_mut()
        .unwrap()[0]
        .payload["proof"]["receipt_base64"] = serde_json::json!("AA==");
    assert!(verify_sfcs_risc0_private_vm_embedding(&attachment_mutation).is_err());

    let mut statement_mutation = artifact;
    statement_mutation.embedded_proof.public_inputs["journal_digest"] = serde_json::json!(
        "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
    );
    statement_mutation.refresh_phx_fingerprint().unwrap();
    statement_mutation.verify().unwrap();
    assert!(matches!(
        verify_sfcs_risc0_private_vm_embedding(&statement_mutation),
        Err(SfcsRisc0Error::InvalidEmbedding(_))
    ));
}

#[test]
fn fake_receipts_and_wrong_guest_images_are_rejected() {
    let image_id = compute_image_id(GUEST_PROGRAM).unwrap();
    let claim = ReceiptClaim::ok(image_id, Vec::<u8>::new());
    let fake = Receipt::new(InnerReceipt::Fake(FakeReceipt::new(claim)), Vec::new());
    assert!(matches!(
        SfcsRisc0PrivateVmProof::from_receipt(GUEST_PROGRAM, fake),
        Err(SfcsRisc0Error::FakeReceipt)
    ));

    let proof = real_proof();
    let mut wrong_elf = GUEST_PROGRAM.to_vec();
    let last = wrong_elf.len() - 1;
    wrong_elf[last] ^= 1;
    assert!(proof.verify(&wrong_elf).is_err());
}

#[test]
fn program_projection_is_deterministic_and_content_bound() {
    let first = SfcsRisc0PrivateVmProof::program_graph(GUEST_PROGRAM).unwrap();
    let first_digest = first.fractal_digest().unwrap();
    for _ in 0..32 {
        let next = SfcsRisc0PrivateVmProof::program_graph(GUEST_PROGRAM).unwrap();
        assert_eq!(next.fractal_digest().unwrap(), first_digest);
    }

    let mut changed = GUEST_PROGRAM.to_vec();
    let last = changed.len() - 1;
    changed[last] ^= 1;
    assert!(SfcsRisc0PrivateVmProof::program_graph(&changed)
        .map(|graph| graph.fractal_digest().unwrap())
        .map_or(true, |digest| digest != first_digest));
}

#[test]
fn receipt_verifies_offline_inside_rootprint_memory_capsule() {
    let proof = real_proof();
    let artifact = proof.to_pha_artifact("private-sum", GUEST_PROGRAM).unwrap();
    let rootprint = Rootprint::new("private-sum", artifact.clone()).unwrap();
    let capsule = MemoryCapsuleBuilder::new("sfcs-risc0-private-sum")
        .with_pha(artifact)
        .with_rootprint(rootprint)
        .with_replay_required()
        .build()
        .unwrap();

    let verified =
        verify_sfcs_risc0_private_vm_capsule(&capsule, MemoryVerificationPolicy::strict()).unwrap();
    assert_eq!(verified.statement, proof.statement);
}
