use super::*;

pub(crate) fn observe_managed_build_value_v1(tcx: TyCtxt<'_>) -> Result<[u8; 32], String> {
    decode_sha256_environment(tcx, CARGO_METADATA_BUILD_OBSERVATION_ENV_V2)
}

#[test]
fn managed_session_hex_decode_preserves_exact_bytes_and_case_acceptance() {
    assert_eq!(
        decode_sha256_environment_value("binding", &"aB".repeat(32)).unwrap(),
        [0xab; 32]
    );
    assert_eq!(
        decode_sha256_environment_value("binding", &"00".repeat(32)).unwrap(),
        [0; 32],
        "zero refusal remains in the existing provider definition validation phase"
    );
}

#[test]
fn managed_session_hex_decode_preserves_malformed_error_and_never_trims() {
    for value in [
        String::new(),
        "a".repeat(63),
        "a".repeat(65),
        "g".repeat(64),
        format!(" {}", "a".repeat(64)),
        format!("{}\n", "a".repeat(64)),
    ] {
        assert_eq!(
            decode_sha256_environment_value("binding", &value),
            Err("managed build supplied malformed binding".to_owned())
        );
    }
}
