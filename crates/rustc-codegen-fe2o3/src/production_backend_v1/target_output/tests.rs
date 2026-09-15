//! Real backend/text/FFI constructors, inert descriptors. These tests do not
//! manufacture formal owners or claim worker execution / machine refinement.
use super::*;
use fe2o3_compiler_ffi::{CodeObjectVersion, CompilerFfiEnvelopeV1};
use fe2o3_kernel_ir::*;

mod phase {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-amdgcn-model/src/lowering/v13/reusable_phase_tests/fixture.rs"
    ));
}

fn return_only() -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("target-output");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    let mut kernel = Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    module
}

fn target(cpu: &str) -> ProductionBackendTargetV1 {
    ProductionBackendTargetV1::from_live_cpu(cpu).unwrap()
}

fn lower(
    module: &Module,
    version: CanonicalKernelIrVersionV1,
    cpu: &str,
) -> ProductionBackendLoweredModuleV1 {
    let owner = VerifiedCanonicalKernelIrV1::from_module(module.clone(), version).unwrap();
    let launch =
        dialect_amdgcn::ProductionTargetLaunchEvidenceKirV1::for_static_launches(&owner, 7)
            .unwrap();
    let backend = target(cpu);
    let closure = dialect_amdgcn::legalize_production_target_capabilities_kir_v1(
        &owner,
        7,
        &launch,
        backend.inner.profile,
    )
    .unwrap();
    backend
        .lower_declared_module_v1(&owner, 7, &closure)
        .unwrap()
}

fn descriptor(module: &Module, cpu: &str, marker: u8) -> CompilerDescriptorSourceV1 {
    use fe2o3_kernel_descriptor::*;
    let evidence = BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([marker; 32]),
        EvidenceDigest::from_sha256_bytes([marker.wrapping_add(1); 32]),
    );
    let kernels = module
        .kernels
        .iter()
        .map(|kernel| {
            KernelDescriptorV1::new(
                KernelId::from_bytes([marker; 32]),
                ValidName::new(kernel.id.as_str()).unwrap(),
                ValidName::new(kernel.id.as_str()).unwrap(),
                ValidName::new(format!("{}.kd", kernel.id.as_str())).unwrap(),
                evidence,
                evidence,
                vec![],
                KernelAbiLayoutV1::new(0, 0, 8).unwrap(),
                LaunchConstraintsV1::new(
                    1,
                    BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
                    DimensionsV1::new(64, 1, 1).unwrap(),
                    64,
                    256,
                    0,
                )
                .unwrap(),
                vec![],
            )
            .unwrap()
        })
        .collect();
    CompilerDescriptorSourceV1::new(
        DeviceDescriptorTableV1::new(
            CanonicalCodeObjectDigest::from_bytes([0; 32]),
            CodeObjectVersion::V6,
            CompilerIdentityV1::new(
                Text::new("inert-output-test").unwrap(),
                Text::new("1").unwrap(),
                [0; 20],
            ),
            ProducerIdentityV1::new(
                Text::new("inert-output-test").unwrap(),
                Text::new("1").unwrap(),
            ),
            target(cpu).device_target(),
            vec![],
            vec![],
            kernels,
        )
        .unwrap(),
    )
    .unwrap()
}

fn handoff(module: &InertCompilerModuleTextV1, cpu: &str, text: &str) -> CompilerModuleHandoffV2 {
    let target = target(cpu).device_target();
    CompilerModuleHandoffV2::new(
        CompilerModuleKindV1::LlvmTextIr,
        target,
        CodeObjectVersion::V6,
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(target, CodeObjectVersion::V6)
            .unwrap(),
        crate::compiler_module_contract::construct_symbol_manifest(module).unwrap(),
        text.as_bytes(),
    )
    .unwrap()
}

fn prepared(
    module: &Module,
    version: CanonicalKernelIrVersionV1,
    cpu: &str,
) -> (
    ProductionBackendDescriptorModuleV1,
    CompilerDescriptorSourceV1,
) {
    let backend = target(cpu);
    let output = lower(module, version, cpu)
        .bind_worker_layout_v1(&backend)
        .unwrap()
        .retain_compiler_module(backend.device_target(), module)
        .unwrap();
    let source = descriptor(module, cpu, 1);
    (output.bind_descriptor(&source).unwrap(), source)
}

