use super::super::super::resource_tests::argument_correspondence_tests::native_helper_argument_owner;
use super::*;

#[test]
fn checked_physical_slots_cover_tuples_zst_and_both_rust_call_forms() {
    for shape in [None, Some(false), Some(true)] {
        let owner = native_helper_argument_owner(shape);
        let module = owner.executable.module();
        let inputs = if shape.is_none() {
            vec![constant(7), constant(11)]
        } else {
            vec![constant(7), constant(11), constant(17)]
        };
        let expected = inputs.last().unwrap().clone();
        let mut checked_calls = 0;
        for root in owner.semantic_ssa.source_semantic().roots() {
            let association = owner
                .correspondence
                .lowered_functions
                .iter()
                .find(|row| row.correspondence_owner == *root && row.semantic_function == *root)
                .unwrap();
            let entry = module
                .functions
                .iter()
                .find(|function| function.id == association.kernel_ir_function)
                .unwrap();
            for block in &entry.body.as_ref().unwrap().blocks {
                for (ordinal, operation) in block.operations.iter().enumerate() {
                    let OperationKind::Call { callee, arguments } = &operation.kind else {
                        continue;
                    };
                    checked_calls += 1;
                    let helper = module
                        .functions
                        .iter()
                        .find(|function| function.id == *callee)
                        .unwrap();
                    assert_eq!(helper.body.as_ref().unwrap().blocks.len(), 1);
                    assert_eq!(arguments.len(), inputs.len());
                    assert_eq!(helper.signature.parameters.len(), arguments.len());
                    if shape.is_some() {
                        assert_eq!(arguments[0], arguments[2]);
                        assert_ne!(
                            helper.body.as_ref().unwrap().parameters[0],
                            helper.body.as_ref().unwrap().parameters[2]
                        );
                    }
                    let location = FunctionOperationLocation::new(block.id, ordinal);
                    let probe = |meter: &mut TestMeter<'_, '_>, _: &Cell<bool>| {
                        with_native_helper_values(
                            &owner.semantic_ssa,
                            module,
                            &owner.correspondence,
                            *root,
                            entry,
                            meter,
                            |context, meter| {
                                let template =
                                    context.root_call(entry, location, operation, meter)?;
                                let (expression, bytes) = template.instantiate(&inputs, meter)?;
                                assert_eq!(expression, expected);
                                drop(expression);
                                meter.release(bytes)?;
                                Ok(())
                            },
                        )
                    };
                    let (result, floor, work, peak, _) = run(10_000_000, 32 * 1024 * 1024, probe);
                    result.unwrap();
                    assert_eq!(floor, 4096);
                    let exact = run(work, peak, probe);
                    exact.0.unwrap();
                    assert_eq!(exact.1, 4096);
                    for (work, storage) in [(work - 1, peak), (work, peak - 1)] {
                        let short = run(work, storage, probe);
                        assert!(short.0.is_err());
                        assert_eq!(short.1, 4096);
                    }
                }
            }
        }
        assert_eq!(checked_calls, 4);
    }
}

fn assert_bad_correspondence_is_not_a_resource_failure(
    owner: &ProductionPreRankedKirOwnerV1,
    changed: &SemanticKirCorrespondenceV1,
    mutation: usize,
) {
    let module = owner.executable.module();
    let root = owner.semantic_ssa.source_semantic().roots()[0];
    let association = owner
        .correspondence
        .lowered_functions
        .iter()
        .find(|row| row.correspondence_owner == root && row.semantic_function == root)
        .unwrap();
    let entry = module
        .functions
        .iter()
        .find(|function| function.id == association.kernel_ir_function)
        .unwrap();
    let (location, operation) = call(entry);
    let entered = Cell::new(false);
    let (result, floor, _, _, foreign) = run(10_000_000, 32 << 20, |meter, _| {
        with_native_helper_values(
            &owner.semantic_ssa,
            module,
            changed,
            root,
            entry,
            meter,
            |context, meter| {
                entered.set(true);
                let row = context
                    .calls
                    .iter()
                    .find(|row| std::ptr::eq(row.operation, operation))
                    .unwrap();
                assert!(!row.checked_arguments, "mutation {mutation} was accepted");
                assert!(!meter.exhausted());
                context
                    .root_call(entry, location, operation, meter)
                    .map(|_| ())
            },
        )
    });
    assert!(entered.get(), "mutation {mutation} bypassed checked query");
    assert_eq!(result, Err("native call callee, arity or result changed"));
    assert_eq!((floor, foreign), (4096, 31));
}

