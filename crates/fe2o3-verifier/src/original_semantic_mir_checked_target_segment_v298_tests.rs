//! Source/target emission and custody tests. These are not Verus evidence.
use super::super::super::{
    expanded_generation::ExpandedGenerationV221, slots::tests::with_tile_slots,
};
use super::*;
use fe2o3_kernel_ir::{EndiannessV2, ExecutionTileLayoutV1 as Layout};
use fe2o3_mir_model::semantic_mir_v1::SemanticCheckedBinaryOpV1 as SourceOp;

const LIMIT: usize = 512 * 1024 * 1024;

#[path = "original_semantic_mir_checked_target_witness_v298_tests.rs"]
mod witness;

fn fixture(
    layout: Layout,
    operator: SourceOp,
    work: usize,
    storage: usize,
    examine: impl FnOnce(
        &InvocationPlan<'_, '_>,
        &SourceSlots<'_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    fixture_with_prefix(layout, operator, false, work, storage, examine)
}

fn fixture_with_prefix(
    layout: Layout,
    operator: SourceOp,
    retained_prefix: bool,
    work: usize,
    storage: usize,
    examine: impl FnOnce(
        &InvocationPlan<'_, '_>,
        &SourceSlots<'_, '_>,
        &mut Writer<'_, '_>,
    ) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    use fe2o3_mir_model::semantic_mir_v1::*;
    super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| {
            super::super::super::paired::aggregate_tests::checked_leaf_transform(
                types, functions, operator,
            );
            if retained_prefix {
                let old = functions.last_mut().unwrap();
                let mut blocks = old.blocks().to_vec();
                let mut statements = blocks[0].statements().to_vec();
                let SemanticStatementKindV1::Assign(assignment) = statements.last().unwrap().kind()
                else {
                    panic!("Checked assignment");
                };
                let SemanticRvalueKindV1::CheckedBinary(checked) = assignment.value().kind() else {
                    panic!("Checked rvalue");
                };
                let SemanticOperandV1::Copy(left) = checked.left() else {
                    panic!("copied left operand");
                };
                let prefix = SemanticStatementV1::new(
                    old.source(),
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        left.clone(),
                        SemanticRvalueV1::new(
                            left.ty(),
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::BitXor,
                                left: checked.left().clone(),
                                right: SemanticOperandV1::Constant(SemanticConstantV1::new(
                                    left.ty(),
                                    SemanticConstantValueV1::Scalar(
                                        SemanticScalarValueV1::new(0x8000_0000, 4).unwrap(),
                                    ),
                                )),
                            },
                        ),
                    )),
                );
                statements.insert(statements.len() - 1, prefix);
                blocks[0] = SemanticBasicBlockV1::new(
                    blocks[0].identity(),
                    old.source(),
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
                    old.source(),
                    old.abi().clone(),
                    old.locals().to_vec(),
                    old.entry(),
                    blocks,
                )
                .unwrap();
            }
        },
        |plan, out| with_tile_slots(plan, layout, out, |slots, out| examine(plan, slots, out)),
    )
}

