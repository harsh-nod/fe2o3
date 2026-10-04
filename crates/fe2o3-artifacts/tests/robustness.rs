mod common;

use std::panic::{AssertUnwindSafe, catch_unwind};

use common::manifest;
use fe2o3_artifacts::ManifestV1;

#[test]
fn every_truncation_and_deterministic_malformed_input_is_panic_free() {
    let valid = manifest().to_bytes();

    for length in 0..valid.len() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            ManifestV1::from_bytes(&valid[..length])
        }));
        assert!(
            result.is_ok(),
            "decoder panicked for prefix length {length}"
        );
        assert!(result.unwrap().is_err());
    }

    let mut state = 0x6a09_e667_f3bc_c909_u64;
    for case in 0..1024 {
        let mut input = valid.clone();
        let mutation_count = 1 + case % 4;
        for _ in 0..mutation_count {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let position = (state as usize) % input.len();
            input[position] ^= (state >> 32) as u8 | 1;
        }
        if case % 17 == 0 {
            input.extend_from_slice(&state.to_le_bytes());
        }
        let result = catch_unwind(AssertUnwindSafe(|| ManifestV1::from_bytes(&input)));
        assert!(result.is_ok(), "decoder panicked for corpus case {case}");
        if let Ok(decoded) = result.unwrap() {
            assert_eq!(
                decoded.to_bytes(),
                input,
                "successful decode was not canonical for corpus case {case}"
            );
        }
    }
}
