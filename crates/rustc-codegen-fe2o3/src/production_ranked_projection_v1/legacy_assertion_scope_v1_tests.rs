mod legacy_scope_tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;

    #[test]
    fn same_legacy_owner_preserves_graph_and_canonical_buffers_through_projection_and_attachment() {
        let materialized = assertion_materialized(literal_assertion(true, true, false));
        let graph = materialized.executable();
        let identity = *graph.canonical().identity();
        let functions = graph.module().functions.as_ptr();
        let canonical_buffer = graph.canonical().canonical_bytes().as_ptr();
        let graph_storage = materialized.executable_storage();
        let origins_storage = materialized.assert_origin_storage();
        let program = assertion_project(materialized).unwrap();
        assert_eq!(program.root_count(), 1);
        assert!(program.all_kernel_checks_are_clean());
        assert!(!program.grants_artifact_or_launch_authority());
        assert_eq!(
            program
                .materialized
                .executable()
                .module()
                .functions
                .as_ptr(),
            functions
        );
        assert_eq!(
            program
                .materialized
                .executable()
                .canonical()
                .canonical_bytes()
                .as_ptr(),
            canonical_buffer
        );
        let ProductionRankedSemanticProgramV1 {
            materialized,
            roots,
        } = program;
        let root = roots.into_vec().into_iter().next().unwrap();
        assert!(root.lowering.all_mandatory_reports_are_clean());
        // This source/ranked receipt is the existing checked-attachment input,
        // not a fabricated functional-verification or publication receipt.
        let receipt = materialized_ranked_fixture_receipt_v1(materialized, root);
        let attached = fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks(receipt).unwrap();
        let retained = attached.pre_ranked_executable().unwrap();
        assert_eq!(retained.canonical().identity(), &identity);
        assert_eq!(retained.module().functions.as_ptr(), functions);
        assert_eq!(
            retained.canonical().canonical_bytes().as_ptr(),
            canonical_buffer
        );
        assert!(std::ptr::eq(attached.module(), retained.module()));
        assert!(std::ptr::eq(
            attached.pre_ranked_assert_origins().unwrap().executable(),
            retained
        ));
        assert_eq!(
            attached.pre_ranked_executable_storage(),
            Some(graph_storage)
        );
        assert_eq!(
            attached.pre_ranked_assert_origin_storage(),
            Some(origins_storage)
        );
    }

    #[test]
    fn legacy_scope_requires_both_borrowed_payloads_before_any_callback_or_work() {
        let source = assertion_materialized(literal_assertion(true, true, false));
        let graph = source.executable_storage().retained_storage();
        let origins = source.assert_origin_storage().payload_storage();
        assert!(origins > 0);
        for floor in [0, graph, graph + origins - 1] {
            let mut work = Work::new(7);
            let mut budget = Budget::new(&mut work, floor);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(floor).unwrap();
            let result = with_canonical_assertions_budget_v1(
                &source,
                &mut budget,
                |_| -> Result<(), ProductionRankedProjectionErrorV1> {
                    panic!("an underreserved owner must not reach analysis or the callback")
                },
            );
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(Resource::Accounting)
                ))
            ));
            assert_eq!(
                (budget.work(), budget.storage(), budget.peak_storage()),
                (7, floor, floor)
            );
        }
    }

    #[test]
    fn legacy_unwind_restores_the_floor_and_preserves_history_and_original_payload() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        struct BodyDrop(Arc<AtomicUsize>);
        impl Drop for BodyDrop {
            fn drop(&mut self) {
                assert_eq!(self.0.swap(1, Ordering::SeqCst), 0);
            }
        }
        struct Payload(Arc<AtomicUsize>);
        impl Drop for Payload {
            fn drop(&mut self) {
                assert_eq!(self.0.swap(2, Ordering::SeqCst), 1);
            }
        }
        let source = assertion_materialized(literal_assertion(true, true, false));
        let executable = source.executable() as *const _;
        let floor = source.executable_storage().retained_storage()
            + source.assert_origin_storage().payload_storage()
            + 31;
        let work_limit =
            usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap();
        let storage_limit = crate::production_canonical_phase_policy_v1::STORAGE_LIMIT;
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(floor).unwrap();
        // These deliberately oversized attempts seed history, not a measured
        // threshold for the later inventory/sparse scope.
        let failed_work = match budget.charge_work(work_limit) {
            Err(Resource::Work(limit)) => limit.actual(),
            other => panic!("expected oversized work failure, got {other:?}"),
        };
        assert!(matches!(
            budget.reserve_storage(storage_limit),
            Err(Resource::Storage(_))
        ));
        let failed_storage = floor + storage_limit;
        budget.reserve_storage(storage_limit - floor).unwrap();
        budget.release_storage(storage_limit - floor).unwrap();
        let order = Arc::new(AtomicUsize::new(0));
        let payload = Box::new(Payload(Arc::clone(&order)));
        let payload_address = (&*payload as *const Payload) as usize;
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), ProductionRankedProjectionErrorV1> =
                with_canonical_assertions_budget_v1(&source, &mut budget, |session| {
                    let _drop_before_resume = BodyDrop(Arc::clone(&order));
                    assert!(session.retained_floor_for_test_v1() > floor);
                    let mut facts = session.for_source(ROOT, ROOT);
                    assert_eq!(
                        facts.condition(0, true, SemanticBlockIdV1::from_index(1))?,
                        ProjectedAssertionConditionV1::Bool(true)
                    );
                    std::panic::panic_any(payload)
                });
        }))
        .expect_err("the original panic must propagate out of the private scope");
        let payload = *caught
            .downcast::<Box<Payload>>()
            .expect("original payload type");
        assert_eq!((&*payload as *const Payload) as usize, payload_address);
        assert_eq!(order.load(Ordering::SeqCst), 1);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), storage_limit);
        assert_eq!(budget.failed_storage(), Some(failed_storage));
        assert!(budget.work() > 7);
        assert_eq!(source.executable() as *const _, executable);
        source.semantic_ssa().verify_replay().unwrap();
        drop(payload);
        assert_eq!(order.load(Ordering::SeqCst), 2);
        assert_eq!(work.failed_work(), Some(failed_work));
    }
}
