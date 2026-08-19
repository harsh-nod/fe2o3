#![cfg(target_os = "linux")]
#![doc = "Integration tests for the exact scalar-add Worker V2 bridge."]

use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use fe2o3_amdgcn_pliron_llvm::{ScalarKernelModuleV1, lower_scalar_kernel_v2};
use fe2o3_artifact_transaction::{
    BuildInvocation, BuildSession, ConsumedCompilerModuleHandoffV1, ProducerIdentity,
    begin_build_attempt, consume_compiler_module_handoff_v1, publish_compiler_module_handoff_v1,
};
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerFfiEnvelopeV1, CompilerModuleHandoffV2, CompilerModuleKindV1,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1, DeviceTargetV1,
};
use fe2o3_hsaco_finalize::{
    ContentIdentityV1, LinkInputKindClosureV1, LinkInputV1, LinkOptionV1, LinkOutputV1,
    LinkPlanError, MultiInputLinkPlanV1, PinnedWorkerV1, ProvenanceNodeV1,
    WorkerExecutionErrorKind, WorkerExecutionLimitsV1, WorkerInputKindV1, WorkerMeasurementV1,
    WorkerOutputConstraintsV1,
};
use fe2o3_llvm_handoff::{
    BasicBlockV2, BinaryOperationV2, CallingConventionV2, DeviceLibraryInputV1,
    DeviceLibraryKindV1, ExecutableModuleV2, FloatBinaryOperationV2, FunctionAttributeV1,
    FunctionAttributeV2, FunctionIdV2, FunctionKindV2, FunctionV2, Gfx942HandoffInputV1,
    Gfx942HandoffV1, Gfx942HandoffV2, GlobalIdV2, GlobalLinkageV2, GlobalV2, IdentityV1,
    InstructionKindV2, InstructionV2, IntrinsicReferenceV2, IntrinsicV2, KernelEntryV1,
    ModuleMetadataV1, ScalarConstantV2, ScalarTypeV1, StageIdentitiesV1, TerminatorV2,
    WavesPerEuV1,
};
use fe2o3_llvm_worker_handoff::EXACT_LLVM_BUILD_IDENTITY_V1;
use fe2o3_pliron_worker_v2::{
    ConstructScalarAddWorkerRequestV2ErrorV1, PrepareScalarAddWorkerV2ErrorV1,
    SCALAR_ADD_DEVICE_TARGET_V1, SCALAR_ADD_KERNEL_DESCRIPTOR_SYMBOL_V1,
    SCALAR_ADD_KERNEL_SYMBOL_V1, ScalarAddProfileFieldV1, ScalarAddWorkerRequestFieldV1,
    construct_scalar_add_worker_request_v2, prepare_scalar_add_worker_v2,
};

const EXPECTED_HSACO: &[u8] = b"expected scalar-add hsaco bytes";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-pliron-worker-v2-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn source_request(seed: u8) -> ScalarKernelModuleV1 {
    ScalarKernelModuleV1::canonical(
        "scalar_module",
        SCALAR_ADD_KERNEL_SYMBOL_V1,
        IdentityV1::new([seed; 32]).unwrap(),
        StageIdentitiesV1::new(
            [seed.wrapping_add(1); 32],
            [seed.wrapping_add(2); 32],
            [seed.wrapping_add(3); 32],
        )
        .unwrap(),
    )
}

fn exact_handoff(seed: u8) -> Gfx942HandoffV2 {
    lower_scalar_kernel_v2(&source_request(seed)).unwrap()
}

fn target() -> DeviceTargetV1 {
    DeviceTargetV1::parse(SCALAR_ADD_DEVICE_TARGET_V1).unwrap()
}

fn pinned_worker(worker_build: &str, llvm_build: &str) -> PinnedWorkerV1 {
    pinned_worker_at(Path::new("/usr/bin/true"), worker_build, llvm_build)
}

fn pinned_worker_at(path: &Path, worker_build: &str, llvm_build: &str) -> PinnedWorkerV1 {
    let bytes = fs::read(path).unwrap();
    let measurement = WorkerMeasurementV1::new(
        ContentIdentityV1::calculate(&bytes),
        worker_build,
        llvm_build,
    )
    .unwrap();
    PinnedWorkerV1::open(path, measurement).unwrap()
}

fn plan(
    input: ContentIdentityV1,
    plan_target: DeviceTargetV1,
    code_object_version: &str,
    optimization: &str,
) -> MultiInputLinkPlanV1 {
    plan_with_options(
        input,
        plan_target,
        vec![
            LinkOptionV1::new("code-object-version", code_object_version).unwrap(),
            LinkOptionV1::new("opt-level", optimization).unwrap(),
            LinkOptionV1::new("strip-debug", "true").unwrap(),
            LinkOptionV1::new("verify-each", "true").unwrap(),
        ],
    )
}

