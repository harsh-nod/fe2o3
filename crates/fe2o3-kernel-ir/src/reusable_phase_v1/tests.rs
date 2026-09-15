use super::*;
use crate::*;
include!("../execution_capability_v1/subgroup_partition/fixture.rs");
#[path = "tests/memory.rs"]
mod memory;
#[path = "tests/canonical.rs"]
mod canonical;
#[path = "tests/physical.rs"]
mod physical;
#[path = "tests/terminal_occurrence.rs"]
mod terminal_occurrence;

// Structural KIR tests only. These source commitments do not authenticate Rust.
fn call(caller: u32, callee: u32) -> PhaseCallOccurrenceV1 {
    PhaseCallOccurrenceV1 { source: ExecutionCapabilitySourceV1 {
        function: [caller as u8 + 100; 32], operation: [callee as u8 + 100; 32], block: callee,
        occurrence: ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts([99;32],[100;32],[101;32],caller,0),
    }, callee_instance: callee, original_normal_target: callee + 1, expanded_normal_target: 0 }
}
fn terminal(caller: u32, operation: u32) -> PhaseTerminalCallOccurrenceV1 {
    PhaseTerminalCallOccurrenceV1 { source: ExecutionCapabilitySourceV1 {
        function: [caller as u8 + 100; 32], operation: [operation as u8 + 100; 32], block: operation,
        occurrence: ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts([99;32],[100;32],[101;32],caller,0),
    }, original_normal_target: operation + 1, expanded_normal_target: 0 }
}
// CFG fixtures that relocate operations must also relocate their source sites.
fn remap_barrier_sites(m: &mut Module) {
    let body = m.functions[0].body.as_mut().unwrap();
    let mut remapped = Vec::new();
    for block in &mut body.blocks {
        for operation in &mut block.operations {
            let OperationKind::ExecutionCapability(e) = &mut operation.kind else { continue; };
            if !matches!(e.operation, ExecutionCapabilityOperationV1::WorkgroupBarrier { .. }) { continue; }
            let old = e.source;
            let o = old.occurrence.unwrap();
            e.source.occurrence = ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                o.root_source_identity(), o.expansion_identity(), o.expanded_root_identity(), o.caller_instance(), block.id.0);
            remapped.push((old, e.source));
        }
    }
    for block in &mut body.blocks {
        for operation in &mut block.operations {
            let OperationKind::ReusablePhase(p) = &mut operation.kind else { continue; };
            let ReusablePhaseOperationV1::Seal { barrier_call, .. } = &mut p.operation else { continue; };
            let matches = remapped.iter().filter(|(old, _)| *old == barrier_call.source).collect::<Vec<_>>();
            assert_eq!(matches.len(), 1);
            barrier_call.source = matches[0].1;
            barrier_call.expanded_normal_target = block.id.0;
        }
    }
}
fn defined(caller: u32, callee: u32, args: &[u8], output: u8) -> PhaseDefinedCallV1 {
    PhaseDefinedCallV1 { call: call(caller,callee), signature: ExecutionCapabilitySignatureV1::new(
        &args.iter().copied().map(identity).collect::<Vec<_>>(),identity(output)).unwrap(),
        defined_abi:[71;32],defined_body:[72;32],incoming_count:2,incoming_digest:[73;32],source_binding:[74;32] }
}
fn append(m: &mut Module, operation: ReusablePhaseOperationV1, source: PhaseOperationSourceV1,
    operands: &[ValueId], next: &mut u32) -> Vec<ValueId> {
    let p = ReusablePhaseOpV1 { operands:operands.to_vec(), obligations:Obligations::from_bits(operation.required_obligations()),
        operation, provenance:provenance(),source };
    let input = operands.iter().map(|id| (*id,&operations(m).iter().flat_map(|o| &o.results)
        .find(|r| r.id == *id).unwrap().ty)).collect::<Vec<_>>();
    let (operation,after) = p.checked_operation(&input,ValueId(*next)).expect("fixture bounded emission");
    *next=after.0;
    let ids = operation.results.iter().map(|v|v.id).collect();
    operations_mut(m).push(operation);
    ids
}
#[derive(Clone)]
struct Trace { begin:usize, binds:Vec<usize>, closes:Vec<usize>, barrier:usize,
    seal:usize, relay:usize, drop:usize, end:usize }
