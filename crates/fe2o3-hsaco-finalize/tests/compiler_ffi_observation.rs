use fe2o3_hsaco_finalize::{
    CompilerFfiCodeObjectVersion, CompilerFfiContractV1, CompilerFfiDeviceTargetV1,
    CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1,
    StagedCompilerFfiEnvelopeBlockerV1, stage_compiler_ffi_envelope_v1,
};
use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_EXPORT_V1, DeviceFfiContractFieldsV1, DeviceFfiDirectionV1,
    derive_device_ffi_contract_id_v1,
};

const EXPORT_ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn compiler_envelope(semantic_byte: u8) -> fe2o3_hsaco_finalize::CompilerFfiEnvelopeV1 {
    let target = CompilerFfiDeviceTargetV1::parse("gfx942:xnack-").unwrap();
    let semantic_identity = [semantic_byte; 32];
    let semantic_text = lower_hex(&semantic_identity);
    let fields = DeviceFfiContractFieldsV1 {
        direction: DEVICE_FFI_DIRECTION_EXPORT_V1,
        symbol: "rust_helper",
        calling_convention: "C",
        code_object_version: 5,
        target: "gfx942:xnack-",
        physical_abi: EXPORT_ABI,
        effects: "none",
        semantic_identity: &semantic_text,
    };
    let owner = CompilerFfiSourceOwnerV1::new(
        "ffi_crate",
        "ffi_crate::rust_helper",
        [0x22; 16],
        "_RINvNtCs1234_9ffi_craterust_helper",
    )
    .unwrap();
    let contract = CompilerFfiContractV1::new(
        derive_device_ffi_contract_id_v1(fields),
        DeviceFfiDirectionV1::Export,
        CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition,
        target,
        CompilerFfiCodeObjectVersion::V5,
        owner,
        "rust_helper",
        EXPORT_ABI,
        "none",
        semantic_identity,
    )
    .unwrap();
    let mut builder =
        CompilerFfiEnvelopeBuilderV1::new(target, CompilerFfiCodeObjectVersion::V5, 1).unwrap();
    builder.push(contract).unwrap();
    builder.finish().unwrap()
}

#[test]
fn complete_cross_crate_envelope_is_staged_without_worker_authority() {
    let envelope = compiler_envelope(0x22);
    let envelope_identity = envelope.identity();
    let staged = stage_compiler_ffi_envelope_v1(envelope);
    let inspection = staged.inspection();

    assert_eq!(inspection.envelope_identity(), envelope_identity);
    assert_eq!(inspection.target().to_string(), "gfx942:xnack-");
    assert_eq!(
        inspection.code_object_version(),
        CompilerFfiCodeObjectVersion::V5
    );
    assert_eq!(inspection.import_count(), 0);
    assert_eq!(inspection.export_count(), 1);
    assert_eq!(
        inspection.blocker(),
        StagedCompilerFfiEnvelopeBlockerV1::MissingExactCompilerModuleArtifactAndWorkerProtocolV2
    );
    assert_eq!(
        staged.identity().to_hex(),
        "fa427b43b74f5be4255fc4e3e033b7d46bddcac33b4958277b2cc30d71c0915c"
    );
    assert!(!staged.grants_worker_authority());
}

#[test]
fn staged_identity_binds_the_complete_canonical_envelope() {
    let first = stage_compiler_ffi_envelope_v1(compiler_envelope(0x22));
    let repeated = stage_compiler_ffi_envelope_v1(compiler_envelope(0x22));
    let changed_semantics = stage_compiler_ffi_envelope_v1(compiler_envelope(0x23));

    assert_eq!(first.identity(), repeated.identity());
    assert_ne!(first.identity(), changed_semantics.identity());
    assert_ne!(
        first.inspection().envelope_identity(),
        changed_semantics.inspection().envelope_identity()
    );
}
