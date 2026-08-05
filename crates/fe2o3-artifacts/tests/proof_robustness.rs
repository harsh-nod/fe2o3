#[allow(dead_code)]
mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};

use common::{digest, name, text};
use fe2o3_artifacts::{
    ConfigurationEntry, DigestAlgorithm, MeasuredToolIdentity, PayloadDigest,
    ProofArtifactIdentity, ProofExecutionIdentity, ProofOutcome, ProofProperty, ProofRecordV1,
    ProofTargetIdentity, SourceContractIdentity, TrustedItem, VerificationModelIdentity,
};

fn sha(byte: u8) -> PayloadDigest {
    PayloadDigest::new(DigestAlgorithm::Sha256, digest(byte))
}

fn measured_tool(name: &str, version: &str, byte: u8) -> MeasuredToolIdentity {
    MeasuredToolIdentity::new(text(name), text(version), sha(byte), sha(byte + 1))
}

fn proof_bytes() -> Vec<u8> {
    ProofRecordV1::new(
        ProofTargetIdentity::new(
            ProofArtifactIdentity::new(
                sha(1),
                sha(2),
                sha(3),
                sha(4),
                sha(5),
                sha(6),
                sha(7),
                sha(8),
            ),
            SourceContractIdentity::new(sha(9), sha(10), sha(11), sha(12), sha(13)),
        ),
        vec![
            ConfigurationEntry::new(name("cfg_target"), text("amdgpu")),
            ConfigurationEntry::new(name("feature_bounds"), text("enabled")),
        ],
        ProofExecutionIdentity::new(
            VerificationModelIdentity::new(text("fe2o3-gpu-v1"), sha(14)),
            measured_tool("verus", "0.2026.08.04", 15),
            measured_tool("z3", "4.15.2", 17),
            measured_tool("fe2o3-proof-driver", "0.1.0", 19),
            sha(21),
        ),
        ProofOutcome::Proved,
        vec![
            ProofProperty::Bounds,
            ProofProperty::MemorySafety,
            ProofProperty::RaceFreedom,
        ],
        vec![TrustedItem::new(name("model_axiom"), sha(22))],
    )
    .unwrap()
    .to_bytes()
}

#[test]
fn deterministic_proof_mutation_corpus_is_panic_free_and_canonical_when_accepted() {
    let valid = proof_bytes();
    let mut state = 0xbb67_ae85_84ca_a73b_u64;

    for case in 0..4096 {
        let mut input = valid.clone();
        state = next_state(state);
        match case % 4 {
            0 => {
                let index = state as usize % input.len();
                input[index] ^= ((state >> 17) as u8) | 1;
            }
            1 => input.truncate(state as usize % input.len()),
            2 => {
                let count = (state as usize % 8) + 1;
                for _ in 0..count {
                    state = next_state(state);
                    input.push(state as u8);
                }
            }
            3 => {
                let count = (state as usize % 8) + 1;
                for _ in 0..count {
                    state = next_state(state);
                    let index = state as usize % input.len();
                    input[index] = (state >> 29) as u8;
                }
            }
            _ => unreachable!(),
        }

        let result = catch_unwind(AssertUnwindSafe(|| ProofRecordV1::from_bytes(&input)));
        assert!(result.is_ok(), "decoder panicked for mutation case {case}");
        if let Ok(record) = result.unwrap() {
            assert_eq!(
                record.to_bytes(),
                input,
                "accepted mutation case {case} was not canonical"
            );
        }
    }
}

fn next_state(mut state: u64) -> u64 {
    state ^= state << 13;
    state ^= state >> 7;
    state ^ (state << 17)
}