fn plan_with_options(
    input: ContentIdentityV1,
    plan_target: DeviceTargetV1,
    options: Vec<LinkOptionV1>,
) -> MultiInputLinkPlanV1 {
    let output = ContentIdentityV1::calculate(EXPECTED_HSACO);
    let input_node = ProvenanceNodeV1::new(input, vec![]).unwrap();
    let output_node = ProvenanceNodeV1::new(output, vec![input]).unwrap();
    MultiInputLinkPlanV1::canonicalized(
        plan_target,
        vec![LinkInputV1::new(input, plan_target)],
        options,
        LinkOutputV1::new(output, plan_target),
        vec![input_node, output_node],
    )
    .unwrap()
}

fn exact_plan(
    prepared: &fe2o3_pliron_worker_v2::PreparedScalarAddWorkerV2,
) -> MultiInputLinkPlanV1 {
    plan(prepared.assembly_content_identity(), target(), "6", "2")
}

fn exact_kinds(plan: &MultiInputLinkPlanV1) -> LinkInputKindClosureV1 {
    LinkInputKindClosureV1::new(plan, vec![WorkerInputKindV1::LlvmTextIr]).unwrap()
}

fn exact_output() -> WorkerOutputConstraintsV1 {
    WorkerOutputConstraintsV1::new(EXPECTED_HSACO.len() as u64).unwrap()
}

fn producer() -> ProducerIdentity {
    ProducerIdentity::from_codegen(
        "fe2o3_pliron_worker_v2_fixture",
        Some(Path::new("src/scalar_add.rs")),
    )
    .unwrap()
}

fn consumed_bytes(
    directory: &TestDirectory,
    bytes: &[u8],
    seed: u8,
) -> ConsumedCompilerModuleHandoffV1 {
    let producer = producer();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([seed; 32]),
        BuildSession::from_bytes([seed.wrapping_add(1); 16]),
    )
    .unwrap();
    publish_compiler_module_handoff_v1(&directory.0, &producer, attempt, bytes).unwrap();
    consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap()
}

#[test]
fn exact_scalar_handoff_reaches_one_fully_bound_inert_request() {
    let source = exact_handoff(0x41);
    let source_identity = source.identity();
    let prepared = prepare_scalar_add_worker_v2(source).unwrap();
    let plan = exact_plan(&prepared);
    let kinds = exact_kinds(&plan);
    let directory = TestDirectory::new();
    let consumed = consumed_bytes(
        &directory,
        prepared.compiler_handoff().canonical_bytes(),
        0x51,
    );
    let attempt = consumed.attempt();
    let worker = pinned_worker("measured-scalar-worker-v1", "measured-upstream-llvm-v1");
    let evidence = construct_scalar_add_worker_request_v2(
        prepared,
        &plan,
        &worker,
        consumed,
        &kinds,
        exact_output(),
    )
    .unwrap();

    assert_eq!(evidence.source_identity(), source_identity);
    assert_eq!(evidence.attempt(), attempt);
    assert_eq!(evidence.plan_identity(), plan.identity());
    assert_ne!(evidence.request_id(), &[0; 32]);
    assert_ne!(evidence.request_identity(), &[0; 32]);
    assert_eq!(
        evidence.transaction_handoff_identity(),
        evidence.compiler_handoff_request().handoff_identity()
    );
    assert_eq!(
        evidence.manifest_identity(),
        evidence.compiler_handoff_request().manifest_identity()
    );
    let request = evidence.sealed_request();
    assert_eq!(request.target(), target());
    assert_eq!(request.code_object_version(), CodeObjectVersion::V6);
    assert_eq!(
        request.compiler_module().kind(),
        WorkerInputKindV1::LlvmTextIr
    );
    assert_eq!(
        request.compiler_module().identity(),
        ContentIdentityV1::from_parts(
            *evidence.assembly_sha256().as_bytes(),
            evidence.assembly_len()
        )
    );
    assert!(
        request
            .compiler_module()
            .identity()
            .matches(request.compiler_module().bytes())
    );
    assert!(request.external_providers().is_empty());
    assert!(request.import_symbols().is_empty());
    assert!(request.export_symbols().is_empty());
    assert_eq!(
        request.final_symbols(),
        [
            SCALAR_ADD_KERNEL_SYMBOL_V1.to_owned(),
            SCALAR_ADD_KERNEL_DESCRIPTOR_SYMBOL_V1.to_owned(),
        ]
    );
    assert_eq!(
        request.worker_executable(),
        worker.measurement().executable()
    );
    assert_eq!(
        request.worker_build_identity(),
        worker.measurement().worker_build_identity()
    );
    assert_eq!(
        request.llvm_build_identity(),
        worker.measurement().llvm_build_identity()
    );
    assert_eq!(evidence.worker_measurement(), worker.measurement());
    assert!(!evidence.grants_worker_authority());
    assert!(!evidence.grants_link_authority());
    assert!(!evidence.grants_publication_authority());
    assert!(!evidence.grants_load_authority());
    assert!(!evidence.grants_launch_authority());
}