fn p_mut(m:&mut Module, index:usize)->&mut ReusablePhaseOpV1 {
    let OperationKind::ReusablePhase(p)=&mut operations_mut(m)[index].kind else { panic!("phase op"); }; p
}
fn fixture(phases:usize, stores:usize)->(Module,Vec<Trace>) {
    let mut m=module(); operations_mut(&mut m).truncate(2);
    let mut storage=Vec::new();
    let layout=ExecutionElementLayoutV1 { byte_size:4,byte_alignment:4 };
    for i in 0..stores {
        let raw=80+i as u32*2; let reusable=raw+1;
        let alloc=contract(ExecutionCapabilityOperationV1::LdsAllocate {
            workgroup:identity(11),lds:identity(80),element:identity(82),layout,elements:64 },&[11],80,&[1],80);
        operations_mut(&mut m).push(Operation::effect_free(ValueDef::new(ValueId(raw),capability(80,
            ExecutionCapabilityRoleV1::Lds { element:identity(82),layout,elements:64,state:ExecutionLdsStateV1::Uninitialized })),OperationKind::ExecutionCapability(alloc)));
        let conv=ReusableLdsConversionV1 { input:identity(80),output:identity(81),element:identity(82),layout,elements:64,
            defined_function:[93;32],defined_abi:[94;32],defined_body:[95;32],source_binding:[96;32] };
        let mut c=contract(ExecutionCapabilityOperationV1::ReusableLdsConversion(conv),&[80],81,&[raw],93);
        c.source.occurrence=call(1,2+i as u32).source.occurrence;
        operations_mut(&mut m).push(Operation::effect_free(ValueDef::new(ValueId(reusable),capability(81,
            ExecutionCapabilityRoleV1::ReusableLds { element:identity(82),layout,elements:64 })),OperationKind::ExecutionCapability(c)));
        storage.push(ValueId(reusable));
    }
    let mut next=200;
    let mut owner=append(&mut m,ReusablePhaseOperationV1::OwnerConvert {workgroup:identity(11),owner:identity(90)},
        PhaseOperationSourceV1::Defined(defined(1,8,&[11],90)),&[ValueId(1)],&mut next)[0];
    let mut traces=Vec::new();
    for generation in 0..phases {
        let instance=20+generation as u32*20;
        let wrapper=defined(1,instance,&[91,92],93);
        let invoke=defined(instance,instance+2,&[92,94],95);
        let issue=defined(instance,instance+1,&[91],96);
        let key=PhaseKeyV1::for_begin(issue.call).unwrap();
        let protocol=[150+generation as u8;32];
        let brand=[160+generation as u8;32];
        let epoch=[170+generation as u8;32];
        let advanced=[180+generation as u8;32];
        let begin=operations(&m).len();
        let issued=append(&mut m,ReusablePhaseOperationV1::Begin {owner_reference:identity(91),owner:identity(90),
            phase_workgroup:identity(96),outer_brand:[7;32],phase_brand:brand,dynamic_epoch:epoch,wrapper,invoke,source_protocol:protocol},
            PhaseOperationSourceV1::Defined(issue),&[owner],&mut next);
        let mut cursor=issued[1]; let mut closed=Vec::new(); let mut binds=Vec::new(); let mut closes=Vec::new();
        for (i,s) in storage.iter().enumerate() {
            let bind=defined(instance+2,instance+3+i as u32,&[97,98],99);
            binds.push(operations(&m).len());
            let lease=append(&mut m,ReusablePhaseOperationV1::Bind {phase_reference:identity(97),storage_reference:identity(98),
                phase_workgroup:identity(96),reusable_storage:identity(81),phase_lds:identity(99),element:identity(82),layout,elements:64},
                PhaseOperationSourceV1::Defined(bind),&[cursor,issued[0],*s],&mut next);
            cursor=lease[0]; closes.push(operations(&m).len());
            closed.push(append(&mut m,ReusablePhaseOperationV1::CloseStorage {last_lease:identity(99)},
                PhaseOperationSourceV1::LeaseEnd {phase:key,bind:bind.call,source_event:PhaseLifetimeEndV1 {
                    function:invoke.call.source.operation,instance:instance+2,original_block:0,original_position:PhaseSourcePositionV1::Statement(9+i as u32),
                    expanded_block:0,expanded_position:PhaseSourcePositionV1::Statement(9+i as u32)},source_protocol:protocol},
                &[lease[2],lease[1]],&mut next)[0]);
        }
        let finish=defined(instance+2,instance+12,&[96],101);
        let barrier_call=terminal(instance+12,instance+13);
        let mut barrier=contract(ExecutionCapabilityOperationV1::WorkgroupBarrier {input_workgroup:identity(96),output_workgroup:identity(100),
            semantics:ExecutionMemorySemanticsV1 {scope:ExecutionMemoryScopeV1::Workgroup,ordering:ExecutionMemoryOrderingV1::AcquireRelease,
                spaces:ExecutionMemorySpacesV1::Workgroup}},&[96],100,&[issued[0].0],130);
        barrier.source=barrier_call.source; barrier.workgroup_brand=Some(brand);barrier.epoch_before=Some(epoch);barrier.epoch_after=Some(advanced);
        let barrier_index=operations(&m).len(); let result=ValueId(next);next+=1;
        operations_mut(&mut m).push(Operation::effect_free(ValueDef::new(result,Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
            source_type:identity(100),provenance:provenance(),workgroup_brand:Some(brand),epoch:Some(advanced),role:ExecutionCapabilityRoleV1::Workgroup})),
            OperationKind::ExecutionCapability(barrier)));
        let seal=operations(&m).len();
        let sealed=append(&mut m,ReusablePhaseOperationV1::Seal {workgroup_before_barrier:identity(96),workgroup_after_barrier:identity(100),
            completion:identity(101),barrier_call},PhaseOperationSourceV1::Defined(finish),&[cursor,result],&mut next);
        let relay=operations(&m).len();
        let carried=append(&mut m,ReusablePhaseOperationV1::RelayClosure {completion:identity(101)},PhaseOperationSourceV1::ClosureReturn {
            phase:key,closure:invoke,pack_block:0,pack_statement:10,return_block:0,source_protocol:protocol},&[sealed[1]],&mut next);
        let drop=operations(&m).len();
        let ready=append(&mut m,ReusablePhaseOperationV1::RelayDrop {completion:identity(101)},PhaseOperationSourceV1::WrapperDrop {
            phase:key,drop_call:PhaseDropOccurrenceV1::CallEntry(call(instance,instance+14)),drop_abi:[201;32],source_protocol:protocol},&[sealed[0],carried[0]],&mut next);
        let end=operations(&m).len();
        let inputs=ready.iter().chain(&closed).copied().collect::<Vec<_>>();
        let restored=append(&mut m,ReusablePhaseOperationV1::End {storage_count:stores as u8},PhaseOperationSourceV1::WrapperEnd {
            phase:key,wrapper_normal_target:wrapper.call.expanded_normal_target,source_protocol:protocol},&inputs,&mut next);
        owner=restored[0]; storage=restored[1..].to_vec();
        traces.push(Trace {begin,binds,closes,barrier:barrier_index,seal,relay,drop,end});
    }
    for (i,op) in operations_mut(&mut m).iter_mut().enumerate() {
        if let OperationKind::ExecutionCapability(e)=&mut op.kind {
            if !matches!(e.operation,ExecutionCapabilityOperationV1::WorkgroupBarrier { .. }) {
                e.source.function=[101;32];e.source.block=i as u32;
                e.source.occurrence=call(1,2).source.occurrence;
            }
        }
    }
    m.required_capabilities=m.derived_capabilities();
    m.functions[0].required_capabilities=m.required_capabilities.clone();
    m.kernels[0].required_capabilities=m.required_capabilities.clone();
    (m,traces)
}
fn checked(m:&Module)->Result<ReusablePhaseCheckUsageV1,ReusablePhaseCheckErrorV1> {
    let f=&m.functions[0];let cfg=analyze_control_flow(f).unwrap();
    verify_reusable_phase_function_v1(f,&cfg,ReusablePhaseCheckLimitsV1::DEFAULT)
}

