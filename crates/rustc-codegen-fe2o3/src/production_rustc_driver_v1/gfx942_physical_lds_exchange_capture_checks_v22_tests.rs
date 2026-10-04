//! Assertions over real typed captures only; no source parser or ISA evaluator.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Ledger,
    Gfx942PhysicalLdsExchangeOpcodeV1 as Opcode, OperationKind, ScalarType, ValueId,
    VerifiedCanonicalKernelIrModuleV22 as Owner,
};
use fe2o3_kir_debugger::{
    PhysicalLdsExchangeDebugNavigationV22 as Navigation,
    PhysicalLdsExchangeDebugSessionV22 as Session,
};
use fe2o3_kir_sim::*;
type Target = crate::production_pipeline::physical_lds_exchange_diagnostic_v22::AuthenticatedPhysicalLdsExchangeDiagnosticV22;
type Record<'a> = PhysicalLdsExchangeDebugRecordRefV22<'a>;
const RECORDS: usize = 16384;
const REQUEST_STORAGE: usize = 8192;

fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_reachable_functions: 1,
        max_reachable_operations: 64,
        max_invocations: 128,
        max_workgroups: 1,
        max_scheduled_slots: 128,
        max_steps: 10000,
        max_call_depth: 1,
        max_ssa_values: 192,
        max_allocations: 4,
        max_allocation_bytes: 4096,
        max_total_bytes: 8192,
        max_memory_access_records: 1024,
        ..SimulationLimitsV1::default()
    }
}
fn options(records: usize) -> PhysicalLdsExchangeDebugOptionsV22 {
    PhysicalLdsExchangeDebugOptionsV22::new(
        limits(),
        SimulationDebugCaptureLimitsV1::new(1, 192, 4, 8192).unwrap(),
        records,
    )
    .unwrap()
}
fn binding<'a>(r: Record<'a>, id: ValueId) -> Option<PhysicalLdsExchangeDebugBindingRefV22<'a>> {
    (0..r.binding_count(0)?)
        .filter_map(|n| r.binding(0, n))
        .find(|b| b.value() == id)
}
fn operation(owner: &Owner, opcode: Opcode) -> (u32, ValueId) {
    let body = owner.module().functions[0].body.as_ref().unwrap();
    let mut found=body.blocks[0].operations.iter().enumerate().filter(|(_,o)|matches!(o.kind,OperationKind::Gfx942PhysicalLdsExchangeStep(s)if s.instruction.opcode==opcode));
    let (index, op) = found.next().unwrap();
    assert!(found.next().is_none());
    (
        u32::try_from(index).unwrap(),
        op.results.first().map_or(ValueId(u32::MAX), |v| v.id),
    )
}
pub(super) fn observe(target: Target, feature: &str, canonical: &[u8], llvm: &[u8]) -> Value {
    assert_eq!(
        target.materialized().executable().canonical_bytes(),
        canonical
    );
    assert_eq!(target.llvm_ir().as_bytes(), llvm);
    assert!(!target.materialized().grants_artifact_or_launch_authority());
    assert_eq!(
        target
            .materialized()
            .semantic_ssa()
            .source_semantic()
            .wire_version(),
        fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V39
    );
    let expected_register = if feature == FEATURES[1] { 26 } else { 18 };
    let owner = target.materialized().executable();
    let (store, _) = operation(owner, Opcode::GlobalStoreDword);
    let OperationKind::Gfx942PhysicalLdsExchangeStep(step) =
        owner.module().functions[0].body.as_ref().unwrap().blocks[0].operations[store as usize]
            .kind
    else {
        panic!("actual store")
    };
    assert_eq!(step.instruction.source1, expected_register);
    let (target,observation)=target.observe_with_owned_ledger(|source,mut ledger|{
        let source_floor=ledger.storage();let source_work=ledger.work();let source_peak=ledger.peak_storage();
        let failures=(ledger.failed_work(),ledger.failed_storage());

        ledger.with_budget(|b|source.verify_equivalence(b)).unwrap();
        assert_eq!(ledger.storage(),source_floor);
        let executable=source.executable();
        let entry=executable.module().kernels[0].id.as_str();
        let (admitted,storage)=ledger.with_budget(|b|AdmittedSimulationModuleV1::admit_v22_with_verification_budget(executable,limits(),b)).unwrap();
        ledger.with_budget(|b|b.reserve_storage(storage.retained_storage())).unwrap();
        let admitted_floor=ledger.storage();
        let mut cases=Vec::new();
        for (seed,output) in [(0x8000_0001,129usize),(u32::MAX,13usize)] {
            ledger.with_budget(|b|b.reserve_storage(REQUEST_STORAGE)).unwrap();
            let req=super::super::request(entry,128,output,128,seed,None);
            let session=Session::capture(&admitted,executable,&req,options(RECORDS),ledger);
            let (case,returned)=positive(session,executable,&req,seed,output);
            ledger=returned;cases.push(case);drop(req);
            ledger.with_budget(|b|b.release_storage(REQUEST_STORAGE)).unwrap();
            assert_eq!(ledger.storage(),admitted_floor);
        }
        let mut negatives=0;
        for mode in 0..4 {
            ledger.with_budget(|b|b.reserve_storage(REQUEST_STORAGE)).unwrap();
            let mut req=super::super::request(entry,if mode==1{127}else{128},0,128,19,(mode==0).then_some(127));
            if mode>=2 {
                req.shared_buffers[0].buffer=BufferArgumentV1::new(ScalarType::U32,AccessMode::ReadWrite,4,vec![0;1040],vec![true;1040],SimulationTargetV1::amdgpu_64()).unwrap();
                req.arguments[1]=SimulationArgumentV1::BufferView(BufferViewArgumentV1::new(BufferBackingIdV1(7),ScalarType::U32,AccessMode::ReadWrite,4,if mode==2{8}else{520},128,SimulationTargetV1::amdgpu_64()).unwrap());
            }
            let c=admitted.capture_physical_lds_exchange_debug_v22(executable,&req,options(RECORDS),ledger);
            assert_eq!(c.error(),None);
            assert_eq!(c.outcome(),if mode<2{PhysicalLdsExchangeDebugOutcomeV22::ExecutionFailed}else{PhysicalLdsExchangeDebugOutcomeV22::PreflightRefused});
            if mode>=2{assert!(c.is_empty());}
            assert!(!(0..c.len()).filter_map(|i|c.record(i)).any(|r|matches!(r.memory_access(),Some((SimulationDebugMemoryAccessV1::WriteCommitted,_,_,_,AddressSpace::Global)))));
            let usage=c.usage();ledger=c.into_budget();assert_eq!(ledger.storage(),usage.entry_storage);
            assert_eq!(ledger.work(),usage.work);assert_eq!(ledger.peak_storage(),usage.peak_storage);
            drop(req);ledger.with_budget(|b|b.release_storage(REQUEST_STORAGE)).unwrap();negatives+=1;
        }
        ledger.with_budget(|b|b.reserve_storage(REQUEST_STORAGE)).unwrap();
        let req=super::super::request(entry,128,129,128,19,None);
        let mut records=super::super::Records(0);
        assert!(matches!(admitted.simulate_debugged_with_sink(&req,SimulationTargetV1::amdgpu_64(),limits(),SimulationDebugCaptureLimitsV1::new(1,192,4,8192).unwrap(),&mut records),
            Err(SimulationErrorV1::Preflight(SimulationPreflightErrorV1::PhysicalLdsExchangeDebugUnavailableV22))));
        assert_eq!(records.0,0);drop(req);ledger.with_budget(|b|b.release_storage(REQUEST_STORAGE)).unwrap();
        drop(admitted);ledger.with_budget(|b|b.release_storage(storage.retained_storage())).unwrap();
        ledger.with_budget(|b|source.verify_equivalence(b)).unwrap();
        assert_eq!(ledger.storage(),source_floor);
        assert!(ledger.work()>source_work&&ledger.peak_storage()>=source_peak);
        assert_eq!((ledger.failed_work(),ledger.failed_storage()),failures);
        let result=json!({"cases":cases,"negative_capture_cases":negatives,"generic_debug_records":0,
            "source_floor":source_floor,"source_work_before":source_work,"source_work_after":ledger.work(),
            "peak_before":source_peak,"peak_after":ledger.peak_storage(),"same_owned_ledger":true,
            "original_denial_history_preserved":true,"source_replay_before_and_after":true});
        (result,ledger)
    });
    assert_eq!(
        target.materialized().executable().canonical_bytes(),
        canonical
    );
    assert_eq!(target.llvm_ir().as_bytes(), llvm);
    assert!(!target.materialized().grants_artifact_or_launch_authority());
    observation
}

