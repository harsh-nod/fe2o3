use super::*;
use crate::{BasicBlock, Kernel, Signature, StorageLayoutLimitsV1, SwitchCase, ValueDef};

fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

fn fixture(case_value: u64, scalar: ScalarType) -> Module {
    let element = Type::Scalar(ScalarType::U32);
    let slice = Type::slice(element.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let pointer = Type::pointer(element.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let mut guard = BasicBlock::new(BlockId(10));
    guard.operations = vec![
        op(
            4,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        op(
            7,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(1),
                rhs: ValueId(4),
            },
        ),
        op(
            13,
            Type::Scalar(scalar),
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value: ValueId(7),
                to: Type::Scalar(scalar),
            },
        ),
    ];
    guard.terminator = Some(Terminator::Switch {
        selector: ValueId(13),
        cases: vec![SwitchCase {
            value: case_value,
            target: BlockId(if case_value == 1 { 20 } else { 30 }),
            arguments: vec![],
        }],
        default_target: BlockId(if case_value == 1 { 30 } else { 20 }),
        default_arguments: vec![],
    });
    let mut read = BasicBlock::new(BlockId(20));
    read.operations = vec![
        op(
            10,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(
            11,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(10),
                offset: ValueId(1),
            },
        ),
        op(
            12,
            element,
            OperationKind::Load {
                pointer: ValueId(11),
                access: MemoryAccess::new(AddressSpace::Global, 1),
            },
        ),
    ];
    read.terminator = Some(Terminator::Return { values: vec![] });
    let mut exit = BasicBlock::new(BlockId(30));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("single-case-guard-truth");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                slice,
                Type::INDEX,
                Type::INDEX,
                Type::Scalar(ScalarType::U8),
            ],
            vec![],
        ),
        (0..4).map(ValueId).collect(),
        vec![guard, read, exit],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn at() -> Coordinate {
    Coordinate {
        block: BlockCoordinate {
            function: FunctionCoordinate(0),
            block: 1,
        },
        operation: 2,
    }
}

fn check<O>(
    view: &CheckedCanonicalGuardedGlobalReadsV1<'_, '_, O>,
    budget: &mut Budget<'_>,
    expected: Option<usize>,
) -> Result<()> {
    let storage = budget.storage();
    let mut previous = None;
    for _ in 0..3 {
        let before = budget.work();
        match (view.read_at(at(), budget)?, expected) {
            (CanonicalGuardedGlobalReadOutcomeV1::ProvedLocalConditions(read), Some(ordinal)) => {
                assert_eq!(read.operation(), at());
                assert_eq!(read.domain().predicate(), ValueId(7));
                assert_eq!(read.comparison_operands(), (ValueId(1), ValueId(4)));
                assert_eq!(
                    read.normalized_index_origin(),
                    CanonicalGuardedReadIndexOriginV1::ProvenOrigin(ValueId(1))
                );
                assert_eq!(read.normalized_length_origin(), ValueId(4));
                assert_eq!(
                    read.domain().path(),
                    FormalGuardedPathV1::TrueEdge {
                        source: BlockId(10),
                        ordinal,
                        target: BlockId(20),
                    }
                );
                assert!(read.requires_runtime_allocation_binding());
                let truth = view
                    .true_at(at(), ValueId(7), budget)?
                    .expect("same authenticated Bool truth");
                assert_eq!(truth.edge(), (BlockId(10), ordinal, BlockId(20)));
                assert!(
                    view.true_at(at(), ValueId(13), budget)?.is_none(),
                    "integer transport is not a Bool truth"
                );
            }
            (
                CanonicalGuardedGlobalReadOutcomeV1::NotProved(
                    CanonicalGuardedGlobalReadReasonV1::MissingBoundOrProvenance,
                ),
                None,
            ) => {}
            _ => panic!("one-case read proof outcome differs"),
        }
        let work = budget.work() - before;
        assert!(previous.is_none_or(|cost| cost == work));
        previous = Some(work);
        assert_eq!(budget.storage(), storage);
    }
    Ok(())
}

