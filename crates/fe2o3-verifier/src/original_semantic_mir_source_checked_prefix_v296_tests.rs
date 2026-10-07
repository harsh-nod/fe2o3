//! Emission/owner tests only; generated laws still require Verus verification.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
const LIMIT: usize = 512 * 1024 * 1024;

#[test]
fn checked_prefix_scanner_headers_cover_owner_and_two_live_queries() {
    assert_eq!(
        headers(),
        size_of::<FramePlan<'_, '_, '_, '_>>()
            + size_of::<Result<FramePlan<'_, '_, '_, '_>>>()
            + 2 * size_of::<Prefix<'_, '_, '_, '_, '_>>()
            + 2 * size_of::<Result<Prefix<'_, '_, '_, '_, '_>>>()
            + 2 * size_of::<Event>()
            + 2 * size_of::<Result<Event>>()
            + 2 * size_of::<Option<usize>>()
            + size_of::<Option<(usize, usize, usize)>>()
            + 4 * size_of::<Range<usize>>()
            + 20 * size_of::<usize>()
            + 14 * size_of::<&()>()
            + 4 * size_of::<Result<()>>()
    );
}

fn fixture(work: usize, storage: usize) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| {
            super::super::super::paired::aggregate_tests::checked_transform(
                types,
                functions,
                SemanticCheckedBinaryOpV1::Add,
                false,
                true,
            );
            let old = functions.last_mut().unwrap();
            let source = old.source();
            let mut locals = old.locals().to_vec();
            let pair = locals[4].ty();
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([250; 32]),
                pair,
                SemanticLocalRoleV1::Temporary,
                source,
            ));
            let mut blocks = old.blocks().to_vec();
            let mut statements = blocks[0].statements().to_vec();
            let SemanticStatementKindV1::Assign(original) = statements.last().unwrap().kind()
            else {
                panic!("checked assignment")
            };
            let value = original.value().clone();
            statements.push(SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(5), vec![], pair).unwrap(),
                    value,
                )),
            ));
            blocks[0] = SemanticBasicBlockV1::new(
                blocks[0].identity(),
                source,
                statements,
                blocks[0].terminator().clone(),
            )
            .unwrap();
            *old = SemanticFunctionDeclV1::new(
                old.identity(),
                old.role(),
                old.item_definition_identity(),
                old.monomorphization_identity(),
                old.generic_type_arguments_identity(),
                old.const_generic_arguments_identity(),
                source,
                old.abi().clone(),
                locals,
                old.entry(),
                blocks,
            )
            .unwrap();
        },
        |plan, out| {
            super::super::tests::with_slots(plan, out, |slots, out| {
                let program = SourceByteProgram::derive(plan, slots, out)?;
                let expected = program.emit_checked_local_add_proofs_v288(out)?;
                let before = out.text.len();
                let count = program.emit_checked_prefix_projections_v296(plan, out)?;
                assert!(count > 0);
                assert_eq!(count, expected);
                let emitted = &out.text[before..];
                assert_eq!(
                    emitted
                        .matches("proof fn checked_add_actual_demanded_step_")
                        .count(),
                    count
                );
                assert_eq!(
                    emitted
                        .matches("proof fn checked_add_actual_demanded_prefix_")
                        .count(),
                    count
                );
                assert_eq!(
                    emitted.matches("spec fn checked_prefix_demands_").count(),
                    count
                );
                assert!(emitted.contains(
                    ".leaves[seq![0int]] == MemoryValueV30::Scalar((left + right) % 4294967296)"
                ));
                assert!(emitted.contains(
                    ".leaves[seq![1int]] == MemoryValueV30::Scalar(if left + right >= 4294967296"
                ));
                assert!(
                    emitted.contains("invocation_source_stored_aggregate_projection_v296(after,")
                );
                assert!(emitted.contains("== before.machine.values["));
                for tail in emitted
                    .split("proof fn checked_add_actual_demanded_step_")
                    .skip(1)
                {
                    let declaration = tail.split("\nproof fn ").next().unwrap();
                    let (requires, ensures) = declaration.split_once("\n ensures").unwrap();
                    assert!(requires.contains("c.source.machine.pc == "));
                    assert!(requires.contains("c.next_statement == c.observations.len()"));
                    assert!(!requires.contains("n.source"));
                    assert!(!requires.contains("target"));
                    assert!(
                        ensures.contains("invocation_source_byte_state_well_formed_v36(n.source)")
                    );
                    assert!(ensures.contains("checked_prefix_demands_"));
                    assert!(!ensures.contains("invocation_expanded_frame_contract_"));
                    assert!(!ensures.contains("execution_map_current"));
                }
                for tail in emitted
                    .split("proof fn checked_add_actual_demanded_prefix_")
                    .skip(1)
                {
                    let declaration = tail.split("\nproof fn ").next().unwrap();
                    let (requires, ensures) = declaration.split_once("\n ensures").unwrap();
                    assert!(requires.contains("let p = invocation_source_micro_run_"));
                    assert!(!requires.contains("n.source"));
                    assert!(!requires.contains("target"));
                    assert!(ensures.contains("_v296(p.source, n.source, left, right)"));
                    assert!(!ensures.contains("invocation_expanded_frame_contract_"));
                }
                Ok(())
            })
        },
    )
}

