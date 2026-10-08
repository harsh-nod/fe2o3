//! Authentic selection/emission/accounting checks; these do not execute Verus.
use super::super::super::super::super::super::invocations::tests as fixtures;
use super::super::super::super::super::slots::tests::with_tile_slots;
use super::super::super::super::super::source_bytes::{Event, SourceByteBody};
use super::super::super::super::super::source_scalar::SourceScalarStatements;
use super::*;
use fe2o3_kernel_ir::ExecutionTileLayoutV1 as Layout;

const LIMIT: usize = 512 * 1024 * 1024;

fn scalar_fixture(
    functions: &mut Vec<fe2o3_mir_model::semantic_mir_v1::SemanticFunctionDeclV1>,
    callables: &mut Vec<fe2o3_mir_model::semantic_mir_v1::SemanticCallableDeclV1>,
    nested: bool,
    moved: bool,
    transfer: bool,
) {
    use fe2o3_mir_model::semantic_mir_v1::*;
    let index = functions.len() - 1;
    let helper = functions[index].clone();
    let source = helper.source();
    let word = helper.locals()[0].ty();
    let place =
        |local| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], word).unwrap();
    let assign = |local, value| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local),
                SemanticRvalueV1::new(word, value),
            )),
        )
    };
    let statements = vec![
        assign(
            3,
            if transfer {
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1)))
            } else {
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::BitOr,
                    left: SemanticOperandV1::Copy(place(1)),
                    right: SemanticOperandV1::Copy(place(1)),
                }
            },
        ),
        assign(
            3,
            SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::Not,
                operand: SemanticOperandV1::Copy(place(3)),
            },
        ),
        assign(
            0,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::BitAnd,
                left: if moved {
                    SemanticOperandV1::Move(place(3))
                } else {
                    SemanticOperandV1::Copy(place(3))
                },
                right: SemanticOperandV1::Copy(place(if moved { 1 } else { 3 })),
            },
        ),
    ];
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(source, terminator),
        )
        .unwrap()
    };
    let scalar = SemanticFunctionDeclV1::new(
        helper.identity(),
        helper.role(),
        helper.item_definition_identity(),
        helper.monomorphization_identity(),
        helper.generic_type_arguments_identity(),
        helper.const_generic_arguments_identity(),
        source,
        helper.abi().clone(),
        helper.locals().to_vec(),
        SemanticBlockIdV1::from_index(0),
        vec![block(230, statements, SemanticTerminatorKindV1::Return)],
    )
    .unwrap();
    if nested {
        let child = functions.len() as u32;
        functions[index] = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([254; 32]),
            helper.role(),
            SemanticItemDefinitionIdentityV1::from_sha256([254; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([254; 32]),
            helper.generic_type_arguments_identity(),
            helper.const_generic_arguments_identity(),
            source,
            helper.abi().clone(),
            helper.locals().to_vec(),
            SemanticBlockIdV1::from_index(0),
            vec![
                block(
                    231,
                    vec![],
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new(
                            SemanticFunctionIdV1::from_index(child),
                            vec![
                                SemanticOperandV1::Copy(place(1)),
                                SemanticOperandV1::Copy(place(2)),
                            ],
                            Some(SemanticCallDestinationV1::new(
                                place(0),
                                SemanticControlFlowEdgeV1::new(
                                    SemanticEdgeRoleV1::CallReturn,
                                    SemanticBlockIdV1::from_index(1),
                                ),
                            )),
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                ),
                block(232, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
        .unwrap();
        functions.push(scalar);
        callables.push(SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(child),
        ));
    } else {
        functions[index] = scalar;
    }
}

fn body<'a>(text: &'a str, root: usize, pc: usize, goal: &str) -> &'a str {
    text.split_once(&format!(
        "proof fn invocation_paired_cut_{root}_pc{pc}_{goal}_v85("
    ))
    .unwrap()
    .1
    .split("proof fn ")
    .next()
    .unwrap()
}

fn run(
    layout: Layout,
    work: usize,
    storage: usize,
    enabled: bool,
    examine: impl FnOnce(&str),
) -> (Result<()>, usize, usize, usize) {
    fixtures::run_variant(work, storage, false, |plan, out| {
        with_tile_slots(plan, layout, out, |slots, out| {
            let mut program = SourceByteProgram::derive(plan, slots, out)?;
            let mut paired =
                PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
            let mut selected = Vec::new();
            for (root, row) in paired.roots.iter().enumerate() {
                let allocation = root_returns::scan(&paired, root, out)?;
                let routes = scan(&paired, root, out)?;
                for (block, cut) in row.cuts.iter().enumerate() {
                    let Some(cut) = cut else { continue };
                    let hint = row
                        .step_hints
                        .as_ref()
                        .unwrap()
                        .cuts
                        .iter()
                        .find(|hint| hint.pc == cut.source)
                        .unwrap();
                    if !matches!(cut.end, End::Return) {
                        assert!(hint.scalar_return_v325.is_none());
                    }
                    if let Some(summary) =
                        derive(&paired, &allocation, &routes, root, block, hint, out)?
                    {
                        assert!(hint.instance > 0);
                        assert!(summary.source.statements.len() >= 2);
                        assert!(summary.target_fuel > 1, "real empty return bridge");
                        assert_eq!(summary.target_pc, row.blocks.start + block);
                        assert!(
                            summary
                                .source
                                .statements
                                .windows(2)
                                .all(|pair| pair[0].graph != pair[1].graph)
                        );
                        selected.push((
                            root,
                            hint.pc,
                            hint.instance,
                            hint.statements,
                            summary.source.block,
                            summary.source.returned.locals.clone(),
                            summary.source.returned.destination,
                            summary.source.returned.continuation,
                            summary.target_result,
                        ));
                    }
                }
            }
            assert!(selected.len() >= 4);
            assert!(selected.iter().any(|row| row.0 > 0));
            if !enabled {
                for root in &mut paired.roots {
                    for hint in &mut root.step_hints.as_mut().unwrap().cuts {
                        hint.scalar_return_v325 = None;
                    }
                }
            }
            program.emit(out)?;
            paired.emit(out)?;
            program.emit_cut_frame_proofs_v93(Some(&paired), out)?;
            super::super::super::super::super::support_closure::retain_referenced(out)?;
            for (
                root,
                pc,
                instance,
                statements,
                block,
                locals,
                destination,
                continuation,
                target_result,
            ) in selected
            {
                for goal in ["heap", "control", "halted"] {
                    let emitted = body(&out.text, root, pc, goal);
                    assert!(emitted.contains(&format!("requires invocation_paired_related_{root}_v36(source, target), invocation_paired_source_defined_{root}_v36(source, 1),\n source.machine.pc == {pc},")));
                    if enabled {
                        for ordinal in 0..statements {
                            assert!(emitted.contains(&format!("hide(invocation_source_scalar_{root}_{instance}_{block}_{ordinal}_v36);")));
                            assert!(emitted.contains(&format!(
                                "invocation_source_put_local_preserves_heap_v78(state_{ordinal},"
                            )));
                        }
                        assert!(emitted.contains("assert(original == returned.source);"));
                        assert!(emitted.contains(&format!(
                            "local: {destination}, component: None, memory: None, descriptor: None"
                        )));
                        assert!(!emitted.contains("MemoryValueV30::Unit, None, -1int"));
                        if goal == "heap" {
                            assert!(emitted.contains(&format!("invocation_source_plain_return_preserves_heap_v78(state_{statements}, {}, {}, value, destination, {continuation}, little_endian);", locals.start, locals.end)));
                            assert!(emitted.contains("invocation_empty_private_map_has_no_private_source_v78(source.machine.memory, target.memory, map);"));
                        } else if goal == "control" {
                            assert!(emitted.contains(&format!(
                                "assert(value == actual.values[{target_result}])"
                            )));
                        }
                    } else {
                        assert!(!emitted.contains("let state_0 = source;"));
                        assert!(emitted.contains(&format!("reveal_with_fuel(invocation_source_micro_run_{root}_{instance}_v36, {});", statements + 1)));
                    }
                    let mut ordinary = false;
                    for line in emitted
                        .split_once("\n{\n")
                        .unwrap()
                        .1
                        .lines()
                        .map(str::trim)
                    {
                        if line.starts_with("hide(") {
                            assert!(!ordinary);
                        } else if !line.is_empty() {
                            ordinary = true;
                        }
                    }
                    for forbidden in ["assume(", "admit(", "external_body"] {
                        assert!(!emitted.contains(forbidden));
                    }
                }
            }
            examine(&out.text);
            Ok(())
        })
    })
}

#[test]
fn scalar_plain_return_summary_joins_multiple_real_sites_and_preserves_complete_headers() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let mut baseline = String::new();
        run(layout, LIMIT, LIMIT, false, |text| {
            baseline = text.to_owned()
        })
        .0
        .unwrap();
        run(layout, LIMIT, LIMIT, true, |text| {
            let headers = |text: &str| {
                text.split("proof fn ")
                    .skip(1)
                    .map(|body| body.split_once("\n{").unwrap().0.to_owned())
                    .collect::<Vec<_>>()
            };
            assert_eq!(headers(text), headers(&baseline));
            let recursive = |text: &str| {
                text.lines()
                    .filter(|line| line.trim_start().starts_with("decreases "))
                    .count()
            };
            assert_eq!(recursive(text), recursive(&baseline));
            assert_ne!(text, baseline);
        })
        .0
        .unwrap();
    }
}

