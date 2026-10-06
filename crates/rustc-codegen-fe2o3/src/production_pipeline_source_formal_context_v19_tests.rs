use super::*;

#[test]
fn report_binding_components_reject_every_original_descriptor_or_entry_substitution() {
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Signature, Terminator,
    };
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticKernelBindingIdentityV1, SemanticKernelEntryV1, SemanticKernelSourceContractV1,
        SemanticLinkSymbolV1,
    };
    let entry = |symbol: &str| {
        SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(symbol.as_bytes().to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([7; 32]),
            SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
        )
    };
    let function = |id: &str| {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: Vec::new() });
        Function::kernel_entry(
            id,
            Signature::new(Vec::new(), Vec::new()),
            Vec::new(),
            vec![block],
        )
    };
    let kernel = |id: &str, function: &str| {
        Kernel::new(
            id,
            function,
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        )
    };
    check_root_names(
        &entry("original"),
        &kernel("original", "original"),
        &function("original"),
        [7; 32],
        "original",
    )
    .unwrap();
    for (source, physical, actual, binding, descriptor) in [
        (
            entry("other"),
            kernel("original", "original"),
            function("original"),
            [7; 32],
            "original",
        ),
        (
            entry("original"),
            kernel("other", "original"),
            function("original"),
            [7; 32],
            "original",
        ),
        (
            entry("original"),
            kernel("original", "other"),
            function("original"),
            [7; 32],
            "original",
        ),
        (
            entry("original"),
            kernel("original", "original"),
            function("other"),
            [7; 32],
            "original",
        ),
        (
            entry("original"),
            kernel("original", "original"),
            function("original"),
            [8; 32],
            "original",
        ),
        (
            entry("original"),
            kernel("original", "original"),
            function("original"),
            [7; 32],
            "other",
        ),
    ] {
        let error = check_root_names(&source, &physical, &actual, binding, descriptor).unwrap_err();
        assert!(
            matches!(error, Error::Pipeline(error) if matches!(*error, ProductionPipelineError::Geometry(crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure)))
        );
    }
}

#[test]
fn coordinate_preflight_work_is_bounded_by_fields_axes_and_actual_target_table() {
    for profile in [TargetProfile::Gfx942, TargetProfile::Gfx950] {
        let bytes = profile.device_target().len();
        let expected =
            5 * fe2o3_amd_target::KNOWN_PROCESSORS.len() * (bytes + 1) + 8 * bytes + 128 + 3 * 32;
        assert_eq!(coordinate_work(profile).unwrap(), expected);
        let mut exact_work = Work::new(expected);
        let mut exact = Budget::new(&mut exact_work, 0);
        exact
            .charge_work(coordinate_work(profile).unwrap())
            .unwrap();
        assert_eq!(exact.work(), expected);
        let mut short_work = Work::new(expected - 1);
        let mut short = Budget::new(&mut short_work, 0);
        let error = short
            .charge_work(coordinate_work(profile).unwrap())
            .unwrap_err();
        assert!(matches!(error, Resource::Work(_)));
        assert_eq!(short.work(), 0);
        assert_eq!(short.check_prior_denials_v1(), Err(error));
    }
}

#[test]
fn report_policy_header_accounts_for_actual_new_owned_carriers() {
    fn consumer(_: &Report<'_, '_>, _: &Report<'_, '_>, _: &mut Budget<'_>) -> Result<(), Error> {
        Ok(())
    }
    fn check<F>(_: F)
    where
        F: for<'before, 'input, 'after, 'output, 'work> FnMut(
            &Report<'before, 'input>,
            &Report<'after, 'output>,
            &mut Budget<'work>,
        ) -> Result<(), Error>,
    {
        let expected = entry_headers_for_handoff::<(), F, IntegerHandoff<'static, 'static>>()
            .unwrap()
            + size_of::<Vec<Launch>>()
            + size_of::<Option<F>>()
            + size_of::<Result<(), Error>>()
            + size_of::<std::thread::Result<Result<(), Error>>>()
            + size_of::<crate::production_geometry_v1::ProductionCoordinateGeometryV19>()
            + size_of::<fe2o3_amd_target::AmdTargetCapabilities>()
            + size_of::<ReportOptimizationErrorV19>()
            + align_of::<ReportOptimizationErrorV19>()
            + size_of::<ProductionPipelineError>()
            + align_of::<ProductionPipelineError>();
        assert_eq!(
            <OriginalFormalReportsV19 as SourceHandoffPolicyV29<(), F>>::entry_headers().unwrap(),
            expected
        );
    }
    check(consumer);
}

