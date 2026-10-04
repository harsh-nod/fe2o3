#[allow(dead_code)]
mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};

use common::{kernel_with_object_digest, object_identity, target, text};
use fe2o3_artifacts::{
    ArtifactContainerV1, CodeObjectPayload, CompilerIdentity, DigestAlgorithm, ManifestV1,
    PointerWidth, ToolIdentity,
};

fn container_bytes() -> Vec<u8> {
    let values = [
        b"first code object".as_slice(),
        b"second code object".as_slice(),
    ];
    let identities = values
        .iter()
        .map(|bytes| {
            let digest = DigestAlgorithm::Sha256.calculate(bytes).bytes();
            object_identity(digest, bytes.len() as u64)
        })
        .collect();
    let manifest = ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target(PointerWidth::Bits64, vec![]),
        identities,
        vec![kernel_with_object_digest(
            0x11,
            "kernel",
            "kernel.kd",
            DigestAlgorithm::Sha256.calculate(values[0]).bytes(),
            vec![],
        )],
    )
    .unwrap();
    let payloads = values
        .iter()
        .map(|bytes| {
            CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, bytes.to_vec()).unwrap()
        })
        .collect();
    ArtifactContainerV1::new(manifest, DigestAlgorithm::Sha256, payloads)
        .unwrap()
        .to_bytes()
}

#[test]
fn every_container_truncation_is_rejected_without_panicking() {
    let valid = container_bytes();
    assert!(ArtifactContainerV1::from_bytes(&valid).is_ok());

    for length in 0..valid.len() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            ArtifactContainerV1::from_bytes(&valid[..length])
        }));
        assert!(result.is_ok(), "decoder panicked at prefix length {length}");
        assert!(
            result.unwrap().is_err(),
            "decoder accepted prefix length {length}"
        );
    }
}

#[test]
fn deterministic_mutation_corpus_is_panic_free_and_canonical_when_accepted() {
    let valid = container_bytes();
    let mut state = 0x6a09_e667_f3bc_c909_u64;

    for case in 0..4096 {
        let mut input = valid.clone();
        state = next_state(state);
        match case % 4 {
            0 => {
                let index = state as usize % input.len();
                input[index] ^= ((state >> 17) as u8) | 1;
            }
            1 => {
                input.truncate(state as usize % input.len());
            }
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

        let result = catch_unwind(AssertUnwindSafe(|| ArtifactContainerV1::from_bytes(&input)));
        assert!(result.is_ok(), "decoder panicked for mutation case {case}");
        if let Ok(container) = result.unwrap() {
            assert_eq!(
                container.to_bytes(),
                input,
                "accepted mutation case {case} was not canonical"
            );
            for payload in container.payloads() {
                assert_eq!(payload.digest().verify(payload.bytes()), Ok(()));
            }
        }
    }
}

fn next_state(mut state: u64) -> u64 {
    state ^= state << 13;
    state ^= state >> 7;
    state ^ (state << 17)
}
