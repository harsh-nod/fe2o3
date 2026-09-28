use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkBudgetV1 as Work,
};
use pliron::{
    builtin::{
        ops::ModuleOp,
        types::{IntegerType, Signedness},
    },
    op::Op,
};

fn narrow() -> StructuralCensus {
    StructuralCensus {
        functions: 1,
        blocks: 1,
        operations: 3,
        values: 3,
        results: 2,
        operands: 4,
        successors: 1,
        max_definition_arity: 1,
        max_operands: 2,
        max_successors: 1,
        ..Default::default()
    }
}

#[test]
fn structural_admission_has_independent_exact_algebra_and_keeps_history_caps() {
    let census = narrow();
    let limits = Limits::for_structure(census)
        .unwrap()
        .for_policy3()
        .unwrap();
    let admission = ObserverAdmissionV18::for_structure(census).unwrap();
    assert_eq!(
        limits,
        Limits {
            nodes: 33,
            events: 330,
            targets: 330
        }
    );
    assert_eq!(admission.validation().unwrap(), 31);
    assert_eq!(admission.event_work().unwrap(), 88);
    assert_eq!(admission.work(limits).unwrap(), 33_264);
    assert_eq!(admission.map_nodes().unwrap(), 12);
    let (profile, map_limits) =
        crate::optimization_v12::policy3_execution_resources_v18(37, 33, admission).unwrap();
    assert_eq!(
        (map_limits.node_limit(), map_limits.event_limit()),
        (12, 330)
    );
    assert_eq!(admission.map_work(map_limits).unwrap(), 25_152);
    assert_eq!(map_limits.storage().unwrap(), 52_480);
    assert_eq!(profile.work(), 31_592_128);
    // Historical policy-3 remains exactly 320*N*N + 256*N, independent of
    // the new enforced V18 arity profile.
    assert_eq!(limits.work().unwrap(), 356_928);
}

#[test]
fn v18_row_node_refinement_preserves_transcript_caps_and_rejects_zero() {
    let original = crate::kir_optimization_map_v12::CaptureLimitsV12::for_policy_bytes(
        0,
        FixedPolicy::Checked3,
    )
    .unwrap();
    assert_eq!((original.node_limit(), original.event_limit()), (64, 640));
    assert_eq!(original.work().unwrap(), 331_776);
    assert_eq!(original.storage().unwrap(), 118_784);
    let refined = original.for_v18_row_node_bound(3).unwrap();
    assert_eq!((refined.node_limit(), refined.event_limit()), (3, 640));
    assert_eq!(refined.work().unwrap(), 15_552);
    // 3*512 + 640*64 + 640*64 + 4096 also proves the private target cap.
    assert_eq!(refined.storage().unwrap(), 87_552);
    assert_eq!(original.for_v18_row_node_bound(usize::MAX), Ok(original));
    assert_eq!(refined.for_v18_row_node_bound(64), Ok(refined));
    assert_eq!(original.for_v18_row_node_bound(0), Err(E::Limit));
    assert_eq!(original.node_limit(), 64);
    assert_eq!(original.event_limit(), 640);
    assert_eq!(original.storage().unwrap(), 118_784);
}

#[test]
fn empty_capture_admission_is_exact_before_construction_and_restores_floor() {
    let mut ctx = Context::new();
    let root = ModuleOp::new(&mut ctx, "empty".try_into().unwrap()).get_operation();
    let source = Module::new("empty");
    let admission = ObserverAdmissionV18::for_structure(StructuralCensus::default()).unwrap();
    let limits = Limits::for_structure(StructuralCensus::default())
        .unwrap()
        .for_policy3()
        .unwrap();
    assert_eq!(admission.work(limits).unwrap(), 512);
    assert_eq!(limits.storage().unwrap(), 11_840);
    for (work_allowance, storage_allowance) in [(511, 11_840), (512, 11_839), (512, 11_840)] {
        let mut work = Work::new(7 + work_allowance);
        work.charge_work(7).unwrap();
        let mut budget = Budget::new(&mut work, 13 + storage_allowance);
        budget.reserve_storage(13).unwrap();
        let mut constructed = false;
        let result = (|| -> Result<()> {
            budget.charge_work(admission.work(limits)?)?;
            budget.reserve_storage(limits.storage()?)?;
            constructed = true;
            let capture = Capture::new_v18(&ctx, root, &source, &Vec::new(), limits, 0, admission)?;
            assert_eq!(capture.failure(), None);
            drop(capture);
            budget.release_storage(limits.storage()?)?;
            Ok(())
        })();
        match (work_allowance, storage_allowance) {
            (511, _) => {
                assert!(
                    matches!(result, Err(E::Resources(Resource::Work(error))) if error.actual() == 519 && error.limit() == 518)
                );
                assert_eq!(budget.work(), 7);
            }
            (_, 11_839) => {
                assert!(
                    matches!(result, Err(E::Resources(Resource::Storage(error))) if error.actual() == 11_853 && error.limit() == 11_852)
                );
                assert_eq!(budget.work(), 519);
            }
            _ => {
                result.unwrap();
                assert_eq!(budget.work(), 519);
            }
        }
        assert_eq!(
            constructed,
            work_allowance == 512 && storage_allowance == 11_840
        );
        assert_eq!(budget.storage(), 13);
    }
}

