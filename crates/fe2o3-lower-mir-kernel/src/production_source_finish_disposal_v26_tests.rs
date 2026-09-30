// Independent layout/control equation shared by actual-entry boundary tests.
pub(crate) fn source_owned_finish_header_oracle_v26<T, E>() -> usize {
    type Payload = Box<dyn std::any::Any + Send>;
    type Settlement<'a> = (
        &'a ScopedSourceCleanupV29,
        usize,
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        usize,
        usize,
    );
    size_of::<[Option<Payload>; 2]>()
        + size_of::<std::panic::AssertUnwindSafe<[Option<Payload>; 2]>>()
        + 2 * size_of::<Payload>()
        + size_of::<std::panic::AssertUnwindSafe<Payload>>()
        + 2 * size_of::<Result<(), Payload>>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<usize>()
        + size_of::<bool>()
        + 3 * size_of::<std::thread::Result<Result<T, E>>>()
        + size_of::<SourceOwnedResultV18<()>>()
        + size_of::<Option<ProductionSourceOwnedViewErrorV18>>()
        + size_of::<Option<SourceOwnedQueryFailureV18>>()
        + 2 * size_of::<ProductionSourceOwnedViewErrorV18>()
        + size_of::<std::panic::AssertUnwindSafe<ProductionSourceOwnedViewErrorV18>>()
        + size_of::<std::thread::Result<E>>()
        + 2 * size_of::<Settlement<'_>>()
        + size_of::<&mut ArgumentBudgetV1<'_>>()
        + size_of::<Result<(), ArgumentResourceV1>>()
}

#[test]
fn source_finish_disposal_has_independent_exact_and_one_short_preflight_bounds() {
    type Value = [u64; 257];
    type Error = [u64; 513];
    let header = source_owned_finish_header_oracle_v26::<Value, Error>();
    assert_eq!(
        source_owned_finish_headers_v26::<Value, Error>().unwrap(),
        header
    );
    assert!(header > 3 * size_of::<Error>());
    let work_bound = 32 + 2 + 1 + 4;
    for (work_limit, storage_limit, succeeds) in [
        (work_bound, 23 + header, true),
        (work_bound - 1, 23 + header, false),
        (work_bound, 23 + header - 1, false),
    ] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(23).unwrap();
        let result = source_owned_finish_preflight_v26::<Value, Error>(&mut budget)
            .and_then(|bytes| budget.reserve_storage(bytes));
        assert_eq!(result.is_ok(), succeeds);
        if succeeds {
            assert_eq!((budget.work(), budget.storage()), (work_bound, 23 + header));
            budget.release_storage(header).unwrap();
        } else if work_limit < work_bound {
            assert!(matches!(result, Err(ArgumentResourceV1::Work(_))));
        } else {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(_))));
        }
        assert_eq!(budget.storage(), 23);
    }
}

#[derive(Debug)]
enum SourceFinishErrorV26 {
    Source(ProductionSourceOwnedViewErrorV18),
}
impl From<ProductionSourceOwnedViewErrorV18> for SourceFinishErrorV26 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}

struct SourceFinishDropV26<'a> {
    cleanup: &'a ScopedSourceCleanupV29,
    dropped: &'a std::cell::Cell<bool>,
    deny: bool,
}
impl Drop for SourceFinishDropV26<'_> {
    fn drop(&mut self) {
        self.dropped.set(true);
        if self.deny {
            self.cleanup.deny_refund();
        }
        panic!("source finish rejected output destructor");
    }
}

#[test]
fn source_finish_postflight_disposal_rechecks_cleanup_before_exact_refund() {
    for deny in [false, true] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
        budget.reserve_storage(23).unwrap();
        let boundary =
            ScopedSourceCleanupBoundaryV29::new::<(), SourceFinishErrorV26>(23, &mut budget)
                .unwrap();
        let floor = budget.storage();
        let headers = source_owned_finish_preflight_v26::<
            SourceFinishDropV26<'_>,
            SourceFinishErrorV26,
        >(&mut budget)
        .unwrap();
        budget.reserve_storage(headers).unwrap();
        let paid_work = budget.work();
        let dropped = std::cell::Cell::new(false);
        let result: Result<_, SourceFinishErrorV26> = source_owned_finish_callback_v18(
            Ok(Ok(SourceFinishDropV26 {
                cleanup: &boundary.cleanup,
                dropped: &dropped,
                deny,
            })),
            None,
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "selected postflight refusal",
            )),
            &boundary.cleanup,
            &mut budget,
            headers,
        );
        assert!(dropped.get());
        assert!(matches!(
            result,
            Err(SourceFinishErrorV26::Source(
                ProductionSourceOwnedViewErrorV18::Binding("selected postflight refusal")
            ))
        ));
        assert_eq!(budget.work(), paid_work);
        assert_eq!(budget.storage(), floor + if deny { headers } else { 0 });
        assert_eq!(boundary.cleanup.is_denied(), deny);
    }
}

#[test]
fn source_finish_failed_refund_destroys_returned_value_without_refunding_credit() {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
    budget.reserve_storage(23).unwrap();
    let boundary =
        ScopedSourceCleanupBoundaryV29::new::<(), SourceFinishErrorV26>(23, &mut budget).unwrap();
    let headers =
        source_owned_finish_preflight_v26::<SourceFinishDropV26<'_>, SourceFinishErrorV26>(
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(headers).unwrap();
    let retained = budget.storage();
    let paid_work = budget.work();
    let dropped = std::cell::Cell::new(false);
    let result: Result<_, SourceFinishErrorV26> = source_owned_finish_callback_v18(
        Ok(Ok(SourceFinishDropV26 {
            cleanup: &boundary.cleanup,
            dropped: &dropped,
            deny: false,
        })),
        None,
        Ok(()),
        &boundary.cleanup,
        &mut budget,
        retained + 1,
    );
    assert!(dropped.get());
    assert!(matches!(
        result,
        Err(SourceFinishErrorV26::Source(
            ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
        ))
    ));
    assert!(boundary.cleanup.is_denied());
    assert_eq!((budget.work(), budget.storage()), (paid_work, retained));
}
