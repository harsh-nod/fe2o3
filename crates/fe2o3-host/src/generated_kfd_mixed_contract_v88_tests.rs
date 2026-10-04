use super::*;
use fe2o3_kernel_descriptor::mixed_conditional_v86::*;

fn predicated_contract(
    table: &DeviceDescriptorTableV3<'_>,
    unused_input: bool,
    mutate: impl FnOnce(
        &mut MixedContractSubjectsV26,
        &mut Vec<MixedArgumentV26>,
        &mut Vec<MixedOccurrenceV86>,
    ),
) -> Vec<u8> {
    let original = contract(table, unused_input, |_, _, _| {});
    let original = decode_mixed_contract_v26(&original, &mut free).unwrap();
    let mut subjects = *original.subjects();
    let mut arguments: Vec<_> = (0..original.argument_count())
        .map(|i| original.argument(i, &mut free).unwrap())
        .collect();
    let mut occurrences: Vec<_> = (0..original.occurrence_count())
        .map(|i| {
            let row = original.occurrence(i, &mut free).unwrap();
            MixedOccurrenceV86 {
                argument: row.argument,
                original_instance: row.original_instance,
                original_operation: row.original_operation,
                output_operation: row.output_operation,
                original_formation: row.original_formation,
                output_formation: row.output_formation,
                output_address_index: row.output_address_index,
                output_guard: MixedAccessGuardV86::ExplicitPredicate {
                    condition: row.output_guard_condition,
                    bound_comparison: row.output_guard_condition,
                },
                slice_value: row.slice_value,
                pointer_value: row.pointer_value,
                index_value: row.index_value,
                guard_index_value: row.guard_index_value,
                length_value: row.length_value,
                predicate_value: row.predicate_value,
                path: row.path,
                element_bytes: row.element_bytes,
                alignment: row.alignment,
                address_space: row.address_space,
                writing: row.writing,
                volatile: row.volatile,
                invocation_axis: row.invocation_axis,
                invocation_value: row.invocation_value,
                access_envelope: row.access_envelope,
                formation_envelope: row.formation_envelope,
            }
        })
        .collect();
    mutate(&mut subjects, &mut arguments, &mut occurrences);
    let input = MixedContractInputV86 {
        subjects,
        arguments: &arguments,
        occurrences: &occurrences,
    };
    let mut bytes = vec![0; encoded_mixed_contract_v86_len(&input, &mut free).unwrap()];
    encode_mixed_contract_v86(&input, &mut bytes, &mut free).unwrap();
    bytes
}

