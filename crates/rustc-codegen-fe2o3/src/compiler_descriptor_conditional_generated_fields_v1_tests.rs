use super::*;
use crate::compiler_descriptor::ScalarTypeV1;
use fe2o3_artifacts::RustcAbiClassV1;
use fe2o3_kernel_ir::{
    AccessMode as KirAccess, AddressSpace, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    ScalarType, SliceType,
};
use fe2o3_mir_model::semantic_mir_v1::*;

mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/defined_helper_semantic_fixture_v1.rs"
    ));
}

// These tests exercise bounded projection subchecks. Rows are deliberately
// inert test data, not a forged Request, source relation, or compiler owner.
fn row(parameter: u32, source: u32, role: Role) -> ConditionalGeneratedArgumentV1 {
    ConditionalGeneratedArgumentV1 {
        projection: ConditionalArgumentBindingV1 {
            canonical_parameter: parameter,
            source_argument: source,
            adjusted_argument: source,
            semantic_local: source + 1,
            semantic_type: 8,
            generated_field: source as u16,
            role,
            source_type_identity: [3; 32],
            device_layout_identity: [4; 32],
        },
        canonical_value: ValueId(parameter + 10),
    }
}
fn read(parameter: u32, operation: u32) -> Occurrence {
    Occurrence {
        canonical: FunctionOperationLocation::new(BlockId(1), operation as usize),
        ranked: (2, operation + 100),
        parameter,
    }
}
fn ample<T>(run: impl FnOnce(&mut Budget<'_>) -> T) -> T {
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    run(&mut budget)
}

#[test]
fn canonical_table_sorted_but_read_multiplicity_and_order_preserved() {
    ample(|budget| {
        let mut scratch = Scratch::new();
        // The physical canonical slot, original source and table positions
        // deliberately differ. Source ordinals are not canonical ordinals.
        let output = row(8, 0, Role::Output);
        let input_a = row(2, 1, Role::Input);
        let input_b = row(5, 2, Role::Input);
        scratch.insert(output, budget).unwrap();
        for (argument, occurrence) in [
            (input_b, read(5, 3)),
            (input_a, read(2, 1)),
            (input_b, read(5, 9)),
        ] {
            scratch.insert(argument, budget).unwrap();
            scratch.push_read(occurrence, budget).unwrap();
        }
        assert_eq!(scratch.finish(8, 3, budget).unwrap(), 2);
        assert_eq!(scratch.arguments(), &[input_a, input_b, output]);
        assert_eq!(scratch.read_arguments(), &[1, 0, 1]);
        assert_eq!(scratch.arguments()[2].projection().generated_field, 0);
        assert_eq!(scratch.arguments()[2].allocation_origin(), 1);
        assert_eq!(scratch.arguments()[2].canonical_value(), ValueId(18));
    });
}

#[test]
fn output_only_is_complete_without_a_fabricated_read_row() {
    ample(|budget| {
        let mut scratch = Scratch::new();
        scratch.insert(row(4, 0, Role::Output), budget).unwrap();
        assert_eq!(scratch.finish(4, 0, budget).unwrap(), 0);
        assert!(scratch.read_arguments().is_empty());
    });
}

#[test]
fn every_conflicting_coordinate_of_a_repeated_parameter_is_rejected() {
    ample(|budget| {
        let original = row(2, 1, Role::Input);
        for case in 0..10 {
            let mut scratch = Scratch::new();
            scratch.insert(original, budget).unwrap();
            let mut changed = original;
            match case {
                0 => changed.projection.source_argument += 1,
                1 => changed.projection.adjusted_argument += 1,
                2 => changed.projection.semantic_local += 1,
                3 => changed.projection.semantic_type += 1,
                4 => changed.projection.generated_field += 1,
                5 => changed.projection.source_type_identity[0] ^= 1,
                6 => changed.projection.device_layout_identity[0] ^= 1,
                7 => changed.projection.role = Role::Output,
                8 => changed.canonical_value.0 += 1,
                9 => changed.projection.adjusted_argument = u32::MAX,
                _ => unreachable!(),
            }
            assert!(
                scratch.insert(changed, budget).is_err(),
                "coordinate {case}"
            );
            assert_eq!(scratch.arguments(), &[original]);
        }
    });
}

#[test]
fn duplicate_output_and_cross_parameter_field_substitutions_fail() {
    ample(|budget| {
        let output = row(4, 0, Role::Output);
        let mut scratch = Scratch::new();
        scratch.insert(output, budget).unwrap();
        assert!(scratch.insert(output, budget).is_err());
        for case in 0..5 {
            let mut candidate = row(7, 1, Role::Input);
            match case {
                0 => candidate.projection.generated_field = output.projection.generated_field,
                1 => candidate.projection.source_argument = output.projection.source_argument,
                2 => candidate.projection.semantic_local = output.projection.semantic_local,
                3 => candidate.canonical_value = output.canonical_value,
                4 => candidate.projection.adjusted_argument = output.projection.adjusted_argument,
                _ => unreachable!(),
            }
            assert!(
                scratch.insert(candidate, budget).is_err(),
                "substitution {case}"
            );
            assert_eq!(scratch.arguments(), &[output]);
        }
    });
}

#[test]
fn missing_extra_and_cross_wired_rosters_fail() {
    ample(|budget| {
        for case in 0..7 {
            let mut scratch = Scratch::new();
            if case != 0 {
                scratch.insert(row(4, 0, Role::Output), budget).unwrap();
            }
            if case != 1 {
                scratch.insert(row(7, 1, Role::Input), budget).unwrap();
            }
            if case == 2 {
                scratch.insert(row(8, 2, Role::Input), budget).unwrap();
            }
            if case == 3 {
                scratch.insert(row(9, 3, Role::Output), budget).unwrap();
            }
            scratch
                .push_read(read(if case == 4 { 4 } else { 7 }, 0), budget)
                .unwrap();
            assert!(
                scratch
                    .finish(
                        if case == 5 { 99 } else { 4 },
                        if case == 6 { 2 } else { 1 },
                        budget,
                    )
                    .is_err(),
                "roster {case}"
            );
        }
    });
}

#[test]
fn duplicate_locations_fail_without_deduplicating_legitimate_repeated_reads() {
    ample(|budget| {
        let mut scratch = Scratch::new();
        let first = read(1, 3);
        scratch.push_read(first, budget).unwrap();
        let mut same_canonical = read(1, 5);
        same_canonical.canonical = first.canonical;
        let mut same_ranked = read(1, 7);
        same_ranked.ranked = first.ranked;
        for changed in [first, same_canonical, same_ranked] {
            assert!(scratch.push_read(changed, budget).is_err());
        }
        scratch.push_read(read(1, 9), budget).unwrap();
    });
}

#[test]
fn independent_cardinality_checks_reject_missing_extra_and_oversized_inputs() {
    for (outputs, reads, rows) in [
        (0, 0, 0),
        (2, 0, 2),
        (1, 2, 2),
        (1, 2, 4),
        (
            1,
            MAX_CONDITIONAL_READS_V1 + 1,
            MAX_CONDITIONAL_READS_V1 + 2,
        ),
        (1, usize::MAX, usize::MAX),
    ] {
        assert!(check_cardinality(outputs, reads, rows).is_err());
    }
    assert!(check_cardinality(1, 0, 1).is_ok());
    assert!(check_cardinality(1, MAX_CONDITIONAL_READS_V1, MAX_CONDITIONAL_READS_V1 + 1).is_ok());
}

#[test]
fn argument_and_occurrence_caps_are_independent() {
    ample(|budget| {
        let mut scratch = Scratch::new();
        for i in 0..MAX_CONDITIONAL_ARGUMENTS_V1 as u32 {
            scratch
                .insert(
                    row(i, i, if i == 0 { Role::Output } else { Role::Input }),
                    budget,
                )
                .unwrap();
        }
        assert!(scratch.insert(row(64, 64, Role::Input), budget).is_err());
        // Repeated reads do not consume new unique-argument slots at the cap.
        scratch.insert(row(1, 1, Role::Input), budget).unwrap();
        let mut scratch = Scratch::new();
        scratch.insert(row(0, 0, Role::Output), budget).unwrap();
        scratch.insert(row(1, 1, Role::Input), budget).unwrap();
        for i in 0..MAX_CONDITIONAL_READS_V1 as u32 {
            scratch.push_read(read(1, i), budget).unwrap();
        }
        assert!(
            scratch
                .push_read(read(1, MAX_CONDITIONAL_READS_V1 as u32), budget)
                .is_err()
        );
        assert_eq!(
            scratch.finish(0, MAX_CONDITIONAL_READS_V1, budget).unwrap(),
            0
        );
        assert_eq!(scratch.read_arguments(), &[1; MAX_CONDITIONAL_READS_V1]);
    });
}

#[test]
fn work_denial_precedes_projection_mutation() {
    let mut scratch = Scratch::new();
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, 0);
    assert!(matches!(
        scratch.insert(row(0, 0, Role::Output), &mut budget),
        Err(Error::Resource(_))
    ));
    assert!(scratch.arguments().is_empty());
    assert!(matches!(
        scratch.push_read(read(1, 0), &mut budget),
        Err(Error::Resource(_))
    ));
    assert!(scratch.read_arguments().is_empty());
    assert!(work.failed_work().is_some());
}

