//! Additional ROOT-only tests over the actual seven-pass observer.
use super::*;
use fe2o3_kernel_analysis::check_canonical_kir_transition_receipt_v1;
use fe2o3_kernel_ir::InertCanonicalKirTransitionReceiptV1 as Receipt;

fn binary_fixture(op: BinaryOp) -> Module {
    let mut entry = returning(10, &[2, 3]);
    for result in [2, 3] {
        entry.operations.push(Operation::effect_free(
            value(result),
            OperationKind::Binary {
                op,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ));
    }
    function_module(
        vec![u32_type(); 2],
        vec![u32_type(); 2],
        vec![ValueId(0), ValueId(1)],
        vec![entry],
    )
}
fn wire_check(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    rows: Candidate<'_>,
    budget: &mut Budget<'_>,
) -> std::result::Result<(), CheckError> {
    let floor = budget.storage();
    let (encoded, es) =
        Receipt::from_candidate_with_budget(&input.identity(), &output.identity(), rows, budget)
            .unwrap();
    budget.reserve_storage(es.retained_storage()).unwrap();
    let (decoded, ds) = Receipt::decode_with_budget(encoded.canonical_bytes(), budget).unwrap();
    budget.reserve_storage(ds.retained_storage()).unwrap();
    let checked_floor = budget.storage();
    let result = {
        match check_canonical_kir_transition_receipt_v1(input, output, &decoded, budget) {
            Ok((checked, storage)) => {
                assert_eq!(budget.storage(), checked_floor);
                budget.reserve_storage(storage.retained_storage()).unwrap();
                assert!(std::ptr::eq(checked.input(), input));
                assert!(std::ptr::eq(checked.output(), output));
                assert!(std::ptr::eq(checked.rows().uses, decoded.candidate().uses));
                assert!(!checked.grants_authority());
                Ok(storage.retained_storage())
            }
            Err(error) => {
                assert_eq!(budget.storage(), checked_floor);
                Err(error)
            }
        }
    };
    let result = result.map(|retained| budget.release_storage(retained).unwrap());
    drop(decoded);
    budget.release_storage(ds.retained_storage()).unwrap();
    drop(encoded);
    budget.release_storage(es.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    result
}

#[test]
fn actual_capture_wire_roundtrip_preserves_repeated_edges_selected_merge_and_cse() {
    for (index, module) in [
        duplicate_edges(None, false),
        duplicate_edges(Some(true), false),
        binary_fixture(BinaryOp::BitXor),
    ]
    .into_iter()
    .enumerate()
    {
        with_transition(&module, |observed, input, output, budget| {
            let rows = observed.occurrences().candidate();
            match index {
                0 => assert_eq!(rows.edges.len(), 2),
                1 => assert!(rows.segments.len() > rows.blocks.len()),
                2 => {
                    assert_eq!(input.operations().len(), 2);
                    assert_eq!(output.operations().len(), 1);
                }
                _ => unreachable!(),
            }
            wire_check(input, output, rows, budget).unwrap();
        });
    }
}

#[test]
fn actual_capture_wire_keeps_duplicate_successor_occurrences_distinct() {
    with_transition(
        &duplicate_edges(None, false),
        |observed, input, output, budget| {
            let base = observed.occurrences().candidate();
            assert_eq!(base.edges.len(), 2);
            with_row_copy(base.edges, budget, |rows, budget| {
                let first = rows[0].input;
                rows[0].input = rows[1].input;
                rows[1].input = first;
                assert!(matches!(
                    wire_check(
                        input,
                        output,
                        Candidate {
                            edges: rows,
                            ..base
                        },
                        budget
                    ),
                    Err(CheckError::Rule(_))
                ));
            });
        },
    );
}

#[test]
fn actual_cse_capture_wire_rejects_a_fabricated_constant_origin() {
    with_transition(
        &binary_fixture(BinaryOp::BitXor),
        |observed, input, output, budget| {
            let base = observed.occurrences().candidate();
            assert_eq!(base.operations.len(), 1);
            with_row_copy(base.operations, budget, |rows, budget| {
                rows[0].origin = Origin::ConstantFrom(Definition::FunctionArgument {
                    function: FunctionCoordinate(0),
                    argument: 0,
                });
                assert!(matches!(
                    wire_check(
                        input,
                        output,
                        Candidate {
                            operations: rows,
                            ..base
                        },
                        budget
                    ),
                    Err(CheckError::Rule(_))
                ));
            });
        },
    );
}

#[test]
fn potentially_effectful_additions_remain_distinct_through_wire_replay() {
    with_transition(
        &binary_fixture(BinaryOp::Add),
        |observed, input, output, budget| {
            assert_eq!(input.operations().len(), 2);
            assert_eq!(output.operations().len(), 2);
            let rows = observed.occurrences().candidate();
            assert_eq!(rows.operations.len(), 2);
            assert_ne!(rows.operations[0].origin, rows.operations[1].origin);
            wire_check(input, output, rows, budget).unwrap();
        },
    );
}
