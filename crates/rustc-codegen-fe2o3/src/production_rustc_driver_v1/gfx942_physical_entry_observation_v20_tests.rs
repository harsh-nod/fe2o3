//! Runs only after a genuine live-source pre-ranked diagnostic continuation.
use super::*;

pub(super) fn observe(
    target: crate::production_pipeline::physical_entry_diagnostic_v20::AuthenticatedPhysicalEntryDiagnosticV20,
    feature: &str,
    output: &Path,
) -> Result<Value, String> {
    let materialized = target.materialized();
    assert!(!materialized.grants_artifact_or_launch_authority());
    let [launch] = materialized.source_launch().roots() else {
        panic!("one source launch")
    };
    assert_eq!(launch.layout().global_extents(), [128, 1, 1]);
    assert_eq!(launch.layout().workgroup_extents(), [64, 1, 1]);
    let semantic = materialized.semantic_ssa().source_semantic();
    assert_eq!(semantic.wire_version(), SemanticMirWireVersionV1::V37);
    assert_eq!(semantic.functions().len(), 1);
    let function = &semantic.functions()[0];
    let mut calls = function
        .blocks()
        .iter()
        .filter_map(|block| match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => Some((
                block.identity(),
                call,
                call.physical_entry_source_v37().expect("source occurrence"),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    calls.sort_by_key(|(_, _, source)| source.occurrence());
    let executable = materialized.executable();
    let module = executable.module();
    let [kernel] = module.kernels.as_slice() else {
        panic!("one actual kernel")
    };
    let [entry] = module.functions.as_slice() else {
        panic!("one actual entry")
    };
    assert_eq!(kernel.entry, entry.id);
    assert_eq!(kernel.id.as_str(), entry.id.as_str());
    assert_eq!(
        function.kernel_entry().unwrap().export_symbol().as_bytes(),
        entry.id.as_str().as_bytes()
    );
    let body = entry.body.as_ref().unwrap();
    let OperationKind::Gfx942PhysicalEntryDeclaration(declaration) =
        body.blocks[0].operations[0].kind
    else {
        panic!("declaration")
    };
    let diamond = feature != FEATURES[0];
    assert_eq!(declaration.block_count, if diamond { 4 } else { 1 });
    assert_eq!(
        declaration.native_instruction_count,
        if diamond { 25 } else { 21 }
    );
    assert_eq!(calls.len(), if diamond { 31 } else { 23 });
    for (index, (block, call, source)) in calls.iter().enumerate() {
        assert_eq!(usize::from(source.occurrence()), index);
        assert_eq!(source.block_identity(), *block.as_bytes());
        assert!(source.matches_function(function));
        assert_eq!(declaration.origin.root_axes, source.root_axes());
        assert_eq!(declaration.origin.mir_body, source.mir_body());
        assert_eq!(
            declaration.origin.source_signature,
            source.source_signature()
        );
        assert_eq!(declaration.origin.rustc_fn_abi, source.rustc_fn_abi());
        assert_eq!(
            declaration.origin.frontend_bytes_sha256,
            source.frontend_bytes_sha256()
        );
        let actual = if index == 0 {
            Some(declaration.begin_site)
        } else {
            declaration.blocks[..usize::from(declaration.block_count)]
                .iter()
                .flat_map(|b| [b.label_site, b.terminator_site])
                .chain(
                    body.blocks
                        .iter()
                        .flat_map(|b| b.operations.iter())
                        .filter_map(|o| match o.kind {
                            OperationKind::Gfx942PhysicalEntryStep(step) => Some(step.site),
                            _ => None,
                        }),
                )
                .find(|site| usize::from(site.occurrence) == index)
        }
        .expect("every actual source occurrence represented exactly");
        assert_eq!(actual.raw_block, source.raw_block());
        assert_eq!(actual.semantic_block_identity, *block.as_bytes());
        assert_eq!(actual.semantic_callable_index, call.callee().index());
    }
    let output_register = if feature == FEATURES[2] { 22 } else { 8 };
    let stores = body
        .blocks
        .iter()
        .flat_map(|b| &b.operations)
        .filter_map(|o| match o.kind {
            OperationKind::Gfx942PhysicalEntryStep(step)
                if step.instruction.opcode
                    == fe2o3_kernel_ir::Gfx942PhysicalEntryOpcodeV20::GlobalStoreDword =>
            {
                Some(step)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(stores.len(), 1);
    assert_eq!(stores[0].instruction.source1, output_register);
    let correspondence = materialized.correspondence();
    assert_eq!(
        correspondence.semantic_sha256(),
        semantic.semantic_sha256().as_bytes()
    );
    assert_eq!(
        correspondence.source_blocks().len(),
        function.blocks().len()
    );
    assert_eq!(correspondence.parameters().len(), 5);
    let cpu_cases = observe_cpu(executable, diamond);
    let llvm = target.llvm_ir();
    assert!(llvm.contains("define amdgpu_kernel") && llvm.contains("asm sideeffect"));
    assert!(llvm.contains("unreachable"));
    assert!(!llvm.contains("ret void"));
    assert!(
        target
            .native_observation()
            .starts_with("FE2O3_PHYSICAL_ENTRY_V20_NATIVE_OBSERVATION_INPUT_V1\n")
    );
    // Same live immutable source/emission owners supply all three inert outputs.
    // This is not a normal production descriptor, ranked check or worker handoff.
    super::super::physical_entry_diagnostic_export_v20::publish(&target, output)?;
    Ok(json!({
        "stage":"actual_source_mir37_pre_ranked_kir20_cpu_emitter_diagnostics",
        "semantic_sha256":super::super::lower_hex_v1(semantic.semantic_sha256().as_bytes()),
        "canonical_identity":super::super::lower_hex_v1(executable.identity().digest()),
        "entry_symbol":entry.id.as_str(),"authored_blocks":declaration.block_count,
        "native_instruction_count":declaration.native_instruction_count,"source_occurrences":calls.len(),
        "output_register":output_register,"source_correspondence_exact":true,
        "cpu_cases":cpu_cases,"cpu_grids":[64,128],"cpu_tail_lengths":[0,1,63,64,65,127,128,129],
        "cpu_selectors":[0,1,u32::MAX],"canaries_unchanged":true,
        "canonical_bytes_sha256":digest(executable.canonical_bytes()),
        "llvm_sha256":digest(llvm.as_bytes()),"native_observation_sha256":digest(target.native_observation().as_bytes()),
        "normal_checked_continuation":false,"normal_worker_preparation":false,
        "ranked_formal_descriptor_continuation":"unavailable",
        "native_llvm_executed":false,"hardware_observed":false,"grants_artifact_or_launch_authority":false,
    }))
}

pub(crate) fn observe_cpu(
    executable: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV20,
    diamond: bool,
) -> usize {
    let admitted =
        AdmittedSimulationModuleV1::admit_v20(executable, SimulationLimitsV1::default()).unwrap();
    let target = SimulationTargetV1::amdgpu_64();
    let parameters = &executable.module().functions[0].signature.parameters;
    assert_eq!(parameters.len(), 5);
    assert!(matches!(parameters[0], Type::Slice(_)));
    assert!(
        parameters[1..]
            .iter()
            .all(|ty| *ty == Type::Scalar(ScalarType::U32))
    );
    let inputs = [
        [0_u32, u32::MAX, 1_u32],
        [u32::MAX, 0, 0x8000_0000],
        [0xaaaa_5555, 0x5555_aaaa, 19],
        [19, 23, 42],
    ];
    let mut count = 0;
    for [a, b, c] in inputs {
        for selector in [0_u32, 1, u32::MAX] {
            for length in [0_usize, 1, 63, 64, 65, 127, 128, 129] {
                for grid in [64_u64, 128] {
                    let backing = BufferBackingIdV1(7);
                    let bytes = (length + 4) * 4;
                    let buffer = BufferArgumentV1::new(
                        ScalarType::U32,
                        AccessMode::ReadWrite,
                        4,
                        vec![0x5a; bytes],
                        vec![false; bytes],
                        target,
                    )
                    .unwrap();
                    let view = BufferViewArgumentV1::new(
                        backing,
                        ScalarType::U32,
                        AccessMode::ReadWrite,
                        4,
                        8,
                        length,
                        target,
                    )
                    .unwrap();
                    let mut arguments = vec![SimulationArgumentV1::BufferView(view)];
                    arguments.extend([a, b, c, selector].map(|value| {
                        SimulationArgumentV1::Scalar(
                            ScalarBitsV1::new(ScalarType::U32, u128::from(value), target).unwrap(),
                        )
                    }));
                    let request = SimulationRequestV1::new(
                        executable.module().kernels[0].id.clone(),
                        [grid, 1, 1],
                        [64, 1, 1],
                        arguments,
                    )
                    .with_shared_buffers(vec![SharedBufferV1 {
                        id: backing,
                        buffer,
                    }]);
                    let original = request.clone();
                    let execution = admitted
                        .simulate(&request, target, SimulationLimitsV1::default())
                        .unwrap();
                    assert_eq!(request, original);
                    assert!(!execution.grants_execution_authority());
                    assert_eq!(execution.identity().wire_version(), 20);
                    assert_eq!(
                        execution.identity().digest(),
                        executable.identity().digest()
                    );
                    // Independent source-intent oracle, not descriptor/model evaluation.
                    let expected = if diamond && selector != 0 { b } else { a };
                    let written = length.min(grid as usize);
                    let output = execution.shared_buffer(backing).unwrap();
                    for element in 0..length + 4 {
                        let range = element * 4..(element + 1) * 4;
                        if (2..2 + written).contains(&element) {
                            assert_eq!(&output.bytes()[range.clone()], &expected.to_le_bytes());
                            assert!(output.initialized()[range].iter().all(|value| *value));
                        } else {
                            // Two words on BOTH sides, plus in-view elements beyond grid.
                            assert_eq!(&output.bytes()[range.clone()], &[0x5a; 4]);
                            assert!(output.initialized()[range].iter().all(|value| !*value));
                        }
                    }
                    count += 1;
                }
            }
        }
    }
    assert_eq!(count, 192);
    count
}