fn field(role: Role) -> (TypedDescriptorArgumentV1, Type) {
    let input = role == Role::Input;
    (
        TypedDescriptorArgumentV1 {
            name: "field".into(),
            kind: if input {
                Kind::SharedSlice(ScalarTypeV1::F32)
            } else {
                Kind::DisjointSlice(ScalarTypeV1::F32)
            },
            access: if input {
                AccessMode::ReadOnly
            } else {
                AccessMode::ReadWrite
            },
            offset: 16,
            layout: None,
            source_size: 16,
            source_alignment: 8,
            rustc_abi_class: RustcAbiClassV1::ScalarPair,
            semantic_type_identity: SemanticTypeIdentityV1::from_sha256([8; 32]),
        },
        Type::Slice(SliceType::new(
            Type::Scalar(ScalarType::F32),
            AddressSpace::Global,
            if input {
                KirAccess::ReadOnly
            } else {
                KirAccess::ReadWrite
            },
        )),
    )
}

#[test]
fn all_input_and_output_field_kinds_and_identities_are_checked() {
    ample(|budget| {
        for role in [Role::Input, Role::Output] {
            let (original, physical) = field(role);
            check_field(
                &original,
                role,
                original.semantic_type_identity,
                &physical,
                budget,
            )
            .unwrap();
            for case in 0..7 {
                let mut argument = original.clone();
                let mut ty = physical.clone();
                match case {
                    0 => {
                        argument.semantic_type_identity =
                            SemanticTypeIdentityV1::from_sha256([9; 32])
                    }
                    1 => argument.access = AccessMode::ByValue,
                    2 => argument.kind = Kind::Scalar(ScalarTypeV1::F32),
                    3 => {
                        argument.kind = if role == Role::Input {
                            Kind::DisjointSlice(ScalarTypeV1::F32)
                        } else {
                            Kind::SharedSlice(ScalarTypeV1::F32)
                        }
                    }
                    4 => ty = Type::Scalar(ScalarType::F32),
                    5 => {
                        ty = Type::Slice(SliceType::new(
                            Type::Scalar(ScalarType::U32),
                            AddressSpace::Global,
                            if role == Role::Input {
                                KirAccess::ReadOnly
                            } else {
                                KirAccess::ReadWrite
                            },
                        ))
                    }
                    6 => {
                        ty = Type::Slice(SliceType::new(
                            Type::Scalar(ScalarType::F32),
                            AddressSpace::Global,
                            if role == Role::Input {
                                KirAccess::ReadWrite
                            } else {
                                KirAccess::ReadOnly
                            },
                        ))
                    }
                    _ => unreachable!(),
                }
                assert!(
                    check_field(
                        &argument,
                        role,
                        original.semantic_type_identity,
                        &ty,
                        budget
                    )
                    .is_err(),
                    "role {role:?}, field mutation {case}"
                );
            }
        }
    });
}

