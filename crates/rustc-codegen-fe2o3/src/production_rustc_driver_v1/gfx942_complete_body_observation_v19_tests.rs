//! Runs only after a genuine live-source normal target continuation succeeds.
use super::*;

pub(super) fn observe(
    target: crate::production_pipeline::AuthenticatedCompleteBodyTargetModuleV19,
    feature: &str,
    output: &Path,
) -> Result<Value, String> {
    assert_eq!(target.target_name(), "gfx942:xnack-");
    let checked = target.checked();
    let [source_root] = checked.source_launch().roots() else {
        panic!("one exact source launch root is required");
    };
    // This is the real retained max_grid=[2,1,1] source envelope, not the
    // later CPU request or an envelope synthesized by this observer.
    assert_eq!(source_root.layout().global_extents(), [128, 1, 1]);
    assert_eq!(source_root.layout().workgroup_extents(), [64, 1, 1]);
    assert!(!checked.grants_artifact_or_launch_authority());
    let semantic = checked.semantic_ssa().source_semantic();
    assert_eq!(semantic.wire_version(), SemanticMirWireVersionV1::V36);
    assert_eq!(semantic.roots().len(), 1);
    assert_eq!(semantic.functions().len(), 1);
    let function = &semantic.functions()[semantic.roots()[0].index() as usize];
    let calls = function
        .blocks()
        .iter()
        .filter_map(|block| match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => call.complete_body_source_vnext(),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    let source = calls[0];
    assert!(source.matches_function(function));

    let executable = checked.executable();
    let module = executable.module();
    assert_eq!(module.functions.len(), 1);
    assert_eq!(module.kernels.len(), 1);
    assert_eq!(module.kernels[0].entry, module.functions[0].id);
    let entry_symbol = module.functions[0].id.as_str().to_owned();
    let canonical_kernel_id = module.kernels[0].id.as_str().to_owned();
    assert_eq!(canonical_kernel_id, entry_symbol);
    assert_eq!(
        function.kernel_entry().unwrap().export_symbol().as_bytes(),
        entry_symbol.as_bytes()
    );
    let operations = module.functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let declarations = operations
        .iter()
        .filter_map(|operation| match operation.kind {
            OperationKind::Gfx942CompleteBodyDeclaration(declaration) => {
                assert!(operation.results.is_empty());
                Some(declaration)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(declarations.len(), 1);
    let declaration = declarations[0];
    assert_eq!(declaration.origin.root_axes, source.root_axes());
    assert_eq!(declaration.origin.mir_body, source.mir_body());
    assert_eq!(declaration.origin.semantic_block, source.block_identity());
    assert_eq!(
        declaration.origin.source_signature,
        source.source_signature()
    );
    assert_eq!(declaration.origin.rustc_fn_abi, source.rustc_fn_abi());
    assert_eq!(
        declaration.origin.frontend_bytes_sha256,
        source.frontend_bytes_sha256()
    );
    assert_eq!(declaration.origin.raw_block, source.raw_block());
    assert!(declaration.origin.is_complete());
    assert_eq!(declaration.registers.scratch(), 32);
    assert_eq!(declaration.registers.output(), 33);
    assert_eq!(declaration.registers.inputs(), [34, 35, 36]);
    let diamond = feature == FEATURES[1];
    assert_eq!(declaration.block_count, if diamond { 4 } else { 1 });
    assert_eq!(declaration.instruction_count, if diamond { 2 } else { 1 });
    let steps = operations
        .iter()
        .filter(|operation| matches!(operation.kind, OperationKind::Gfx942CompleteBodyStep(_)))
        .count();
    assert_eq!(steps, usize::from(declaration.instruction_count));
    let correspondence = checked.materialized().correspondence();
    assert_eq!(
        correspondence.semantic_sha256(),
        semantic.semantic_sha256().as_bytes()
    );
    assert_eq!(correspondence.parameters().len(), 5);
    assert_eq!(
        correspondence.source_blocks().len(),
        function.blocks().len()
    );

    let cpu_cases = observe_cpu(executable, diamond);
    let canonical = executable.canonical_bytes().to_vec();
    let canonical_identity = super::super::lower_hex_v1(executable.identity().digest());
    let semantic_identity = super::super::lower_hex_v1(semantic.semantic_sha256().as_bytes());
    let source_axes = declaration
        .origin
        .root_axes
        .map(|value| super::super::lower_hex_v1(&value));
    let llvm = target.llvm_ir().to_owned();
    assert!(llvm.len() <= 16 * 1024);
    assert!(llvm.contains("define amdgpu_kernel"));
    assert!(llvm.contains("asm sideeffect"));
    assert!(llvm.contains("v_mov_b32_e32"));
    assert_eq!(llvm.matches("s_endpgm").count(), 1);

    // Consume the SAME immutable actual-source target used above; no conversion
    // from canonical bytes, mutable graph or caller-supplied origin is involved.
    let prepared =
        crate::production_worker_handoff::prepare_complete_body_worker_handoff_v19(target)
            .map_err(|error| error.to_string())?;
    let (handoff, descriptor) = prepared
        .into_validated_parts()
        .map_err(|error| error.to_string())?;
    assert_eq!(handoff.target().to_string(), "gfx942:xnack-");
    assert!(!handoff.authenticates_compiler_origin());
    assert!(!handoff.grants_compiler_authority());
    assert!(!descriptor.authenticates_compiler_origin());
    assert!(!descriptor.grants_launch_authority());
    assert_eq!(descriptor.table().kernels().len(), 1);
    let descriptor_kernel = &descriptor.table().kernels()[0];
    assert_eq!(descriptor_kernel.entry_name().as_str(), entry_symbol);
    let descriptor_kernel_id = super::super::lower_hex_v1(descriptor_kernel.kernel_id().as_bytes());
    assert_eq!(
        descriptor.table().canonical_code_object_digest().as_bytes(),
        &[0; 32]
    );
    let worker_llvm = std::str::from_utf8(handoff.module_bytes()).unwrap();
    assert!(
        worker_llvm.starts_with(&llvm),
        "descriptor binding must preserve executable LLVM"
    );
    assert!(worker_llvm[llvm.len()..].contains(".fe2o3.kd.v1"));
    let decoded =
        fe2o3_compiler_ffi::CompilerModuleHandoffV2::decode(handoff.canonical_bytes()).unwrap();
    assert_eq!(decoded.module_bytes(), handoff.module_bytes());
    assert_eq!(decoded.canonical_bytes(), handoff.canonical_bytes());
    let decoded_descriptor =
        fe2o3_compiler_ffi::CompilerDescriptorSourceV1::decode(descriptor.canonical_bytes())
            .unwrap();
    assert_eq!(decoded_descriptor, descriptor);

    for (name, bytes, cap) in [
        ("canonical-v19.bin", canonical.as_slice(), 1024 * 1024),
        ("canonical.ll", llvm.as_bytes(), 16 * 1024),
        ("worker.ll", handoff.module_bytes(), 1024 * 1024),
        ("handoff-v2.bin", handoff.canonical_bytes(), 1024 * 1024),
        (
            "descriptor-v1.bin",
            descriptor.canonical_bytes(),
            1024 * 1024,
        ),
    ] {
        super::super::publish_new_inert_output(&output.join(name), bytes, cap, name)?;
    }
    Ok(json!({
        "stage": "actual_source_v36_checked_v19_cpu_and_normal_worker_handoff",
        "semantic_sha256": semantic_identity,
        "canonical_v19_identity": canonical_identity,
        "entry_symbol": entry_symbol,
        "canonical_kernel_id": canonical_kernel_id,
        "descriptor_kernel_id": descriptor_kernel_id,
        "same_owner_source_canonical_descriptor_entry": true,
        "canonical_bytes_sha256": digest(&canonical),
        "canonical_v19_length": canonical.len(),
        "source_root_axes": source_axes,
        "authored_blocks": declaration.block_count,
        "authored_steps": declaration.instruction_count,
        "cpu_cases": cpu_cases,
        "cpu_grid_sizes": [64, 128],
        "cpu_tail_lengths": [0, 1, 63, 64, 65, 127, 128, 129],
        "cpu_selectors": [0, 1, u32::MAX],
        "canaries_unchanged": true,
        "normal_checked_continuation": true,
        "normal_worker_preparation": true,
        "canonical_llvm_sha256": digest(llvm.as_bytes()),
        "worker_llvm_sha256": digest(handoff.module_bytes()),
        "handoff_sha256": digest(handoff.canonical_bytes()),
        "descriptor_sha256": digest(descriptor.canonical_bytes()),
    }))
}

fn observe_cpu(
    executable: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV19,
    diamond: bool,
) -> usize {
    let admitted =
        AdmittedSimulationModuleV1::admit_v19(executable, SimulationLimitsV1::default()).unwrap();
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
                    assert_eq!(execution.identity().wire_version(), 19);
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