#[test]
fn scalar_plain_return_summary_rejects_missing_foreign_and_unsupported_coordinates() {
    fixtures::run_variant(LIMIT, LIMIT, false, |plan, out| {
        with_tile_slots(plan, Layout::Blocked, out, |slots, out| {
            let program = SourceByteProgram::derive(plan, slots, out)?;
            let mut paired =
                PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
            let root = 1;
            let hints = paired.roots[root].step_hints.as_mut().unwrap();
            let index = hints
                .cuts
                .iter()
                .position(|hint| hint.scalar_return_v325.is_some())
                .unwrap();
            let mut hint = hints.cuts.remove(index);
            let block = paired.roots[root]
                .cuts
                .iter()
                .position(|cut| cut.as_ref().is_some_and(|cut| cut.source == hint.pc))
                .unwrap();
            let allocation = root_returns::scan(&paired, root, out)?;
            let routes = scan(&paired, root, out)?;
            assert!(derive(&paired, &allocation, &routes, root, block, &hint, out)?.is_some());
            let saved = hint.scalar_return_v325.take();
            assert!(derive(&paired, &allocation, &routes, root, block, &hint, out)?.is_none());
            hint.scalar_return_v325 = saved;
            hint.scalar_return_v325.as_mut().unwrap().root += 1;
            assert!(matches!(
                derive(&paired, &allocation, &routes, root, block, &hint, out),
                Err(Error::Statement(_))
            ));
            hint.scalar_return_v325.as_mut().unwrap().root -= 1;
            hint.scalar_return_v325.as_mut().unwrap().instance += 1;
            assert!(matches!(
                derive(&paired, &allocation, &routes, root, block, &hint, out),
                Err(Error::Statement(_))
            ));
            hint.scalar_return_v325.as_mut().unwrap().instance -= 1;
            hint.scalar_return_v325.as_mut().unwrap().pc += 1;
            assert!(matches!(
                derive(&paired, &allocation, &routes, root, block, &hint, out),
                Err(Error::Statement(_))
            ));
            hint.scalar_return_v325.as_mut().unwrap().pc -= 1;
            let saved_owner = hint.scalar_return_v325.as_ref().unwrap().returned.owner;
            hint.scalar_return_v325.as_mut().unwrap().returned.owner = u32::MAX;
            assert!(matches!(
                derive(&paired, &allocation, &routes, root, block, &hint, out),
                Err(Error::Statement(_))
            ));
            hint.scalar_return_v325.as_mut().unwrap().returned.owner = saved_owner;
            let old = hint
                .scalar_return_v325
                .as_ref()
                .unwrap()
                .returned
                .destination;
            hint.scalar_return_v325
                .as_mut()
                .unwrap()
                .returned
                .destination = usize::MAX;
            assert!(derive(&paired, &allocation, &routes, root, block, &hint, out)?.is_none());
            hint.scalar_return_v325
                .as_mut()
                .unwrap()
                .returned
                .destination = old;
            let other_routes = scan(&paired, 0, out)?;
            // Same-account coordinate refusals still authenticate the retained model.
            let validation_before = (
                out.budget.work(),
                out.budget.storage(),
                out.budget.peak_storage(),
                out.text.len(),
            );
            paired.check(out)?;
            let validation_work = out.budget.work() - validation_before.0;
            assert!(validation_work > 0);
            assert_eq!(
                (
                    out.budget.storage(),
                    out.budget.peak_storage(),
                    out.text.len()
                ),
                (
                    validation_before.1,
                    validation_before.2,
                    validation_before.3
                )
            );
            let before = (
                out.budget.work(),
                out.budget.storage(),
                out.budget.peak_storage(),
                out.text.len(),
            );
            assert!(matches!(
                derive(&paired, &allocation, &other_routes, root, block, &hint, out),
                Err(Error::Statement(_))
            ));
            assert_eq!(
                (
                    out.budget.work(),
                    out.budget.storage(),
                    out.budget.peak_storage(),
                    out.text.len(),
                ),
                (before.0 + validation_work, before.1, before.2, before.3)
            );
            let other_allocation = root_returns::scan(&paired, 0, out)?;
            let before = (
                out.budget.work(),
                out.budget.storage(),
                out.budget.peak_storage(),
                out.text.len(),
            );
            assert!(matches!(
                derive(&paired, &other_allocation, &routes, root, block, &hint, out),
                Err(Error::Statement(_))
            ));
            assert_eq!(
                (
                    out.budget.work(),
                    out.budget.storage(),
                    out.budget.peak_storage(),
                    out.text.len(),
                ),
                (before.0 + 2 * validation_work, before.1, before.2, before.3)
            );
            assert!(out.text.is_empty());
            Ok(())
        })
    })
    .0
    .unwrap();
    for unit in [false, true] {
        let check = |plan: &mut InvocationPlan<'_, '_>, out: &mut Writer<'_, '_>| {
            with_tile_slots(plan, Layout::Striped, out, |slots, out| {
                let program = SourceByteProgram::derive(plan, slots, out)?;
                let paired =
                    PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                for (root, row) in paired.roots.iter().enumerate() {
                    let allocation = root_returns::scan(&paired, root, out)?;
                    let routes = scan(&paired, root, out)?;
                    for (block, cut) in row.cuts.iter().enumerate() {
                        let Some(cut) = cut else { continue };
                        let hint = row
                            .step_hints
                            .as_ref()
                            .unwrap()
                            .cuts
                            .iter()
                            .find(|hint| hint.pc == cut.source)
                            .unwrap();
                        assert!(
                            derive(&paired, &allocation, &routes, root, block, hint, out)?
                                .is_none()
                        );
                    }
                }
                Ok(())
            })
        };
        if unit {
            fixtures::run_variant(LIMIT, LIMIT, true, check).0.unwrap();
        } else {
            fixtures::run_scalar_allocation_variant(LIMIT, LIMIT, check)
                .0
                .unwrap();
        }
    }
}

