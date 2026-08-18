#![forbid(unsafe_code)]

//! Cross-crate conformance tests for the bounded Pliron integration surfaces.

use dialect_autotune::CandidateSetOp;
use dialect_dispatch::{
    DispatchIdAttr, DispatchIntentOpInterface, DispatchModeAttr, GraphCapacityAttr, GraphIntentOp,
};
use dialect_gpu::{AddressSpaceAttr, HierarchyAttr, HierarchyIdOp, TargetNeutralGpuOpInterface};
use dialect_kernel::{AlgorithmOp, IterationDomainAttr};
use dialect_mir::{
    MirTypeId,
    pliron::{MirDialectLimits, MirIdentityAttr, MirModuleOp, mir_dialect_registration},
};
use dialect_proof::{
    CoveredBoundaryAttr, EvidenceRefOp, EvidenceStatusAttr, ProofIdAttr, ProofOverlayOpInterface,
    PropertyAttr,
};
use dialect_schedule::{NonExecutableScheduleOp, PlanOp};
use dialect_tile::MaterializeOp;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Module, Signature,
    Terminator,
};
use fe2o3_kir_pliron_bridge::{
    BridgeError, BridgeLimits, CanonicalKirRecord, HARD_MAX_CANONICAL_BYTES,
    HARD_MAX_SHELL_OPERATIONS, KirVersion, ShellOperationKind, recover_exact,
};
use fe2o3_lower_kernel_gpu as lower_kernel_gpu;
use fe2o3_lower_mir_kernel as lower_mir_kernel;
use fe2o3_pliron::{
    ContextBuildError, DialectRegistration, PlironSession, RegistrationHookError, ShellLimits,
};
use pliron::{
    builtin::{attributes::UnitAttr, op_interfaces::SingleBlockRegionInterface},
    context::{Context, Ptr},
    dialect::DialectName,
    identifier::Identifier,
    linked_list::ContainsLinkedList,
    op::{Op, op_cast},
    operation::{Operation, verify_operation},
};

const FORWARD_DIALECTS: [&str; 8] = [
    dialect_mir::DIALECT,
    dialect_kernel::DIALECT_NAME,
    dialect_schedule::DIALECT_NAME,
    dialect_tile::DIALECT_NAME,
    dialect_gpu::DIALECT_NAME,
    dialect_proof::DIALECT_NAME,
    dialect_dispatch::DIALECT_NAME,
    dialect_autotune::DIALECT_NAME,
];

const REVERSE_DIALECTS: [&str; 8] = [
    dialect_autotune::DIALECT_NAME,
    dialect_dispatch::DIALECT_NAME,
    dialect_proof::DIALECT_NAME,
    dialect_gpu::DIALECT_NAME,
    dialect_tile::DIALECT_NAME,
    dialect_schedule::DIALECT_NAME,
    dialect_kernel::DIALECT_NAME,
    dialect_mir::DIALECT,
];

fn registration_error(error: impl std::fmt::Display) -> RegistrationHookError {
    RegistrationHookError::new(error.to_string())
}

fn kernel_registration(
    context: &mut Context,
    name: &DialectName,
) -> Result<(), RegistrationHookError> {
    dialect_kernel::register_dialect(context, name)
        .map(|_| ())
        .map_err(registration_error)
}

fn schedule_registration(
    context: &mut Context,
    name: &DialectName,
) -> Result<(), RegistrationHookError> {
    dialect_schedule::register_dialect(context, name)
        .map(|_| ())
        .map_err(registration_error)
}

fn tile_registration(
    context: &mut Context,
    name: &DialectName,
) -> Result<(), RegistrationHookError> {
    dialect_tile::register_dialect(context, name)
        .map(|_| ())
        .map_err(registration_error)
}

fn gpu_registration(
    context: &mut Context,
    name: &DialectName,
) -> Result<(), RegistrationHookError> {
    require_hook_name(name, dialect_gpu::DIALECT_NAME)?;
    dialect_gpu::register_dialect(context)
        .map(|_| ())
        .map_err(registration_error)
}

fn proof_registration(
    context: &mut Context,
    name: &DialectName,
) -> Result<(), RegistrationHookError> {
    require_hook_name(name, dialect_proof::DIALECT_NAME)?;
    dialect_proof::register_dialect(context)
        .map(|_| ())
        .map_err(registration_error)
}

