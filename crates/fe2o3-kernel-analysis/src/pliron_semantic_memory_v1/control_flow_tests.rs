use super::*;
use pliron::builtin::op_interfaces::OneRegionInterface;

fn block(context: &mut Context, function: &FuncOp, name: &str) -> Ptr<BasicBlock> {
    let block = BasicBlock::new(context, Some(name.try_into().unwrap()), vec![]);
    block.insert_at_back(function.get_region(context), context);
    block
}

fn move_to(context: &Context, op: &impl Op, block: Ptr<BasicBlock>) {
    op.get_operation().unlink(context);
    op.get_operation().insert_at_back(block, context);
}

fn split(context: &mut Context) -> (Fixture, Ptr<BasicBlock>, Ptr<BasicBlock>) {
    let f = fixture(context);
    // Deliberately opposite to execution order: entry, consumer, producer.
    let consumer = block(context, &f.function, "consumer");
    let producer = block(context, &f.function, "producer");
    move_to(context, &f.access, producer);
    move_to(context, &f.read, producer);
    move_to(context, &f.sum, consumer);
    move_to(context, &f.write, consumer);
    move_to(context, &f.ret, consumer);
    let first = BranchOp::new(context, producer);
    append(context, &f.function, &first);
    BranchOp::new(context, consumer)
        .get_operation()
        .insert_at_back(producer, context);
    (f, producer, consumer)
}

fn guard(
    context: &mut Context,
    f: &Fixture,
    threshold: u64,
    yes: Ptr<BasicBlock>,
    no: Ptr<BasicBlock>,
) {
    let entry = f.function.get_entry_block(context);
    let previous = entry.deref(context).get_terminator(context).unwrap();
    Operation::erase(previous, context);
    let limit = IndexConstantOp::new(context, threshold);
    append(context, &f.function, &limit);
    let lhs = f.index.result(context);
    let rhs = limit.result(context);
    let branch = IndexLessThanBranchOp::new(context, lhs, rhs, yes, no);
    append(context, &f.function, &branch);
}

fn traces(
    context: &Context,
    function: &FuncOp,
) -> (
    BoundedPlironFunctionInventoryV1,
    Vec<crate::pliron_invocation_trace::PlironInvocationTraceV1>,
) {
    let inventory = BoundedPlironFunctionInventoryV1::collect(context, function).unwrap();
    let sparse = analyze_pliron_sparse_indices_v1(context, function).unwrap();
    let traces =
        trace_pliron_invocations_with_inputs_v1(context, &inventory, &sparse, None).unwrap();
    (inventory, traces)
}

#[test]
fn physically_later_dominating_producer_keeps_its_exact_snapshot() {
    let mut context = setup();
    let (f, _, _) = split(&mut context);
    let proof = prove_live_pliron_semantic_memory_v1(&context, &f.function).unwrap();
    proof
        .with_live_reads(&context, &f.function, |reads| {
            assert_eq!(reads[0].site, PlironSemanticMemorySiteV1::new(2, 1));
            assert_eq!(reads[0].producer(), f.read.get_operation());
            assert_eq!(reads[0].instances().len(), 64);
            assert!(reads[0].reads_initial_memory());
        })
        .unwrap();
    let inputs = LivePlironInitialReadInputsV1::prove(&context, &f.function).unwrap();
    assert_eq!(
        inputs.commitment_leaf(f.read.get_operation()).unwrap(),
        Some((SEMANTIC_TYPED_READ_SYMBOL_BASE_V1, scalar()))
    );
    f.sum.get_operation().unlink(&context);
    assert_eq!(
        inputs.commitment_leaf(f.read.get_operation()),
        Err(E::MutationEpochChanged)
    );
}