#[test]
fn nominal_identities_are_the_existing_producer_records_not_a_new_hash() {
    ample(|budget| {
        budget
            .reserve_storage(fe2o3_kernel_descriptor::DESCRIPTOR_QUERY_STORAGE_V3)
            .unwrap();
        for role in [Role::Input, Role::Output] {
            let (argument, _) = field(role);
            let (source, layout) = nominal_v3::records(argument.kind, budget).unwrap();
            let expected_source = match role {
                Role::Input => {
                    fe2o3_kernel_descriptor::SourceTypeDescriptorV3::SharedSlice(ScalarTypeV1::F32)
                }
                Role::Output => fe2o3_kernel_descriptor::SourceTypeDescriptorV3::DisjointSlice(
                    ScalarTypeV1::F32,
                ),
            };
            assert_eq!(source.descriptor(), expected_source);
            let expected =
                fe2o3_kernel_descriptor::SourceTypeRecordV3::new(expected_source, &mut |n| {
                    budget.charge_work(n)
                })
                .unwrap();
            assert_eq!(source.identity(), expected.identity());
            let expected_layout = match role {
                Role::Input => fe2o3_kernel_descriptor::DeviceLayoutDescriptorV1::shared_slice(
                    ScalarTypeV1::F32,
                ),
                Role::Output => fe2o3_kernel_descriptor::DeviceLayoutDescriptorV1::disjoint_slice(
                    ScalarTypeV1::F32,
                ),
            };
            let expected_layout =
                fe2o3_kernel_descriptor::device_layout_record_v3(expected_layout, &mut |n| {
                    budget.charge_work(n)
                })
                .unwrap();
            assert_eq!(layout.identity(), expected_layout.identity());
        }
    });
}

