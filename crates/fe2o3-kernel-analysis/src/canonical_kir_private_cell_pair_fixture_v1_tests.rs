use super::*;

pub(super) const WORK: usize = 100_000_000;
pub(super) const STORAGE: usize = 64 << 20;
pub(super) const FLOOR: usize = 37;
pub(super) const TYPES: [ScalarType; 8] = [
    ScalarType::I8,
    ScalarType::U8,
    ScalarType::I16,
    ScalarType::U16,
    ScalarType::I32,
    ScalarType::U32,
    ScalarType::I64,
    ScalarType::U64,
];

pub(super) fn coord(operation: u32) -> Coordinate {
    location(0, 0, operation)
}
pub(super) fn location(function: u32, block: u32, operation: u32) -> Coordinate {
    Coordinate {
        block: Block {
            function: FunctionCoordinate(function),
            block,
        },
        operation,
    }
}
pub(super) fn ops(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}
pub(super) fn constant(id: u32, value: u64) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::INDEX),
        Kind::Constant(Constant::Index(value)),
    )
}
pub(super) fn fixture(scalar: ScalarType, count: Option<u64>) -> Module {
    let ty = Type::Scalar(scalar);
    let bytes = u32::from(scalar.bit_width().unwrap_or(64) / 8).max(1);
    let pointer = Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let access = MemoryAccess::new(AddressSpace::Private, bytes);
    let mut block = BasicBlock::new(BlockId(91));
    block.operations = vec![
        constant(10, count.unwrap_or(1)),
        Operation::effect_free(
            ValueDef::new(ValueId(11), pointer.clone()),
            Kind::Alloca {
                element: ty.clone(),
                count: count.map(|_| ValueId(10)),
                address_space: AddressSpace::Private,
                alignment: bytes,
            },
        ),
        constant(12, 0),
        Operation::effect_free(
            ValueDef::new(ValueId(13), pointer),
            Kind::GetElementPointer {
                base: ValueId(11),
                offset: ValueId(12),
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(13),
                value: ValueId(0),
                access,
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(11),
                value: ValueId(1),
                access,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(14), ty.clone()),
            Kind::Load {
                pointer: ValueId(13),
                access,
            },
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(14)],
    });
    let mut module = Module::new("private-cell-pair");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![ty.clone(), ty.clone(), Type::INDEX, Type::BOOL],
            vec![ty],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![block],
    ));
    module
}

pub(super) type CopySpec = (Coordinate, Coordinate, Coordinate, ValueId);

/// Explicit test oracle only: the caller lists every changed occurrence.
/// It never invokes the production census or checker to decide what changes.
pub(super) fn candidate(
    input: &Module,
    removed: &[Coordinate],
    copies: &[CopySpec],
) -> (Module, Vec<Origin>) {
    let mut output = input.clone();
    let mut origins = Vec::new();
    for (f, function) in output.functions.iter_mut().enumerate() {
        let Some(body) = &mut function.body else {
            continue;
        };
        for (b, block) in body.blocks.iter_mut().enumerate() {
            let mut kept = Vec::new();
            for (o, mut operation) in std::mem::take(&mut block.operations)
                .into_iter()
                .enumerate()
            {
                let input = location(f as u32, b as u32, o as u32);
                if removed.contains(&input) {
                    continue;
                }
                let kind = if let Some((_, allocation, previous_store, stored_value)) =
                    copies.iter().find(|row| row.0 == input)
                {
                    operation.kind = Kind::Binary {
                        op: BinaryOp::BitOr,
                        lhs: *stored_value,
                        rhs: *stored_value,
                    };
                    OriginKind::LoadCopy {
                        allocation: *allocation,
                        previous_store: *previous_store,
                        stored_value: *stored_value,
                    }
                } else {
                    OriginKind::Retained
                };
                origins.push(Origin {
                    input,
                    output: location(f as u32, b as u32, kept.len() as u32),
                    kind,
                });
                kept.push(operation);
            }
            block.operations = kept;
        }
    }
    (output, origins)
}
pub(super) fn promoted(input: &Module) -> (Module, Vec<Origin>) {
    candidate(
        input,
        &[coord(1), coord(3), coord(4), coord(5)],
        &[(coord(6), coord(1), coord(5), ValueId(1))],
    )
}
pub(super) fn admit(module: &Module) -> Owner {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    Owner::from_module_ref_with_verification_budget_v12(module, &mut budget)
        .unwrap()
        .0
}
pub(super) fn run(
    input: &Owner,
    output: &Owner,
    selected: &[Coordinate],
    rows: &[Origin],
    work_limit: usize,
    storage_limit: usize,
) -> (Result<()>, usize, usize) {
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = {
        check_canonical_kir_private_cell_promotion_v1(
            input,
            output,
            selected,
            rows,
            Limits::default(),
            &mut budget,
        )
        .map(|(checked, receipt)| {
            assert!(std::ptr::eq(checked.input(), input));
            assert!(std::ptr::eq(checked.output(), output));
            assert!(std::ptr::eq(checked.origins(), rows));
            assert!(std::ptr::eq(checked.selected_allocations(), selected));
            assert!(!checked.grants_authority());
            assert_eq!(
                receipt.retained_storage(),
                size_of::<CheckedCanonicalKirPrivateCellPromotionV1<'_>>()
            );
        })
    };
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.work_ledger_identity_v1() == ledger);
    (result, budget.work(), budget.peak_storage())
}
pub(super) fn accept(input: &Module, output: &Module, selected: &[Coordinate], origins: &[Origin]) {
    assert!(
        run(
            &admit(input),
            &admit(output),
            selected,
            origins,
            WORK,
            STORAGE
        )
        .0
        .is_ok()
    );
}
pub(super) fn refuse(
    input: &Module,
    output: &Module,
    selected: &[Coordinate],
    origins: &[Origin],
    reason: &'static str,
) {
    assert_eq!(
        run(
            &admit(input),
            &admit(output),
            selected,
            origins,
            WORK,
            STORAGE
        )
        .0,
        Err(Error::Mismatch(reason))
    );
}