#[test]
fn phase_two_generations_restore_the_same_owner_and_allocation() {
    let (m,t)=fixture(2,1);
    assert!(checked(&m).is_ok(),"{:?}",checked(&m));
    verify_module(&m).unwrap();
    assert_eq!(operation_contract(&operations(&m)[t[0].barrier]).operation.memory_effects().len(),1);
    assert_eq!(operations(&m).iter().filter(|o| matches!(o.kind,OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
        operation:ExecutionCapabilityOperationV1::LdsAllocate { .. }, .. }))).count(),1);
    assert_eq!(p_mut(&mut m.clone(),t[1].begin).operands[0],operations(&m)[t[0].end].results[0].id);
    let Type::ExecutionCapability(storage)=&operations(&m)[t[1].end].results[1].ty else {panic!()};
    assert_eq!(storage.epoch,Some([8;32])); assert_eq!(storage.workgroup_brand,Some([7;32]));
}
#[test]
fn phase_multiple_binds_restore_in_exact_cursor_order() {
    let (m,_)=fixture(2,3); checked(&m).unwrap();verify_module(&m).unwrap();
    let (mut m,t)=fixture(1,2); p_mut(&mut m,t[0].end).operands.swap(2,3);
    assert_eq!(checked(&m),Err(ReusablePhaseCheckErrorV1::WrongStorage));
}
#[test]
fn phase_cannot_bind_same_allocation_twice_or_reuse_before_end() {
    let (mut m,t)=fixture(1,2);
    let original=p_mut(&mut m,t[0].binds[0]).operands[2];p_mut(&mut m,t[0].binds[1]).operands[2]=original;
    assert_eq!(checked(&m),Err(ReusablePhaseCheckErrorV1::DuplicateConsumption));
    let (mut m,t)=fixture(2,1);
    let original=p_mut(&mut m,t[0].begin).operands[0];p_mut(&mut m,t[1].begin).operands[0]=original;
    assert_eq!(checked(&m),Err(ReusablePhaseCheckErrorV1::DuplicateConsumption));
}
#[test]
fn phase_completion_must_pass_both_relays() {
    let (mut m,t)=fixture(1,1);
    let completion=operations(&m)[t[0].seal].results[1].id;p_mut(&mut m,t[0].drop).operands[1]=completion;
    assert_eq!(checked(&m),Err(ReusablePhaseCheckErrorV1::WrongCompletion));
}
#[test]
fn phase_close_cannot_substitute_same_typed_other_lease() {
    let (mut m,t)=fixture(1,2);
    let other=operations(&m)[t[0].binds[0]].results[1].id;p_mut(&mut m,t[0].closes[1]).operands[1]=other;
    assert_eq!(checked(&m),Err(ReusablePhaseCheckErrorV1::WrongStorage));
}
#[test]
fn phase_missing_close_or_end_is_not_a_dormant_waiver() {
    let (mut m,t)=fixture(1,1);operations_mut(&mut m).remove(t[0].end);
    assert_eq!(checked(&m),Err(ReusablePhaseCheckErrorV1::MissingEnd));
    let (mut m,t)=fixture(1,1);operations_mut(&mut m).remove(t[0].closes[0]);
    assert_eq!(checked(&m),Err(ReusablePhaseCheckErrorV1::MissingProducer));
}
#[test]
fn phase_occurrence_protocol_and_obligation_substitutions_reject() {
    for mutation in 0..5 {
        let (mut m,t)=fixture(1,1);
        match mutation {
            0=>p_mut(&mut m,t[0].begin).obligations=Obligations::from_bits(0),
            1=>{let PhaseOperationSourceV1::ClosureReturn {source_protocol,..}=&mut p_mut(&mut m,t[0].relay).source else {panic!()};*source_protocol=[222;32];},
            2=>{let PhaseOperationSourceV1::Defined(d)=&mut p_mut(&mut m,t[0].binds[0]).source else {panic!()};d.call.source.occurrence=call(8,9).source.occurrence;},
            3=>{let PhaseOperationSourceV1::WrapperEnd {wrapper_normal_target,..}=&mut p_mut(&mut m,t[0].end).source else {panic!()};*wrapper_normal_target=88;},
            4=>contract_mut(&mut m,t[0].barrier).source.operation=[234;32],
            _=>unreachable!(),
        }
        let expected=match mutation {0=>ReusablePhaseCheckErrorV1::LocalContract,4=>ReusablePhaseCheckErrorV1::WrongCompletion,_=>ReusablePhaseCheckErrorV1::WrongOccurrence};
        assert_eq!(checked(&m),Err(expected),"mutation {mutation}");
    }
}
#[test]
fn phase_active_tokens_cannot_escape_through_ordinary_operations() {
    let (mut m,t)=fixture(1,1);let token=operations(&m)[t[0].begin].results[1].id;
    operations_mut(&mut m).insert(t[0].begin+1,Operation::new(vec![],OperationKind::Call {callee:FunctionId::new("other"),arguments:vec![token]}));
    assert_eq!(checked(&m),Err(ReusablePhaseCheckErrorV1::UnsupportedPhaseConsumer));
}
#[test]
fn phase_work_and_actual_temporary_peak_are_enforced_without_limit_increases() {
    let (m,_)=fixture(2,2);let f=&m.functions[0];let cfg=analyze_control_flow(f).unwrap();
    let usage=checked(&m).unwrap();
    assert!(usage.work>0 && usage.peak_temporary_bytes>0);
    assert_eq!(verify_reusable_phase_function_v1(f,&cfg,ReusablePhaseCheckLimitsV1 {work:usage.work-1,..ReusablePhaseCheckLimitsV1::DEFAULT}),Err(ReusablePhaseCheckErrorV1::WorkLimit));
    assert_eq!(verify_reusable_phase_function_v1(f,&cfg,ReusablePhaseCheckLimitsV1 {temporary_bytes:usage.peak_temporary_bytes-1,..ReusablePhaseCheckLimitsV1::DEFAULT}),Err(ReusablePhaseCheckErrorV1::StorageLimit));
    assert_eq!(verify_reusable_phase_function_v1(f,&cfg,ReusablePhaseCheckLimitsV1 {work:usage.work,temporary_bytes:usage.peak_temporary_bytes}).unwrap(),usage);
}
#[test]
fn phase_cannot_be_encoded_as_old_canonical_thirteen() {
    let (m,_)=fixture(1,1);assert!(encode_module_v13(&m).is_err());
    let old=module(); let bytes=encode_module_v13(&old).unwrap();
    assert_eq!(decode_module_v13(&bytes).unwrap(),old);
    assert_eq!(encode_module_v13(&decode_module_v13(&bytes).unwrap()).unwrap(),bytes);
}

