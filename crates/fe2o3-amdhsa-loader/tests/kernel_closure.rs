use std::{env, fs};

use fe2o3_amdhsa_loader::{
    AdmittedProfile, CLOSED_RELOCATION_POLICY_ID, KernelClosureError, KernelDispatchAbiErrorV1,
    KernelGlobalBufferAbiV1, ValidatedKernelEnvelope, validate,
};
use fe2o3_hsaco::ArgumentAccess;
use sha2::{Digest, Sha256};

const PROFILE: AdmittedProfile = AdmittedProfile::Gfx942XnackOffCov6;

#[test]
#[ignore = "requires FE2O3_TEST_VECADD_COV6 to name a real gfx942:xnack- COV6 vecadd"]
fn closes_real_vecadd_semantics_symbols_resources_and_identity() {
    assert_real_closure("FE2O3_TEST_VECADD_COV6", "vecadd");
}

#[test]
#[ignore = "requires FE2O3_TEST_SCALAR_GEMM_COV6 to name a real gfx942:xnack- COV6 scalar GEMM"]
fn closes_real_scalar_gemm_semantics_symbols_resources_and_identity() {
    assert_real_closure("FE2O3_TEST_SCALAR_GEMM_COV6", "scalar_gemm_v1");
}

#[test]
#[ignore = "requires FE2O3_TEST_SCALAR_GEMM_COV6 to name a real gfx942:xnack- COV6 scalar GEMM"]
fn reconciles_complete_scalar_gemm_dispatch_abi_and_rejects_hostile_rosters() {
    const BASELINE: [KernelGlobalBufferAbiV1<'static>; 3] = [
        KernelGlobalBufferAbiV1::new(0, "arg0.data", 0, 4, ArgumentAccess::ReadOnly),
        KernelGlobalBufferAbiV1::new(2, "arg1.data", 16, 4, ArgumentAccess::ReadOnly),
        KernelGlobalBufferAbiV1::new(4, "arg2.data", 32, 4, ArgumentAccess::WriteOnly),
    ];
    let bytes = scalar_gemm_bytes();
    let raw = scalar_gemm_closure(&bytes);
    for index in [0, 2, 4] {
        assert_eq!(raw.dispatch_pointee_alignment(index), None);
        assert_eq!(raw.dispatch_actual_access(index), None);
    }

    assert_eq!(
        reconciliation_error(
            scalar_gemm_closure(&bytes).reconcile_dispatch_abi([0; 32], &BASELINE),
        ),
        KernelDispatchAbiErrorV1::MissingSourceContractIdentity
    );
    let first = scalar_gemm_closure(&bytes)
        .reconcile_dispatch_abi([0x31; 32], &BASELINE)
        .unwrap();
    let first_identity = first.dispatch_abi_identity().unwrap();
    for (index, access) in [
        (0, ArgumentAccess::ReadOnly),
        (2, ArgumentAccess::ReadOnly),
        (4, ArgumentAccess::WriteOnly),
    ] {
        assert_eq!(first.dispatch_pointee_alignment(index), Some(4));
        assert_eq!(first.dispatch_actual_access(index), Some(access));
    }
    assert_eq!(
        reconciliation_error(first.reconcile_dispatch_abi([0x31; 32], &BASELINE)),
        KernelDispatchAbiErrorV1::AlreadyReconciled
    );

    let reordered = [BASELINE[2], BASELINE[0], BASELINE[1]];
    assert_eq!(
        scalar_gemm_closure(&bytes)
            .reconcile_dispatch_abi([0x31; 32], &reordered)
            .unwrap()
            .dispatch_abi_identity(),
        Some(first_identity)
    );
    assert_ne!(
        scalar_gemm_closure(&bytes)
            .reconcile_dispatch_abi([0x32; 32], &BASELINE)
            .unwrap()
            .dispatch_abi_identity(),
        Some(first_identity)
    );

    let hostile = [
        (
            &BASELINE[..2],
            KernelDispatchAbiErrorV1::GlobalBufferCardinality,
        ),
        (
            &[
                BASELINE[0],
                BASELINE[1],
                KernelGlobalBufferAbiV1::new(99, "arg2.data", 32, 4, ArgumentAccess::WriteOnly),
            ][..],
            KernelDispatchAbiErrorV1::ExplicitArgumentIndex,
        ),
        (
            &[
                BASELINE[0],
                BASELINE[1],
                KernelGlobalBufferAbiV1::new(2, "arg1.data", 16, 4, ArgumentAccess::ReadOnly),
            ][..],
            KernelDispatchAbiErrorV1::DuplicateExplicitArgument,
        ),
        (
            &[
                BASELINE[0],
                BASELINE[1],
                KernelGlobalBufferAbiV1::new(3, "arg1.len", 24, 4, ArgumentAccess::ReadOnly),
            ][..],
            KernelDispatchAbiErrorV1::NotGlobalBuffer,
        ),
        (
            &[
                BASELINE[0],
                BASELINE[1],
                KernelGlobalBufferAbiV1::new(4, "", 32, 4, ArgumentAccess::WriteOnly),
            ][..],
            KernelDispatchAbiErrorV1::InvalidArgumentName,
        ),
        (
            &[
                BASELINE[0],
                BASELINE[1],
                KernelGlobalBufferAbiV1::new(4, "wrong.data", 32, 4, ArgumentAccess::WriteOnly),
            ][..],
            KernelDispatchAbiErrorV1::ArgumentNameMismatch,
        ),
        (
            &[
                BASELINE[0],
                BASELINE[1],
                KernelGlobalBufferAbiV1::new(4, "arg2.data", 40, 4, ArgumentAccess::WriteOnly),
            ][..],
            KernelDispatchAbiErrorV1::ArgumentOffsetMismatch,
        ),
        (
            &[
                BASELINE[0],
                BASELINE[1],
                KernelGlobalBufferAbiV1::new(4, "arg2.data", 32, 0, ArgumentAccess::WriteOnly),
            ][..],
            KernelDispatchAbiErrorV1::InvalidPointeeAlignment,
        ),
        (
            &[
                BASELINE[0],
                BASELINE[1],
                KernelGlobalBufferAbiV1::new(4, "arg2.data", 32, 3, ArgumentAccess::WriteOnly),
            ][..],
            KernelDispatchAbiErrorV1::InvalidPointeeAlignment,
        ),
    ];
    for (roster, expected) in hostile {
        assert_eq!(
            reconciliation_error(
                scalar_gemm_closure(&bytes).reconcile_dispatch_abi([0x31; 32], roster),
            ),
            expected
        );
    }
}

