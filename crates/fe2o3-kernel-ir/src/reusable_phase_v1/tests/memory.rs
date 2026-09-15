use super::*;

// Uses the old exact memory operation contracts, not a synthetic phase-memory
// event list. These KIR records are inert and do not authenticate Rust source.
pub(super) fn memory_fixture(phases: usize) -> Module {
    let (mut m, traces) = fixture(phases, 1);
    for (generation, t) in traces.iter().enumerate().rev() {
        let bind = operations(&m)[t.binds[0]].clone();
        let phase = p_mut(&mut m, t.begin).clone();
        let ReusablePhaseOperationV1::Begin { invoke, .. } = phase.operation else { panic!() };
        let Type::ExecutionCapability(initial) = bind.results[1].ty.clone() else { panic!() };
        let layout = ExecutionElementLayoutV1 { byte_size: 4, byte_alignment: 4 };
        let epoch = [210 + generation as u8; 32];
        let base = 1000 + generation as u32 * 20;
        let value = ValueId(base);
        let initialized = ValueId(base + 1);
        let published_workgroup = ValueId(base + 2);
        let published_lds = ValueId(base + 3);
        let index = ValueId(base + 4);
        let input_workgroup = p_mut(&mut m, t.binds[0]).operands[1];
        let make_type = |source, role, epoch| Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
            source_type: identity(source), role, epoch: Some(epoch), ..initial.clone()
        });
        let lds = |state| ExecutionCapabilityRoleV1::Lds { element: identity(82), layout, elements: 64, state };
        let mut init = contract(ExecutionCapabilityOperationV1::LdsInitializeByInvocation {
            input_lds: identity(99), workgroup: identity(96), output_lds: identity(111),
            element: identity(82), layout, elements: 64,
        }, &[99, 96, 82], 111, &[bind.results[1].id.0, input_workgroup.0, value.0], 201);
        init.source = call(invoke.call.callee_instance, invoke.call.callee_instance + 30).source;
        init.workgroup_brand = initial.workgroup_brand;
        init.epoch_before = initial.epoch;
        init.epoch_after = None;
        assert!(init.is_complete(), "exact existing initialize contract");
        let mut publish = contract(ExecutionCapabilityOperationV1::LdsPublish {
            input_workgroup: identity(96), input_lds: identity(111), output_lds: identity(112),
            transition: identity(110), element: identity(82), layout, elements: 64,
        }, &[96, 111], 110, &[input_workgroup.0, initialized.0], 202);
        publish.source = call(invoke.call.callee_instance, invoke.call.callee_instance + 31).source;
        publish.workgroup_brand = initial.workgroup_brand;
        publish.epoch_before = initial.epoch;
        publish.epoch_after = Some(epoch);
        assert!(publish.is_complete(), "exact existing publish contract");
        let mut read = contract(ExecutionCapabilityOperationV1::LdsReadPublished {
            lds_reference: identity(113), lds: identity(112), workgroup: identity(110),
            index: identity(114), option: identity(115), element: identity(82), layout, elements: 64,
        }, &[113, 110, 114], 115, &[published_lds.0, published_workgroup.0, index.0], 203);
        read.source = call(invoke.call.callee_instance, invoke.call.callee_instance + 32).source;
        read.workgroup_brand = initial.workgroup_brand;
        read.epoch_before = Some(epoch);
        read.epoch_after = None;
        assert!(read.is_complete(), "exact existing read contract");
        let memory = vec![
            Operation::effect_free(ValueDef::new(value, Type::F32), OperationKind::Constant(Constant::F32Bits(1f32.to_bits()))),
            Operation::effect_free(ValueDef::new(initialized, make_type(111, lds(ExecutionLdsStateV1::InvocationInitialized), initial.epoch.unwrap())), OperationKind::ExecutionCapability(init)),
            Operation::new(vec![
                ValueDef::new(published_workgroup, make_type(110, ExecutionCapabilityRoleV1::Workgroup, epoch)),
                ValueDef::new(published_lds, make_type(112, lds(ExecutionLdsStateV1::Published), epoch)),
            ], OperationKind::ExecutionCapability(publish)),
            Operation::effect_free(ValueDef::new(index, Type::INDEX), OperationKind::Constant(Constant::Index(0))),
            Operation::new(vec![ValueDef::new(ValueId(base + 5), Type::F32), ValueDef::new(ValueId(base + 6), Type::BOOL)], OperationKind::ExecutionCapability(read)),
        ];
        let close = p_mut(&mut m, t.closes[0]);
        close.operation = ReusablePhaseOperationV1::CloseStorage { last_lease: identity(112) };
        close.operands[1] = published_lds;
        let barrier = contract_mut(&mut m, t.barrier);
        let ExecutionCapabilityOperationV1::WorkgroupBarrier { input_workgroup, .. } = &mut barrier.operation else { panic!() };
        *input_workgroup = identity(110);
        barrier.signature = ExecutionCapabilitySignatureV1::new(&[identity(110)], identity(100)).unwrap();
        barrier.operands[0] = published_workgroup;
        barrier.epoch_before = Some(epoch);
        let seal = p_mut(&mut m, t.seal);
        let ReusablePhaseOperationV1::Seal { workgroup_before_barrier, .. } = &mut seal.operation else { panic!() };
        *workgroup_before_barrier = identity(110);
        let PhaseOperationSourceV1::Defined(source) = &mut seal.source else { panic!() };
        source.signature = ExecutionCapabilitySignatureV1::new(&[identity(110)], identity(101)).unwrap();
        operations_mut(&mut m).splice(t.closes[0]..t.closes[0], memory);
    }
    m.required_capabilities = m.derived_capabilities();
    m.functions[0].required_capabilities = m.required_capabilities.clone();
    m.kernels[0].required_capabilities = m.required_capabilities.clone();
    m
}