#[test]
fn inactive_lanes_keep_original_invocation_ids_and_eventless_paths() {
    for (threshold, suffix, start) in [(16, false, 0), (48, true, 48)] {
        let mut context = setup();
        let (f, producer, _) = split(&mut context);
        let exit = block(&mut context, &f.function, "inactive");
        ReturnOp::new(&mut context)
            .get_operation()
            .insert_at_back(exit, &context);
        let (yes, no) = if suffix {
            (exit, producer)
        } else {
            (producer, exit)
        };
        guard(&mut context, &f, threshold, yes, no);
        let (_, traces) = traces(&context, &f.function);
        assert_eq!(traces.len(), 64);
        assert_eq!(traces.iter().filter(|t| t.events.is_empty()).count(), 48);
        let proof = prove_live_pliron_semantic_memory_v1(&context, &f.function).unwrap();
        proof
            .with_live_reads(&context, &f.function, |reads| {
                assert_eq!(reads[0].instances.len(), 16);
                for (i, instance) in reads[0].instances.iter().enumerate() {
                    assert_eq!(instance.invocation, start + i);
                    assert_eq!(instance.coordinates, [(start + i) as u64]);
                    assert_eq!(instance.indices, [(start + i) as u64]);
                }
            })
            .unwrap();
    }
}

#[test]
fn consumer_coverage_can_be_a_strict_subset_of_producer_coverage() {
    let mut context = setup();
    let (f, producer, consumer) = split(&mut context);
    // Read all lanes in entry; only sixteen execute the consumer.
    let entry = f.function.get_entry_block(&context);
    let terminator = entry.deref(&context).get_terminator(&context).unwrap();
    for op in [f.access.get_operation(), f.read.get_operation()] {
        op.unlink(&context);
        op.insert_before(&context, terminator);
    }
    let exit = block(&mut context, &f.function, "inactive");
    ReturnOp::new(&mut context)
        .get_operation()
        .insert_at_back(exit, &context);
    guard(&mut context, &f, 16, producer, exit);
    assert_eq!(
        consumer,
        f.sum
            .get_operation()
            .deref(&context)
            .get_parent_block()
            .unwrap()
    );
    let proof = prove_live_pliron_semantic_memory_v1(&context, &f.function).unwrap();
    proof
        .with_live_reads(&context, &f.function, |reads| {
            assert_eq!(reads[0].instances.len(), 64);
        })
        .unwrap();
}

#[test]
fn same_operation_ordinals_in_different_blocks_do_not_alias_reads() {
    let mut context = setup();
    let (f, _, _) = split(&mut context);
    let view = f.view.result(&context);
    let index = f.index.result(&context);
    let access =
        RankedAccessOp::new(&mut context, AccessKindAttr::Read, view, vec![index]).unwrap();
    access
        .get_operation()
        .insert_before(&context, f.sum.get_operation());
    let read = SemanticTypedReadOp::new(
        &mut context,
        SEMANTIC_TYPED_READ_SYMBOL_BASE_V1 + 1,
        scalar(),
        MemorySpaceAttr::Global,
        SemanticReadVolatilityAttr::NonVolatile,
        SemanticReadOrderingAttr::Unordered,
        view,
        vec![index],
        None,
    )
    .unwrap();
    read.get_operation()
        .insert_before(&context, f.sum.get_operation());
    Operation::replace_operand(f.sum.get_operation(), &context, 1, read.result(&context));
    let proof = prove_live_pliron_semantic_memory_v1(&context, &f.function).unwrap();
    proof
        .with_live_reads(&context, &f.function, |reads| {
            assert_eq!(reads.len(), 2);
            assert_eq!(reads[0].site.operation, reads[1].site.operation);
            assert_ne!(reads[0].site.block, reads[1].site.block);
            assert_ne!(reads[0].producer, reads[1].producer);
            assert!(
                reads
                    .iter()
                    .all(|r| r.instances.len() == 64 && r.reads_initial_memory())
            );
        })
        .unwrap();
}

#[test]
fn cross_block_prior_write_keeps_original_event_identity() {
    let mut context = setup();
    let (f, _, _) = split(&mut context);
    f.write.get_operation().unlink(&context);
    f.write
        .get_operation()
        .insert_before(&context, f.access.get_operation());
    let proof = prove_live_pliron_semantic_memory_v1(&context, &f.function).unwrap();
    proof
        .with_live_reads(&context, &f.function, |reads| {
            for instance in &reads[0].instances {
                assert_eq!(
                    instance.version,
                    PlironSemanticMemoryVersionV1::AfterWrite {
                        invocation: instance.invocation,
                        event: 0,
                        block: 2,
                        operation: 0,
                    }
                );
            }
        })
        .unwrap();
    assert!(matches!(
        LivePlironInitialReadInputsV1::prove(&context, &f.function),
        Err(E::NonInitialRead { .. })
    ));
}