#[test]
fn preparation_is_deterministic_and_evidence_changes_rebind_the_llvm_bytes() {
    let first = prepare_scalar_add_worker_v2(exact_handoff(0x41)).unwrap();
    let repeat = prepare_scalar_add_worker_v2(exact_handoff(0x41)).unwrap();
    assert_eq!(first.source_identity(), repeat.source_identity());
    assert_eq!(first.assembly_sha256(), repeat.assembly_sha256());
    assert_eq!(first.assembly_len(), repeat.assembly_len());
    assert_eq!(
        first.compiler_handoff().canonical_bytes(),
        repeat.compiler_handoff().canonical_bytes()
    );
    assert_eq!(
        first.compiler_handoff_identity(),
        repeat.compiler_handoff_identity()
    );
    assert_eq!(first.manifest_identity(), repeat.manifest_identity());

    let changed_evidence = prepare_scalar_add_worker_v2(exact_handoff(0x61)).unwrap();
    assert_ne!(first.source_identity(), changed_evidence.source_identity());
    assert_ne!(first.assembly_sha256(), changed_evidence.assembly_sha256());
    assert_ne!(
        first.compiler_handoff().module_bytes(),
        changed_evidence.compiler_handoff().module_bytes()
    );
    assert_ne!(
        first.compiler_handoff_identity(),
        changed_evidence.compiler_handoff_identity()
    );
    assert_eq!(
        first.manifest_identity(),
        changed_evidence.manifest_identity()
    );
}

#[test]
fn upstream_erased_module_names_are_stable_but_retained_names_are_exact() {
    let baseline = source_request(0x41);
    let mut renamed_module = baseline.clone();
    renamed_module.module_name = "renamed_source_module".to_owned();
    assert_eq!(
        lower_scalar_kernel_v2(&baseline).unwrap(),
        lower_scalar_kernel_v2(&renamed_module).unwrap()
    );

    let mut renamed_parameter = baseline.clone();
    renamed_parameter.input_parameter = "renamed_input".to_owned();
    assert_eq!(
        prepare_scalar_add_worker_v2(lower_scalar_kernel_v2(&renamed_parameter).unwrap())
            .unwrap_err(),
        PrepareScalarAddWorkerV2ErrorV1::Profile(ScalarAddProfileFieldV1::KernelAbi)
    );

    let mut renamed_kernel = baseline;
    renamed_kernel.kernel_symbol = "renamed_scalar_add".to_owned();
    assert_eq!(
        prepare_scalar_add_worker_v2(lower_scalar_kernel_v2(&renamed_kernel).unwrap()).unwrap_err(),
        PrepareScalarAddWorkerV2ErrorV1::Profile(ScalarAddProfileFieldV1::KernelInventory)
    );
}

