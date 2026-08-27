#![forbid(unsafe_code)]

//! Cross-crate conformance tests for the bounded Pliron integration surfaces.

use dialect_autotune::CandidateSetOp;
use dialect_dispatch::{
    DispatchIdAttr, DispatchIntentOpInterface, DispatchModeAttr, GraphCapacityAttr, GraphIntentOp,
};
use dialect_gpu::{HierarchyAttr, HierarchyIdOp, TargetNeutralGpuOpInterface};
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
use fe2o3_lower_mir_kernel as lower_mir_kernel;
use fe2o3_pliron::{ContextBuildError, DialectRegistration, PlironSession, ShellLimits};
use pliron::{
    context::Context,
    identifier::Identifier,
    op::{Op, op_cast},
    operation::verify_operation,
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

fn registration(name: &str) -> DialectRegistration {
    match name {
        dialect_mir::DIALECT => mir_dialect_registration().expect("valid MIR registration"),
        dialect_kernel::DIALECT_NAME => {
            dialect_kernel::dialect_registration().expect("valid kernel registration")
        }
        dialect_schedule::DIALECT_NAME => {
            dialect_schedule::dialect_registration().expect("valid schedule registration")
        }
        dialect_tile::DIALECT_NAME => {
            dialect_tile::dialect_registration().expect("valid tile registration")
        }
        dialect_gpu::DIALECT_NAME => {
            dialect_gpu::dialect_registration().expect("valid GPU registration")
        }
        dialect_proof::DIALECT_NAME => {
            dialect_proof::dialect_registration().expect("valid proof registration")
        }
        dialect_dispatch::DIALECT_NAME => {
            dialect_dispatch::dialect_registration().expect("valid dispatch registration")
        }
        dialect_autotune::DIALECT_NAME => {
            dialect_autotune::dialect_registration().expect("valid autotune registration")
        }
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

fn register_lowering(context: &mut Context) {
    assert_eq!(
        lower_mir_kernel::register_pass(context),
        Ok(lower_mir_kernel::PassRegistrationOutcome::Registered)
    );
    assert_eq!(
        lower_mir_kernel::register_pass(context),
        Ok(lower_mir_kernel::PassRegistrationOutcome::AlreadyRegistered)
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
            register_lowering(context);

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

fn exercise_lowering(session: &mut PlironSession) -> lower_mir_kernel::LoweringRecord {
    session
        .with_context_mut(|context| {
            register_lowering(context);
            let source = mir_source(context, "lowering");
            let mut mir_service = lower_mir_kernel::MirKernelLoweringPass::new(mir_config(2));
            let mir_record = {
                let result = mir_service
                    .run_checked(source.get_operation(), context)
                    .expect("supported MIR lowering");
                result.validate(context).expect("valid MIR lowering result");
                assert!(!result.grants_authority());
                result.record().clone()
            };
            mir_record
        })
        .expect("fresh session remains healthy")
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
    let forward = exercise_lowering(&mut forward);
    let reverse = exercise_lowering(&mut reverse);

    assert_eq!(forward, reverse);
    assert_eq!(forward.source().identity(), "lowering");
    assert_eq!(forward.rewrite_count(), 1);
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
            register_lowering(context);
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
        })
        .unwrap();
}

#[test]
fn terminal_unsupported_inputs_clear_results_without_fallback_reuse() {
    let mut session = combined_session(&FORWARD_DIALECTS);
    session
        .with_context_mut(|context| {
            register_lowering(context);
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
        })
        .unwrap();
}

#[test]
fn lowering_results_reject_populated_foreign_contexts_before_pointer_dereference() {
    let mut session = combined_session(&FORWARD_DIALECTS);
    let mir_result = session
        .with_context_mut(|context| {
            register_lowering(context);
            let mir = mir_source(context, "context-bound");
            let mut mir_service = lower_mir_kernel::MirKernelLoweringPass::new(mir_config(1));
            mir_service
                .run_checked(mir.get_operation(), context)
                .expect("MIR lowering");
            let mir_result = mir_service.take_result().expect("MIR result");

            mir_result
        })
        .unwrap();

    let mut foreign = combined_session(&REVERSE_DIALECTS);
    foreign
        .with_context_mut(|context| {
            register_lowering(context);
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
}