fn generate(layout: Layout, work: usize, storage: usize) -> (Result<()>, usize, usize, usize) {
    fixture(layout, SourceOp::Add, work, storage, |plan, slots, out| {
        let model = ExpandedGenerationV221::derive(
            plan,
            slots,
            FormalIndexWidth::Bits64,
            EndiannessV2::Little,
            out,
        )?;
        model.emit_support(out)?;
        model.finish(out)?;
        let text = &out.text;
        let count = text
            .matches("proof fn checked_actual_source_target_step_")
            .count();
        assert!(
            count > 0,
            "genuine retained Checked operations must be selected"
        );
        assert_eq!(
            count,
            text.matches("proof fn checked_target_actual_step_").count()
        );
        assert_eq!(
            count,
            text.matches("proof fn checked_actual_source_prefix_target_step_")
                .count()
        );
        assert_eq!(
            count,
            text.matches("spec fn checked_actual_segment_inputs_")
                .count()
        );
        assert_eq!(
            count,
            text.matches("spec fn checked_actual_segment_results_")
                .count()
        );
        for tail in text
            .split("proof fn checked_actual_source_target_step_")
            .skip(1)
        {
            let declaration = tail.split("\nproof fn ").next().unwrap();
            let site = declaration.split_once('(').unwrap().0;
            let (root, remaining) = site.split_once('_').unwrap();
            let (instance, _) = remaining.split_once('_').unwrap();
            let (header, body) = declaration.split_once("\n{\n").unwrap();
            assert_eq!(
                header,
                format!(
                    "{site}(\n s: InvocationSourceMicroStateV36, t: MemoryMicroStateV30,\n left: int, right: int, little_endian: bool,\n)\n requires checked_actual_segment_inputs_{site}(s, t, left, right, little_endian),\n ensures checked_actual_segment_results_{site}(s, t,\n     invocation_source_micro_step_{root}_{instance}_v36(s, little_endian),\n     byte_micro_step_{root}_v30(t, little_endian), left, right),"
                )
            );
            let source_header =
                format!(" hide(invocation_source_micro_step_{root}_{instance}_v36);\n");
            let target_header = format!(" hide(byte_micro_step_{root}_v30);\n");
            let demand_site = site.strip_suffix("_v298").unwrap();
            let summary_headers = [
                " hide(invocation_source_byte_state_well_formed_v36);\n".to_owned(),
                format!(" hide(invocation_source_active_{root}_{instance}_v36);\n"),
                format!(" hide(byte_inputs_{root}_v55);\n"),
                format!(" hide(checked_prefix_demands_{demand_site}_v296);\n"),
                format!(" hide(checked_target_next_{site});\n"),
            ];
            assert!(body.starts_with(&format!(
                "{source_header}{target_header}{} reveal(checked_actual_segment_inputs_{site});\n",
                summary_headers.concat()
            )));
            assert_eq!(body.matches(" hide(").count(), 7);
            assert_eq!(body.matches(&source_header).count(), 1);
            assert_eq!(body.matches(&target_header).count(), 1);
            let mut previous_body = body.to_owned();
            for directive in &summary_headers {
                assert_eq!(body.matches(directive).count(), 1);
                previous_body = previous_body.replacen(directive, "", 1);
            }
            let target_declaration = text
                .split_once(&format!("proof fn checked_target_actual_step_{site}("))
                .unwrap()
                .1
                .split_once("\nspec fn ")
                .unwrap()
                .0;
            let mut updates = target_declaration.split(".update(").skip(1);
            let value: usize = updates
                .next()
                .unwrap()
                .split_once("int,")
                .unwrap()
                .0
                .parse()
                .unwrap();
            let overflow: usize = updates
                .next()
                .unwrap()
                .split_once("int,")
                .unwrap()
                .0
                .parse()
                .unwrap();
            assert!(updates.next().is_none());
            assert_eq!(
                previous_body,
                format!(
                    "{source_header}{target_header} reveal(checked_actual_segment_inputs_{site});\n checked_add_actual_micro_step_{demand_site}_v293(s, left, right, little_endian);\n checked_add_actual_demanded_step_{demand_site}_v296(s, left, right, little_endian);\n checked_target_actual_step_{site}(t, left, right, little_endian);\n let n = byte_micro_step_{root}_v30(t, little_endian);\n assert forall|i: int| 0 <= i < t.state.values.len() && i != {value} && i != {overflow}\n     implies #[trigger] n.next.state.values[i] == t.state.values[i] by {{ }}\n reveal(checked_actual_segment_results_{site});\n}}"
                )
            );
            let (requires, rest) = declaration.split_once("\n ensures").unwrap();
            assert!(requires.contains("requires checked_actual_segment_inputs_"));
            assert!(!requires.contains("results_"));
            assert!(!requires.contains("after_"));
            assert!(rest.contains("invocation_source_micro_step_"));
            assert!(rest.contains("byte_micro_step_"));
            assert!(rest.contains("checked_add_actual_demanded_step_"));
            assert!(rest.contains("checked_target_actual_step_"));
            assert!(!declaration.contains("assume("));
            assert!(!declaration.contains("admit("));
        }
        assert!(text.contains(".leaves[seq![0int]]"));
        assert!(text.contains(".leaves[seq![1int]]"));
        assert!(text.contains("MemoryValueV30::Scalar(if left + right >= 4294967296"));
        assert!(text.contains("partial projections, no initialization or complete-frame claim"));
        for tail in text
            .split("spec fn checked_actual_segment_results_")
            .skip(1)
        {
            let predicate = tail.split("\nproof fn ").next().unwrap();
            assert!(predicate.contains("checked_prefix_demands_"));
            assert!(predicate.contains("forall|i: int|"));
            assert!(!predicate.contains("expanded_frame_contract"));
            assert!(!predicate.contains("map_current"));
        }
        Ok(())
    })
}

