use super::*;
use crate::{
    CanonicalPolicy5SemanticErrorV1 as P5Error, CanonicalPolicy6SemanticErrorV1 as P6Error,
    CanonicalPolicy7SemanticErrorV1 as P7Error, CanonicalPolicy8CompositionErrorV1 as P8Error,
};

fn attempt(p: &Prepared8, wire: &[u8], semantic: bool) -> Result<Summary, Rejection> {
    run(
        wire,
        p.tail.output(),
        p.floor + wire.len() + 31,
        WORK,
        STORAGE,
        semantic,
        false,
    )
    .result
}
fn p7(error: Rejection) -> P7Error {
    match error {
        Rejection::Semantic(P8Error::Policy7(error)) => *error,
        other => panic!("expected P7 semantic refusal: {other:?}"),
    }
}
fn p6(error: Rejection) -> P6Error {
    match p7(error) {
        P7Error::Policy6(error) => *error,
        other => panic!("expected P6 semantic refusal: {other:?}"),
    }
}
fn p5(error: Rejection) -> P5Error {
    match p6(error) {
        P6Error::Policy5(error) => error,
        other => panic!("expected P5 semantic refusal: {other:?}"),
    }
}

#[test]
fn malformed_opaque_prefixes_materialize_but_fail_at_the_existing_nested_stage() {
    let p = prepared(true, true);
    let wire = make(&p);
    for axis in [1, 2, 4, 5, 6] {
        let mut bad = wire.canonical_bytes().to_vec();
        let start = section(&bad, axis).start;
        // Axis 6 changes only its still-opaque embedded P6 record, not P7 syntax.
        bad[start + if axis == 6 { 16 } else { 0 }] ^= 1;
        assert!(attempt(&p, &bad, false).is_ok());
        let error = attempt(&p, &bad, true).err().unwrap();
        match axis {
            1 => assert!(matches!(p5(error), P5Error::Policy4(_))),
            2 => assert!(matches!(p5(error), P5Error::ExecutionClaim)),
            4 => assert!(matches!(p6(error), P6Error::Claim(_))),
            5 => assert!(matches!(p6(error), P6Error::Transition(_))),
            6 => assert!(matches!(p6(error), P6Error::Composition)),
            _ => unreachable!(),
        }
    }
}