#[test]
#[ignore = "requires FE2O3_TEST_VECADD_COV6 to name a real gfx942:xnack- COV6 vecadd"]
fn hostile_substitution_descriptor_metadata_and_entry_drift_fail_or_rebind() {
    let path = env::var("FE2O3_TEST_VECADD_COV6").expect("set FE2O3_TEST_VECADD_COV6");
    let bytes = fs::read(path).unwrap();
    let envelope = validate(&bytes, PROFILE).unwrap();
    let metadata_offset = envelope.plan().metadata_note().file_offset() as usize;
    let closure = envelope.bind_kernel("vecadd").unwrap();
    let descriptor = closure.descriptor_bytes().to_vec();
    let entry = closure.entry_bytes().to_vec();
    let descriptor_offset = unique_subslice_offset(&bytes, &descriptor);
    let entry_offset = unique_subslice_offset(&bytes, &entry);
    let original_identity = closure.identity_inputs();
    drop(closure);

    let mut metadata_drift = bytes.clone();
    metadata_drift[metadata_offset] ^= 0xff;
    let envelope = validate(&metadata_drift, PROFILE).unwrap();
    assert!(matches!(
        envelope.bind_kernel("vecadd"),
        Err(KernelClosureError::Inspection(_))
    ));

    let mut substituted_note = bytes.clone();
    let note_header = note_section_header(&bytes);
    let note_offset = read_u64(&bytes, note_header + 24) as usize;
    let note_size = read_u64(&bytes, note_header + 32) as usize;
    let replacement_offset = substituted_note.len();
    substituted_note.extend_from_slice(&bytes[note_offset..note_offset + note_size]);
    write_u64(
        &mut substituted_note,
        note_header + 24,
        replacement_offset as u64,
    );
    let envelope = validate(&substituted_note, PROFILE).unwrap();
    assert!(matches!(
        envelope.bind_kernel("vecadd"),
        Err(KernelClosureError::MetadataRangeMismatch { .. })
    ));

    let mut descriptor_drift = bytes.clone();
    descriptor_drift[descriptor_offset + 12] = 1;
    let envelope = validate(&descriptor_drift, PROFILE).unwrap();
    assert!(matches!(
        envelope.bind_kernel("vecadd"),
        Err(KernelClosureError::Inspection(_))
    ));

    let mut entry_drift = bytes.clone();
    entry_drift[entry_offset] ^= 1;
    let rebound = validate(&entry_drift, PROFILE)
        .unwrap()
        .bind_kernel("vecadd")
        .unwrap();
    let rebound_identity = rebound.identity_inputs();
    assert_ne!(
        rebound_identity.object_sha256(),
        original_identity.object_sha256()
    );
    assert_eq!(
        rebound_identity.metadata_sha256(),
        original_identity.metadata_sha256()
    );
    assert_eq!(
        rebound_identity.descriptor_sha256(),
        original_identity.descriptor_sha256()
    );
    assert_ne!(
        rebound_identity.entry_sha256(),
        original_identity.entry_sha256()
    );
    assert_ne!(
        rebound_identity.closure_sha256(),
        original_identity.closure_sha256()
    );
}