#[test]
fn helper_global_intrinsic_and_provider_closures_reject() {
    let exact = exact_handoff(0x41);
    let base = exact.base().clone();
    let function = exact.module().functions()[0].clone();
    let evidence = function.evidence().clone();

    let helper = FunctionV2::new(
        FunctionIdV2::new(1),
        "helper",
        FunctionKindV2::Helper,
        CallingConventionV2::C,
        fe2o3_llvm_handoff::ReturnTypeV2::Void,
        vec![],
        vec![FunctionAttributeV2::NoUnwind],
        fe2o3_llvm_handoff::BlockIdV2::new(0),
        vec![BasicBlockV2::new(
            fe2o3_llvm_handoff::BlockIdV2::new(0),
            vec![],
            TerminatorV2::Return(None),
        )],
        evidence.clone(),
    )
    .unwrap();
    let helper_module = ExecutableModuleV2::new(
        base.module().flags().to_vec(),
        base.module().named_metadata().to_vec(),
        vec![],
        vec![],
        vec![function.clone(), helper],
    )
    .unwrap();
    assert_profile_error(
        Gfx942HandoffV2::new(base.clone(), helper_module).unwrap(),
        ScalarAddProfileFieldV1::CompilerClosure,
    );

    let global = GlobalV2::new(
        GlobalIdV2::new(0),
        "constant",
        GlobalLinkageV2::Internal,
        fe2o3_llvm_handoff::AddressSpaceV1::Constant,
        false,
        ScalarTypeV1::F32,
        Some(ScalarConstantV2::new(ScalarTypeV1::F32, 0).unwrap()),
        evidence.clone(),
    )
    .unwrap();
    let global_module = ExecutableModuleV2::new(
        base.module().flags().to_vec(),
        base.module().named_metadata().to_vec(),
        vec![global],
        vec![],
        vec![function.clone()],
    )
    .unwrap();
    assert_profile_error(
        Gfx942HandoffV2::new(base.clone(), global_module).unwrap(),
        ScalarAddProfileFieldV1::CompilerClosure,
    );

    let intrinsic_module = ExecutableModuleV2::new(
        base.module().flags().to_vec(),
        base.module().named_metadata().to_vec(),
        vec![],
        vec![IntrinsicReferenceV2::new(
            IntrinsicV2::AmdGpuBarrier,
            evidence,
        )],
        vec![function.clone()],
    )
    .unwrap();
    assert_profile_error(
        Gfx942HandoffV2::new(base.clone(), intrinsic_module).unwrap(),
        ScalarAddProfileFieldV1::CompilerClosure,
    );

    let provider = DeviceLibraryInputV1::new(DeviceLibraryKindV1::Ocml, [0x91; 32], 1).unwrap();
    let provider_metadata = ModuleMetadataV1::new(
        base.module().flags().to_vec(),
        base.module().named_metadata().to_vec(),
        vec![provider],
    )
    .unwrap();
    let provider_base = Gfx942HandoffV1::new(Gfx942HandoffInputV1 {
        stage_identities: *base.stage_identities(),
        target: base.target().clone(),
        kernels: base.kernels().to_vec(),
        module: provider_metadata,
        origins: base.origins().to_vec(),
        obligations: base.obligations().to_vec(),
    })
    .unwrap();
    let provider_module = ExecutableModuleV2::new(
        provider_base.module().flags().to_vec(),
        provider_base.module().named_metadata().to_vec(),
        vec![],
        vec![],
        vec![function],
    )
    .unwrap();
    assert_profile_error(
        Gfx942HandoffV2::new(provider_base, provider_module).unwrap(),
        ScalarAddProfileFieldV1::ProviderClosure,
    );
}

#[test]
fn body_and_attribute_substitutions_reject() {
    let exact = exact_handoff(0x41);
    let base = exact.base().clone();
    let function = &exact.module().functions()[0];
    let block = &function.blocks()[0];
    let [load, add, store] = block.instructions() else {
        panic!("exact source fixture lost its scalar body")
    };
    let changed_add = InstructionV2::new(
        add.result(),
        InstructionKindV2::Binary {
            operation: BinaryOperationV2::Float(FloatBinaryOperationV2::Subtract),
            left: fe2o3_llvm_handoff::ValueIdV2::new(3),
            right: fe2o3_llvm_handoff::ValueIdV2::new(2),
        },
        add.evidence().clone(),
    )
    .unwrap();
    let changed_body = FunctionV2::new(
        function.id(),
        function.symbol(),
        function.kind(),
        function.calling_convention(),
        function.return_type(),
        function.parameters().to_vec(),
        function.attributes().to_vec(),
        function.entry(),
        vec![BasicBlockV2::new(
            block.id(),
            vec![load.clone(), changed_add, store.clone()],
            block.terminator().clone(),
        )],
        function.evidence().clone(),
    )
    .unwrap();
    let body_module = ExecutableModuleV2::new(
        base.module().flags().to_vec(),
        base.module().named_metadata().to_vec(),
        vec![],
        vec![],
        vec![changed_body],
    )
    .unwrap();
    assert_profile_error(
        Gfx942HandoffV2::new(base.clone(), body_module).unwrap(),
        ScalarAddProfileFieldV1::KernelBody,
    );

    let mut v1_attributes = base.kernels()[0].function_attributes().to_vec();
    v1_attributes.push(FunctionAttributeV1::WavesPerEu(
        WavesPerEuV1::new(1, 2).unwrap(),
    ));
    let changed_kernel = KernelEntryV1::new(
        SCALAR_ADD_KERNEL_SYMBOL_V1,
        base.kernels()[0].parameters().to_vec(),
        v1_attributes.clone(),
        base.kernels()[0].origin(),
    )
    .unwrap();
    let changed_base = Gfx942HandoffV1::new(Gfx942HandoffInputV1 {
        stage_identities: *base.stage_identities(),
        target: base.target().clone(),
        kernels: vec![changed_kernel],
        module: base.module().clone(),
        origins: base.origins().to_vec(),
        obligations: base.obligations().to_vec(),
    })
    .unwrap();
    let changed_function = FunctionV2::new(
        function.id(),
        function.symbol(),
        function.kind(),
        function.calling_convention(),
        function.return_type(),
        function.parameters().to_vec(),
        v1_attributes
            .into_iter()
            .map(FunctionAttributeV2::from)
            .collect(),
        function.entry(),
        function.blocks().to_vec(),
        function.evidence().clone(),
    )
    .unwrap();
    let changed_module = ExecutableModuleV2::new(
        changed_base.module().flags().to_vec(),
        changed_base.module().named_metadata().to_vec(),
        vec![],
        vec![],
        vec![changed_function],
    )
    .unwrap();
    assert_profile_error(
        Gfx942HandoffV2::new(changed_base, changed_module).unwrap(),
        ScalarAddProfileFieldV1::KernelAttributes,
    );
}