fn dispatch_registration(
    context: &mut Context,
    name: &DialectName,
) -> Result<(), RegistrationHookError> {
    require_hook_name(name, dialect_dispatch::DIALECT_NAME)?;
    dialect_dispatch::register_dialect(context)
        .map(|_| ())
        .map_err(registration_error)
}

fn autotune_registration(
    context: &mut Context,
    name: &DialectName,
) -> Result<(), RegistrationHookError> {
    dialect_autotune::register_dialect(context, name)
        .map(|_| ())
        .map_err(registration_error)
}

fn require_hook_name(actual: &DialectName, expected: &str) -> Result<(), RegistrationHookError> {
    if actual.as_ref() == expected {
        Ok(())
    } else {
        Err(RegistrationHookError::new("dialect hook name mismatch"))
    }
}

fn registration(name: &str) -> DialectRegistration {
    match name {
        dialect_mir::DIALECT => mir_dialect_registration().expect("valid MIR registration"),
        dialect_kernel::DIALECT_NAME => {
            DialectRegistration::new(name, kernel_registration).expect("valid kernel registration")
        }
        dialect_schedule::DIALECT_NAME => DialectRegistration::new(name, schedule_registration)
            .expect("valid schedule registration"),
        dialect_tile::DIALECT_NAME => {
            DialectRegistration::new(name, tile_registration).expect("valid tile registration")
        }
        dialect_gpu::DIALECT_NAME => {
            DialectRegistration::new(name, gpu_registration).expect("valid GPU registration")
        }
        dialect_proof::DIALECT_NAME => {
            DialectRegistration::new(name, proof_registration).expect("valid proof registration")
        }
        dialect_dispatch::DIALECT_NAME => DialectRegistration::new(name, dispatch_registration)
            .expect("valid dispatch registration"),
        dialect_autotune::DIALECT_NAME => DialectRegistration::new(name, autotune_registration)
            .expect("valid autotune registration"),
        _ => panic!("unknown conformance dialect {name}"),
    }
}

fn combined_session(order: &[&str]) -> PlironSession {
    PlironSession::new(
        ShellLimits::default(),
        order.iter().copied().map(registration),
    )
    .expect("combined registration must succeed in a fresh context")
}

fn register_lowerings(context: &mut Context) {
    assert_eq!(
        lower_mir_kernel::register_pass(context),
        Ok(lower_mir_kernel::PassRegistrationOutcome::Registered)
    );
    assert_eq!(
        lower_kernel_gpu::register_pass(context),
        Ok(lower_kernel_gpu::PassRegistrationOutcome::Registered)
    );
    assert_eq!(
        lower_mir_kernel::register_pass(context),
        Ok(lower_mir_kernel::PassRegistrationOutcome::AlreadyRegistered)
    );
    assert_eq!(
        lower_kernel_gpu::register_pass(context),
        Ok(lower_kernel_gpu::PassRegistrationOutcome::AlreadyRegistered)
    );
}

fn mir_source(context: &mut Context, identity: &str) -> MirModuleOp {
    let limits = MirDialectLimits::new(4, 4, 128).expect("bounded MIR limits");
    let module = MirModuleOp::try_new(context, identity, limits).expect("valid MIR module");
    module
        .append_function(
            context,
            format!("{identity}::entry"),
            &[MirTypeId(7), MirTypeId(2)],
        )
        .expect("valid MIR function");
    module
}

fn mir_config(rank: u32) -> lower_mir_kernel::LoweringConfig {
    lower_mir_kernel::LoweringConfig::new(
        lower_mir_kernel::LoweringLimits::new(1, 4, 8, 32, 4).expect("bounded MIR lowering limits"),
        rank,
    )
    .expect("bounded structured rank")
}

fn gpu_config(rank: &[u32]) -> lower_kernel_gpu::LoweringConfig {
    lower_kernel_gpu::LoweringConfig::new(
        lower_kernel_gpu::WorkgroupShape::new(rank).expect("bounded workgroup"),
        1,
        &[AddressSpaceAttr::Global, AddressSpaceAttr::Workgroup],
        lower_kernel_gpu::SynchronizationMode::WorkgroupBarrier,
        6,
    )
    .expect("bounded GPU lowering configuration")
}