#[test]
fn actual_checked_target_segments_bind_two_results_and_both_real_interpreters() {
    for layout in [Layout::Blocked, Layout::Striped] {
        generate(layout, LIMIT, LIMIT).0.unwrap();
    }
}

#[test]
fn actual_checked_target_segments_do_not_select_other_checked_operators() {
    for operation in [SourceOp::Subtract, SourceOp::Multiply] {
        fixture(
            Layout::Blocked,
            operation,
            LIMIT,
            LIMIT,
            |plan, slots, out| {
                let target = TileTargetV176::derive(slots, out)?;
                let program = SourceByteProgram::derive(plan, slots, out)?;
                assert_eq!(
                    program.emit_checked_target_segments_v298(
                        plan,
                        &target,
                        FormalIndexWidth::Bits64,
                        out
                    )?,
                    0
                );
                assert!(
                    !out.text
                        .contains("proof fn checked_actual_source_target_step_")
                );
                Ok(())
            },
        )
        .0
        .unwrap();
    }
}

#[test]
fn actual_checked_target_segment_classifiers_refuse_unretained_and_noncontiguous_spans() {
    use fe2o3_kernel_ir::{
        CanonicalKirBlockCoordinateV1 as Block, CanonicalKirFunctionCoordinateV1 as Function,
    };
    let original = Operation {
        block: Block {
            function: Function(7),
            block: 11,
        },
        operation: 23,
    };
    let output = Operation {
        block: Block {
            function: Function(13),
            block: 17,
        },
        operation: 29,
    };
    // Copied descriptor tests do not fabricate an owning source/target relation.
    // Production queries obtain these descriptors from the retained owners.
    for (row, expected) in [
        (
            NeutralOperation::RemovedUnreachable { input: original },
            Unsupported::Removed,
        ),
        (
            NeutralOperation::Rewritten { input: original },
            Unsupported::Rewritten,
        ),
    ] {
        assert_eq!(
            neutral_selection(original, row).unwrap(),
            NeutralSelection::Unsupported(expected)
        );
        assert!(matches!(
            neutral_selection(output, row),
            Err(Error::Statement(_))
        ));
    }
    assert_eq!(
        neutral_selection(
            original,
            NeutralOperation::Retained {
                input: original,
                output
            }
        )
        .unwrap(),
        NeutralSelection::Retained(output)
    );
    assert!(matches!(
        neutral_selection(
            original,
            NeutralOperation::Retained {
                input: output,
                output
            }
        ),
        Err(Error::Statement(_))
    ));
    let span = fe2o3_lower_mir_kernel::ProductionSourceTileOperationSpanV159 {
        original,
        expansion: fe2o3_kernel_opt::TileScalarOperationProjectionV159 {
            input: output,
            first: 71,
            end: 72,
        },
    };
    assert_eq!(
        singleton_span(original, output, span).unwrap(),
        Some(Operation {
            block: output.block,
            operation: 71
        })
    );
    for end in [71, 73, u32::MAX] {
        let mut changed = span;
        changed.expansion.end = end;
        assert_eq!(singleton_span(original, output, changed).unwrap(), None);
    }
    let mut changed = span;
    changed.expansion.end = 70;
    assert!(matches!(
        singleton_span(original, output, changed),
        Err(Error::Statement(_))
    ));
    assert!(matches!(
        singleton_span(output, output, span),
        Err(Error::Statement(_))
    ));
    assert!(matches!(
        singleton_span(original, original, span),
        Err(Error::Statement(_))
    ));
}