#[test]
fn compiler_handoff_substitutions_reject_after_public_consumption() {
    let prepared = prepare_scalar_add_worker_v2(exact_handoff(0x41)).unwrap();
    let plan = exact_plan(&prepared);
    let kinds = exact_kinds(&plan);
    let worker = pinned_worker("measured-worker", "measured-llvm");

    let mut changed_module = prepared.compiler_handoff().module_bytes().to_vec();
    changed_module.push(b'\n');
    let byte_substitution = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target(),
        CodeObjectVersion::V6,
        prepared.compiler_handoff().envelope().clone(),
        prepared.compiler_handoff().symbol_manifest().clone(),
        &changed_module,
    )
    .unwrap();
    let directory = TestDirectory::new();
    let consumed = consumed_bytes(&directory, byte_substitution.canonical_bytes(), 0x71);
    assert_eq!(
        construct_scalar_add_worker_request_v2(
            prepared,
            &plan,
            &worker,
            consumed,
            &kinds,
            exact_output(),
        )
        .unwrap_err(),
        ConstructScalarAddWorkerRequestV2ErrorV1::Binding(
            ScalarAddWorkerRequestFieldV1::ConsumedHandoff
        )
    );

    let prepared = prepare_scalar_add_worker_v2(exact_handoff(0x41)).unwrap();
    let plan = exact_plan(&prepared);
    let kinds = exact_kinds(&plan);
    let changed_manifest = CompilerModuleSymbolManifestV1::new([
        (
            CompilerModuleSymbolRoleV1::KernelEntry,
            SCALAR_ADD_KERNEL_SYMBOL_V1,
        ),
        (
            CompilerModuleSymbolRoleV1::KernelDescriptor,
            SCALAR_ADD_KERNEL_DESCRIPTOR_SYMBOL_V1,
        ),
        (CompilerModuleSymbolRoleV1::InternalHelper, "helper"),
    ])
    .unwrap();
    let manifest_substitution = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target(),
        CodeObjectVersion::V6,
        prepared.compiler_handoff().envelope().clone(),
        changed_manifest,
        prepared.compiler_handoff().module_bytes(),
    )
    .unwrap();
    let directory = TestDirectory::new();
    let consumed = consumed_bytes(&directory, manifest_substitution.canonical_bytes(), 0x72);
    assert_eq!(
        construct_scalar_add_worker_request_v2(
            prepared,
            &plan,
            &worker,
            consumed,
            &kinds,
            exact_output(),
        )
        .unwrap_err(),
        ConstructScalarAddWorkerRequestV2ErrorV1::Binding(
            ScalarAddWorkerRequestFieldV1::ConsumedHandoff
        )
    );

    let prepared = prepare_scalar_add_worker_v2(exact_handoff(0x41)).unwrap();
    let plan = exact_plan(&prepared);
    let kinds = exact_kinds(&plan);
    let substituted_target = DeviceTargetV1::parse("gfx942:xnack+").unwrap();
    let target_substitution = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        substituted_target,
        CodeObjectVersion::V6,
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(
            substituted_target,
            CodeObjectVersion::V6,
        )
        .unwrap(),
        prepared.compiler_handoff().symbol_manifest().clone(),
        prepared.compiler_handoff().module_bytes(),
    )
    .unwrap();
    let directory = TestDirectory::new();
    let consumed = consumed_bytes(&directory, target_substitution.canonical_bytes(), 0x73);
    assert_eq!(
        construct_scalar_add_worker_request_v2(
            prepared,
            &plan,
            &worker,
            consumed,
            &kinds,
            exact_output(),
        )
        .unwrap_err(),
        ConstructScalarAddWorkerRequestV2ErrorV1::Binding(
            ScalarAddWorkerRequestFieldV1::ConsumedHandoff
        )
    );

    let prepared = prepare_scalar_add_worker_v2(exact_handoff(0x41)).unwrap();
    let plan = exact_plan(&prepared);
    let kinds = exact_kinds(&plan);
    let cov_substitution = CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target(),
        CodeObjectVersion::V5,
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target(), CodeObjectVersion::V5)
            .unwrap(),
        prepared.compiler_handoff().symbol_manifest().clone(),
        prepared.compiler_handoff().module_bytes(),
    )
    .unwrap();
    let directory = TestDirectory::new();
    let consumed = consumed_bytes(&directory, cov_substitution.canonical_bytes(), 0x74);
    assert_eq!(
        construct_scalar_add_worker_request_v2(
            prepared,
            &plan,
            &worker,
            consumed,
            &kinds,
            exact_output(),
        )
        .unwrap_err(),
        ConstructScalarAddWorkerRequestV2ErrorV1::Binding(
            ScalarAddWorkerRequestFieldV1::ConsumedHandoff
        )
    );
}

