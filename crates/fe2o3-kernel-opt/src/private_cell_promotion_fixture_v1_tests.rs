use super::*;

pub(super) const WORK: usize = 100_000_000;
pub(super) const STORAGE: usize = 64 << 20;
pub(super) const FLOOR: usize = 43;
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
    let mut module = Module::new("owning-private-cells");
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
pub(super) fn admit(module: &Module) -> (Owner, OutputStorage) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap()
}
pub(super) fn with_input(module: Module, run: impl FnOnce(&Owner, &mut Budget<'_>)) {
    let (input, receipt) = admit(&module);
    drop(module);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget
        .reserve_storage(FLOOR + receipt.retained_storage())
        .unwrap();
    let floor = budget.storage();
    run(&input, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(input);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
pub(super) fn replay(
    owned: &OwnedPrivateCellPromotionContinuationV1,
    input: &Owner,
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    let receipt = {
        let (pair, receipt) = owned.replay_against(input, budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(std::ptr::eq(pair.input(), input));
        assert!(std::ptr::eq(pair.output(), owned.output()));
        assert_eq!(pair.selected_allocations(), owned.selected_allocations());
        assert_eq!(pair.origins(), owned.origins());
        assert!(!pair.grants_authority());
        receipt
    };
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}
pub(super) fn release(owned: OwnedPrivateCellPromotionContinuationV1, budget: &mut Budget<'_>) {
    let receipt = owned.retained_storage();
    drop(owned);
    budget.release_storage(receipt).unwrap();
}

/// Independent test oracle for the fixed single-block integer/cell fixture.
/// Fixture construction/admission/oracle allocations are not service work.
pub(super) fn evaluate(module: &Module, x: u128, y: u128) -> u128 {
    use std::collections::BTreeMap;
    let mut values = BTreeMap::from([
        (ValueId(0), x),
        (ValueId(1), y),
        (ValueId(2), 0),
        (ValueId(3), 0),
    ]);
    let mut pointers = BTreeMap::new();
    let mut cells = BTreeMap::new();
    let block = &module.functions[0].body.as_ref().unwrap().blocks[0];
    for op in &block.operations {
        match op.kind {
            Kind::Constant(Constant::Index(value)) => {
                values.insert(op.results[0].id, u128::from(value));
            }
            Kind::Alloca { .. } => {
                pointers.insert(op.results[0].id, (op.results[0].id, 0u128));
            }
            Kind::GetElementPointer { base, offset } => {
                let (allocation, position) = pointers[&base];
                pointers.insert(op.results[0].id, (allocation, position + values[&offset]));
            }
            Kind::Store { pointer, value, .. } => {
                cells.insert(pointers[&pointer], values[&value]);
            }
            Kind::Load { pointer, .. } => {
                values.insert(op.results[0].id, cells[&pointers[&pointer]]);
            }
            Kind::Binary {
                op: BinaryOp::BitOr,
                lhs,
                rhs,
            } => {
                values.insert(op.results[0].id, values[&lhs] | values[&rhs]);
            }
            _ => panic!("outside private-cell test oracle"),
        }
    }
    let Some(Terminator::Return { values: returned }) = &block.terminator else {
        panic!("fixed return")
    };
    values[&returned[0]]
}