#[test]
fn scalar_plain_return_summary_foreign_and_refunded_accounts_fail_before_debit() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    for foreign in [false, true] {
        let result = fixtures::run_variant(LIMIT, LIMIT, false, |plan, out| {
            with_tile_slots(plan, Layout::Blocked, out, |slots, out| {
                let program = SourceByteProgram::derive(plan, slots, out)?;
                let paired =
                    PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                let row = &paired.roots[0];
                let hint = row
                    .step_hints
                    .as_ref()
                    .unwrap()
                    .cuts
                    .iter()
                    .find(|hint| hint.scalar_return_v325.is_some())
                    .unwrap();
                let block = row
                    .cuts
                    .iter()
                    .position(|cut| cut.as_ref().is_some_and(|cut| cut.source == hint.pc))
                    .unwrap();
                let allocation = root_returns::scan(&paired, 0, out)?;
                let routes = scan(&paired, 0, out)?;
                if foreign {
                    let mut work = Work::new(LIMIT);
                    let mut budget = Budget::new(&mut work, LIMIT);
                    budget.reserve_storage(out.budget.storage())?;
                    let before = (budget.work(), budget.storage(), budget.peak_storage());
                    {
                        let mut writer = Writer::new(&mut budget)?;
                        assert!(matches!(
                            derive(&paired, &allocation, &routes, 0, block, hint, &mut writer),
                            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                        ));
                        assert!(writer.text.is_empty());
                    }
                    assert_eq!(
                        (budget.work(), budget.storage(), budget.peak_storage()),
                        before
                    );
                }
                out.budget
                    .release_storage(out.budget.storage() - routes.required + 1)?;
                let before = (
                    out.budget.work(),
                    out.budget.storage(),
                    out.budget.peak_storage(),
                );
                assert!(matches!(
                    derive(&paired, &allocation, &routes, 0, block, hint, out),
                    Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                ));
                assert_eq!(
                    (
                        out.budget.work(),
                        out.budget.storage(),
                        out.budget.peak_storage()
                    ),
                    before
                );
                derive(&paired, &allocation, &routes, 0, block, hint, out).map(|_| ())
            })
        });
        assert!(matches!(
            result.0,
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
    }
}

#[test]
fn scalar_plain_return_summary_exact_and_one_short_accounts_include_full_output() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let measured = run(layout, LIMIT, LIMIT, true, |_| {});
        measured.0.unwrap();
        assert_eq!(measured.2, fixtures::FLOOR);
        let exact = run(layout, measured.1, measured.3, true, |_| {});
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (measured.1, fixtures::FLOOR, measured.3)
        );
        for (work, storage) in [(measured.1 - 1, measured.3), (measured.1, measured.3 - 1)] {
            let short = run(layout, work, storage, true, |_| {});
            assert!(short.0.is_err());
            assert_eq!(short.2, fixtures::FLOOR);
        }
    }
}