fn exact_error(result: Result<(), Error>, expected: &str) {
    assert!(
        matches!(&result, Err(Error::TargetOutput(actual)) if *actual == expected),
        "{result:?}"
    );
}

#[test]
fn target_output_v13_preserves_old_lowering_layout_descriptor_and_handoff_bytes() {
    for cpu in ["gfx942", "gfx950"] {
        let module = return_only();
        let backend = target(cpu);
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
        let closure = backend
            .close_semantic_capabilities_v1(&canonical, 7)
            .unwrap();
        let old = dialect_amdgcn::lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(
            &canonical,
            7,
            closure.inner.closure.launch_evidence(),
            backend.inner.profile,
        )
        .unwrap();
        let output = backend
            .lower_v13_module_v1(&canonical, 7, &closure)
            .unwrap();
        assert_eq!(old.llvm_ir(), output.llvm_ir());
        assert_eq!(
            old.structured_derivation(),
            output.lowered.structured_derivation()
        );
        assert_eq!(
            old.unsupported_operational_translation(),
            output.lowered.unsupported_operational_translation()
        );
        assert!(
            !output
                .lowered
                .has_complete_operational_translation_derivation()
        );
        let bound = output.bind_worker_layout_v1(&backend).unwrap();
        assert_eq!(
            bound.llvm_ir(),
            backend.bind_worker_layout_v1(old.llvm_ir()).unwrap()
        );
        let old =
            retain_production_compiler_module_text_v1(&module, bound.llvm_ir().to_owned()).unwrap();
        let source = descriptor(&module, cpu, 1);
        let old = bind_compiler_descriptor_source_v1(old, &source).unwrap();
        let exact = bound
            .retain_compiler_module(backend.device_target(), &module)
            .unwrap()
            .bind_descriptor(&source)
            .unwrap();
        assert_eq!(old, *exact.compiler_module());
        let original = handoff(&old, cpu, old.llvm_ir());
        let current = handoff(
            exact.compiler_module(),
            cpu,
            exact.compiler_module().llvm_ir(),
        );
        assert_eq!(original.canonical_bytes(), current.canonical_bytes());
        let output = exact.bind_worker_input(&current, &source).unwrap();
        output.validate_worker_input(&current, &source).unwrap();
        output.require_frozen_v13_artifact().unwrap();
    }
}

#[test]
fn target_output_v14_two_phase_memory_retains_complete_projection_and_writer() {
    for cpu in ["gfx942", "gfx950"] {
        let module = phase::terminal_drops(phase::memory(2));
        let (exact, source) = prepared(&module, CanonicalKernelIrVersionV1::V14, cpu);
        let replay = exact.backend.lowered.declared_replay().unwrap();
        assert_eq!(replay.version(), CanonicalKernelIrVersionV1::V14);
        assert!(replay.structured().is_some());
        assert_eq!(
            exact
                .compiler_module()
                .llvm_ir()
                .matches("call void asm sideeffect \"s_barrier\"")
                .count(),
            4
        );
        assert_eq!(
            exact
                .compiler_module()
                .llvm_ir()
                .matches("store float")
                .count(),
            2
        );
        assert_eq!(
            exact
                .compiler_module()
                .llvm_ir()
                .matches("internal addrspace(3) global")
                .count(),
            1
        );
        let input = handoff(
            exact.compiler_module(),
            cpu,
            exact.compiler_module().llvm_ir(),
        );
        let output = exact.bind_worker_input(&input, &source).unwrap();
        output.validate_worker_input(&input, &source).unwrap();
        exact_error(
            output.require_frozen_v13_artifact(),
            "V14 artifact lineage and machine-proof consumption are not implemented",
        );
    }
}

#[test]
fn target_output_rejects_layout_target_substitution() {
    let output = lower(&return_only(), CanonicalKernelIrVersionV1::V13, "gfx942");
    exact_error(
        output.bind_worker_layout_v1(&target("gfx950")).map(|_| ()),
        "worker layout target changed",
    );
}

