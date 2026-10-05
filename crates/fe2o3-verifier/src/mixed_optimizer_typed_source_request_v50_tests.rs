use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[test]
fn typed_source_request_runtime_identity_binds_launch_rank_extents_width_and_endian() {
    let measure = |roots, launches: &[ExplicitLaunchExtent], width, endian, limit| {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 0);
        let result = runtime_identity(roots, launches, width, endian, &mut budget);
        assert_eq!(budget.storage(), 0);
        (result, budget.work())
    };
    let launches = [ExplicitLaunchExtent::Exact {
        rank: 1,
        extents: [64, 1, 1],
    }; 2];
    let domain = b"FE2O3/TYPED-SOURCE-TAIL/RUNTIME/V50\0";
    let work = 8 + 8 + domain.len() + 8 + 2 + 2 * (8 + 8 + 1 + 3 * (8 + 8));
    let (result, exact) = measure(
        2,
        &launches,
        FormalIndexWidth::Bits64,
        EndiannessV2::Little,
        work,
    );
    let identity = result.unwrap();
    assert_eq!(exact, work);
    assert_eq!(
        measure(
            2,
            &launches,
            FormalIndexWidth::Bits64,
            EndiannessV2::Little,
            work
        )
        .0
        .unwrap(),
        identity
    );
    let (denied, _) = measure(
        2,
        &launches,
        FormalIndexWidth::Bits64,
        EndiannessV2::Little,
        work - 1,
    );
    assert!(matches!(denied, Err(Error::Resource(Resource::Work(error)))
        if error.limit() == work - 1 && error.actual() == work));
    for (width, endian) in [
        (FormalIndexWidth::Bits32, EndiannessV2::Little),
        (FormalIndexWidth::Bits64, EndiannessV2::Big),
    ] {
        assert_ne!(
            measure(2, &launches, width, endian, work).0.unwrap(),
            identity
        );
    }
    for changed in [
        ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [65, 1, 1],
        },
        ExplicitLaunchExtent::Exact {
            rank: 2,
            extents: [64, 1, 1],
        },
    ] {
        assert_ne!(
            measure(
                2,
                &[launches[0], changed],
                FormalIndexWidth::Bits64,
                EndiannessV2::Little,
                work
            )
            .0
            .unwrap(),
            identity
        );
    }
    assert!(matches!(
        measure(
            1,
            &launches,
            FormalIndexWidth::Bits64,
            EndiannessV2::Little,
            work
        )
        .0,
        Err(Error::Binding("typed tail complete native launch census"))
    ));
    assert!(matches!(
        measure(
            2,
            &launches,
            FormalIndexWidth::Unknown,
            EndiannessV2::Little,
            work
        )
        .0,
        Err(Error::Binding("typed tail INDEX width"))
    ));
    for extents in [[0, 1, 1], [64, 2, 1], [u64::from(u32::MAX) + 1, 1, 1]] {
        assert!(matches!(
            measure(
                1,
                &[ExplicitLaunchExtent::Exact { rank: 1, extents }],
                FormalIndexWidth::Bits32,
                EndiannessV2::Little,
                work
            )
            .0,
            Err(Error::Binding("typed tail native launch extent"))
        ));
    }
}

#[test]
fn typed_source_request_headers_include_four_graphs_complete_census_and_owned_bytes() {
    type SubjectFields = (
        [u8; 32],
        [u8; 32],
        [Identity; 4],
        [u8; 32],
        [u8; 32],
        [u8; 32],
        [usize; 6],
    );
    type RequestFields = (
        &'static Source<'static>,
        &'static Native<'static, 'static, 'static, 'static>,
        CanonicalGeneratedVerusProofInputV3,
        TypedSourceTailSubjectV50,
        EndiannessV2,
        &'static [()],
        usize,
        usize,
    );
    type R = Result<Request<'static, 'static, 'static, 'static, 'static>>;
    type C = (
        &'static Source<'static>,
        &'static Native<'static, 'static, 'static, 'static>,
        EndiannessV2,
        &'static [()],
        usize,
    );
    assert_eq!(
        size_of::<TypedSourceTailSubjectV50>(),
        size_of::<SubjectFields>()
    );
    assert_eq!(
        size_of::<Request<'static, 'static, 'static, 'static, 'static>>(),
        size_of::<RequestFields>()
    );
    let expected = 2 * SOURCE_LIMIT
        + size_of::<RequestFields>()
        + align_of::<RequestFields>()
        + 2 * size_of::<R>()
        + size_of::<std::thread::Result<R>>()
        + size_of::<C>()
        + align_of::<C>()
        + size_of::<std::panic::AssertUnwindSafe<C>>()
        + PrefixView::inspection_storage_v29().unwrap()
        + 4 * size_of::<Inventory<'_>>()
        + 4 * size_of::<fe2o3_kernel_analysis::CanonicalKirInventoryStorageV1>()
        + size_of::<fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<'_, '_, '_, '_>>()
        + size_of::<Pair<'_>>()
        + size_of::<fe2o3_kernel_analysis::CheckedCanonicalKirCrossBlockForwardingV18<'_>>()
        + size_of::<fe2o3_kernel_analysis::CanonicalKirCrossBlockForwardingStorageV1>()
        + size_of::<fe2o3_kernel_analysis::CanonicalKirLicmStorageV1>()
        + size_of::<Writer<'_, '_>>()
        + size_of::<Result<Writer<'_, '_>>>()
        + size_of::<(String, [usize; 6])>()
        + size_of::<Result<(String, [usize; 6])>>()
        + size_of::<SubjectFields>()
        + 2 * size_of::<Sha256>()
        + size_of::<[Identity; 4]>()
        + 6 * size_of::<usize>()
        + 8 * size_of::<&()>();
    assert_eq!(headers().unwrap(), expected);
}