#[test]
fn actual_checked_target_segment_coordinates_and_foreign_owners_refuse_before_emission() {
    for mutation in 0..10 {
        fixture(Layout::Blocked, SourceOp::Add, LIMIT, LIMIT, |plan, slots, out| {
            let target = TileTargetV176::derive(slots, out)?;
            let foreign_target = TileTargetV176::derive(slots, out)?;
            let program = SourceByteProgram::derive(plan, slots, out)?;
            let foreign_program = SourceByteProgram::derive(plan, slots, out)?;
            let frames = FramePlan::derive(plan, slots, out)?;
            let scalar = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
            let mut seen = 0;
            for root in 0..program.roots.len() {
                let range = &program.roots[root].0;
                for instance in 0..range.len() {
                    let Some(function) = &program.functions[range.start + instance] else { continue; };
                    for block in 0..function.control.len() {
                        for statement in 0..function.control[block].statements {
                            if function.body.event_at(block, statement, out)?.checked_prefix_site_v296().is_none() { continue; }
                            let mut before = frames.partial_prefix_v296(root, instance, block, statement, out)?;
                            let mut after = frames.partial_prefix_v296(root, instance, block, statement + 1, out)?;
                            let selected = CheckedTargetSegment::derive(&program, &target, plan, &frames, &scalar,
                                &before, &after, FormalIndexWidth::Bits64, out)?;
                            if let Selection::Ready(segment) = selected {
                                let inventory = target.inventory(out)?;
                                let operation = &inventory.operations()[segment.target_operation];
                                assert_eq!(operation.coordinate.block.function, target.root_function(root, out)?);
                                assert_eq!(block_index(inventory, operation.coordinate.block)?, segment.target_block);
                                assert_eq!(operation.coordinate.operation as usize, segment.target_prefix);
                                assert_eq!(segment.target_operation,
                                    inventory.blocks()[segment.target_block].operations.start + segment.target_prefix);
                                let next = segment.target_operation + 1;
                                assert_eq!(segment.target_next,
                                    (next < inventory.blocks()[segment.target_block].operations.end).then_some(next));
                                let length = out.text.len();
                                if (5..7).contains(&mutation) {
                                    let result = if mutation == 5 {
                                        segment.check(&foreign_program, &target, out)
                                    } else { segment.check(&program, &foreign_target, out) };
                                    assert!(matches!(result, Err(Error::Statement(_))));
                                } else {
                                    match mutation {
                                        0 => after.frame = usize::MAX,
                                        1 => after.block = usize::MAX,
                                        2 => after.statement = before.statement,
                                        3 => after.pc = usize::MAX,
                                        4 => after.locals[destination_local(&segment, &frames, before.frame)].current = None,
                                        7 => {
                                            let base = frames.frames[before.frame].locals.start;
                                            before.locals[segment.source_operands[0] - base].current = before.locals[segment.source_operands[1] - base].current;
                                        }
                                        8 => after.locals[destination_local(&segment, &frames, before.frame)].ty = fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1::from_index(u32::MAX),
                                    _ => {
                                        // A scalar operand cannot authenticate the
                                        // aggregate destination's post-statement SSA.
                                        let base = frames.frames[before.frame].locals.start;
                                            after.locals[destination_local(&segment, &frames, before.frame)].current = before.locals[segment.source_operands[0] - base].current;
                                        }
                                    }
                                    let result = CheckedTargetSegment::derive(&program, &target, plan, &frames, &scalar,
                                        &before, &after, FormalIndexWidth::Bits64, out);
                                    if mutation == 4 {
                                        assert!(matches!(result, Ok(Selection::Unsupported(Unsupported::UnavailableSsa))));
                                    } else { assert!(matches!(result, Err(Error::Statement(_)))); }
                                }
                                assert_eq!(out.text.len(), length);
                                seen += 1;
                            }
                            after.discard(out)?;
                            before.discard(out)?;
                        }
                    }
                }
            }
            assert!(seen > 0);
            Ok(())
        }).0.unwrap();
    }
}

