use super::*;
use crate::private_cell_promotion_resources_v1::Meter as ScopeMeter;
use crate::{
    CanonicalRefinedForwardingHistoryErrorV1 as PrefixError,
    CheckedCanonicalRefinedForwardingHistoryV1 as PrefixChecked, CheckedLoopUnrollHistoryV1,
};
use fe2o3_kernel_analysis::{
    CanonicalKirCrossBlockForwardingErrorV1 as ForwardingError,
    CheckedCanonicalKirLoopUnrollPairV1 as UnrollPair, check_canonical_kir_loop_unroll_pair_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkLedgerIdentityV1, CanonicalKirControlFlowScopeErrorV1 as FlowError,
};
use std::{any::Any, mem::align_of};

// Typed scope state only: no production reservation or cleanup algorithm.
struct ScopeLayout<'a, 'w> {
    _budget: &'a mut Budget<'w>,
    _slot: usize,
    _ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    _floor: usize,
    _live: usize,
    _cleanup: bool,
    _failed: bool,
    _nested_panic: Option<Box<dyn Any + Send>>,
}
fn scope_header() -> usize {
    let header = size_of::<ScopeLayout<'_, '_>>();
    let alignment = align_of::<ScopeLayout<'_, '_>>();
    assert_eq!(header, size_of::<ScopeMeter<'_, '_, Error>>());
    assert_eq!(header, size_of::<ScopeMeter<'_, '_, SemanticError>>());
    assert_eq!(alignment, align_of::<ScopeMeter<'_, '_, Error>>());
    assert_eq!(alignment, align_of::<ScopeMeter<'_, '_, SemanticError>>());
    header
}

