use super::*;
include!("canonical_kir_aggregate_occurrences_v30_tests.rs");
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function, Operation, Signature,
    StorageLayoutLimitsV1, StorageLayoutV1,
};

const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 4,
    edges: 4,
    containment_depth: 2,
    object_bytes: 64,
};
fn fixture() -> Module {
    let mut m = Module::new("independent-aggregate-ssa-pair");
    m.storage_layouts.push(StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: LayoutKind::Scalar(ScalarType::U32),
    });
    let mut block = BasicBlock::new(BlockId(100));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(
                ValueId(10),
                Type::pointer(
                    Type::StorageObject(StorageLayoutIdV1(0)),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ),
            Kind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(0)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::new(
            vec![],
            Kind::Storage(Storage::WriteValue {
                address: ValueId(10),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
            Kind::Storage(Storage::ReadValue {
                address: ValueId(10),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(20)],
    });
    m.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32)],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        vec![ValueId(0)],
        vec![block],
    ));
    m
}
fn expected() -> Module {
    let mut m = fixture();
    m.functions[0].body.as_mut().unwrap().blocks[0].operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(21), Type::BOOL),
            Kind::Constant(Constant::Bool(true)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
            Kind::Select {
                condition: ValueId(21),
                true_value: ValueId(0),
                false_value: ValueId(0),
            },
        ),
    ];
    m
}
fn admit(m: &Module) -> (Owner, usize) {
    let mut work = Work::new(20_000_000);
    let mut budget = Budget::new(&mut work, 16 * 1024 * 1024);
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v18(m, LAYOUTS, &mut budget).unwrap();
    (owner, receipt.retained_storage())
}
fn assert_checked(
    input: &Owner,
    output: &Owner,
    witness: &CanonicalKirAggregateSsaWitnessV18,
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    let (checked, receipt) =
        check_canonical_kir_aggregate_ssa_v18(input, output, witness, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert!(std::ptr::eq(checked.input(), input));
    assert!(std::ptr::eq(checked.output(), output));
    assert!(std::ptr::eq(checked.witness(), witness));
    assert!(!checked.grants_authority());
    drop(checked);
    budget.release_storage(receipt.retained_storage()).unwrap();
}
#[test]
fn aggregate_ssa_analysis_rederives_every_claim_and_exact_actual_select() {
    let (input, ib) = admit(&fixture());
    let (output, ob) = admit(&expected());
    let mut work = Work::new(20_000_000);
    let mut budget = Budget::new(&mut work, 16 * 1024 * 1024);
    budget.reserve_storage(ib + ob + 37).unwrap();
    let mut witness = derive_canonical_kir_aggregate_ssa_v18(&input, &mut budget).unwrap();
    budget.reserve_storage(witness.retained_storage()).unwrap();
    assert_checked(&input, &output, &witness, &mut budget);
    let original = witness.rows.actions[2];
    witness.rows.actions[2] = Action::Copy(ValueId(20));
    assert!(matches!(
        check_canonical_kir_aggregate_ssa_v18(&input, &output, &witness, &mut budget),
        Err(Error::Inconsistent(
            "complete deterministic original witness"
        ))
    ));
    witness.rows.actions[2] = original;
    assert_checked(&input, &output, &witness, &mut budget);
    witness.rows.conditions[0] = Some(ValueId(10));
    assert!(matches!(
        check_canonical_kir_aggregate_ssa_v18(&input, &output, &witness, &mut budget),
        Err(Error::Inconsistent(
            "complete deterministic original witness"
        ))
    ));
    witness.rows.conditions[0] = Some(ValueId(21));
    assert_checked(&input, &output, &witness, &mut budget);
    let mut changed = expected();
    changed.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind =
        Kind::Constant(Constant::Bool(false));
    let (changed, cb) = admit(&changed);
    budget.reserve_storage(cb).unwrap();
    assert!(matches!(
        check_canonical_kir_aggregate_ssa_v18(&input, &changed, &witness, &mut budget),
        Err(Error::Inconsistent("exact concrete true condition"))
    ));
    drop(changed);
    budget.release_storage(cb).unwrap();
    assert_checked(&input, &output, &witness, &mut budget);
}
#[test]
fn aggregate_ssa_analysis_rejects_foreign_original_even_with_valid_final_owner() {
    let (input, ib) = admit(&fixture());
    let (output, ob) = admit(&expected());
    let mut other = fixture();
    other.id = "foreign-original".into();
    let (other, fb) = admit(&other);
    let mut work = Work::new(20_000_000);
    let mut budget = Budget::new(&mut work, 16 * 1024 * 1024);
    budget.reserve_storage(ib + ob + fb + 37).unwrap();
    let witness = derive_canonical_kir_aggregate_ssa_v18(&input, &mut budget).unwrap();
    budget.reserve_storage(witness.retained_storage()).unwrap();
    assert!(matches!(
        check_canonical_kir_aggregate_ssa_v18(&other, &output, &witness, &mut budget),
        Err(Error::ForeignInput)
    ));
    assert_checked(&input, &output, &witness, &mut budget);
}

#[test]
fn aggregate_ssa_memory_view_binds_original_slot_reset_read_and_write() {
    use CanonicalKirAggregateSsaMemoryEventV18 as Memory;
    let (input, ib) = admit(&fixture());
    let (output, ob) = admit(&expected());
    let mut work = Work::new(20_000_000);
    let mut budget = Budget::new(&mut work, 16 * 1024 * 1024);
    budget.reserve_storage(ib + ob + 37).unwrap();
    let mut witness = derive_canonical_kir_aggregate_ssa_v18(&input, &mut budget).unwrap();
    budget.reserve_storage(witness.retained_storage()).unwrap();
    let slot = CanonicalKirAggregateSsaMemorySlotV18 {
        allocation: 0,
        layout: StorageLayoutIdV1(0),
        offset: 0,
        ty: CanonicalKirAggregateSsaLeafTypeV18::Scalar(ScalarType::U32),
    };
    assert_eq!(witness.memory_slots(), [Some(slot)]);
    assert_eq!(
        witness.memory_events(),
        [
            Memory::Allocate { allocation: 0 },
            Memory::Write {
                slot: 0,
                value: ValueId(0)
            },
            Memory::Read {
                slot: 0,
                output: ValueId(20),
                replacement: ValueId(0)
            }
        ]
    );
    assert!(witness.memory_parameters().is_empty());
    assert_checked(&input, &output, &witness, &mut budget);
    for fault in 0..5 {
        match fault {
            0 => witness.rows.memory_slots[0].as_mut().unwrap().offset = 4,
            1 => witness.rows.memory_slots[0].as_mut().unwrap().allocation = 1,
            2 => witness.rows.memory_slots[0] = None,
            3 => witness.rows.memory_events[0] = Memory::None,
            _ => {
                witness.rows.memory_events[2] = Memory::Read {
                    slot: 0,
                    output: ValueId(20),
                    replacement: ValueId(20),
                }
            }
        }
        assert!(matches!(
            check_canonical_kir_aggregate_ssa_v18(&input, &output, &witness, &mut budget),
            Err(Error::Inconsistent(
                "complete deterministic original witness"
            ))
        ));
        witness.rows.memory_slots[0] = Some(slot);
        witness.rows.memory_events[0] = Memory::Allocate { allocation: 0 };
        witness.rows.memory_events[2] = Memory::Read {
            slot: 0,
            output: ValueId(20),
            replacement: ValueId(0),
        };
        assert_checked(&input, &output, &witness, &mut budget);
    }
}

#[test]
fn aggregate_ssa_memory_phi_link_is_derived_from_the_actual_batched_plan() {
    let mut original = fixture();
    let template = original.functions[0].body.as_ref().unwrap().blocks[0]
        .operations
        .clone();
    let scalar = Type::Scalar(ScalarType::U32);
    original.functions[0].signature.parameters = vec![scalar.clone(), scalar.clone(), Type::BOOL];
    let body = original.functions[0].body.as_mut().unwrap();
    body.parameters = vec![ValueId(0), ValueId(1), ValueId(2)];
    let branch = |value| Terminator::Branch {
        target: BlockId(103),
        arguments: value,
    };
    let mut entry = BasicBlock::new(BlockId(100));
    entry.operations = vec![template[0].clone()];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(101),
        then_arguments: vec![],
        else_target: BlockId(102),
        else_arguments: vec![],
    });
    let mut left = BasicBlock::new(BlockId(101));
    left.operations = vec![template[1].clone()];
    left.terminator = Some(branch(vec![]));
    let mut right = BasicBlock::new(BlockId(102));
    right.operations = vec![template[1].clone()];
    if let Kind::Storage(Storage::WriteValue { value, .. }) = &mut right.operations[0].kind {
        *value = ValueId(1);
    }
    right.terminator = Some(branch(vec![]));
    let mut join = BasicBlock::new(BlockId(103));
    join.operations = vec![template[2].clone()];
    join.terminator = Some(Terminator::Return {
        values: vec![ValueId(20)],
    });
    body.blocks = vec![entry, left, right, join];

    // This final graph is independently constructed, not materializer output.
    let mut final_module = original.clone();
    let blocks = &mut final_module.functions[0].body.as_mut().unwrap().blocks;
    blocks[0].operations = vec![Operation::effect_free(
        ValueDef::new(ValueId(22), Type::BOOL),
        Kind::Constant(Constant::Bool(true)),
    )];
    for (index, value) in [(1, 0), (2, 1)] {
        blocks[index].operations.clear();
        blocks[index].terminator = Some(branch(vec![ValueId(value)]));
    }
    blocks[3].parameters = vec![ValueDef::new(ValueId(21), scalar)];
    blocks[3].operations = vec![Operation::effect_free(
        ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
        Kind::Select {
            condition: ValueId(22),
            true_value: ValueId(21),
            false_value: ValueId(21),
        },
    )];
    let (input, ib) = admit(&original);
    let (output, ob) = admit(&final_module);
    let mut work = Work::new(20_000_000);
    let mut budget = Budget::new(&mut work, 16 * 1024 * 1024);
    budget.reserve_storage(ib + ob + 37).unwrap();
    let mut witness = derive_canonical_kir_aggregate_ssa_v18(&input, &mut budget).unwrap();
    budget.reserve_storage(witness.retained_storage()).unwrap();
    assert_eq!(
        witness.memory_parameters(),
        [CanonicalKirAggregateSsaMemoryParameterV18 {
            parameter: 0,
            slot: 0
        }]
    );
    assert_checked(&input, &output, &witness, &mut budget);
    witness.rows.memory_parameters[0].slot = 1;
    assert!(matches!(
        check_canonical_kir_aggregate_ssa_v18(&input, &output, &witness, &mut budget),
        Err(Error::Inconsistent(
            "complete deterministic original witness"
        ))
    ));
    witness.rows.memory_parameters[0].slot = 0;
    assert_checked(&input, &output, &witness, &mut budget);
    witness.rows.memory_parameters[0].parameter = 1;
    assert!(matches!(
        check_canonical_kir_aggregate_ssa_v18(&input, &output, &witness, &mut budget),
        Err(Error::Inconsistent(
            "complete deterministic original witness"
        ))
    ));
    witness.rows.memory_parameters[0].parameter = 0;
    assert_checked(&input, &output, &witness, &mut budget);
}
