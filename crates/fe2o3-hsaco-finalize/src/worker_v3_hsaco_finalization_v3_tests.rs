use std::time::Duration;

use super::*;

const OUTER_BYTES: &[u8] = b"strict-v3-outer";
const OTHER_OUTER_BYTES: &[u8] = b"strict-v3-outer-mutated";
const PLAN_BYTES: &[u8] = b"canonical-link-plan";
const OTHER_PLAN_BYTES: &[u8] = b"canonical-link-plan-mutated";
const RAW_BYTES: &[u8] = b"raw-hsaco";
const OTHER_RAW_BYTES: &[u8] = b"raw-hsaco-mutated";
const FINAL_BYTES: &[u8] = b"finalized-hsaco";
const OTHER_FINAL_BYTES: &[u8] = b"finalized-hsaco-mutated";
const DESCRIPTOR_BYTES: &[u8] = b"canonical-descriptor-table";
const OTHER_DESCRIPTOR_BYTES: &[u8] = b"canonical-descriptor-table-mutated";

#[test]
fn native_v3_finalization_identity_is_deterministic_and_nonzero() {
    let preimage = fixture_preimage();
    let first = calculate_protected_finalized_identity_v3(&preimage);
    let second = calculate_protected_finalized_identity_v3(&preimage);
    assert_eq!(first, second);
    // Independently computed from the pre-extraction 94a6909 implementation.
    assert_eq!(
        first,
        [
            0x9b, 0x88, 0x88, 0x0e, 0xa3, 0x10, 0x00, 0xa2, 0xdf, 0x7a, 0x6e, 0x3a, 0xf9, 0x5b,
            0x3b, 0x68, 0x1d, 0xdd, 0x59, 0x09, 0x2e, 0x07, 0x8d, 0x17, 0xb2, 0xe2, 0xc0, 0xa3,
            0x3c, 0x09, 0x3c, 0xfe,
        ]
    );
    assert_ne!(
        first,
        calculate_finalized_identity_in_domain(&preimage, NOMINAL_FINALIZED_IDENTITY_DOMAIN_V3)
    );
    let v4 =
        calculate_finalized_identity_in_domain(&preimage, NOMINAL_FINALIZED_IDENTITY_DOMAIN_V4);
    assert_ne!(first, v4);
    assert_ne!(
        v4,
        calculate_finalized_identity_in_domain(&preimage, NOMINAL_FINALIZED_IDENTITY_DOMAIN_V3)
    );
    assert_eq!(
        NOMINAL_FINALIZED_IDENTITY_DOMAIN_V5,
        b"FE2O3/STRICT-V3-PROTECTED-WORKER-NOMINAL-DESCRIPTOR-FINALIZATION/V5\0"
    );
    let v5 =
        calculate_finalized_identity_in_domain(&preimage, NOMINAL_FINALIZED_IDENTITY_DOMAIN_V5);
    assert_eq!(
        v5,
        calculate_finalized_identity_in_domain(&preimage, NOMINAL_FINALIZED_IDENTITY_DOMAIN_V5)
    );
    assert_ne!(v5, [0; 32]);
    assert_ne!(v5, first);
    assert_ne!(v5, v4);
    assert_ne!(
        v5,
        calculate_finalized_identity_in_domain(&preimage, NOMINAL_FINALIZED_IDENTITY_DOMAIN_V3)
    );
}

