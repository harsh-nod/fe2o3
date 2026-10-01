use super::super::super::invocations;
use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;

const LIMIT: usize = 100_000_000;

#[test]
fn original_mir_actual_all_step_emission_consumes_each_checked_edge_and_dynamic_frame() {
    run_control_case(LIMIT, LIMIT, false, true, |actual, out| {
        actual.emit_refinement(out)?;
        for (index, body) in actual.original.bodies.iter().enumerate() {
            let Some(body) = body else { continue };
            let bindings = actual.bodies[index].as_ref().unwrap();
            for (block, source) in body.control.blocks.iter().enumerate() {
                let Some(source) = source else { continue };
                let key = body.blocks.start + block;
                let edges = match &source.branch {
                    super::super::super::control::Branch::Switch { cases, .. } => cases.len() + 1,
                    _ => 1,
                };
                for ordinal in 0..edges {
                    let name = format!("open spec fn actual_invocation_segment_{key}_{ordinal}_v36(");
                    assert_eq!(out.text.matches(&name).count(), 1);
                    let path = format!("open spec fn actual_invocation_path_{key}_{ordinal}_v36() -> Seq<int> {{ seq![{}int,", bindings.bindings[block].as_ref().unwrap().physical.block);
                    assert!(out.text.contains(&path));
                }
            }
        }
        for root in 0..2 {
            for stem in ["invocation_initial_relation", "invocation_step_relation", "invocation_all_steps", "invocation_finite_trace", "invocation_initial_concrete_trace"] {
                assert!(out.text.contains(&format!("proof fn {stem}_{root}_v36(")));
            }
        }
        assert!(out.text.contains("byte_enter_frame_v30(f, "));
        assert!(out.text.contains("byte_pop_frame_v30(f)"));
        assert!(out.text.contains("byte_root_frame_v30("));
        assert!(out.text.contains("byte_frame_runtime_well_formed_v30(n.frames)"));
        assert!(!out.text.contains("byte_end_frame_v30("));
        assert!(!out.text.contains("assume("));
        assert!(!out.text.contains("admit("));
        Ok(())
    }).0.unwrap();
}

