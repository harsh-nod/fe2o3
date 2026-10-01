use super::super::source_function::tests::with_slots;
use super::*;

const LIMIT: usize = 256 * 1024 * 1024;

fn run(
    work: usize,
    storage: usize,
    examine: impl FnOnce(&PairedInvocations<'_, '_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::invocations::tests::run_variant(work, storage, true, |plan, out| {
        with_slots(plan, out, |slots, out| {
            let program = SourceByteProgram::derive(plan, slots, out)?;
            let paired = PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
            examine(&paired, out)
        })
    })
}

#[test]
fn original_mir_paired_consumer_keeps_complete_cuts_call_arguments_and_suspended_callers() {
    run(LIMIT, LIMIT, |paired, out| {
        assert_eq!(paired.roots.len(), 2);
        assert_eq!(paired.instances.iter().flatten().count(), 6);
        for root in &paired.roots {
            let mut source_blocks = Vec::new();
            for cut in root.cuts.iter().flatten() {
                assert!(root.instances.contains(&cut.instance));
                assert!(!source_blocks.contains(&cut.source));
                source_blocks.push(cut.source);
            }
            assert!(!source_blocks.is_empty());
        }
        assert!(
            paired
                .instances
                .iter()
                .flatten()
                .any(|row| !row.suspended.is_empty())
        );
        assert!(
            paired
                .instances
                .iter()
                .flatten()
                .any(|row| row.arguments.len() == 2)
        );
        paired.emit(out)?;
        assert!(
            out.text
                .contains("invocation_source_byte_storage_related_0_v36(source, target)")
        );
        assert!(out.text.contains("source_result.operands.len() == 2"));
        assert!(
            out.text
                .contains("let original = source_result.operands[1].value")
        );
        assert!(
            out.text
                .contains("match source_result.returned { Some(original)")
        );
        assert!(out.text.contains("head.observations + tail.observations"));
        assert!(!out.text.contains("assume("));
        assert!(!out.text.contains("invocation_actual_segment_"));
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_paired_consumer_preserves_independent_source_validity_and_physical_lifetime() {
    run(LIMIT, LIMIT, |paired, out| {
        paired.emit(out)?;
        assert!(
            out.text
                .contains("invocation_paired_source_defined_0_v36(source, 1)")
        );
        assert!(out.text.contains("source.machine.valid && target.valid"));
        assert!(out.text.contains("invocation_byte_follow_0_v36(head.state"));
        assert!(out.text.contains("!invocation_byte_cut_0_v36(target.pc)"));
        assert!(out.text.contains("fuel == 0 || target.pc <"));
        assert!(out.text.contains("source.machine.frames.active.len() == 0"));
        assert!(!out.text.contains("byte_end_frame_v30"));
        assert!(
            !out.text
                .contains("invocation_source_byte_activate_v36(source, target")
        );
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_paired_consumer_uses_real_scalar_storage_and_same_byte_dispatcher() {
    super::super::super::invocations::tests::run_scalar_allocation_variant(
        LIMIT,
        LIMIT,
        |plan, out| {
            with_slots(plan, out, |slots, out| {
                let program = SourceByteProgram::derive(plan, slots, out)?;
                let paired =
                    PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                let inventory = slots.correspondence(out)?.inventory(out.budget)?;
                let (physical_analysis, physical_storage) =
                    fe2o3_kernel_analysis::analyze_canonical_kir_private_bytes_v38(
                        inventory,
                        fe2o3_kernel_analysis::CanonicalKirPrivateByteLimitsV38 {
                            max_boundaries: 4096,
                        },
                        out.budget,
                    )?;
                out.budget
                    .reserve_storage(physical_storage.retained_storage())?;
                for root in 0..paired.roots.len() {
                    let physical = plan.root(root, out)?.physical;
                    super::super::super::super::byte_function_v30::ByteFunctionV30::derive(
                        inventory,
                        &physical_analysis,
                        fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(physical as u32),
                        FormalIndexWidth::Bits64,
                        slots,
                        out,
                    )?
                    .emit(root, out)?;
                }
                super::super::byte_bindings::SourceByteBindings::derive(slots, out)?.emit(out)?;
                paired.emit(out)?;
                assert!(out.text.contains("MemoryOperationEffectV30::Allocate"));
                assert!(out.text.contains("MemoryOperationEffectV30::Read"));
                assert!(out.text.contains("MemoryOperationEffectV30::Write"));
                assert!(out.text.contains("let head = byte_block_step_0_v30"));
                assert!(
                    out.text
                        .contains("invocation_source_byte_storage_related_0_v36(source, target)")
                );
                drop(physical_analysis);
                out.budget
                    .release_storage(physical_storage.retained_storage())?;
                Ok(())
            })
        },
    )
    .0
    .unwrap();
}

#[test]
fn original_mir_paired_consumer_exact_and_one_short_complete_resource_replay() {
    let execute = |work, storage| run(work, storage, |paired, out| paired.emit(out));
    let measured = execute(LIMIT, LIMIT);
    measured.0.unwrap();
    execute(measured.1, measured.3).0.unwrap();
    assert!(execute(measured.1 - 1, measured.3).0.is_err());
    assert!(execute(measured.1, measured.3 - 1).0.is_err());
}

#[test]
fn original_mir_paired_consumer_rejects_unknown_width_before_any_emission() {
    super::super::super::invocations::tests::run_variant(LIMIT, LIMIT, true, |plan, out| {
        with_slots(plan, out, |slots, out| {
            let program = SourceByteProgram::derive(plan, slots, out)?;
            let before = out.text.len();
            assert!(matches!(
                PairedInvocations::derive(plan, &program, FormalIndexWidth::Unknown, out),
                Err(Error::Statement(_))
            ));
            assert_eq!(before, out.text.len());
            Ok(())
        })
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_paired_initial_and_trace_share_explicit_execution_coordinates() {
    run(LIMIT, LIMIT, |paired, out| {
        paired.emit(out)?;
        for root in 0..2 {
            assert!(out.text.contains(&format!(
                "invocation_paired_raw_initial_{root}_v36(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37)"
            )));
            assert!(out.text.contains(&format!(
                "invocation_source_initial_runtime_{root}_v36(arguments, external, execution)"
            )));
            assert!(out.text.contains(&format!(
                "invocation_paired_ready_{root}_v36(arguments, external, execution)"
            )));
            assert!(out.text.contains(&format!(
                "Some(execution) => invocation_runtime_execution_{root}_v37(execution), None => false"
            )));
        }
        assert!(!out.text.contains("frames: byte_root_frame_v30("));
        assert!(out.text.contains("frames: byte_root_frame_with_execution_v37("));
        assert!(super::super::bytes::INVOCATION_BYTES_V36.contains(
            "source.frames.execution == target.frames.execution"
        ));
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_paired_source_readiness_is_an_independent_native_input_obligation() {
    run(LIMIT, LIMIT, |paired, out| {
        paired.emit(out)?;
        for root in 0..2 {
            let predicate = out
                .text
                .split(&format!("open spec fn invocation_paired_native_inputs_{root}_v38"))
                .nth(1)
                .unwrap()
                .split("proof fn")
                .next()
                .unwrap();
            assert!(predicate.contains(&format!(
                "invocation_runtime_execution_{root}_v37(execution)"
            )));
            assert!(predicate.contains(&format!(
                "invocation_paired_arguments_{root}_v36(arguments)"
            )));
            assert!(predicate.contains("byte_memory_well_formed_v30(external)"));
            assert!(predicate.contains("byte_native_view_inputs_v38(external, arguments)"));
            assert!(predicate.contains(
                "external.live.contains_key(allocation) ==> !invocation_private_allocation_v36(allocation)"
            ));
            assert!(!predicate.contains("source_initial"));
            assert!(!predicate.contains("source_ready"));
            assert!(!predicate.contains("machine.valid"));

            let readiness = out
                .text
                .split(&format!("proof fn invocation_paired_source_ready_{root}_v38"))
                .nth(1)
                .unwrap()
                .split("open spec fn")
                .next()
                .unwrap();
            let required = readiness.split(" requires ").nth(1).unwrap();
            let (premise, consequence) = required.split_once(" ensures ").unwrap();
            assert_eq!(
                premise.trim(),
                format!("invocation_paired_native_inputs_{root}_v38(arguments, external, execution),")
            );
            assert!(consequence.contains(&format!(
                "invocation_source_initial_runtime_{root}_v36(arguments, external, execution).machine.valid"
            )));
            assert!(out.text.contains(&format!(
                "valid: invocation_paired_native_inputs_{root}_v38(arguments, external, execution)"
            )));
            for theorem in ["initial", "initial_trace"] {
                let theorem = out
                    .text
                    .split(&format!("proof fn invocation_paired_{theorem}_{root}_v36"))
                    .nth(1)
                    .unwrap();
                let premise = theorem
                    .split(" requires ")
                    .nth(1)
                    .unwrap()
                    .split(" ensures ")
                    .next()
                    .unwrap();
                assert!(premise.contains(&format!(
                    "invocation_paired_native_inputs_{root}_v38(arguments, external, execution)"
                )));
            }
            assert!(out.text.contains(&format!(
                "{{ invocation_paired_source_ready_{root}_v38(arguments, external, execution); }}"
            )));
        }
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_execution_domain_preserves_original_required_and_maximum_workgroup_contract() {
    use fe2o3_mir_model::semantic_mir_v1::*;
    super::super::super::invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        |_, functions| {
            for function in &mut functions[..2] {
                let entry = function.kernel_entry().unwrap();
                let contract = entry.source_contract();
                let launch = SemanticKernelLaunchBoundsV1::new(
                    Some(SemanticWorkgroupDimensionsV1::new([32, 1, 1]).unwrap()),
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    None,
                )
                .unwrap();
                *function = function
                    .clone()
                    .with_kernel_entry(SemanticKernelEntryV1::new(
                        entry.export_symbol().clone(),
                        entry.kernel_binding_identity(),
                        SemanticKernelSourceContractV1::new_with_resources(
                            Some(launch),
                            contract.resources(),
                            contract.unsafe_assembly(),
                            contract.reachable_assembly(),
                        )
                        .unwrap(),
                    ));
            }
        },
        |plan, out| {
            with_slots(plan, out, |slots, out| {
                for root in 0..2 {
                    super::super::emit_execution_v37(slots.correspondence(out)?, root, out)?;
                }
                assert!(
                    out.text
                        .contains("execution.workgroup == seq![32int, 1int, 1int]")
                );
                assert!(out.text.contains("execution.workgroup[0] <= 64"));
                assert!(out.text.contains("execution.workgroup[1] <= 1"));
                assert!(out.text.contains("execution.workgroup[2] <= 1"));
                assert!(
                    out.text
                        .contains("execution.extent == invocation_runtime_launch_0_v36().1")
                );
                assert!(
                    out.text
                        .contains("byte_execution_well_formed_v37(execution)")
                );
                assert!(
                    out.text
                        .contains("memory_value_modulus_v30(invocation_runtime_index_bytes_v36())")
                );
                Ok(())
            })
        },
    )
    .0
    .unwrap();
}

#[test]
fn original_mir_paired_root_abi_keeps_ignored_unit_arguments_out_of_physical_parameters() {
    use fe2o3_mir_model::semantic_mir_v1::*;
    super::super::super::invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        |_, functions| {
            let unit = SemanticTypeIdV1::from_index(1);
            for (root, function) in functions[..2].iter_mut().enumerate() {
                let mut parameters: Vec<_> = function
                    .abi()
                    .arguments()
                    .iter()
                    .map(|argument| argument.value().clone())
                    .collect();
                let argument = parameters.len() as u32;
                parameters.push(SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore));
                let abi = SemanticFunctionAbiV1::new(
                    SemanticAbiIdentityV1::from_sha256([210 + root as u8; 32]),
                    SemanticLayoutIdentityV1::from_sha256([212 + root as u8; 32]),
                    SemanticCanonAbiV1::GpuKernel,
                    false,
                    false,
                    parameters,
                    SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
                )
                .unwrap();
                let mut locals = function.locals().to_vec();
                locals.push(SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([214 + root as u8; 32]),
                    unit,
                    SemanticLocalRoleV1::Argument(argument),
                    function.source(),
                ));
                *function = SemanticFunctionDeclV1::new(
                    function.identity(),
                    function.role(),
                    function.item_definition_identity(),
                    function.monomorphization_identity(),
                    function.generic_type_arguments_identity(),
                    function.const_generic_arguments_identity(),
                    function.source(),
                    abi,
                    locals,
                    function.entry(),
                    function.blocks().to_vec(),
                )
                .unwrap()
                .with_kernel_entry(function.kernel_entry().unwrap().clone());
            }
        },
        |plan, out| {
            with_slots(plan, out, |slots, out| {
                let program = SourceByteProgram::derive(plan, slots, out)?;
                let paired =
                    PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                for root in &paired.roots {
                    let entry = paired.instances[root.instances.start].as_ref().unwrap();
                    assert_eq!(entry.arguments.len(), 3);
                    assert_eq!(entry.arguments[2].definition, None);
                    assert_eq!(root.parameters.len(), 2);
                    assert_eq!(
                        root.parameters
                            .iter()
                            .map(|row| row.source)
                            .collect::<Vec<_>>(),
                        [0, 1]
                    );
                }
                paired.emit(out)?;
                assert!(out.text.contains("arguments.len() == 3"));
                assert!(
                    out.text
                        .contains("let value = arguments[2];  value == MemoryValueV30::Unit")
                );
                assert!(!out.text.contains(", arguments[2])"));
                Ok(())
            })
        },
    )
    .0
    .unwrap();
}