#[test]
fn target_cov_plan_input_kind_and_output_substitutions_reject() {
    let worker = pinned_worker("measured-worker", "measured-llvm");

    assert_request_binding_error(
        plan(
            prepare_scalar_add_worker_v2(exact_handoff(0x41))
                .unwrap()
                .assembly_content_identity(),
            DeviceTargetV1::parse("gfx942:xnack+").unwrap(),
            "6",
            "2",
        ),
        None,
        exact_output(),
        &worker,
        ScalarAddWorkerRequestFieldV1::PlanTarget,
        0x81,
    );

    let prepared = prepare_scalar_add_worker_v2(exact_handoff(0x41)).unwrap();
    let cov_plan = plan(prepared.assembly_content_identity(), target(), "5", "2");
    assert_request_binding_error_with_prepared(
        prepared,
        cov_plan,
        None,
        exact_output(),
        &worker,
        ScalarAddWorkerRequestFieldV1::PlanOptions,
        0x82,
    );

    let prepared = prepare_scalar_add_worker_v2(exact_handoff(0x41)).unwrap();
    let wrong_input = ContentIdentityV1::calculate(b"substituted LLVM module");
    let wrong_plan = plan(wrong_input, target(), "6", "2");
    assert_request_binding_error_with_prepared(
        prepared,
        wrong_plan,
        None,
        exact_output(),
        &worker,
        ScalarAddWorkerRequestFieldV1::PlanInputs,
        0x83,
    );

    let prepared = prepare_scalar_add_worker_v2(exact_handoff(0x41)).unwrap();
    let plan = exact_plan(&prepared);
    let wrong_kinds =
        LinkInputKindClosureV1::new(&plan, vec![WorkerInputKindV1::LlvmBitcode]).unwrap();
    assert_request_binding_error_with_prepared(
        prepared,
        plan,
        Some(wrong_kinds),
        exact_output(),
        &worker,
        ScalarAddWorkerRequestFieldV1::InputKinds,
        0x84,
    );

    let prepared = prepare_scalar_add_worker_v2(exact_handoff(0x41)).unwrap();
    let plan = exact_plan(&prepared);
    assert_request_binding_error_with_prepared(
        prepared,
        plan,
        None,
        WorkerOutputConstraintsV1::new(EXPECTED_HSACO.len() as u64 + 1).unwrap(),
        &worker,
        ScalarAddWorkerRequestFieldV1::Output,
        0x85,
    );
}

#[test]
fn exact_worker_options_reject_omission_false_and_unknown_entries() {
    let worker = pinned_worker("measured-worker", "measured-llvm");
    let hostile_options = [
        vec![
            LinkOptionV1::new("code-object-version", "6").unwrap(),
            LinkOptionV1::new("opt-level", "2").unwrap(),
            LinkOptionV1::new("verify-each", "true").unwrap(),
        ],
        vec![
            LinkOptionV1::new("code-object-version", "6").unwrap(),
            LinkOptionV1::new("opt-level", "2").unwrap(),
            LinkOptionV1::new("strip-debug", "true").unwrap(),
        ],
        vec![
            LinkOptionV1::new("code-object-version", "6").unwrap(),
            LinkOptionV1::new("opt-level", "2").unwrap(),
            LinkOptionV1::new("strip-debug", "false").unwrap(),
            LinkOptionV1::new("verify-each", "true").unwrap(),
        ],
        vec![
            LinkOptionV1::new("code-object-version", "6").unwrap(),
            LinkOptionV1::new("opt-level", "2").unwrap(),
            LinkOptionV1::new("strip-debug", "true").unwrap(),
            LinkOptionV1::new("verify-each", "false").unwrap(),
        ],
        vec![
            LinkOptionV1::new("code-object-version", "6").unwrap(),
            LinkOptionV1::new("opt-level", "2").unwrap(),
            LinkOptionV1::new("scalar-add-extra", "true").unwrap(),
            LinkOptionV1::new("strip-debug", "true").unwrap(),
            LinkOptionV1::new("verify-each", "true").unwrap(),
        ],
    ];

    for (index, options) in hostile_options.into_iter().enumerate() {
        let prepared = prepare_scalar_add_worker_v2(exact_handoff(0x41)).unwrap();
        let hostile_plan =
            plan_with_options(prepared.assembly_content_identity(), target(), options);
        assert_request_binding_error_with_prepared(
            prepared,
            hostile_plan,
            None,
            exact_output(),
            &worker,
            ScalarAddWorkerRequestFieldV1::PlanOptions,
            0xa0 + u8::try_from(index).unwrap(),
        );
    }
}