fn read_index(m: &Module) -> usize {
    operations(m).iter().position(|o| matches!(&o.kind, OperationKind::ExecutionCapability(e)
        if matches!(e.operation, ExecutionCapabilityOperationV1::LdsReadPublished { .. }))).unwrap()
}

fn read_loop() -> Module {
    let mut m = memory_fixture(1);
    let at = read_index(&m);
    let mut tail = operations_mut(&mut m).split_off(at);
    let read = tail.remove(0);
    operations_mut(&mut m).push(Operation::effect_free(ValueDef::new(ValueId(2000), Type::BOOL), OperationKind::Constant(Constant::Bool(true))));
    let body = m.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch { target: BlockId(1), arguments: vec![] });
    let mut condition = BasicBlock::new(BlockId(1));
    condition.terminator = Some(Terminator::ConditionalBranch { condition: ValueId(2000),
        then_target: BlockId(2), then_arguments: vec![], else_target: BlockId(3), else_arguments: vec![] });
    let mut reading = BasicBlock::new(BlockId(2));
    reading.operations.push(read);
    reading.terminator = Some(Terminator::Branch { target: BlockId(1), arguments: vec![] });
    let mut finishing = BasicBlock::new(BlockId(3));
    finishing.operations = tail;
    finishing.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.extend([condition, reading, finishing]);
    remap_barrier_sites(&mut m);
    m
}

#[test]
fn phase_memory_two_phases_preserve_one_allocation_and_two_barriers_each() {
    let m = memory_fixture(2);
    assert_eq!(checked(&m).map(|_| ()), Ok(()));
    verify_module(&m).unwrap();
    let ops = operations(&m);
    assert_eq!(ops.iter().filter(|o| matches!(&o.kind, OperationKind::ExecutionCapability(e)
        if matches!(e.operation, ExecutionCapabilityOperationV1::LdsAllocate { .. }))).count(), 1);
    assert_eq!(ops.iter().filter(|o| matches!(&o.kind, OperationKind::ExecutionCapability(e)
        if matches!(e.operation, ExecutionCapabilityOperationV1::LdsPublish { .. } | ExecutionCapabilityOperationV1::WorkgroupBarrier { .. }))).count(), 4);
    assert!(encode_module_v13(&m).is_err());
}

