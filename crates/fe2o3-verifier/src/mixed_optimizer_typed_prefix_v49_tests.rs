use super::super::prefix_v49::PrefixSegmentsV49;
use super::*;
use fe2o3_kernel_ir::FormalIndexWidth;

#[test]
fn typed_prefix_prepaid_dominance_matches_scoped_queries_including_disconnected_blocks() {
    use super::super::prefix_v49::models;
    let mut module = fixture();
    for id in 5..10 {
        module.functions[0]
            .body
            .as_mut()
            .unwrap()
            .blocks
            .push(block(
                id,
                vec![],
                Terminator::Return {
                    values: vec![ValueId(0)],
                },
            ));
    }
    with_chain_module(&module, |prefix, _, _, floor| {
        run(floor, LIMIT, LIMIT, |out| {
            let inventory = prefix.input();
            let before = out.budget.storage();
            models::with_dominance(inventory, 0, out, |prepared, out| {
                let text = std::mem::take(&mut out.text);
                let failure = out.failure.take();
                let (text, failure) = fe2o3_kernel_ir::with_canonical_kir_control_flow_v18(
                    inventory.owner(),
                    inventory.functions()[0].coordinate,
                    Default::default(),
                    out.budget,
                    |flow, budget| {
                        let mut writer = Writer {
                            text,
                            budget,
                            failure,
                        };
                        for (block, row) in inventory.blocks().iter().enumerate() {
                            for gap in row.operations.start..=row.operations.end {
                                for definition in inventory.definitions() {
                                    assert_eq!(
                                        prepared.available(
                                            definition.coordinate,
                                            block,
                                            gap,
                                            &mut writer
                                        )?,
                                        models::available(
                                            inventory,
                                            flow,
                                            definition.coordinate,
                                            block,
                                            gap,
                                            &mut writer
                                        )?
                                    );
                                }
                            }
                        }
                        Ok::<_, Error>((writer.text, writer.failure))
                    },
                )?;
                out.text = text;
                out.failure = failure;
                Ok(())
            })?;
            assert_eq!(out.budget.storage(), before);
            Ok(())
        })
        .0
        .unwrap();
    });
}

#[test]
fn typed_prefix_prepaid_dominance_rejects_foreign_and_underpaid_queries_before_work() {
    use super::super::prefix_v49::models;
    with_chain(|prefix, _, _, floor| {
        for foreign in [false, true] {
            run(floor, LIMIT, LIMIT, |out| {
                let inventory = prefix.input();
                let mut paid = 0;
                let result = models::with_dominance(inventory, 0, out, |prepared, out| {
                    paid = out.budget.storage();
                    let definition = inventory.definitions()[0].coordinate;
                    if foreign {
                        let mut work = Work::new(LIMIT);
                        let mut budget = Budget::new(&mut work, LIMIT);
                        budget.reserve_storage(out.budget.storage())?;
                        let mut foreign = Writer::new(&mut budget)?;
                        assert!(matches!(
                            prepared.available(definition, 0, 0, &mut foreign),
                            Err(Error::Resource(Resource::Accounting))
                        ));
                        assert_eq!(foreign.budget.work(), 0);
                    } else {
                        out.budget.release_storage(1)?;
                        let before = out.budget.work();
                        assert!(matches!(
                            prepared.available(definition, 0, 0, out),
                            Err(Error::Resource(Resource::Accounting))
                        ));
                        assert_eq!(out.budget.work(), before);
                        out.budget.reserve_storage(1)?;
                    }
                    let before = out.budget.work();
                    assert!(matches!(
                        prepared.available(definition, 0, 0, out),
                        Err(Error::Resource(Resource::Accounting))
                    ));
                    assert_eq!(out.budget.work(), before);
                    Ok(())
                });
                assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                assert_eq!(out.budget.storage(), paid);
                Ok(())
            })
            .0
            .unwrap();
        }
    });
}