struct Measured {
    result: Result<usize, SemanticError>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn check_capacity<C: rows::Coordinate>(bytes: &[u8], family: u8, b: &mut Budget<'_>) {
    let floor = b.storage();
    let result: Result<(), Error> = resources::scoped(b, |meter| {
        meter.reserve(size_of::<Vec<Origin<C>>>() + rows::SCRATCH)?;
        let (values, storage) = rows::decode::<C>(bytes, family, meter)?;
        assert_eq!(storage, values.capacity() * size_of::<Origin<C>>());
        assert_eq!(values.len(), rows::header(bytes, family)?);
        drop(values);
        meter.release(storage)?;
        Ok(())
    });
    result.unwrap();
    assert_eq!(b.storage(), floor);
}
// Decoded fixture backing remains actually live outside each isolated budget
// measurement; its complete real receipt/floor is prepaid on that measurement.
fn observe(
    decoded: &DecodedLoopUnrollHistoryV1<'_, '_>,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> Measured {
    let sibling = vec![0x53u8; 37];
    let floor = floor + sibling.capacity() + size_of::<Vec<u8>>();
    let mut work = Work::new(work_limit);
    let (result, accepted, peak, failed_storage) = {
        let mut b = Budget::new(&mut work, storage_limit);
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        b.charge_work(17).unwrap();
        let result = decoded.check_semantics(&mut b).and_then(|checked| {
            let retained = checked.storage().retained_storage();
            b.reserve_storage(retained)
                .map_err(SemanticError::Resource)?;
            let result = checked.replay(&mut b).map(|()| retained);
            drop(checked);
            b.release_storage(retained).unwrap();
            result
        });
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(sibling, [0x53u8; 37]);
        (result, b.work(), b.peak_storage(), b.failed_storage())
    };
    Measured {
        result,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}

// Independent public-stage composition, never U check_semantics or U replay.
// The two actual borrowed children stay paid through the direct F replay;
// public type sizes account separately for their enclosing U metadata.
fn prefix_replay_reference(
    decoded: &DecodedLoopUnrollHistoryV1<'_, '_>,
    inherited: usize,
    work_limit: usize,
    storage_limit: usize,
) -> Measured {
    let sibling = vec![0x53u8; 37];
    let floor = inherited + sibling.capacity() + size_of::<Vec<u8>>();
    let header = scope_header();
    let wrapper = size_of::<CheckedLoopUnrollHistoryV1<'_>>()
        .checked_sub(size_of::<PrefixChecked<'_>>())
        .and_then(|n| n.checked_sub(size_of::<UnrollPair<'_, '_, '_>>()))
        .unwrap();
    let mut work = Work::new(work_limit);
    let (result, accepted, peak, failed_storage) = {
        let mut b = Budget::new(&mut work, storage_limit);
        b.reserve_storage(floor).unwrap();
        let ledger = b.work_ledger_identity_v1();
        b.charge_work(17).unwrap();
        b.reserve_storage(header).unwrap();
        b.reserve_storage(wrapper).unwrap();
        let prefix = decoded.prefix().check_semantics(&mut b).unwrap();
        let ps = prefix.storage().retained_storage();
        b.reserve_storage(ps).unwrap();
        let (pair, pair_storage) = check_canonical_kir_loop_unroll_pair_v1(
            prefix.output(),
            decoded.output(),
            decoded.origins(),
            decoded.limits(),
            &mut b,
        )
        .unwrap();
        let us = pair_storage.retained_storage();
        b.reserve_storage(us).unwrap();
        let retained = wrapper.checked_add(ps).unwrap().checked_add(us).unwrap();
        b.charge_work(1).unwrap();
        // Ending initial admission removes only its scope state. The returned
        // receipt's children and metadata remain continuously reserved here.
        b.release_storage(header).unwrap();
        assert_eq!(b.storage(), floor + retained);
        assert_eq!(b.failed_storage(), None);

        // U replay begins a fresh scope before invoking the admitted F replay.
        b.reserve_storage(header).unwrap();
        let replay_floor = b.storage();
        let result = prefix
            .replay(prefix.limits(), &mut b)
            .map(|()| retained)
            .map_err(SemanticError::Prefix);
        assert_eq!(b.storage(), replay_floor);
        drop(pair);
        drop(prefix);
        b.release_storage(header + retained).unwrap();
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(sibling, [0x53u8; 37]);
        (result, b.work(), b.peak_storage(), b.failed_storage())
    };
    Measured {
        result,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}

fn forwarding_flow_storage(result: Result<usize, SemanticError>) -> (usize, usize) {
    let Err(SemanticError::Prefix(PrefixError::Forwarding(ForwardingError::ControlFlow(
        FlowError::Resource(Resource::Storage(error)),
    )))) = result
    else {
        panic!("exact admitted F replay forwarding CFG Storage phase");
    };
    (error.actual(), error.limit())
}
#[test]
fn unroll_history_exact_limits_and_final_work_short_are_typed() {
    for bound in [None, Some(3)] {
        let bytes = encoded(bound);
        with_decoded(bytes.canonical_bytes(), |decoded, b| {
            let measured = observe(decoded, b.storage(), WORK, STORAGE);
            let retained = measured.result.unwrap();
            assert_eq!(
                (measured.failed_work, measured.failed_storage),
                (None, None)
            );
            let exact = observe(decoded, b.storage(), measured.work, measured.peak);
            assert_eq!(exact.result.unwrap(), retained);
            assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
            let short = observe(decoded, b.storage(), measured.work - 1, measured.peak);
            let Err(SemanticError::Resource(Resource::Work(error))) = short.result else {
                panic!("final U-history replay charge must be Work");
            };
            assert_eq!(
                (error.actual(), error.limit()),
                (measured.work, measured.work - 1)
            );
            assert_eq!(
                (
                    short.work,
                    short.peak,
                    short.failed_work,
                    short.failed_storage
                ),
                (measured.work - 1, measured.peak, Some(measured.work), None)
            );
        });
    }
}
#[test]
fn unroll_history_initial_header_denial_precedes_nested_traversal() {
    let bytes = encoded(Some(3));
    let floor = bytes.storage().retained_storage();
    let header = scope_header();
    let first_reservation = floor.checked_add(header).unwrap();
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, floor);
    b.reserve_storage(floor).unwrap();
    let error = read_loop_unroll_history_v1(bytes.canonical_bytes(), &mut b)
        .err()
        .unwrap();
    let Error::Resource(Resource::Storage(error)) = error else {
        panic!("first scope reservation");
    };
    assert_eq!((error.actual(), error.limit()), (first_reservation, floor));
    assert_eq!(
        (b.work(), b.storage(), b.peak_storage(), b.failed_storage()),
        (0, floor, floor, Some(first_reservation))
    );
    assert_eq!(work.failed_work(), None);
    let limit = floor + header + size_of::<InertLoopUnrollHistoryRefV1<'_>>() + SCRATCH - 1;
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, limit);
    b.reserve_storage(floor).unwrap();
    let Error::Resource(Resource::Storage(error)) =
        read_loop_unroll_history_v1(bytes.canonical_bytes(), &mut b)
            .err()
            .unwrap()
    else {
        panic!("new frame/scratch reservation");
    };
    assert_eq!((error.actual(), error.limit()), (limit + 1, limit));
    assert_eq!(
        (b.work(), b.storage(), b.peak_storage(), b.failed_storage()),
        (0, floor, floor + header, Some(limit + 1))
    );
    assert_eq!(work.failed_work(), None);
}
#[test]
fn unroll_history_row_capacity_floor_and_sibling_cleanup_are_exact() {
    let bytes = encoded(Some(3));
    with_decoded(bytes.canonical_bytes(), |decoded, b| {
        let sibling = vec![0x91u8; 29];
        let sibling_storage = sibling.capacity() + size_of::<Vec<u8>>();
        b.reserve_storage(sibling_storage).unwrap();
        let floor = b.storage();
        let ledger = b.work_ledger_identity_v1();
        let checked = decoded.check_semantics(b).unwrap();
        let retained = checked.storage().retained_storage();
        b.reserve_storage(retained).unwrap();
        let full = b.storage();
        b.release_storage(1).unwrap();
        assert!(matches!(
            checked.replay(b),
            Err(SemanticError::Resource(Resource::Accounting))
        ));
        b.reserve_storage(1).unwrap();
        assert_eq!(b.storage(), full);
        let first_storage = b.storage_limit() + 1;
        assert!(matches!(
            b.reserve_storage(first_storage),
            Err(Resource::Storage(_))
        ));
        let first = b.failed_storage();
        checked.replay(b).unwrap();
        assert_eq!(b.failed_storage(), first);
        assert_eq!(b.storage(), full);
        drop(checked);
        b.release_storage(retained).unwrap();
        // The existing scoped Meter must restore this same ledger after unwind.
        let result: Result<(), Error> = resources::scoped(b, |meter| {
            meter.reserve(41)?;
            meter.work(3)?;
            panic!("U codec fixture unwind");
        });
        assert!(matches!(result, Err(Error::Panicked)));
        assert_eq!(b.storage(), floor);
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(sibling, [0x91u8; 29]);
        assert_eq!(
            decoded.origins().blocks.len() * size_of::<Origin<Block>>(),
            std::mem::size_of_val(decoded.origins().blocks)
        );
        drop(sibling);
        b.release_storage(sibling_storage).unwrap();
    });
    // Exact real capacities, including conservative nested owning headers.
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    b.reserve_storage(bytes.storage().retained_storage())
        .unwrap();
    let frame = read_loop_unroll_history_v1(bytes.canonical_bytes(), &mut b).unwrap();
    b.reserve_storage(frame.storage().retained_storage())
        .unwrap();
    let decoded = materialize_loop_unroll_history_v1(&frame, &mut b).unwrap();
    let ds = decoded.storage().retained_storage();
    b.reserve_storage(ds).unwrap();
    assert!(
        ds > size_of::<DecodedLoopUnrollHistoryV1<'_, '_>>()
            + decoded.prefix().storage().retained_storage()
    );
    check_capacity::<Block>(frame.fields[2], 0, &mut b);
    check_capacity::<Definition>(frame.fields[3], 1, &mut b);
    check_capacity::<Site>(frame.fields[4], 2, &mut b);
    check_capacity::<Block>(frame.fields[5], 3, &mut b);
    check_capacity::<Edge>(frame.fields[6], 4, &mut b);
    check_capacity::<Argument>(frame.fields[7], 5, &mut b);
    drop(decoded);
    drop(frame);
    b.release_storage(b.storage() - bytes.storage().retained_storage())
        .unwrap();
}
#[test]
fn unroll_history_storage_short_matches_independent_prefix_replay() {
    for bound in [None, Some(3)] {
        let bytes = encoded(bound);
        with_decoded(bytes.canonical_bytes(), |decoded, b| {
            let measured = observe(decoded, b.storage(), WORK, STORAGE);
            let retained = measured.result.unwrap();
            assert!(retained > 0);
            let reference =
                prefix_replay_reference(decoded, b.storage(), measured.work, measured.peak - 1);
            let short = observe(decoded, b.storage(), measured.work, measured.peak - 1);
            let expected = forwarding_flow_storage(reference.result);
            assert_eq!(forwarding_flow_storage(short.result), expected);
            assert_eq!(expected, (measured.peak, measured.peak - 1));
            assert_eq!(reference.failed_work, None);
            assert_eq!(reference.failed_storage, Some(expected.0));
            assert_eq!(
                (
                    short.work,
                    short.peak,
                    short.failed_work,
                    short.failed_storage
                ),
                (reference.work, reference.peak, None, Some(expected.0))
            );
            assert!(short.work <= measured.work);
            assert!(short.peak < measured.peak);
        });
    }
}