#[test]
fn predicated_mixed_pack_retains_exact_v86_rows_and_runtime_identity() {
    let descriptor = descriptor();
    let table = decode_device_descriptor_table_v3(&descriptor, &mut free).unwrap();
    let bytes = predicated_contract(&table, false, |_, _, _| {});
    let contract = decode_mixed_contract_v86(&bytes, &mut free).unwrap();
    let old_bytes = super::contract(&table, false, |_, _, _| {});
    let old = decode_mixed_contract_v26(&old_bytes, &mut free).unwrap();
    assert!(decode_mixed_contract_v26(&bytes, &mut free).is_err());
    assert!(decode_mixed_contract_v86(&old_bytes, &mut free).is_err());
    assert_ne!(contract.identity(), old.identity());
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let input = [1, 2, 3];
    let mut output = [0; 3];
    let value = packed(&input, &mut output, &mut budget);
    let floor = budget.storage();
    let bound = value
        .bind_predicated_mixed_conditional_premises_v88(&table, &contract, geometry(), &mut budget)
        .unwrap();
    let retained = bound.retained_storage();
    budget.reserve_storage(retained).unwrap();
    let (packed, premises) = bound.into_parts();
    assert_eq!(premises.accesses().len(), 3);
    assert!(!premises.accesses()[1].writing);
    assert!(premises.accesses()[2].writing);
    assert_eq!(premises.accesses()[1].slice, premises.accesses()[2].slice);
    for (index, row) in premises.accesses().iter().enumerate() {
        assert_eq!(
            row.occurrence_identity,
            contract.occurrence_identity(index, &mut free).unwrap()
        );
        assert_ne!(
            row.occurrence_identity,
            old.occurrence_identity(index, &mut free).unwrap()
        );
        assert_eq!(
            contract
                .occurrence(index, &mut free)
                .unwrap()
                .output_guard
                .edge(),
            None
        );
    }
    let bound = GeneratedMixedKfdArgumentsV26 {
        packed,
        premises,
        retained_storage: retained,
    };
    let (runtime, completion) = bound.into_runtime_inputs(geometry(), 0, 1000).unwrap();
    assert!(matches!(runtime.invocation_binding(),
        fe2o3_runtime::Gfx942RuntimeInvocationBindingV1::ConditionalMixedV26 { contract_identity, .. }
        if contract_identity == *contract.identity()));
    assert_eq!(completion.buffers.len(), 2);
    drop((runtime, completion));
    budget.release_storage(floor + retained).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn predicated_mixed_pack_keeps_unused_and_empty_slice_obligations() {
    let descriptor = descriptor();
    let table = decode_device_descriptor_table_v3(&descriptor, &mut free).unwrap();
    for empty in [false, true] {
        let bytes = predicated_contract(&table, true, |_, _, _| {});
        let contract = decode_mixed_contract_v86(&bytes, &mut free).unwrap();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let input = [1, 2, 3];
        let mut output = [0; 3];
        let length = if empty { 0 } else { 3 };
        let value = packed(&input[..length], &mut output[..length], &mut budget);
        let floor = budget.storage();
        let bound = value
            .bind_predicated_mixed_conditional_premises_v88(
                &table,
                &contract,
                geometry(),
                &mut budget,
            )
            .unwrap();
        let (packed, premises) = bound.into_parts();
        assert_eq!(premises.unused_slices().len(), 1);
        assert_eq!(
            premises.unused_slices()[0].source_argument_identity,
            contract.argument_identity(0, &mut free).unwrap()
        );
        for row in premises.accesses() {
            assert_eq!(row.address_domain, Domain::InvocationAxis { axis: 0 });
            assert_eq!(row.access_domain, Domain::LogicalExtent { slice: 1 });
        }
        drop((packed, premises));
        assert_eq!(budget.storage(), floor);
        budget.release_storage(floor).unwrap();
    }
}

#[test]
fn predicated_mixed_pack_refuses_logical_only_address_formation_even_when_empty() {
    let descriptor = descriptor();
    let table = decode_device_descriptor_table_v3(&descriptor, &mut free).unwrap();
    for empty in [false, true] {
        let bytes = predicated_contract(&table, false, |_, _, rows| {
            rows[2].formation_envelope = MixedIndexEnvelopeV26::LogicalExtent { argument: 1 };
        });
        let contract = decode_mixed_contract_v86(&bytes, &mut free).unwrap();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let input = [1, 2, 3];
        let mut output = [0; 3];
        let length = if empty { 0 } else { 3 };
        let value = packed(&input[..length], &mut output[..length], &mut budget);
        let floor = budget.storage();
        assert!(matches!(
            value.bind_predicated_mixed_conditional_premises_v88(
                &table,
                &contract,
                geometry(),
                &mut budget,
            ),
            Err(Error::Binding(
                "predicated mixed formation needs its invocation envelope"
            ))
        ));
        assert_eq!(budget.storage(), floor);
        budget.release_storage(floor).unwrap();
    }
}

#[test]
fn predicated_mixed_pack_preserves_full_guard_identity() {
    let descriptor = descriptor();
    let table = decode_device_descriptor_table_v3(&descriptor, &mut free).unwrap();
    let original = predicated_contract(&table, false, |_, _, _| {});
    let original = decode_mixed_contract_v86(&original, &mut free).unwrap();
    for mutate_bound in [false, true] {
        let bytes = predicated_contract(&table, false, |_, _, rows| {
            let MixedAccessGuardV86::ExplicitPredicate {
                condition,
                bound_comparison,
            } = &mut rows[2].output_guard
            else {
                panic!()
            };
            let definition = if mutate_bound {
                bound_comparison
            } else {
                condition
            };
            let MixedDefinitionV26::Result { operation, .. } = definition else {
                panic!()
            };
            operation.operation += 1;
        });
        let contract = decode_mixed_contract_v86(&bytes, &mut free).unwrap();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let input = [1, 2, 3];
        let mut output = [0; 3];
        let value = packed(&input, &mut output, &mut budget);
        let floor = budget.storage();
        let bound = value
            .bind_predicated_mixed_conditional_premises_v88(
                &table,
                &contract,
                geometry(),
                &mut budget,
            )
            .unwrap();
        let (packed, premises) = bound.into_parts();
        assert_ne!(contract.identity(), original.identity());
        assert_eq!(
            premises.accesses()[2].occurrence_identity,
            contract.occurrence_identity(2, &mut free).unwrap()
        );
        assert_ne!(
            premises.accesses()[2].occurrence_identity,
            original.occurrence_identity(2, &mut free).unwrap()
        );
        drop((packed, premises));
        assert_eq!(budget.storage(), floor);
        budget.release_storage(floor).unwrap();
    }
}

#[test]
fn predicated_mixed_pack_rejects_wrong_subject_layout_content_and_launch() {
    let descriptor = descriptor();
    let table = decode_device_descriptor_table_v3(&descriptor, &mut free).unwrap();
    for mutation in 0..6 {
        let bytes = predicated_contract(&table, false, |subjects, args, _| match mutation {
            0 => subjects.kernel_id[0] ^= 1,
            1 => subjects.descriptor_identity[0] ^= 1,
            2 => args[1].device_layout_identity[0] ^= 1,
            3 => subjects.exact_grid[0] -= 1,
            _ => (),
        });
        let contract = decode_mixed_contract_v86(&bytes, &mut free).unwrap();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let input = [1, 2, 3];
        let mut output = [0; 3];
        let mut value = packed(&input, &mut output, &mut budget);
        if mutation == 4 {
            value.explicit_kernarg[8] ^= 1;
        }
        if mutation == 5 {
            value.packing_observation.buffers[0].initial_sha256[0] ^= 1;
        }
        let floor = budget.storage();
        assert!(
            value
                .bind_predicated_mixed_conditional_premises_v88(
                    &table,
                    &contract,
                    geometry(),
                    &mut budget,
                )
                .is_err(),
            "mutation={mutation}"
        );
        assert_eq!(budget.storage(), floor);
        budget.release_storage(floor).unwrap();
    }
}

#[test]
fn predicated_mixed_pack_exact_and_one_short_resources_preserve_custody() {
    let descriptor = descriptor();
    let table = decode_device_descriptor_table_v3(&descriptor, &mut free).unwrap();
    let bytes = predicated_contract(&table, false, |_, _, _| {});
    let contract = decode_mixed_contract_v86(&bytes, &mut free).unwrap();
    let run = |work_limit, storage_limit| {
        let mut packing_work = Work::new(usize::MAX);
        let mut packing_budget = Budget::new(&mut packing_work, usize::MAX);
        let input = [1, 2, 3];
        let mut output = [0; 3];
        let value = packed(&input, &mut output, &mut packing_budget);
        let floor = packing_budget.storage();
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = value.bind_predicated_mixed_conditional_premises_v88(
            &table,
            &contract,
            geometry(),
            &mut budget,
        );
        let ok = result.is_ok();
        drop(result);
        assert_eq!(budget.storage(), floor);
        let counters = (ok, budget.work(), budget.storage(), budget.peak_storage());
        budget.release_storage(floor).unwrap();
        packing_budget.release_storage(floor).unwrap();
        counters
    };
    let (ok, work, retained, peak) = run(usize::MAX, usize::MAX);
    assert!(ok);
    assert_eq!(run(work, peak), (true, work, retained, peak));
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
}