fn proof_id(value: u64) -> ProofIdAttr {
    ProofIdAttr::new([0, 0, 0, value])
}

fn dispatch_id(value: u64) -> DispatchIdAttr {
    DispatchIdAttr::new([0, 0, 0, value])
}

#[derive(Debug, Eq, PartialEq)]
struct SurfaceSnapshot {
    mir_functions: usize,
    kernel_rank: u32,
    schedule_rank: u32,
    tile_elements: u32,
    proof_status: EvidenceStatusAttr,
    autotune_candidates: u32,
}

fn exercise_registered_surface(session: &mut PlironSession) -> SurfaceSnapshot {
    session
        .with_context_mut(|context| {
            register_lowerings(context);

            let mir = mir_source(context, "surface");
            let kernel = AlgorithmOp::new(context, 2).expect("bounded kernel algorithm");
            let schedule = PlanOp::new(context, 2, 16, 2).expect("bounded schedule");
            let tile = MaterializeOp::new(context, 2, 32, 4).expect("bounded tile");
            let gpu = HierarchyIdOp::new(context, HierarchyAttr::Grid);
            let proof = EvidenceRefOp::new(
                context,
                proof_id(1),
                proof_id(2),
                PropertyAttr::Bounds,
                EvidenceStatusAttr::Checked,
                CoveredBoundaryAttr::TargetNeutralGpu,
            );
            let dispatch = GraphIntentOp::new(
                context,
                dispatch_id(1),
                GraphCapacityAttr::Nodes16,
                DispatchModeAttr::UnfusedFinite,
            );
            let autotune = CandidateSetOp::new(context, 4, 8).expect("bounded candidates");

            for operation in [
                mir.get_operation(),
                kernel.get_operation(),
                schedule.get_operation(),
                tile.get_operation(),
                gpu.get_operation(),
                proof.get_operation(),
                dispatch.get_operation(),
                autotune.get_operation(),
            ] {
                verify_operation(operation, context).expect("registered operation must verify");
            }

            let schedule_interface = op_cast::<dyn NonExecutableScheduleOp>(&schedule)
                .expect("schedule interface is registered");
            assert!(!schedule_interface.is_executable());
            let gpu_interface = op_cast::<dyn TargetNeutralGpuOpInterface>(&gpu)
                .expect("GPU interface is registered");
            assert!(!gpu_interface.grants_runtime_authority());
            let proof_interface = op_cast::<dyn ProofOverlayOpInterface>(&proof)
                .expect("proof interface is registered");
            assert!(!proof_interface.grants_authority());
            let dispatch_interface = op_cast::<dyn DispatchIntentOpInterface>(&dispatch)
                .expect("dispatch interface is registered");
            assert!(!dispatch_interface.grants_runtime_authority());
            assert!(!EvidenceStatusAttr::Checked.grants_authority());

            SurfaceSnapshot {
                mir_functions: mir.function_count(context),
                kernel_rank: kernel.iteration_domain(context).unwrap().rank(),
                schedule_rank: schedule.parameters(context).unwrap().rank(),
                tile_elements: tile
                    .distribution(context)
                    .unwrap()
                    .total_elements()
                    .unwrap(),
                proof_status: proof.status(context).unwrap(),
                autotune_candidates: autotune.budget(context).unwrap().candidates(),
            }
        })
        .expect("fresh session remains healthy")
}

#[derive(Debug, Eq, PartialEq)]
struct LoweringSnapshot {
    mir: lower_mir_kernel::LoweringRecord,
    gpu: lower_kernel_gpu::LoweringRecord,
}

fn exercise_lowerings(session: &mut PlironSession) -> LoweringSnapshot {
    session
        .with_context_mut(|context| {
            register_lowerings(context);
            let source = mir_source(context, "lowering");
            let mut mir_service = lower_mir_kernel::MirKernelLoweringPass::new(mir_config(2));
            let (kernel_root, mir_record) = {
                let result = mir_service
                    .run_checked(source.get_operation(), context)
                    .expect("supported MIR lowering");
                result.validate(context).expect("valid MIR lowering result");
                assert!(!result.grants_authority());
                (result.operations()[0], result.record().clone())
            };

            let mut gpu_service = lower_kernel_gpu::KernelGpuLoweringPass::new(gpu_config(&[8, 4]));
            let gpu_record = {
                let result = gpu_service
                    .run_checked(kernel_root, context)
                    .expect("supported kernel lowering");
                result.validate(context).expect("valid GPU lowering result");
                assert!(!result.grants_authority());
                result.record().clone()
            };

            LoweringSnapshot {
                mir: mir_record,
                gpu: gpu_record,
            }
        })
        .expect("fresh session remains healthy")
}