#[test]
fn exact_worker_options_follow_link_plan_canonicalization() {
    let prepared = prepare_scalar_add_worker_v2(exact_handoff(0x41)).unwrap();
    let input = prepared.assembly_content_identity();
    let reversed_options = vec![
        LinkOptionV1::new("verify-each", "true").unwrap(),
        LinkOptionV1::new("strip-debug", "true").unwrap(),
        LinkOptionV1::new("opt-level", "2").unwrap(),
        LinkOptionV1::new("code-object-version", "6").unwrap(),
    ];
    let canonicalized = plan_with_options(input, target(), reversed_options.clone());
    assert!(canonicalized.options().iter().map(LinkOptionV1::name).eq([
        "code-object-version",
        "opt-level",
        "strip-debug",
        "verify-each",
    ]));

    let output = ContentIdentityV1::calculate(EXPECTED_HSACO);
    assert_eq!(
        MultiInputLinkPlanV1::new(
            target(),
            vec![LinkInputV1::new(input, target())],
            reversed_options,
            LinkOutputV1::new(output, target()),
            vec![
                ProvenanceNodeV1::new(input, vec![]).unwrap(),
                ProvenanceNodeV1::new(output, vec![input]).unwrap(),
            ],
        ),
        Err(LinkPlanError::NonCanonicalOrder("link options"))
    );

    let kinds = exact_kinds(&canonicalized);
    let directory = TestDirectory::new();
    let consumed = consumed_bytes(
        &directory,
        prepared.compiler_handoff().canonical_bytes(),
        0xa6,
    );
    let worker = pinned_worker("measured-worker", "measured-llvm");
    let evidence = construct_scalar_add_worker_request_v2(
        prepared,
        &canonicalized,
        &worker,
        consumed,
        &kinds,
        exact_output(),
    )
    .unwrap();
    assert_eq!(
        evidence.sealed_request().options(),
        fe2o3_hsaco_finalize::WorkerOptionsV1::new(
            fe2o3_hsaco_finalize::WorkerOptimizationLevelV1::O2,
            true,
            true,
        )
    );
}

#[test]
fn actual_worker_measurement_is_bound_and_substitution_rejects_before_spawn() {
    assert_ne!(EXACT_LLVM_BUILD_IDENTITY_V1, "actual-test-llvm-build");
    let prepared = prepare_scalar_add_worker_v2(exact_handoff(0x41)).unwrap();
    let plan = exact_plan(&prepared);
    let kinds = exact_kinds(&plan);
    let directory = TestDirectory::new();
    let consumed = consumed_bytes(
        &directory,
        prepared.compiler_handoff().canonical_bytes(),
        0x91,
    );
    let actual = pinned_worker("actual-test-worker-build", "actual-test-llvm-build");
    let substituted = pinned_worker("substituted-worker-build", "actual-test-llvm-build");
    let substituted_llvm = pinned_worker("actual-test-worker-build", "substituted-llvm-build");
    let substituted_executable = pinned_worker_at(
        Path::new("/usr/bin/false"),
        "actual-test-worker-build",
        "actual-test-llvm-build",
    );
    let evidence = construct_scalar_add_worker_request_v2(
        prepared,
        &plan,
        &actual,
        consumed,
        &kinds,
        exact_output(),
    )
    .unwrap();

    assert_eq!(
        evidence.sealed_request().worker_build_identity(),
        "actual-test-worker-build"
    );
    assert_eq!(
        evidence.sealed_request().llvm_build_identity(),
        "actual-test-llvm-build"
    );
    assert_ne!(
        evidence.sealed_request().llvm_build_identity(),
        EXACT_LLVM_BUILD_IDENTITY_V1
    );
    let error = substituted
        .execute_compiler_handoff_v2(
            evidence.compiler_handoff_request(),
            WorkerExecutionLimitsV1::default(),
        )
        .unwrap_err();
    assert_eq!(
        error.kind(),
        &WorkerExecutionErrorKind::WorkerBuildIdentityMismatch
    );
    let error = substituted_llvm
        .execute_compiler_handoff_v2(
            evidence.compiler_handoff_request(),
            WorkerExecutionLimitsV1::default(),
        )
        .unwrap_err();
    assert_eq!(
        error.kind(),
        &WorkerExecutionErrorKind::LlvmBuildIdentityMismatch
    );
    let error = substituted_executable
        .execute_compiler_handoff_v2(
            evidence.compiler_handoff_request(),
            WorkerExecutionLimitsV1::default(),
        )
        .unwrap_err();
    assert!(matches!(
        error.kind(),
        WorkerExecutionErrorKind::WorkerIdentityMismatch { .. }
    ));
}