#[test]
fn typed_prefix_prepaid_dominance_does_not_refund_rejected_cfg_callback_storage() {
    use super::super::prefix_v49::models;
    with_chain(|prefix, _, _, floor| {
        run(floor, LIMIT, LIMIT, |out| {
            let inventory = prefix.input();
            let before = out.budget.storage();
            let result = models::with_dominance(inventory, 0, out, |_, out| {
                fe2o3_kernel_ir::with_canonical_kir_control_flow_v18(
                    inventory.owner(),
                    inventory.functions()[0].coordinate,
                    Default::default(),
                    out.budget,
                    |_, budget| {
                        budget.reserve_storage(7)?;
                        Ok::<_, Error>(())
                    },
                )
            });
            assert!(matches!(
                result,
                Err(Error::Flow(
                    fe2o3_kernel_ir::CanonicalKirControlFlowScopeErrorV1::Resource(
                        Resource::Accounting
                    )
                ))
            ));
            assert_eq!(out.budget.storage(), before + 7);
            out.budget.release_storage(7)?;
            Ok(())
        })
        .0
        .unwrap();
    });
}

#[test]
fn typed_prefix_checked_segments_partition_merged_operations_and_keep_empty_cuts() {
    with_cfg_chain_module(&fixture(), |prefix, licm, output, floor| {
        let origins = Origins {
            inventory: prefix.input(),
            duplicate: false,
        };
        run(floor, LIMIT, LIMIT, |out| {
            let bridge = AllocationBridgeV48::derive(prefix, licm, output, &origins, out)?;
            let plan = PrefixSegmentsV49::derive(&bridge, out)?;
            let mut merged = false;
            let mut empty = false;
            for row in prefix.rows().blocks {
                let block = block_index(prefix.output(), row.output)?;
                let start = row.segments.start as usize;
                let end = start + row.segments.len as usize;
                merged |= row.segments.len > 1;
                let mut next_operation = prefix.output().blocks()[block].operations.start;
                for position in start..end {
                    let original =
                        block_index(prefix.input(), prefix.rows().segments[position].input)?;
                    let segment = plan.segment(original, out)?.unwrap();
                    assert_eq!(segment.output_block, block);
                    assert_eq!(segment.start, next_operation);
                    assert!(segment.start <= segment.end);
                    empty |= segment.start == segment.end;
                    next_operation = segment.end;
                    let next = if position + 1 < end {
                        Some(block_index(
                            prefix.input(),
                            prefix.rows().segments[position + 1].input,
                        )?)
                    } else {
                        None
                    };
                    assert_eq!(segment.next_original, next);
                    for operation in segment.start..segment.end {
                        if let Origin::Retained(site) = prefix.rows().operations[operation].origin {
                            assert_eq!(site.block, prefix.input().blocks()[original].coordinate);
                        }
                    }
                }
                assert_eq!(
                    next_operation,
                    prefix.output().blocks()[block].operations.end
                );
            }
            assert!(
                merged,
                "genuine fixed-policy fixture must merge a source chain"
            );
            assert!(
                empty,
                "removed source constant must leave an empty original cut"
            );
            assert!(matches!(
                plan.segment(prefix.input().blocks().len(), out),
                Err(Error::Statement(_))
            ));
            plan.segment(0, out)?;
            Ok(())
        })
        .0
        .unwrap();
    });
}

#[test]
fn typed_prefix_segment_custody_precedes_queries_and_sticks_after_restore() {
    with_chain(|prefix, licm, output, floor| {
        let origins = Origins {
            inventory: prefix.input(),
            duplicate: false,
        };
        for foreign in [false, true] {
            run(floor, LIMIT, LIMIT, |out| {
                let bridge = AllocationBridgeV48::derive(prefix, licm, output, &origins, out)?;
                let bridge_floor = out.budget.storage();
                let plan = PrefixSegmentsV49::derive(&bridge, out)?;
                let plan_floor = out.budget.storage();
                assert!(plan_floor > bridge_floor);
                if foreign {
                    let mut work = Work::new(LIMIT);
                    let mut budget = Budget::new(&mut work, LIMIT);
                    budget.reserve_storage(plan_floor)?;
                    let mut other = Writer::new(&mut budget)?;
                    let before = (other.budget.work(), other.budget.storage());
                    assert!(matches!(
                        plan.segment(0, &mut other),
                        Err(Error::Resource(Resource::Accounting))
                    ));
                    assert_eq!((other.budget.work(), other.budget.storage()), before);
                } else {
                    out.budget.release_storage(1)?;
                    let before = out.budget.work();
                    assert!(matches!(
                        plan.segment(0, out),
                        Err(Error::Resource(Resource::Accounting))
                    ));
                    assert_eq!(out.budget.work(), before);
                    out.budget.reserve_storage(1)?;
                }
                let before = (out.budget.work(), out.budget.storage());
                assert!(matches!(
                    plan.segment(0, out),
                    Err(Error::Resource(Resource::Accounting))
                ));
                assert_eq!((out.budget.work(), out.budget.storage()), before);
                Ok(())
            })
            .0
            .unwrap();
        }
    });
}