#[test]
fn exact_original_p5_row_coordinates_are_not_accepted_as_authority() {
    let p = prepared_module(&crate::checked_load_forwarding_v1::tests::fixture());
    assert_eq!(p.inputs().prefix.prefix.prefix.load_rows.len(), 2);
    let wire = make(&p);
    let mut bad = wire.canonical_bytes().to_vec();
    let start = section(&bad, 3).start + 4;
    // Only the first producer coordinate changes; load ordering remains genuine.
    bad[start..start + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(attempt(&p, &bad, false).is_ok());
    assert!(matches!(
        p5(attempt(&p, &bad, true).err().unwrap()),
        P5Error::Forwarding(_)
    ));
}

fn p7_record(
    p: &Prepared8,
    rows: &[fe2o3_kernel_analysis::CanonicalKirRedundantStoreRowV1],
    retained: &[fe2o3_kernel_analysis::CanonicalKirRedundantStoreRetainedOperationV1],
) -> Vec<u8> {
    encode(
        p.prefix.checked.execution().canonical_bytes(),
        p.prefix.checked.owner(),
        p.prefix.continuation.output(),
        rows,
        retained,
    )
}

#[test]
fn missing_p7_deletion_and_retained_rows_reach_independent_relation_refusal() {
    let p = prepared(true, true);
    let wire = make(&p);
    let rows = p.prefix.continuation.rows();
    let retained = p.prefix.continuation.retained_operations();
    assert!(rows.len() > 1 && retained.len() > 1);
    for record in [
        p7_record(&p, &rows[1..], retained),
        p7_record(&p, rows, &retained[1..]),
    ] {
        let bad = replace_section(wire.canonical_bytes(), 6, &record);
        assert!(attempt(&p, &bad, false).is_ok());
        assert!(matches!(
            p7(attempt(&p, &bad, true).err().unwrap()),
            P7Error::Continuation(_)
        ));
    }
}

#[test]
fn duplicated_reordered_and_wrong_header_p7_rows_fail_in_existing_decoder() {
    let p = prepared(true, true);
    let wire = make(&p);
    let original = p.prefix.continuation.rows();
    assert!(original.len() > 1);
    for mode in 0..3 {
        let mut rows = original.to_vec();
        match mode {
            0 => rows.push(rows[0]),
            1 => rows.swap(0, 1),
            _ => {}
        }
        let mut record = p7_record(&p, &rows, p.prefix.continuation.retained_operations());
        if mode == 2 {
            record[0] ^= 1;
        }
        let bad = replace_section(wire.canonical_bytes(), 6, &record);
        let error = attempt(&p, &bad, false).err().unwrap();
        match error {
            Rejection::Inputs(InputsError::Policy7Rows(error)) => match mode {
                2 => assert!(matches!(*error, P7Error::Record)),
                _ => assert!(matches!(*error, P7Error::Rows)),
            },
            other => panic!("expected row decoder refusal: {other:?}"),
        }
    }
}

#[test]
fn complete_neutral_tail_materialization_does_not_bypass_operation_roster_checks() {
    let p = prepared(true, true);
    let original = p.inputs().continuation.occurrences;
    assert!(original.operations.len() > 1);
    for mode in 0..4 {
        let mut operations = original.operations.to_vec();
        match mode {
            0 => {
                operations.remove(0);
            }
            1 => operations.push(operations[0]),
            2 => operations.swap(0, 1),
            _ => operations[0].output.operation = u32::MAX,
        }
        let mut candidate = original;
        candidate.operations = &operations;
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = p.floor + operations.capacity() * size_of_val(&operations[0]);
        budget.reserve_storage(floor).unwrap();
        let (rows, storage) = encode_rows(candidate, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let bad = literal(inputs(&p, &rows));
        assert!(attempt(&p, &bad, false).is_ok());
        assert!(matches!(
            attempt(&p, &bad, true),
            Err(Rejection::Semantic(P8Error::Continuation(
                CanonicalPolicy8SemanticErrorV1::Continuation(_)
            )))
        ));
        drop(rows);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn wrong_external_k_and_invalid_pool_graph_remain_earlier_refusals() {
    let p = prepared(true, true);
    let other = prepared(false, false);
    let wire = make(&p);
    assert!(matches!(
        run(
            wire.canonical_bytes(),
            other.tail.output(),
            p.floor + other.floor + wire.storage().retained_storage(),
            WORK,
            STORAGE,
            false,
            false
        )
        .result,
        Err(Rejection::Frame(TransportError::Role))
    ));
    let mut bad = wire.canonical_bytes().to_vec();
    assert!(word(&bad, 20) > 0);
    let start = section(&bad, 0).start + 4;
    bad[start] ^= 1;
    assert!(matches!(
        attempt(&p, &bad, false),
        Err(Rejection::Inputs(InputsError::Graphs(
            CanonicalPolicy8GraphPoolErrorV1::Admission { .. }
        )))
    ));
}

#[test]
fn first_prefix_error_precedes_independently_bad_tail_rows() {
    let p = prepared(true, true);
    let original = p.inputs().continuation.occurrences;
    let mut operations = original.operations.to_vec();
    operations[0].output.operation = u32::MAX;
    let mut candidate = original;
    candidate.operations = &operations;
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (rows, _) = encode_rows(candidate, &mut budget).unwrap();
    let mut bad = literal(inputs(&p, &rows));
    let start = section(&bad, 2).start;
    bad[start] ^= 1;
    assert!(attempt(&p, &bad, false).is_ok());
    assert!(matches!(
        p5(attempt(&p, &bad, true).err().unwrap()),
        P5Error::ExecutionClaim
    ));
}