fn inspect_segments(
    plan: &InvocationPlan<'_, '_>,
    slots: &SourceSlots<'_, '_>,
    out: &mut Writer<'_, '_>,
    mut inspect: impl FnMut(&CheckedTargetSegment<'_, '_, '_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> Result<usize> {
    let target = TileTargetV176::derive(slots, out)?;
    let program = SourceByteProgram::derive(plan, slots, out)?;
    let frames = FramePlan::derive(plan, slots, out)?;
    let scalar = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
    let mut count = 0;
    for root in 0..program.roots.len() {
        let range = &program.roots[root].0;
        for instance in 0..range.len() {
            let Some(function) = &program.functions[range.start + instance] else {
                continue;
            };
            for block in 0..function.control.len() {
                for statement in 0..function.control[block].statements {
                    if function
                        .body
                        .event_at(block, statement, out)?
                        .checked_prefix_site_v296()
                        .is_none()
                    {
                        continue;
                    }
                    let before =
                        frames.partial_prefix_v296(root, instance, block, statement, out)?;
                    let after =
                        frames.partial_prefix_v296(root, instance, block, statement + 1, out)?;
                    if let Selection::Ready(segment) = CheckedTargetSegment::derive(
                        &program,
                        &target,
                        plan,
                        &frames,
                        &scalar,
                        &before,
                        &after,
                        FormalIndexWidth::Bits64,
                        out,
                    )? {
                        inspect(&segment, out)?;
                        count += 1;
                    }
                    after.discard(out)?;
                    before.discard(out)?;
                }
            }
        }
    }
    Ok(count)
}

#[test]
fn actual_checked_target_segment_uses_global_target_cursors_and_relative_history() {
    let (mut nonzero_function, mut nonzero_block, mut nonzero_prefix, mut terminal) =
        (false, false, false, false);
    let (mut source_nonzero_prefix, mut source_nonzero_offset) = (false, false);
    for retained_prefix in [false, true] {
        fixture_with_prefix(
            Layout::Striped,
            SourceOp::Add,
            retained_prefix,
            LIMIT,
            LIMIT,
            |plan, slots, out| {
                assert!(
                    inspect_segments(plan, slots, out, |segment, out| {
                        let inventory = segment.target.inventory(out)?;
                        let operation = &inventory.operations()[segment.target_operation];
                        if retained_prefix {
                            use fe2o3_kernel_ir::{BinaryOp, OperationKind};
                            assert!(
                                segment.target_prefix > 0,
                                "XOR must precede the actual Checked operation"
                            );
                            let block = &inventory.blocks()[segment.target_block];
                            let prefix = inventory.operations()
                                [block.operations.start..segment.target_operation]
                                .iter()
                                .find(|row| {
                                    matches!(
                                        row.operation.kind,
                                        OperationKind::Binary {
                                            op: BinaryOp::BitXor,
                                            ..
                                        }
                                    ) && row.results.contains(&segment.checked.operands[0])
                                })
                                .expect("retained XOR result must feed Checked operand zero");
                            assert_eq!(prefix.results.len(), 1);
                            assert_eq!(prefix.results.start, segment.checked.operands[0]);
                            assert_eq!(prefix.coordinate.block, operation.coordinate.block);
                            assert_eq!(prefix.operands.len(), 2);
                            let OperationKind::Binary { lhs, rhs, .. } = prefix.operation.kind
                            else {
                                unreachable!()
                            };
                            assert_ne!(
                                lhs, rhs,
                                "nonzero constant XOR must not be self-cancelling"
                            );
                            for (ordinal, value) in [lhs, rhs].into_iter().enumerate() {
                                let usage = &inventory.uses()[prefix.operands.start + ordinal];
                                assert_eq!(usage.value, value);
                                assert_eq!(
                                    inventory.definitions()[usage.definition].value,
                                    Some(value)
                                );
                                assert_eq!(
                                    inventory.definitions()[usage.definition].ty,
                                    &fe2o3_kernel_ir::Type::Scalar(
                                        fe2o3_kernel_ir::ScalarType::U32
                                    )
                                );
                            }
                        } else {
                            assert_eq!(
                                segment.target_prefix, 0,
                                "original copied-local fixture has no retained prefix operation"
                            );
                        }
                        nonzero_function |= operation.coordinate.block.function.0 != 0;
                        nonzero_block |= segment.target_block != 0;
                        nonzero_prefix |= segment.target_prefix != 0;
                        source_nonzero_prefix |= segment.statement != 0;
                        source_nonzero_offset |= segment.source_pc != segment.block;
                        terminal |= segment.target_next.is_none();
                        let start = out.text.len();
                        segment.emit(out)?;
                        let text = &out.text[start..];
                        assert!(text.contains(&format!(
                            "t.state.pc == {}, t.next_operation == {}",
                            segment.target_block, segment.target_operation
                        )));
                        assert!(text.contains(&format!(
                            "t.observations.len() == {}",
                            segment.target_prefix
                        )));
                        assert!(text.contains(&format!(
                            "s.source.machine.pc == {} && s.next_statement == {}",
                            segment.source_pc, segment.statement
                        )));
                        if segment.target_next.is_none() {
                            assert!(text.contains("_v298() -> int { -1 }"));
                        }
                        Ok(())
                    })? > 0
                );
                Ok(())
            },
        )
        .0
        .unwrap();
    }
    assert!(
        nonzero_function,
        "target function coordinate must be nonzero"
    );
    assert!(nonzero_block, "target block coordinate must be nonzero");
    assert!(
        nonzero_prefix,
        "genuine retained target prefix must be nonempty"
    );
    assert!(terminal, "terminal Checked must retain the -1 next cursor");
    assert!(
        source_nonzero_prefix,
        "source statement ordinal must be nonzero"
    );
    assert!(
        source_nonzero_offset,
        "absolute source PC must differ from relative block"
    );
}

#[test]
fn actual_checked_target_segment_foreign_and_refunded_accounts_are_not_unsupported() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    for foreign in [false, true] {
        let mut reached = false;
        let result = fixture(
            Layout::Blocked,
            SourceOp::Add,
            LIMIT,
            LIMIT,
            |plan, slots, out| {
                inspect_segments(plan, slots, out, |segment, out| {
                    reached = true;
                    let error = if foreign {
                        let mut work = Work::new(LIMIT);
                        let mut budget = Budget::new(&mut work, LIMIT);
                        budget.reserve_storage(segment.required)?;
                        let mut other = Writer::new(&mut budget)?;
                        segment
                            .check(segment.program, segment.target, &mut other)
                            .unwrap_err()
                    } else {
                        out.budget.release_storage(1)?;
                        segment
                            .check(segment.program, segment.target, out)
                            .unwrap_err()
                    };
                    assert!(matches!(
                        error,
                        Error::Resource(Resource::Accounting)
                            | Error::Source(SourceError::Resource(Resource::Accounting))
                    ));
                    assert!(segment.check(segment.program, segment.target, out).is_err());
                    assert!(
                        segment
                            .program
                            .emit_checked_target_segments_v298(
                                plan,
                                segment.target,
                                FormalIndexWidth::Bits64,
                                out
                            )
                            .is_err()
                    );
                    Err(error)
                })
                .map(|_| ())
            },
        );
        assert!(reached && result.0.is_err());
    }
}

