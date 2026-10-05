use super::*;
use fe2o3_kernel_ir::{EndiannessV2, ExplicitLaunchExtent, FormalIndexWidth};
use std::fmt::Write as _;

const WRITE_EQUATIONS: &str = include_str!("original_semantic_mir_thread_write_v88_tests.vrs");

#[derive(Clone, Copy)]
enum WriteProofShape {
    MovedValue,
    NonzeroStatements,
}

fn write_fixture(
    types: &mut Vec<Type>,
    functions: &mut Vec<Function>,
    callables: &mut Vec<SemanticCallableDeclV1>,
    disjoint: bool,
    copied_index: bool,
    shape: Option<WriteProofShape>,
) {
    assert!(!disjoint || !copied_index);
    indexed_fixture_kind(types, functions, callables, disjoint, None);
    // The transformed call no longer returns Option<&mut T>; replace that
    // otherwise unreachable type instead of leaving it outside the root closure.
    let boolean = functions[0].locals()[11].ty();
    assert!(matches!(
        types[boolean.index() as usize].shape(),
        Shape::Enum { .. }
    ));
    // Keep the inert slot's identity ordered before the optional owned witness.
    let boolean_identity = types[boolean.index() as usize].identity();
    types[boolean.index() as usize] = Type::new(
        boolean_identity,
        SemanticLayoutIdentityV1::from_sha256([219; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            BackendRepr::scalar(BackendScalar::initialized(
                BackendPrimitive::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            )),
            false,
        )
        .unwrap(),
        Shape::Scalar(SemanticScalarTypeV1::Bool),
    );
    assert!(
        types
            .windows(2)
            .all(|pair| pair[0].identity() < pair[1].identity())
    );
    let mut writes = 0;
    for callable in callables.iter_mut() {
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding, operation, ..
        } = callable
        else {
            continue;
        };
        if let SemanticCompilerIntrinsicOperationV1::DisjointSliceLen {
            disjoint_slice,
            element,
            raw_index,
            index_space,
        } = *operation
        {
            *operation = SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceLen {
                disjoint_slice,
                element,
                raw_index,
                index_space,
            };
        }
        let (descriptor, witness, element, raw) = match *operation {
            SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
                disjoint_slice,
                index_witness,
                element,
                raw_index,
            }
            | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut {
                disjoint_slice,
                index_witness,
                element,
                raw_index,
                ..
            } => (disjoint_slice, index_witness, element, raw_index),
            _ => continue,
        };
        let plain = SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap();
        let old = binding.abi();
        let mut inputs: Vec<_> = old
            .arguments()
            .iter()
            .map(|argument| argument.value().clone())
            .collect();
        inputs.push(SemanticAbiValueV1::new(
            element,
            SemanticAbiPassModeV1::Direct(plain),
        ));
        let mut ownership = old.source_argument_ownership().to_vec();
        ownership.push(SemanticSourceArgumentOwnershipV1::ByValue);
        let result_attributes = SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::ZeroExtend,
            0,
            None,
        )
        .unwrap();
        let abi = SemanticFunctionAbiV1::new(
            old.identity(),
            old.layout_identity(),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            inputs,
            SemanticAbiValueV1::new(boolean, SemanticAbiPassModeV1::Direct(result_attributes)),
        )
        .unwrap()
        .with_source_argument_ownership(ownership)
        .unwrap();
        *binding = SemanticNonBodyCallableBindingV1::new(
            binding.identity(),
            SemanticItemDefinitionIdentityV1::from_sha256([220; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([220; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([220; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([220; 32]),
            functions[0].source(),
            abi,
        );
        *operation = SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
            disjoint_slice: descriptor,
            witness,
            element,
            raw_index: raw,
            index_space: SemanticDisjointIndexSpaceV1::Index1d,
            kind: SemanticWriteOnlyDisjointWriteKindV1::Thread { disjoint },
        };
        writes += 1;
    }
    assert_eq!(writes, 1);
    for function in &mut functions[..2] {
        let old = function.clone();
        let mut locals = old.locals().to_vec();
        let local = &locals[11];
        locals[11] =
            SemanticLocalDeclV1::new(local.identity(), boolean, local.role(), local.source());
        let mut blocks = old.blocks().to_vec();
        let write_at = blocks.len() - 3;
        let Terminator::SwitchInt { targets, .. } = blocks[write_at + 1].terminator().kind() else {
            panic!("original Option continuation")
        };
        let resume = targets.otherwise().target();
        let Terminator::Call(call) = blocks[write_at].terminator().kind() else {
            panic!("original descriptor call")
        };
        let mut arguments = call.arguments().to_vec();
        if copied_index {
            let Operand::Move(index) = &arguments[1] else {
                panic!("owned index")
            };
            arguments[1] = Operand::Copy(index.clone());
        }
        let moved_value = matches!(shape, Some(WriteProofShape::MovedValue));
        let value = Place::new(
            SemanticLocalIdV1::from_index(if moved_value { 14 } else { 1 }),
            vec![],
            TypeId::from_index(0),
        )
        .unwrap();
        arguments.push(if moved_value {
            Operand::Move(value)
        } else {
            Operand::Copy(value)
        });
        let call = SemanticDirectCallV1::new_callable(
            call.callee(),
            arguments,
            Some(SemanticCallDestinationV1::new(
                Place::new(SemanticLocalIdV1::from_index(11), vec![], boolean).unwrap(),
                SemanticControlFlowEdgeV1::new(SemanticEdgeRoleV1::CallReturn, resume),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        if moved_value {
            // The resumed scalar body still uses argument1. Move a dedicated
            // temporary instead, initialized before the zero-statement cut.
            assert_eq!(locals[14].ty(), TypeId::from_index(0));
            assert_eq!(locals[14].role(), SemanticLocalRoleV1::Temporary);
            let predecessor = &blocks[write_at - 1];
            let Terminator::Call(previous) = predecessor.terminator().kind() else {
                panic!("original witness call");
            };
            assert_eq!(
                previous.destination().unwrap().edge().target().index() as usize,
                write_at
            );
            let mut statements = predecessor.statements().to_vec();
            statements.push(fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1::new(
                old.source(),
                fe2o3_mir_model::semantic_mir_v1::SemanticStatementKindV1::Assign(
                    SemanticAssignmentV1::new(
                        Place::new(
                            SemanticLocalIdV1::from_index(14),
                            vec![],
                            TypeId::from_index(0),
                        )
                        .unwrap(),
                        SemanticRvalueV1::new(
                            TypeId::from_index(0),
                            fe2o3_mir_model::semantic_mir_v1::SemanticRvalueKindV1::Use(
                                Operand::Copy(
                                    Place::new(
                                        SemanticLocalIdV1::from_index(1),
                                        vec![],
                                        TypeId::from_index(0),
                                    )
                                    .unwrap(),
                                ),
                            ),
                        ),
                    ),
                ),
            ));
            blocks[write_at - 1] = SemanticBasicBlockV1::new(
                predecessor.identity(),
                old.source(),
                statements,
                predecessor.terminator().clone(),
            )
            .unwrap();
        }
        blocks[write_at] = SemanticBasicBlockV1::new(
            blocks[write_at].identity(),
            old.source(),
            if matches!(shape, Some(WriteProofShape::NonzeroStatements)) {
                vec![fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1::new(
                    old.source(),
                    fe2o3_mir_model::semantic_mir_v1::SemanticStatementKindV1::Nop,
                )]
            } else {
                vec![]
            },
            SemanticTerminatorV1::new(old.source(), Terminator::Call(call)),
        )
        .unwrap();
        blocks.truncate(write_at + 1);
        *function = Function::new(
            old.identity(),
            old.role(),
            old.item_definition_identity(),
            old.monomorphization_identity(),
            old.generic_type_arguments_identity(),
            old.const_generic_arguments_identity(),
            old.source(),
            old.abi().clone(),
            locals,
            old.entry(),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(old.kernel_entry().unwrap().clone());
    }
}

fn run_write(
    work: usize,
    storage: usize,
    disjoint: bool,
    copied_index: bool,
) -> (Result<()>, usize, usize, usize) {
    run_write_model(work, storage, disjoint, copied_index, |_| {})
}

fn run_write_model(
    work: usize,
    storage: usize,
    disjoint: bool,
    copied_index: bool,
    examine: impl FnOnce(&str),
) -> (Result<()>, usize, usize, usize) {
    run_write_attempt(work, storage, disjoint, copied_index, None, examine)
}

fn run_write_attempt(
    work: usize,
    storage: usize,
    disjoint: bool,
    copied_index: bool,
    custody_fault: Option<bool>,
    examine: impl FnOnce(&str),
) -> (Result<()>, usize, usize, usize) {
    run_write_shape_attempt(
        work,
        storage,
        disjoint,
        copied_index,
        custody_fault,
        None,
        examine,
    )
}

fn run_write_shape_attempt(
    work: usize,
    storage: usize,
    disjoint: bool,
    copied_index: bool,
    custody_fault: Option<bool>,
    shape: Option<WriteProofShape>,
    examine: impl FnOnce(&str),
) -> (Result<()>, usize, usize, usize) {
    let reached = std::cell::Cell::new(0);
    let result =
        super::super::super::super::super::invocations::tests::run_captured_callable_transform(
            work,
            storage,
            |types, functions, callables| {
                write_fixture(types, functions, callables, disjoint, copied_index, shape)
            },
            |owner, launch, budget| {
                capture_element_with_access(
                    owner,
                    launch,
                    budget,
                    DescriptorScalar::U32,
                    AccessMode::WriteOnly,
                )
            },
            |plan, out| {
                super::super::super::super::source_function::tests::with_slots(
                    plan,
                    out,
                    |slots, out| {
                        let program =
                            super::super::super::super::source_function::SourceByteProgram::derive(
                                plan, slots, out,
                            )?;
                        for root in 0..2 {
                            let row = plan.instance(root, 0, out)?;
                            let block = row.blocks.len() - 1;
                            assert!(program.in_place_call(root, 0, block, out)?);
                            reached.set(reached.get() + 1);
                        }
                        if let Some(foreign) = custody_fault {
                            if foreign {
                                let mut work =
                                    fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(LIMIT);
                                let mut budget = Budget::new(&mut work, LIMIT);
                                budget.reserve_storage(out.budget.storage())?;
                                let before =
                                    (budget.work(), budget.storage(), budget.peak_storage());
                                {
                                    let mut writer = Writer::new(&mut budget)?;
                                    assert!(matches!(
                                        program.source_slots(&mut writer),
                                        Err(Error::Source(SourceError::Resource(
                                            Resource::Accounting
                                        )))
                                    ));
                                    assert!(matches!(
                                        program.emit_cut_frame_proofs_v93(&mut writer),
                                        Err(Error::Source(SourceError::Resource(
                                            Resource::Accounting
                                        )))
                                    ));
                                    assert!(matches!(
                                        program.emit_thread_write_normal_proofs_v94(&mut writer),
                                        Err(Error::Source(SourceError::Resource(
                                            Resource::Accounting
                                        )))
                                    ));
                                    assert!(writer.text.is_empty());
                                }
                                assert_eq!(
                                    (budget.work(), budget.storage(), budget.peak_storage()),
                                    before
                                );
                            } else {
                                out.budget.release_storage(1)?;
                            }
                            let before = (out.budget.work(), out.budget.storage(), out.text.len());
                            for _ in 0..2 {
                                assert!(matches!(
                                    program.source_slots(out),
                                    Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                                ));
                                assert!(matches!(
                                    program.emit_cut_frame_proofs_v93(out),
                                    Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                                ));
                                assert!(matches!(
                                    program.emit_thread_write_normal_proofs_v94(out),
                                    Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                                ));
                                assert_eq!(
                                    (out.budget.work(), out.budget.storage(), out.text.len()),
                                    before
                                );
                            }
                            return Err(Error::Source(SourceError::Resource(Resource::Accounting)));
                        }
                        for root in 0..2 {
                            assert!(
                                program.conservation_fuels(root, out)?.is_none(),
                                "a real write must not use memory-neutral preservation"
                            );
                            let hints = program.step_hints(root, out)?.unwrap();
                            assert!(!hints.conserves_heap);
                            assert!(!hints.cuts.is_empty());
                            let original = plan.instance(root, 0, out)?;
                            let write_pc = original.blocks.end - 1;
                            assert!(
                                !hints
                                    .cuts
                                    .iter()
                                    .find(|cut| cut.pc == write_pc)
                                    .unwrap()
                                    .frame_preserving
                            );
                            for cut in &hints.cuts {
                                assert!(hints.fuels[cut.instance] > cut.statements);
                                if cut.has_descriptor_wf() {
                                    assert!(cut.frame_preserving);
                                    assert_ne!(cut.pc, write_pc);
                                }
                            }
                        }
                        let launches = [ExplicitLaunchExtent::Exact {
                            rank: 1,
                            extents: [64, 1, 1],
                        }; 2];
                        let census = super::super::super::super::generate_refinement_v36(
                            slots.correspondence(out)?,
                            &launches,
                            FormalIndexWidth::Bits64,
                            EndiannessV2::Little,
                            out,
                        )?;
                        assert_eq!(census[0], 2);
                        write!(out, "\nverus! {{\n{WRITE_EQUATIONS}\n}}\n")
                            .map_err(|_| out.error())?;
                        assert!(out.text.contains("InvocationSourceByteEventV36::ThreadWrite(InvocationSourceThreadWriteV88"));
                        assert!(out.text.contains("witness_type:"));
                        assert!(out.text.contains(&format!(
                            "moved_receiver: true, moved_index: {}, value:",
                            !copied_index,
                        )));
                        assert_eq!(
                            out.text.matches("proof fn invocation_paired_step_").count(),
                            2
                        );
                        assert!(
                            !out.text
                                .contains("proof fn invocation_paired_source_preserved_")
                        );
                        for forbidden in
                            ["assume(", "external_body", "admit(", "assume_specification"]
                        {
                            assert!(!out.text.contains(forbidden));
                        }
                        assert_eq!(WRITE_EQUATIONS.matches("proof fn ").count(), 3);
                        examine(&out.text);
                        Ok(())
                    },
                )
            },
        );
    if result.0.is_ok() {
        assert_eq!(reached.get(), 2);
    }
    if custody_fault.is_some() {
        assert_eq!(reached.get(), 2);
    }
    result
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
fn protected_original_thread_write_preserves_logical_extent_value_and_bool() {
    use crate::{CanonicalGeneratedVerusProofInputV3, FunctionalRefinementVerusRuntimeLeaseV1};
    use std::time::{Duration, Instant};
    for (disjoint, copied) in [(false, false), (false, true), (true, false)] {
        let mut generated = None;
        let result = run_write_model(LIMIT, LIMIT, disjoint, copied, |text| {
            generated = Some(text.to_owned());
        });
        result
            .0
            .expect("generate the complete original write-source proof");
        assert_eq!(result.2, 37);
        let input = CanonicalGeneratedVerusProofInputV3::new(
            generated.expect("actual complete write model").into_bytes(),
        )
        .expect("canonical full write model");
        let runtime = FunctionalRefinementVerusRuntimeLeaseV1::open_pinned_contexts_v3(
            "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5",
        )
        .expect("requires the actual pinned-runtime lease");
        runtime
            .revalidate()
            .expect("revalidate before source-write proof");
        let mut attempt = runtime.begin_attempt().expect("source-write proof attempt");
        let output = runtime
            .execute_generated_rust_verify(
                &mut attempt,
                &input,
                Instant::now() + Duration::from_secs(120),
                16 * 1024,
            )
            .expect("all source-write model obligations must execute");
        runtime
            .revalidate()
            .expect("revalidate after source-write proof");
        attempt
            .complete()
            .expect("complete source-write proof attempt");
        crate::functional_refinement_receipt_v2::validate_proved_output(&output)
            .expect("full source-write model and equations must genuinely verify");
    }
}

#[test]
fn original_thread_write_captures_nominal_receiver_witness_value_and_bool() {
    for (disjoint, copied) in [(false, false), (false, true), (true, false)] {
        run_write(LIMIT, LIMIT, disjoint, copied).0.unwrap();
    }
}

#[test]
fn original_thread_write_has_exact_and_one_short_capture_accounts() {
    for (disjoint, copied) in [(false, false), (false, true), (true, false)] {
        let (result, work, floor, peak) = run_write(LIMIT, LIMIT, disjoint, copied);
        result.unwrap();
        assert_eq!(floor, 37);
        let (exact, exact_work, exact_floor, exact_peak) = run_write(work, peak, disjoint, copied);
        exact.unwrap();
        assert_eq!((exact_work, exact_peak, exact_floor), (work, peak, floor));
        for (work, storage) in [(work - 1, peak), (work, peak - 1)] {
            assert!(run_write(work, storage, disjoint, copied).0.is_err());
        }
    }
}

#[test]
fn original_thread_write_retains_owner_floor_ledger_and_sticky_refusal() {
    for foreign in [false, true] {
        let result = run_write_attempt(LIMIT, LIMIT, false, false, Some(foreign), |_| {
            panic!("refused owner must not reach source emission");
        });
        assert!(matches!(
            result.0,
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
    }
}

#[test]
fn original_thread_write_partitions_every_authentic_cut_without_heap_conservation() {
    for (disjoint, copied) in [(false, false), (false, true), (true, false)] {
        run_write_model(LIMIT, LIMIT, disjoint, copied, |text| {
            for root in 0..2 {
                let step_name = format!("proof fn invocation_paired_step_{root}_v36(");
                let step = text.split_once(&step_name).unwrap().1.split("proof fn ").next().unwrap();
                let prefix = format!("invocation_paired_cut_{root}_pc");
                let mut count = 0;
                for part in step.split(&prefix).skip(1) {
                    let (pc, _) = part.split_once("_all_v85(source, target);").unwrap();
                    assert!(pc.bytes().all(|byte| byte.is_ascii_digit()));
                    let name = format!("proof fn {prefix}{pc}_all_v85(");
                    let cut = text.split_once(&name).unwrap().1.split("proof fn ").next().unwrap();
                    let (header, body) = cut.split_once("\n{\n").unwrap();
                    let premises = header.split_once(" requires ").unwrap().1.split_once(" ensures ").unwrap().0;
                    assert_eq!(premises.trim(), format!("invocation_paired_related_{root}_v36(source, target), invocation_paired_source_defined_{root}_v36(source, 1),\n source.machine.pc == {pc},"));
                    for conclusion in [
                        format!("invocation_paired_related_{root}_v36(invocation_paired_source_step_{root}_v36(source).state, invocation_paired_actual_step_{root}_v36(target).state)"),
                        format!("invocation_paired_observations_related_{root}_v39(invocation_paired_source_step_{root}_v36(source).events, invocation_paired_actual_step_{root}_v36(target).events)"),
                        format!("invocation_paired_source_step_{root}_v36(source).halted == invocation_paired_actual_step_{root}_v36(target).halted"),
                        format!("source.machine.pc >= 0 ==> invocation_paired_control_values_{root}_v36(source, invocation_source_block_runtime_{root}_v36(source), invocation_byte_boundary_{root}_v36(target))"),
                    ] {
                        assert!(header.contains(&conclusion), "{name}: {conclusion}");
                    }
                    for goal in ["relation", "observations", "halted", "control"] {
                        assert!(body.contains(&format!("{prefix}{pc}_{goal}_v85(source, target);")));
                    }
                    for goal in ["map", "heap", "residual"] {
                        assert!(text.contains(&format!("{prefix}{pc}_{goal}_v85(source, target);")));
                    }
                    count += 1;
                }
                assert!(count > 0);
            }
            assert!(text.contains(" invocation_scalar_store_facts_v92();"));
            assert!(!text.contains("invocation_paired_source_preserved_"));
        }).0.unwrap();
    }
}

#[test]
fn scalar_store_support_keeps_modified_bytes_epochs_and_relocations_explicit() {
    let laws = include_str!("original_semantic_mir_scalar_store_laws_v92.vrs");
    assert_eq!(laws.matches("proof fn ").count(), 6);
    for required in [
        "byte_object_relocations_well_formed_v37(written)",
        "byte_token_well_formed_v37(written.bytes[i])",
        "0 <= written.write_epochs[i] <= written.write_clock",
        "byte_relocation_fragment_v37(relocation, i)",
        "invocation_source_store_enabled_v36(source, pointer, width, alignment, value)",
        "invocation_scalar_store_well_formed_v92(source.memory, pointer, width, bits, little_endian);",
        "invocation_scalar_store_source_well_formed_v92(source, pointer, width, alignment, value, little_endian);",
        "before.live.dom() == after.live.dom()",
    ] {
        assert!(laws.contains(required), "{required}");
    }
    for forbidden in [
        "assume(",
        "admit(",
        "external_body",
        "Map::empty()",
        "memory == source.memory",
    ] {
        assert!(!laws.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn writing_cut_frame_summaries_keep_authentic_coordinates_and_explicit_domains() {
    let frames = include_str!("original_semantic_mir_cut_frame_laws_v93.vrs");
    assert_eq!(frames.matches("proof fn ").count(), 8);
    for required in [
        "source_after.valid, target_after.valid",
        "source_after.memory == source.memory, target_after.memory == target.memory",
        "source_after.generations == source.generations",
        "target_after.generations == target.generations",
        "source_after.frames == source.frames, target_after.frames == target.frames",
        "invocation_source_frame_equal_v93(before, after) && after.machine.pc == before.machine.pc",
    ] {
        assert!(frames.contains(required), "{required}");
    }
    for forbidden in ["assume(", "admit(", "external_body", "spinoff_prover"] {
        assert!(!frames.contains(forbidden), "{forbidden}");
    }
    for (disjoint, copied) in [(false, false), (false, true), (true, false)] {
        run_write_model(LIMIT, LIMIT, disjoint, copied, |text| {
            assert_eq!(text.matches(frames).count(), 1);
            assert!(!text.contains("proof fn invocation_external_store_"));
            let mut summaries = 0;
            for suffix in text.split("proof fn invocation_cut_source_frame_").skip(1) {
                let (coordinates, body) = suffix.split_once("_v93(source:").unwrap();
                let (root, pc) = coordinates.split_once('_').unwrap();
                assert!(root.bytes().all(|byte| byte.is_ascii_digit()));
                assert!(pc.bytes().all(|byte| byte.is_ascii_digit()));
                let body = body.split("proof fn ").next().unwrap();
                let premises = body
                    .split_once(" requires ")
                    .unwrap()
                    .1
                    .split_once(" ensures ")
                    .unwrap()
                    .0;
                assert_eq!(premises.trim(), format!("source.machine.pc == {pc},"));
                assert!(body.contains("invocation_cut_frame_descriptor_length_v93("));
                assert!(body.contains(
                    "invocation_source_frame_equal_v93(source, invocation_paired_source_step_"
                ));
                assert!(text.contains(&format!(
                    " invocation_cut_source_frame_{coordinates}_v93(source);"
                )));
                summaries += 1;
            }
            assert!(summaries >= 2);
            // These conditional support laws do not replace either root's step theorem.
            assert_eq!(text.matches("proof fn invocation_paired_step_").count(), 2);
            assert!(!text.contains("invocation_paired_source_preserved_"));
        })
        .0
        .unwrap();
    }
}

#[test]
fn writing_cut_support_has_exact_full_model_work_and_storage_limits() {
    for (disjoint, copied) in [(false, false), (false, true), (true, false)] {
        let (result, work, floor, peak) = run_write_model(LIMIT, LIMIT, disjoint, copied, |_| {});
        result.unwrap();
        assert_eq!(floor, 37);
        let (exact, used, after, used_peak) = run_write_model(work, peak, disjoint, copied, |_| {});
        exact.unwrap();
        assert_eq!((used, after, used_peak), (work, floor, peak));
        for (work_limit, storage_limit) in [(work - 1, peak), (work, peak - 1)] {
            let (short, _, after, _) =
                run_write_model(work_limit, storage_limit, disjoint, copied, |_| {});
            assert!(short.is_err());
            assert_eq!(after, floor);
        }
    }
}

#[test]
fn descriptor_wf_composition_uses_authentic_events_and_retains_map_obligations() {
    let laws = include_str!("original_semantic_mir_source_wf_laws_v95.vrs");
    assert_eq!(laws.matches("proof fn ").count(), 5);
    for forbidden in ["assume(", "admit(", "external_body", "Map::empty()"] {
        assert!(!laws.contains(forbidden), "{forbidden}");
    }
    assert!(laws.contains("!source.objects.contains_key(destination)"));
    assert!(laws.contains("parent: None }).machine.valid"));
    for (disjoint, copied) in [(false, false), (false, true), (true, false)] {
        run_write_model(LIMIT, LIMIT, disjoint, copied, |text| {
            assert_eq!(text.matches(laws).count(), 1);
            let mut mapped = 0;
            for proof in text.split("proof fn invocation_paired_cut_").skip(1) {
                let proof = proof.split("proof fn ").next().unwrap();
                if !proof.contains(" invocation_source_descriptor_length_wf_v95(") {
                    continue;
                }
                let (header, body) = proof.split_once("\n{\n").unwrap();
                let (coordinates, _) = header.split_once("_map_v85(").unwrap();
                let (root, pc) = coordinates.split_once("_pc").unwrap();
                let (premises, conclusion) = header.split_once(" requires ").unwrap().1.split_once(" ensures ").unwrap();
                assert_eq!(premises.trim(), format!("invocation_paired_related_{root}_v36(source, target), invocation_paired_source_defined_{root}_v36(source, 1),\n source.machine.pc == {pc},"));
                assert_eq!(conclusion.trim(), format!("invocation_source_byte_map_valid_{root}_v36(invocation_paired_source_step_{root}_v36(source).state, invocation_paired_actual_step_{root}_v36(target).state),"));
                assert!(body.starts_with(" hide(invocation_source_byte_state_well_formed_v36);\n"));
                assert!(body.contains(" invocation_source_slice_copy_wf_v95("));
                assert!(body.contains(" invocation_source_descriptor_borrow_wf_v95("));
                assert!(body.contains("assert(!c"));
                assert!(body.contains(".machine.valid) by {"));
                for event in body.lines().filter_map(|line| line.strip_prefix(" match invocation_source_byte_event_")) {
                    let event = event.strip_suffix(" {").unwrap();
                    assert!(text.contains(&format!("let after = match invocation_source_byte_event_{event} {{")), "{event}");
                }
                mapped += 1;
            }
            assert_eq!(mapped, 2);
            assert_eq!(text.matches("proof fn invocation_paired_step_").count(), 2);
        }).0.unwrap();
    }
}

#[test]
fn thread_write_normalization_uses_authentic_literals_without_new_step_premises() {
    let laws = include_str!("original_semantic_mir_thread_write_normal_laws_v94.vrs");
    assert_eq!(laws.matches("proof fn ").count(), 4);
    assert_eq!(laws.matches("#[verifier::spinoff_prover]").count(), 1);
    for required in [
        "local != write.input, local != write.index",
        "ordinary_memory_width_v30(write.recipe.width)",
        "after.machine.valid ==> source.machine.valid",
        "after.machine.memory == byte_store_v30(source.machine.memory,",
        "else { after.machine.memory == source.machine.memory }",
    ] {
        assert!(laws.contains(required), "{required}");
    }
    for forbidden in ["assume(", "admit(", "external_body", "Map::empty()"] {
        assert!(!laws.contains(forbidden), "{forbidden}");
    }
    for (disjoint, copied) in [(false, false), (false, true), (true, false)] {
        run_write_model(LIMIT, LIMIT, disjoint, copied, |text| {
            assert_eq!(text.matches(laws).count(), 1);
            let mut calls = 0;
            for proof in text.split("proof fn invocation_paired_cut_").skip(1) {
                let proof = proof.split("proof fn ").next().unwrap();
                let Some((_, after)) = proof.split_once(" let normalized_write = ") else {
                    continue;
                };
                let (literal, _) = after.split_once(";\n").unwrap();
                assert!(text.contains(&format!(
                    "let event = InvocationSourceByteEventV36::ThreadWrite({literal});"
                )));
                let (header, body) = proof.split_once("\n{\n").unwrap();
                assert!(header.contains("_heap_v85(source:"));
                assert!(!header.contains("normalized_write"));
                assert!(!header.contains("local_normal_form"));
                assert!(body.starts_with(" hide(invocation_source_thread_write_v88);\n"));
                assert!(body.contains(
                    " invocation_thread_write_local_normal_form_v93(source, normalized_write,"
                ));
                calls += 1;
            }
            assert_eq!(calls, 2);
            assert_eq!(text.matches("proof fn invocation_paired_step_").count(), 2);
        })
        .0
        .unwrap();
    }
}

#[test]
fn thread_write_normalization_shape_misses_keep_original_full_obligations() {
    for shape in [
        WriteProofShape::MovedValue,
        WriteProofShape::NonzeroStatements,
    ] {
        run_write_shape_attempt(LIMIT, LIMIT, false, false, None, Some(shape), |text| {
            assert!(!text.contains("proof fn invocation_thread_write_local_normal_"));
            assert!(!text.contains(" let normalized_write = "));
            assert!(text.contains(
                "InvocationSourceByteEventV36::ThreadWrite(InvocationSourceThreadWriteV88"
            ));
            assert_eq!(text.matches("proof fn invocation_paired_step_").count(), 2);
            for root in 0..2 {
                let name = format!("proof fn invocation_paired_step_{root}_v36(");
                let proof = text
                    .split_once(&name)
                    .unwrap()
                    .1
                    .split("proof fn ")
                    .next()
                    .unwrap();
                let header = proof.split_once("\n{\n").unwrap().0;
                for conclusion in [
                    "paired_related_",
                    "paired_observations_related_",
                    ".halted ==",
                    "paired_control_values_",
                ] {
                    assert!(header.contains(conclusion), "{conclusion}");
                }
            }
        })
        .0
        .unwrap();
    }
}