#[test]
fn native_v3_finalization_identity_binds_every_lineage_axis() {
    let base = fixture_preimage();
    let expected = calculate_protected_finalized_identity_v3(&base);
    let nominal =
        calculate_finalized_identity_in_domain(&base, NOMINAL_FINALIZED_IDENTITY_DOMAIN_V3);
    let conditional =
        calculate_finalized_identity_in_domain(&base, NOMINAL_FINALIZED_IDENTITY_DOMAIN_V4);
    let conditional_v5 =
        calculate_finalized_identity_in_domain(&base, NOMINAL_FINALIZED_IDENTITY_DOMAIN_V5);
    macro_rules! assert_axis {
        ($field:ident, $value:expr) => {{
            let mut changed = base.clone();
            changed.$field = $value;
            assert_ne!(
                calculate_protected_finalized_identity_v3(&changed),
                expected,
                "V3 finalization identity omitted {}",
                stringify!($field)
            );
            assert_ne!(
                calculate_finalized_identity_in_domain(
                    &changed,
                    NOMINAL_FINALIZED_IDENTITY_DOMAIN_V3
                ),
                nominal,
                "nominal finalization identity omitted {}",
                stringify!($field)
            );
            assert_ne!(
                calculate_finalized_identity_in_domain(
                    &changed,
                    NOMINAL_FINALIZED_IDENTITY_DOMAIN_V4
                ),
                conditional,
                "conditional finalization identity omitted {}",
                stringify!($field)
            );
            assert_ne!(
                calculate_finalized_identity_in_domain(
                    &changed,
                    NOMINAL_FINALIZED_IDENTITY_DOMAIN_V5
                ),
                conditional_v5,
                "V5 conditional finalization identity omitted {}",
                stringify!($field)
            );
        }};
    }

    assert_axis!(raw_inspection_identity, digest(2));
    assert_axis!(source_evidence_identity, digest(3));
    assert_axis!(binding_identity, digest(4));
    assert_axis!(attempt, attempt(2));
    assert_axis!(transaction_identity, digest(5));
    assert_axis!(receipt_byte_len, 101);
    assert_axis!(outer_handoff_sha256, digest(6));
    assert_axis!(outer_handoff_byte_len, 102);
    assert_axis!(exact_outer_handoff_bytes, OTHER_OUTER_BYTES);
    assert_axis!(capsule_sha256, digest(7));
    assert_axis!(capsule_byte_len, 103);
    assert_axis!(invocation_digest, digest(8));
    assert_axis!(pair_binding_sha256, digest(9));
    assert_axis!(pair_binding_byte_len, 104);
    assert_axis!(nested_handoff_sha256, digest(10));
    assert_axis!(nested_handoff_byte_len, 105);
    assert_axis!(final_commitment_receipt_sha256, digest(11));
    assert_axis!(final_commitment_receipt_byte_len, 106);
    assert_axis!(final_commitment_sha256, digest(12));
    assert_axis!(final_commitment_byte_len, 107);
    assert_axis!(compiler_closure, compiler_closure(0x30));
    assert_axis!(
        worker_executable,
        ContentIdentityV1::calculate(b"other-worker")
    );
    assert_axis!(worker_build_identity, "worker-build-mutated");
    assert_axis!(llvm_build_identity, "upstream-llvm-mutated");
    assert_axis!(
        worker_limits,
        WorkerExecutionLimitsV1::new(Duration::from_secs(4), 4096, 512).unwrap()
    );
    assert_axis!(link_plan_identity, digest(13));
    assert_axis!(exact_link_plan_bytes, OTHER_PLAN_BYTES);
    assert_axis!(response_identity, digest(14));
    assert_axis!(raw_output, ContentIdentityV1::calculate(OTHER_RAW_BYTES));
    assert_axis!(exact_raw_bytes, OTHER_RAW_BYTES);
    assert_axis!(policy_identity, digest(15));
    assert_axis!(descriptor_observation_identity, digest(16));
    assert_axis!(abi_observation_identity, digest(17));
    assert_axis!(resource_observation_identity, digest(18));
    assert_axis!(target, "gfx942:xnack+");
    assert_axis!(code_object_version, CodeObjectVersion::V5);
    assert_axis!(required_workgroup_size, [128, 1, 1]);
    assert_axis!(max_flat_workgroup_size, 128);
    assert_axis!(wavefront_size, 32);
    assert_axis!(observed_kernel_symbols_identity, digest(19));
    assert_axis!(
        finalized_output,
        ContentIdentityV1::calculate(OTHER_FINAL_BYTES)
    );
    assert_axis!(exact_finalized_bytes, OTHER_FINAL_BYTES);
    assert_axis!(canonical_digest, digest(20));
    assert_axis!(
        canonical_descriptor_evidence,
        ContentIdentityV1::calculate(OTHER_DESCRIPTOR_BYTES)
    );
    assert_axis!(exact_canonical_descriptor_bytes, OTHER_DESCRIPTOR_BYTES);
}

