#![cfg(feature = "sfcs-risc0")]

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use power_house::{
    memory::semantic_packet_digest, provenance::Rootprint, verify_sfcs_risc0_private_vm_capsule,
    verify_sfcs_risc0_private_vm_embedding, MemoryCapsuleBuilder, MemoryVerificationPolicy,
    ObservatorySidecar, SfcsRisc0PrivateVmProof,
};
use serde_json::json;
use std::{collections::BTreeMap, sync::OnceLock};

const GENERAL_PROGRAM: &[u8] = include_bytes!("../conformance/sfcs-risc0/private-general-v1.bin");
const LANES: usize = 32;
const BYTES: usize = LANES * 4;
const OUTPUT_WORDS: usize = 8;
const BYTES_PER_WORD: usize = 4;

fn general_input_words() -> [u32; LANES + 2] {
    let mut words = [0_u32; LANES + 2];
    words[0] = 63;
    words[1] = 31;
    let mut state = 0x243f_6a88_u32;
    for word in &mut words[2..] {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        *word = state ^ state.rotate_left(13);
    }
    words
}

fn general_private_input() -> Vec<u8> {
    general_input_words()
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect()
}

#[inline(never)]
fn reference_mix_lane(left: u32, right: u32, round: u32) -> u32 {
    let rotation = (round ^ right) & 31;
    let combined = left.rotate_left(rotation)
        ^ right.rotate_right((left ^ round) & 31)
        ^ round.wrapping_mul(0x9e37_79b9);
    combined.wrapping_mul(0x85eb_ca6b).wrapping_add(0xc2b2_ae35)
}

fn reference_output() -> [u32; 8] {
    let private_words = general_input_words();
    let rounds = (private_words[0] & 63) + 1;
    let active = ((private_words[1] & 31) + 1) as usize;
    let mut lanes = [0_u32; LANES];
    lanes.copy_from_slice(&private_words[2..]);

    let mut branch_counts = [0_u32; 4];
    for round in 0..rounds {
        let index = (round as usize * 7 + (lanes[round as usize % active] as usize)) % active;
        let peer = (index + 1 + ((lanes[index] >> 27) as usize)) % active;
        let left = lanes[index];
        let right = lanes[peer];

        let mixed = if (left & 1) == 0 {
            branch_counts[0] += 1;
            reference_mix_lane(left ^ right, left | right, round)
        } else if (left as i32) < (right as i32) {
            branch_counts[1] += 1;
            reference_mix_lane(left & right, left.wrapping_add(right), round)
        } else if left != right && left < right {
            branch_counts[2] += 1;
            reference_mix_lane(!left, right ^ round, round)
        } else {
            branch_counts[3] += 1;
            reference_mix_lane(left.wrapping_sub(right), left ^ right, round)
        };

        lanes[index] = mixed;
        lanes[peer] = lanes[peer].wrapping_add(mixed.rotate_right((index as u32) & 31)) ^ round;
    }

    let mut bytes = [0_u8; BYTES];
    for (index, lane) in lanes.iter().enumerate() {
        let offset = index * 4;
        bytes[offset..offset + 4].copy_from_slice(&lane.to_le_bytes());
    }

    let byte_index = (lanes[0] as usize) % BYTES;
    bytes[byte_index] ^= (rounds as u8).wrapping_mul(17);
    let halfword_index = ((lanes[1] as usize) % (BYTES - 1)) & !1;
    let halfword = u16::from_le_bytes([bytes[halfword_index], bytes[halfword_index + 1]])
        .rotate_left(rounds & 15)
        ^ 0xa55a;
    bytes[halfword_index..halfword_index + 2].copy_from_slice(&halfword.to_le_bytes());

    let mut digest = 0x6a09_e667_u32;
    let mut minimum = u32::MAX;
    let mut signed_negative = 0_u32;
    for chunk in bytes.chunks_exact(4) {
        let word = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        minimum = minimum.min(word);
        signed_negative += u32::from((word as i32) < 0);
        digest = digest.rotate_left(5) ^ word.wrapping_mul(0x27d4_eb2d);
    }

    let mut byte_checksum = 0_u32;
    for (index, byte) in bytes.iter().enumerate() {
        byte_checksum = byte_checksum.wrapping_add((*byte as u32) * (index as u32 + 1));
    }

    [
        digest,
        minimum,
        signed_negative,
        branch_counts[0],
        branch_counts[1],
        branch_counts[2],
        branch_counts[3],
        byte_checksum,
    ]
}

fn real_general_proof() -> SfcsRisc0PrivateVmProof {
    static PROOF: OnceLock<SfcsRisc0PrivateVmProof> = OnceLock::new();
    PROOF
        .get_or_init(|| {
            SfcsRisc0PrivateVmProof::prove(GENERAL_PROGRAM, &general_private_input())
                .expect("the general conformance guest must produce a real receipt")
        })
        .clone()
}

