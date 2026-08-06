use fe2o3_hsaco_finalize::{
    CompilerFfiCodeObjectVersion, CompilerFfiContractV1, CompilerFfiDeviceTargetV1,
    CompilerFfiEnvelopeBuilderV1, CompilerFfiLinkRoleV1, CompilerFfiSourceOwnerV1,
    ContentIdentityV1, LinkInputKindClosureV1, LinkInputV1, LinkOptionV1, LinkOutputV1,
    LinkSymbolClosureV1, MultiInputLinkPlanV1, ProvenanceNodeV1, WorkerEvidenceClassV2,
    WorkerInputKindV1, WorkerInputV1, WorkerMeasurementV1, WorkerOptimizationLevelV1,
    WorkerOptionsV1, WorkerOutputConstraintsV1, construct_worker_request_v2,
    stage_compiler_ffi_envelope_v1, stage_exact_compiler_module_artifact_v1,
};
use fe2o3_kernel_descriptor::{CodeObjectVersion, DeviceTargetV1};
use reserved_fe2o3_symbols::{
    DEVICE_FFI_DIRECTION_EXPORT_V1, DEVICE_FFI_DIRECTION_IMPORT_V1, DeviceFfiContractFieldsV1,
    DeviceFfiDirectionV1, derive_device_ffi_contract_id_v1,
};

const ABI: &str = "C(u32[size=4,align=4])->u32[size=4,align=4]";
const MODULE: &[u8] = b"exact compiler module bitcode";
const PROVIDER: &[u8] = b"exact external provider object";

struct Fixture {
    plan: MultiInputLinkPlanV1,
    input_kinds: LinkInputKindClosureV1,
    provider: WorkerInputV1,
    output_bound: WorkerOutputConstraintsV1,
}

fn target() -> DeviceTargetV1 {
    DeviceTargetV1::parse("gfx942:xnack-").unwrap()
}

fn options() -> WorkerOptionsV1 {
    WorkerOptionsV1::new(WorkerOptimizationLevelV1::O2, true, true)
}

fn fixture() -> Fixture {
    let module = WorkerInputV1::new(WorkerInputKindV1::LlvmBitcode, MODULE.to_vec()).unwrap();
    let provider =
        WorkerInputV1::new(WorkerInputKindV1::AmdGpuRelocatable, PROVIDER.to_vec()).unwrap();
    let mut inputs = [module, provider.clone()];
    inputs.sort_by_key(|input| (input.identity(), input.kind()));
    let link_inputs = inputs
        .iter()
        .map(|input| LinkInputV1::new(input.identity(), target()))
        .collect::<Vec<_>>();
    let output_identity = ContentIdentityV1::calculate(b"expected exact hsaco");
    let mut provenance = link_inputs
        .iter()
        .map(|input| ProvenanceNodeV1::new(input.identity(), vec![]).unwrap())
        .collect::<Vec<_>>();
    provenance.push(
        ProvenanceNodeV1::new(
            output_identity,
            link_inputs.iter().map(|input| input.identity()).collect(),
        )
        .unwrap(),
    );
    let plan = MultiInputLinkPlanV1::canonicalized(
        target(),
        link_inputs,
        vec![
            LinkOptionV1::new("code-object-version", "6").unwrap(),
            LinkOptionV1::new("opt-level", "2").unwrap(),
            LinkOptionV1::new("strip-debug", "true").unwrap(),
            LinkOptionV1::new("verify-each", "true").unwrap(),
        ],
        LinkOutputV1::new(output_identity, target()),
        provenance,
    )
    .unwrap();
    let input_kinds =
        LinkInputKindClosureV1::new(&plan, inputs.iter().map(|input| input.kind()).collect())
            .unwrap();
    Fixture {
        plan,
        input_kinds,
        provider,
        output_bound: WorkerOutputConstraintsV1::new(output_identity.byte_len()).unwrap(),
    }
}

fn envelope(imports: bool) -> fe2o3_hsaco_finalize::CompilerFfiEnvelopeV1 {
    let compiler_target = CompilerFfiDeviceTargetV1::parse("gfx942:xnack-").unwrap();
    let count = if imports { 2 } else { 1 };
    let mut builder =
        CompilerFfiEnvelopeBuilderV1::new(compiler_target, CompilerFfiCodeObjectVersion::V6, count)
            .unwrap();
    if imports {
        builder
            .push(contract(
                "external_add",
                DeviceFfiDirectionV1::Import,
                CompilerFfiLinkRoleV1::RequiresExternalDefinition,
                0x31,
            ))
            .unwrap();
    }
    builder
        .push(contract(
            "rust_helper",
            DeviceFfiDirectionV1::Export,
            CompilerFfiLinkRoleV1::RequiresCompilerModuleDefinition,
            0x42,
        ))
        .unwrap();
    builder.finish().unwrap()
}