#[test]
fn phase_emission_rejects_substituted_input_id_and_overflow_before_emitting() {
    let (m,t)=fixture(1,1);
    let OperationKind::ReusablePhase(p)=&operations(&m)[t[0].begin].kind else {panic!()};
    let id=p.operands[0];let ty=&operations(&m).iter().flat_map(|o|&o.results).find(|r|r.id==id).unwrap().ty;
    assert!(p.clone().checked_operation(&[(ValueId(id.0+1),ty)],ValueId(900)).is_none());
    assert!(p.clone().checked_operation(&[(id,ty)],id).is_none());
    assert!(p.clone().checked_operation(&[(id,ty)],ValueId(u32::MAX)).is_none());
    let (operation,next)=p.clone().checked_operation(&[(id,ty)],ValueId(900)).unwrap();
    assert_eq!(next,ValueId(902));assert_eq!(operation.results.iter().map(|r|r.id).collect::<Vec<_>>(),[ValueId(900),ValueId(901)]);
}

#[test]
fn phase_end_cannot_be_bypassed_on_an_actual_cfg_path() {
    let (mut m,t)=fixture(1,1);
    let end=operations_mut(&mut m).remove(t[0].end);
    operations_mut(&mut m).push(Operation::effect_free(ValueDef::new(ValueId(900),Type::BOOL),OperationKind::Constant(Constant::Bool(true))));
    let body=m.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator=Some(Terminator::ConditionalBranch {condition:ValueId(900),then_target:BlockId(1),then_arguments:vec![],else_target:BlockId(2),else_arguments:vec![]});
    let mut closed=BasicBlock::new(BlockId(1));closed.operations.push(end);closed.terminator=Some(Terminator::Return {values:vec![]});
    let mut bypass=BasicBlock::new(BlockId(2));bypass.terminator=Some(Terminator::Return {values:vec![]});
    body.blocks.extend([closed,bypass]);
    assert_eq!(checked(&m),Err(ReusablePhaseCheckErrorV1::MissingEnd));
}

#[test]
fn phase_generation_inside_a_cfg_loop_remains_explicitly_unsupported() {
    let (mut m,_)=fixture(1,1);
    m.functions[0].body.as_mut().unwrap().blocks[0].terminator=Some(Terminator::Branch {target:BlockId(0),arguments:vec![]});
    assert_eq!(checked(&m),Err(ReusablePhaseCheckErrorV1::UnsupportedCycle));
}

#[test]
fn phase_kahn_scheduler_is_independent_of_canonical_block_number_order() {
    let (mut m,t)=fixture(2,1);
    let later=operations_mut(&mut m).split_off(t[1].begin);
    let body=m.functions[0].body.as_mut().unwrap();body.blocks[0].id=BlockId(90);
    body.blocks[0].terminator=Some(Terminator::Branch {target:BlockId(2),arguments:vec![]});
    let mut tail=BasicBlock::new(BlockId(2));tail.operations=later;tail.terminator=Some(Terminator::Return {values:vec![]});body.blocks.push(tail);
    remap_barrier_sites(&mut m);
    checked(&m).unwrap();
}