#[test]
fn scalar_plain_return_summary_uses_genuine_alias_dedup_zero_and_nested_owner_coordinates() {
    let mut coverage = Vec::new();
    for layout in [Layout::Blocked, Layout::Striped] {
        for nested in [false, true] {
            fixtures::run_callable_transform(LIMIT, LIMIT,
                |_, functions, callables| scalar_fixture(functions, callables, nested, false, false),
                |plan, out| with_tile_slots(plan, layout, out, |slots, out| {
                    let mut program = SourceByteProgram::derive(plan, slots, out)?;
                    let paired = PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                    let inventory = paired.slots.correspondence(out)?.inventory(out.budget)?;
                    let (mut scalar_count, mut empty_count) = (0, 0);
                    let source_hint_count: usize = paired.roots.iter().map(|row|
                        row.step_hints.as_ref().unwrap().cuts.iter()
                            .filter(|hint| hint.scalar_return_v325.is_some()).count()).sum();
                    let mut joined_hint_count = 0;
                    let mut coordinates = Vec::new();
                    for (root, row) in paired.roots.iter().enumerate() {
                        let allocation = root_returns::scan(&paired, root, out)?;
                        let routes = scan(&paired, root, out)?;
                        let allocation_free = allocation.allocation_free(&paired, root, out)?;
                        for (block, cut) in row.cuts.iter().enumerate() {
                            let Some(cut) = cut else { continue };
                            let hint = row.step_hints.as_ref().unwrap().cuts.iter().find(|hint| hint.pc == cut.source).unwrap();
                            let summary = derive(&paired, &allocation, &routes, root, block, hint, out)?;
                            let candidate = hint.scalar_return_v325.as_ref().map(|source| source.statements.len());
                            joined_hint_count += usize::from(candidate.is_some());
                            assert!(coordinates.len() < 64, "bounded fixture coordinate census");
                            coordinates.push((root, hint.instance, hint.pc, row.blocks.start + block, candidate, summary.is_some()));
                            if let Some(source) = &hint.scalar_return_v325 {
                                let first = &inventory.blocks()[row.blocks.start + block];
                                let owner = paired.instances[cut.instance].as_ref().unwrap();
                                let binding = owner.returned.map(|binding| (
                                    binding.frame, binding.definition,
                                    match binding.source { SourceValue::Local(local) => Some(local), _ => None },
                                    matches!(binding.logical, LogicalBinding::Plain),
                                    binding.definition.map(|id| inventory.definitions()[id].ty.as_scalar()),
                                ));
                                let edge = inventory.edges().get(first.edges.start).filter(|_| !first.edges.is_empty());
                                let transfer = edge.and_then(|edge| inventory.edge_arguments().get(edge.bindings.start)
                                    .filter(|_| !edge.bindings.is_empty())).map(|binding| (
                                        binding.incoming_definition, binding.target_definition,
                                        inventory.definitions()[binding.incoming_definition].ty.as_scalar(),
                                        inventory.definitions()[binding.target_definition].ty.as_scalar(),
                                    ));
                                let route = edge.and_then(|edge| routes.rows.get(edge.target.block as usize)).copied();
                                let continuation = match route {
                                    Some(Route::Cut { block, .. }) => row.cuts[block].as_ref().map(|next| {
                                        let parent = paired.instances[next.instance].as_ref().unwrap();
                                        (next.source, parent.owners.len(), parent.owners.as_slice()
                                            == &owner.owners[..owner.owners.len() - 1])
                                    }),
                                    _ => None,
                                };
                                eprintln!("V328_SCALAR_RETURN_TARGET layout={layout:?} nested={nested} root={root} instance={} source_pc={} target_block={} allocation_free={allocation_free} ops={} edges={} branch={} binding(frame,definition,source_local,plain,scalar_type)={binding:?} edge(function,block,bindings)={:?} transfer(incoming,result,incoming_scalar,result_scalar)={transfer:?} route={route:?} continuation(pc,depth,owner_prefix)={continuation:?} source(destination,continuation,depth,bits)={:?}",
                                    hint.instance, hint.pc, row.blocks.start + block, first.operations.len(), first.edges.len(),
                                    matches!(first.terminator, KirTerminator::Branch { .. }),
                                    edge.map(|edge| (edge.target.function, edge.target.block, edge.bindings.len())),
                                    (source.returned.destination, source.returned.continuation, source.returned.depth, source.returned.bits));
                            }
                            let Some(summary) = summary else { continue };
                            if summary.source.statements.is_empty() {
                                assert!(nested);
                                assert_eq!(summary.source.returned.depth, 2);
                                empty_count += 1;
                            } else {
                                assert_eq!(summary.source.returned.depth, if nested { 3 } else { 2 });
                                let statements = &summary.source.statements;
                                assert_eq!(statements.len(), 3);
                                assert!(statements[0].inputs[0].is_some());
                                assert_eq!(statements[0].inputs[1], None);
                                assert_ne!(statements[0].inputs[0], Some(statements[0].destination));
                                assert_eq!(statements[0].destination, statements[1].destination);
                                assert_eq!(statements[1].inputs, [Some(statements[1].destination), None]);
                                assert_eq!(statements[2].inputs, [Some(statements[1].destination), None]);
                                scalar_count += 1;
                            }
                        }
                    }
                    eprintln!("V328_SCALAR_RETURN_SELECTION layout={layout:?} nested={nested} source_hints={source_hint_count} joined_hints={joined_hint_count} scalar={scalar_count} empty={empty_count} coordinates(root,instance,source_pc,target_block,candidate_statements,selected)={coordinates:?}");
                    program.emit(out)?;
                    paired.emit(out)?;
                    let emitted_values = out.text.contains("let value_2 = MemoryValueV30::Scalar(original_invocation_source_scalar_trace_");
                    assert!(coverage.len() < 4, "bounded fixture layout census");
                    coverage.push((layout, nested, scalar_count, empty_count, emitted_values));
                    Ok(())
                })).0.unwrap();
        }
        for (moved, transfer) in [(true, false), (false, true)] {
            fixtures::run_callable_transform(
                LIMIT,
                LIMIT,
                |_, functions, callables| {
                    scalar_fixture(functions, callables, false, moved, transfer)
                },
                |plan, out| {
                    with_tile_slots(plan, layout, out, |slots, out| {
                        let program = SourceByteProgram::derive(plan, slots, out)?;
                        let paired = PairedInvocations::derive(
                            plan,
                            &program,
                            FormalIndexWidth::Bits64,
                            out,
                        )?;
                        let mut observed = 0;
                        for (root, row) in paired.roots.iter().enumerate() {
                            for instance in 1..row.instances.len() {
                                if paired.instances[row.instances.start + instance].is_none() {
                                    continue;
                                }
                                let body =
                                    SourceByteBody::derive(plan, slots, root, instance, out)?;
                                let first = body.event_at(0, 0, out)?;
                                if transfer {
                                    assert!(matches!(first, Event::Transfer { .. }));
                                } else {
                                    assert!(matches!(first, Event::Scalar));
                                    assert!(matches!(body.event_at(0, 2, out)?, Event::Scalar));
                                }
                                let scalar =
                                    SourceScalarStatements::new(plan, slots, root, instance, out)?;
                                let rejected = if transfer { 0 } else { 2 };
                                assert!(
                                    scalar
                                        .copy_coordinates_v325(root, instance, 0, rejected, out)?
                                        .is_none()
                                );
                                observed += 1;
                            }
                            assert!(
                                row.step_hints
                                    .as_ref()
                                    .unwrap()
                                    .cuts
                                    .iter()
                                    .all(|hint| hint.scalar_return_v325.is_none())
                            );
                        }
                        assert_eq!(observed, 4);
                        Ok(())
                    })
                },
            )
            .0
            .unwrap();
        }
    }
    assert_eq!(coverage.len(), 4);
    for (layout, nested, scalar_count, empty_count, emitted_values) in coverage {
        assert!(
            scalar_count >= 4,
            "layout={layout:?} nested={nested} scalar={scalar_count}"
        );
        assert_eq!(empty_count > 0, nested, "layout={layout:?} nested={nested}");
        assert!(emitted_values, "layout={layout:?} nested={nested}");
    }
}