#[test]
fn typed_prefix_removed_original_block_has_no_fabricated_cursor_or_runtime_state() {
    let mut module = fixture();
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .push(block(
            5,
            vec![Instruction::effect_free(
                ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(17)),
            )],
            Terminator::Return {
                values: vec![ValueId(20)],
            },
        ));
    with_cfg_chain_module(&module, |prefix, licm, output, floor| {
        let origins = Origins {
            inventory: prefix.input(),
            duplicate: false,
        };
        run(floor, LIMIT, LIMIT, |out| {
            let bridge = AllocationBridgeV48::derive(prefix, licm, output, &origins, out)?;
            let plan = PrefixSegmentsV49::derive(&bridge, out)?;
            let removed = prefix.input().functions()[0].blocks.end - 1;
            assert!(
                !prefix
                    .rows()
                    .segments
                    .iter()
                    .any(|row| row.input == prefix.input().blocks()[removed].coordinate)
            );
            assert_eq!(plan.segment(removed, out)?, None);
            Ok(())
        })
        .0
        .unwrap();
    });
}

#[test]
fn typed_prefix_licm_composition_uses_one_middle_cursor_and_both_checked_results() {
    let mut module = fixture();
    let loop_body = &mut module.functions[0].body.as_mut().unwrap().blocks[3];
    let repeated = [
        loop_body.operations.remove(1),
        loop_body.operations.remove(1),
    ];
    assert_eq!(repeated[0].results[0].id, ValueId(11));
    assert_eq!(repeated[1].results[0].id, ValueId(12));
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .extend(repeated);
    for argument in 0..24 {
        let scalar = match argument % 4 {
            0 => ScalarType::Index,
            1 => ScalarType::U64,
            2 => ScalarType::Bool,
            _ => ScalarType::U32,
        };
        module.functions[0]
            .signature
            .parameters
            .push(Type::Scalar(scalar));
        module.functions[0]
            .body
            .as_mut()
            .unwrap()
            .parameters
            .push(ValueId(100 + argument));
    }
    let body = &mut module.functions[0].body.as_mut().unwrap().blocks[3];
    body.operations.insert(
        0,
        Instruction::new(
            vec![
                ValueDef::new(ValueId(14), Type::Scalar(ScalarType::U32)),
                ValueDef::new(ValueId(15), Type::BOOL),
            ],
            OperationKind::Binary {
                op: BinaryOp::Checked(fe2o3_kernel_ir::CheckedBinaryOperator::Add),
                lhs: ValueId(0),
                rhs: ValueId(0),
            },
        ),
    );
    body.operations.push(Instruction::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(2),
            value: ValueId(14),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        },
    ));
    body.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(15),
        then_target: BlockId(4),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
        with_chain_module(&module, |prefix, licm, output, floor| {
            let origins = Origins {
                inventory: prefix.input(),
                duplicate: false,
            };
            let generate = |out: &mut Writer<'_, '_>| -> Result<()> {
                let limits = fe2o3_kernel_analysis::CanonicalKirPrivateByteLimitsV38 {
                    max_boundaries: 4096,
                };
                let (original_physical, storage) =
                    fe2o3_kernel_analysis::analyze_canonical_kir_private_bytes_v38(
                        prefix.input(),
                        limits,
                        out.budget,
                    )?;
                out.budget.reserve_storage(storage.retained_storage())?;
                let (prefix_physical, storage) =
                    fe2o3_kernel_analysis::analyze_canonical_kir_private_bytes_v38(
                        prefix.output(),
                        limits,
                        out.budget,
                    )?;
                out.budget.reserve_storage(storage.retained_storage())?;
                let (output_physical, storage) =
                    fe2o3_kernel_analysis::analyze_canonical_kir_private_bytes_v38(
                        output, limits, out.budget,
                    )?;
                out.budget.reserve_storage(storage.retained_storage())?;
                let original_contracts =
                    target_view_contracts_v38::TargetByteViewContractsV38::derive(
                        prefix.input(),
                        width,
                        out,
                    )?;
                let prefix_contracts =
                    target_view_contracts_v38::TargetByteViewContractsV38::derive(
                        prefix.output(),
                        width,
                        out,
                    )?;
                let output_contracts =
                    target_view_contracts_v38::TargetByteViewContractsV38::derive(
                        output, width, out,
                    )?;
                let bridge = AllocationBridgeV48::derive(prefix, licm, output, &origins, out)?;
                bridge.emit_typed_prefix_licm_v49(
                    &original_physical,
                    &prefix_physical,
                    &output_physical,
                    &original_contracts,
                    &prefix_contracts,
                    &output_contracts,
                    width,
                    7,
                    out,
                )?;
                let mut checked = 0;
                for (operation, row) in prefix.output().operations().iter().enumerate() {
                    if matches!(
                        row.operation.kind,
                        OperationKind::Binary {
                            op: BinaryOp::Checked(_),
                            ..
                        }
                    ) && licm.origins()[operation].hoist.is_some()
                    {
                        checked += 1;
                        assert_eq!(row.results.len(), 2);
                        for result in row.results.clone() {
                            assert!(out.text.contains(&format!(
                                "typed_relocation_result_0_v48({result}, before, little_endian)"
                            )));
                        }
                    }
                }
                assert_eq!(checked, 1);
                Ok(())
            };
            let measured = run(floor, LIMIT, LIMIT, generate);
            let expected = measured.0.unwrap();
            let prefix_relation = expected
                .split_once("spec fn typed_prefix_related_0_v49(")
                .unwrap()
                .1
                .split_once("\n}\n")
                .unwrap()
                .0;
            let originals: Vec<_> = [ValueId(11), ValueId(12)]
                .iter()
                .map(|id| {
                    prefix
                        .input()
                        .operations()
                        .iter()
                        .find(|row| row.operation.results.iter().any(|result| result.id == *id))
                        .unwrap()
                        .results
                        .start
                })
                .collect();
            let mut retained_distinct_sources = false;
            for segment in prefix_relation.split("\n if before.pc == ").skip(1) {
                let mut equalities = std::collections::BTreeSet::new();
                for line in segment.lines() {
                    let Some(equality) = line.strip_prefix(" && before.values[") else {
                        continue;
                    };
                    let (original, target) = equality.split_once("] == after.values[").unwrap();
                    let target = target.split_once(']').unwrap().0;
                    let pair = (
                        original.parse::<usize>().unwrap(),
                        target.parse::<usize>().unwrap(),
                    );
                    assert!(
                        equalities.insert(pair),
                        "duplicate boundary equality {pair:?}"
                    );
                }
                retained_distinct_sources |= equalities.iter().any(|&(original, target)| {
                    original == originals[0] && equalities.contains(&(originals[1], target))
                });
            }
            assert!(
                retained_distinct_sources,
                "distinct CSE source values must both constrain the target"
            );
            for (helper, relation, inventory) in [
                (
                    "typed_prefix_arguments_0_v102",
                    "typed_prefix_related_0_v49",
                    prefix.output(),
                ),
                (
                    "typed_licm_arguments_0_v102",
                    "typed_licm_cursor_related_0_v49",
                    output,
                ),
            ] {
                assert_eq!(expected.matches(&format!("spec fn {helper}(")).count(), 1);
                let body = expected
                    .split_once(&format!("spec fn {helper}("))
                    .unwrap()
                    .1
                    .split_once("\n}\n")
                    .unwrap()
                    .0;
                let relation_body = expected
                    .split_once(&format!("spec fn {relation}("))
                    .unwrap()
                    .1
                    .split_once("\n}\n")
                    .unwrap()
                    .0;
                assert_eq!(
                    relation_body
                        .matches(&format!("{helper}(before, after)"))
                        .count(),
                    1
                );
                assert!(relation_body.contains("before.pc == -1 || before.pc == -2"));
                assert!(
                    relation_body.find("else {").unwrap() < relation_body.find(helper).unwrap()
                );
                let mut arguments = 0;
                for (index, definition) in inventory.definitions().iter().enumerate() {
                    if !matches!(definition.coordinate, Definition::FunctionArgument { .. }) {
                        assert!(!body.contains(&format!("after.values[{index}]")));
                        continue;
                    }
                    arguments += 1;
                    let Type::Scalar(scalar) = definition.ty else {
                        panic!("fixture scalar argument");
                    };
                    let modulus = match scalar {
                        ScalarType::Bool => "2",
                        ScalarType::U32 => "memory_value_modulus_v30(4)",
                        ScalarType::U64 => "memory_value_modulus_v30(8)",
                        ScalarType::Index if width == FormalIndexWidth::Bits32 => {
                            "memory_value_modulus_v30(4)"
                        }
                        ScalarType::Index => "memory_value_modulus_v30(8)",
                        _ => panic!("fixture scalar argument"),
                    };
                    let predicate = format!(
                        "before.values[{index}] == after.values[{index}] && byte_scalar_type_v57(after.values[{index}], {modulus})"
                    );
                    assert_eq!(body.matches(&predicate).count(), 1);
                    assert!(
                        !relation_body
                            .contains(&format!("byte_scalar_type_v57(after.values[{index}],"))
                    );
                }
                assert_eq!(arguments, 26);
                assert_eq!(body.matches("byte_scalar_type_v57(").count(), arguments);
            }
            for text in [
                "typed_composed_entry_relation_0_v49",
                "typed_composed_step_0_v49",
                "typed_composed_trace_0_v49",
                "typed_licm_cursor_related_0_v49(middle, actual, little_endian)",
                "before_cursor.segment == after_cursor.segment",
                "typed_relocated_event_v49(right).observations.is_some()",
            ] {
                assert!(expected.contains(text), "missing {text}");
            }
            assert_eq!(
                expected.matches("struct TypedMemoryObservationV48").count(),
                1
            );
            assert_eq!(
                expected
                    .matches("spec fn typed_relocated_event_v49(")
                    .count(),
                1
            );
            assert!(!expected.contains("assume("));
            assert!(!expected.contains("external_body"));
            let exact = run(floor, measured.1, measured.2, generate);
            assert_eq!(exact.0.unwrap(), expected);
            assert_eq!((exact.1, exact.2), (measured.1, measured.2));
            for work in [true, false] {
                let work_limit = measured.1 - usize::from(work);
                let storage_limit = measured.2 - usize::from(!work);
                match run(floor, work_limit, storage_limit, generate)
                    .0
                    .err()
                    .unwrap()
                {
                    Error::Resource(Resource::Work(error)) if work => {
                        assert_eq!(error.limit(), work_limit);
                        assert_eq!(error.actual(), measured.1);
                    }
                    Error::Resource(Resource::Storage(error)) if !work => {
                        assert_eq!(error.limit(), storage_limit);
                        assert_eq!(error.actual(), measured.2);
                    }
                    other => panic!("wrong exact composed resource boundary: {other:?}"),
                }
            }
        });
    }
}

