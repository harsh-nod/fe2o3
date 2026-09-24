//! Runs only after a genuine live-source pre-ranked diagnostic continuation.
use super::*;

pub(super) fn observe(
    mut target: crate::production_pipeline::physical_global_copy_diagnostic_v21::AuthenticatedPhysicalGlobalCopyDiagnosticV21,
    feature: &str,
    output: &Path,
) -> Result<Value, String> {
    let cpu_cases = target.observe_with_budget(|materialized, budget| {
        materialized.verify_equivalence(budget).unwrap();
        observe_cpu(materialized.executable(), budget)
    });
    let materialized = target.materialized();
    assert!(!materialized.grants_artifact_or_launch_authority());
    let [launch] = materialized.source_launch().roots() else {
        panic!("one source launch")
    };
    assert_eq!(launch.layout().global_extents(), [128, 1, 1]);
    assert_eq!(launch.layout().workgroup_extents(), [64, 1, 1]);
    let semantic = materialized.semantic_ssa().source_semantic();
    assert_eq!(semantic.wire_version(), SemanticMirWireVersionV1::V38);
    assert_eq!(semantic.functions().len(), 1);
    let function = &semantic.functions()[0];
    let mut calls = function
        .blocks()
        .iter()
        .filter_map(|block| match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => Some((
                block.identity(),
                call,
                call.physical_global_copy_source_v38()
                    .expect("source occurrence"),
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
    let OperationKind::Gfx942PhysicalGlobalCopyDeclaration(declaration) =
        body.blocks[0].operations[0].kind
    else {
        panic!("declaration")
    };
    assert_eq!(body.blocks.len(), 1);
    assert_eq!(declaration.native_instruction_count, 24);
    assert_eq!(calls.len(), 26);
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
            [declaration.block]
                .iter()
                .flat_map(|b| [b.label_site, b.terminator_site])
                .chain(
                    body.blocks
                        .iter()
                        .flat_map(|b| b.operations.iter())
                        .filter_map(|o| match o.kind {
                            OperationKind::Gfx942PhysicalGlobalCopyStep(step) => Some(step.site),
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
    let output_register = if feature == FEATURES[1] { 22 } else { 8 };
    let stores = body
        .blocks
        .iter()
        .flat_map(|b| &b.operations)
        .filter_map(|o| match o.kind {
            OperationKind::Gfx942PhysicalGlobalCopyStep(step)
                if step.instruction.opcode
                    == fe2o3_kernel_ir::Gfx942PhysicalGlobalCopyOpcodeV1::GlobalStoreDword =>
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
    assert_eq!(correspondence.parameters().len(), 2);
    let llvm = target.llvm_ir();
    assert!(llvm.contains("define amdgpu_kernel") && llvm.contains("asm sideeffect"));
    assert!(llvm.contains("unreachable"));
    assert!(!llvm.contains("ret void"));
    assert!(
        target
            .native_observation()
            .starts_with("FE2O3_PHYSICAL_GLOBAL_COPY_V21_NATIVE_OBSERVATION_INPUT_V1\n")
    );
    // Same live immutable source/emission owners supply all three inert outputs.
    // This is not a normal production descriptor, ranked check or worker handoff.
    super::super::physical_global_copy_diagnostic_export_v21::publish(&target, output)?;
    Ok(json!({
        "stage":"actual_source_mir38_pre_ranked_kir21_cpu_emitter_diagnostics",
        "semantic_sha256":super::super::lower_hex_v1(semantic.semantic_sha256().as_bytes()),
        "canonical_identity":super::super::lower_hex_v1(executable.identity().digest()),
        "entry_symbol":entry.id.as_str(),"authored_blocks":body.blocks.len(),
        "native_instruction_count":declaration.native_instruction_count,"source_occurrences":calls.len(),
        "output_register":output_register,"source_correspondence_exact":true,
        "cpu_cases":cpu_cases,"cpu_grids":[64,128],"cpu_tail_lengths":[0,1,63,64,65,127,128,129],
        "cpu_input_seeds":[0,u32::MAX,0xaaaa_5555_u32,19],"canaries_unchanged":true,
        "cpu_exact_negative_cases":3,"full_exec_input_bounds_and_initialization_checked":true,
        "same_backing_even_nonoverlap_refused":true,"same_compiler_ledger_for_cpu_view":true,
        "canonical_bytes_sha256":digest(executable.canonical_bytes()),
        "llvm_sha256":digest(llvm.as_bytes()),"native_observation_sha256":digest(target.native_observation().as_bytes()),
        "normal_checked_continuation":false,"normal_worker_preparation":false,
        "ranked_formal_descriptor_continuation":"unavailable",
        "native_llvm_executed":false,"hardware_observed":false,"grants_artifact_or_launch_authority":false,
    }))
}

fn word(index: usize, seed: u32) -> u32 {
    (index as u32).wrapping_mul(0x9e37_79b9).wrapping_add(seed)
}
fn request(
    entry: &str,
    input_len: usize,
    output_len: usize,
    grid: u64,
    seed: u32,
    uninitialized: Option<usize>,
) -> SimulationRequestV1 {
    let target = SimulationTargetV1::amdgpu_64();
    let mut bytes = vec![0x5a; (input_len + 4) * 4];
    for index in 0..input_len {
        bytes[(index + 2) * 4..(index + 3) * 4].copy_from_slice(&word(index, seed).to_le_bytes());
    }
    let mut initialized = vec![true; bytes.len()];
    if let Some(index) = uninitialized {
        initialized[(index + 2) * 4..(index + 3) * 4].fill(false);
    }
    let input = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadOnly,
        4,
        bytes,
        initialized,
        target,
    )
    .unwrap();
    let output = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        vec![0xa5; (output_len + 4) * 4],
        vec![false; (output_len + 4) * 4],
        target,
    )
    .unwrap();
    let args = [
        (BufferBackingIdV1(7), AccessMode::ReadOnly, input_len),
        (BufferBackingIdV1(9), AccessMode::ReadWrite, output_len),
    ]
    .into_iter()
    .map(|(id, access, len)| {
        SimulationArgumentV1::BufferView(
            BufferViewArgumentV1::new(id, ScalarType::U32, access, 4, 8, len, target).unwrap(),
        )
    })
    .collect();
    SimulationRequestV1::new(entry, [grid, 1, 1], [64, 1, 1], args).with_shared_buffers(vec![
        SharedBufferV1 {
            id: BufferBackingIdV1(7),
            buffer: input,
        },
        SharedBufferV1 {
            id: BufferBackingIdV1(9),
            buffer: output,
        },
    ])
}
pub(crate) fn observe_cpu(
    executable: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV21,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> usize {
    let floor = budget.storage();
    let (admitted, storage) = AdmittedSimulationModuleV1::admit_v21_with_verification_budget(
        executable,
        SimulationLimitsV1::default(),
        budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(admitted.module(), executable.module());
    let parameters = &executable.module().functions[0].signature.parameters;
    assert_eq!(parameters.len(), 2);
    for (ty, access) in parameters
        .iter()
        .zip([AccessMode::ReadOnly, AccessMode::ReadWrite])
    {
        assert_eq!(
            *ty,
            Type::slice(
                Type::Scalar(ScalarType::U32),
                fe2o3_kernel_ir::AddressSpace::Global,
                access
            )
        );
    }
    let entry = executable.module().kernels[0].id.as_str();
    let target = SimulationTargetV1::amdgpu_64();
    let limits = SimulationLimitsV1::default();
    let mut count = 0;
    for seed in [0_u32, u32::MAX, 0xaaaa_5555, 19] {
        for length in [0_usize, 1, 63, 64, 65, 127, 128, 129] {
            for grid in [64_u64, 128] {
                let req = request(entry, grid as usize, length, grid, seed, None);
                let original = req.clone();
                let run = admitted.simulate(&req, target, limits).unwrap();
                assert_eq!(req, original);
                assert!(!run.grants_execution_authority());
                assert_eq!(run.identity().wire_version(), 21);
                assert_eq!(run.identity().digest(), executable.identity().digest());
                let output = run.shared_buffer(BufferBackingIdV1(9)).unwrap();
                for (index, bytes) in output.bytes().chunks_exact(4).enumerate() {
                    let written = (2..2 + length.min(grid as usize)).contains(&index);
                    assert_eq!(
                        bytes,
                        if written {
                            word(index - 2, seed).to_le_bytes()
                        } else {
                            [0xa5; 4]
                        }
                    );
                    assert!(
                        output.initialized()[index * 4..index * 4 + 4]
                            .iter()
                            .all(|v| *v == written)
                    );
                }
                let input = run.shared_buffer(BufferBackingIdV1(7)).unwrap();
                assert_eq!(input.bytes(), original.shared_buffers[0].buffer.bytes());
                assert_eq!(
                    input.initialized(),
                    original.shared_buffers[0].buffer.initialized()
                );
                count += 1;
            }
        }
    }
    // No output lane is active in these controls. Actual full-EXEC input reads
    // still require valid initialized storage; output masking is not a proof.
    for (req, uninitialized) in [
        (request(entry, 64, 0, 64, 19, Some(63)), true),
        (request(entry, 63, 0, 64, 19, None), false),
    ] {
        let Err(fe2o3_kir_sim::SimulationErrorV1::Execution(error)) =
            admitted.simulate(&req, target, limits)
        else {
            panic!("full-EXEC input must refuse")
        };
        if uninitialized {
            assert!(matches!(
                error.kind,
                fe2o3_kir_sim::SimulationExecutionErrorKindV1::UninitializedRead { .. }
            ));
        } else {
            assert!(matches!(
                error.kind,
                fe2o3_kir_sim::SimulationExecutionErrorKindV1::OutOfBounds { .. }
            ));
        }
    }
    let mut alias = request(entry, 64, 64, 64, 19, None);
    alias.shared_buffers[0].buffer = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        vec![0; 1024],
        vec![true; 1024],
        target,
    )
    .unwrap();
    alias.arguments[1] = SimulationArgumentV1::BufferView(
        BufferViewArgumentV1::new(
            BufferBackingIdV1(7),
            ScalarType::U32,
            AccessMode::ReadWrite,
            4,
            264,
            64,
            target,
        )
        .unwrap(),
    );
    let SimulationArgumentV1::BufferView(input) = &alias.arguments[0] else {
        panic!("input view");
    };
    let SimulationArgumentV1::BufferView(output) = &alias.arguments[1] else {
        panic!("output view");
    };
    assert_eq!(input.byte_offset(), 8);
    assert_eq!(input.elements(), 64);
    assert_eq!(output.byte_offset(), 264);
    assert!(input.byte_offset() + input.elements() * 4 <= output.byte_offset());
    assert!(matches!(
        admitted.preflight(&alias, target, limits),
        Err(fe2o3_kir_sim::SimulationPreflightErrorV1::PhysicalGlobalCopyAliasedArgumentsV21)
    ));
    drop(admitted);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(count, 64);
    count
}
