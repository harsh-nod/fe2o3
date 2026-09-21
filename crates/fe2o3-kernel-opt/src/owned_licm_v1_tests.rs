use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, CastKind,
    CheckedBinaryOperator, ComparePredicate, Constant, Function, Kernel, LaunchDomain,
    LaunchExtent, MemoryAccess, Signature, Terminator, ValueDef, ValueId,
    VerifiedCanonicalKernelIrV12,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationEventKindV1 as EventKind, SimulationEventSinkErrorV1, SimulationEventSinkV1,
    SimulationEventV1, SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};
const WORK: usize = 500_000_000;
const STORAGE: usize = 128 << 20;
fn u32_ty() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn branch(target: u32, args: &[u32]) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: args.iter().copied().map(ValueId).collect(),
    }
}
fn conditional(condition: u32, yes: u32, no: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(condition),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    }
}
fn block(id: u32, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.terminator = Some(terminator);
    block
}
fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn blocks(module: &mut Module) -> &mut Vec<BasicBlock> {
    &mut module.functions[0].body.as_mut().unwrap().blocks
}
fn fixture() -> Module {
    let mut entry = block(10, branch(20, &[10]));
    entry.operations = vec![
        op(10, u32_ty(), OperationKind::Constant(Constant::U32(0))),
        op(11, u32_ty(), OperationKind::Constant(Constant::U32(1))),
    ];
    let mut header = block(20, conditional(21, 30, 60));
    header.parameters.push(ValueDef::new(ValueId(20), u32_ty()));
    header.operations.push(op(
        21,
        Type::BOOL,
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(20),
            rhs: ValueId(0),
        },
    ));
    let decision = block(30, conditional(3, 40, 50));
    let mut body = block(40, branch(50, &[]));
    body.operations = vec![
        op(30, u32_ty(), OperationKind::Constant(Constant::U32(7))),
        op(
            31,
            u32_ty(),
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(1),
            },
        ),
        op(
            32,
            u32_ty(),
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(31),
                rhs: ValueId(30),
            },
        ),
        op(
            33,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(1),
                rhs: ValueId(30),
            },
        ),
        op(
            34,
            u32_ty(),
            OperationKind::Select {
                condition: ValueId(33),
                true_value: ValueId(32),
                false_value: ValueId(30),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(34),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    let mut latch = block(50, branch(20, &[35]));
    latch.operations.push(op(
        35,
        u32_ty(),
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(20),
            rhs: ValueId(11),
        },
    ));
    let mut module = Module::new("total-integer-licm-dynamic-conditional");
    module.functions.push(Function::kernel_entry(
        "licm_impl",
        Signature::new(
            vec![
                u32_ty(),
                u32_ty(),
                Type::pointer(u32_ty(), AddressSpace::Global, AccessMode::ReadWrite),
                Type::BOOL,
            ],
            vec![],
        ),
        (0..4).map(ValueId).collect(),
        vec![
            entry,
            header,
            decision,
            body,
            latch,
            block(60, Terminator::Return { values: vec![] }),
        ],
    ));
    module.kernels.push(Kernel::new(
        "licm",
        "licm_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}
fn nested() -> Module {
    let mut outer = block(20, conditional(0, 30, 80));
    outer.parameters.push(ValueDef::new(ValueId(20), u32_ty()));
    let mut pre = block(30, branch(40, &[]));
    pre.operations
        .push(op(100, u32_ty(), OperationKind::Constant(Constant::U32(3))));
    let mut body = block(50, branch(40, &[]));
    body.operations = vec![
        op(101, u32_ty(), OperationKind::Constant(Constant::U32(7))),
        op(
            102,
            u32_ty(),
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(1),
                rhs: ValueId(101),
            },
        ),
        op(
            103,
            u32_ty(),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(100),
                rhs: ValueId(102),
            },
        ),
        op(
            104,
            u32_ty(),
            OperationKind::Binary {
                op: BinaryOp::BitOr,
                lhs: ValueId(20),
                rhs: ValueId(103),
            },
        ),
    ];
    let mut module = Module::new("nested-total-licm");
    module.functions.push(Function::internal_helper(
        "nested",
        Signature::new(vec![Type::BOOL, u32_ty()], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![
            block(10, branch(20, &[1])),
            outer,
            pre,
            block(40, conditional(0, 50, 70)),
            body,
            block(70, branch(20, &[20])),
            block(80, Terminator::Return { values: vec![] }),
        ],
    ));
    module
}
fn irreducible() -> Module {
    let mut side = block(20, branch(30, &[]));
    side.operations
        .push(op(10, u32_ty(), OperationKind::Constant(Constant::U32(9))));
    let mut module = Module::new("irreducible-licm-noop");
    module.functions.push(Function::internal_helper(
        "irreducible",
        Signature::new(vec![Type::BOOL], vec![]),
        vec![ValueId(0)],
        vec![
            block(10, conditional(0, 20, 30)),
            side,
            block(30, conditional(0, 20, 40)),
            block(40, Terminator::Return { values: vec![] }),
        ],
    ));
    module
}
fn admit(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    (owner, storage.retained_storage())
}
fn with_input(module: Module, run: impl FnOnce(&Owner, &mut Budget<'_>)) {
    let (input, bytes) = admit(&module);
    let sibling = vec![0xa5u8; 43];
    let floor = bytes + sibling.capacity();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    run(&input, &mut budget);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(input.module(), &module);
    assert_eq!(sibling, vec![0xa5; 43]);
}
fn release(owner: OwnedLicmContinuationV1, budget: &mut Budget<'_>) {
    let bytes = owner.retained_storage();
    drop(owner);
    budget.release_storage(bytes).unwrap();
}
fn replay(owner: &OwnedLicmContinuationV1, input: &Owner, budget: &mut Budget<'_>) {
    let floor = budget.storage();
    let retained = {
        let (pair, storage) = owner.replay_against(input, budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        assert!(std::ptr::eq(pair.input(), input));
        assert!(std::ptr::eq(pair.output(), owner.output()));
        assert_eq!(pair.origins(), owner.origins());
        assert!(!pair.grants_authority());
        storage.retained_storage()
    };
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn dependency_worklist_has_exact_stable_order_complete_ids_and_one_movement() {
    with_input(fixture(), |input, budget| {
        let owner = prepare_owned_licm_v1(input, budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        let out = &owner.output().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks;
        assert_eq!(
            out[0]
                .operations
                .iter()
                .map(|o| o.results[0].id.0)
                .collect::<Vec<_>>(),
            [10, 11, 30, 31, 33, 32, 34]
        );
        assert_eq!(out[3].operations.len(), 1);
        assert!(matches!(
            out[3].operations[0].kind,
            OperationKind::Store { .. }
        ));
        let moved: Vec<_> = owner
            .origins()
            .iter()
            .filter(|row| row.hoist.is_some())
            .collect();
        assert_eq!(moved.len(), 5);
        assert_eq!(
            moved
                .iter()
                .map(|r| r.hoist.unwrap().sequence)
                .collect::<Vec<_>>(),
            [0, 1, 3, 2, 4]
        );
        assert!(moved.iter().all(|r| r.input.block.block == 3
            && r.output.block.block == 0
            && r.hoist.unwrap().header.block == 1));
        assert_eq!(owner.origins().len(), 10);
        replay(&owner, input, budget);
        assert!(!owner.grants_authority());
        let second = prepare_owned_licm_v1(owner.output(), budget).unwrap();
        budget.reserve_storage(second.retained_storage()).unwrap();
        assert!(second.origins().iter().all(|r| r.hoist.is_none()));
        assert_eq!(
            second.output().canonical().canonical_bytes(),
            owner.output().canonical().canonical_bytes()
        );
        release(second, budget);
        release(owner, budget);
    });
}

#[test]
fn nested_and_multiple_functions_recheck_already_hoisted_operands() {
    let mut module = nested();
    let mut other = module.functions[0].clone();
    other.id = "second".into();
    module.functions.push(other);
    with_input(module, |input, budget| {
        let owner = prepare_owned_licm_v1(input, budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        for function in &owner.output().module().functions {
            let blocks = &function.body.as_ref().unwrap().blocks;
            assert_eq!(
                blocks[0]
                    .operations
                    .iter()
                    .map(|o| o.results[0].id.0)
                    .collect::<Vec<_>>(),
                [100, 101, 102, 103]
            );
            assert_eq!(
                blocks[2]
                    .operations
                    .iter()
                    .map(|o| o.results[0].id.0)
                    .collect::<Vec<_>>(),
                [104]
            );
            assert!(blocks[4].operations.is_empty());
        }
        assert_eq!(
            owner.origins().iter().filter(|r| r.hoist.is_some()).count(),
            10
        );
        let last: Vec<_> = owner
            .origins()
            .iter()
            .filter(|r| r.output.block.block == 2)
            .collect();
        assert_eq!(last.len(), 2);
        assert!(last.iter().all(|r| r.hoist.unwrap().header.block == 3));
        replay(&owner, input, budget);
        let second = prepare_owned_licm_v1(owner.output(), budget).unwrap();
        budget.reserve_storage(second.retained_storage()).unwrap();
        assert!(second.origins().iter().all(|r| r.hoist.is_none()));
        assert_eq!(
            second.output().canonical().canonical_bytes(),
            owner.output().canonical().canonical_bytes()
        );
        release(second, budget);
        release(owner, budget);
    });
}

#[test]
fn irreducible_and_missing_dedicated_preheader_are_byte_exact_noops() {
    let mut no_preheader = fixture();
    blocks(&mut no_preheader)[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(3),
        then_target: BlockId(20),
        then_arguments: vec![ValueId(10)],
        else_target: BlockId(20),
        else_arguments: vec![ValueId(10)],
    });
    for module in [irreducible(), no_preheader] {
        with_input(module, |input, budget| {
            let owner = prepare_owned_licm_v1(input, budget).unwrap();
            budget.reserve_storage(owner.retained_storage()).unwrap();
            assert!(owner.origins().iter().all(|r| r.hoist.is_none()));
            assert_eq!(
                owner.output().canonical().canonical_bytes(),
                input.canonical().canonical_bytes()
            );
            replay(&owner, input, budget);
            release(owner, budget);
        });
    }
}

#[test]
fn excluded_index_float_arithmetic_cast_shift_and_memory_stay_at_original_sites() {
    let mut module = fixture();
    let extra = vec![
        op(40, Type::INDEX, OperationKind::Constant(Constant::Index(1))),
        op(
            41,
            Type::Scalar(ScalarType::F32),
            OperationKind::Constant(Constant::F32Bits(0)),
        ),
        op(
            42,
            u32_ty(),
            OperationKind::Cast {
                kind: CastKind::FloatToInteger,
                value: ValueId(41),
                to: u32_ty(),
            },
        ),
        op(
            43,
            u32_ty(),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(1),
                rhs: ValueId(11),
            },
        ),
        op(
            44,
            u32_ty(),
            OperationKind::Binary {
                op: BinaryOp::Subtract,
                lhs: ValueId(1),
                rhs: ValueId(11),
            },
        ),
        op(
            45,
            u32_ty(),
            OperationKind::Binary {
                op: BinaryOp::Multiply,
                lhs: ValueId(1),
                rhs: ValueId(11),
            },
        ),
        op(
            46,
            u32_ty(),
            OperationKind::Binary {
                op: BinaryOp::Divide,
                lhs: ValueId(1),
                rhs: ValueId(11),
            },
        ),
        op(
            47,
            u32_ty(),
            OperationKind::Binary {
                op: BinaryOp::Remainder,
                lhs: ValueId(1),
                rhs: ValueId(11),
            },
        ),
        op(
            48,
            u32_ty(),
            OperationKind::Binary {
                op: BinaryOp::ShiftLeft,
                lhs: ValueId(1),
                rhs: ValueId(11),
            },
        ),
        op(
            49,
            u32_ty(),
            OperationKind::Binary {
                op: BinaryOp::ShiftRight,
                lhs: ValueId(1),
                rhs: ValueId(11),
            },
        ),
        op(
            50,
            u32_ty(),
            OperationKind::Load {
                pointer: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        Operation::checked_binary(
            ValueDef::new(ValueId(51), u32_ty()),
            ValueDef::new(ValueId(52), Type::BOOL),
            CheckedBinaryOperator::Add,
            ValueId(1),
            ValueId(11),
        ),
        op(
            53,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(41),
                rhs: ValueId(41),
            },
        ),
    ];
    blocks(&mut module)[3].operations.extend(extra.clone());
    with_input(module, |input, budget| {
        let owner = prepare_owned_licm_v1(input, budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        let body = &owner.output().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks[3];
        assert_eq!(&body.operations[1..], extra);
        assert_eq!(
            owner.origins().iter().filter(|r| r.hoist.is_some()).count(),
            5
        );
        assert!(
            owner
                .origins()
                .iter()
                .filter(|r| r.input.block.block == 3 && r.input.operation >= 6)
                .all(|r| r.hoist.is_none())
        );
        replay(&owner, input, budget);
        release(owner, budget);
    });
}

#[derive(Default)]
struct Effects(Vec<SimulationEventV1>);
impl SimulationEventSinkV1 for Effects {
    fn record(
        &mut self,
        event: &SimulationEventV1,
    ) -> std::result::Result<(), SimulationEventSinkErrorV1> {
        if matches!(
            event.kind,
            EventKind::MemoryRead { .. }
                | EventKind::MemoryWrite { .. }
                | EventKind::MemoryAtomic { .. }
                | EventKind::MemoryFence { .. }
                | EventKind::AllocationPreexisting { .. }
                | EventKind::AllocationCreated { .. }
                | EventKind::AllocationReleased { .. }
                | EventKind::WorkgroupBarrierArrive { .. }
                | EventKind::WorkgroupBarrierRelease { .. }
                | EventKind::Call { .. }
                | EventKind::Return
        ) {
            if self.0.len() == 128 {
                return Err(SimulationEventSinkErrorV1 {
                    detail: "complete LICM effect bound".into(),
                });
            }
            self.0.push(event.clone());
        }
        Ok(())
    }
}
fn sim(owner: &Owner) -> AdmittedSimulationModuleV1 {
    let bytes = VerifiedCanonicalKernelIrV12::from_canonical_bytes(
        owner.canonical().canonical_bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(bytes.identity(), owner.canonical().identity());
    AdmittedSimulationModuleV1::admit_v12(bytes, SimulationLimitsV1::default()).unwrap()
}

fn translated_effects(pair: &Pair<'_>, events: &[SimulationEventV1]) -> Vec<SimulationEventV1> {
    events
        .iter()
        .map(|event| {
            let mut translated = event.clone();
            let function = event.site.function_ordinal;
            let input = pair.input().module().functions[function]
                .body
                .as_ref()
                .unwrap();
            let output = pair.output().module().functions[function]
                .body
                .as_ref()
                .unwrap();
            let block = input
                .blocks
                .iter()
                .position(|block| block.id == event.site.block)
                .unwrap();
            assert_eq!(input.blocks[block].id, output.blocks[block].id);
            if let Some(operation) = event.site.operation {
                let mut rows = pair.origins().iter().filter(|row| {
                    usize::try_from(row.input.block.function.0).unwrap() == function
                        && usize::try_from(row.input.block.block).unwrap() == block
                        && row.input.operation == operation
                });
                let row = rows.next().unwrap();
                assert!(rows.next().is_none());
                assert_eq!(row.hoist, None);
                assert_eq!(row.output.block, row.input.block);
                let destination = &output.blocks[usize::try_from(row.output.block.block).unwrap()];
                assert_eq!(
                    input.blocks[block].operations[usize::try_from(operation).unwrap()],
                    destination.operations[usize::try_from(row.output.operation).unwrap()]
                );
                translated.site.block = destination.id;
                translated.site.operation = Some(row.output.operation);
            } else {
                assert_eq!(
                    input.blocks[block].terminator,
                    output.blocks[block].terminator
                );
                assert_eq!(translated.site, event.site);
            }
            translated
        })
        .collect()
}

#[test]
fn sim_dynamic_zero_trip_conditional_body_preserves_cpu_values_masks_and_ordered_effects() {
    with_input(fixture(), |input, budget| {
        let owner = prepare_owned_licm_v1(input, budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        assert_eq!(
            owner.origins().iter().filter(|r| r.hoist.is_some()).count(),
            5
        );
        let pair_bytes = {
            let (pair, storage) = owner.replay_against(input, budget).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            assert!(std::ptr::eq(pair.input(), input));
            assert!(std::ptr::eq(pair.output(), owner.output()));
            let before = sim(input);
            let after = sim(owner.output());
            let target = SimulationTargetV1::amdgpu_64();
            for limit in [0u32, 1, 4] {
                for enabled in [false, true] {
                    for x in [0u32, 7, 99] {
                        let request = SimulationRequestV1::new(
                            "licm",
                            [1, 1, 1],
                            [1, 1, 1],
                            vec![
                                SimulationArgumentV1::Scalar(ScalarBitsV1::u32(limit)),
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
                        let unchanged = request.clone();
                        for _ in 0..2 {
                            let mut first = Effects::default();
                            let mut last = Effects::default();
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
                            assert_eq!(a.arguments(), b.arguments());
                            assert_eq!(translated_effects(&pair, &first.0), last.0);
                            assert_eq!(
                                first
                                    .0
                                    .iter()
                                    .filter(|e| matches!(e.kind, EventKind::MemoryWrite { .. }))
                                    .count(),
                                if enabled { limit as usize } else { 0 }
                            );
                            for result in [&a, &b] {
                                let buffer = result.buffer(2).unwrap();
                                let expected = if enabled && limit > 0 {
                                    (if x < 7 { !x ^ 7 } else { 7 }).to_le_bytes()
                                } else {
                                    [0xa5; 4]
                                };
                                assert_eq!(&buffer.bytes()[..4], &expected);
                                assert_eq!(&buffer.bytes()[4..], &[0xa5; 4]);
                                assert_eq!(&buffer.initialized()[..4], &[enabled && limit > 0; 4]);
                                assert_eq!(&buffer.initialized()[4..], &[false; 4]);
                                assert_eq!(result.invocations_executed(), 1);
                                assert!(!result.grants_execution_authority());
                            }
                            assert_eq!(a.conflict_assessment(), b.conflict_assessment());
                        }
                        assert_eq!(request, unchanged);
                    }
                }
            }
            storage.retained_storage()
        };
        budget.release_storage(pair_bytes).unwrap();
        release(owner, budget);
    });
}

fn deterministic_record() -> String {
    let mut record = None;
    with_input(fixture(), |input, budget| {
        let owner = prepare_owned_licm_v1(input, budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        assert_eq!(
            owner.origins().iter().filter(|r| r.hoist.is_some()).count(),
            5
        );
        record = Some(format!(
            "{:?}|{:?}",
            owner.output().canonical().canonical_bytes(),
            owner.origins()
        ));
        release(owner, budget);
    });
    record.unwrap()
}
#[test]
fn licm_fresh_process_child_emits_actual_complete_subject() {
    println!("\nLICM_RECORD_V1 {}", deterministic_record());
}
#[test]
fn two_fresh_processes_match_actual_bytes_and_complete_lineage() {
    let expected = deterministic_record();
    let child = "owned_licm_v1::tests::licm_fresh_process_child_emits_actual_complete_subject";
    for _ in 0..2 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", child, "--nocapture", "--test-threads=1"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let records: Vec<_> = stdout
            .lines()
            .filter_map(|line| line.strip_prefix("LICM_RECORD_V1 "))
            .collect();
        assert_eq!(records, [expected.as_str()]);
        assert!(stdout.contains("1 passed; 0 failed; 0 ignored"));
        assert!(stdout.contains(child));
    }
}

#[test]
fn exact_work_peak_and_typed_initial_storage_denials_preserve_existing_history() {
    let (input, bytes) = admit(&fixture());
    let sibling = vec![0xa5u8; 43];
    let floor = bytes + sibling.capacity();
    let mut baseline = Work::new(WORK);
    let (needed, peak) = {
        let mut budget = Budget::new(&mut baseline, STORAGE);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let owner = prepare_owned_licm_v1(&input, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        drop(owner);
        (budget.work(), budget.peak_storage())
    };
    for (limit, prior_denial) in [(needed, false), (needed - 1, false), (needed, true)] {
        let mut work = Work::new(limit);
        {
            let mut budget = Budget::new(&mut work, peak);
            budget.reserve_storage(floor).unwrap();
            if prior_denial {
                assert!(
                    matches!(budget.charge_work(limit + 1), Err(Resource::Work(ref e))
                    if e.actual() == limit + 1 && e.limit() == limit)
                );
            }
            budget.charge_work(17).unwrap();
            let result = prepare_owned_licm_v1(&input, &mut budget);
            if limit == needed {
                assert!(result.is_ok());
            } else {
                assert!(
                    matches!(result, Err(Error::Resource(Resource::Work(ref e))) if e.actual() == needed && e.limit() == needed - 1)
                );
            }
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.failed_storage(), None);
            assert_eq!(budget.peak_storage(), peak);
        }
        assert_eq!(work.work(), limit);
        assert_eq!(
            work.failed_work(),
            if prior_denial {
                Some(limit + 1)
            } else {
                (limit != needed).then_some(needed)
            }
        );
    }
    let denied = floor + size_of::<Meter<'_, '_>>() + header().unwrap();
    let mut work = Work::new(WORK);
    {
        let mut budget = Budget::new(&mut work, denied - 1);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let result = prepare_owned_licm_v1(&input, &mut budget);
        assert!(
            matches!(result, Err(Error::Resource(Resource::Storage(ref e))) if e.actual() == denied && e.limit() == denied - 1)
        );
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), Some(denied));
        assert_eq!(budget.peak_storage(), floor + size_of::<Meter<'_, '_>>());
        assert_eq!(budget.work(), 17);
    }
    assert_eq!(work.failed_work(), None);
    assert_eq!(sibling, vec![0xa5; 43]);
}

#[test]
fn actual_candidate_error_and_unwind_drop_before_scope_restores_sibling_floor() {
    let module = fixture();
    let (input, bytes) = admit(&module);
    let sibling = vec![0xa5u8; 43];
    let floor = bytes + sibling.capacity();
    for panic_mode in [false, true] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result: Result<()> = resources::scoped(&mut budget, |meter| {
            let (candidate, receipt) =
                meter.derive(|b| Ok(input.copy_module_for_transformation_v12(b)?))?;
            meter.reserve(receipt.retained_storage())?;
            let (owner, storage) = meter.derive(|b| {
                Ok(Owner::from_module_ref_with_verification_budget_v12(
                    &candidate, b,
                )?)
            })?;
            meter.reserve(storage.retained_storage())?;
            assert_eq!(owner.module(), &module);
            assert_eq!(candidate, module);
            if panic_mode {
                std::panic::panic_any("LICM admitted partial candidate");
            }
            Err(Error::Recipe("injected after actual candidate admission"))
        });
        assert!(matches!(
            (panic_mode, result),
            (true, Err(Error::Panicked))
                | (
                    false,
                    Err(Error::Recipe("injected after actual candidate admission"))
                )
        ));
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(budget.work() > 0);
        assert!(budget.peak_storage() > floor);
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(sibling, vec![0xa5; 43]);
    }
}

#[test]
fn replay_requires_actual_input_and_live_receipt() {
    with_input(fixture(), |input, budget| {
        let mut owner = prepare_owned_licm_v1(input, budget).unwrap();
        let mut tiny_work = Work::new(WORK);
        let mut empty = Budget::new(&mut tiny_work, STORAGE);
        assert!(matches!(
            owner.replay_against(input, &mut empty),
            Err(Error::Resource(Resource::Accounting))
        ));
        budget.reserve_storage(owner.retained_storage()).unwrap();
        let (foreign, foreign_bytes) = admit(&irreducible());
        budget.reserve_storage(foreign_bytes).unwrap();
        assert!(matches!(
            owner.replay_against(&foreign, budget),
            Err(Error::ForeignInput)
        ));
        drop(foreign);
        budget.release_storage(foreign_bytes).unwrap();
        replay(&owner, input, budget);
        let original = owner.origins[3];
        owner.origins[3].hoist.as_mut().unwrap().sequence = u32::MAX;
        assert!(matches!(
            owner.replay_against(input, budget),
            Err(Error::Pair(PairError::Mismatch(
                "unique bounded movement sequence"
            )))
        ));
        owner.origins[3] = original;
        replay(&owner, input, budget);
        release(owner, budget);
    });
}

#[test]
fn prepaid_worklist_bound_rejects_growth_even_with_allocator_slack() {
    let mut slots = Vec::with_capacity(4);
    let floor = slots.capacity() * size_of::<usize>() + 43;
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let result: Result<()> = resources::scoped(&mut budget, |meter| {
        meter.derive(|budget| {
            push_prepaid(&mut slots, 10usize, 2, budget)?;
            push_prepaid(&mut slots, 20usize, 2, budget)?;
            push_prepaid(&mut slots, 30usize, 2, budget)
        })
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(slots, [10, 20]);
    assert_eq!(budget.work(), 6);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn measured_peak_minus_one_records_exact_first_denial_for_strict_successor() {
    let (input, bytes) = admit(&fixture());
    let sibling = vec![0xa5u8; 43];
    let floor = bytes + sibling.capacity();
    let mut baseline = Work::new(WORK);
    let (complete_work, peak) = {
        let mut budget = Budget::new(&mut baseline, STORAGE);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let owner = prepare_owned_licm_v1(&input, &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        drop(owner);
        (budget.work(), budget.peak_storage())
    };
    let mut work = Work::new(complete_work);
    {
        let mut budget = Budget::new(&mut work, peak - 1);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let error = match prepare_owned_licm_v1(&input, &mut budget) {
            Ok(owner) => {
                drop(owner);
                panic!("measured peak-minus-one must refuse");
            }
            Err(error) => error,
        };
        assert_eq!(budget.failed_storage(), Some(peak));
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert!(budget.work() >= 17 && budget.work() < complete_work);
        assert!(budget.peak_storage() < peak);
        let Error::Pair(PairError::Loops(LoopError::Resource(Resource::Storage(error)))) = error
        else {
            panic!("expected independent loop replay Storage refusal")
        };
        assert_eq!((error.actual(), error.limit()), (peak, peak - 1));
        assert_eq!(peak, 43_224);
        assert_eq!((budget.work(), budget.peak_storage()), (21_884, 43_112));
    }
    assert_eq!(work.failed_work(), None);
    assert_eq!(sibling, vec![0xa5; 43]);
}
