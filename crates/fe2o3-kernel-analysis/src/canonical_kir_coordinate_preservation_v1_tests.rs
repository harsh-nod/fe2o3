use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Constant, Function, Operation,
    OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
};

const WORK: usize = 1_000_000_000;
const STORAGE: usize = 1_000_000;
const PRIOR: usize = 7;
const FLOOR: usize = 23;

fn admit(module: &Module) -> Owner {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    Owner::from_module_ref_with_verification_budget_v12(module, &mut budget)
        .unwrap()
        .0
}

fn body(value: u32, id: u32) -> Module {
    let mut block = BasicBlock::new(BlockId(41));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(value)),
    ));
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(id)],
    });
    let mut module = Module::new("coordinates");
    module.functions.push(Function::internal_helper(
        "entry",
        Signature::new(vec![], vec![Type::Scalar(ScalarType::U32)]),
        vec![],
        vec![block],
    ));
    module
}

#[test]
fn complete_borrowed_identity_accepts_canonical_equal_owners_and_rejects_changed_executable_fields()
{
    let input = admit(&body(7, 93));
    let equal = admit(&body(7, 93));
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (checked, storage) =
        check_canonical_kir_coordinate_preservation_v1(&input, &equal, &mut budget).unwrap();
    assert!(std::ptr::eq(checked.input(), &input));
    assert!(std::ptr::eq(checked.output(), &equal));
    assert!(!checked.grants_authority());
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(
        storage.retained_storage(),
        size_of::<CheckedCanonicalKirCoordinatePreservationV1<'_, '_>>()
    );
    drop(checked);
    for candidate in [body(8, 93), body(7, 94)] {
        let other = admit(&candidate);
        assert!(matches!(
            check_canonical_kir_coordinate_preservation_v1(&input, &other, &mut budget),
            Err(Error::Mismatch("function declaration or executable body"))
        ));
        assert_eq!(budget.storage(), FLOOR);
    }
    let mut renamed = body(7, 93);
    renamed.id = Module::new("different").id;
    let other = admit(&renamed);
    assert!(matches!(
        check_canonical_kir_coordinate_preservation_v1(&input, &other, &mut budget),
        Err(Error::Mismatch("module identity or cardinality"))
    ));
}

#[test]
fn capability_extension_is_monotone_but_does_not_identify_a_target() {
    let source = Module::new("m");
    let input = admit(&source);
    let mut augmented = source.clone();
    augmented
        .required_capabilities
        .insert(TargetCapability::WaveWidth(
            fe2o3_kernel_ir::WaveWidth::Wave64,
        ));
    let output = admit(&augmented);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (checked, _) =
        check_canonical_kir_coordinate_preservation_v1(&input, &output, &mut budget).unwrap();
    assert!(!checked.grants_authority());
    drop(checked);
    assert!(matches!(
        check_canonical_kir_coordinate_preservation_v1(&output, &input, &mut budget),
        Err(Error::Mismatch("removed capability"))
    ));
}

#[test]
fn empty_coordinate_check_has_literal_80_work_boundary_and_preserves_first_denial() {
    let input = admit(&Module::new("m"));
    assert_eq!(input.canonical().canonical_bytes().len(), 37);
    // Entry 1 + complete two-payload prepayment 74 + declaration 3 + empty merge 2.
    for allowance in [80, 79] {
        let mut work = Work::new(PRIOR + allowance);
        {
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.charge_work(PRIOR).unwrap();
            budget.reserve_storage(FLOOR).unwrap();
            let result =
                check_canonical_kir_coordinate_preservation_v1(&input, &input, &mut budget);
            if allowance == 80 {
                assert!(result.is_ok());
                assert_eq!(budget.work(), PRIOR + 80);
            } else {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                assert_eq!(budget.work(), PRIOR + 79);
                assert!(budget.charge_work(WORK).is_err());
            }
            assert_eq!(budget.storage(), FLOOR);
        }
        assert_eq!(work.failed_work(), (allowance == 79).then_some(PRIOR + 80));
    }
}

#[test]
fn checked_view_storage_is_admitted_before_complete_payload_comparison() {
    let input = admit(&Module::new("m"));
    let bytes = size_of::<CheckedCanonicalKirCoordinatePreservationV1<'_, '_>>();
    for allowance in [bytes, bytes - 1] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, FLOOR + allowance);
        budget.reserve_storage(FLOOR).unwrap();
        let result = check_canonical_kir_coordinate_preservation_v1(&input, &input, &mut budget);
        if allowance == bytes {
            let (checked, receipt) = result.unwrap();
            assert_eq!(receipt.retained_storage(), bytes);
            assert_eq!(budget.work(), 80);
            drop(checked);
        } else {
            assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
            assert_eq!(budget.work(), 1);
            assert_eq!(budget.failed_storage(), Some(FLOOR + bytes));
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}