#[test]
fn source_visit_capture_has_independent_exact_and_short_owned_aligned_bounds() {
    #[repr(align(256))]
    struct Large([u8; 8192]);
    let make = || {
        let capture = Large([7; 8192]);
        let pending = PendingConsumerV19::new(move || capture);
        move || drop(pending)
    };
    let visit = make();
    let exact = 2 * std::mem::size_of_val(&visit) + std::mem::align_of_val(&visit);
    assert!(exact >= 2 * size_of::<Large>());
    for (work_limit, storage_limit, expected) in [
        (exact, exact, 0),
        (exact - 1, exact, 1),
        (exact, exact - 1, 2),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        let result = pay_source_visit_capture_v19(&visit, &mut budget);
        match (expected, result) {
            (0, Ok(())) => assert_eq!((budget.work(), budget.storage()), (exact, exact)),
            (1, Err(Resource::Work(error))) => {
                assert_eq!(error.actual(), exact);
                assert_eq!(error.limit(), exact - 1);
                assert_eq!(budget.storage(), 0);
            }
            (2, Err(Resource::Storage(error))) => {
                assert_eq!(error.actual(), exact);
                assert_eq!(error.limit(), exact - 1);
                assert_eq!(budget.storage(), 0);
            }
            other => panic!("unexpected exact capture boundary: {other:?}"),
        }
    }
    let payload = Large([9; 8192]);
    assert_eq!(payload.0[0], 9);
    visit();
}

#[test]
fn source_visit_header_refusal_survives_owned_capture_drop_panic() {
    struct PanicDrop<'a>(&'a std::cell::Cell<usize>);
    impl Drop for PanicDrop<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            panic!("rejected compiler capture");
        }
    }
    for storage in [false, true] {
        let dropped = std::cell::Cell::new(0);
        let value = PanicDrop(&dropped);
        let pending = PendingConsumerV19::new(move || drop(value));
        let visit = move || drop(pending);
        let size = 2 * std::mem::size_of_val(&visit) + std::mem::align_of_val(&visit);
        assert!(size > 0);
        let mut work = Work::new(if storage { size } else { size - 1 });
        let mut budget = Budget::new(&mut work, if storage { size - 1 } else { size });
        let result = pay_source_visit_capture_v19(&visit, &mut budget);
        let error = result.unwrap_err();
        drop(visit);
        assert_eq!(dropped.get(), 1);
        assert_eq!(budget.check_prior_denials_v1(), Err(error));
        match error {
            Resource::Work(value) if !storage => {
                assert_eq!(value.actual(), size);
                assert_eq!(value.limit(), size - 1);
            }
            Resource::Storage(value) if storage => {
                assert_eq!(value.actual(), size);
                assert_eq!(value.limit(), size - 1);
            }
            other => panic!("lost original capture refusal: {other:?}"),
        }
    }
}

#[test]
fn pending_source_consumer_transfers_once_and_contains_preparation_refusal_teardown() {
    struct Count<'a>(&'a std::cell::Cell<usize>, bool);
    impl Drop for Count<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            assert!(!self.1, "preparation capture");
        }
    }
    let dropped = std::cell::Cell::new(0);
    let value = Count(&dropped, false);
    let mut pending = PendingConsumerV19::new(move || drop(value));
    let consumed = pending.take();
    drop(pending);
    assert_eq!(dropped.get(), 0);
    consumed();
    assert_eq!(dropped.get(), 1);
    let fail_preparation = || -> Result<(), Error> {
        let value = Count(&dropped, true);
        let _pending = PendingConsumerV19::new(move || drop(value));
        Err(Error::Unsupported("exact preparation refusal"))
    };
    assert!(matches!(
        fail_preparation(),
        Err(Error::Unsupported("exact preparation refusal"))
    ));
    assert_eq!(dropped.get(), 2);
}