#[test]
fn scalar_plain_return_bridge_cache_is_linear_for_shared_paths_and_rejects_cycles() {
    // Synthetic graph tests exercise the actual cache algorithm, not source
    // admission or proof premises. Genuine target routes are checked above.
    fixtures::run_variant(LIMIT, LIMIT, false, |_, out| {
        for count in [8, 128] {
            let mut visits = vec![0; count];
            let routes = resolve_routes(
                count,
                |node, _| {
                    visits[node] += 1;
                    Ok(if node == count - 1 {
                        Bridge::Cut
                    } else if node < count / 2 {
                        Bridge::Next(count / 2)
                    } else {
                        Bridge::Next(node + 1)
                    })
                },
                out,
            )?;
            assert!(visits.iter().all(|&visits| visits == 1));
            assert_eq!(
                routes[0],
                Route::Cut {
                    block: count - 1,
                    hops: count / 2
                }
            );
            assert_eq!(
                routes[count - 1],
                Route::Cut {
                    block: count - 1,
                    hops: 0
                }
            );
            let cycle = resolve_routes(count, |node, _| Ok(Bridge::Next((node + 1) % count)), out)?;
            assert!(cycle.iter().all(|route| *route == Route::Unsupported));
            let unsupported = resolve_routes(
                count,
                |node, _| {
                    Ok(if node + 1 == count {
                        Bridge::Unsupported
                    } else {
                        Bridge::Next(node + 1)
                    })
                },
                out,
            )?;
            assert!(unsupported.iter().all(|route| *route == Route::Unsupported));
        }
        assert!(matches!(
            resolve_routes(1, |_, _| Ok(Bridge::Next(1)), out),
            Err(Error::Statement(_))
        ));
        assert!(bridge_shape(0, 1, 0, true));
        for (operations, edges, bindings, branch) in [
            (1, 1, 0, true),
            (0, 2, 0, true),
            (0, 1, 1, true),
            (0, 1, 0, false),
        ] {
            assert!(!bridge_shape(operations, edges, bindings, branch));
        }
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn scalar_plain_return_requires_exact_authenticated_scalar_width() {
    use fe2o3_kernel_ir::ScalarType;
    assert_eq!(
        scalar_width(&Type::Scalar(ScalarType::U32), FormalIndexWidth::Bits64),
        Some(32)
    );
    assert_eq!(
        scalar_width(&Type::Scalar(ScalarType::Bool), FormalIndexWidth::Bits64),
        Some(1)
    );
    assert_eq!(
        scalar_width(&Type::Scalar(ScalarType::F64), FormalIndexWidth::Bits32),
        Some(64)
    );
    assert_eq!(
        scalar_width(&Type::Scalar(ScalarType::Index), FormalIndexWidth::Bits32),
        Some(32)
    );
    assert_eq!(
        scalar_width(&Type::Scalar(ScalarType::Index), FormalIndexWidth::Bits64),
        Some(64)
    );
    assert_eq!(
        scalar_width(&Type::Scalar(ScalarType::Index), FormalIndexWidth::Unknown),
        None
    );
    assert_eq!(scalar_width(&Type::Unit, FormalIndexWidth::Bits64), None);
    fixtures::run_variant(LIMIT, LIMIT, false, |plan, out| {
        with_tile_slots(plan, Layout::Blocked, out, |slots, out| {
            let program = SourceByteProgram::derive(plan, slots, out)?;
            let mut paired =
                PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
            let hints = paired.roots[0].step_hints.as_mut().unwrap();
            let index = hints
                .cuts
                .iter()
                .position(|hint| hint.scalar_return_v325.is_some())
                .unwrap();
            let mut hint = hints.cuts.remove(index);
            let block = paired.roots[0]
                .cuts
                .iter()
                .position(|cut| cut.as_ref().is_some_and(|cut| cut.source == hint.pc))
                .unwrap();
            let allocation = root_returns::scan(&paired, 0, out)?;
            let routes = scan(&paired, 0, out)?;
            assert!(derive(&paired, &allocation, &routes, 0, block, &hint, out)?.is_some());
            let saved = hint.scalar_return_v325.as_ref().unwrap().returned.bits;
            hint.scalar_return_v325.as_mut().unwrap().returned.bits = saved * 2;
            assert!(derive(&paired, &allocation, &routes, 0, block, &hint, out)?.is_none());
            Ok(())
        })
    })
    .0
    .unwrap();
}