#[test]
#[ignore = "requires FE2O3_TEST_VECADD_COV6 to name a real gfx942:xnack- COV6 vecadd"]
fn hostile_truncation_and_kernel_selection_fail_closed() {
    let path = env::var("FE2O3_TEST_VECADD_COV6").expect("set FE2O3_TEST_VECADD_COV6");
    let bytes = fs::read(path).unwrap();
    let plan = validate(&bytes, PROFILE).unwrap().plan().to_owned();
    let metadata_end = plan
        .metadata_note()
        .file_offset()
        .checked_add(plan.metadata_note().byte_len())
        .unwrap() as usize;
    for end in [0, 63, metadata_end - 1, bytes.len() - 1] {
        assert!(
            validate(&bytes[..end], PROFILE).is_err(),
            "accepted prefix {end}"
        );
    }

    assert!(matches!(
        validate(&bytes, PROFILE).unwrap().bind_kernel("not_vecadd"),
        Err(KernelClosureError::KernelNotFound)
    ));
    let oversized = "x".repeat(fe2o3_hsaco::MAX_MESSAGEPACK_STRING_BYTES + 1);
    assert!(matches!(
        validate(&bytes, PROFILE).unwrap().bind_kernel(&oversized),
        Err(KernelClosureError::KernelNameTooLong)
    ));
}

fn assert_real_closure(environment: &str, kernel_name: &str) {
    let path = env::var(environment).unwrap_or_else(|_| panic!("set {environment}"));
    let bytes = fs::read(path).unwrap();
    let closure = validate(&bytes, PROFILE)
        .unwrap()
        .bind_kernel(kernel_name)
        .unwrap();
    assert_eq!(closure.selected_kernel().name(), kernel_name);
    assert_eq!(closure.selected_kernel_index(), 0);
    let binding = closure.selected_binding();
    assert_eq!(binding.kernel_index(), closure.selected_kernel_index());
    assert_eq!(closure.descriptor_bytes().len(), 64);
    assert!(!closure.entry_bytes().is_empty());
    assert_eq!(
        closure.resources().kernarg_segment_size(),
        closure.selected_kernel().kernarg_segment_size()
    );
    assert_eq!(binding.descriptor(), closure.resources().descriptor());
    assert_eq!(
        u64::from(binding.descriptor().kernarg_size()),
        closure.resources().kernarg_segment_size()
    );
    assert_eq!(
        u64::from(binding.descriptor().private_segment_fixed_size()),
        closure.resources().private_segment_fixed_size()
    );
    assert_eq!(
        u64::from(binding.descriptor().group_segment_fixed_size()),
        closure.resources().group_segment_fixed_size()
    );
    assert_eq!(
        i128::from(binding.descriptor_address())
            + i128::from(binding.descriptor().kernel_code_entry_byte_offset()),
        i128::from(binding.entry_address())
    );
    assert_eq!(
        closure.resources().wavefront_size(),
        closure.resources().descriptor().wavefront_size()
    );
    assert_eq!(
        closure.relocation_evidence().policy_id(),
        CLOSED_RELOCATION_POLICY_ID
    );
    assert_eq!(closure.relocation_evidence().admitted_relocation_count(), 0);
    assert_eq!(closure.relocation_evidence().applied_relocation_count(), 0);
    assert_eq!(
        closure.identity_inputs().object_sha256(),
        <[u8; 32]>::from(Sha256::digest(&bytes))
    );
    assert_eq!(
        closure.identity_inputs().metadata_sha256(),
        <[u8; 32]>::from(Sha256::digest(closure.envelope().metadata_descriptor()))
    );
    assert_eq!(
        closure.identity_inputs().descriptor_sha256(),
        <[u8; 32]>::from(Sha256::digest(closure.descriptor_bytes()))
    );
    assert_eq!(
        closure.identity_inputs().entry_sha256(),
        <[u8; 32]>::from(Sha256::digest(closure.entry_bytes()))
    );
    let mut image = vec![0xa5; closure.envelope().materialization().image_len() as usize];
    closure.materialize_into(&mut image).unwrap();
}

fn scalar_gemm_bytes() -> Vec<u8> {
    let path = env::var("FE2O3_TEST_SCALAR_GEMM_COV6").expect("set FE2O3_TEST_SCALAR_GEMM_COV6");
    fs::read(path).unwrap()
}

fn scalar_gemm_closure(bytes: &[u8]) -> ValidatedKernelEnvelope<'_> {
    validate(bytes, PROFILE)
        .unwrap()
        .bind_kernel("scalar_gemm_v1")
        .unwrap()
}

fn reconciliation_error(
    result: Result<ValidatedKernelEnvelope<'_>, KernelDispatchAbiErrorV1>,
) -> KernelDispatchAbiErrorV1 {
    match result {
        Ok(_) => panic!("hostile dispatch ABI unexpectedly reconciled"),
        Err(error) => error,
    }
}

fn unique_subslice_offset(bytes: &[u8], needle: &[u8]) -> usize {
    let matches = bytes
        .windows(needle.len())
        .enumerate()
        .filter_map(|(offset, candidate)| (candidate == needle).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "expected one exact subslice occurrence");
    matches[0]
}

fn note_section_header(bytes: &[u8]) -> usize {
    let section_offset = read_u64(bytes, 40) as usize;
    let section_entry_size = usize::from(read_u16(bytes, 58));
    let section_count = usize::from(read_u16(bytes, 60));
    (0..section_count)
        .map(|index| section_offset + index * section_entry_size)
        .find(|header| read_u32(bytes, *header + 4) == 7)
        .expect("real artifact must contain SHT_NOTE")
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