#[test]
fn nondominating_diamond_and_never_executed_read_are_rejected() {
    let mut context = setup();
    let (f, producer, consumer) = split(&mut context);
    guard(&mut context, &f, 16, producer, consumer);
    assert!(matches!(
        prove_live_pliron_semantic_memory_v1(&context, &f.function),
        Err(E::NonDominatingOperand { .. })
    ));

    let mut context = setup();
    let (f, producer, _) = split(&mut context);
    let exit = block(&mut context, &f.function, "inactive");
    ReturnOp::new(&mut context)
        .get_operation()
        .insert_at_back(exit, &context);
    guard(&mut context, &f, 0, producer, exit);
    assert!(matches!(
        prove_live_pliron_semantic_memory_v1(&context, &f.function),
        Err(E::IncompleteTrace)
    ));
}

#[test]
fn completed_trace_coverage_rejects_omission_duplication_and_summary() {
    let mut context = setup();
    let (f, _, _) = split(&mut context);
    let (inventory, original) = traces(&context, &f.function);
    let reads = collect::reads(&context, &inventory).unwrap();
    for mutation in 0..5 {
        let mut changed = original.clone();
        match mutation {
            0 => {
                changed[0].blocks.remove(1);
            }
            1 => {
                let visit = changed[0].blocks[1].clone();
                changed[0].blocks.insert(2, visit);
            }
            2 => {
                changed[0].blocks[1].summarized = true;
            }
            3 => {
                changed[0].blocks.pop();
            }
            4 => {
                changed[0].events.remove(0);
            }
            _ => unreachable!(),
        }
        let mut work = control_flow::Work::new(MAX_PLIRON_TRACE_TOTAL_STEPS_V1);
        assert!(
            matches!(
                coverage::check(&context, &inventory, &reads, &changed, &mut work),
                Err(E::IncompleteTrace)
            ),
            "mutation {mutation}"
        );
    }
}

#[test]
fn graph_and_all_invocation_coverage_share_one_exact_work_limit() {
    let mut context = setup();
    let (f, _, _) = split(&mut context);
    let (inventory, traces) = traces(&context, &f.function);
    let reads = collect::reads(&context, &inventory).unwrap();
    let epoch_before = epoch(&context).unwrap();
    let run = |limit| {
        let mut work = control_flow::Work::new(limit);
        control_flow::validate(&context, &f.function, &inventory, &mut work)?;
        coverage::check(&context, &inventory, &reads, &traces, &mut work)?;
        Ok::<_, E>(work.used())
    };
    let exact = run(MAX_PLIRON_TRACE_TOTAL_STEPS_V1).unwrap();
    assert_eq!(run(exact), Ok(exact));
    assert_eq!(run(exact - 1), Err(E::ResourceLimit));
    assert_eq!(epoch(&context).unwrap(), epoch_before);
}

#[test]
fn cycles_foreign_successors_and_foreign_users_fail_before_dominance() {
    for case in 0..4 {
        let mut context = setup();
        let (f, producer, _) = split(&mut context);
        let term = producer.deref(&context).get_terminator(&context).unwrap();
        match case {
            0 => Operation::replace_successor(term, &context, 0, producer),
            1 => {
                let foreign = BasicBlock::new(&mut context, None, vec![]);
                Operation::replace_successor(term, &context, 0, foreign);
            }
            2 => {
                // Unlinked consumers still live in the reverse-use lists.
                BranchOp::new(&mut context, producer);
            }
            3 => {
                let value = f.read.result(&context);
                SemanticTypedBinaryOp::new(
                    &mut context,
                    SemanticTypedBinaryKindAttr::Add,
                    SemanticOverflowAttr::Wrapping,
                    scalar(),
                    value,
                    value,
                );
            }
            _ => unreachable!(),
        }
        assert!(
            matches!(
                prove_live_pliron_semantic_memory_v1(&context, &f.function),
                Err(E::UnsupportedControlFlow | E::InvalidGraph)
            ),
            "case {case}"
        );
    }
}

