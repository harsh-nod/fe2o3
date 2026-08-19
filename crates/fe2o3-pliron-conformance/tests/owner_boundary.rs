#![forbid(unsafe_code)]

//! Independent hostile conformance tests for context-owned Pliron artifacts.

use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
};

use dialect_gpu::AddressSpaceAttr;
use dialect_kernel::AlgorithmOp;
use dialect_mir::{
    MirTypeId,
    pliron::{MirDialectLimits, MirModuleOp},
};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Module, Signature,
    Terminator,
};
use fe2o3_kir_pliron_bridge::{
    BridgeEnvelope, BridgeError, BridgeLimits, CanonicalKirRecord, KirVersion, recover_exact,
};
use fe2o3_lower_kernel_gpu as lower_kernel_gpu;
use fe2o3_lower_mir_kernel as lower_mir_kernel;
use fe2o3_pliron::{
    CONTEXT_IDENTITY_MARKER_KEY, ContextBuildError, Diagnostic, DiagnosticCode,
    DialectRegistration, PlironSession, RegistrationHookError, ShellLimits,
};
use pliron::{
    context::Context, dialect::DialectName, identifier::Identifier, op::Op, operation::Operation,
};

fn mir_source(context: &mut Context, identity: &str) -> MirModuleOp {
    let limits = MirDialectLimits::new(2, 2, 64).expect("bounded MIR limits");
    let module = MirModuleOp::try_new(context, identity, limits).expect("valid MIR module");
    module
        .append_function(context, format!("{identity}::entry"), &[MirTypeId(1)])
        .expect("valid MIR function");
    module
}

fn mir_config() -> lower_mir_kernel::LoweringConfig {
    lower_mir_kernel::LoweringConfig::new(
        lower_mir_kernel::LoweringLimits::new(1, 2, 2, 16, 2).expect("bounded MIR lowering limits"),
        1,
    )
    .expect("bounded MIR lowering configuration")
}

fn gpu_config() -> lower_kernel_gpu::LoweringConfig {
    lower_kernel_gpu::LoweringConfig::new(
        lower_kernel_gpu::WorkgroupShape::new(&[64]).expect("bounded workgroup"),
        1,
        &[AddressSpaceAttr::Global],
        lower_kernel_gpu::SynchronizationMode::None,
        4,
    )
    .expect("bounded GPU lowering configuration")
}

fn kir_module() -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("owner-boundary-kir");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn lower_mir(context: &mut Context, identity: &str) -> lower_mir_kernel::LoweringResult {
    let source = mir_source(context, identity);
    let mut service = lower_mir_kernel::MirKernelLoweringPass::new(mir_config());
    service
        .run_checked(source.get_operation(), context)
        .expect("MIR lowering succeeds");
    service.take_result().expect("MIR result exists")
}

fn lower_gpu(context: &mut Context) -> lower_kernel_gpu::LoweringResult {
    let source = AlgorithmOp::new(context, 1).expect("valid kernel source");
    let mut service = lower_kernel_gpu::KernelGpuLoweringPass::new(gpu_config());
    service
        .run_checked(source.get_operation(), context)
        .expect("GPU lowering succeeds");
    service.take_result().expect("GPU result exists")
}

struct BoundArtifacts {
    mir: lower_mir_kernel::LoweringResult,
    gpu: lower_kernel_gpu::LoweringResult,
    bridge: BridgeEnvelope,
}

fn populated_context(record: &CanonicalKirRecord) -> (Context, BoundArtifacts) {
    let mut context = Context::new();
    lower_mir_kernel::register_pass(&mut context).expect("MIR registration succeeds");
    lower_kernel_gpu::register_pass(&mut context).expect("GPU registration succeeds");
    let mir = lower_mir(&mut context, "owner-boundary-mir");
    let gpu = lower_gpu(&mut context);
    let bridge = record
        .project_to_pliron(&mut context, BridgeLimits::default())
        .expect("bridge projection succeeds");
    (context, BoundArtifacts { mir, gpu, bridge })
}