fn run(
    module: &Module,
    v18: bool,
    expected: Option<usize>,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<()>, usize, usize) {
    let mut construction_work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut construction = Budget::new(&mut construction_work, 10_000_000);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    let result;
    let floor;
    if v18 {
        let (owner, receipt) =
            VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                module,
                StorageLayoutLimitsV1 {
                    rows: 8,
                    edges: 16,
                    containment_depth: 8,
                    object_bytes: 1024,
                },
                &mut construction,
            )
            .unwrap();
        floor = receipt.retained_storage() + 17;
        result = budget
            .reserve_storage(floor)
            .map_err(Failure::from)
            .and_then(|()| {
                with_canonical_guarded_global_reads_v18(
                    &owner,
                    Default::default(),
                    &mut budget,
                    |view, budget| check(view, budget, expected),
                )
            });
    } else {
        let (owner, _) =
            VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                module,
                &mut construction,
            )
            .unwrap();
        floor = 17;
        result = budget
            .reserve_storage(floor)
            .map_err(Failure::from)
            .and_then(|()| {
                with_canonical_guarded_global_reads_v1(
                    &owner,
                    Default::default(),
                    &mut budget,
                    |view, budget| check(view, budget, expected),
                )
            });
    }
    assert_eq!(construction.storage(), 0);
    assert_eq!(budget.storage(), floor);
    (result, budget.work(), budget.peak_storage())
}

fn both(module: &Module, expected: Option<usize>) {
    for v18 in [false, true] {
        run(module, v18, expected, 100_000_000, 100_000_000)
            .0
            .unwrap();
    }
}

#[test]
fn one_case_switch_truth_v12_v18_tracks_explicit_true_and_default_true_all_integer_widths() {
    for scalar in [
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
    ] {
        for (case, ordinal) in [(1, 0), (0, 1)] {
            both(&fixture(case, scalar), Some(ordinal));
        }
    }
}

#[test]
fn one_case_switch_truth_v12_v18_refuses_false_edges_same_targets_and_false_bypass() {
    for case in [0, 1] {
        for fault in 0..3 {
            let mut module = fixture(case, ScalarType::U32);
            let body = module.functions[0].body.as_mut().unwrap();
            let Some(Terminator::Switch {
                cases,
                default_target,
                ..
            }) = &mut body.blocks[0].terminator
            else {
                unreachable!()
            };
            match fault {
                0 => std::mem::swap(&mut cases[0].target, default_target),
                1 => {
                    cases[0].target = BlockId(20);
                    *default_target = BlockId(20);
                }
                2 => {
                    body.blocks[2].terminator = Some(Terminator::Branch {
                        target: BlockId(20),
                        arguments: vec![],
                    })
                }
                _ => unreachable!(),
            }
            both(&module, None);
        }
    }
}

#[test]
fn one_case_switch_truth_v12_v18_rejects_raw_integer_foreign_width_index_and_guard() {
    for fault in 0..7 {
        let mut module = fixture(1, ScalarType::U32);
        let function = &mut module.functions[0];
        let body = function.body.as_mut().unwrap();
        match fault {
            0 => {
                function.signature.parameters[3] = Type::Scalar(ScalarType::U32);
                let Some(Terminator::Switch { selector, .. }) = &mut body.blocks[0].terminator
                else {
                    unreachable!()
                };
                *selector = ValueId(3);
            }
            1 => {
                let OperationKind::Cast { value, .. } = &mut body.blocks[0].operations[2].kind
                else {
                    unreachable!()
                };
                *value = ValueId(3);
            }
            2 => {
                let OperationKind::Compare { lhs, .. } = &mut body.blocks[0].operations[1].kind
                else {
                    unreachable!()
                };
                *lhs = ValueId(2);
            }
            3 => {
                let OperationKind::Compare { predicate, .. } =
                    &mut body.blocks[0].operations[1].kind
                else {
                    unreachable!()
                };
                *predicate = ComparePredicate::LessThanOrEqual;
            }
            4 => {
                let Some(Terminator::Switch { cases, .. }) = &mut body.blocks[0].terminator else {
                    unreachable!()
                };
                cases[0].value = 2;
            }
            5 => {
                function.signature.parameters[3] = Type::Scalar(ScalarType::I8);
                let OperationKind::Cast { kind, value, .. } =
                    &mut body.blocks[0].operations[2].kind
                else {
                    unreachable!()
                };
                *kind = CastKind::SignExtend;
                *value = ValueId(3);
            }
            6 => {
                function.signature.parameters[3] = Type::BOOL;
                let OperationKind::Cast { value, .. } = &mut body.blocks[0].operations[2].kind
                else {
                    unreachable!()
                };
                *value = ValueId(3);
            }
            _ => unreachable!(),
        }
        both(&module, None);
    }
}