#[test]
fn original_mir_actual_all_step_emission_rejects_changed_body_before_any_text() {
    run_control_case(LIMIT, LIMIT, false, true, |actual, out| {
        let target = actual.roots[0]
            .targets
            .iter_mut()
            .find(|target| {
                target.program.nodes.iter().any(|node| {
                    matches!(
                        node.expression,
                        super::super::super::ExpressionV30::Binary { .. }
                    )
                })
            })
            .unwrap();
        let node = target
            .program
            .nodes
            .iter_mut()
            .find(|node| {
                matches!(
                    node.expression,
                    super::super::super::ExpressionV30::Binary { .. }
                )
            })
            .unwrap();
        if let super::super::super::ExpressionV30::Binary { operation, .. } = &mut node.expression {
            *operation = super::super::super::OperatorV30::And;
        }
        assert!(actual.emit_refinement(out).is_err());
        assert!(out.text.is_empty());
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_actual_all_step_generator_has_exact_and_one_short_resources() {
    let probe = |work, storage| {
        run_control_case(work, storage, false, true, |actual, out| {
            actual.emit_refinement(out)
        })
    };
    let measured = probe(LIMIT, LIMIT);
    measured.0.unwrap();
    probe(measured.1, measured.3).0.unwrap();
    assert!(probe(measured.1 - 1, measured.3).0.is_err());
    assert!(probe(measured.1, measured.3 - 1).0.is_err());
}

#[test]
fn original_mir_actual_all_step_generator_frame_has_independent_field_oracle() {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    let expected = 5 * h::<&()>()
        + h::<&super::super::super::target_trace::ConcreteTrace<'_, '_>>()
        + h::<Vec<usize>>()
        + h::<&[(usize, Option<usize>)]>()
        + h::<&[usize]>()
        + h::<Option<(usize, usize)>>()
        + h::<Option<usize>>()
        + 32 * size_of::<usize>()
        + 16 * size_of::<&()>();
    assert_eq!(generate::headers(), expected);
    assert_eq!(
        physical::headers(),
        7 * h::<&()>() + 32 * size_of::<usize>() + 16 * size_of::<&()>()
    );
}

#[test]
fn original_mir_invocation_request_consumes_the_complete_retained_source() {
    use crate::{
        PreparedOriginalSemanticMirRefinementV31, PreparedOriginalSemanticMirRefinementV36,
        prepare_original_semantic_mir_refinement_v36,
    };
    use std::any::TypeId;
    assert_ne!(
        TypeId::of::<PreparedOriginalSemanticMirRefinementV31<'static, 'static>>(),
        TypeId::of::<PreparedOriginalSemanticMirRefinementV36<'static, 'static>>()
    );
    invocations::tests::run_control_variant(LIMIT, LIMIT, false, true, |plan, out| {
        let source = plan.source(out)?;
        let floor = out.budget.storage();
        use fe2o3_kernel_ir::{EndiannessV2, ExplicitLaunchExtent, FormalIndexWidth};
        let launches = [ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [64, 1, 1],
        }; 2];
        let request = prepare_original_semantic_mir_refinement_v36(
            source,
            &launches,
            FormalIndexWidth::Bits64,
            EndiannessV2::Little,
            out.budget,
        )?;
        assert_eq!(out.budget.storage(), floor);
        let retained = request.retained_storage();
        out.budget.reserve_storage(retained)?;
        request.check_original_source(source, out.budget)?;
        request.check_runtime_v36(
            &launches,
            FormalIndexWidth::Bits64,
            EndiannessV2::Little,
            out.budget,
        )?;
        assert!(
            request
                .check_runtime_v36(
                    &launches,
                    FormalIndexWidth::Bits32,
                    EndiannessV2::Little,
                    out.budget
                )
                .is_err()
        );
        assert!(
            request
                .check_runtime_v36(
                    &launches,
                    FormalIndexWidth::Bits64,
                    EndiannessV2::Big,
                    out.budget
                )
                .is_err()
        );
        let mut changed = launches;
        changed[1] = ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [65, 1, 1],
        };
        assert!(
            request
                .check_runtime_v36(
                    &changed,
                    FormalIndexWidth::Bits64,
                    EndiannessV2::Little,
                    out.budget
                )
                .is_err()
        );
        assert_eq!(&request.subject(out.budget)?.census()[..4], &[2, 8, 28, 28]);
        let text = std::str::from_utf8(request.generated_source(out.budget)?).unwrap();
        assert!(text.starts_with("use vstd::prelude::*;"));
        assert!(text.contains("struct MemoryFrameRuntimeV30"));
        assert!(text.contains("struct InvocationByteMapV36"));
        assert!(text.contains("open spec fn invocation_activation_witness_v36("));
        assert!(text.contains("open spec fn invocation_source_read_enabled_v36("));
        assert!(text.contains("open spec fn invocation_frame_end_witness_v36("));
        assert!(text.contains("open spec fn invocation_source_slot_count_v36("));
        assert!(text.contains("open spec fn invocation_source_byte_initial_0_v36("));
        assert!(text.contains("open spec fn invocation_source_micro_step_0_0_v36("));
        assert!(text.contains("open spec fn invocation_source_byte_storage_related_0_v36("));
        assert!(text.contains("open spec fn invocation_runtime_index_bytes_v36() -> int { 8 }"));
        assert!(
            text.contains("open spec fn invocation_runtime_little_endian_v36() -> bool { true }")
        );
        assert!(text.contains("open spec fn invocation_actual_micro_runtime_0_v36("));
        for root in 0..2 {
            assert!(text.contains(&format!(
                "proof fn invocation_paired_finite_trace_{root}_v36("
            )));
            assert!(text.contains(&format!(
                "let head = byte_block_step_{root}_v30(target, invocation_runtime_little_endian_v36());"
            )));
            assert!(text.contains(&format!("proof fn invocation_paired_initial_{root}_v36(")));
            assert!(text.contains(&format!(
                "proof fn invocation_paired_initial_trace_{root}_v36("
            )));
            assert!(text.contains(&format!(
                "&& invocation_source_byte_storage_related_{root}_v36(source, target)"
            )));
        }
        assert!(text.contains("invocation_source_operands_effects_v36(result.operands"));
        assert!(!text.contains("struct ActualInvocationRuntimeV36"));
        assert!(!text.contains("assume("));
        assert!(!text.contains("external_body"));
        assert!(!request.authenticates_executed_proof());
        assert!(!request.grants_artifact_or_launch_authority());
        drop(request);
        out.budget.release_storage(retained)?;
        assert_eq!(out.budget.storage(), floor);
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_actual_microstep_observations_and_fuel_are_concretely_emitted() {
    run_control_case(LIMIT, LIMIT, false, true, |actual, out| {
        actual.emit_refinement(out)?;
        for (index, body) in actual.original.bodies.iter().enumerate() {
            let Some(body) = body else { continue };
            let binding = actual.bodies[index].as_ref().unwrap();
            for (block, source) in body.control.blocks.iter().enumerate() {
                let Some(source) = source else { continue };
                let key = body.blocks.start + block;
                let count = match &source.branch {
                    super::super::super::control::Branch::Switch { cases, .. } => cases.len() + 1,
                    _ => 1,
                };
                for ordinal in 0..count {
                    assert_eq!(
                        out.text
                            .matches(&format!(
                                "proof fn invocation_segment_concrete_{key}_{ordinal}_v36("
                            ))
                            .count(),
                        1
                    );
                    assert!(out.text.contains(&format!(
                        "actual_invocation_path_{key}_{ordinal}_v36().len()"
                    )));
                }
                for definition in binding.bindings[block]
                    .as_ref()
                    .unwrap()
                    .assignments
                    .iter()
                    .flatten()
                {
                    assert!(out.text.contains(&format!("body[{definition}],")));
                }
            }
        }
        assert!(out.text.contains("invocation_physical_trace_split_v36("));
        assert!(
            out.text
                .contains("invocation_runtime_projection_0_v36(o, fuel);")
        );
        assert!(
            out.text
                .contains("invocation_concrete_finite_trace_0_v36(o.scalar, fuel);")
        );
        for root in 0..2 {
            assert_eq!(
                out.text
                    .matches(&format!(
                        "open spec fn invocation_actual_raw_initial_{root}_v36("
                    ))
                    .count(),
                1
            );
            assert!(out.text.contains(&format!(
                "actual_invocation_ready_{root}_v36(invocation_actual_raw_initial_{root}_v36(a, p))"
            )));
            assert!(out.text.contains(&format!(
                "invocation_prefix_concrete_{root}_v36(raw);"
            )));
            assert!(out.text.contains(&format!(
                "raw, actual_invocation_prefix_{root}_v36().len(), invocation_physical_fuel_{root}_v36(o.scalar, fuel)"
            )));
        }
        assert!(!out.text.contains("op!"));
        Ok(())
    })
    .0
    .unwrap();
}

fn run(work: usize, storage: usize) -> (Result<()>, usize, usize, usize) {
    run_case(work, storage, false, |actual, out| {
        actual.check_segments(out)
    })
}

fn run_case(
    work: usize,
    storage: usize,
    unit: bool,
    examine: impl FnOnce(
        &mut ActualInvocations<'_, '_, '_, '_, '_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    run_control_case(work, storage, unit, false, examine)
}

fn run_control_case(
    work: usize,
    storage: usize,
    unit: bool,
    control: bool,
    examine: impl FnOnce(
        &mut ActualInvocations<'_, '_, '_, '_, '_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    run_root_case(work, storage, unit, control, 2, examine)
}

fn run_root_case(
    work: usize,
    storage: usize,
    unit: bool,
    control: bool,
    root_count: u8,
    examine: impl FnOnce(
        &mut ActualInvocations<'_, '_, '_, '_, '_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    invocations::tests::run_root_variant(work, storage, unit, control, root_count, |plan, out| {
        let source = plan.source(out)?;
        let owner = source.canonical(out.budget)?;
        let (inventory, receipt) = Inventory::derive_v18(owner, out.budget)?;
        out.budget.reserve_storage(receipt.retained_storage())?;
        let result =
            source.with_ranked_correspondence_v18(&inventory, out.budget, |relation, budget| {
                let result = (|| {
                    let mut out = Writer::new(budget)?;
                    let transfers = CallTransfers::derive(plan, &mut out)?;
                    let original = InvocationBodies::derive(&transfers, &mut out)?;
                    let mut actual = ActualInvocations::derive(&original, relation, &mut out)?;
                    actual.check(&out)?;
                    assert!(std::ptr::eq(actual.inventory, &inventory));
                    assert_eq!(actual.roots.len(), root_count as usize);
                    assert_eq!(actual.bodies.len(), original.bodies.len());
                    let mut active = 0;
                    let mut cuts = 0;
                    for (root, physical) in actual.roots.iter().enumerate() {
                        assert_eq!(physical.physical, plan.root(root, &out)?.physical);
                        for &(body, block) in physical.cuts.iter().flatten() {
                            let source_body = original.bodies[body].as_ref().unwrap();
                            let actual_body = actual.bodies[body].as_ref().unwrap();
                            assert_eq!(source_body.root, root);
                            let binding = actual_body.bindings[block].as_ref().unwrap();
                            assert_eq!(binding.physical.function.0 as usize, physical.physical);
                            assert_eq!(
                                physical.cuts[binding.physical.block as usize],
                                Some((body, block))
                            );
                            assert_eq!(
                                binding.live.len(),
                                source_body.control.blocks[block]
                                    .as_ref()
                                    .unwrap()
                                    .live
                                    .len()
                            );
                            cuts += 1;
                        }
                    }
                    for (body, binding) in original.bodies.iter().zip(&actual.bodies) {
                        assert_eq!(body.is_some(), binding.is_some());
                        if body.is_some() {
                            active += 1;
                        }
                    }
                    assert_eq!(
                        (active, cuts),
                        (
                            3 * root_count as usize,
                            (if control { 11 } else { 5 }) * root_count as usize
                        )
                    );
                    examine(&mut actual, &mut out)
                })();
                result.map_err(|error| match error {
                    Error::Resource(resource) => {
                        Error::Source(source.retain_query_resource_error_v18(resource))
                    }
                    other => other,
                })
            });
        drop(inventory);
        if result.is_ok() {
            out.budget.release_storage(receipt.retained_storage())?;
        }
        result
    })
}

#[test]
fn original_mir_actual_call_segments_keep_post_statement_arguments_all_returns_parallel_edges_and_loops()
 {
    run_control_case(LIMIT, LIMIT, false, true, |actual, out| {
        actual.check_segments(out)
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_actual_segment_capacity_has_independent_many_root_arithmetic() {
    use super::super::super::control::Branch;
    let mut paid = Vec::new();
    for root_count in [2, 4, 8] {
        run_root_case(LIMIT, LIMIT, false, true, root_count, |actual, out| {
            assert_eq!(actual.original.locals, 16 * root_count as usize);
            let mut counts = [0usize; 4];
            let mut total = 0;
            for range in &actual.original.roots {
                for index in range.clone() {
                    let Some(body) = &actual.original.bodies[index] else {
                        continue;
                    };
                    let binding = actual.bodies[index].as_ref().unwrap();
                    assert_eq!(body.control.locals, 4);
                    let suspended = if body.instance == 1 { 2 } else { 0 };
                    assert_eq!(binding.suspended.len(), suspended);
                    for source in body.control.blocks.iter().flatten() {
                        let (expected, category, edges) = match &source.branch {
                            Branch::Call { .. } => (4 + 2 * 4 + 3, 0, 1),
                            Branch::Return if body.returned.is_none() => (3, 1, 1),
                            Branch::Switch { cases, .. } => (
                                suspended + 4 + 3,
                                if suspended == 2 { 2 } else { 3 },
                                cases.len() + 1,
                            ),
                            Branch::Return => {
                                (suspended + 4 + 3, if suspended == 2 { 2 } else { 3 }, 1)
                            }
                            _ => panic!("unexpected many-root fixture branch"),
                        };
                        let before = (out.budget.work(), out.budget.storage());
                        assert_eq!(
                            segments::segment_extras(
                                body,
                                binding,
                                &source.branch,
                                actual.original,
                                out
                            )?,
                            expected
                        );
                        assert_eq!(
                            (out.budget.work(), out.budget.storage()),
                            (before.0 + 8, before.1)
                        );
                        counts[category] += edges;
                        total += expected * edges;
                    }
                }
            }
            let roots = root_count as usize;
            assert_eq!(counts, [2 * roots, roots, 7 * roots, 7 * roots]);
            // Two calls, root return, and seven edges from each of two helpers.
            assert_eq!(total, (2 * 15 + 3 + 7 * 9 + 7 * 7) * roots);
            let before = (out.budget.work(), out.budget.storage());
            actual.check_segments(out)?;
            paid.push((
                root_count as usize,
                out.budget.work() - before.0,
                out.budget.storage() - before.1,
            ));
            Ok(())
        })
        .0
        .unwrap();
    }
    // All retained segment backing and all traversal work repeat once per root.
    // Only the single enclosing header reservation is shared across roots.
    let (_, baseline_work, baseline_storage) = paid[0];
    for (roots, work, storage) in paid {
        assert_eq!(2 * work, roots * baseline_work);
        assert_eq!(
            2 * (storage - segments::headers()),
            roots * (baseline_storage - segments::headers())
        );
    }
}

#[test]
fn original_mir_actual_segment_many_root_scope_still_checks_the_last_body() {
    use super::super::super::{ExpressionV30, OperatorV30};
    let result = run_root_case(LIMIT, LIMIT, false, true, 8, |actual, out| {
        let last = actual.roots.last_mut().unwrap();
        let node = last
            .targets
            .iter_mut()
            .flat_map(|target| &mut target.program.nodes)
            .find(|node| {
                matches!(
                    node.expression,
                    ExpressionV30::Binary {
                        operation: OperatorV30::Xor,
                        ..
                    }
                )
            })
            .unwrap();
        let ExpressionV30::Binary { left, right, .. } = node.expression else {
            unreachable!()
        };
        node.expression = ExpressionV30::Binary {
            operation: OperatorV30::And,
            left,
            right,
        };
        actual.check_segments(out)
    });
    assert!(
        matches!(result.0, Err(Error::Statement(_))),
        "{:?}",
        result.0
    );
}

#[test]
fn original_mir_actual_call_segments_refuse_changed_parallel_edge_order_and_dead_call_result() {
    use super::super::super::canonical::control::TargetBranch;
    for fault in 0..2 {
        let result = run_control_case(LIMIT, LIMIT, false, true, |actual, out| {
            if fault == 0 {
                let branch = actual.roots[0]
                    .targets
                    .iter_mut()
                    .find_map(|target| match &mut target.branch {
                        TargetBranch::Switch { cases, .. } if cases.len() == 2 => Some(cases),
                        _ => None,
                    })
                    .unwrap();
                branch.swap(0, 1);
            } else {
                let child = actual.original.roots[0].start + 1;
                let body = actual.bodies[child].as_mut().unwrap();
                assert!(body.returned.flatten().is_some());
                body.returned = Some(body.inputs[0].1);
            }
            actual.check_segments(out)
        });
        assert!(
            matches!(result.0, Err(Error::Statement(_))),
            "fault {fault}: {:?}",
            result.0
        );
    }
}

#[test]
fn original_mir_actual_call_segments_check_repeated_callees_and_unit_returns() {
    for unit in [false, true] {
        run_case(LIMIT, LIMIT, unit, |actual, out| actual.check_segments(out))
            .0
            .unwrap();
    }
}

#[test]
fn original_mir_actual_call_segments_reject_changed_operators_cuts_and_preserved_frames() {
    use super::super::super::{ExpressionV30, OperatorV30, control::Branch};
    for fault in 0..7 {
        let result = run_case(LIMIT, LIMIT, false, |actual, out| {
            let root = 0;
            let index = actual.original.roots[root].start;
            let caller = actual.original.bodies[index].as_ref().unwrap();
            let site = caller
                .control
                .blocks
                .iter()
                .position(|row| {
                    row.as_ref()
                        .is_some_and(|row| matches!(row.branch, Branch::Call { .. }))
                })
                .unwrap();
            let Branch::Call {
                transfer,
                continuation,
            } = caller.control.blocks[site].as_ref().unwrap().branch
            else {
                unreachable!()
            };
            let call = actual.original.transfer(transfer, out)?;
            let child_index = index + call.child;
            let child = actual.original.bodies[child_index].as_ref().unwrap();
            let entry = child.control.entry.get() as usize;
            let call_cut = actual.bodies[index].as_ref().unwrap().bindings[site]
                .as_ref()
                .unwrap()
                .physical;
            let child_cut = actual.bodies[child_index].as_ref().unwrap().bindings[entry]
                .as_ref()
                .unwrap()
                .physical;
            let continuation_cut = actual.bodies[index].as_ref().unwrap().bindings
                [continuation.get() as usize]
                .as_ref()
                .unwrap()
                .physical;
            match fault {
                0 => {
                    let node = actual.roots[root].targets[child_cut.block as usize]
                        .program
                        .nodes
                        .iter_mut()
                        .find(|node| matches!(node.expression, ExpressionV30::Binary { .. }))
                        .unwrap();
                    let ExpressionV30::Binary { left, right, .. } = node.expression else {
                        unreachable!()
                    };
                    node.expression = ExpressionV30::Binary {
                        operation: OperatorV30::Or,
                        left,
                        right,
                    };
                }
                1 => {
                    actual.roots[root].targets[call_cut.block as usize].edges[0].target =
                        continuation_cut
                }
                2 => {
                    actual.roots[root].targets[child_cut.block as usize].edges[0].target = call_cut
                }
                3 => {
                    let child_actual = actual.bodies[child_index].as_mut().unwrap();
                    let binding = child_actual.bindings[entry].as_mut().unwrap();
                    binding.assignments[0] = binding.live[0].1;
                }
                4 => {
                    let carried = actual.bodies[child_index]
                        .as_ref()
                        .unwrap()
                        .suspended
                        .iter()
                        .find_map(|row| row.definition)
                        .unwrap();
                    let target = &mut actual.roots[root].targets[child_cut.block as usize];
                    let node = target
                        .program
                        .nodes
                        .iter()
                        .position(|node| matches!(node.expression, ExpressionV30::Binary { .. }))
                        .unwrap();
                    target.program.definitions[carried - target.program.definition_start] =
                        Some(node);
                }
                5 => actual.roots[root].cuts[child_cut.block as usize] = Some((index, site)),
                6 => {
                    actual.bodies[child_index].as_mut().unwrap().bindings[entry]
                        .as_mut()
                        .unwrap()
                        .assignments
                        .pop();
                }
                _ => unreachable!(),
            }
            actual.check_segments(out)
        });
        assert!(
            matches!(result.0, Err(Error::Statement(_))),
            "fault {fault}: {:?}",
            result.0
        );
    }
}

#[test]
fn original_mir_actual_call_segments_refuse_funded_foreign_ledger_before_work() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    run_case(LIMIT, LIMIT, false, |actual, out| {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(out.budget.storage())?;
        let mut foreign = Writer::new(&mut budget)?;
        let before = (
            foreign.budget.work(),
            foreign.budget.storage(),
            foreign.budget.peak_storage(),
        );
        assert!(matches!(
            actual.check_segments(&mut foreign),
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
        assert_eq!(
            (
                foreign.budget.work(),
                foreign.budget.storage(),
                foreign.budget.peak_storage()
            ),
            before
        );
        assert!(foreign.text.is_empty());
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_actual_invocation_census_keeps_repeated_and_inactive_source_bodies() {
    let first = run(LIMIT, LIMIT);
    let second = run(LIMIT, LIMIT);
    first.0.unwrap();
    second.0.unwrap();
    assert_eq!((first.1, first.2, first.3), (second.1, 37, second.3));
}

#[test]
fn original_mir_actual_invocation_census_keeps_exact_and_one_short_resources() {
    let measured = run(LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = run(measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (measured.1, 37, measured.3));
    assert!(matches!(run(measured.1 - 1, measured.3).0,
        Err(Error::Source(SourceError::Resource(Resource::Work(error))))
        if error.actual() == measured.1 && error.limit() == measured.1 - 1));
    assert!(matches!(run(measured.1, measured.3 - 1).0,
        Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
        if error.actual() == measured.3 && error.limit() == measured.3 - 1));
}

#[test]
fn original_mir_actual_call_segments_keep_exact_and_one_short_shared_return_resources() {
    let run = |work, storage| {
        run_control_case(work, storage, false, true, |actual, out| {
            actual.check_segments(out)
        })
    };
    let measured = run(LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = run(measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (measured.1, 37, measured.3));
    assert!(matches!(run(measured.1 - 1, measured.3).0,
        Err(Error::Source(SourceError::Resource(Resource::Work(error))))
        if error.actual() == measured.1 && error.limit() == measured.1 - 1));
    assert!(matches!(run(measured.1, measured.3 - 1).0,
        Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
        if error.actual() == measured.3 && error.limit() == measured.3 - 1));
}

#[test]
fn original_mir_actual_invocation_frames_have_independent_fields_and_result_envelopes() {
    use super::super::super::{
        NodeV30, ScalarV30, call_transfers::DirectTransfer,
        control::SourceBlock as InterpretedBlock, target_trace::ConcreteTrace,
    };
    use fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 as Block;
    use std::mem::align_of;
    type BodyFields = (
        Vec<Option<BlockBindings>>,
        Vec<Suspended>,
        Vec<(u32, Option<usize>)>,
        Option<Option<usize>>,
    );
    type RootFields = (
        usize,
        Vec<TargetBlock>,
        Vec<Option<(usize, usize)>>,
        Vec<bool>,
        usize,
    );
    type CarryFields = (usize, Option<usize>, ScalarV30);
    type Fields<'a> = (
        &'a InvocationBodies<'a, 'a, 'a, 'a>,
        &'a Inventory<'a>,
        Vec<ActualRoot>,
        Vec<Option<ActualBody>>,
        usize,
    );
    assert_eq!(
        (size_of::<ActualBody>(), align_of::<ActualBody>()),
        (size_of::<BodyFields>(), align_of::<BodyFields>())
    );
    assert_eq!(
        (size_of::<ActualRoot>(), align_of::<ActualRoot>()),
        (size_of::<RootFields>(), align_of::<RootFields>())
    );
    assert_eq!(
        (size_of::<Suspended>(), align_of::<Suspended>()),
        (size_of::<CarryFields>(), align_of::<CarryFields>())
    );
    assert_eq!(
        (
            size_of::<ActualInvocations<'_, '_, '_, '_, '_, '_>>(),
            align_of::<ActualInvocations<'_, '_, '_, '_, '_, '_>>()
        ),
        (size_of::<Fields<'_>>(), align_of::<Fields<'_>>())
    );
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    let census = h::<ActualInvocations<'_, '_, '_, '_, '_, '_>>()
        + h::<ActualBody>()
        + h::<ActualRoot>()
        + h::<BlockBindings>()
        + h::<Vec<ActualRoot>>()
        + h::<Vec<Option<ActualBody>>>()
        + h::<Vec<Option<BlockBindings>>>()
        + h::<Vec<Option<(usize, usize)>>>()
        + h::<Vec<(u32, Option<usize>)>>()
        + h::<Vec<Option<usize>>>()
        + h::<Vec<TargetBlock>>()
        + h::<Vec<bool>>()
        + h::<TargetBlock>()
        + h::<Vec<Suspended>>()
        + h::<Suspended>()
        + h::<Option<Option<usize>>>()
        + h::<&fe2o3_mir_model::SsaConstructionPlanV1>()
        + h::<&[fe2o3_mir_model::SsaArgumentV1]>()
        + h::<fe2o3_mir_model::SsaEdgeIdV1>()
        + h::<&Correspondence<'_>>()
        + h::<&Inventory<'_>>()
        + h::<&InvocationBodies<'_, '_, '_, '_>>()
        + h::<Option<Block>>()
        + h::<Option<usize>>()
        + h::<SourceBlock>()
        + h::<Range<usize>>()
        + h::<Option<(usize, usize)>>()
        + h::<(&Inventory<'_>, &Range<usize>, Option<usize>, ScalarV30)>()
        + 16 * size_of::<usize>();
    assert_eq!(headers(), census);
    let segment = h::<&ActualInvocations<'_, '_, '_, '_, '_, '_>>()
        + h::<&InvocationBodies<'_, '_, '_, '_>>()
        + h::<&Body>()
        + h::<&ActualBody>()
        + h::<&ActualRoot>()
        + h::<&InterpretedBlock>()
        + h::<&BlockBindings>()
        + h::<&TargetBlock>()
        + h::<&DirectTransfer>()
        + h::<ConcreteTrace<'_, '_>>()
        + h::<Vec<Suspended>>()
        + h::<Vec<NodeV30>>()
        + h::<Vec<Option<Option<usize>>>>()
        + h::<Vec<Option<usize>>>()
        + h::<Vec<bool>>()
        + h::<Vec<(usize, usize)>>()
        + h::<Vec<usize>>()
        + h::<Vec<(usize, Option<usize>)>>()
        + h::<Option<Vec<(usize, usize)>>>()
        + h::<Option<&DirectTransfer>>()
        + h::<[Vec<usize>; 2]>()
        + h::<(usize, usize, usize)>()
        + h::<(Block, Vec<usize>)>()
        + h::<&mut Writer<'_, '_>>()
        + h::<NodeV30>()
        + h::<Suspended>()
        + h::<Block>()
        + 32 * size_of::<usize>()
        + 16 * size_of::<&()>();
    assert_eq!(segments::headers(), segment);
}