fn kir_module(identity: &str) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new(identity);
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D2 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Static(4),
        },
    ));
    module
}

#[test]
fn combined_registration_is_fresh_idempotent_and_order_independent() {
    let mut forward = combined_session(&FORWARD_DIALECTS);
    let mut reverse = combined_session(&REVERSE_DIALECTS);

    assert_eq!(
        forward.manifest().registration_order(),
        &FORWARD_DIALECTS.map(str::to_owned)
    );
    assert_eq!(
        reverse.manifest().registration_order(),
        &REVERSE_DIALECTS.map(str::to_owned)
    );
    assert_eq!(
        forward.manifest().pliron_revision(),
        reverse.manifest().pliron_revision()
    );
    assert_eq!(
        exercise_registered_surface(&mut forward),
        exercise_registered_surface(&mut reverse)
    );
}

#[test]
fn bounded_lowering_records_are_deterministic_across_registration_orders() {
    let mut forward = combined_session(&FORWARD_DIALECTS);
    let mut reverse = combined_session(&REVERSE_DIALECTS);
    let forward = exercise_lowerings(&mut forward);
    let reverse = exercise_lowerings(&mut reverse);

    assert_eq!(forward, reverse);
    assert_eq!(forward.mir.source().identity(), "lowering");
    assert_eq!(forward.mir.rewrite_count(), 1);
    assert_eq!(forward.gpu.source_rank(), 2);
    assert_eq!(
        forward.gpu.memory_spaces(),
        &[AddressSpaceAttr::Workgroup, AddressSpaceAttr::Global]
    );
    assert_eq!(forward.gpu.rewrite_count(), 6);
}

#[test]
fn duplicate_and_colliding_registration_fail_before_use() {
    let duplicate = registration(dialect_mir::DIALECT);
    let result = PlironSession::new(ShellLimits::default(), [duplicate.clone(), duplicate]);
    assert!(matches!(
        result,
        Err(ContextBuildError::DuplicateDialect(name)) if name == dialect_mir::DIALECT
    ));

    let mut mir_collision = Context::new();
    install_foreign_marker(
        &mut mir_collision,
        lower_mir_kernel::PASS_REGISTRATION_MARKER_KEY,
    );
    assert_eq!(
        lower_mir_kernel::register_pass(&mut mir_collision),
        Err(lower_mir_kernel::PassRegistrationError::MarkerCollision)
    );
    corrupt_foreign_marker(
        &mut mir_collision,
        lower_mir_kernel::PASS_REGISTRATION_MARKER_KEY,
    );
    assert_eq!(
        lower_mir_kernel::register_pass(&mut mir_collision),
        Err(lower_mir_kernel::PassRegistrationError::CorruptMarker)
    );
    let mir_source = mir_source(&mut mir_collision, "poisoned-mir");
    let mut mir_service = lower_mir_kernel::MirKernelLoweringPass::new(mir_config(1));
    assert_eq!(
        mir_service.run_checked(mir_source.get_operation(), &mut mir_collision),
        Err(lower_mir_kernel::LoweringError::RegistrationCorrupt)
    );
    assert!(mir_service.last_result().is_none());

    let mut gpu_collision = Context::new();
    install_foreign_marker(
        &mut gpu_collision,
        lower_kernel_gpu::PASS_REGISTRATION_MARKER_KEY,
    );
    assert_eq!(
        lower_kernel_gpu::register_pass(&mut gpu_collision),
        Err(lower_kernel_gpu::PassRegistrationError::MarkerCollision)
    );
    corrupt_foreign_marker(
        &mut gpu_collision,
        lower_kernel_gpu::PASS_REGISTRATION_MARKER_KEY,
    );
    assert_eq!(
        lower_kernel_gpu::register_pass(&mut gpu_collision),
        Err(lower_kernel_gpu::PassRegistrationError::CorruptMarker)
    );
    let kernel_source = AlgorithmOp::new(&mut gpu_collision, 1).expect("valid kernel source");
    let mut gpu_service = lower_kernel_gpu::KernelGpuLoweringPass::new(gpu_config(&[64]));
    assert_eq!(
        gpu_service.run_checked(kernel_source.get_operation(), &mut gpu_collision),
        Err(lower_kernel_gpu::LoweringError::RegistrationCorrupt)
    );
    assert!(gpu_service.last_result().is_none());
}

