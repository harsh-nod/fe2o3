use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, ComparePredicate, Constant,
    Function, Kernel, LaunchDomain, LaunchExtent, Module, Operation, Signature, Terminator,
    ValueDef, VerifiedCanonicalKernelIrV12,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationEventKindV1 as EventKind, SimulationEventSinkErrorV1, SimulationEventSinkV1,
    SimulationEventV1, SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};
use std::mem::size_of_val;
const WORK: usize = 200_000_000;
const STORAGE: usize = 128 << 20;
fn ty() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn jump(target: u32, args: &[u32]) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: args.iter().copied().map(ValueId).collect(),
    }
}
fn branch(condition: u32, yes: u32, no: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(condition),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    }
}
fn block(id: u32, operations: Vec<Operation>, terminator: Terminator) -> BasicBlock {
    let mut result = BasicBlock::new(BlockId(id));
    result.operations = operations;
    result.terminator = Some(terminator);
    result
}
fn value(id: u32, ty: Type, kind: Kind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn allocation(id: u32, ty: Type) -> Operation {
    let Type::Scalar(scalar) = ty else {
        panic!("scalar test cell")
    };
    let alignment = u32::from(scalar.bit_width().unwrap() / 8);
    value(
        id,
        Type::pointer(
            Type::Scalar(scalar),
            AddressSpace::Private,
            AccessMode::ReadWrite,
        ),
        Kind::Alloca {
            element: Type::Scalar(scalar),
            count: None,
            address_space: AddressSpace::Private,
            alignment,
        },
    )
}
fn store(pointer: u32, value: u32, access: MemoryAccess) -> Operation {
    Operation::new(
        vec![],
        Kind::Store {
            pointer: ValueId(pointer),
            value: ValueId(value),
            access,
        },
    )
}
fn fixture(scalar: ScalarType) -> Module {
    let ty = Type::Scalar(scalar);
    let access = MemoryAccess::new(
        AddressSpace::Private,
        u32::from(scalar.bit_width().unwrap() / 8),
    );
    let mut module = Module::new("owned-cross-block-forwarding");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![ty.clone(), Type::BOOL], vec![ty.clone()]),
        vec![ValueId(0), ValueId(1)],
        vec![
            block(
                10,
                vec![allocation(100, ty.clone()), store(100, 0, access)],
                branch(1, 20, 30),
            ),
            block(20, vec![], jump(40, &[])),
            block(30, vec![], jump(40, &[])),
            block(
                40,
                vec![value(
                    101,
                    ty,
                    Kind::Load {
                        pointer: ValueId(100),
                        access,
                    },
                )],
                Terminator::Return {
                    values: vec![ValueId(101)],
                },
            ),
        ],
    ));
    module
}
fn blocks(module: &mut Module) -> &mut Vec<BasicBlock> {
    &mut module.functions[0].body.as_mut().unwrap().blocks
}
fn admit(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    (owner, receipt.retained_storage())
}
fn with_input(module: Module, next: impl FnOnce(&Owner, &mut Budget<'_>)) {
    let (input, bytes) = admit(&module);
    let sibling = vec![0x71u8; 43];
    let floor = bytes + size_of_val(&sibling) + sibling.capacity();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    next(&input, &mut budget);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(sibling, [0x71; 43]);
    assert_eq!(input.module(), &module);
}
fn release(owner: OwnedCrossBlockForwardingV1, budget: &mut Budget<'_>) {
    let bytes = owner.retained_storage();
    drop(owner);
    budget.release_storage(bytes).unwrap();
}
fn selected(owner: &OwnedCrossBlockForwardingV1) -> usize {
    owner
        .origins()
        .iter()
        .filter(|row| row.store.is_some())
        .count()
}
fn assert_count(module: Module, count: usize) {
    with_input(module, |input, budget| {
        let owner =
            prepare_owned_cross_block_forwarding_v1(input, Limits::default(), budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        assert_eq!(selected(&owner), count);
        let receipt = {
            let (pair, receipt) = owner.replay_against(input, budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert!(std::ptr::eq(pair.input(), input));
            assert!(std::ptr::eq(pair.output(), owner.output()));
            assert_eq!(pair.origins(), owner.origins());
            receipt
        };
        budget.release_storage(receipt.retained_storage()).unwrap();
        if count == 0 {
            assert_eq!(
                input.canonical().canonical_bytes(),
                owner.output().canonical().canonical_bytes()
            );
        } else {
            assert_ne!(
                input.canonical().canonical_bytes(),
                owner.output().canonical().canonical_bytes()
            );
        }
        assert!(!owner.grants_authority());
        release(owner, budget);
    });
}
#[test]
fn cross_block_owned_all_fixed_integer_widths_preserve_exact_ids_and_accesses() {
    for scalar in [
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
    ] {
        assert_count(fixture(scalar), 1);
    }
}
#[test]
fn cross_block_owned_phi_sccs_need_every_predecessor_grounded() {
    let mut module = fixture(ScalarType::U32);
    blocks(&mut module)[1].terminator = Some(branch(1, 30, 40));
    blocks(&mut module)[2].terminator = Some(branch(1, 20, 40));
    assert_count(module, 1);
    let mut module = fixture(ScalarType::U32);
    blocks(&mut module)[0].terminator = Some(jump(40, &[]));
    blocks(&mut module)[1].terminator = Some(branch(1, 20, 40));
    blocks(&mut module)[2].terminator = Some(Terminator::Unreachable);
    assert_count(module, 0);
}
#[test]
fn cross_block_owned_duplicate_edges_missing_initialization_and_equal_store_occurrences() {
    let mut module = fixture(ScalarType::U32);
    blocks(&mut module)[0].terminator = Some(branch(1, 20, 20));
    blocks(&mut module)[2].terminator = Some(Terminator::Unreachable);
    assert_count(module, 1);
    let mut module = fixture(ScalarType::U32);
    blocks(&mut module)[0].operations.remove(1);
    assert_count(module, 0);
    let mut module = fixture(ScalarType::U32);
    blocks(&mut module)[1].operations.push(store(
        100,
        0,
        MemoryAccess::new(AddressSpace::Private, 4),
    ));
    assert_count(module, 0);
}
#[test]
fn cross_block_owned_function_local_ids_never_cross_functions_and_limits_are_exact() {
    let mut module = fixture(ScalarType::U32);
    let mut second = module.functions[0].clone();
    second.id = "second".into();
    blocks(&mut module)[0].operations.remove(1);
    module.functions.push(second);
    assert_count(module, 1);
    with_input(fixture(ScalarType::U32), |input, budget| {
        let mut limits = Limits::default();
        limits.memory.operations = 2;
        assert!(matches!(
            prepare_owned_cross_block_forwarding_v1(input, limits, budget),
            Err(Error::Memory(MemoryError::InputLimit {
                resource: fe2o3_kernel_analysis::CanonicalKirMemorySsaResourceV1::Operations,
                actual: 3,
                limit: 2
            }))
        ));
    });
}
#[test]
fn cross_block_owned_preserves_existing_first_denial_history_during_success() {
    with_input(fixture(ScalarType::U32), |input, budget| {
        let floor = budget.storage();
        let failed_work = budget.work().checked_add(WORK + 1).unwrap();
        assert!(budget.charge_work(WORK + 1).is_err());
        let failed_storage = floor.checked_add(STORAGE + 1).unwrap();
        assert!(budget.reserve_storage(STORAGE + 1).is_err());
        let owner =
            prepare_owned_cross_block_forwarding_v1(input, Limits::default(), budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        assert_eq!(selected(&owner), 1);
        assert_eq!(budget.failed_storage(), Some(failed_storage));
        // Budget exposes storage history; the nested preexisting work denial is
        // checked through a separate Work owner below, after dropping its borrow.
        assert!(failed_work > WORK);
        release(owner, budget);
    });
    let (input, bytes) = admit(&fixture(ScalarType::U32));
    let mut work = Work::new(WORK);
    assert!(work.charge_work(WORK + 1).is_err());
    {
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(bytes).unwrap();
        let owner = prepare_owned_cross_block_forwarding_v1(&input, Limits::default(), &mut budget)
            .unwrap();
        assert_eq!(selected(&owner), 1);
        drop(owner);
        assert_eq!(budget.storage(), bytes);
    }
    assert_eq!(work.failed_work(), Some(WORK + 1));
}
#[test]
fn cross_block_owned_other_defs_prior_reads_and_non_total_pure_operations_are_cuts() {
    let mut module = fixture(ScalarType::U32);
    blocks(&mut module)[1]
        .operations
        .push(allocation(200, ty()));
    assert_count(module, 0);
    let mut module = fixture(ScalarType::U32);
    blocks(&mut module)[1].operations.push(value(
        200,
        ty(),
        Kind::Load {
            pointer: ValueId(100),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        },
    ));
    // This invocation selects only the earlier Load; its original occurrence is
    // still a conservative cut for the later load in this single pass.
    assert_count(module, 1);
    for binary in [BinaryOp::Add, BinaryOp::Divide, BinaryOp::Remainder] {
        let mut module = fixture(ScalarType::U32);
        blocks(&mut module)[1].operations.push(value(
            200,
            ty(),
            Kind::Binary {
                op: binary,
                lhs: ValueId(0),
                rhs: ValueId(0),
            },
        ));
        assert_count(module, 0);
    }
    let mut module = fixture(ScalarType::U32);
    blocks(&mut module)[1].operations.push(value(
        200,
        ty(),
        Kind::Binary {
            op: BinaryOp::BitAnd,
            lhs: ValueId(0),
            rhs: ValueId(0),
        },
    ));
    assert_count(module, 1);
}
#[test]
fn cross_block_owned_escaping_volatile_and_unequal_alignment_cells_are_not_selected() {
    let mut module = fixture(ScalarType::U32);
    let Kind::Load { access, .. } = &mut blocks(&mut module)[3].operations[0].kind else {
        panic!("load")
    };
    access.volatile = true;
    assert_count(module, 0);
    let mut module = fixture(ScalarType::U32);
    let Kind::Load { access, .. } = &mut blocks(&mut module)[3].operations[0].kind else {
        panic!("load")
    };
    access.alignment = 1;
    assert_count(module, 0);
    let mut module = fixture(ScalarType::U32);
    blocks(&mut module)[1].parameters.push(ValueDef::new(
        ValueId(200),
        Type::pointer(ty(), AddressSpace::Private, AccessMode::ReadWrite),
    ));
    let Some(Terminator::ConditionalBranch { then_arguments, .. }) =
        &mut blocks(&mut module)[0].terminator
    else {
        panic!("branch")
    };
    then_arguments.push(ValueId(100));
    // A valid pointer use outside the closed direct Load/Store census disqualifies
    // the allocation even if it is unreachable or has no memory effect.
    assert_count(module, 0);
}
#[test]
fn cross_block_owned_replay_requires_actual_input_and_stored_receipt() {
    with_input(fixture(ScalarType::U32), |input, budget| {
        let mut owner =
            prepare_owned_cross_block_forwarding_v1(input, Limits::default(), budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        let floor = budget.storage();
        let mut other = input.module().clone();
        other.id = "another-forwarding-subject".into();
        let (other, _) = admit(&other);
        assert!(matches!(
            owner.replay_against(&other, budget),
            Err(Error::ForeignInput)
        ));
        owner.retained += 1;
        assert!(matches!(
            owner.replay_against(input, budget),
            Err(Error::Resource(Resource::Accounting))
        ));
        owner.retained -= 1;
        owner.origins.swap(0, 1);
        assert!(matches!(
            owner.replay_against(input, budget),
            Err(Error::Pair(PairError::Mismatch(
                "complete unchanged coordinates"
            )))
        ));
        owner.origins.swap(0, 1);
        assert_eq!(budget.storage(), floor);
        release(owner, budget);
    });
}
fn resource_run(
    input: &Owner,
    floor: usize,
    w: usize,
    p: usize,
) -> (
    Result<OwnedCrossBlockForwardingV1>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
) {
    let mut work = Work::new(w);
    work.charge_work(17).unwrap();
    let (result, used, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, p);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = prepare_owned_cross_block_forwarding_v1(input, Limits::default(), &mut budget);
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    (result, used, peak, work.failed_work(), failed_storage)
}
#[test]
fn cross_block_owned_exact_work_storage_and_first_denial_preserve_incoming_floor() {
    let module = fixture(ScalarType::U32);
    let (input, bytes) = admit(&module);
    let sibling = vec![0x81u8; 37];
    let floor = bytes + size_of_val(&sibling) + sibling.capacity();
    let full = resource_run(&input, floor, WORK, STORAGE);
    let owner = full.0.unwrap();
    assert_eq!(selected(&owner), 1);
    drop(owner);
    let exact = resource_run(&input, floor, full.1, full.2);
    drop(exact.0.unwrap());
    assert_eq!(
        (exact.1, exact.2, exact.3, exact.4),
        (full.1, full.2, None, None)
    );
    let short = resource_run(&input, floor, full.1 - 1, full.2);
    match short.0 {
        Err(Error::Resource(Resource::Work(error))) => {
            assert_eq!((error.actual(), error.limit()), (full.1, full.1 - 1))
        }
        _ => panic!("final owning work charge"),
    }
    assert_eq!(
        (short.1, short.2, short.3, short.4),
        (full.1 - 1, full.2, Some(full.1), None)
    );
    let short = resource_run(&input, floor, WORK, full.2 - 1);
    let Err(Error::Pair(PairError::ControlFlow(
        fe2o3_kernel_ir::CanonicalKirControlFlowScopeErrorV1::Resource(Resource::Storage(limit)),
    ))) = &short.0
    else {
        panic!(
            "exact dominator-interval stack refusal: {:?}",
            short.0.as_ref().err()
        )
    };
    assert_eq!((limit.actual(), limit.limit()), (full.2, full.2 - 1));
    let body = module.functions[0].body.as_ref().unwrap();
    let blocks = body.blocks.len();
    let edges = body
        .blocks
        .iter()
        .map(|b| match b.terminator.as_ref().unwrap() {
            Terminator::Branch { .. } => 1,
            Terminator::ConditionalBranch { .. } => 2,
            Terminator::Return { .. } => 0,
            _ => panic!("unchanged acyclic diamond"),
        })
        .sum::<usize>();
    let operations = body
        .blocks
        .iter()
        .map(|b| b.operations.len())
        .sum::<usize>();
    assert_eq!((blocks, edges, operations), (4, 4, 3));
    // Same final CFG suffix as the pair: interval walk + reducibility, then
    // 3O checks, one reachable/dominates query pair, and pair/owner transfers.
    let tail_work = 23 * blocks + 11 * edges + 8 + 3 * operations + 15 + 2;
    assert_eq!(
        (short.1, short.2, short.3, short.4),
        (full.1 - tail_work, full.2 - 2 * blocks, None, Some(full.2))
    );
    assert_eq!(sibling, [0x81; 37]);
}

fn loop_fixture() -> Module {
    let mut header = block(
        20,
        vec![value(
            21,
            Type::BOOL,
            Kind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(20),
                rhs: ValueId(0),
            },
        )],
        branch(21, 30, 60),
    );
    header.parameters.push(ValueDef::new(ValueId(20), ty()));
    let mut module = Module::new("dynamic-private-forwarding");
    module.functions.push(Function::kernel_entry(
        "f",
        Signature::new(
            vec![
                ty(),
                ty(),
                Type::pointer(ty(), AddressSpace::Global, AccessMode::ReadWrite),
                Type::BOOL,
            ],
            vec![],
        ),
        (0..4).map(ValueId).collect(),
        vec![
            block(
                10,
                vec![
                    allocation(100, ty()),
                    value(10, ty(), Kind::Constant(Constant::U32(0))),
                    value(11, ty(), Kind::Constant(Constant::U32(1))),
                ],
                jump(20, &[10]),
            ),
            header,
            block(
                30,
                vec![store(100, 1, MemoryAccess::new(AddressSpace::Private, 4))],
                branch(3, 40, 50),
            ),
            block(
                40,
                vec![
                    value(
                        101,
                        ty(),
                        Kind::Load {
                            pointer: ValueId(100),
                            access: MemoryAccess::new(AddressSpace::Private, 4),
                        },
                    ),
                    store(2, 101, MemoryAccess::new(AddressSpace::Global, 4)),
                ],
                jump(50, &[]),
            ),
            block(
                50,
                vec![value(
                    30,
                    ty(),
                    Kind::Binary {
                        op: BinaryOp::Add,
                        lhs: ValueId(20),
                        rhs: ValueId(11),
                    },
                )],
                jump(20, &[30]),
            ),
            block(60, vec![], Terminator::Return { values: vec![] }),
        ],
    ));
    module.kernels.push(Kernel::new(
        "forward",
        "f",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}
#[derive(Default)]
struct Events(Vec<SimulationEventV1>);
impl SimulationEventSinkV1 for Events {
    fn record(
        &mut self,
        event: &SimulationEventV1,
    ) -> std::result::Result<(), SimulationEventSinkErrorV1> {
        if self.0.len() == 4096 {
            return Err(SimulationEventSinkErrorV1 {
                detail: "complete forwarding event bound".into(),
            });
        }
        self.0.push(event.clone());
        Ok(())
    }
}
fn sim(owner: &Owner) -> AdmittedSimulationModuleV1 {
    let wire = VerifiedCanonicalKernelIrV12::from_canonical_bytes(
        owner.canonical().canonical_bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(wire.identity(), owner.canonical().identity());
    AdmittedSimulationModuleV1::admit_v12(wire, SimulationLimitsV1::default()).unwrap()
}
fn without_selected_reads(
    owner: &OwnedCrossBlockForwardingV1,
    input: &Owner,
    events: &[SimulationEventV1],
) -> (Vec<SimulationEventV1>, usize) {
    let mut retained = Vec::new();
    let mut removed = 0;
    for (index, event) in events.iter().enumerate() {
        if let EventKind::MemoryRead {
            allocation,
            offset,
            bytes,
        } = event.kind
        {
            let body = input.module().functions[event.site.function_ordinal]
                .body
                .as_ref()
                .unwrap();
            let block = body
                .blocks
                .iter()
                .position(|b| b.id == event.site.block)
                .unwrap();
            let selected = owner.origins().iter().find(|r| {
                r.store.is_some()
                    && r.input.block.function.0 as usize == event.site.function_ordinal
                    && r.input.block.block as usize == block
                    && Some(r.input.operation) == event.site.operation
            });
            if let Some(row) = selected {
                let store = row.store.unwrap();
                let store_block = body.blocks[store.block.block as usize].id;
                assert_eq!((offset, bytes), (0, 4));
                assert!(
                    events[..index]
                        .iter()
                        .any(|prior| prior.invocation == event.invocation
                            && prior.site.block == store_block
                            && prior.site.operation == Some(store.operation)
                            && prior.kind
                                == (EventKind::MemoryWrite {
                                    allocation,
                                    offset,
                                    bytes
                                }))
                );
                assert!(events[..index].iter().any(|created|created.invocation==event.invocation&&matches!(created.kind,
                    EventKind::AllocationCreated{allocation:a,address_space:AddressSpace::Private,bytes:4} if a==allocation)));
                assert_eq!(row.input, row.output);
                removed += 1;
                continue;
            }
        }
        retained.push(event.clone());
    }
    (retained, removed)
}
#[test]
fn cross_block_owned_dynamic_zero_one_many_iterations_preserve_all_nonread_events_and_cpu_results()
{
    with_input(loop_fixture(), |input, budget| {
        let owner =
            prepare_owned_cross_block_forwarding_v1(input, Limits::default(), budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        assert_eq!(selected(&owner), 1);
        let before = sim(input);
        let after = sim(owner.output());
        let target = SimulationTargetV1::amdgpu_64();
        let mut comparisons = 0;
        for bound in [0u32, 1, 4] {
            for enabled in [false, true] {
                for x in [0u32, 7, u32::MAX] {
                    let request = SimulationRequestV1::new(
                        "forward",
                        [1, 1, 1],
                        [1, 1, 1],
                        vec![
                            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(bound)),
                            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(x)),
                            SimulationArgumentV1::Buffer(
                                BufferArgumentV1::new(
                                    ScalarType::U32,
                                    AccessMode::ReadWrite,
                                    4,
                                    vec![0xa5; 8],
                                    vec![false; 8],
                                    target,
                                )
                                .unwrap(),
                            ),
                            SimulationArgumentV1::Scalar(ScalarBitsV1::boolean(enabled)),
                        ],
                    );
                    for _ in 0..2 {
                        let mut first = Events::default();
                        let mut last = Events::default();
                        let a = before
                            .simulate_observed_with_sink(
                                &request,
                                target,
                                SimulationLimitsV1::default(),
                                &mut first,
                            )
                            .unwrap();
                        let b = after
                            .simulate_observed_with_sink(
                                &request,
                                target,
                                SimulationLimitsV1::default(),
                                &mut last,
                            )
                            .unwrap();
                        let (expected, removed) = without_selected_reads(&owner, input, &first.0);
                        assert_eq!(removed, if enabled { bound as usize } else { 0 });
                        assert_eq!(expected, last.0);
                        assert_eq!(a.arguments(), b.arguments());
                        for result in [&a, &b] {
                            let buffer = result.buffer(2).unwrap();
                            let writes = enabled && bound > 0;
                            assert_eq!(
                                &buffer.bytes()[..4],
                                &if writes { x.to_le_bytes() } else { [0xa5; 4] }
                            );
                            assert_eq!(&buffer.bytes()[4..], &[0xa5; 4]);
                            assert_eq!(&buffer.initialized()[..4], &[writes; 4]);
                            assert_eq!(&buffer.initialized()[4..], &[false; 4]);
                            assert_eq!(result.invocations_executed(), 1);
                            assert!(!result.grants_execution_authority());
                        }
                        assert_eq!(a.conflict_assessment(), b.conflict_assessment());
                        comparisons += 1;
                    }
                }
            }
        }
        assert_eq!(comparisons, 36);
        release(owner, budget);
    });
}
fn record() -> String {
    let mut result = None;
    with_input(fixture(ScalarType::U32), |input, budget| {
        let owner =
            prepare_owned_cross_block_forwarding_v1(input, Limits::default(), budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        assert_eq!(selected(&owner), 1);
        result = Some(format!(
            "{:?}|{:?}",
            owner.output().canonical().canonical_bytes(),
            owner.origins()
        ));
        release(owner, budget);
    });
    result.unwrap()
}
#[test]
fn cross_block_owned_fresh_process_child() {
    println!("\nCROSS_BLOCK_FORWARDING_RECORD {}", record());
}
#[test]
fn cross_block_owned_two_fresh_processes_match_complete_bytes_and_lineage() {
    let expected = record();
    let name = "owned_cross_block_forwarding_v1::tests::cross_block_owned_fresh_process_child";
    for _ in 0..2 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name, "--nocapture", "--test-threads=1"])
            .output()
            .unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("test result: ok. 1 passed; 0 failed; 0 ignored;"));
        let records: Vec<_> = text
            .lines()
            .filter_map(|line| line.strip_prefix("CROSS_BLOCK_FORWARDING_RECORD "))
            .collect();
        assert_eq!(records, [expected.as_str()]);
    }
}

#[test]
fn cross_block_owned_prepays_twelve_temporary_headers_and_complete_queue_before_backing() {
    assert_eq!(
        plan_headers().unwrap(),
        12 * size_of::<Vec<u8>>() + size_of::<Queue>()
    );
    with_input(fixture(ScalarType::U32), |input, budget| {
        let (inventory, storage) = Inventory::derive(input, budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let (memory, memory_storage) =
            Memory::derive(&inventory, Limits::default().memory, budget).unwrap();
        budget
            .reserve_storage(memory_storage.retained_storage())
            .unwrap();
        let floor = budget.storage();
        let before = floor + size_of::<Meter<'_, '_>>() + header().unwrap();
        let attempted = before + plan_headers().unwrap();
        let mut work = Work::new(WORK);
        work.charge_work(17).unwrap();
        {
            let mut limited = Budget::new(&mut work, attempted - 1);
            limited.reserve_storage(floor).unwrap();
            let result = resources::scoped(&mut limited, |meter| {
                meter.reserve(header()?)?;
                plan(&inventory, &memory, Limits::default(), meter)
            });
            match result {
                Err(Error::Resource(Resource::Storage(error))) => {
                    assert_eq!((error.actual(), error.limit()), (attempted, attempted - 1))
                }
                _ => panic!("exact producer plan header prepayment"),
            }
            assert_eq!(
                (
                    limited.work(),
                    limited.storage(),
                    limited.peak_storage(),
                    limited.failed_storage()
                ),
                (30, floor, before, Some(attempted))
            );
        }
        assert_eq!(work.failed_work(), None);
        drop(memory);
        budget
            .release_storage(memory_storage.retained_storage())
            .unwrap();
        drop(inventory);
        budget.release_storage(storage.retained_storage()).unwrap();
    });
}
