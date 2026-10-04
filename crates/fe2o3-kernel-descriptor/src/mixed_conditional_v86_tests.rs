use super::*;
use crate::mixed_conditional_v26::{self as legacy, MixedScalarV26};

fn pay(_: usize) -> Result<(), ()> {
    Ok(())
}

fn subjects() -> MixedContractSubjectsV26 {
    MixedContractSubjectsV26 {
        kernel_id: [1; 32],
        source_semantic_identity: [2; 32],
        original_graph_identity: [3; 32],
        output_graph_identity: [4; 32],
        descriptor_identity: [5; 32],
        original_root: 7,
        output_function: 2,
        source_rank: 3,
        index_width: 32,
        exact_grid: [64, 1, 1],
        source_argument_count: 5,
        generated_field_count: 5,
        explicit_argument_bytes: 80,
        kernarg_alignment: 8,
    }
}
fn argument(source: u32, reads: u32, writes: u32) -> MixedArgumentV26 {
    MixedArgumentV26 {
        source_argument: source,
        generated_field: source as u16,
        physical_parameter: source + 9,
        semantic_type: source + 20,
        semantic_type_identity: [6; 32],
        descriptor_type_identity: [7; 32],
        device_layout_identity: [8; 32],
        scalar: MixedScalarV26::U32,
        pointer_offset: source * 16,
        length_offset: source * 16 + 8,
        reads,
        writes,
        source_exclusive: writes != 0,
    }
}
fn operation(function: u32, block: u32, operation: u32) -> MixedOperationV26 {
    MixedOperationV26 {
        function,
        block,
        operation,
    }
}
fn occurrence(argument: u16, ordinal: u32, writing: bool) -> MixedOccurrenceV86 {
    MixedOccurrenceV86 {
        argument,
        original_instance: 3,
        original_operation: operation(6, 4, ordinal),
        output_operation: operation(2, 5, ordinal),
        original_formation: operation(6, 1, ordinal),
        output_formation: operation(2, 2, ordinal),
        output_address_index: MixedDefinitionV26::FunctionArgument {
            function: 2,
            argument: 0,
        },
        output_guard: MixedAccessGuardV86::CfgEdge {
            edge: MixedEdgeV26 {
                function: 2,
                block: 4,
                successor: 0,
            },
            condition: MixedDefinitionV26::Result {
                operation: operation(2, 4, 8),
                result: 1,
            },
        },
        slice_value: 19,
        pointer_value: 29,
        index_value: 39,
        guard_index_value: 49,
        length_value: 59,
        predicate_value: 69,
        path: MixedGuardPathV26::TrueEdge {
            source: 87,
            successor: 0,
            target: 42,
        },
        element_bytes: 4,
        alignment: 4,
        address_space: MixedMemorySpaceV26::Global,
        writing,
        volatile: false,
        invocation_axis: 0,
        invocation_value: 39,
        access_envelope: MixedIndexEnvelopeV26::LogicalExtent { argument },
        formation_envelope: MixedIndexEnvelopeV26::InvocationAxis { axis: 0 },
    }
}
fn encoded(
    subjects: MixedContractSubjectsV26,
    arguments: &[MixedArgumentV26],
    occurrences: &[MixedOccurrenceV86],
) -> Vec<u8> {
    let input = MixedContractInputV86 {
        subjects,
        arguments,
        occurrences,
    };
    let len = encoded_mixed_contract_v86_len(&input, &mut pay).unwrap();
    let mut bytes = vec![0; len];
    encode_mixed_contract_v86(&input, &mut bytes, &mut pay).unwrap();
    bytes
}

fn explicit(ordinal: u32) -> MixedOccurrenceV86 {
    let mut row = occurrence(0, ordinal, true);
    row.path = MixedGuardPathV26::ExplicitPredicate;
    row.output_guard = MixedAccessGuardV86::ExplicitPredicate {
        condition: MixedDefinitionV26::Result {
            operation: operation(2, 4, 9),
            result: 0,
        },
        bound_comparison: MixedDefinitionV26::Result {
            operation: operation(2, 4, 8),
            result: 0,
        },
    };
    row
}