fn install_foreign_marker(context: &mut Context, key: &str) {
    let key: Identifier = key.try_into().expect("fixed marker key is valid");
    let marker = context.aux_data.insert(Box::new(7_u32));
    context.aux_data_map.insert(key, marker);
}

fn corrupt_foreign_marker(context: &mut Context, key: &str) {
    let key: Identifier = key.try_into().expect("fixed marker key is valid");
    let marker = *context
        .aux_data_map
        .get(&key)
        .expect("foreign marker was installed");
    context.aux_data.remove(marker);
}

#[test]
fn stale_source_and_mutated_outputs_fail_postconditions() {
    let mut session = combined_session(&FORWARD_DIALECTS);
    session
        .with_context_mut(|context| {
            register_lowerings(context);
            let source = mir_source(context, "mutable-source");
            let mut mir_service = lower_mir_kernel::MirKernelLoweringPass::new(mir_config(1));
            mir_service
                .run_checked(source.get_operation(), context)
                .expect("initial MIR lowering");
            let mir_result = mir_service.take_result().expect("MIR result");

            source.set_attr_module_identity(context, MirIdentityAttr::new("stale-source"));
            assert_eq!(
                mir_result.validate(context),
                Err(lower_mir_kernel::PostconditionError::SourceEvidenceMismatch)
            );
            source.set_attr_module_identity(context, MirIdentityAttr::new("mutable-source"));

            let lowered_kernel = AlgorithmOp::from_operation(mir_result.operations()[0]);
            lowered_kernel.set_iteration_domain(
                context,
                IterationDomainAttr::new(2).expect("bounded conflicting rank"),
            );
            assert_eq!(
                mir_result.validate(context),
                Err(lower_mir_kernel::PostconditionError::InvalidKernelOperation { index: 0 })
            );

            let source_kernel = AlgorithmOp::new(context, 1).expect("valid kernel source");
            let mut gpu_service = lower_kernel_gpu::KernelGpuLoweringPass::new(gpu_config(&[64]));
            gpu_service
                .run_checked(source_kernel.get_operation(), context)
                .expect("initial GPU lowering");
            let gpu_result = gpu_service.take_result().expect("GPU result");
            gpu_result.operations()[0]
                .deref_mut(context)
                .attributes
                .0
                .clear();
            assert_eq!(
                gpu_result.validate(context),
                Err(lower_kernel_gpu::PostconditionError::InvalidGpuOperation { index: 0 })
            );
        })
        .unwrap();
}

#[test]
fn terminal_unsupported_inputs_clear_results_without_fallback_reuse() {
    let mut session = combined_session(&FORWARD_DIALECTS);
    session
        .with_context_mut(|context| {
            register_lowerings(context);
            let mir = mir_source(context, "terminal");
            let kernel = AlgorithmOp::new(context, 1).expect("valid kernel source");

            let mut mir_service = lower_mir_kernel::MirKernelLoweringPass::new(mir_config(1));
            mir_service
                .run_checked(mir.get_operation(), context)
                .expect("initial MIR success");
            assert!(mir_service.last_result().is_some());
            assert_eq!(
                mir_service.run_checked(kernel.get_operation(), context),
                Err(lower_mir_kernel::LoweringError::UnsupportedSourceOperation)
            );
            assert!(mir_service.last_result().is_none());

            let mut gpu_service = lower_kernel_gpu::KernelGpuLoweringPass::new(gpu_config(&[64]));
            gpu_service
                .run_checked(kernel.get_operation(), context)
                .expect("initial GPU success");
            assert!(gpu_service.last_result().is_some());
            assert_eq!(
                gpu_service.run_checked(mir.get_operation(), context),
                Err(lower_kernel_gpu::LoweringError::UnsupportedSourceOperation)
            );
            assert!(gpu_service.last_result().is_none());
        })
        .unwrap();
}