fn marker_key(value: &str) -> Identifier {
    value.try_into().expect("fixed marker key is valid")
}

fn take_marker(context: &mut Context, key: &str) -> Box<dyn Any> {
    let index = context
        .aux_data_map
        .remove(&marker_key(key))
        .expect("marker is indexed");
    context.aux_data.remove(index).expect("marker is present")
}

fn install_marker(context: &mut Context, key: &str, marker: Box<dyn Any>) {
    let index = context.aux_data.insert(marker);
    context.aux_data_map.insert(marker_key(key), index);
}

fn transplant_marker(owner: &mut Context, foreign: &mut Context, key: &str) {
    let owner_marker = take_marker(owner, key);
    drop(take_marker(foreign, key));
    install_marker(foreign, key, owner_marker);
}

#[test]
fn equal_slot_foreign_contexts_reject_every_owner_bound_artifact() {
    let limits = BridgeLimits::default();
    let record = CanonicalKirRecord::from_module(&kir_module(), KirVersion::V5, limits)
        .expect("canonical KIR record");
    let (owner_context, owner) = populated_context(&record);
    let (foreign_context, foreign) = populated_context(&record);

    assert_eq!(owner.mir.source_root(), foreign.mir.source_root());
    assert_eq!(owner.mir.operations(), foreign.mir.operations());
    assert_eq!(owner.gpu.operations(), foreign.gpu.operations());
    assert_eq!(
        owner.bridge.shell().get_operation(),
        foreign.bridge.shell().get_operation()
    );

    assert_eq!(owner.mir.validate(&owner_context), Ok(()));
    assert_eq!(owner.gpu.validate(&owner_context), Ok(()));
    recover_exact(&owner_context, &owner.bridge, &record, limits)
        .expect("owner recovers its bridge envelope");

    let rejection = catch_unwind(AssertUnwindSafe(|| {
        (
            owner.mir.validate(&foreign_context),
            owner.gpu.validate(&foreign_context),
            recover_exact(&foreign_context, &owner.bridge, &record, limits),
        )
    }))
    .expect("foreign validation must not unwind");
    assert_eq!(
        rejection.0,
        Err(lower_mir_kernel::PostconditionError::ContextMismatch)
    );
    assert_eq!(
        rejection.1,
        Err(lower_kernel_gpu::PostconditionError::ContextMismatch)
    );
    assert!(matches!(rejection.2, Err(BridgeError::ContextMismatch)));
}

#[test]
fn transplanted_identity_and_registration_markers_cannot_transfer_ownership() {
    let limits = BridgeLimits::default();
    let record = CanonicalKirRecord::from_module(&kir_module(), KirVersion::V5, limits)
        .expect("canonical KIR record");
    let (mut owner_context, owner) = populated_context(&record);
    let (mut foreign_context, foreign) = populated_context(&record);

    assert_eq!(owner.mir.operations(), foreign.mir.operations());
    assert_eq!(owner.gpu.operations(), foreign.gpu.operations());
    assert_eq!(
        owner.bridge.shell().get_operation(),
        foreign.bridge.shell().get_operation()
    );

    for key in [
        CONTEXT_IDENTITY_MARKER_KEY,
        lower_mir_kernel::PASS_REGISTRATION_MARKER_KEY,
        lower_kernel_gpu::PASS_REGISTRATION_MARKER_KEY,
    ] {
        transplant_marker(&mut owner_context, &mut foreign_context, key);
    }

    assert_eq!(
        lower_mir_kernel::register_pass(&mut foreign_context),
        Err(lower_mir_kernel::PassRegistrationError::CorruptMarker)
    );
    assert_eq!(
        lower_kernel_gpu::register_pass(&mut foreign_context),
        Err(lower_kernel_gpu::PassRegistrationError::CorruptMarker)
    );

    let rejection = catch_unwind(AssertUnwindSafe(|| {
        (
            owner.mir.validate(&foreign_context),
            owner.gpu.validate(&foreign_context),
            recover_exact(&foreign_context, &owner.bridge, &record, limits),
        )
    }))
    .expect("transplanted-marker validation must not unwind");
    assert_eq!(
        rejection.0,
        Err(lower_mir_kernel::PostconditionError::ContextMismatch)
    );
    assert_eq!(
        rejection.1,
        Err(lower_kernel_gpu::PostconditionError::ContextMismatch)
    );
    assert!(matches!(
        rejection.2,
        Err(BridgeError::ContextIdentity(
            fe2o3_pliron::ContextIdentityError::CorruptMarker
        ))
    ));
}