fn decode_journal(proof: &SfcsRisc0PrivateVmProof) -> [u32; 8] {
    let bytes = BASE64
        .decode(&proof.statement.journal_base64)
        .expect("journal must be canonical base64");
    assert_eq!(bytes.len(), OUTPUT_WORDS * BYTES_PER_WORD);
    std::array::from_fn(|index| {
        let start = index * 4;
        u32::from_le_bytes(bytes[start..start + 4].try_into().unwrap())
    })
}

#[test]
fn arbitrary_private_program_receipt_matches_independent_execution() {
    let proof = real_general_proof();
    proof.verify(GENERAL_PROGRAM).unwrap();

    let expected = reference_output();
    let actual = decode_journal(&proof);
    assert_eq!(actual, expected);
    assert!(
        actual[3..7].iter().all(|count| *count > 0),
        "the vector must exercise every control-flow class: {actual:?}"
    );

    let private_input = general_private_input();
    let receipt = BASE64.decode(&proof.receipt_base64).unwrap();
    assert!(
        !receipt
            .windows(private_input.len())
            .any(|window| window == private_input),
        "the receipt transport disclosed the exact private witness"
    );
}

#[test]
fn arbitrary_private_program_preserves_identity_capsule_and_slbit_boundaries() {
    let proof = real_general_proof();
    let artifact = proof
        .to_pha_artifact("general-private-program", GENERAL_PROGRAM)
        .unwrap();
    verify_sfcs_risc0_private_vm_embedding(&artifact).unwrap();
    let core_fingerprint = artifact.phx_fingerprint.clone();

    let rootprint = Rootprint::new("general-private-program", artifact.clone()).unwrap();
    let replay = rootprint.replay().unwrap();
    let branch = rootprint.root_branch.clone();
    let mut packet = json!({
        "schema": "slbit/viz-packet/v3",
        "packet_id": "slp_sfcs_risc0_general",
        "packet_digest": "",
        "claim": {
            "claim_id": "claim_sfcs_risc0_general",
            "label": "general private control-flow and memory execution",
            "domain": "sfcs-risc0-private-vm",
            "status": "verified",
            "bound_core": {
                "capsule_id": "phm_general-private-program",
                "branch_id": branch,
                "replay_fingerprint": replay.state_fingerprint,
                "profile": power_house::SFCS_RISC0_PRIVATE_VM_PROTOCOL_V1
            },
            "public_statement": proof.statement
        },
        "transcript": {"rounds": []},
        "semantic_dag": {"nodes": [], "edges": []},
        "views": {"timeline": [], "claim_cards": [], "graphs": [], "diffs": []},
        "explanation_constraints": {
            "allowed_sources": ["public_statement"],
            "forbid_unbound_claims": true,
            "mark_generated_text_non_authoritative": true
        }
    });
    packet["packet_digest"] = json!(semantic_packet_digest(&packet).unwrap());
    let sidecar = ObservatorySidecar::new(
        &rootprint,
        BTreeMap::from([(branch.clone(), packet.clone())]),
    )
    .unwrap();

    let capsule = MemoryCapsuleBuilder::new("general-private-program")
        .with_pha(artifact)
        .with_rootprint(rootprint.clone())
        .with_replay_required()
        .with_semantic_packet(
            "slbit/viz-packet/v3",
            "slp_sfcs_risc0_general",
            branch,
            replay.state_fingerprint,
            "verified_public_statement",
            packet,
        )
        .unwrap()
        .with_sidecar(sidecar)
        .build()
        .unwrap();

    let verified =
        verify_sfcs_risc0_private_vm_capsule(&capsule, MemoryVerificationPolicy::strict()).unwrap();
    assert_eq!(verified.statement, proof.statement);
    assert_eq!(capsule.core.pha.phx_fingerprint, core_fingerprint);

    let mut semantic_mutation = capsule;
    semantic_mutation.semantics.as_mut().unwrap().packets[0]
        .packet
        .as_mut()
        .unwrap()["claim"]["label"] = json!("unbound semantic mutation");
    semantic_mutation.header.capsule_digest =
        Some(semantic_mutation.calculate_capsule_digest().unwrap());
    assert_eq!(
        semantic_mutation.core.pha.phx_fingerprint, core_fingerprint,
        "SLBIT meaning must remain outside .pha identity"
    );
    assert!(
        verify_sfcs_risc0_private_vm_capsule(
            &semantic_mutation,
            MemoryVerificationPolicy::strict()
        )
        .is_err(),
        "semantic integrity mutation must be rejected"
    );
}
