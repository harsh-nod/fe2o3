use super::*;
use crate::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{cell::Cell, mem::size_of_val, rc::Rc};

const WORK: usize = 100_000_000;
const STORAGE: usize = 100_000_000;
const COUNTS: [u32; 9] = [1, 1, 2, 2, 3, 2, 2, 2, 2];
// Independent literal grammar fixture, including every tagged variant. This is
// historical F2NTR compatibility evidence, not a P8 frame-wrapping technique.
const WORDS: &[u32] = &[
    0, 1, 1, 3, 0, 2, 0, 2, 1, 0, 2, 1, 0, 7, 0, 0, 0, 0, 1, 3, 0, 0, 0, 2, 6, 0, 0, 1, 3, 1, 1, 2,
    0, 2, 6, 1, 0, 0, 0, 0, 4, 0, 1, 1, 0, 2, 0, 5, 1, 0, 2, 0, 2, 6, 1, 1, 1, 2, 1, 3, 0, 0, 0, 0,
    1, 0, 0, 4, 1, 0, 1, 3, 0, 2, 1, 0, 2, 0, 4, 1, 1, 3, 0, 1, 0, 0, 2, 6, 0, 1, 3, 0, 0, 7, 0, 1,
    3, 1, 0, 7, 1, 1, 3, 0, 0, 0, 7, 0, 2, 1, 3, 1, 1, 0, 7, 1, 3,
];
fn body() -> Vec<u8> {
    WORDS.iter().flat_map(|word| word.to_le_bytes()).collect()
}
fn historical() -> Vec<u8> {
    let mut bytes = b"F2NTR1\0\0".to_vec();
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&600u32.to_le_bytes());
    bytes.extend_from_slice(&[0x11; 32]);
    bytes.extend_from_slice(&37u64.to_le_bytes());
    bytes.extend_from_slice(&[0x22; 32]);
    bytes.extend_from_slice(&91u64.to_le_bytes());
    for count in COUNTS {
        bytes.extend_from_slice(&count.to_le_bytes());
    }
    bytes.extend_from_slice(&body());
    assert_eq!(bytes.len(), 600);
    bytes
}
fn empty() -> Candidate<'static> {
    Candidate {
        functions: &[],
        blocks: &[],
        segments: &[],
        operations: &[],
        definitions: &[],
        definition_outputs: &[],
        uses: &[],
        edges: &[],
        edge_arguments: &[],
    }
}
fn parse(bytes: &[u8], counts: [u32; 9]) -> Result<()> {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(13).unwrap();
    let result =
        read_canonical_kir_occurrence_row_bytes_v1(bytes, counts, &mut budget).map(|view| {
            assert_eq!(view.canonical_row_bytes(), bytes);
            assert_eq!(view.counts(), counts);
            assert!(!view.grants_authority());
        });
    assert_eq!(budget.storage(), 13);
    result
}
fn set_word(bytes: &mut [u8], index: usize, value: u32) {
    bytes[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn neutral_body_is_exact_historical_nine_axis_grammar_without_a_frame() {
    let wire = historical();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(23).unwrap();
    let (old, old_storage) =
        InertCanonicalKirTransitionReceiptV1::decode_with_budget(&wire, &mut budget).unwrap();
    budget
        .reserve_storage(old_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let (encoded, storage) =
        encode_canonical_kir_occurrence_row_bytes_v1(old.candidate(), &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(encoded.counts(), COUNTS);
    assert_eq!(encoded.canonical_row_bytes(), body());
    assert_eq!(encoded.canonical_row_bytes().len(), 468);
    assert_eq!(
        storage.0,
        size_of::<InertCanonicalKirOccurrenceRowBytesV1>() + encoded.bytes.capacity()
    );
    assert!(!encoded.grants_authority());
    {
        let view = read_canonical_kir_occurrence_row_bytes_v1(
            encoded.canonical_row_bytes(),
            COUNTS,
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(view.storage().0).unwrap();
        assert_eq!(view.canonical_row_bytes(), encoded.canonical_row_bytes());
    }
    budget
        .release_storage(size_of::<CanonicalKirOccurrenceRowsRefV1<'_>>())
        .unwrap();
    drop(encoded);
    budget.release_storage(storage.0).unwrap();
    assert_eq!(budget.storage(), floor);
    drop(old);
    budget
        .release_storage(old_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), 23);
}

#[test]
fn every_tag_and_reserved_padding_word_is_checked_by_shared_readers() {
    // Word indices refer to the complete independent literal above.
    for (index, value) in [
        (8, 2),
        (14, 2),
        (15, 1),
        (16, 1),
        (17, 1), // connector
        (21, 2),
        (25, 1),
        (26, 1),
        (30, 2), // origin
        (31, 3),
        (36, 3),
        (38, 1),
        (39, 1), // function definition
        (43, 3),
        (46, 1),
        (50, 3), // block/result definitions
        (57, 3),
        (62, 2),
        (63, 3),
        (65, 1),
        (66, 1),
        (68, 2), // descendants
        (69, 2),
        (74, 2),
        (77, 1),
        (79, 2),
        (82, 1),
        (84, 2), // uses
    ] {
        let mut bytes = body();
        set_word(&mut bytes, index, value);
        assert!(
            matches!(
                parse(&bytes, COUNTS),
                Err(CanonicalKirTransitionReceiptErrorV1::Malformed(_))
            ),
            "word {index}"
        );
    }
}

#[test]
fn both_partitions_reject_gaps_overlap_missing_and_empty_block_segments() {
    for (index, value) in [
        (4, 1),
        (5, 0),
        (5, 1),
        (5, 3),
        (41, 1),
        (42, 0),
        (48, 0),
        (49, 1),
        (55, 0),
        (56, 0),
        (56, 2),
    ] {
        let mut bytes = body();
        set_word(&mut bytes, index, value);
        assert!(
            matches!(
                parse(&bytes, COUNTS),
                Err(CanonicalKirTransitionReceiptErrorV1::RangePartition)
            ),
            "word {index}"
        );
    }
}

#[test]
fn all_truncations_extra_bytes_and_each_count_substitution_refuse() {
    let bytes = body();
    for end in 0..bytes.len() {
        assert!(parse(&bytes[..end], COUNTS).is_err());
    }
    let mut extra = bytes.clone();
    extra.push(0);
    assert!(parse(&extra, COUNTS).is_err());
    for axis in 0..9 {
        let mut counts = COUNTS;
        counts[axis] += 1;
        assert!(parse(&bytes, counts).is_err());
        counts[axis] = u32::MAX;
        assert!(parse(&bytes, counts).is_err());
    }
}

#[test]
fn cap_and_empty_grammar_are_distinct_from_semantic_admission() {
    assert!(parse(&[], [0; 9]).is_ok());
    let bytes = vec![0; MAX_CANONICAL_KIR_TRANSITION_RECEIPT_BYTES_V1 + 1];
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    assert!(matches!(
        read_canonical_kir_occurrence_row_bytes_v1(&bytes, [0; 9], &mut budget),
        Err(CanonicalKirTransitionReceiptErrorV1::Limit)
    ));
    assert_eq!((budget.work(), budget.peak_storage()), (1, 0));
    let mut bytes = body();
    set_word(&mut bytes, 0, u32::MAX); // Coordinates are not graph-checked here.
    assert!(parse(&bytes, COUNTS).is_ok());
}

fn observe_read(
    bytes: &[u8],
    work_limit: usize,
    storage_limit: usize,
) -> (bool, usize, usize, Option<usize>) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(19).unwrap();
    budget.charge_work(7).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = read_canonical_kir_occurrence_row_bytes_v1(bytes, COUNTS, &mut budget);
    let ok = result.is_ok();
    drop(result);
    assert_eq!(budget.storage(), 19);
    assert!(budget.work_ledger_identity_v1() == ledger);
    (
        ok,
        budget.work(),
        budget.peak_storage(),
        budget.failed_storage(),
    )
}

#[test]
fn reader_exact_one_short_and_repeated_cumulative_limits_preserve_floor() {
    let bytes = body();
    let baseline = observe_read(&bytes, WORK, STORAGE);
    assert!(baseline.0);
    assert_eq!(observe_read(&bytes, baseline.1, baseline.2), baseline);
    assert!(!observe_read(&bytes, baseline.1 - 1, baseline.2).0);
    let short = observe_read(&bytes, baseline.1, baseline.2 - 1);
    assert!(!short.0);
    assert_eq!(short.3, Some(baseline.2));
    let mut work = Work::new(baseline.1 - 7);
    let mut budget = Budget::new(&mut work, STORAGE);
    {
        let _view =
            read_canonical_kir_occurrence_row_bytes_v1(&bytes, COUNTS, &mut budget).unwrap();
    }
    let accepted = budget.work();
    assert!(read_canonical_kir_occurrence_row_bytes_v1(&bytes, COUNTS, &mut budget).is_err());
    assert_eq!((budget.work(), budget.storage()), (accepted, 0));
}

#[test]
fn encoder_exact_one_short_requested_capacity_and_invalid_ranges_restore_floor() {
    let wire = historical();
    let mut import_work = Work::new(WORK);
    let mut import_budget = Budget::new(&mut import_work, STORAGE);
    let (old, _) =
        InertCanonicalKirTransitionReceiptV1::decode_with_budget(&wire, &mut import_budget)
            .unwrap();
    let observe = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(29).unwrap();
        budget.charge_work(11).unwrap();
        let result = encode_canonical_kir_occurrence_row_bytes_v1(old.candidate(), &mut budget);
        let ok = result.is_ok();
        if let Ok((owner, receipt)) = &result {
            assert_eq!(receipt.0, size_of_val(owner) + owner.bytes.capacity());
            assert!(owner.bytes.capacity() >= owner.bytes.len());
        }
        drop(result);
        assert_eq!(budget.storage(), 29);
        (
            ok,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    let baseline = observe(WORK, STORAGE);
    assert!(baseline.0);
    assert_eq!(observe(baseline.1, baseline.2), baseline);
    assert!(!observe(baseline.1 - 1, baseline.2).0);
    assert!(!observe(baseline.1, baseline.2 - 1).0);
    let requested = 29 + size_of::<InertCanonicalKirOccurrenceRowBytesV1>() + 468;
    assert_eq!(observe(WORK, requested - 1).3, Some(requested));
    let blocks = [BlockRow {
        output: Block {
            function: Function(0),
            block: 0,
        },
        segments: Range { start: 0, len: 0 },
    }];
    let mut candidate = empty();
    candidate.blocks = &blocks;
    assert!(encode_canonical_kir_occurrence_row_bytes_v1(candidate, &mut import_budget).is_err());
    assert_eq!(import_budget.storage(), 0);
}

#[test]
fn empty_encoder_has_a_real_header_receipt_even_without_backing_capacity() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, receipt) =
        encode_canonical_kir_occurrence_row_bytes_v1(empty(), &mut budget).unwrap();
    assert!(owner.canonical_row_bytes().is_empty());
    assert_eq!(
        receipt.0,
        size_of::<InertCanonicalKirOccurrenceRowBytesV1>()
    );
    assert_eq!(budget.storage(), 0);
}

#[test]
fn observed_spare_capacity_exact_short_and_below_request_are_checked() {
    let backing = Vec::<u8>::with_capacity(33);
    let observed = backing.capacity();
    assert!(observed >= 33);
    for shortage in [0, 1] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, 17 + observed - shortage);
        budget.reserve_storage(17).unwrap();
        let result = scoped(&mut budget, |budget| {
            budget.reserve_storage(1)?;
            reconcile_capacity(1, observed, budget)
        });
        assert_eq!(result.is_ok(), shortage == 0);
        assert_eq!(budget.storage(), 17);
        if shortage == 1 {
            assert_eq!(budget.failed_storage(), Some(17 + observed));
        }
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    assert!(reconcile_capacity(2, 1, &mut budget).is_err());
    assert_eq!(budget.storage(), 0);
}

struct DropProbe(Rc<Cell<usize>>);
impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
struct ExplodingPayload;
impl Drop for ExplodingPayload {
    fn drop(&mut self) {
        panic!("private destructor probe");
    }
}

#[test]
fn private_scope_partial_backing_error_panic_and_payload_drop_restore_floor() {
    for mode in 0..4 {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, 1000);
        budget.reserve_storage(17).unwrap();
        let drops = Rc::new(Cell::new(0));
        let result = catch_unwind(AssertUnwindSafe(|| {
            scoped::<()>(&mut budget, |budget| {
                budget.charge_work(3)?;
                budget.reserve_storage(8)?;
                let _backing = Vec::from([0u8; 8]);
                let _probe = DropProbe(drops.clone());
                match mode {
                    0 => {
                        budget.reserve_storage(1000)?;
                        Ok(())
                    }
                    1 => Err(Resource::Allocation.into()),
                    2 => panic!("private scope probe"),
                    _ => std::panic::panic_any(ExplodingPayload),
                }
            })
        }));
        assert_eq!(budget.storage(), 17);
        assert_eq!(budget.work(), 3);
        assert_eq!(drops.get(), 1);
        if mode == 3 {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        if mode == 0 {
            assert_eq!(budget.failed_storage(), Some(1025));
        }
    }
}

#[test]
fn private_scope_foreign_ledger_and_undercut_never_refund_invalid_accounting() {
    for mode in 0..3 {
        let mut first_work = Work::new(WORK);
        let mut foreign_work = Work::new(WORK);
        let mut budget = Budget::new(&mut first_work, STORAGE);
        let mut foreign = Budget::new(&mut foreign_work, STORAGE);
        budget.reserve_storage(17).unwrap();
        foreign.reserve_storage(53).unwrap();
        let original = budget.work_ledger_identity_v1();
        let result = scoped::<()>(&mut budget, |budget| {
            budget.reserve_storage(9)?;
            std::mem::swap(budget, &mut foreign);
            match mode {
                0 => Ok(()),
                1 => Err(Resource::Allocation.into()),
                _ => panic!("foreign"),
            }
        });
        assert!(matches!(
            result,
            Err(CanonicalKirTransitionReceiptErrorV1::Resource(
                Resource::Accounting
            ))
        ));
        assert_eq!((budget.storage(), foreign.storage()), (53, 26));
        assert!(foreign.work_ledger_identity_v1() == original);
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(17).unwrap();
    assert!(matches!(
        scoped::<()>(&mut budget, |budget| {
            budget.release_storage(1)?;
            Ok(())
        }),
        Err(CanonicalKirTransitionReceiptErrorV1::Resource(
            Resource::Accounting
        ))
    ));
    assert_eq!(budget.storage(), 16);
}

#[test]
fn private_rejected_value_destructor_cannot_trigger_foreign_refund() {
    let mut own_work = Work::new(WORK);
    let mut other_work = Work::new(WORK);
    let mut budget = Budget::new(&mut own_work, STORAGE);
    let mut other = Budget::new(&mut other_work, STORAGE);
    budget.reserve_storage(17).unwrap();
    other.reserve_storage(41).unwrap();
    let result = scoped(&mut budget, |budget| {
        budget.reserve_storage(3)?;
        std::mem::swap(budget, &mut other);
        Ok(ExplodingPayload)
    });
    assert!(matches!(
        result,
        Err(CanonicalKirTransitionReceiptErrorV1::Resource(
            Resource::Accounting
        ))
    ));
    assert_eq!((budget.storage(), other.storage()), (41, 20));
}