#[test]
fn root_selection_is_exact_owner_export_not_name_parsing_or_function_ordinal() {
    let original = fixture::source(vec![fixture::subtract()]);
    let second = fixture::function(
        90,
        true,
        2,
        vec![
            fixture::block(
                91,
                vec![],
                fixture::call(1, vec![fixture::copy(1), fixture::copy(2)], 3, 1),
            ),
            fixture::block(92, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"second_source".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([61; 32]),
        original.functions()[0]
            .kernel_entry()
            .unwrap()
            .source_contract(),
    ));
    let mut functions = original.functions().to_vec();
    functions.push(second);
    let source = InertSemanticMirRequestV1::new(
        original.target(),
        original.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(2),
        ],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let id = source.roots()[1];
    assert_eq!(id.index(), 2);
    let name = std::str::from_utf8(
        source.functions()[id.index() as usize]
            .kernel_entry()
            .unwrap()
            .export_symbol()
            .as_bytes(),
    )
    .unwrap();
    ample(|budget| {
        let (root, body) = select_source(&source, name, budget).unwrap();
        assert_eq!(root, id);
        assert_eq!(
            body,
            source.select_kernel_body_for_root_v1(id).unwrap().body()
        );
        for substituted in ["", "kernel_0", "helper_value_source_suffix"] {
            assert!(select_source(&source, substituted, budget).is_err());
        }
    });
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, 0);
    assert!(matches!(
        select_source(&source, name, &mut budget),
        Err(Error::Resource(_))
    ));
}

#[test]
fn generated_field_lookup_refuses_hidden_expanded_and_out_of_wire_range_slots() {
    assert_eq!(checked_field_index(1, 1, 3).unwrap(), 1);
    assert_eq!(checked_field_index(63, 63, 64).unwrap(), 63);
    for (source, adjusted, count) in [
        (1, 2, 3),
        (2, 1, 3),
        (0, 0, 0),
        (3, 3, 3),
        (64, 64, 65),
        (u32::MAX, u32::MAX, usize::MAX),
    ] {
        assert!(checked_field_index(source, adjusted, count).is_err());
    }
}

#[test]
fn replayed_semantic_root_is_not_a_generated_field_or_root_roster_position() {
    let selected = SemanticFunctionIdV1::from_index(7);
    require_replayed_root(selected, 7).unwrap();
    for different in [0, 1, 6, 8, u32::MAX] {
        assert!(require_replayed_root(selected, different).is_err());
    }
}

#[test]
fn exact_projection_scratch_is_admitted_before_allocating_or_calling_out() {
    let scratch = scratch_bytes().unwrap();
    assert!(scratch >= size_of::<Scratch>());
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 19 + scratch - 1);
    budget.reserve_storage(19).unwrap();
    let result: Result<(), Error> = budget.with_prepaid_scope(19, 8, 8, scratch, |_| {
        panic!("callback before bounded scratch admission")
    });
    assert!(matches!(result, Err(Error::Resource(_))));
    assert_eq!(budget.storage(), 19);
    assert_eq!(budget.peak_storage(), 19);
}

#[test]
fn common_scope_restores_scratch_on_success_error_and_unwind() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for case in 0..3 {
        let scratch = scratch_bytes().unwrap();
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 19 + scratch);
        budget.reserve_storage(19).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            budget.with_prepaid_scope::<_, Error>(19, 8, 8, scratch, |budget| {
                assert_eq!(budget.storage(), 19 + scratch);
                let rows = Scratch::new();
                assert!(rows.arguments().is_empty());
                match case {
                    0 => Ok(()),
                    1 => Err(Error::Mismatch("consumer refused")),
                    _ => panic!("consumer panic"),
                }
            })
        }));
        assert_eq!(result.is_err(), case == 2);
        assert_eq!(budget.storage(), 19);
        assert_eq!(budget.peak_storage(), 19 + scratch);
        assert_eq!(work.work(), 8);
    }
}

#[test]
fn common_scope_never_refunds_projection_scratch_on_a_substituted_account() {
    let scratch = scratch_bytes().unwrap();
    let mut work = Work::new(100);
    let mut foreign_work = Work::new(100);
    let mut budget = Budget::new(&mut work, 19 + scratch);
    let mut foreign = Budget::new(&mut foreign_work, 7);
    budget.reserve_storage(19).unwrap();
    foreign.reserve_storage(7).unwrap();
    let result: Result<(), Error> = budget.with_prepaid_scope(19, 8, 8, scratch, |budget| {
        std::mem::swap(budget, &mut foreign);
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), 7);
    assert_eq!(foreign.storage(), 19 + scratch);
}
