use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate, ComparePredicate, Constant, Function,
    MemoryAccess, Module, Operation, Signature, Terminator, ValueDef, ValueId,
};
const WORK: usize = 500_000_000;
const STORAGE: usize = 128 << 20;
fn u32_ty() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn block(id: u32, term: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.terminator = Some(term);
    block
}
fn branch(id: u32, args: &[u32]) -> Terminator {
    Terminator::Branch {
        target: BlockId(id),
        arguments: args.iter().copied().map(ValueId).collect(),
    }
}
fn conditional(value: u32, yes: u32, no: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(value),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    }
}
fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn site(block: u32, operation: u32) -> Site {
    Site {
        block: Block {
            function: FunctionCoordinate(0),
            block,
        },
        operation,
    }
}
fn fixture() -> Module {
    let mut entry = block(10, branch(20, &[10]));
    entry.operations = vec![
        op(10, u32_ty(), OperationKind::Constant(Constant::U32(0))),
        op(11, u32_ty(), OperationKind::Constant(Constant::U32(1))),
    ];
    let mut header = block(20, conditional(21, 30, 40));
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
    let mut body = block(30, branch(20, &[32]));
    body.operations = vec![
        op(30, u32_ty(), OperationKind::Constant(Constant::U32(7))),
        op(
            31,
            u32_ty(),
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(1),
                rhs: ValueId(30),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(31),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        op(
            32,
            u32_ty(),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(20),
                rhs: ValueId(11),
            },
        ),
    ];
    let mut module = Module::new("independent-licm-pair");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![
                u32_ty(),
                u32_ty(),
                Type::pointer(u32_ty(), AddressSpace::Global, AccessMode::ReadWrite),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![
            entry,
            header,
            body,
            block(40, Terminator::Return { values: vec![] }),
        ],
    ));
    module
}
fn admit(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    (owner, receipt.retained_storage())
}
fn prescribed(input: &Module) -> (Module, Vec<Row>) {
    let mut output = input.clone();
    let body = output.functions[0].body.as_mut().unwrap();
    let first = body.blocks[2].operations.remove(0);
    let second = body.blocks[2].operations.remove(0);
    body.blocks[0].operations.extend([first, second]);
    let rows = vec![
        Row {
            input: site(0, 0),
            output: site(0, 0),
            hoist: None,
        },
        Row {
            input: site(0, 1),
            output: site(0, 1),
            hoist: None,
        },
        Row {
            input: site(1, 0),
            output: site(1, 0),
            hoist: None,
        },
        Row {
            input: site(2, 0),
            output: site(0, 2),
            hoist: Some(CanonicalKirLicmHoistV1 {
                header: site(1, 0).block,
                sequence: 0,
            }),
        },
        Row {
            input: site(2, 1),
            output: site(0, 3),
            hoist: Some(CanonicalKirLicmHoistV1 {
                header: site(1, 0).block,
                sequence: 1,
            }),
        },
        Row {
            input: site(2, 2),
            output: site(2, 0),
            hoist: None,
        },
        Row {
            input: site(2, 3),
            output: site(2, 1),
            hoist: None,
        },
    ];
    (output, rows)
}
fn relation(input: &Module, output: &Module, rows: &[Row]) -> Result<()> {
    let (a, ac) = admit(input);
    let (b, bc) = admit(output);
    let sibling = vec![0xa5u8; 53];
    let floor = ac + bc + sibling.capacity();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    let result = check_canonical_kir_licm_v1(&a, &b, rows, Limits::default(), &mut budget).map(
        |(pair, receipt)| {
            assert!(std::ptr::eq(pair.input(), &a));
            assert!(std::ptr::eq(pair.output(), &b));
            assert_eq!(pair.origins(), rows);
            assert!(!pair.grants_authority());
            assert_eq!(
                receipt.retained_storage(),
                size_of::<CheckedCanonicalKirLicmV1<'_>>()
            );
        },
    );
    assert_eq!(budget.storage(), floor);
    assert_eq!(sibling, vec![0xa5; 53]);
    result
}

#[test]
fn prescribed_pair_checks_chain_complete_lineage_and_preserved_effects() {
    let original = fixture();
    let (output, rows) = prescribed(&original);
    relation(&original, &output, &rows).unwrap();
    assert_eq!(
        output.functions[0].body.as_ref().unwrap().blocks[2]
            .operations
            .len(),
        2
    );
}

#[test]
fn pair_rejects_missing_duplicate_reordered_and_foreign_lineage() {
    let original = fixture();
    let (output, rows) = prescribed(&original);
    let mut missing = rows.clone();
    missing.pop();
    assert_eq!(
        relation(&original, &output, &missing),
        Err(Error::Mismatch("complete operation cardinality"))
    );
    let mut duplicate = rows.clone();
    duplicate[4].output = duplicate[3].output;
    assert_eq!(
        relation(&original, &output, &duplicate),
        Err(Error::Mismatch("duplicate output operation"))
    );
    let mut reordered = rows.clone();
    reordered.swap(0, 1);
    assert_eq!(
        relation(&original, &output, &reordered),
        Err(Error::Mismatch("original row order"))
    );
    let mut gap = rows.clone();
    gap[4].hoist.as_mut().unwrap().sequence = 3;
    assert_eq!(
        relation(&original, &output, &gap),
        Err(Error::Mismatch("contiguous movement sequence"))
    );
    let mut foreign = rows.clone();
    foreign[3].hoist.as_mut().unwrap().header.block = 3;
    assert_eq!(
        relation(&original, &output, &foreign),
        Err(Error::Mismatch("complete checked movement roster"))
    );
}

