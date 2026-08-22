use std::sync::Arc;

use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerFfiEnvelopeV1, CompilerModuleHandoffErrorV2,
    CompilerModuleHandoffV2, CompilerModuleKindV1, CompilerModuleSymbolManifestV1,
    CompilerModuleSymbolRoleV1, DeviceTargetV1, MAX_COMPILER_MODULE_HANDOFF_BYTES_V2,
};

const LLVM_IR: &[u8] =
    b"; ModuleID = 'shared-v2'\ndefine amdgpu_kernel void @kernel() { ret void }\n";

fn handoff() -> CompilerModuleHandoffV2 {
    let target = DeviceTargetV1::parse("gfx942:xnack-").unwrap();
    let envelope =
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V5)
            .unwrap();
    let manifest = CompilerModuleSymbolManifestV1::new([
        (CompilerModuleSymbolRoleV1::KernelEntry, "kernel"),
        (CompilerModuleSymbolRoleV1::KernelDescriptor, "kernel.kd"),
    ])
    .unwrap();
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V5,
        envelope,
        manifest,
        LLVM_IR,
    )
    .unwrap()
}

#[test]
fn owned_decode_retains_exact_boxed_allocation_and_identity() {
    let expected = handoff();
    let owned = expected.canonical_bytes().to_vec().into_boxed_slice();
    let owned_pointer = owned.as_ptr();

    let decoded = CompilerModuleHandoffV2::decode_owned(owned).unwrap();

    assert_eq!(decoded.canonical_bytes().as_ptr(), owned_pointer);
    assert_eq!(decoded, expected);
    assert_eq!(decoded.identity(), expected.identity());
    assert_eq!(decoded.module_identity(), expected.module_identity());
    assert_eq!(decoded.module_bytes(), LLVM_IR);
}

#[test]
fn shared_range_decode_retains_outer_backing_without_a_v2_copy() {
    const PREFIX_BYTES: usize = 19;
    const SUFFIX_BYTES: usize = 23;

    let expected = handoff();
    let canonical_len = expected.canonical_bytes().len();
    let mut outer = vec![0xa5; PREFIX_BYTES];
    outer.extend_from_slice(expected.canonical_bytes());
    outer.extend_from_slice(&[0x5a; SUFFIX_BYTES]);
    let backing: Arc<[u8]> = outer.into();
    let expected_pointer = backing[PREFIX_BYTES..].as_ptr();

    let decoded = CompilerModuleHandoffV2::decode_shared_range(
        Arc::clone(&backing),
        PREFIX_BYTES,
        canonical_len,
    )
    .unwrap();

    assert_eq!(decoded.canonical_bytes().as_ptr(), expected_pointer);
    assert_eq!(decoded.canonical_bytes(), expected.canonical_bytes());
    assert_eq!(decoded, expected);
    assert_eq!(Arc::strong_count(&backing), 2);
    drop(backing);
    assert_eq!(decoded.module_bytes(), LLVM_IR);
    assert!(decoded.identity().matches(decoded.canonical_bytes()));
}

#[test]
fn full_shared_decode_and_all_legacy_accessors_are_compatible() {
    let expected = handoff();
    let backing: Arc<[u8]> = expected.canonical_bytes().to_vec().into();
    let expected_pointer = backing.as_ptr();
    let decoded = CompilerModuleHandoffV2::try_from(Arc::clone(&backing)).unwrap();

    assert_eq!(decoded.canonical_bytes().as_ptr(), expected_pointer);
    assert_eq!(decoded.kind(), expected.kind());
    assert_eq!(decoded.target(), expected.target());
    assert_eq!(
        decoded.code_object_version(),
        expected.code_object_version()
    );
    assert_eq!(decoded.envelope(), expected.envelope());
    assert_eq!(decoded.symbol_manifest(), expected.symbol_manifest());
    assert_eq!(decoded.module_identity(), expected.module_identity());
    assert_eq!(decoded.identity(), expected.identity());
    assert_eq!(decoded, expected);
}

#[test]
fn hostile_shared_ranges_and_lengths_fail_with_deterministic_errors() {
    let expected = handoff();
    let backing: Arc<[u8]> = expected.canonical_bytes().to_vec().into();

    assert_eq!(
        CompilerModuleHandoffV2::decode_shared_range(Arc::clone(&backing), usize::MAX, 1),
        Err(CompilerModuleHandoffErrorV2::SharedBackingRangeOverflow)
    );
    assert_eq!(
        CompilerModuleHandoffV2::decode_shared_range(Arc::clone(&backing), backing.len(), 1,),
        Err(CompilerModuleHandoffErrorV2::SharedBackingRangeOutOfBounds)
    );
    assert_eq!(
        CompilerModuleHandoffV2::decode_shared_range(
            backing,
            0,
            MAX_COMPILER_MODULE_HANDOFF_BYTES_V2 + 1,
        ),
        Err(CompilerModuleHandoffErrorV2::HandoffByteBoundExceeded)
    );
}

#[test]
fn complete_canonical_buffer_budget_is_exact_for_each_decode_mode() {
    assert_eq!(
        CompilerModuleHandoffV2::OWNED_DECODE_ADDITIONAL_CANONICAL_BUFFERS,
        0
    );
    assert_eq!(
        CompilerModuleHandoffV2::SHARED_DECODE_ADDITIONAL_CANONICAL_BUFFERS,
        0
    );
    assert_eq!(
        CompilerModuleHandoffV2::MAX_BORROWED_DECODE_ADDITIONAL_CANONICAL_BYTES,
        MAX_COMPILER_MODULE_HANDOFF_BYTES_V2
    );
}