#[test]
fn checked_argument_consumer_refuses_corrupted_components_and_zero_rows() {
    for shape in [None, Some(false), Some(true)] {
        let owner = native_helper_argument_owner(shape);
        let root = owner.semantic_ssa.source_semantic().roots()[0];
        let helper = SemanticFunctionIdV1::from_index(0);
        let indices: Vec<_> = owner
            .correspondence
            .parameter_component_bindings
            .iter()
            .enumerate()
            .filter_map(|(index, row)| {
                (row.correspondence_owner == root && row.semantic_function == helper)
                    .then_some(index)
            })
            .collect();
        assert!(indices.len() >= 2);
        for mutation in 0..10 {
            let mut changed = owner.correspondence.clone();
            let [a, b] = [indices[0], indices[1]];
            let row = &mut changed.parameter_component_bindings[a];
            match mutation {
                0 => {
                    *row.projection.last_mut().unwrap() = SemanticKirParameterProjectionV1::Field(1)
                }
                1 => row.semantic_component_type = SemanticTypeIdV1::from_index(0),
                2 => {
                    let first = row.kernel_ir_value;
                    let second = changed.parameter_component_bindings[b].kernel_ir_value;
                    changed.parameter_component_bindings[a].kernel_ir_value = second;
                    changed.parameter_component_bindings[b].kernel_ir_value = first;
                }
                3 => {
                    changed.parameter_component_bindings[b].kernel_ir_value = row.kernel_ir_value;
                }
                4 => row.correspondence_owner = SemanticFunctionIdV1::from_index(99),
                5 => row.semantic_function = root,
                6 | 7 => {
                    let mut rows = changed.parameter_component_bindings.to_vec();
                    if mutation == 6 {
                        rows.remove(a);
                    } else {
                        rows.insert(a, rows[a].clone());
                    }
                    changed.parameter_component_bindings = rows.into_boxed_slice();
                }
                8 => row.projection[0] = SemanticKirParameterProjectionV1::ArrayIndex(0),
                9 => row.semantic_local = SemanticLocalIdV1::from_index(99),
                _ => unreachable!(),
            }
            assert_bad_correspondence_is_not_a_resource_failure(&owner, &changed, mutation);
        }
        if shape.is_none() {
            continue;
        }
        let ignored = owner
            .correspondence
            .ignored_parameter_bindings
            .iter()
            .position(|row| row.correspondence_owner == root && row.semantic_function == helper)
            .unwrap();
        for mutation in 10..14 {
            let mut changed = owner.correspondence.clone();
            match mutation {
                10 | 11 => {
                    let mut rows = changed.ignored_parameter_bindings.to_vec();
                    if mutation == 10 {
                        rows.remove(ignored);
                    } else {
                        rows.insert(ignored, rows[ignored]);
                    }
                    changed.ignored_parameter_bindings = rows.into_boxed_slice();
                }
                12 => {
                    changed.ignored_parameter_bindings[ignored].semantic_type =
                        SemanticTypeIdV1::from_index(1)
                }
                13 => {
                    changed.ignored_parameter_bindings[ignored].semantic_local =
                        changed.parameter_component_bindings[indices[0]].semantic_local
                }
                _ => unreachable!(),
            }
            assert_bad_correspondence_is_not_a_resource_failure(&owner, &changed, mutation);
        }
    }
}