#[test]
fn target_output_rejects_retained_module_substitution() {
    let module = return_only();
    let output = lower(&module, CanonicalKernelIrVersionV1::V13, "gfx942");
    let mut changed = module.clone();
    changed.id = ModuleId::new("substituted");
    exact_error(
        output.validate_module(&changed),
        "retained module is not the lowered declared graph",
    );
}

#[test]
fn target_output_rejects_other_target_closure() {
    let owner =
        VerifiedCanonicalKernelIrV1::from_module(return_only(), CanonicalKernelIrVersionV1::V14)
            .unwrap();
    let launch =
        dialect_amdgcn::ProductionTargetLaunchEvidenceKirV1::for_static_launches(&owner, 7)
            .unwrap();
    let other = dialect_amdgcn::legalize_production_target_capabilities_kir_v1(
        &owner,
        7,
        &launch,
        target("gfx950").inner.profile,
    )
    .unwrap();
    let result = target("gfx942").lower_declared_module_v1(&owner, 7, &other);
    assert!(
        matches!(result, Err(Error::CapabilityClosureChanged)),
        "{result:?}"
    );
}

#[test]
fn target_output_rejects_changed_epoch_before_lowering() {
    let owner =
        VerifiedCanonicalKernelIrV1::from_module(return_only(), CanonicalKernelIrVersionV1::V14)
            .unwrap();
    let launch =
        dialect_amdgcn::ProductionTargetLaunchEvidenceKirV1::for_static_launches(&owner, 7)
            .unwrap();
    let backend = target("gfx942");
    let closure = dialect_amdgcn::legalize_production_target_capabilities_kir_v1(
        &owner,
        7,
        &launch,
        backend.inner.profile,
    )
    .unwrap();
    assert!(matches!(
        backend.lower_declared_module_v1(&owner, 8, &closure),
        Err(Error::V13Lowering(
            dialect_amdgcn::ProductionV13AmdLoweringErrorV1::Capability(
                dialect_amdgcn::ProductionTargetCapabilityErrorV1::LaunchEvidenceSubjectMismatch,
            )
        ))
    ));
}

#[test]
fn target_output_rejects_changed_descriptor_target() {
    let module = return_only();
    let backend = target("gfx942");
    let output = lower(&module, CanonicalKernelIrVersionV1::V13, "gfx942")
        .bind_worker_layout_v1(&backend)
        .unwrap()
        .retain_compiler_module(backend.device_target(), &module)
        .unwrap();
    let result = output.bind_descriptor(&descriptor(&module, "gfx950", 1));
    assert!(matches!(
        result,
        Err(Error::TargetOutput("descriptor target changed"))
    ));
}

#[test]
fn target_output_rejects_changed_descriptor_bytes_after_binding() {
    let module = return_only();
    let (exact, _) = prepared(&module, CanonicalKernelIrVersionV1::V13, "gfx942");
    let input = handoff(
        exact.compiler_module(),
        "gfx942",
        exact.compiler_module().llvm_ir(),
    );
    exact_error(
        exact
            .bind_worker_input(&input, &descriptor(&module, "gfx942", 3))
            .map(|_| ()),
        "worker input, layout, target or descriptor changed",
    );
}

#[test]
fn target_output_rejects_reencoded_worker_text_substitution() {
    let module = phase::memory(2);
    let (exact, source) = prepared(&module, CanonicalKernelIrVersionV1::V14, "gfx950");
    let text = exact
        .compiler_module()
        .llvm_ir()
        .replace("s_barrier", "s_barriex");
    assert_ne!(text, exact.compiler_module().llvm_ir());
    let changed = handoff(exact.compiler_module(), "gfx950", &text);
    exact_error(
        exact.bind_worker_input(&changed, &source).map(|_| ()),
        "worker input, layout, target or descriptor changed",
    );
}

#[test]
fn target_output_rejects_reencoded_worker_target_substitution() {
    let (exact, source) = prepared(&return_only(), CanonicalKernelIrVersionV1::V13, "gfx942");
    let changed = handoff(
        exact.compiler_module(),
        "gfx950",
        exact.compiler_module().llvm_ir(),
    );
    exact_error(
        exact.bind_worker_input(&changed, &source).map(|_| ()),
        "worker input, layout, target or descriptor changed",
    );
}

