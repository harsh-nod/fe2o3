use super::*;
use crate::canonical_kir_inventory_v1::v18_tests::storage_discriminant_module;

#[test]
fn actual_discriminant_reads_are_ordered_and_keep_exact_access_payloads() {
    for space in [AddressSpace::Private, AddressSpace::Workgroup] {
        let original = storage_discriminant_module(space);
        for changed in [false, true] {
            let mut output = original.clone();
            if changed {
                let operation =
                    &mut output.functions[0].body.as_mut().unwrap().blocks[0].operations[18];
                let OperationKind::Storage(Storage::ReadDiscriminant { access, .. }) =
                    &mut operation.kind
                else {
                    panic!("the existing inventory fixture must contain an actual tag read");
                };
                // Both alignment promises are structurally valid; the relation
                // must not identify distinct ordered access payloads.
                access.alignment = 4;
            }
            inspect(
                original.clone(),
                output,
                |input, _| Rows::identity(input),
                |input, output, rows, floor| {
                    for ordinal in [18, 20] {
                        let operation = &input.operations()[ordinal].operation.kind;
                        assert!(matches!(
                            operation,
                            OperationKind::Storage(Storage::ReadDiscriminant { .. })
                        ));
                        assert!(!payload::pure(operation));
                    }
                    if changed {
                        assert_eq!(
                            rejected(input, output, rows, floor),
                            Error::Rule("retained operation payload")
                        );
                    } else {
                        accepted(input, output, rows, floor);
                    }
                },
            );
        }
    }
}

#[test]
fn nonempty_execution_discard_roster_requires_each_exact_acquisition() {
    let mut original = execution_module(1);
    let operations = &mut original.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.truncate(3);
    let mut second = operations[2].clone();
    second.results[0].id = ValueId(14);
    operations.push(second);
    operations.push(execution(
        None,
        Execution::ScopeEnd {
            workgroup: ValueId(11),
            discarded: vec![ValueId(12), ValueId(14)],
        },
    ));
    inspect(
        original.clone(),
        original,
        |input, _| Rows::identity(input),
        |input, output, rows, floor| {
            accepted(input, output, rows, floor);
            let end = op(4);
            let first = rows
                .uses
                .iter()
                .position(|row| {
                    row.input
                        == Use::OperationOperand {
                            operation: end,
                            operand: 1,
                        }
                })
                .unwrap();
            let second = rows
                .uses
                .iter()
                .position(|row| {
                    row.input
                        == Use::OperationOperand {
                            operation: end,
                            operand: 2,
                        }
                })
                .unwrap();
            // Keep the complete output-coordinate roster. Only the claimed
            // original acquisitions are exchanged, despite identical role types.
            let first_input = rows.uses[first].input;
            let second_input = rows.uses[second].input;
            rows.uses[first].input = second_input;
            rows.uses[second].input = first_input;
            assert_eq!(
                rejected(input, output, rows, floor),
                Error::Rule("final operand has no exact descendant")
            );
            rows.uses[first].input = first_input;
            rows.uses[second].input = second_input;
            accepted(input, output, rows, floor);
        },
    );
}