#[test]
fn cross_branch_conflicting_writer_is_not_hidden_by_sparse_read_coverage() {
    let mut context = setup();
    let (f, producer, _) = split(&mut context);
    let writer = block(&mut context, &f.function, "writer");
    let zero = IndexConstantOp::new(&mut context, 0);
    zero.get_operation()
        .insert_before(&context, f.index.get_operation());
    move_to(&context, &f.write, writer);
    Operation::replace_operand(f.write.get_operation(), &context, 1, zero.result(&context));
    ReturnOp::new(&mut context)
        .get_operation()
        .insert_at_back(writer, &context);
    guard(&mut context, &f, 16, producer, writer);
    assert!(matches!(
        prove_live_pliron_semantic_memory_v1(&context, &f.function),
        Err(E::Interference { .. })
    ));
}

#[test]
fn duplicate_destination_edges_preserve_selected_argument_tuple_and_slot() {
    let mut context = setup();
    let (f, producer, _) = split(&mut context);
    let entry = f.function.get_entry_block(&context);
    let old = entry.deref(&context).get_terminator(&context).unwrap();
    Operation::erase(old, &mut context);
    Operation::erase(f.write.get_operation(), &mut context);
    BasicBlock::push_argument(producer, &context, IndexType::get(&context).into());
    let argument = producer.deref(&context).get_argument(0);
    Operation::replace_operand(f.access.get_operation(), &context, 1, argument);
    Operation::replace_operand(f.read.get_operation(), &context, 1, argument);
    let limit = IndexConstantOp::new(&mut context, 16);
    append(&context, &f.function, &limit);
    let zero = IndexConstantOp::new(&mut context, 0);
    append(&context, &f.function, &zero);
    let index = f.index.result(&context);
    let bound = limit.result(&context);
    let zero_value = zero.result(&context);
    let branch = IndexLessThanBranchArgsOp::new(
        &mut context,
        index,
        bound,
        vec![index],
        vec![zero_value],
        producer,
        producer,
    );
    append(&context, &f.function, &branch);
    let (_, traces) = traces(&context, &f.function);
    for (i, trace) in traces.iter().enumerate() {
        assert_eq!(trace.blocks[0].successor, Some(usize::from(i >= 16)));
    }
    let proof = prove_live_pliron_semantic_memory_v1(&context, &f.function).unwrap();
    proof
        .with_live_reads(&context, &f.function, |reads| {
            assert_eq!(reads[0].instances.len(), 64);
            for instance in &reads[0].instances {
                assert_eq!(
                    instance.indices,
                    [if instance.invocation < 16 {
                        instance.invocation as u64
                    } else {
                        0
                    }]
                );
            }
        })
        .unwrap();
    // An incomplete edge tuple cannot be interpreted as the other edge's tuple.
    Operation::remove_operand(branch.get_operation(), &context, 3);
    assert!(matches!(
        prove_live_pliron_semantic_memory_v1(&context, &f.function),
        Err(E::InvalidGraph)
    ));
}

#[test]
fn function_signature_must_match_the_actual_entry_arguments() {
    use pliron::builtin::attributes::TypeAttr;
    for case in 0..4 {
        let mut context = setup();
        let f = fixture(&mut context);
        let index_type = IndexType::get(&context).into();
        if case != 1 {
            BasicBlock::push_argument(f.function.get_entry_block(&context), &context, index_type);
        }
        if case != 0 {
            let argument = if case == 2 {
                SemanticScalarType::get(&context).into()
            } else {
                index_type
            };
            let ty = FunctionType::get(&context, vec![argument], vec![]);
            f.function
                .set_attr_func_type(&mut context, TypeAttr::new(ty.into()));
        }
        let result = prove_live_pliron_semantic_memory_v1(&context, &f.function);
        if case == 3 {
            assert!(result.is_ok());
        } else {
            assert!(matches!(result, Err(E::InvalidGraph)), "case {case}");
        }
    }
}

#[test]
fn typed_block_argument_transport_is_not_implicitly_a_read_proof() {
    let mut context = setup();
    let (f, producer, _) = split(&mut context);
    BasicBlock::push_argument(producer, &context, SemanticScalarType::get(&context).into());
    assert!(matches!(
        prove_live_pliron_semantic_memory_v1(&context, &f.function),
        Err(E::UnsupportedControlFlow)
    ));
}