#[test]
fn target_output_rejects_replacement_after_complete_input_is_bound() {
    let (exact, source) = prepared(&return_only(), CanonicalKernelIrVersionV1::V13, "gfx942");
    let original = handoff(
        exact.compiler_module(),
        "gfx942",
        exact.compiler_module().llvm_ir(),
    );
    let changed = handoff(
        exact.compiler_module(),
        "gfx942",
        &format!("{}\n;changed", exact.compiler_module().llvm_ir()),
    );
    let output = exact.bind_worker_input(&original, &source).unwrap();
    exact_error(
        output.validate_worker_input(&changed, &source),
        "worker input, layout, target or descriptor changed",
    );
}

#[test]
fn target_output_v14_is_not_encodable_as_legacy_v13() {
    let module = phase::memory(2);
    assert!(VerifiedCanonicalKernelIrV13::from_module(module.clone()).is_err());
    let output = lower(&module, CanonicalKernelIrVersionV1::V14, "gfx942");
    output.validate_translation().unwrap();
    assert!(!output.lowered.grants_load_authority());
    assert!(!output.lowered.grants_launch_authority());
}

#[test]
fn target_output_rejects_relabelling_a_v14_output_as_v13() {
    let mut output = lower(&phase::memory(2), CanonicalKernelIrVersionV1::V14, "gfx942");
    output.canonical = *VerifiedCanonicalKernelIrV13::from_module(return_only())
        .unwrap()
        .as_common()
        .identity();
    exact_error(
        output.validate_translation(),
        "V13 structured output changed version",
    );
}

#[test]
fn target_output_rejects_same_profile_other_graph_launch_evidence() {
    let owner =
        VerifiedCanonicalKernelIrV1::from_module(return_only(), CanonicalKernelIrVersionV1::V14)
            .unwrap();
    let launch =
        dialect_amdgcn::ProductionTargetLaunchEvidenceKirV1::for_static_launches(&owner, 7)
            .unwrap();
    let backend = target("gfx942");
    let closure = dialect_amdgcn::legalize_production_target_capabilities_kir_v1(
        &owner,
        7,
        &launch,
        backend.inner.profile,
    )
    .unwrap();
    let mut changed = return_only();
    changed.id = ModuleId::new("same-profile-other-graph");
    let changed =
        VerifiedCanonicalKernelIrV1::from_module(changed, CanonicalKernelIrVersionV1::V14).unwrap();
    assert!(matches!(
        backend.lower_declared_module_v1(&changed, 7, &closure),
        Err(Error::V13Lowering(
            dialect_amdgcn::ProductionV13AmdLoweringErrorV1::Capability(
                dialect_amdgcn::ProductionTargetCapabilityErrorV1::LaunchEvidenceSubjectMismatch,
            )
        ))
    ));
}

#[test]
fn target_output_rejects_retained_module_target_change() {
    let module = return_only();
    let backend = target("gfx942");
    let bound = lower(&module, CanonicalKernelIrVersionV1::V13, "gfx942")
        .bind_worker_layout_v1(&backend)
        .unwrap();
    exact_error(
        bound
            .retain_compiler_module(target("gfx950").device_target(), &module)
            .map(|_| ()),
        "module target changed",
    );
}

#[test]
fn target_output_keeps_exact_descriptor_symbol_roster_rejection() {
    let module = return_only();
    let backend = target("gfx942");
    let exact = lower(&module, CanonicalKernelIrVersionV1::V13, "gfx942")
        .bind_worker_layout_v1(&backend)
        .unwrap()
        .retain_compiler_module(backend.device_target(), &module)
        .unwrap();
    let mut other = return_only();
    other.kernels[0].id = KernelId::new("other-entry");
    assert!(matches!(exact.bind_descriptor(&descriptor(&other, "gfx942", 1)),
        Err(Error::CompilerModule(crate::kernel_ir_codegen::CompilerModuleConstructionError::DescriptorKernelEntryClosureMismatch))));
}
