//! Runs only after a genuine live-source pre-ranked diagnostic continuation.
use super::*;

pub(super) fn observe(
    mut target: crate::production_pipeline::physical_lds_exchange_diagnostic_v22::AuthenticatedPhysicalLdsExchangeDiagnosticV22,
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
    assert_eq!(launch.layout().workgroup_extents(), [128, 1, 1]);
    let semantic = materialized.semantic_ssa().source_semantic();
    assert_eq!(semantic.wire_version(), SemanticMirWireVersionV1::V39);
    assert_eq!(semantic.functions().len(), 1);
    let function = &semantic.functions()[0];
    let mut calls = function
        .blocks()
        .iter()
        .filter_map(|block| match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => Some((
                block.identity(),
                call,
                call.physical_lds_exchange_source_v39()
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
    let OperationKind::Gfx942PhysicalLdsExchangeDeclaration(declaration) =
        body.blocks[0].operations[0].kind
    else {
        panic!("declaration")
    };
    assert_eq!(body.blocks.len(), 1);
    assert_eq!(
        declaration.lds_frame,
        fe2o3_kernel_ir::Gfx942PhysicalLdsExchangeFrameV1 {
            byte_offset: 0,
            byte_length: 512,
            alignment: 4,
            publication_epoch: 1
        }
    );
    assert_eq!(declaration.native_instruction_count, 32);
    assert_eq!(calls.len(), 34);
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
                            OperationKind::Gfx942PhysicalLdsExchangeStep(step) => Some(step.site),
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
    let output_register = if feature == FEATURES[1] { 26 } else { 18 };
    let stores = body
        .blocks
        .iter()
        .flat_map(|b| &b.operations)
        .filter_map(|o| match o.kind {
            OperationKind::Gfx942PhysicalLdsExchangeStep(step)
                if step.instruction.opcode
                    == fe2o3_kernel_ir::Gfx942PhysicalLdsExchangeOpcodeV1::GlobalStoreDword =>
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
    assert!(llvm.contains("\"amdgpu-lds-size\"=\"512,512\""));
    assert!(!llvm.contains("ret void"));
    assert!(
        target
            .native_observation()
            .starts_with("FE2O3_PHYSICAL_LDS_EXCHANGE_V22_NATIVE_OBSERVATION_INPUT_V1\n")
    );
    assert!(
        target
            .native_observation()
            .contains("lds_frame 0 512 4 1\n")
    );
    // Same live immutable source/emission owners supply all three inert outputs.
    // This is not a normal production descriptor, ranked check or worker handoff.
    super::super::physical_lds_exchange_diagnostic_export_v22::publish(&target, output)?;
    Ok(json!({
        "stage":"actual_source_mir39_pre_ranked_kir22_cpu_emitter_diagnostics",
        "semantic_sha256":super::super::lower_hex_v1(semantic.semantic_sha256().as_bytes()),
        "canonical_identity":super::super::lower_hex_v1(executable.identity().digest()),
        "entry_symbol":entry.id.as_str(),"authored_blocks":body.blocks.len(),
        "native_instruction_count":declaration.native_instruction_count,"source_occurrences":calls.len(),
        "output_register":output_register,"source_correspondence_exact":true,
        "cpu_cases":cpu_cases,"cpu_grids":[128],"cpu_tail_lengths":[0,1,63,64,65,127,128,129],
        "cpu_input_seeds":[0,u32::MAX,0xaaaa_5555_u32,19],"canaries_unchanged":true,
        "cpu_exact_negative_cases":6,"full_exec_input_bounds_and_initialization_checked":true,
        "same_backing_even_nonoverlap_refused":true,"same_compiler_ledger_for_cpu_view":true,
        "cpu_observed_barrier_cases":1,"publication_participants":128,
        "lds_frame":{"byte_offset":0,"byte_length":512,"alignment":4,"publication_epoch":1},
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
    SimulationRequestV1::new(entry, [grid, 1, 1], [128, 1, 1], args).with_shared_buffers(vec![
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
    executable: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV22,
    budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> usize {
    use fe2o3_kir_sim::{
        SimulationErrorV1 as E, SimulationExecutionErrorKindV1 as X,
        SimulationPreflightErrorV1 as P,
    };
    let floor = budget.storage();
    let work_before = budget.work();
    let (admitted, storage) = AdmittedSimulationModuleV1::admit_v22_with_verification_budget(
        executable,
        SimulationLimitsV1::default(),
        budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() > work_before);
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
            let req = request(entry, 128, length, 128, seed, None);
            let original = req.clone();
            let run = admitted.simulate(&req, target, limits).unwrap();
            assert_eq!(req, original);
            assert!(!run.grants_execution_authority());
            assert_eq!(run.identity().wire_version(), 22);
            assert_eq!(run.identity().digest(), executable.identity().digest());
            let output = run.shared_buffer(BufferBackingIdV1(9)).unwrap();
            for (index, bytes) in output.bytes().chunks_exact(4).enumerate() {
                let written = (2..2 + length.min(128)).contains(&index);
                assert_eq!(
                    bytes,
                    if written {
                        word((index - 2) ^ 64, seed).to_le_bytes()
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
    for (req, uninitialized) in [
        (request(entry, 128, 0, 128, 19, Some(127)), true),
        (request(entry, 127, 0, 128, 19, None), false),
    ] {
        let Err(E::Execution(error)) = admitted.simulate(&req, target, limits) else {
            panic!("full-EXEC input must refuse")
        };
        if uninitialized {
            assert!(matches!(error.kind, X::UninitializedRead { .. }));
        } else {
            assert!(matches!(error.kind, X::OutOfBounds { .. }));
        }
    }
    for offset in [8, 520] {
        let mut alias = request(entry, 128, 128, 128, 19, None);
        alias.shared_buffers[0].buffer = BufferArgumentV1::new(
            ScalarType::U32,
            AccessMode::ReadWrite,
            4,
            vec![0; 2048],
            vec![true; 2048],
            target,
        )
        .unwrap();
        alias.arguments[1] = SimulationArgumentV1::BufferView(
            BufferViewArgumentV1::new(
                BufferBackingIdV1(7),
                ScalarType::U32,
                AccessMode::ReadWrite,
                4,
                offset,
                128,
                target,
            )
            .unwrap(),
        );
        let SimulationArgumentV1::BufferView(input) = &alias.arguments[0] else {
            panic!("input")
        };
        let SimulationArgumentV1::BufferView(output) = &alias.arguments[1] else {
            panic!("output")
        };
        assert_eq!(input.byte_offset(), 8);
        assert_eq!(input.elements(), 128);
        assert_eq!(
            input.byte_offset() + input.elements() * 4 <= output.byte_offset(),
            offset == 520
        );
        assert!(matches!(
            admitted.preflight(&alias, target, limits),
            Err(P::PhysicalLdsExchangeAliasedArgumentsV22)
        ));
    }
    let mut readonly = request(entry, 128, 128, 128, 19, None);
    readonly.shared_buffers[1].buffer = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadOnly,
        4,
        vec![0; 528],
        vec![true; 528],
        target,
    )
    .unwrap();
    assert!(matches!(
        admitted.preflight(&readonly, target, limits),
        Err(P::BufferAccess { .. })
    ));
    let mut records = Records(0);
    assert!(matches!(
        admitted.simulate_debugged_with_sink(
            &request(entry, 128, 128, 128, 19, None),
            target,
            limits,
            fe2o3_kir_sim::SimulationDebugCaptureLimitsV1::new(8, 768, 8, 4096).unwrap(),
            &mut records
        ),
        Err(E::Preflight(P::PhysicalLdsExchangeDebugUnavailableV22))
    ));
    assert_eq!(records.0, 0);
    observe_barrier(&admitted, executable, entry);
    drop(admitted);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(count, 32);
    count
}
#[derive(Default)]
struct Events(Vec<fe2o3_kir_sim::SimulationEventV1>);
impl fe2o3_kir_sim::SimulationEventSinkV1 for Events {
    fn record(
        &mut self,
        event: &fe2o3_kir_sim::SimulationEventV1,
    ) -> Result<(), fe2o3_kir_sim::SimulationEventSinkErrorV1> {
        assert!(self.0.len() < 16384);
        self.0.push(event.clone());
        Ok(())
    }
}
struct Records(usize);
impl fe2o3_kir_sim::SimulationDebugSinkV1 for Records {
    fn record(
        &mut self,
        _: fe2o3_kir_sim::SimulationDebugRecordV1,
    ) -> fe2o3_kir_sim::SimulationDebugSinkControlV1 {
        self.0 += 1;
        fe2o3_kir_sim::SimulationDebugSinkControlV1::Continue
    }
}
fn observe_barrier(
    admitted: &AdmittedSimulationModuleV1,
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV22,
    entry: &str,
) {
    use fe2o3_kernel_ir::Gfx942PhysicalLdsExchangeOpcodeV1 as O;
    use fe2o3_kir_sim::SimulationEventKindV1 as K;
    let block = &owner.module().functions[0].body.as_ref().unwrap().blocks[0];
    let operation = |opcode| {
        let sites: Vec<_> = block
            .operations
            .iter()
            .enumerate()
            .filter_map(|(i, op)| match op.kind {
                OperationKind::Gfx942PhysicalLdsExchangeStep(step)
                    if step.instruction.opcode == opcode =>
                {
                    Some(i)
                }
                _ => None,
            })
            .collect();
        assert_eq!(sites.len(), 1);
        u32::try_from(sites[0]).unwrap()
    };
    let global_read = operation(O::GlobalLoadDword);
    let local_write = operation(O::LdsWriteB32);
    let publication = operation(O::WorkgroupPublishBarrier);
    let local_read = operation(O::LdsReadB32);
    let global_write = operation(O::GlobalStoreDword);
    let mut events = Events::default();
    let req = request(entry, 128, 13, 128, 19, None);
    let original = req.clone();
    let run = admitted
        .simulate_observed_with_sink(
            &req,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .unwrap();
    assert_eq!(req, original);
    assert!(!run.grants_execution_authority());
    let releases: Vec<_> = events
        .0
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e.kind, K::WorkgroupBarrierRelease { .. }))
        .collect();
    assert_eq!(releases.len(), 1);
    let (release_index, release) = releases[0];
    assert_eq!(release.site.operation, Some(publication));
    assert!(matches!(
        release.kind,
        K::WorkgroupBarrierRelease {
            phase: 0,
            participants: 128
        }
    ));
    for lane in 0..128 {
        let reads: Vec<_> = events
            .0
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.invocation.global[0] == lane && matches!(e.kind, K::MemoryRead { .. })
            })
            .collect();
        let writes: Vec<_> = events
            .0
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.invocation.global[0] == lane && matches!(e.kind, K::MemoryWrite { .. })
            })
            .collect();
        assert_eq!(reads.len(), 2);
        assert_eq!(writes.len(), 1 + usize::from(lane < 13));
        assert_eq!(reads[0].1.site.operation, Some(global_read));
        assert_eq!(reads[1].1.site.operation, Some(local_read));
        assert_eq!(writes[0].1.site.operation, Some(local_write));
        assert!(writes[0].0 < release_index);
        assert!(reads[1].0 > release_index);
        if lane < 13 {
            assert_eq!(writes[1].1.site.operation, Some(global_write));
        }
        for (_, e) in reads.iter().chain(writes.iter()) {
            assert_eq!(e.site.block, block.id);
            assert_eq!(u64::from(e.invocation.local[0]), lane);
        }
    }
    let output = run.shared_buffer(BufferBackingIdV1(9)).unwrap();
    for (index, bytes) in output.bytes().chunks_exact(4).enumerate() {
        let written = (2..15).contains(&index);
        assert_eq!(
            bytes,
            if written {
                word((index - 2) ^ 64, 19).to_le_bytes()
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
}

#[path = "gfx942_physical_lds_exchange_capture_v22_tests.rs"]
mod capture;
