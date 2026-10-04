use super::*;
use std::mem::align_of;

#[test]
fn selected_slice_scope_named_captures_have_an_independent_frame_oracle() {
    type Callback = [u64; 3];
    type Output = [u64; 5];
    type View<'a> = CheckedCanonicalSelectedSliceDomainsV30<'a, 'a>;
    type Capture<'a> = (
        &'a VerifiedCanonicalKernelIrModuleV18,
        &'a [ExplicitLaunchExtent],
        FormalIndexWidth,
        CanonicalGuardedGlobalReadLimitsV1,
        Option<Callback>,
    );
    type CallbackFrame<'a> = (&'a View<'a>, &'a mut Budget<'a>, Callback);
    type ExecutionFrame<'a> = (
        &'a mut scope::SelectedScopeCaptureV30<'a, 'a, Callback>,
        &'a mut Budget<'a>,
        &'a Cell<bool>,
        usize,
        CanonicalKernelIrWorkLedgerIdentityV1,
    );
    type Query<'a> = (
        &'a View<'a>,
        &'a mut Budget<'a>,
        FunctionCoordinate,
        Coordinate,
        Option<&'a CanonicalSelectedSliceAccessV30>,
        Option<&'a [CanonicalSelectedSliceChoiceV30]>,
        &'a [CanonicalSelectedPointerNodeV30],
        &'a [CanonicalSelectedPointerIncomingV30],
        &'a [CanonicalSelectedSliceAccessV30],
        &'a [Option<CanonicalSelectedSliceParameterV30>],
        Option<usize>,
        Range<usize>,
        Failure,
        &'a Coordinate,
    );
    type Scope<'a> = (
        &'a mut Budget<'a>,
        &'a Cell<bool>,
        Accounting,
        Cell<bool>,
        &'a VerifiedCanonicalKernelIrModuleV18,
        Vec<SelectedFunctionV30>,
        Option<SelectedFunctionV30>,
        View<'a>,
        CanonicalKernelIrWorkLedgerIdentityV1,
        [usize; 6],
        Box<dyn std::any::Any + Send>,
    );
    assert_eq!(
        size_of::<scope::SelectedScopeCaptureV30<'_, '_, Callback>>(),
        size_of::<Capture<'_>>()
    );
    assert_eq!(
        size_of::<scope::SelectedCallbackFrameV30<'_, '_, '_, '_, '_, Callback>>(),
        size_of::<CallbackFrame<'_>>()
    );
    assert_eq!(
        size_of::<scope::SelectedExecutionFrameV30<'_, '_, '_, '_, '_, '_, Callback>>(),
        size_of::<ExecutionFrame<'_>>()
    );
    fn carrier<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    let expected = carrier::<Query<'_>>()
        + carrier::<Scope<'_>>()
        + carrier::<Capture<'_>>()
        + carrier::<CallbackFrame<'_>>()
        + carrier::<ExecutionFrame<'_>>()
        + carrier::<Option<Output>>()
        + carrier::<Result<Output>>()
        + 2 * size_of::<std::thread::Result<Result<Output>>>()
        + 2 * size_of::<std::thread::Result<Result<Option<Output>>>>()
        + 2 * align_of::<Capture<'_>>()
        + 2 * align_of::<CallbackFrame<'_>>()
        + 2 * align_of::<ExecutionFrame<'_>>();
    assert_eq!(scope::headers::<Output, Callback>().unwrap(), expected);
}

#[test]
fn selected_slice_retained_graph_storage_uses_actual_capacity_not_declared_limits() {
    for recurrence in [false, true] {
        let module = fixture(true, true, true, recurrence);
        let (result, _, _) = run(&module, 100_000_000, 100_000_000, |batch, budget| {
            let before = budget.storage();
            let function = &batch.functions[0];
            let expected = function.graph.nodes.capacity()
                * size_of::<CanonicalSelectedPointerNodeV30>()
                + function.graph.incoming.capacity()
                    * size_of::<CanonicalSelectedPointerIncomingV30>()
                + function.graph.terminal.capacity() * size_of::<Option<usize>>()
                + function.graph.seeded.capacity() * size_of::<bool>()
                + function.parameters.capacity()
                    * size_of::<Option<CanonicalSelectedSliceParameterV30>>()
                + function.accesses.capacity() * size_of::<CanonicalSelectedSliceAccessV30>()
                + function.choices.capacity() * size_of::<CanonicalSelectedSliceChoiceV30>();
            assert_eq!(function.retained_bytes()?, expected);
            assert!(expected < 100_000);
            assert_eq!(inspect(batch, budget)?, 6);
            assert_eq!(budget.storage(), before);
            Ok(())
        });
        assert!(matches!(result, Ok(Some(()))), "{result:?}");
    }
}

#[test]
fn selected_slice_scope_cannot_erase_first_query_failure_or_refund_lost_custody() {
    let module = fixture(true, false, true, true);
    let (owner, credit) = owner(&module);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = Budget::new(&mut work, 100_000_000);
    budget.reserve_storage(credit + 17).unwrap();
    let retained = Cell::new(0usize);
    let result = with_canonical_selected_slice_domains_v30(
        &owner,
        &[launch()],
        FormalIndexWidth::Bits64,
        Default::default(),
        &mut budget,
        |batch, budget| {
            retained.set(budget.storage());
            budget.release_storage(1).unwrap();
            let first = batch.function_count(budget).unwrap_err();
            assert_eq!(first, Failure::Resource(ResourceError::Accounting));
            budget.reserve_storage(1).unwrap();
            assert_eq!(batch.function_count(budget).unwrap_err(), first);
            Ok(())
        },
    );
    assert_eq!(result, Err(Failure::Resource(ResourceError::Accounting)));
    assert_eq!(
        budget.storage(),
        retained.get(),
        "lost custody prohibits credit recovery even after restoration"
    );
}
