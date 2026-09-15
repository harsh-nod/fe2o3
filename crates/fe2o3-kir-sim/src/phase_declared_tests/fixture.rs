// Mechanically extracted existing KIR fixtures. Commitments here are inert.
use fe2o3_kernel_ir::*;
use fe2o3_kernel_ir::ExecutionSafetyObligationsV1 as Obligations;
fn identity(tag: u8) -> ExecutionTypeIdentityV1 {
    ExecutionTypeIdentityV1::new([tag; 32])
}

fn provenance() -> ExecutionCapabilityProvenanceV1 {
    ExecutionCapabilityProvenanceV1 {
        root: FunctionId::new("partition_entry"),
        kernel_binding: [1; 32],
        frontend_unit: [2; 32],
        kernel_marker: [3; 32],
        target_brand: [4; 32],
        launch_brand: [5; 32],
        issuance: [6; 32],
    }
}

fn contract(
    operation: ExecutionCapabilityOperationV1,
    arguments: &[u8],
    output: u8,
    operands: &[u32],
    source: u8,
) -> ExecutionCapabilityOpV1 {
    ExecutionCapabilityOpV1 {
        signature: ExecutionCapabilitySignatureV1::new(
            &arguments.iter().copied().map(identity).collect::<Vec<_>>(),
            identity(output),
        )
        .unwrap(),
        operands: operands.iter().copied().map(ValueId).collect(),
        provenance: provenance(),
        workgroup_brand: Some([7; 32]),
        epoch_before: Some([8; 32]),
        epoch_after: None,
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &operation,
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [9; 32],
            operation: [source; 32],
            block: 0,
            occurrence: None,
        },
        operation,
    }
}

fn capability(source: u8, role: ExecutionCapabilityRoleV1) -> Type {
    Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type: identity(source),
        provenance: provenance(),
        workgroup_brand: Some([7; 32]),
        epoch: Some([8; 32]),
        role,
    })
}

