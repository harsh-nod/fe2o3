use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, Function, Kernel, LaunchDomain,
    LaunchExtent, Module, Operation, Signature, Terminator, ValueDef, ValueId,
    VerifiedCanonicalKernelIrModuleV12,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticLayoutIdentityV1, SemanticTargetDataLayoutV1};

const INTEGERS: [ScalarType; 8] = [
    ScalarType::I8,
    ScalarType::U8,
    ScalarType::I16,
    ScalarType::U16,
    ScalarType::I32,
    ScalarType::U32,
    ScalarType::I64,
    ScalarType::U64,
];

fn module(from: ScalarType, to: ScalarType, kind: CastKind, helper: bool) -> Module {
    let from = Type::Scalar(from);
    let to = Type::Scalar(to);
    let mut body = BasicBlock::new(BlockId(71));
    body.operations.push(Operation::new(
        vec![ValueDef::new(ValueId(2), to.clone())],
        OperationKind::Cast {
            kind,
            value: ValueId(0),
            to: to.clone(),
        },
    ));
    body.terminator = Some(Terminator::Return {
        values: if helper { vec![ValueId(2)] } else { vec![] },
    });
    let mut result = Module::new("numeric-cast-census");
    if helper {
        let mut root = BasicBlock::new(BlockId(99));
        root.operations.push(Operation::new(
            vec![ValueDef::new(ValueId(2), to.clone())],
            OperationKind::Call {
                callee: "convert".into(),
                arguments: vec![ValueId(0), ValueId(1)],
            },
        ));
        root.terminator = Some(Terminator::Return { values: vec![] });
        result.functions.push(Function::internal_helper(
            "convert",
            Signature::new(vec![from.clone(), from.clone()], vec![to]),
            vec![ValueId(0), ValueId(1)],
            vec![body],
        ));
        result.functions.push(Function::kernel_entry(
            "root",
            Signature::new(vec![from.clone(), from], vec![]),
            vec![ValueId(0), ValueId(1)],
            vec![root],
        ));
    } else {
        result.functions.push(Function::kernel_entry(
            "root",
            Signature::new(vec![from.clone(), from], vec![]),
            vec![ValueId(0), ValueId(1)],
            vec![body],
        ));
    }
    result.kernels.push(Kernel::new(
        "root",
        "root",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    result
}

fn with_inventory(
    module: &Module,
    check: impl FnOnce(&CanonicalKirInventoryV1<'_>, &mut AssertOriginBudgetV1<'_>),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 16 << 20);
    budget.reserve_storage(19).unwrap();
    let (owner, owner_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            module,
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(owner_storage.retained_storage())
        .unwrap();
    let (inventory, inventory_storage) =
        CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    check(&inventory, &mut budget);
    // These are grammar components. The production parent, not this report,
    // owns the source/B/C/O joins and scratch-release transaction.
    budget.release_storage(budget.storage() - floor).unwrap();
    drop(inventory);
    budget
        .release_storage(inventory_storage.retained_storage())
        .unwrap();
    drop(owner);
    budget
        .release_storage(owner_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), 19);
}

#[test]
fn exact_numeric_casts_are_admitted_in_roots_and_retained_scalar_helpers() {
    for integer in INTEGERS {
        for (from, to, kind) in [
            (integer, ScalarType::F32, CastKind::IntegerToFloat),
            (ScalarType::F32, integer, CastKind::FloatToInteger),
        ] {
            for helper in [false, true] {
                with_inventory(&module(from, to, kind, helper), |inventory, budget| {
                    let private = private_memory::check(inventory, 1024, budget).unwrap();
                    let division = unsigned_division::check(
                        inventory,
                        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(
                            [250; 32],
                        )),
                        budget,
                    )
                    .unwrap();
                    let helpers = scalar_helpers::check(inventory, budget).unwrap();
                    census::native(
                        inventory,
                        &private,
                        &division,
                        &helpers,
                        "numeric cast test",
                        |_, _| Ok(false),
                        budget,
                    )
                    .unwrap();
                    let cast = inventory
                        .operations()
                        .iter()
                        .position(|row| matches!(row.operation.kind, OperationKind::Cast { .. }))
                        .unwrap();
                    assert!(native(inventory, cast, budget).unwrap());
                });
            }
        }
    }
}

#[test]
fn exact_numeric_cast_query_has_three_work_no_allocation_and_preserves_floor() {
    with_inventory(
        &module(
            ScalarType::F32,
            ScalarType::I64,
            CastKind::FloatToInteger,
            false,
        ),
        |inventory, outer| {
            let floor = outer.storage();
            for limit in [0, 2, 3] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = AssertOriginBudgetV1::new(&mut work, floor);
                budget.reserve_storage(floor).unwrap();
                let result = native(inventory, 0, &mut budget);
                assert_eq!(budget.storage(), floor);
                if limit == 3 {
                    assert!(result.unwrap());
                    assert_eq!(budget.work(), 3);
                } else {
                    assert!(matches!(
                        result,
                        Err(E::Resource(AssertOriginResourceV1::Work(_)))
                    ));
                    assert_eq!(budget.work(), 0);
                }
            }
        },
    );
}

#[test]
fn f64_narrow_float_and_reinterpretation_do_not_acquire_numeric_cast_permission() {
    for (from, to, kind) in [
        (ScalarType::U32, ScalarType::F64, CastKind::IntegerToFloat),
        (ScalarType::F64, ScalarType::I32, CastKind::FloatToInteger),
        (ScalarType::U16, ScalarType::F16, CastKind::IntegerToFloat),
        (ScalarType::F16, ScalarType::U16, CastKind::FloatToInteger),
        (ScalarType::F32, ScalarType::U32, CastKind::Bitcast),
        (ScalarType::U32, ScalarType::F32, CastKind::Bitcast),
        (ScalarType::F32, ScalarType::F64, CastKind::FloatExtend),
        (ScalarType::F64, ScalarType::F32, CastKind::FloatTruncate),
    ] {
        with_inventory(&module(from, to, kind, true), |inventory, budget| {
            assert!(!native(inventory, 0, budget).unwrap());
            assert!(scalar_helpers::check(inventory, budget).is_err());
        });
    }
}

#[test]
fn malformed_cast_kind_width_and_result_are_refused_before_inventory_permission() {
    for mut candidate in [
        module(
            ScalarType::F32,
            ScalarType::I32,
            CastKind::IntegerToFloat,
            false,
        ),
        module(
            ScalarType::I32,
            ScalarType::F32,
            CastKind::FloatToInteger,
            false,
        ),
        module(
            ScalarType::Bool,
            ScalarType::F32,
            CastKind::IntegerToFloat,
            false,
        ),
        module(
            ScalarType::Index,
            ScalarType::F32,
            CastKind::IntegerToFloat,
            false,
        ),
        module(
            ScalarType::F32,
            ScalarType::Index,
            CastKind::FloatToInteger,
            false,
        ),
    ] {
        assert!(
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrV12::from_module(candidate.clone()).is_err()
        );
        // Mismatched result type is independently rejected even if an attacker
        // tries to attach a plausible integer/F32 destination annotation.
        candidate.functions[0].body.as_mut().unwrap().blocks[0].operations[0].results[0].ty =
            Type::BOOL;
        assert!(fe2o3_kernel_ir::VerifiedCanonicalKernelIrV12::from_module(candidate).is_err());
    }
}