fn raw_constant(ctx: &mut Context, results: usize) -> Ptr<Operation> {
    let ty = IntegerType::get(ctx, 32, Signedness::Unsigned).into();
    Operation::new(
        ctx,
        dialect_gpu::optimization_v1::ConstantOp::get_concrete_op_info(),
        vec![ty; results],
        vec![],
        vec![],
        0,
    )
}

#[test]
fn oversized_callback_is_refused_before_legacy_observer_traversal() {
    struct Probe(Arc<std::sync::atomic::AtomicUsize>);
    impl RewriteObserver for Probe {
        fn observe(&mut self, _: &Context, _: RewriteEvent) {
            self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
    let mut ctx = Context::new();
    let root = ModuleOp::new(&mut ctx, "empty".try_into().unwrap()).get_operation();
    let source = Module::new("empty");
    let admission = ObserverAdmissionV18::for_structure(StructuralCensus::default()).unwrap();
    let limits = Limits::for_structure(StructuralCensus::default())
        .unwrap()
        .for_policy3()
        .unwrap();
    let capture = Capture::new_v18(&ctx, root, &source, &Vec::new(), limits, 0, admission).unwrap();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut observer = capture.observer(Box::new(Probe(calls.clone())));
    let oversized = raw_constant(&mut ctx, 2);
    observer.observe(&ctx, RewriteEvent::OperationInserted(oversized));
    assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 0);
    assert_eq!(capture.failure(), Some(E::Limit));
}

#[test]
fn constant_and_branch_growth_caps_are_enforced_atomically() {
    let mut ctx = Context::new();
    let admission = ObserverAdmissionV18::for_structure(StructuralCensus {
        values: 1,
        conditional_branches: 2,
        conditional_operands: 2,
        max_definition_arity: 2,
        max_operands: 2,
        max_successors: 2,
        ..Default::default()
    })
    .unwrap();
    let mut growth = GrowthV18::default();
    let first = raw_constant(&mut ctx, 1);
    let second = raw_constant(&mut ctx, 1);
    admission.register_growth(&ctx, first, &mut growth).unwrap();
    assert_eq!(
        admission.register_growth(&ctx, second, &mut growth),
        Err(E::Limit)
    );
    assert_eq!(growth.constants, 1);
    let ty = IntegerType::get(&mut ctx, 32, Signedness::Unsigned).into();
    let block = BasicBlock::new(&mut ctx, None, vec![ty; 2]);
    let args = block.deref(&ctx).arguments().collect::<Vec<_>>();
    let first =
        dialect_gpu::optimization_v1::BranchOp::new(&mut ctx, block, args.clone()).get_operation();
    let second = dialect_gpu::optimization_v1::BranchOp::new(&mut ctx, block, args).get_operation();
    admission.register_growth(&ctx, first, &mut growth).unwrap();
    assert_eq!(
        admission.register_growth(&ctx, second, &mut growth),
        Err(E::Limit)
    );
    assert_eq!((growth.branches, growth.operands), (1, 2));
    let empty =
        dialect_gpu::optimization_v1::BranchOp::new(&mut ctx, block, vec![]).get_operation();
    admission.register_growth(&ctx, empty, &mut growth).unwrap();
    let another =
        dialect_gpu::optimization_v1::BranchOp::new(&mut ctx, block, vec![]).get_operation();
    assert_eq!(
        admission.register_growth(&ctx, another, &mut growth),
        Err(E::Limit)
    );
    assert_eq!((growth.branches, growth.operands), (2, 2));
    let argument = block.deref(&ctx).get_argument(0);
    Operation::push_operand(first, &ctx, argument);
    assert_eq!(admission.operation_shape(&ctx, first), Err(E::Limit));
    assert_eq!(
        ObserverAdmissionV18::for_structure(narrow())
            .unwrap()
            .precheck(&ctx, RewriteEvent::BlockErased(block)),
        Err(E::Limit)
    );
}

#[test]
fn arity_and_overflow_are_not_hidden_by_a_smaller_row_count() {
    let mut census = narrow();
    census.operands = 100;
    census.successors = 50;
    let narrow = ObserverAdmissionV18::for_structure(census).unwrap();
    census.max_operands = 100;
    let wide = ObserverAdmissionV18::for_structure(census).unwrap();
    assert!(wide.event_work().unwrap() > narrow.event_work().unwrap());
    assert!(
        ObserverAdmissionV18::for_structure(StructuralCensus {
            values: usize::MAX,
            operations: 1,
            ..Default::default()
        })
        .is_err()
    );
}