fn module() -> Module {
    use ExecutionCapabilityOperationV1 as E;
    use ExecutionCapabilityRoleV1 as R;
    use SubgroupPartitionOperationV1 as P;
    let subgroup = contract(
        E::SubgroupDerive {
            workgroup: identity(11),
            subgroup: identity(12),
            width: 64,
        },
        &[11],
        12,
        &[1],
        31,
    );
    let derive = contract(
        E::SubgroupPartition(P::Derive {
            subgroup_reference: identity(13),
            subgroup: identity(12),
            epoch: identity(14),
            partition: identity(15),
            width: 64,
            partition_width: 16,
        }),
        &[13, 14],
        15,
        &[2, 1],
        32,
    );
    let reduce = contract(
        E::SubgroupPartition(P::ReduceSumF32 {
            partition_reference: identity(16),
            partition: identity(15),
            element: identity(17),
            width: 64,
            partition_width: 16,
        }),
        &[16, 17],
        17,
        &[3, 4],
        33,
    );
    let broadcast = contract(
        E::SubgroupPartition(P::BroadcastF32 {
            partition_reference: identity(16),
            partition: identity(15),
            element: identity(17),
            source_lane: identity(18),
            width: 64,
            partition_width: 16,
        }),
        &[16, 17, 18],
        17,
        &[3, 6, 5],
        34,
    );
    let workgroup = contract(
        E::WorkgroupDerive {
            context: identity(10),
            workgroup: identity(11),
        },
        &[10],
        11,
        &[0],
        30,
    );
    let p = provenance();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(0),
            KernelContextTypeV1::new(
                "partition_entry",
                p.kernel_marker,
                p.target_brand,
                p.launch_brand,
            ),
            KernelContextSourceIdentityV1::new([40; 32], [41; 32], [42; 32], [43; 32]),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(1), capability(11, R::Workgroup)),
            OperationKind::ExecutionCapability(workgroup),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), capability(12, R::Subgroup { width: 64 })),
            OperationKind::ExecutionCapability(subgroup),
        ),
        Operation::effect_free(
            ValueDef::new(
                ValueId(3),
                capability(
                    15,
                    R::SubgroupPartition {
                        width: 64,
                        partition_width: 16,
                    },
                ),
            ),
            OperationKind::ExecutionCapability(derive),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::Scalar(ScalarType::F32)),
            OperationKind::Constant(Constant::F32Bits(1.0_f32.to_bits())),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(15)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(6), Type::Scalar(ScalarType::F32)),
            OperationKind::ExecutionCapability(reduce),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(7), Type::Scalar(ScalarType::F32)),
            OperationKind::ExecutionCapability(broadcast),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let requirements = block
        .operations
        .iter()
        .flat_map(Operation::required_capabilities)
        .collect();
    let mut function = Function::kernel_entry(
        "partition_entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    function.required_capabilities = requirements;
    let mut module = Module::new("partition");
    module.required_capabilities = function.required_capabilities.clone();
    let mut kernel = Kernel::new(
        "partition",
        "partition_entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities = function.required_capabilities.clone();
    module.kernels.push(kernel);
    module.functions.push(function);
    module
}

fn operations(module: &Module) -> &[Operation] {
    &module.functions[0].body.as_ref().unwrap().blocks[0].operations
}

fn operations_mut(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}

fn operation_contract(operation: &Operation) -> &ExecutionCapabilityOpV1 {
    let OperationKind::ExecutionCapability(contract) = &operation.kind else {
        panic!("execution contract")
    };
    contract
}

fn contract_mut(module: &mut Module, index: usize) -> &mut ExecutionCapabilityOpV1 {
    let OperationKind::ExecutionCapability(contract) = &mut operations_mut(module)[index].kind
    else {
        panic!("execution contract")
    };
    contract
}
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

pub(super) fn memory(n:usize) -> Module {memory_fixture(n)}
pub(super) fn terminal_drops(mut module: Module) -> Module {
    for operation in operations_mut(&mut module) {
        let OperationKind::ReusablePhase(p) = &mut operation.kind else { continue; };
        let PhaseOperationSourceV1::WrapperDrop { drop_call, .. } = &mut p.source else { continue; };
        let PhaseDropOccurrenceV1::CallEntry(call) = *drop_call else { panic!() };
        *drop_call = PhaseDropOccurrenceV1::Source(PhaseTerminalCallOccurrenceV1 {
            source: call.source, original_normal_target: call.original_normal_target,
            expanded_normal_target: call.expanded_normal_target,
        });
    }
    module
}
pub(super) fn without_memory() -> Module {fixture(1,1).0}
mod legacy {
use fe2o3_kernel_ir::*;
use std::collections::BTreeSet;
fn identity(byte: u8) -> ExecutionTypeIdentityV1 {
    ExecutionTypeIdentityV1::new([byte; 32])
}

fn provenance() -> ExecutionCapabilityProvenanceV1 {
    ExecutionCapabilityProvenanceV1 {
        root: FunctionId::new("entry"),
        kernel_binding: [1; 32],
        frontend_unit: [2; 32],
        kernel_marker: [3; 32],
        target_brand: [4; 32],
        launch_brand: [5; 32],
        issuance: [6; 32],
    }
}

fn context_type() -> KernelContextTypeV1 {
    let provenance = provenance();
    KernelContextTypeV1::new(
        "entry",
        provenance.kernel_marker,
        provenance.target_brand,
        provenance.launch_brand,
    )
}

fn capability_type(
    source_type: ExecutionTypeIdentityV1,
    workgroup_brand: [u8; 32],
    epoch: [u8; 32],
    role: ExecutionCapabilityRoleV1,
) -> Type {
    Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type,
        provenance: provenance(),
        workgroup_brand: Some(workgroup_brand),
        epoch: Some(epoch),
        role,
    })
}