#[test]
fn lowering_results_reject_populated_foreign_contexts_before_pointer_dereference() {
    let mut session = combined_session(&FORWARD_DIALECTS);
    let (mir_result, gpu_result) = session
        .with_context_mut(|context| {
            register_lowerings(context);
            let mir = mir_source(context, "context-bound");
            let mut mir_service = lower_mir_kernel::MirKernelLoweringPass::new(mir_config(1));
            mir_service
                .run_checked(mir.get_operation(), context)
                .expect("MIR lowering");
            let mir_result = mir_service.take_result().expect("MIR result");

            let kernel = AlgorithmOp::new(context, 1).expect("valid kernel source");
            let mut gpu_service = lower_kernel_gpu::KernelGpuLoweringPass::new(gpu_config(&[64]));
            gpu_service
                .run_checked(kernel.get_operation(), context)
                .expect("GPU lowering");
            let gpu_result = gpu_service.take_result().expect("GPU result");
            (mir_result, gpu_result)
        })
        .unwrap();

    let mut foreign = combined_session(&REVERSE_DIALECTS);
    foreign
        .with_context_mut(|context| {
            register_lowerings(context);
            let _foreign_mir = mir_source(context, "foreign-populated");
            let _foreign_kernel = AlgorithmOp::new(context, 1).expect("foreign kernel source");
        })
        .unwrap();

    let mir_validation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        foreign.with_context_mut(|context| mir_result.validate(context))
    }));
    assert!(
        mir_validation.is_ok(),
        "MIR result dereferenced pointers in a foreign context"
    );
    assert_eq!(
        mir_validation.unwrap().unwrap(),
        Err(lower_mir_kernel::PostconditionError::ContextMismatch)
    );

    let gpu_validation = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        foreign.with_context_mut(|context| gpu_result.validate(context))
    }));
    assert!(
        gpu_validation.is_ok(),
        "GPU result dereferenced pointers in a foreign context"
    );
    assert_eq!(
        gpu_validation.unwrap().unwrap(),
        Err(lower_kernel_gpu::PostconditionError::ContextMismatch)
    );
}

#[test]
fn every_exact_kir_v1_v5_envelope_is_deterministic() {
    let limits = BridgeLimits::default();
    let versions = [
        KirVersion::V1,
        KirVersion::V2,
        KirVersion::V3,
        KirVersion::V4,
        KirVersion::V5,
    ];

    for (index, version) in versions.into_iter().enumerate() {
        let identity = format!("independent-kir-v{}", version.wire_value());
        let module = kir_module(&identity);
        let first = CanonicalKirRecord::from_module(&module, version, limits).unwrap();
        let second = CanonicalKirRecord::from_module(&module, version, limits).unwrap();
        assert_eq!(first.canonical_bytes(), second.canonical_bytes());
        assert_eq!(
            &first.canonical_bytes()[8..10],
            &version.wire_value().to_le_bytes()
        );

        let order = if index % 2 == 0 {
            &FORWARD_DIALECTS
        } else {
            &REVERSE_DIALECTS
        };
        let mut session = combined_session(order);
        session
            .with_context_mut(|context| {
                let envelope = first.project_to_pliron(context, limits).unwrap();
                assert_eq!(
                    envelope
                        .shell()
                        .get_body(context, 0)
                        .deref(context)
                        .iter(context)
                        .count(),
                    2
                );
                let recovered = recover_exact(context, &envelope, &first, limits).unwrap();
                assert_eq!(recovered.version(), version);
                assert_eq!(recovered.module_identity(), identity);
                assert_eq!(recovered.canonical_bytes(), first.canonical_bytes());
            })
            .unwrap();
    }
}