#[test]
fn mixed_v86_preserves_both_guard_kinds_without_fabricating_an_edge() {
    let args = [argument(0, 1, 2)];
    let mut same_comparison = explicit(3);
    same_comparison.output_guard = MixedAccessGuardV86::ExplicitPredicate {
        condition: explicit(2).output_guard.condition(),
        bound_comparison: explicit(2).output_guard.condition(),
    };
    let rows = [occurrence(0, 1, false), explicit(2), same_comparison];
    let bytes = encoded(subjects(), &args, &rows);
    let view = decode_mixed_contract_v86(&bytes, &mut pay).unwrap();
    for (i, row) in rows.iter().enumerate() {
        let actual = view.occurrence(i, &mut pay).unwrap();
        assert_eq!(&actual, row);
        assert_eq!(actual.output_guard.edge().is_some(), i == 0);
        assert_ne!(actual.access_envelope, actual.formation_envelope);
    }
    assert_eq!((view.argument_count(), view.occurrence_count()), (1, 3));
    assert!(!view.grants_artifact_or_launch_authority());
    assert!(legacy::decode_mixed_contract_v26(&bytes, &mut pay).is_err());
    let mut old_version = bytes.clone();
    old_version[..8].copy_from_slice(&legacy::MIXED_CONTRACT_MAGIC_V26);
    old_version[8..10].copy_from_slice(&26u16.to_le_bytes());
    assert!(decode_mixed_contract_v86(&old_version, &mut pay).is_err());
    assert!(legacy::decode_mixed_contract_v26(&old_version, &mut pay).is_err());
}

#[test]
fn mixed_v86_rejects_guard_tag_padding_path_and_foreign_coordinate_mutants() {
    let args = [argument(0, 0, 1)];
    for row in [explicit(2), occurrence(0, 2, true)] {
        let bytes = encoded(subjects(), &args, &[row]);
        let guard_at = PREFIX
            + MixedContractSubjectsV26::BYTES
            + MixedArgumentV26::BYTES
            + 2
            + 4
            + 4 * MixedOperationV26::BYTES
            + MixedDefinitionV26::BYTES;
        for tag in [2, 3, 255] {
            let mut changed = bytes.clone();
            changed[guard_at] = tag;
            assert!(decode_mixed_contract_v86(&changed, &mut pay).is_err());
        }
        let padding = match row.output_guard {
            MixedAccessGuardV86::ExplicitPredicate { .. } => guard_at + 1..guard_at + 13,
            MixedAccessGuardV86::CfgEdge { .. } => guard_at + 30..guard_at + 47,
        };
        for at in padding {
            let mut changed = bytes.clone();
            changed[at] = 1;
            assert!(decode_mixed_contract_v86(&changed, &mut pay).is_err());
        }
        let mut wrong_path = row;
        wrong_path.path = match row.path {
            MixedGuardPathV26::ExplicitPredicate => MixedGuardPathV26::TrueEdge {
                source: 0,
                successor: 0,
                target: 1,
            },
            MixedGuardPathV26::TrueEdge { .. } => MixedGuardPathV26::ExplicitPredicate,
        };
        for changed in [
            wrong_path,
            MixedOccurrenceV86 {
                output_guard: MixedAccessGuardV86::ExplicitPredicate {
                    condition: row.output_guard.condition(),
                    bound_comparison: MixedDefinitionV26::FunctionArgument {
                        function: 99,
                        argument: 0,
                    },
                },
                path: MixedGuardPathV26::ExplicitPredicate,
                ..row
            },
        ] {
            assert!(
                encoded_mixed_contract_v86_len(
                    &MixedContractInputV86 {
                        subjects: subjects(),
                        arguments: &args,
                        occurrences: &[changed],
                    },
                    &mut pay
                )
                .is_err()
            );
        }
    }
}

