use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    Function, MemoryAccess, Module, Operation, OperationKind as Kind, ScalarType, Signature,
    Terminator, Type, ValueDef, ValueId,
};

const WORK: usize = 1_000_000_000;
const STORAGE: usize = 128 * 1024 * 1024;
const FLOOR: usize = 29;

pub(crate) fn fixture() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let mut block = BasicBlock::new(BlockId(73));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(
                ValueId(1),
                Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite),
            ),
            Kind::Alloca {
                element: scalar.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(1),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), scalar.clone()),
            Kind::Load {
                pointer: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    let mut module = Module::new("policy4-private-forwarding");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![scalar.clone()], vec![scalar]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

fn admit(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    (owner, storage.retained_storage())
}

// Closed fixture evaluator only; production semantics come from both checkers.
fn evaluate(owner: &Owner, argument: u32) -> (u32, usize, usize) {
    let body = owner.module().functions[0].body.as_ref().unwrap();
    let block = &body.blocks[0];
    let mut values = std::collections::BTreeMap::new();
    values.insert(body.parameters[0], argument);
    let mut cell = None;
    let mut stores = 0;
    let mut loads = 0;
    for operation in &block.operations {
        match operation.kind {
            Kind::Alloca { .. } => {}
            Kind::Store { value, .. } => {
                cell = Some(values[&value]);
                stores += 1;
            }
            Kind::Load { .. } => {
                values.insert(operation.results[0].id, cell.unwrap());
                loads += 1;
            }
            Kind::Binary {
                op: BinaryOp::BitOr,
                lhs,
                rhs,
            } => {
                values.insert(operation.results[0].id, values[&lhs] | values[&rhs]);
            }
            _ => panic!("not the fixture's closed evaluator grammar"),
        }
    }
    let Some(Terminator::Return { values: returned }) = &block.terminator else {
        panic!("not Return")
    };
    (values[&returned[0]], stores, loads)
}

#[test]
fn policy4_real_pipeline_retains_c_and_rewrites_actual_o_without_relabeling_occurrences() {
    let (input, input_storage) = admit(&fixture());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR + input_storage).unwrap();
    let output = optimize_checked_canonical_kernel_ir_policy4_v1(&input, &mut budget).unwrap();
    assert_eq!(budget.storage(), FLOOR + input_storage);
    budget.reserve_storage(output.retained_storage()).unwrap();
    assert_eq!(output.intermediate_policy3().report().passes().len(), 8);
    assert_eq!(
        output.native_input_audit_bytes(),
        input.canonical().canonical_bytes()
    );
    assert_eq!(output.forwarding_rows().len(), 1);
    assert_ne!(
        output
            .intermediate_policy3()
            .owner()
            .canonical()
            .canonical_bytes(),
        output.owner().canonical().canonical_bytes()
    );
    assert!(!output.grants_authority());
    for value in [0, 1, 0x8000_0000, 0xaaaa_5555, u32::MAX] {
        assert_eq!(evaluate(&input, value), (value, 1, 1));
        assert_eq!(
            evaluate(output.intermediate_policy3().owner(), value),
            (value, 1, 1)
        );
        assert_eq!(evaluate(output.owner(), value), (value, 1, 0));
    }
    output.replay(&input, &mut budget).unwrap();
    let storage = output.retained_storage();
    drop(output);
    budget.release_storage(storage).unwrap();
    assert_eq!(budget.storage(), FLOOR + input_storage);
}

#[test]
fn policy4_empty_noop_still_executes_both_fixed_stages() {
    let (input, size) = admit(&Module::new("empty-policy4"));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR + size).unwrap();
    let output = optimize_checked_canonical_kernel_ir_policy4_v1(&input, &mut budget).unwrap();
    budget.reserve_storage(output.retained_storage()).unwrap();
    assert!(output.forwarding_rows().is_empty());
    assert_eq!(
        input.canonical().canonical_bytes(),
        output.owner().canonical().canonical_bytes()
    );
    assert_eq!(output.intermediate_policy3().report().passes().len(), 8);
    assert_eq!(
        &output.execution().canonical_bytes()[8..16],
        &[1, 0, 4, 0, 3, 0, 1, 0]
    );
    output.replay(&input, &mut budget).unwrap();
    let retained = output.retained_storage();
    drop(output);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR + size);
}

#[test]
fn policy4_deterministic_output_rows_execution_work_and_peak() {
    let (input, size) = admit(&fixture());
    let mut first_work = Work::new(WORK);
    let mut second_work = Work::new(WORK);
    let mut first = Budget::new(&mut first_work, STORAGE);
    let mut second = Budget::new(&mut second_work, STORAGE);
    first.reserve_storage(FLOOR + size).unwrap();
    second.reserve_storage(FLOOR + size).unwrap();
    let a = optimize_checked_canonical_kernel_ir_policy4_v1(&input, &mut first).unwrap();
    let b = optimize_checked_canonical_kernel_ir_policy4_v1(&input, &mut second).unwrap();
    assert_eq!(
        a.owner().canonical().canonical_bytes(),
        b.owner().canonical().canonical_bytes()
    );
    assert_eq!(
        a.intermediate_policy3()
            .owner()
            .canonical()
            .canonical_bytes(),
        b.intermediate_policy3()
            .owner()
            .canonical()
            .canonical_bytes()
    );
    assert_eq!(a.forwarding_rows(), b.forwarding_rows());
    assert_eq!(
        a.execution().canonical_bytes(),
        b.execution().canonical_bytes()
    );
    assert_eq!(a.retained_storage(), b.retained_storage());
    assert_eq!(first.work(), second.work());
    assert_eq!(first.peak_storage(), second.peak_storage());
    assert_eq!(first.storage(), FLOOR + size);
    assert_eq!(second.storage(), FLOOR + size);
}