fn workgroup_derivation(
    context: ValueId,
    result: ValueId,
    workgroup: ExecutionTypeIdentityV1,
    workgroup_brand: [u8; 32],
    epoch: [u8; 32],
    source: u8,
) -> Operation {
    let operation = ExecutionCapabilityOperationV1::WorkgroupDerive {
        context: identity(0xf0),
        workgroup,
    };
    Operation::new(
        vec![ValueDef::new(
            result,
            capability_type(
                workgroup,
                workgroup_brand,
                epoch,
                ExecutionCapabilityRoleV1::Workgroup,
            ),
        )],
        OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
            operands: vec![context],
            signature: ExecutionCapabilitySignatureV1::new(&[identity(0xf0)], workgroup).unwrap(),
            provenance: provenance(),
            workgroup_brand: Some(workgroup_brand),
            epoch_before: Some(epoch),
            epoch_after: None,
            obligations: ExecutionSafetyObligationsV1::from_bits(
                required_execution_obligations_v1(&operation),
            ),
            source: ExecutionCapabilitySourceV1 {
                function: [0xf1; 32],
                operation: [source; 32],
                block: 0,
                occurrence: None,
            },
            operation,
        }),
    )
}

// The fixture keeps each canonical carrier field explicit at call sites.
#[allow(clippy::too_many_arguments)]
fn workgroup_contract(
    operation: ExecutionCapabilityOperationV1,
    operands: Vec<ValueId>,
    arguments: &[ExecutionTypeIdentityV1],
    output: ExecutionTypeIdentityV1,
    before: u8,
    after: Option<u8>,
    source: u8,
) -> ExecutionCapabilityOpV1 {
    ExecutionCapabilityOpV1 {
        operands,
        signature: ExecutionCapabilitySignatureV1::new(arguments, output).unwrap(),
        provenance: provenance(),
        workgroup_brand: Some([120; 32]),
        epoch_before: Some([before; 32]),
        epoch_after: after.map(|epoch| [epoch; 32]),
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &operation,
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [121; 32],
            operation: [source; 32],
            block: 0,
            occurrence: None,
        },
        operation,
    }
}