#[test]
fn typed_prefix_segment_interpreter_preserves_event_suffixes_and_exact_entry_resources() {
    with_chain(|prefix, licm, output, floor| {
        let origins = Origins {
            inventory: prefix.input(),
            duplicate: false,
        };
        let generate = |out: &mut Writer<'_, '_>| -> Result<()> {
            let limits = fe2o3_kernel_analysis::CanonicalKirPrivateByteLimitsV38 {
                max_boundaries: 4096,
            };
            let (before_physical, storage) =
                fe2o3_kernel_analysis::analyze_canonical_kir_private_bytes_v38(
                    prefix.input(),
                    limits,
                    out.budget,
                )?;
            out.budget.reserve_storage(storage.retained_storage())?;
            let (after_physical, storage) =
                fe2o3_kernel_analysis::analyze_canonical_kir_private_bytes_v38(
                    prefix.output(),
                    limits,
                    out.budget,
                )?;
            out.budget.reserve_storage(storage.retained_storage())?;
            let before_contracts = target_view_contracts_v38::TargetByteViewContractsV38::derive(
                prefix.input(),
                FormalIndexWidth::Bits64,
                out,
            )?;
            let after_contracts = target_view_contracts_v38::TargetByteViewContractsV38::derive(
                prefix.output(),
                FormalIndexWidth::Bits64,
                out,
            )?;
            let bridge = AllocationBridgeV48::derive(prefix, licm, output, &origins, out)?;
            bridge.emit_typed_prefix_v49(
                &before_physical,
                &after_physical,
                &before_contracts,
                &after_contracts,
                FormalIndexWidth::Bits64,
                7,
                out,
            )
        };
        let measured = run(floor, LIMIT, LIMIT, generate);
        let expected = measured.0.unwrap();
        for function in prefix.input().functions() {
            if function.function.body.is_none() {
                continue;
            }
            let namespace = function.coordinate.0;
            assert!(expected.contains(&format!("spec fn byte_inputs_{namespace}_v55(")));
            assert!(!expected.contains(&format!(" as byte_inputs_{namespace}_v55,")));
            for operation in function.operations.clone() {
                assert!(expected.contains(&format!(
                    "spec fn byte_operation_{namespace}_{operation}_v30("
                )));
            }
        }
        for text in [
            "TypedPrefixCursorV49",
            "byte_micro_step_",
            "byte_micro_finish_",
            "subrange(initial_count as int",
            "typed_prefix_cursor_valid_0_v49",
            "m.observations[i - 1].after == m.observations[i].before",
            "typed_prefix_entry_relation_0_v49",
            "typed_prefix_input_defined_0_v49",
            "typed_allocation_environment_0_v48(before) == typed_allocation_environment_1_v48(after)",
            "typed_prefix_actual_event_v49(actual).observations.is_some()",
            "returned: result.returned",
            "MemoryOperationEffectV30::Trap",
            "MemoryOperationEffectV30::Refused",
        ] {
            assert!(expected.contains(text), "missing {text}");
        }
        assert!(expected.contains(
            "typed_prefix_related_0_v49(before: MemoryStateV30, cursor: TypedPrefixCursorV49,"
        ));
        assert_closed_float_execution_context_v53(&expected);
        assert!(!expected.contains("let compared = after.values["));
        for forbidden in [
            "base: Seq<int>, initial: int, op:",
            "assume(",
            "external_body",
        ] {
            assert!(!expected.contains(forbidden), "unexpected {forbidden}");
        }
        let exact = run(floor, measured.1, measured.2, generate);
        assert_eq!(exact.0.unwrap(), expected);
        assert_eq!((exact.1, exact.2), (measured.1, measured.2));
        for work in [true, false] {
            let work_limit = measured.1 - usize::from(work);
            let storage_limit = measured.2 - usize::from(!work);
            match run(floor, work_limit, storage_limit, generate)
                .0
                .err()
                .unwrap()
            {
                Error::Resource(Resource::Work(error)) if work => {
                    assert_eq!(error.limit(), work_limit);
                    assert_eq!(error.actual(), measured.1);
                }
                Error::Resource(Resource::Storage(error)) if !work => {
                    assert_eq!(error.limit(), storage_limit);
                    assert_eq!(error.actual(), measured.2);
                }
                other => panic!("wrong exact prefix resource boundary: {other:?}"),
            }
        }
    });
}