#[test]
fn policy4_entry_precharge_and_wrapper_storage_boundaries_are_exact() {
    let (input, size) = admit(&Module::new("boundary"));
    let floor = FLOOR + size;
    let wrapper = size_of::<CheckedCanonicalKernelIrOwnerPolicy4V1>()
        - size_of::<Intermediate>()
        - size_of::<CheckedStoreForwardingOutputV1<'_>>();
    for work_limit in [2, 3] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, floor + wrapper - 1);
        budget.reserve_storage(floor).unwrap();
        let error = optimize_checked_canonical_kernel_ir_policy4_v1(&input, &mut budget)
            .err()
            .unwrap();
        if work_limit == 2 {
            assert!(matches!(error, Error::Resource(Resource::Work(_))));
            assert_eq!(budget.work(), 0);
            assert_eq!(budget.failed_storage(), None);
        } else {
            assert!(matches!(error, Error::Resource(Resource::Storage(_))));
            assert_eq!(budget.work(), 3);
        }
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
    }
}

#[test]
fn policy4_composition_fee_and_full_exact_one_short_boundaries_follow_child_schedules() {
    let (input, size) = admit(&fixture());
    let floor = FLOOR + size;
    // Measure independently tested child stages, then derive only the additive
    // composition schedule from its source. No composite-run calibration.
    let mut child_work = Work::new(WORK);
    let mut child = Budget::new(&mut child_work, STORAGE);
    child.reserve_storage(floor).unwrap();
    let c = optimize_checked_canonical_kernel_ir_policy3_v1(&input, &mut child).unwrap();
    let p3_work = child.work();
    let p3_peak = child.peak_storage() - floor;
    let c_size = c.storage().retained_storage();
    let mut forwarding_work = Work::new(WORK);
    let mut forwarding = Budget::new(&mut forwarding_work, STORAGE);
    forwarding.reserve_storage(floor + c_size).unwrap();
    let f = optimize_checked_store_forwarding_v1(c.owner(), &mut forwarding).unwrap();
    let f_size = f.retained_storage();
    let forwarding_peak = forwarding.peak_storage() - floor - c_size;
    let expected_work = p3_work
        + forwarding.work()
        + 3
        + 2
        + 2 * input.canonical().canonical_bytes().len()
        + POLICY4_EXECUTION_RECORD_BYTES_V1
        + 4;
    let wrapper = size_of::<CheckedCanonicalKernelIrOwnerPolicy4V1>()
        - size_of::<Intermediate>()
        - size_of::<CheckedStoreForwardingOutputV1<'_>>();
    let expected_peak =
        floor + wrapper + p3_peak.max(c_size + forwarding_peak).max(c_size + f_size);
    drop(f);
    drop(c);
    for (work_limit, storage_limit, succeeds) in [
        (expected_work, expected_peak, true),
        (expected_work - 1, expected_peak, false),
        (expected_work, expected_peak - 1, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = optimize_checked_canonical_kernel_ir_policy4_v1(&input, &mut budget);
        if succeeds {
            assert!(result.is_ok());
            assert_eq!(budget.work(), expected_work);
            assert_eq!(budget.peak_storage(), expected_peak);
        } else {
            assert!(result.is_err());
        }
        assert_eq!(budget.storage(), floor);
        if storage_limit < expected_peak {
            assert!(budget.failed_storage().is_some());
        }
        drop(budget);
        if work_limit < expected_work {
            assert!(work.failed_work().is_some());
        }
    }
}

#[test]
fn policy4_replay_rejects_foreign_history_and_semantically_wrong_final_payload() {
    let (input, size) = admit(&fixture());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR + size).unwrap();
    let mut checked = optimize_checked_canonical_kernel_ir_policy4_v1(&input, &mut budget).unwrap();
    budget.reserve_storage(checked.retained_storage()).unwrap();
    let (foreign, foreign_size) = admit(&Module::new("foreign"));
    budget.reserve_storage(foreign_size).unwrap();
    assert!(checked.replay(&foreign, &mut budget).is_err());
    let mut wrong = checked.owner().module().clone();
    let coordinate = checked.forwarding_rows()[0].load;
    let operation = &mut wrong.functions[coordinate.block.function.0 as usize]
        .body
        .as_mut()
        .unwrap()
        .blocks[coordinate.block.block as usize]
        .operations[coordinate.operation as usize];
    let Kind::Binary { lhs, rhs, .. } = operation.kind else {
        panic!("actual forwarded operation")
    };
    operation.kind = Kind::Binary {
        op: BinaryOp::BitXor,
        lhs,
        rhs,
    };
    let (wrong, wrong_size) = admit(&wrong);
    budget.reserve_storage(wrong_size).unwrap();
    // Private module-only corruption tests the independent replay, not a public
    // forging API. Real owner fields remain inaccessible to consumers.
    let original = std::mem::replace(&mut checked.output, wrong);
    assert!(matches!(
        checked.replay(&input, &mut budget),
        Err(Error::Relation(_))
    ));
    drop(original);
}