#[test]
fn actual_checked_target_segment_copied_operand_alias_is_retained() {
    use fe2o3_mir_model::semantic_mir_v1::*;
    super::super::super::super::invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        |types, functions| {
            super::super::super::paired::aggregate_tests::checked_leaf_transform(
                types,
                functions,
                SourceOp::Add,
            );
            let old = functions.last_mut().unwrap();
            let mut blocks = old.blocks().to_vec();
            let mut statements = blocks[0].statements().to_vec();
            let last = statements.last_mut().unwrap();
            let SemanticStatementKindV1::Assign(assignment) = last.kind() else {
                panic!("Checked assignment")
            };
            let SemanticRvalueKindV1::CheckedBinary(checked) = assignment.value().kind() else {
                panic!("Checked rvalue")
            };
            *last = SemanticStatementV1::new(
                old.source(),
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    assignment.destination().clone(),
                    SemanticRvalueV1::new(
                        assignment.value().result_type(),
                        SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                            SourceOp::Add,
                            checked.left().clone(),
                            checked.left().clone(),
                        )),
                    ),
                )),
            );
            blocks[0] = SemanticBasicBlockV1::new(
                blocks[0].identity(),
                old.source(),
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
                old.source(),
                old.abi().clone(),
                old.locals().to_vec(),
                old.entry(),
                blocks,
            )
            .unwrap();
        },
        |plan, out| {
            with_tile_slots(plan, Layout::Blocked, out, |slots, out| {
                assert!(
                    inspect_segments(plan, slots, out, |segment, out| {
                        assert_eq!(segment.source_operands[0], segment.source_operands[1]);
                        // This typed source form stores an aggregate into a different
                        // local from its U32 operands. Numeric schema aliases do not
                        // fabricate an ill-typed source-local destination alias.
                        assert_ne!(segment.destination, segment.source_operands[0]);
                        assert_eq!(segment.checked.operands[0], segment.checked.operands[1]);
                        assert_ne!(segment.checked.results[0], segment.checked.results[1]);
                        segment.emit(out)
                    })? > 0
                );
                Ok(())
            })
        },
    )
    .0
    .unwrap();
}