fn positive(
    mut s: Session,
    owner: &Owner,
    request: &SimulationRequestV1,
    seed: u32,
    output: usize,
) -> (Value, Ledger) {
    use PhysicalLdsExchangeDebugSymbolicKindV22 as Kind;
    use SimulationDebugBarrierActionV1::{Arrive, Release};
    use SimulationDebugCheckpointPhaseV1::{AfterOperation as After, BeforeOperation as Before};
    use SimulationDebugMemoryAccessV1::{Read, WriteCommitted};
    assert_eq!(s.outcome(), PhysicalLdsExchangeDebugOutcomeV22::Completed);
    assert_eq!(s.capture_error(), None);
    assert_eq!(s.capture_stop(), None);
    assert_eq!(s.identity().digest(), owner.identity().digest());
    assert_eq!(s.identity().wire_version(), 22);
    assert!(s.records_len() <= RECORDS);
    s.charge_query_work(
        s.records_len()
            .checked_mul(4096)
            .unwrap()
            .checked_add(65536)
            .unwrap(),
    )
    .unwrap();
    let final_memory = (0..s.records_len())
        .rev()
        .filter_map(|i| s.record(i))
        .find(|r| r.phase().is_some())
        .unwrap();
    let input_allocation = final_memory.memory_allocation(0).unwrap().0;
    let output_allocation = final_memory.memory_allocation(1).unwrap().0;
    let lds_allocation = final_memory.memory_allocation(2).unwrap().0;
    assert_ne!(input_allocation, output_allocation);
    assert_ne!(input_allocation, lds_allocation);
    assert_ne!(output_allocation, lds_allocation);
    let (global, gid) = operation(owner, Opcode::GlobalLoadDword);
    let (read, rid) = operation(owner, Opcode::LdsReadB32);
    let (write, _) = operation(owner, Opcode::LdsWriteB32);
    let (barrier, _) = operation(owner, Opcode::WorkgroupPublishBarrier);
    let (store, _) = operation(owner, Opcode::GlobalStoreDword);
    let mut seen = [false; 128];
    let mut arrived = [false; 128];
    let mut pending_global = [0usize; 128];
    let mut ready_global = [0usize; 128];
    let mut pending_lds = [0usize; 128];
    let mut ready_lds = [0usize; 128];
    let mut release = None;
    let mut max_write = None;
    let mut min_read = None;
    let mut local_writes = 0;
    let mut local_reads = 0;
    let mut global_writes = 0;
    let mut global_reads = 0;
    let mut pending_index = None;
    let mut ready_index = None;
    let mut before_store = None;
    let mut after_store = None;
    let mut before_write = None;
    let mut after_write_wait = None;
    for index in 0..s.records_len() {
        let r = s.record(index).unwrap();
        let local = r.invocation().local[0] as usize;
        assert!(local < 128);
        assert_eq!(r.invocation().global[0], local as u64);
        if let Some(phase) = r.phase() {
            seen[local] = true;
            if local == 0 && r.site().operation == 0 {
                if phase == Before {
                    assert_eq!(r.memory_allocation(2), None);
                } else {
                    assert_eq!(r.memory_allocation(2), Some((lds_allocation, 512)));
                    for i in 0..512 {
                        assert!(!r.memory_byte_at(2, i).unwrap().1);
                    }
                }
            }
            for (op, id, kind, pending, ready, peer) in [
                (
                    global,
                    gid,
                    Kind::PendingGlobalRead,
                    &mut pending_global,
                    &mut ready_global,
                    false,
                ),
                (
                    read,
                    rid,
                    Kind::PendingLdsRead,
                    &mut pending_lds,
                    &mut ready_lds,
                    true,
                ),
            ] {
                if r.site().operation == op && phase == Before {
                    assert!(binding(r, id).is_none());
                }
                if (r.site().operation == op && phase == After)
                    || (r.site().operation == op + 1 && phase == Before)
                {
                    let b = binding(r, id).unwrap();
                    assert_eq!(b.symbolic_kind(), Some(kind));
                    assert_eq!(b.scalar(), None);
                    assert_eq!(b.logical_pointer(), None);
                    pending[local] += 1;
                    if peer && local == 0 && phase == After {
                        pending_index = Some(index);
                    }
                }
                if r.site().operation == op + 1 && phase == After {
                    let b = binding(r, id).unwrap();
                    assert_eq!(b.value(), id);
                    assert_eq!(b.symbolic_kind(), None);
                    assert_eq!(
                        b.scalar(),
                        Some(ScalarBitsV1::u32(super::super::word(
                            if peer { local ^ 64 } else { local },
                            seed
                        )))
                    );
                    ready[local] += 1;
                    if peer && local == 0 {
                        ready_index = Some(index);
                    }
                }
            }
            if local == 0 {
                match (r.site().operation, phase) {
                    (o, Before) if o == store => before_store = Some(index),
                    (o, After) if o == store => after_store = Some(index),
                    (o, After) if o == write => before_write = Some(index),
                    (o, After) if o == write + 1 => after_write_wait = Some(index),
                    _ => {}
                }
            }
        }
        if let Some((access, allocation, offset, width, space)) = r.memory_access() {
            assert_eq!(width, 4);
            match (space, access) {
                (AddressSpace::Workgroup, WriteCommitted) => {
                    assert_eq!(allocation, lds_allocation);
                    assert_eq!(offset, local * 4);
                    assert_eq!(r.site().operation, write);
                    local_writes += 1;
                    max_write = Some(index);
                }
                (AddressSpace::Workgroup, Read) => {
                    assert_eq!(allocation, lds_allocation);
                    assert_eq!(offset, (local ^ 64) * 4);
                    assert_eq!(r.site().operation, read);
                    local_reads += 1;
                    min_read.get_or_insert(index);
                }
                (AddressSpace::Global, WriteCommitted) => {
                    assert_eq!(allocation, output_allocation);
                    assert_eq!(offset, 8 + local * 4);
                    assert_eq!(r.site().operation, store);
                    global_writes += 1;
                }
                (AddressSpace::Global, Read) => {
                    assert_eq!(r.site().operation, global);
                    assert_eq!(allocation, input_allocation);
                    assert_eq!(offset, 8 + local * 4);
                    global_reads += 1;
                }
                _ => panic!("unexpected physical profile memory access"),
            }
        }
        if let Some((action, phase, participants)) = r.barrier() {
            assert_eq!(r.site().operation, barrier);
            assert_eq!(phase, 0);
            match action {
                Arrive => {
                    assert_eq!(participants, 1);
                    assert!(!arrived[local]);
                    arrived[local] = true;
                }
                Release => {
                    assert_eq!(participants, 128);
                    assert!(arrived.iter().all(|v| *v));
                    assert!(release.replace(index).is_none());
                }
            }
        }
    }
    assert!(seen.into_iter().all(|v| v));
    assert!(arrived.into_iter().all(|v| v));
    assert_eq!(pending_global, [2; 128]);
    assert_eq!(pending_lds, [2; 128]);
    assert_eq!(ready_global, [1; 128]);
    assert_eq!(ready_lds, [1; 128]);
    assert_eq!(global_reads, 128);
    assert_eq!(local_writes, 128);
    assert_eq!(local_reads, 128);
    assert_eq!(global_writes, output.min(128));
    assert!(max_write.unwrap() < release.unwrap() && release.unwrap() < min_read.unwrap());
    let final_record = (0..s.records_len())
        .rev()
        .filter_map(|i| s.record(i))
        .find(|r| r.phase().is_some())
        .unwrap();
    assert_eq!(final_record.memory_allocation(3), None);
    assert_eq!(
        final_record.memory_address_space(2),
        Some(AddressSpace::Workgroup)
    );
    assert_eq!(final_record.memory_allocation(2).unwrap().1, 512);
    for i in 0..512 {
        assert_eq!(
            final_record.memory_byte_at(2, i),
            Some((super::super::word(i / 4, seed).to_le_bytes()[i % 4], true))
        );
    }
    for allocation in 0..2 {
        assert_eq!(
            final_record.memory_address_space(allocation),
            Some(AddressSpace::Global)
        );
        let original = &request.shared_buffers[allocation].buffer;
        assert_eq!(
            final_record.memory_allocation(allocation).unwrap().1,
            original.bytes().len()
        );
        for i in 0..original.bytes().len() {
            let written = allocation == 1 && (8..8 + 4 * output.min(128)).contains(&i);
            let expected = if written {
                super::super::word(((i - 8) / 4) ^ 64, seed).to_le_bytes()[i % 4]
            } else {
                original.bytes()[i]
            };
            assert_eq!(
                final_record.memory_byte_at(allocation, i),
                Some((
                    expected,
                    if written {
                        true
                    } else {
                        original.initialized()[i]
                    }
                ))
            );
        }
    }
    let pending = pending_index.unwrap();
    let ready = ready_index.unwrap();
    for index in [ready, pending, ready] {
        assert!(matches!(s.seek(index), Navigation::Record { .. }));
        let b = binding(s.current().unwrap(), rid).unwrap();
        assert_eq!(b.scalar().is_some(), index == ready);
        assert_eq!(b.symbolic_kind().is_some(), index == pending);
    }
    let before = before_store.unwrap();
    let after = after_store.unwrap();
    for (index, byte, init) in [
        (after, super::super::word(64, seed).to_le_bytes()[0], true),
        (before, 0xa5, false),
        (after, super::super::word(64, seed).to_le_bytes()[0], true),
    ] {
        assert!(matches!(s.seek(index), Navigation::Record { .. }));
        assert_eq!(
            s.current().unwrap().memory_byte_at(1, 8),
            Some((byte, init))
        );
    }
    assert!(matches!(s.step_reverse(), Navigation::Record { .. }));
    assert!(s.current().unwrap().is_committed_store());
    assert!(matches!(s.step_reverse(),Navigation::Record{index,..}if index==before));
    for (index, initialized) in [
        (before_write.unwrap(), false),
        (after_write_wait.unwrap(), true),
        (before_write.unwrap(), false),
    ] {
        assert!(matches!(s.seek(index), Navigation::Record { .. }));
        assert_eq!(
            s.current().unwrap().memory_byte_at(2, 0).unwrap().1,
            initialized
        );
    }
    let prior = s.cursor();
    assert_eq!(s.seek(s.records_len() + 1), Navigation::Unavailable);
    assert_eq!(s.cursor(), prior);
    let usage = s.usage();
    let records = s.records_len();
    let ledger = s.into_budget();
    assert_eq!(ledger.storage(), usage.entry_storage);
    assert_eq!(ledger.work(), usage.work);
    assert_eq!(ledger.peak_storage(), usage.peak_storage);
    assert_eq!(ledger.failed_work(), usage.failed_work);
    assert_eq!(ledger.failed_storage(), usage.failed_storage);
    (
        json!({"records":records,"logical_waves":2,"invocations":128,"output_length":output,"seed":seed,
        "global_pending_ready":128,"lds_pending_ready":128,"lds_writes":local_writes,"lds_reads":local_reads,
        "barrier_arrivals":128,"barrier_releases":1,"allocations":3,"reverse_observation_only":true,
        "storage_floor":usage.entry_storage,"work":usage.work,"peak_storage":usage.peak_storage,
        "physical_registers":"unavailable","source_variables":"unavailable","hardware_observed":false}),
        ledger,
    )
}
