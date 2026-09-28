use super::*;

#[test]
fn source_owned_projection_storage_and_work_are_prepaid_at_exact_and_one_short_limits() {
    for limit in [56, 57] {
        let mut work = Work::new(8);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(17).unwrap();
        let result = paid_vec::<u64>(5, &mut budget);
        assert_eq!(budget.work(), 8);
        if limit == 57 {
            let rows = result.unwrap();
            assert_eq!(rows.capacity(), 5);
            assert_eq!(budget.storage(), 57);
            assert_eq!(budget.peak_storage(), 57);
        } else {
            assert!(
                matches!(result, Err(Error::Resource(Resource::Storage(error))) if error.actual() == 57 && error.limit() == 56)
            );
            assert_eq!(budget.storage(), 17);
            assert_eq!(budget.peak_storage(), 17);
            assert_eq!(budget.failed_storage(), Some(57));
        }
    }
    let mut work = Work::new(7);
    let mut budget = Budget::new(&mut work, 57);
    budget.reserve_storage(17).unwrap();
    assert!(
        matches!(paid_vec::<u64>(5, &mut budget), Err(Error::Resource(Resource::Work(error))) if error.actual() == 8 && error.limit() == 7)
    );
    assert_eq!((budget.work(), budget.storage()), (0, 17));
}

#[test]
fn source_owned_projection_overflow_refuses_before_work_or_storage_mutation() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(17).unwrap();
    assert!(matches!(
        paid_vec::<u64>(usize::MAX, &mut budget),
        Err(Error::Resource(Resource::Arithmetic))
    ));
    assert!(matches!(
        paid_vec::<()>(usize::MAX, &mut budget),
        Err(Error::Resource(Resource::Arithmetic))
    ));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (0, 17, 17)
    );
}

#[test]
fn source_owned_entry_header_pays_owned_aligned_capture_and_result_independently() {
    #[repr(align(256))]
    struct Large([u8; 16_384]);
    fn oracle<R, F>(_: &F) -> usize {
        type Invoke<'a, 's, 'w, F> = (
            F,
            &'a Source<'s>,
            &'a Handoff<'a, 's>,
            &'a [AbiRoot<'a>],
            &'a mut Budget<'w>,
        );
        let expected = size_of::<F>()
            + align_of::<F>()
            + size_of::<AssertUnwindSafe<F>>()
            + size_of::<Invoke<'_, '_, '_, F>>()
            + align_of::<Invoke<'_, '_, '_, F>>()
            + size_of::<AssertUnwindSafe<Invoke<'_, '_, '_, F>>>()
            + size_of::<Result<R, Error>>()
            + align_of::<Result<R, Error>>()
            + size_of::<std::thread::Result<Result<R, Error>>>()
            + size_of::<AssertUnwindSafe<Result<R, Error>>>()
            + size_of::<PreparedSsaMaterializationV29>()
            + align_of::<PreparedSsaMaterializationV29>()
            + size_of::<Vec<Class>>()
            + size_of::<Vec<AbiRoot<'_>>>()
            + size_of::<ProductionKernelArgumentAbiInputV18<'_>>()
            + size_of::<ProductionExecutionSourceInputV29<'_>>()
            + size_of::<Work>()
            + size_of::<Budget<'_>>();
        assert_eq!(entry_headers::<R, F>().unwrap(), expected);
        expected
    }
    let empty = || ();
    let owned = Large([7; 16_384]);
    let large = move || owned;
    let small = oracle::<(), _>(&empty);
    let capture = oracle::<(), _>(&large);
    let result = oracle::<Large, _>(&empty);
    assert!(capture > small + 2 * size_of::<Large>());
    assert!(result > small + 2 * size_of::<Large>());
    assert_eq!(large().0[0], 7);
}