fn destination_local(
    segment: &CheckedTargetSegment<'_, '_, '_, '_>,
    frames: &FramePlan<'_, '_, '_, '_>,
    frame: usize,
) -> usize {
    segment.destination - frames.frames[frame].locals.start
}

#[test]
fn actual_checked_target_segments_have_exact_work_storage_and_finish_boundaries() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let baseline = generate(layout, LIMIT, LIMIT);
        baseline.0.unwrap();
        let exact = generate(layout, baseline.1, baseline.3);
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (baseline.1, baseline.2, baseline.3)
        );
        use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
        assert!(matches!(generate(layout, baseline.1 - 1, baseline.3).0,
            Err(Error::Resource(Resource::Work(error))) | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
            if error.actual() == baseline.1 && error.limit() == baseline.1 - 1));
        assert!(matches!(generate(layout, baseline.1, baseline.3 - 1).0,
            Err(Error::Resource(Resource::Storage(error))) | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
            if error.actual() == baseline.3 && error.limit() == baseline.3 - 1));
    }
}

#[test]
fn actual_checked_target_segment_headers_cover_both_owners_and_query_results() {
    let owners = 2 * size_of::<CheckedTargetSegment<'_, '_, '_, '_>>()
        + 2 * size_of::<Selection<'_, '_, '_, '_>>()
        + 2 * size_of::<Result<Selection<'_, '_, '_, '_>>>()
        + size_of::<FramePlan<'_, '_, '_, '_>>()
        + size_of::<Result<FramePlan<'_, '_, '_, '_>>>()
        + 2 * size_of::<Prefix<'_, '_, '_, '_, '_>>()
        + 2 * size_of::<Result<Prefix<'_, '_, '_, '_, '_>>>()
        + size_of::<ExpandedScalarBindingsV196<'_, '_, '_, '_>>()
        + size_of::<Result<ExpandedScalarBindingsV196<'_, '_, '_, '_>>>();
    let endpoints = 4 * size_of::<CheckedByteOperationV48>()
        + 4 * size_of::<Result<CheckedByteOperationV48>>()
        + 4 * size_of::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + 4 * size_of::<Result<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>>()
        + 4 * size_of::<
            std::result::Result<
                fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>,
                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18,
            >,
        >()
        + 4 * size_of::<
            std::result::Result<
                Option<usize>,
                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18,
            >,
        >()
        + 2 * size_of::<
            std::result::Result<
                Option<fe2o3_lower_mir_kernel::ProductionSourceTileOperationSpanV159>,
                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18,
            >,
        >()
        + 2 * size_of::<
            std::result::Result<(), fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18>,
        >()
        + 2 * size_of::<SourceOperation>()
        + 2 * size_of::<Option<Operation>>()
        + 2 * size_of::<NeutralOperation>()
        + 2 * size_of::<NeutralSelection>()
        + 2 * size_of::<Result<NeutralSelection>>()
        + 2 * size_of::<Result<Option<Operation>>>()
        + 2 * size_of::<fe2o3_lower_mir_kernel::ProductionSourceTileOperationSpanV159>()
        + 2 * size_of::<Definition>()
        + 2 * size_of::<Carrier>();
    let scanner = 4 * size_of::<Event>()
        + 4 * size_of::<Result<Event>>()
        + 4 * size_of::<Range<usize>>()
        + 4 * size_of::<Option<usize>>()
        + 2 * size_of::<Option<(usize, usize, usize)>>()
        + 4 * size_of::<Result<()>>()
        + 2 * size_of::<std::ops::Range<usize>>()
        + 2 * size_of::<std::array::IntoIter<usize, 4>>()
        + 2 * size_of::<
            std::iter::Enumerate<std::array::IntoIter<(fe2o3_mir_model::SsaValueV1, usize), 2>>,
        >()
        + 64 * size_of::<usize>()
        + 32 * size_of::<&()>();
    assert_eq!(headers(), owners + endpoints + scanner);
}