#[test]
fn erased_owner_handles_return_typed_errors_without_unwinding() {
    let limits = BridgeLimits::default();
    let record = CanonicalKirRecord::from_module(&kir_module(), KirVersion::V5, limits)
        .expect("canonical KIR record");
    let mut context = Context::new();
    lower_mir_kernel::register_pass(&mut context).expect("MIR registration succeeds");
    lower_kernel_gpu::register_pass(&mut context).expect("GPU registration succeeds");

    let erased_source = lower_mir(&mut context, "erased-source");
    let erased_mir_output = lower_mir(&mut context, "erased-mir-output");
    let erased_gpu_output = lower_gpu(&mut context);
    let erased_bridge = record
        .project_to_pliron(&mut context, limits)
        .expect("bridge projection succeeds");

    Operation::erase(erased_source.source_root(), &mut context);
    Operation::erase(erased_mir_output.operations()[0], &mut context);
    Operation::erase(erased_gpu_output.operations()[0], &mut context);
    Operation::erase(erased_bridge.shell().get_operation(), &mut context);

    let rejection = catch_unwind(AssertUnwindSafe(|| {
        (
            erased_source.validate(&context),
            erased_mir_output.validate(&context),
            erased_gpu_output.validate(&context),
            recover_exact(&context, &erased_bridge, &record, limits),
        )
    }))
    .expect("erased-handle validation must not unwind");
    assert_eq!(
        rejection.0,
        Err(lower_mir_kernel::PostconditionError::SourceNoLongerValid)
    );
    assert_eq!(
        rejection.1,
        Err(lower_mir_kernel::PostconditionError::InvalidKernelOperation { index: 0 })
    );
    assert_eq!(
        rejection.2,
        Err(lower_kernel_gpu::PostconditionError::InvalidGpuOperation { index: 0 })
    );
    assert!(matches!(rejection.3, Err(BridgeError::MalformedShell)));
}

fn panicking_registration(
    _context: &mut Context,
    _name: &DialectName,
) -> Result<(), RegistrationHookError> {
    panic!("hostile-hook-payload");
}

fn panic_diagnostic(limits: ShellLimits) -> Diagnostic {
    let registration =
        DialectRegistration::new("hostile", panicking_registration).expect("valid registration");
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        PlironSession::new(limits, [registration])
    }))
    .expect("a panicking registration hook must be contained");
    match outcome {
        Err(ContextBuildError::RegistrationFailed(diagnostic)) => diagnostic,
        _ => panic!("a contained registration failure was expected"),
    }
}

#[test]
fn panicking_hooks_produce_bounded_deterministic_diagnostics() {
    let limits = ShellLimits::new(1, 1, 23).expect("bounded shell limits");
    let first = panic_diagnostic(limits);
    let second = panic_diagnostic(limits);

    assert_eq!(first, second);
    assert_eq!(first.code(), DiagnosticCode::DialectHookFailed);
    assert_eq!(first.stage(), Some("hostile"));
    assert!(first.message().len() <= limits.max_diagnostic_bytes());
    assert!(first.message().is_char_boundary(first.message().len()));
    assert!(!first.message().contains("hostile-hook-payload"));
}