#[test]
fn mixed_v86_commits_full_predicate_and_independent_formation_and_census() {
    let args = [argument(0, 0, 1)];
    let row = explicit(2);
    let bytes = encoded(subjects(), &args, &[row]);
    let view = decode_mixed_contract_v86(&bytes, &mut pay).unwrap();
    let identity = view.occurrence_identity(0, &mut pay).unwrap();
    let mutations: [fn(&mut MixedOccurrenceV86); 5] = [
        |row| row.formation_envelope = MixedIndexEnvelopeV26::UnsignedWidth { bits: 32 },
        |row| row.access_envelope = MixedIndexEnvelopeV26::InvocationAxis { axis: 0 },
        |row| {
            row.output_address_index = MixedDefinitionV26::FunctionArgument {
                function: 2,
                argument: 3,
            }
        },
        |row| {
            row.output_guard = MixedAccessGuardV86::ExplicitPredicate {
                condition: row.output_guard.condition(),
                bound_comparison: MixedDefinitionV26::FunctionArgument {
                    function: 2,
                    argument: 4,
                },
            }
        },
        |row| {
            row.output_guard = MixedAccessGuardV86::ExplicitPredicate {
                condition: MixedDefinitionV26::FunctionArgument {
                    function: 2,
                    argument: 4,
                },
                bound_comparison: row.output_guard.condition(),
            }
        },
    ];
    for mutate in mutations {
        let mut changed = row;
        mutate(&mut changed);
        let bytes = encoded(subjects(), &args, &[changed]);
        let changed = decode_mixed_contract_v86(&bytes, &mut pay).unwrap();
        assert_ne!(changed.identity(), view.identity());
        assert_ne!(changed.occurrence_identity(0, &mut pay).unwrap(), identity);
        assert!(!changed.grants_artifact_or_launch_authority());
    }
    for rows in [Vec::new(), vec![row, row]] {
        assert!(
            encoded_mixed_contract_v86_len(
                &MixedContractInputV86 {
                    subjects: subjects(),
                    arguments: &args,
                    occurrences: &rows,
                },
                &mut pay
            )
            .is_err()
        );
    }
    for n in 0..bytes.len() {
        assert!(decode_mixed_contract_v86(&bytes[..n], &mut pay).is_err());
    }
}

#[test]
fn mixed_v86_resource_denial_precedes_output_and_query_results() {
    let args = [argument(0, 0, 1)];
    let rows = [explicit(2)];
    let input = MixedContractInputV86 {
        subjects: subjects(),
        arguments: &args,
        occurrences: &rows,
    };
    let len = encoded_mixed_contract_v86_len(&input, &mut pay).unwrap();
    let bound = 4096 + 32 * len;
    for limit in [bound, bound - 1] {
        let mut accepted = 0;
        let mut pay = |n| {
            assert_eq!(n, bound);
            if n > limit {
                Err(())
            } else {
                accepted += n;
                Ok(())
            }
        };
        let mut output = vec![0xa5; len];
        let result = encode_mixed_contract_v86(&input, &mut output, &mut pay);
        if limit == bound {
            result.unwrap();
            assert_eq!(accepted, bound);
        } else {
            assert!(matches!(result, Err(MixedContractErrorV86::Resource(()))));
            assert_eq!(accepted, 0);
            assert_eq!(output, vec![0xa5; len]);
        }
    }
    let bytes = encoded(subjects(), &args, &rows);
    assert!(matches!(
        decode_mixed_contract_v86(&bytes, &mut |_| Err(())),
        Err(MixedContractErrorV86::Resource(()))
    ));
    let view = decode_mixed_contract_v86(&bytes, &mut pay).unwrap();
    assert!(matches!(
        view.occurrence(0, &mut |_| Err(())),
        Err(MixedContractErrorV86::Resource(()))
    ));
    assert!(matches!(
        view.argument(0, &mut |_| Err(())),
        Err(MixedContractErrorV86::Resource(()))
    ));
    assert!(matches!(
        view.occurrence_identity(0, &mut |_| Err(())),
        Err(MixedContractErrorV86::Resource(()))
    ));
    assert!(matches!(
        view.argument_identity(0, &mut |_| Err(())),
        Err(MixedContractErrorV86::Resource(()))
    ));
}