#[test]
fn attribute_count_and_key_size_are_bounded_before_schema_verification() {
    use pliron::builtin::attributes::StringAttr;
    for long_key in [false, true] {
        let mut context = setup();
        let f = fixture(&mut context);
        let mut raw = f.index.get_operation().deref_mut(&context);
        if long_key {
            raw.attributes.set(
                "a".repeat(129).try_into().unwrap(),
                StringAttr::new("x".into()),
            );
        } else {
            for i in 0..65 {
                raw.attributes.set(
                    format!("extra_{i}").try_into().unwrap(),
                    StringAttr::new("x".into()),
                );
            }
        }
        drop(raw);
        assert!(matches!(
            prove_live_pliron_semantic_memory_v1(&context, &f.function),
            Err(E::ResourceLimit)
        ));
    }
}

#[test]
fn native_same_block_verification_distance_is_charged_before_identity() {
    let mut costs = Vec::new();
    for chained in [false, true] {
        let mut context = setup();
        let f = fixture(&mut context);
        let seed = f.index.result(&context);
        let mut previous = seed;
        for _ in 0..32 {
            let op =
                IndexUnsignedCastOp::new(&mut context, if chained { previous } else { seed }, 64);
            op.get_operation()
                .insert_before(&context, f.ret.get_operation());
            previous = op.result(&context);
        }
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &f.function).unwrap();
        let mut work = control_flow::Work::new(MAX_PLIRON_TRACE_TOTAL_STEPS_V1);
        control_flow::validate(&context, &f.function, &inventory, &mut work).unwrap();
        costs.push(work.used());
    }
    assert_eq!(costs[0] - costs[1], 5 * 31 + 32 * 31 / 2);
}

#[test]
fn restored_control_edge_does_not_revive_a_retained_memory_proof() {
    let mut context = setup();
    let (f, producer, consumer) = split(&mut context);
    let proof = prove_live_pliron_semantic_memory_v1(&context, &f.function).unwrap();
    let entry = f.function.get_entry_block(&context);
    let term = entry.deref(&context).get_terminator(&context).unwrap();
    Operation::replace_successor(term, &context, 0, consumer);
    Operation::replace_successor(term, &context, 0, producer);
    assert_eq!(
        proof.revalidate(&context, &f.function),
        Err(E::MutationEpochChanged)
    );
    assert!(prove_live_pliron_semantic_memory_v1(&context, &f.function).is_ok());
}

#[test]
fn block_attribute_keys_have_the_same_preverification_bounds() {
    use pliron::builtin::attributes::StringAttr;
    for long_key in [false, true] {
        let mut context = setup();
        let f = fixture(&mut context);
        let entry = f.function.get_entry_block(&context);
        let mut raw = entry.deref_mut(&context);
        if long_key {
            raw.attributes.set(
                "a".repeat(129).try_into().unwrap(),
                StringAttr::new("x".into()),
            );
        } else {
            for i in 0..65 {
                raw.attributes.set(
                    format!("extra_{i}").try_into().unwrap(),
                    StringAttr::new("x".into()),
                );
            }
        }
        drop(raw);
        assert!(matches!(
            prove_live_pliron_semantic_memory_v1(&context, &f.function),
            Err(E::ResourceLimit)
        ));
    }
}

#[test]
fn mismatched_signature_cache_miss_does_not_forgive_a_mutation_attempt() {
    let mut context = setup();
    let f = fixture(&mut context);
    BasicBlock::push_argument(
        f.function.get_entry_block(&context),
        &context,
        IndexType::get(&context).into(),
    );
    let before = epoch(&context).unwrap();
    assert!(matches!(
        prove_live_pliron_semantic_memory_v1(&context, &f.function),
        Err(E::InvalidGraph)
    ));
    let after = epoch(&context).unwrap();
    assert!(after > before);
    assert!(matches!(
        prove_live_pliron_semantic_memory_v1(&context, &f.function),
        Err(E::InvalidGraph)
    ));
    assert_eq!(epoch(&context).unwrap(), after);
}