#[test]
fn phase_memory_shared_read_loop_preserves_live_handles_on_all_backedges() {
    let m = read_loop();
    assert_eq!(checked(&m).map(|_| ()), Ok(()));
    verify_module(&m).unwrap();
}

#[test]
fn phase_memory_read_after_close_backedge_rejects() {
    let mut m = read_loop();
    m.functions[0].body.as_mut().unwrap().blocks[3].terminator = Some(Terminator::Branch { target: BlockId(2), arguments: vec![] });
    assert_eq!(checked(&m), Err(ReusablePhaseCheckErrorV1::ConsumedValue));
}

#[test]
fn phase_memory_loop_exit_cannot_bypass_close_and_end() {
    let mut m = read_loop();
    let body = m.functions[0].body.as_mut().unwrap();
    body.blocks[2].terminator = Some(Terminator::ConditionalBranch { condition: ValueId(2000), then_target: BlockId(1), then_arguments: vec![], else_target: BlockId(4), else_arguments: vec![] });
    let mut exit = BasicBlock::new(BlockId(4));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(exit);
    assert_eq!(checked(&m), Err(ReusablePhaseCheckErrorV1::MissingEnd));
}

#[test]
fn phase_memory_read_cannot_use_the_prepublication_epoch() {
    let mut m = memory_fixture(1);
    let at = read_index(&m);
    contract_mut(&mut m, at).epoch_before = Some([170; 32]);
    assert_eq!(checked(&m), Err(ReusablePhaseCheckErrorV1::LocalContract));
}

#[test]
fn phase_memory_read_cannot_substitute_prepublication_handle() {
    let mut m = memory_fixture(1);
    let at = read_index(&m);
    let original = operations(&m).iter().find_map(|o| match &o.kind {
        OperationKind::ReusablePhase(p) if matches!(p.operation, ReusablePhaseOperationV1::Bind { .. }) => Some(o.results[1].id), _ => None,
    }).unwrap();
    contract_mut(&mut m, at).operands[0] = original;
    assert_eq!(checked(&m), Err(ReusablePhaseCheckErrorV1::LocalContract));
}

#[test]
fn phase_memory_operation_cannot_substitute_expanded_root() {
    let mut m = memory_fixture(1);
    let at = read_index(&m);
    let source = &mut contract_mut(&mut m, at).source;
    let old = source.occurrence.unwrap();
    source.occurrence = ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
        old.root_source_identity(), old.expansion_identity(), [1; 32], old.caller_instance(), old.expanded_block());
    assert_eq!(checked(&m), Err(ReusablePhaseCheckErrorV1::WrongOccurrence));
}

#[test]
fn phase_memory_all_edge_checks_share_the_existing_budget() {
    let m = read_loop();
    let usage = checked(&m).unwrap();
    let f = &m.functions[0];
    let cfg = analyze_control_flow(f).unwrap();
    assert_eq!(verify_reusable_phase_function_v1(f, &cfg, ReusablePhaseCheckLimitsV1 {
        work: usage.work - 1, ..ReusablePhaseCheckLimitsV1::DEFAULT
    }), Err(ReusablePhaseCheckErrorV1::WorkLimit));
    assert_eq!(verify_reusable_phase_function_v1(f, &cfg, ReusablePhaseCheckLimitsV1 {
        temporary_bytes: usage.peak_temporary_bytes - 1, ..ReusablePhaseCheckLimitsV1::DEFAULT
    }), Err(ReusablePhaseCheckErrorV1::StorageLimit));
    assert_eq!(verify_reusable_phase_function_v1(f, &cfg, ReusablePhaseCheckLimitsV1 {
        work: usage.work, temporary_bytes: usage.peak_temporary_bytes,
    }), Ok(usage));
}