#[test]
fn checked_prefix_consumers_emit_only_actual_source_demand_projections() {
    fixture(LIMIT, LIMIT).0.unwrap();
}

#[test]
fn checked_prefix_consumers_refuse_mismatched_query_coordinates_before_emission() {
    super::super::super::super::invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        |types, functions| {
            super::super::super::paired::aggregate_tests::checked_transform(
                types,
                functions,
                SemanticCheckedBinaryOpV1::Add,
                false,
                false,
            )
        },
        |plan, out| {
            super::super::tests::with_slots(plan, out, |slots, out| {
                let program = SourceByteProgram::derive(plan, slots, out)?;
                let frames = FramePlan::derive(plan, slots, out)?;
                let mut witnesses = 0;
                for root in 0..program.roots.len() {
                    let range = &program.roots[root].0;
                    for instance in 0..range.len() {
                        let Some(function) = &program.functions[range.start + instance] else {
                            continue;
                        };
                        for (block, row) in function.control.iter().enumerate() {
                            for statement in 0..row.statements {
                                let event = function.body.event_at(block, statement, out)?;
                                if event.checked_prefix_site_v296().is_none() {
                                    continue;
                                }
                                let before = frames
                                    .partial_prefix_v296(root, instance, block, statement, out)?;
                                let mut after = frames.partial_prefix_v296(
                                    root,
                                    instance,
                                    block,
                                    statement + 1,
                                    out,
                                )?;
                                let text = out.text.len();
                                for which in 0..4 {
                                    let saved =
                                        (after.frame, after.block, after.statement, after.pc);
                                    match which {
                                        0 => after.frame = usize::MAX,
                                        1 => after.block = usize::MAX,
                                        2 => after.statement = before.statement,
                                        _ => after.pc = usize::MAX,
                                    }
                                    assert!(matches!(
                                        event.emit_checked_prefix_v296(
                                            &frames, &before, &after, out
                                        ),
                                        Err(Error::Statement(_))
                                    ));
                                    assert_eq!(out.text.len(), text);
                                    (after.frame, after.block, after.statement, after.pc) = saved;
                                }
                                after.discard(out)?;
                                before.discard(out)?;
                                witnesses += 1;
                            }
                        }
                    }
                }
                assert!(witnesses > 0);
                Ok(())
            })
        },
    )
    .0
    .unwrap();
}

#[test]
fn checked_prefix_consumers_have_exact_work_storage_and_finish_boundaries() {
    let baseline = fixture(LIMIT, LIMIT);
    baseline.0.unwrap();
    let exact = fixture(baseline.1, baseline.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (baseline.1, baseline.2, baseline.3)
    );
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    assert!(matches!(fixture(baseline.1 - 1, baseline.3).0,
        Err(Error::Resource(Resource::Work(error))) | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
        if error.actual() == baseline.1 && error.limit() == baseline.1 - 1));
    assert!(matches!(fixture(baseline.1, baseline.3 - 1).0,
        Err(Error::Resource(Resource::Storage(error))) | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
        if error.actual() == baseline.3 && error.limit() == baseline.3 - 1));
}