#[test]
fn kir_substitution_and_projection_order_confusion_fail_closed() {
    let limits = BridgeLimits::default();
    let expected =
        CanonicalKirRecord::from_module(&kir_module("expected"), KirVersion::V5, limits).unwrap();
    let substituted =
        CanonicalKirRecord::from_module(&kir_module("substituted"), KirVersion::V5, limits)
            .unwrap();
    let mut session = combined_session(&REVERSE_DIALECTS);
    session
        .with_context_mut(|context| {
            let envelope = expected.project_to_pliron(context, limits).unwrap();
            assert!(matches!(
                recover_exact(context, &envelope, &substituted, limits),
                Err(BridgeError::RecordSubstitution)
            ));

            let body = envelope.shell().get_body(context, 0);
            let first = body.deref(context).get_head().expect("kernel projection");
            first.unlink(context);
            first.insert_at_back(body, context);
            assert!(matches!(
                recover_exact(context, &envelope, &expected, limits),
                Err(BridgeError::ShellOperationConflict {
                    index: 0,
                    expected: ShellOperationKind::KernelAlgorithm,
                })
            ));
        })
        .unwrap();
}

#[test]
fn kir_trust_boundary_rejects_extra_metadata_and_preflights_bounds() {
    let limits = BridgeLimits::default();
    let expected =
        CanonicalKirRecord::from_module(&kir_module("trust-boundary"), KirVersion::V5, limits)
            .unwrap();
    let mut session = combined_session(&FORWARD_DIALECTS);
    session
        .with_context_mut(|context| {
            let extra = expected.project_to_pliron(context, limits).unwrap();
            let extra_key: Identifier = "hostile_extra_metadata".try_into().unwrap();
            extra
                .shell()
                .get_operation()
                .deref_mut(context)
                .attributes
                .set(extra_key, UnitAttr);
            assert!(matches!(
                recover_exact(context, &extra, &expected, limits),
                Err(BridgeError::UnexpectedMetadata)
            ));

            let canonical_limited = expected.project_to_pliron(context, limits).unwrap();
            let canonical_limit = BridgeLimits::new(
                expected.canonical_bytes().len() - 1,
                HARD_MAX_SHELL_OPERATIONS,
            )
            .unwrap();
            assert!(matches!(
                recover_exact(context, &canonical_limited, &expected, canonical_limit),
                Err(BridgeError::CanonicalBytesLimit { .. })
            ));

            let shell_limited = expected.project_to_pliron(context, limits).unwrap();
            let shell_limit = BridgeLimits::new(HARD_MAX_CANONICAL_BYTES, 1).unwrap();
            assert!(matches!(
                recover_exact(context, &shell_limited, &expected, shell_limit),
                Err(BridgeError::ShellOperationsLimit { actual: 2, max: 1 })
            ));
        })
        .unwrap();
}

#[test]
fn lowering_and_kir_outputs_remain_distinct_and_non_authoritative() {
    let limits = BridgeLimits::default();
    let record =
        CanonicalKirRecord::from_module(&kir_module("independent-kir"), KirVersion::V5, limits)
            .unwrap();
    let mut session = combined_session(&FORWARD_DIALECTS);
    session
        .with_context_mut(|context| {
            register_lowerings(context);
            let source = mir_source(context, "independent-mir");
            let mut mir_service = lower_mir_kernel::MirKernelLoweringPass::new(mir_config(2));
            mir_service
                .run_checked(source.get_operation(), context)
                .expect("MIR lowering");
            let mir_result = mir_service.take_result().expect("MIR result");
            let lowered_kernel = mir_result.operations()[0];

            let mut gpu_service = lower_kernel_gpu::KernelGpuLoweringPass::new(gpu_config(&[8, 4]));
            gpu_service
                .run_checked(lowered_kernel, context)
                .expect("kernel lowering");
            let gpu_result = gpu_service.take_result().expect("GPU result");

            let envelope = record.project_to_pliron(context, limits).unwrap();
            let bridge_operations: Vec<Ptr<Operation>> = envelope
                .shell()
                .get_body(context, 0)
                .deref(context)
                .iter(context)
                .collect();
            assert_eq!(bridge_operations.len(), 2);
            assert!(!bridge_operations.contains(&lowered_kernel));
            assert!(
                gpu_result
                    .operations()
                    .iter()
                    .all(|operation| !bridge_operations.contains(operation))
            );
            assert_ne!(
                mir_result.record().source().identity(),
                record.module_identity()
            );
            assert!(!mir_result.grants_authority());
            assert!(!gpu_result.grants_authority());
            assert_eq!(
                recover_exact(context, &envelope, &record, limits).unwrap(),
                record
            );
        })
        .unwrap();
}