#[test]
fn pair_rejects_valid_payload_cfg_and_preheader_order_mutations() {
    let original = fixture();
    let (output, rows) = prescribed(&original);
    let mut changed = output.clone();
    changed.functions[0].body.as_mut().unwrap().blocks[0].operations[2].kind =
        OperationKind::Constant(Constant::U32(9));
    assert_eq!(
        relation(&original, &changed, &rows),
        Err(Error::Mismatch("exact operation payload and ValueIds"))
    );
    let mut changed = output.clone();
    changed.id = "different".into();
    assert_eq!(
        relation(&original, &changed, &rows),
        Err(Error::Mismatch("module payload"))
    );
    let mut changed = output.clone();
    changed.functions[0].body.as_mut().unwrap().blocks[1].terminator =
        Some(conditional(21, 40, 30));
    assert_eq!(
        relation(&original, &changed, &rows),
        Err(Error::Mismatch(
            "exact CFG, parameters and edge occurrences"
        ))
    );
    let mut changed = output.clone();
    let mut mapping = rows.clone();
    changed.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .swap(1, 2);
    mapping[1].output.operation = 2;
    mapping[3].output.operation = 1;
    assert_eq!(
        relation(&original, &changed, &mapping),
        Err(Error::Mismatch(
            "unchanged retained order before appended hoists"
        ))
    );
}

#[test]
fn pair_rejects_freshly_admitted_arithmetic_shift_and_memory_hoists() {
    for kind in [
        OperationKind::Binary {
            op: BinaryOp::Add,
            lhs: ValueId(1),
            rhs: ValueId(11),
        },
        OperationKind::Binary {
            op: BinaryOp::Divide,
            lhs: ValueId(1),
            rhs: ValueId(11),
        },
        OperationKind::Binary {
            op: BinaryOp::ShiftLeft,
            lhs: ValueId(1),
            rhs: ValueId(11),
        },
        OperationKind::Load {
            pointer: ValueId(2),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ] {
        let mut original = fixture();
        original.functions[0].body.as_mut().unwrap().blocks[2].operations[0].kind = kind;
        let (output, rows) = prescribed(&original);
        assert_eq!(
            relation(&original, &output, &rows),
            Err(Error::Mismatch(
                "eligible total operation and exact loop/preheader"
            ))
        );
    }
    for scalar_type in [
        ScalarType::Index,
        ScalarType::F16,
        ScalarType::Bf16,
        ScalarType::F32,
        ScalarType::F64,
    ] {
        assert!(!scalar(&Type::Scalar(scalar_type)));
    }
}

#[test]
fn pair_rejects_non_dominating_speculation_and_wrong_dependency_order() {
    // A real dominating loop-body value cannot become available in its preheader.
    let mut original = fixture();
    original.functions[0].body.as_mut().unwrap().blocks[2].operations[1].kind =
        OperationKind::Binary {
            op: BinaryOp::BitXor,
            lhs: ValueId(20),
            rhs: ValueId(30),
        };
    let (output, _) = prescribed(&original);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    assert!(
        matches!(Owner::from_module_ref_with_verification_budget_v12(&output, &mut budget),
        Err(fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12::Verification(ref e))
        if e.contains(fe2o3_kernel_ir::DiagnosticCode::NonDominatingUse)),
        "a malformed SSA output cannot be passed to the independent pair API"
    );
    let original = fixture();
    let (output, mut rows) = prescribed(&original);
    rows[3].hoist.as_mut().unwrap().sequence = 1;
    rows[4].hoist.as_mut().unwrap().sequence = 0;
    assert_eq!(
        relation(&original, &output, &rows),
        Err(Error::Mismatch("preheader append order"))
    );
}

#[test]
fn pair_exact_work_and_initial_storage_denials_keep_history_and_siblings() {
    let original = fixture();
    let (output, rows) = prescribed(&original);
    let (a, ac) = admit(&original);
    let (b, bc) = admit(&output);
    let sibling = vec![0xa5u8; 37];
    let floor = ac + bc + sibling.capacity();
    let mut baseline = Work::new(WORK);
    let (needed, peak) = {
        let mut budget = Budget::new(&mut baseline, STORAGE);
        budget.reserve_storage(floor).unwrap();
        check_canonical_kir_licm_v1(&a, &b, &rows, Limits::default(), &mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        (budget.work(), budget.peak_storage())
    };
    for limit in [needed, needed - 1] {
        let mut work = Work::new(limit);
        {
            let mut budget = Budget::new(&mut work, peak);
            budget.reserve_storage(floor).unwrap();
            let result = check_canonical_kir_licm_v1(&a, &b, &rows, Limits::default(), &mut budget);
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
        assert_eq!(work.failed_work(), (limit != needed).then_some(needed));
    }
    let denied = floor + size_of::<Meter<'_, '_>>() + size_of::<CheckedCanonicalKirLicmV1<'_>>();
    let mut work = Work::new(WORK);
    {
        let mut budget = Budget::new(&mut work, denied - 1);
        budget.reserve_storage(floor).unwrap();
        let result = check_canonical_kir_licm_v1(&a, &b, &rows, Limits::default(), &mut budget);
        assert!(
            matches!(result, Err(Error::Resource(Resource::Storage(ref e))) if e.actual() == denied && e.limit() == denied - 1)
        );
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), Some(denied));
        assert_eq!(budget.peak_storage(), floor + size_of::<Meter<'_, '_>>());
        assert_eq!(budget.work(), 0);
    }
    assert_eq!(work.failed_work(), None);
    assert_eq!(sibling, vec![0xa5; 37]);
}