#[test]
fn transaction_replay_rejects_and_all_prepared_surfaces_are_inert() {
    let prepared = prepare_scalar_add_worker_v2(exact_handoff(0x41)).unwrap();
    assert!(!prepared.authenticates_compiler_origin());
    assert!(!prepared.grants_compiler_authority());
    assert!(!prepared.grants_worker_authority());
    assert!(!prepared.grants_link_authority());
    assert!(!prepared.grants_publication_authority());
    assert!(!prepared.grants_load_authority());
    assert!(!prepared.grants_launch_authority());
    assert!(!prepared.compiler_handoff().authenticates_compiler_origin());
    assert!(!prepared.compiler_handoff().grants_compiler_authority());
    assert!(!prepared.compiler_handoff().grants_worker_authority());
    assert!(!prepared.compiler_handoff().grants_link_authority());
    assert!(!prepared.compiler_handoff().grants_load_authority());
    assert!(!prepared.compiler_handoff().grants_launch_authority());

    let directory = TestDirectory::new();
    let producer = producer();
    let attempt = begin_build_attempt(
        &directory.0,
        &producer,
        BuildInvocation::from_bytes([0xa1; 32]),
        BuildSession::from_bytes([0xa2; 16]),
    )
    .unwrap();
    publish_compiler_module_handoff_v1(
        &directory.0,
        &producer,
        attempt,
        prepared.compiler_handoff().canonical_bytes(),
    )
    .unwrap();
    let consumed = consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).unwrap();
    assert!(!consumed.grants_compiler_authority());
    assert!(!consumed.grants_link_authority());
    assert!(!consumed.grants_publication_authority());
    assert!(!consumed.grants_load_authority());
    assert!(!consumed.grants_launch_authority());
    assert!(consume_compiler_module_handoff_v1(&directory.0, &producer, attempt).is_err());
}

fn assert_profile_error(handoff: Gfx942HandoffV2, field: ScalarAddProfileFieldV1) {
    assert_eq!(
        prepare_scalar_add_worker_v2(handoff).unwrap_err(),
        PrepareScalarAddWorkerV2ErrorV1::Profile(field)
    );
}

fn assert_request_binding_error(
    plan: MultiInputLinkPlanV1,
    kinds: Option<LinkInputKindClosureV1>,
    output: WorkerOutputConstraintsV1,
    worker: &PinnedWorkerV1,
    field: ScalarAddWorkerRequestFieldV1,
    seed: u8,
) {
    assert_request_binding_error_with_prepared(
        prepare_scalar_add_worker_v2(exact_handoff(0x41)).unwrap(),
        plan,
        kinds,
        output,
        worker,
        field,
        seed,
    );
}

fn assert_request_binding_error_with_prepared(
    prepared: fe2o3_pliron_worker_v2::PreparedScalarAddWorkerV2,
    plan: MultiInputLinkPlanV1,
    kinds: Option<LinkInputKindClosureV1>,
    output: WorkerOutputConstraintsV1,
    worker: &PinnedWorkerV1,
    field: ScalarAddWorkerRequestFieldV1,
    seed: u8,
) {
    let directory = TestDirectory::new();
    let consumed = consumed_bytes(
        &directory,
        prepared.compiler_handoff().canonical_bytes(),
        seed,
    );
    let kinds = kinds.unwrap_or_else(|| exact_kinds(&plan));
    assert_eq!(
        construct_scalar_add_worker_request_v2(prepared, &plan, worker, consumed, &kinds, output,)
            .unwrap_err(),
        ConstructScalarAddWorkerRequestV2ErrorV1::Binding(field)
    );
}