fn lds_epoch_roundtrip_module() -> Module {
    let layout = ExecutionElementLayoutV1 {
        byte_size: 4,
        byte_alignment: 4,
    };
    let workgroup = identity(160);
    let uninitialized = identity(161);
    let initialized = identity(162);
    let published = identity(163);
    let transition = identity(164);
    let element = identity(165);
    let index = identity(166);
    let option = identity(167);
    let allocate = ExecutionCapabilityOperationV1::LdsAllocate {
        workgroup,
        lds: uninitialized,
        element,
        layout,
        elements: 4,
    };
    let initialize = ExecutionCapabilityOperationV1::LdsInitializeByInvocation {
        input_lds: uninitialized,
        workgroup,
        output_lds: initialized,
        element,
        layout,
        elements: 4,
    };
    let publish = ExecutionCapabilityOperationV1::LdsPublish {
        input_workgroup: workgroup,
        input_lds: initialized,
        output_lds: published,
        transition,
        element,
        layout,
        elements: 4,
    };
    let read = ExecutionCapabilityOperationV1::LdsReadPublished {
        lds_reference: transition,
        lds: published,
        workgroup: transition,
        index,
        option,
        element,
        layout,
        elements: 4,
    };
    let requirements = [&allocate, &initialize, &publish, &read]
        .into_iter()
        .flat_map(|operation| operation.required_capabilities())
        .collect::<BTreeSet<_>>();
    let lds = |source_type, epoch, state| {
        capability_type(
            source_type,
            [120; 32],
            [epoch; 32],
            ExecutionCapabilityRoleV1::Lds {
                element,
                layout,
                elements: 4,
                state,
            },
        )
    };
    let output = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let output_pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(2),
            context_type(),
            KernelContextSourceIdentityV1::new([0x91; 32], [0x92; 32], [0x93; 32], [0x94; 32]),
        ),
        workgroup_derivation(
            ValueId(2),
            ValueId(3),
            workgroup,
            [120; 32],
            [168; 32],
            0x95,
        ),
        Operation::new(
            vec![ValueDef::new(
                ValueId(4),
                lds(uninitialized, 168, ExecutionLdsStateV1::Uninitialized),
            )],
            OperationKind::ExecutionCapability(workgroup_contract(
                allocate,
                vec![ValueId(3)],
                &[workgroup],
                uninitialized,
                168,
                None,
                0x96,
            )),
        ),
        Operation::new(
            vec![ValueDef::new(
                ValueId(5),
                lds(initialized, 168, ExecutionLdsStateV1::InvocationInitialized),
            )],
            OperationKind::ExecutionCapability(workgroup_contract(
                initialize,
                vec![ValueId(4), ValueId(3), ValueId(0)],
                &[uninitialized, workgroup, element],
                initialized,
                168,
                None,
                0x97,
            )),
        ),
        Operation::new(
            vec![
                ValueDef::new(
                    ValueId(6),
                    capability_type(
                        transition,
                        [120; 32],
                        [169; 32],
                        ExecutionCapabilityRoleV1::Workgroup,
                    ),
                ),
                ValueDef::new(
                    ValueId(7),
                    lds(published, 169, ExecutionLdsStateV1::Published),
                ),
            ],
            OperationKind::ExecutionCapability(workgroup_contract(
                publish,
                vec![ValueId(3), ValueId(5)],
                &[workgroup, initialized],
                transition,
                168,
                Some(169),
                0x98,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        Operation::new(
            vec![
                ValueDef::new(ValueId(9), Type::Scalar(ScalarType::U32)),
                ValueDef::new(ValueId(10), Type::BOOL),
            ],
            OperationKind::ExecutionCapability(workgroup_contract(
                read,
                vec![ValueId(7), ValueId(6), ValueId(8)],
                &[transition, transition, index],
                option,
                169,
                None,
                0x99,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(11), output_pointer.clone()),
            OperationKind::SliceData { slice: ValueId(1) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(12), output_pointer),
            OperationKind::GetElementPointer {
                base: ValueId(11),
                offset: ValueId(8),
            },
        ),
        Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(12),
                value: ValueId(9),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![Type::Scalar(ScalarType::U32), output], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    entry.required_capabilities = requirements.clone();
    let mut kernel = Kernel::new(
        "lds-epoch-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(4, 1, 1));
    let mut module = Module::new("v13-lds-epoch-roundtrip");
    module.functions.push(entry);
    module.kernels.push(kernel);
    module.required_capabilities = requirements;
    module
}

fn workgroup_memory_roundtrip_module() -> Module {
    let layout = ExecutionElementLayoutV1 {
        byte_size: 4,
        byte_alignment: 4,
    };
    let workgroup = identity(190);
    let view = identity(191);
    let witness = identity(192);
    let index_space = identity(193);
    let published = identity(194);
    let transition = identity(195);
    let element = identity(196);
    let store_result = identity(197);
    let load_option = identity(198);
    let allocate = ExecutionCapabilityOperationV1::WorkgroupMemoryAllocate {
        workgroup,
        view,
        element,
        layout,
        elements: 4,
        index_space,
    };
    let index = ExecutionCapabilityOperationV1::WorkgroupMemoryIndex { workgroup, witness };
    let store = ExecutionCapabilityOperationV1::MemoryStore {
        view,
        workgroup: Some(workgroup),
        index: witness,
        element,
        layout,
        result: store_result,
        space: ExecutionMemoryAddressSpaceV1::Workgroup,
        access: ExecutionMemoryAccessV1::DisjointWrite,
    };
    let publish = ExecutionCapabilityOperationV1::WorkgroupMemoryPublish {
        input_workgroup: workgroup,
        input_view: view,
        output_view: published,
        transition,
        element,
        layout,
    };
    let load = ExecutionCapabilityOperationV1::MemoryLoad {
        view: published,
        workgroup: Some(transition),
        index: identity(199),
        option: load_option,
        element,
        layout,
        space: ExecutionMemoryAddressSpaceV1::Workgroup,
        access: ExecutionMemoryAccessV1::ReadOnly,
    };
    let requirements = [&allocate, &index, &store, &publish, &load]
        .into_iter()
        .flat_map(|operation| operation.required_capabilities())
        .collect::<BTreeSet<_>>();
    let write_view = capability_type(
        view,
        [120; 32],
        [200; 32],
        ExecutionCapabilityRoleV1::MemoryView {
            element,
            layout,
            space: ExecutionMemoryAddressSpaceV1::Workgroup,
            access: ExecutionMemoryAccessV1::DisjointWrite,
            extent: ExecutionMemoryExtentV1::Static(4),
            initialization: ExecutionMemoryInitializationV1::Uninitialized,
            index_space: Some(index_space),
            atomic_scope: None,
        },
    );
    let read_view = capability_type(
        published,
        [120; 32],
        [201; 32],
        ExecutionCapabilityRoleV1::MemoryView {
            element,
            layout,
            space: ExecutionMemoryAddressSpaceV1::Workgroup,
            access: ExecutionMemoryAccessV1::ReadOnly,
            extent: ExecutionMemoryExtentV1::Static(4),
            initialization: ExecutionMemoryInitializationV1::Published,
            index_space: None,
            atomic_scope: None,
        },
    );
    let output = Type::slice(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let output_pointer = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        Operation::kernel_context_issue(
            ValueId(2),
            context_type(),
            KernelContextSourceIdentityV1::new([0xb1; 32], [0xb2; 32], [0xb3; 32], [0xb4; 32]),
        ),
        workgroup_derivation(
            ValueId(2),
            ValueId(3),
            workgroup,
            [120; 32],
            [200; 32],
            0xb5,
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(4), write_view)],
            OperationKind::ExecutionCapability(workgroup_contract(
                allocate,
                vec![ValueId(3)],
                &[workgroup],
                view,
                200,
                None,
                0xb6,
            )),
        ),
        Operation::new(
            vec![ValueDef::new(
                ValueId(5),
                capability_type(
                    witness,
                    [120; 32],
                    [200; 32],
                    ExecutionCapabilityRoleV1::WorkgroupMemoryIndex,
                ),
            )],
            OperationKind::ExecutionCapability(workgroup_contract(
                index,
                vec![ValueId(3)],
                &[workgroup],
                witness,
                200,
                None,
                0xb7,
            )),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(6), Type::BOOL)],
            OperationKind::ExecutionCapability(workgroup_contract(
                store,
                vec![ValueId(4), ValueId(3), ValueId(5), ValueId(0)],
                &[view, workgroup, witness, element],
                store_result,
                200,
                None,
                0xb8,
            )),
        ),
        Operation::new(
            vec![
                ValueDef::new(
                    ValueId(7),
                    capability_type(
                        transition,
                        [120; 32],
                        [201; 32],
                        ExecutionCapabilityRoleV1::Workgroup,
                    ),
                ),
                ValueDef::new(ValueId(8), read_view),
            ],
            OperationKind::ExecutionCapability(workgroup_contract(
                publish,
                vec![ValueId(3), ValueId(4)],
                &[workgroup, view],
                transition,
                200,
                Some(201),
                0xb9,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(9), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Local,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        Operation::new(
            vec![
                ValueDef::new(ValueId(10), Type::Scalar(ScalarType::U32)),
                ValueDef::new(ValueId(11), Type::BOOL),
            ],
            OperationKind::ExecutionCapability(workgroup_contract(
                load,
                vec![ValueId(8), ValueId(7), ValueId(9)],
                &[published, transition, identity(199)],
                load_option,
                201,
                None,
                0xba,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(12), output_pointer.clone()),
            OperationKind::SliceData { slice: ValueId(1) },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(13), output_pointer),
            OperationKind::GetElementPointer {
                base: ValueId(12),
                offset: ValueId(9),
            },
        ),
        Operation::new(
            Vec::new(),
            OperationKind::Store {
                pointer: ValueId(13),
                value: ValueId(10),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry = Function::kernel_entry(
        "entry",
        Signature::new(vec![Type::Scalar(ScalarType::U32), output], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    );
    entry.required_capabilities = requirements.clone();
    let mut kernel = Kernel::new(
        "workgroup-memory-kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(4, 1, 1));
    let mut module = Module::new("v13-workgroup-memory-roundtrip");
    module.functions.push(entry);
    module.kernels.push(kernel);
    module.required_capabilities = requirements;
    module
}

pub(super) fn modules()->[Module;2] {[lds_epoch_roundtrip_module(),workgroup_memory_roundtrip_module()]}
}
pub(super) fn legacy_modules()->[Module;2] {legacy::modules()}
