#[allow(dead_code)]
mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};

use common::{digest, name, target};
use fe2o3_artifacts::{
    BundleIndexV1, BundleKernelIndexEntryV1, BundlePayloadReferenceV1, BundleTargetAssociationV1,
    CodeObjectFormat, PointerWidth,
};

fn index_bytes() -> Vec<u8> {
    BundleIndexV1::new(
        vec![BundleTargetAssociationV1::new(
            digest(0x10),
            target(PointerWidth::Bits64, vec![]),
        )],
        vec![
            BundlePayloadReferenceV1::new(digest(0x30), CodeObjectFormat::NativeExecutable, 64)
                .unwrap(),
            BundlePayloadReferenceV1::new(digest(0x40), CodeObjectFormat::LlvmBitcode, 128)
                .unwrap(),
        ],
        vec![
            BundleKernelIndexEntryV1::new(
                digest(0x50),
                name("kernel.kd"),
                digest(0x10),
                vec![digest(0x40), digest(0x30)],
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .to_bytes()
}

#[test]
fn every_bundle_index_truncation_is_rejected_without_panicking() {
    let valid = index_bytes();
    assert!(BundleIndexV1::from_bytes(&valid).is_ok());

    for length in 0..valid.len() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            BundleIndexV1::from_bytes(&valid[..length])
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
    let valid = index_bytes();
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

        let result = catch_unwind(AssertUnwindSafe(|| BundleIndexV1::from_bytes(&input)));
        assert!(result.is_ok(), "decoder panicked for mutation case {case}");
        if let Ok(index) = result.unwrap() {
            assert_eq!(
                index.to_bytes(),
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