fn contract(
    symbol: &str,
    direction: DeviceFfiDirectionV1,
    role: CompilerFfiLinkRoleV1,
    semantic_byte: u8,
) -> CompilerFfiContractV1 {
    let semantic_identity = [semantic_byte; 32];
    let semantic_text = lower_hex(&semantic_identity);
    let direction_tag = match direction {
        DeviceFfiDirectionV1::Import => DEVICE_FFI_DIRECTION_IMPORT_V1,
        DeviceFfiDirectionV1::Export => DEVICE_FFI_DIRECTION_EXPORT_V1,
    };
    let fields = DeviceFfiContractFieldsV1 {
        direction: direction_tag,
        symbol,
        calling_convention: "C",
        code_object_version: 6,
        target: "gfx942:xnack-",
        physical_abi: ABI,
        effects: "none",
        semantic_identity: &semantic_text,
    };
    let owner = CompilerFfiSourceOwnerV1::new(
        "ffi_crate",
        &format!("ffi_crate::{symbol}"),
        [semantic_byte; 16],
        &format!("_RINvNtCs1234_9ffi_crate{symbol}"),
    )
    .unwrap();
    CompilerFfiContractV1::new(
        derive_device_ffi_contract_id_v1(fields),
        direction,
        role,
        CompilerFfiDeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        CompilerFfiCodeObjectVersion::V6,
        owner,
        symbol,
        ABI,
        "none",
        semantic_identity,
    )
    .unwrap()
}

fn closure() -> LinkSymbolClosureV1 {
    LinkSymbolClosureV1::new(
        strings(&["external_add", "kernel_main", "rust_helper"]),
        strings(&["external_add"]),
        strings(&["rust_helper"]),
    )
    .unwrap()
}

fn measurement() -> WorkerMeasurementV1 {
    WorkerMeasurementV1::new(
        ContentIdentityV1::calculate(b"pinned worker executable"),
        "worker-v2-build",
        "llvm-v2-build",
    )
    .unwrap()
}

#[test]
fn sealed_v2_request_binds_every_compiler_link_input() {
    let fixture = fixture();
    let staged = stage_compiler_ffi_envelope_v1(envelope(true));
    let compiler_identity = staged.inspection().envelope_identity();
    let artifact =
        stage_exact_compiler_module_artifact_v1(WorkerInputKindV1::LlvmBitcode, MODULE.to_vec())
            .unwrap();
    let artifact_identity = artifact.identity();
    let request = construct_worker_request_v2(
        &fixture.plan,
        &measurement(),
        target(),
        CodeObjectVersion::V6,
        options(),
        staged,
        artifact,
        vec![fixture.provider.clone()],
        &fixture.input_kinds,
        &closure(),
        fixture.output_bound.clone(),
    )
    .unwrap();

    assert_eq!(request.canonical_bytes()[..8], *b"F3LREQ02");
    assert_eq!(request.compiler_module().identity(), artifact_identity);
    assert_eq!(
        request.compiler_envelope_identity().as_bytes(),
        compiler_identity.as_bytes()
    );
    assert_eq!(request.external_providers(), &[fixture.provider]);
    assert_eq!(request.import_symbols(), strings(&["external_add"]));
    assert_eq!(request.export_symbols(), strings(&["rust_helper"]));
    assert_eq!(
        request.final_symbols(),
        strings(&["external_add", "kernel_main", "rust_helper"])
    );
    assert_eq!(
        request.evidence_class(),
        WorkerEvidenceClassV2::CompilerFfiLink
    );
    assert!(!request.grants_publication_authority());
    assert!(!request.grants_load_authority());
    assert!(!request.grants_launch_authority());
}

#[test]
fn exact_artifact_and_envelope_closure_mismatches_fail_closed() {
    let fixture = fixture();
    let wrong_artifact = stage_exact_compiler_module_artifact_v1(
        WorkerInputKindV1::LlvmBitcode,
        b"different module".to_vec(),
    )
    .unwrap();
    assert!(
        construct_worker_request_v2(
            &fixture.plan,
            &measurement(),
            target(),
            CodeObjectVersion::V6,
            options(),
            stage_compiler_ffi_envelope_v1(envelope(true)),
            wrong_artifact,
            vec![fixture.provider.clone()],
            &fixture.input_kinds,
            &closure(),
            fixture.output_bound.clone(),
        )
        .is_err()
    );

    let artifact =
        stage_exact_compiler_module_artifact_v1(WorkerInputKindV1::LlvmBitcode, MODULE.to_vec())
            .unwrap();
    assert!(
        construct_worker_request_v2(
            &fixture.plan,
            &measurement(),
            target(),
            CodeObjectVersion::V6,
            options(),
            stage_compiler_ffi_envelope_v1(envelope(false)),
            artifact,
            vec![fixture.provider],
            &fixture.input_kinds,
            &closure(),
            fixture.output_bound,
        )
        .is_err()
    );
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