#[test]
fn one_case_switch_truth_v12_v18_preserves_ordered_two_case_and_closed_other_switch_shapes() {
    for fault in 0..4 {
        let mut module = fixture(1, ScalarType::U32);
        let Some(Terminator::Switch {
            cases,
            default_target,
            ..
        }) = &mut module.functions[0].body.as_mut().unwrap().blocks[0].terminator
        else {
            unreachable!()
        };
        cases.insert(
            0,
            SwitchCase {
                value: 0,
                target: BlockId(30),
                arguments: vec![],
            },
        );
        match fault {
            0 => {}
            1 => cases.reverse(),
            2 => *default_target = BlockId(20),
            3 => cases.push(SwitchCase {
                value: 2,
                target: BlockId(30),
                arguments: vec![],
            }),
            _ => unreachable!(),
        }
        both(&module, (fault == 0).then_some(1));
    }
}

#[test]
fn one_case_switch_truth_v18_keeps_unhandled_typed_integer_switch_closed() {
    let mut module = fixture(1, ScalarType::U32);
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator =
        Some(Terminator::IntegerSwitch {
            selector: ValueId(13),
            cases: vec![crate::IntegerSwitchCase {
                value: Constant::U32(1),
                target: BlockId(20),
                arguments: vec![],
            }],
            default_target: BlockId(30),
            default_arguments: vec![],
        });
    run(&module, true, None, 100_000_000, 100_000_000)
        .0
        .unwrap();
}

#[test]
fn one_case_switch_truth_v12_v18_exact_and_one_short_whole_transaction_budgets() {
    for v18 in [false, true] {
        for (case, ordinal) in [(1, 0), (0, 1)] {
            let module = fixture(case, ScalarType::U32);
            let (result, work, peak) = run(&module, v18, Some(ordinal), 100_000_000, 100_000_000);
            result.unwrap();
            let (result, exact_work, exact_peak) = run(&module, v18, Some(ordinal), work, peak);
            result.unwrap();
            assert_eq!((exact_work, exact_peak), (work, peak));
            let (result, accepted, _) = run(&module, v18, Some(ordinal), work - 1, peak);
            assert!(
                matches!(result, Err(Failure::Resource(ResourceError::Work(error)))
                if error.limit() == work - 1 && error.actual() == work)
            );
            assert!(accepted <= work - 1);
            let (result, _, accepted_peak) = run(&module, v18, Some(ordinal), work, peak - 1);
            assert!(
                matches!(result, Err(Failure::Resource(ResourceError::Storage { actual, limit }))
                if actual == peak && limit == peak - 1)
            );
            assert!(accepted_peak <= peak - 1);
        }
    }
}

#[test]
fn one_case_switch_truth_helper_has_independent_exact_and_one_short_carrier_and_work_cuts() {
    let exact = size_of::<(
        &BasicBlock,
        Option<&Terminator>,
        &Vec<SwitchCase>,
        &[SwitchCase],
        &SwitchCase,
        &BlockId,
        Edge,
        Option<Edge>,
        [usize; 2],
        u64,
        [bool; 2],
    )>() + size_of::<&mut GuardLedger>()
        + 2 * size_of::<std::result::Result<(), ResourceError>>()
        + 2 * size_of::<std::result::Result<usize, ResourceError>>()
        + size_of::<Option<usize>>();
    for (local_bytes, local_work) in [(exact - 1, 8), (exact, 7), (exact, 8)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.reserve_storage(17).unwrap();
        let result = {
            let mut meter = LiveGuardMeter::new(&mut budget, local_work, local_bytes, 0);
            prepay_single_case_switch_truth(&mut meter)
        };
        if local_bytes < exact {
            assert_eq!(
                result,
                Err(ResourceError::Storage {
                    actual: exact,
                    limit: exact - 1
                })
            );
            assert_eq!(budget.storage(), 17);
            assert_eq!(budget.work(), 0);
        } else {
            if local_work < 8 {
                assert!(
                    matches!(result, Err(ResourceError::Work(error)) if error.actual() == 8 && error.limit() == 7)
                );
                assert_eq!(
                    budget.work(),
                    0,
                    "local work denial precedes external debit"
                );
            } else {
                result.unwrap();
                assert_eq!(budget.work(), 8);
            }
            assert_eq!(budget.storage(), 17 + exact);
            budget.release_storage(exact).unwrap();
        }
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(budget.storage(), 17);
        assert_eq!(work.failed_work(), None);
    }
}