#[test]
fn finalized_and_descriptor_byte_mutations_are_independently_bound() {
    let base = fixture_preimage();
    let expected = calculate_protected_finalized_identity_v3(&base);

    let mut finalized_bytes_changed = base.clone();
    finalized_bytes_changed.exact_finalized_bytes = OTHER_FINAL_BYTES;
    assert_ne!(
        calculate_protected_finalized_identity_v3(&finalized_bytes_changed),
        expected
    );

    let mut descriptor_bytes_changed = base.clone();
    descriptor_bytes_changed.exact_canonical_descriptor_bytes = OTHER_DESCRIPTOR_BYTES;
    assert_ne!(
        calculate_protected_finalized_identity_v3(&descriptor_bytes_changed),
        expected
    );

    let mut descriptor_digest_changed = base;
    descriptor_digest_changed.canonical_digest = digest(0xee);
    assert_ne!(
        calculate_protected_finalized_identity_v3(&descriptor_digest_changed),
        expected
    );
}

fn fixture_preimage() -> ProtectedFinalizationIdentityPreimageV3<'static> {
    ProtectedFinalizationIdentityPreimageV3 {
        raw_inspection_identity: digest(0x01),
        source_evidence_identity: digest(0x02),
        binding_identity: digest(0x03),
        attempt: attempt(1),
        slot: CompilerModuleHandoffSlotV3::Production,
        transaction_identity: digest(0x04),
        receipt_byte_len: 100,
        outer_handoff_sha256: digest(0x05),
        outer_handoff_byte_len: OUTER_BYTES.len() as u64,
        exact_outer_handoff_bytes: OUTER_BYTES,
        capsule_sha256: digest(0x06),
        capsule_byte_len: 200,
        invocation_digest: digest(0x07),
        pair_binding_sha256: digest(0x08),
        pair_binding_byte_len: 201,
        nested_handoff_sha256: digest(0x09),
        nested_handoff_byte_len: 202,
        final_commitment_receipt_sha256: digest(0x0a),
        final_commitment_receipt_byte_len: 203,
        final_commitment_sha256: digest(0x0b),
        final_commitment_byte_len: 204,
        compiler_closure: compiler_closure(0x10),
        worker_executable: ContentIdentityV1::calculate(b"worker"),
        worker_build_identity: "worker-build",
        llvm_build_identity: "upstream-llvm-build",
        worker_limits: WorkerExecutionLimitsV1::new(Duration::from_secs(3), 2048, 256).unwrap(),
        link_plan_identity: digest(0x0c),
        exact_link_plan_bytes: PLAN_BYTES,
        response_identity: digest(0x0d),
        raw_output: ContentIdentityV1::calculate(RAW_BYTES),
        exact_raw_bytes: RAW_BYTES,
        policy_identity: digest(0x0e),
        descriptor_observation_identity: digest(0x0f),
        abi_observation_identity: digest(0x10),
        resource_observation_identity: digest(0x11),
        target: "gfx942:xnack-",
        code_object_version: CodeObjectVersion::V6,
        required_workgroup_size: [256, 1, 1],
        max_flat_workgroup_size: 256,
        wavefront_size: 64,
        observed_kernel_symbols_identity: digest(0x12),
        finalized_output: ContentIdentityV1::calculate(FINAL_BYTES),
        exact_finalized_bytes: FINAL_BYTES,
        canonical_digest: CanonicalCodeObjectDigest::calculate_from_canonicalized_hsaco(
            FINAL_BYTES,
        )
        .as_bytes()
        .to_owned(),
        canonical_descriptor_evidence: ContentIdentityV1::calculate(DESCRIPTOR_BYTES),
        exact_canonical_descriptor_bytes: DESCRIPTOR_BYTES,
    }
}

fn attempt(generation: u64) -> BuildAttempt {
    BuildAttempt::from_env_value(&format!(
        "{generation}:{}:{}",
        "11".repeat(16),
        "22".repeat(32)
    ))
    .unwrap()
}

fn compiler_closure(seed: u8) -> CompilerClosureV2 {
    CompilerClosureV2::new(
        digest(seed),
        digest(seed.wrapping_add(1)),
        digest(seed.wrapping_add(2)),
        digest(seed.wrapping_add(3)),
        digest(seed.wrapping_add(4)),
        digest(seed.wrapping_add(5)),
    )
    .unwrap()
}

const fn digest(seed: u8) -> [u8; 32] {
    [seed; 32]
}