#[test]
fn phase_memory_publish_barrier_cannot_replace_finish_barrier() {
    let mut m = memory_fixture(1);
    let published_workgroup = operations(&m).iter().find_map(|o| match &o.kind {
        OperationKind::ExecutionCapability(e) if matches!(e.operation, ExecutionCapabilityOperationV1::LdsPublish { .. }) => Some(o.results[0].id),
        _ => None,
    }).unwrap();
    let finish = operations(&m).iter().position(|o| matches!(&o.kind, OperationKind::ExecutionCapability(e)
        if matches!(e.operation, ExecutionCapabilityOperationV1::WorkgroupBarrier { .. }))).unwrap();
    operations_mut(&mut m).remove(finish);
    let seal = operations_mut(&mut m).iter_mut().find_map(|o| match &mut o.kind {
        OperationKind::ReusablePhase(p) if matches!(p.operation, ReusablePhaseOperationV1::Seal { .. }) => Some(p), _ => None,
    }).unwrap();
    let ReusablePhaseOperationV1::Seal { workgroup_before_barrier, workgroup_after_barrier, .. } = &mut seal.operation else { panic!() };
    *workgroup_before_barrier = identity(96);
    *workgroup_after_barrier = identity(110);
    let PhaseOperationSourceV1::Defined(source) = &mut seal.source else { panic!() };
    source.signature = ExecutionCapabilitySignatureV1::new(&[identity(96)], identity(101)).unwrap();
    seal.operands[1] = published_workgroup;
    // Keep local output types consistent with the substituted input so the
    // negative reaches the actual missing Finish-barrier producer check.
    for result in operations_mut(&mut m).iter_mut().flat_map(|o| &mut o.results) {
        match &mut result.ty {
            Type::ExecutionCapability(c) if c.role == ExecutionCapabilityRoleV1::ReusablePhaseCompletion => c.epoch = Some([210; 32]),
            Type::ReusablePhaseToken(t) => {
                if let ReusablePhaseTokenRoleV1::CompletionReady { barrier_epoch, .. } = &mut t.role { *barrier_epoch = [210; 32]; }
            }
            _ => {}
        }
    }
    assert_eq!(checked(&m), Err(ReusablePhaseCheckErrorV1::WrongCompletion));
}

#[test]
fn phase_memory_same_lease_cannot_be_initialized_twice() {
    let mut m = memory_fixture(1);
    let at = operations(&m).iter().position(|o| matches!(&o.kind, OperationKind::ExecutionCapability(e)
        if matches!(e.operation, ExecutionCapabilityOperationV1::LdsInitializeByInvocation { .. }))).unwrap();
    let mut duplicate = operations(&m)[at].clone();
    duplicate.results[0].id = ValueId(2000);
    operations_mut(&mut m).insert(at + 1, duplicate);
    assert_eq!(checked(&m), Err(ReusablePhaseCheckErrorV1::DuplicateConsumption));
}

#[test]
fn phase_memory_later_generation_cannot_read_the_first_lease() {
    let mut m = memory_fixture(2);
    let reads = operations(&m).iter().enumerate().filter_map(|(i, o)| match &o.kind {
        OperationKind::ExecutionCapability(e) if matches!(e.operation, ExecutionCapabilityOperationV1::LdsReadPublished { .. }) => Some(i), _ => None,
    }).collect::<Vec<_>>();
    let earlier = contract_mut(&mut m, reads[0]).operands[0];
    contract_mut(&mut m, reads[1]).operands[0] = earlier;
    assert_eq!(checked(&m), Err(ReusablePhaseCheckErrorV1::WrongPhase));
}

#[test]
fn phase_memory_close_cannot_release_a_pending_async_copy() {
    let m = memory_fixture(1);
    let close = operations(&m).iter().find_map(|o| match &o.kind {
        OperationKind::ReusablePhase(p) if matches!(p.operation, ReusablePhaseOperationV1::CloseStorage { .. }) => Some(p), _ => None,
    }).unwrap();
    let mut types = close.operands.iter().map(|id| operations(&m).iter().flat_map(|o| &o.results)
        .find(|r| r.id == *id).unwrap().ty.clone()).collect::<Vec<_>>();
    let Type::ExecutionCapability(c) = &mut types[1] else { panic!() };
    let ExecutionCapabilityRoleV1::Lds { state, .. } = &mut c.role else { panic!() };
    *state = ExecutionLdsStateV1::PendingAsyncCopy;
    assert!(close.checked_result_types(&types.iter().collect::<Vec<_>>()).is_none());
}
