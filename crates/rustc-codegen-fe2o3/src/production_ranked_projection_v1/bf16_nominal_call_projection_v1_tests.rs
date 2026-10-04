//! Synthetic effect/resource controls only. Genuine source/canonical positives
//! run through the production leaf's hook in the existing CPU ladder.
use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function, FunctionId, Kernel,
    LaunchDomain, LaunchExtent, MatrixOperation, Module, Operation, ScalarType, Signature,
    Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrModuleV12, WorkgroupSize,
};
use std::cell::Cell;
const LIMIT: usize = 100_000_000;
const FLOOR: usize = 23;

fn synthetic() -> (VerifiedCanonicalKernelIrModuleV12, usize) {
    let mut end = BasicBlock::new(BlockId(3));
    end.terminator = Some(Terminator::Return { values: vec![] });
    let root = Function::kernel_entry(
        "n2_synthetic_root",
        Signature::new(vec![], vec![]),
        vec![],
        vec![end],
    );
    let mut body = BasicBlock::new(BlockId(7));
    body.operations.push(Operation::new(
        (20..24)
            .map(|i| ValueDef::new(ValueId(i), Type::F32))
            .collect(),
        OperationKind::Matrix(
            MatrixOperation::multiply_accumulate(
                [ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
                [ValueId(4), ValueId(5), ValueId(6), ValueId(7)],
                [ValueId(8), ValueId(9), ValueId(10), ValueId(11)],
            )
            .with_declared_tensor_layout(
                TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
                    .with_zero_filled_predicate_inputs(),
            ),
        ),
    ));
    body.terminator = Some(Terminator::Return {
        values: (20..24).map(ValueId).collect(),
    });
    let helper = Function::internal_helper(
        "n2_synthetic_helper",
        Signature::new(
            (0..12)
                .map(|i| {
                    Type::Scalar(if i < 8 {
                        ScalarType::Bf16
                    } else {
                        ScalarType::F32
                    })
                })
                .collect(),
            vec![Type::F32; 4],
        ),
        (0..12).map(ValueId).collect(),
        vec![body],
    );
    let mut module = Module::new("n2_synthetic_effects");
    module.functions.extend([root, helper]);
    let mut kernel = Kernel::new(
        "n2_synthetic_root",
        FunctionId::new("n2_synthetic_root"),
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &module,
            &mut budget,
        )
        .unwrap();
    (owner, receipt.retained_storage())
}

#[test]
fn complete_empty_mfma_keeps_nominal_tensor_convergence_and_result_obligations() {
    let (owner, retained) = synthetic();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR + retained).unwrap();
    let (inventory, receipt) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let floor = budget.storage();
    with_effect_report(&inventory, &mut budget, |effects, budget| {
        require_effect_inventory(&inventory, effects, budget)?;
        let helper = inventory
            .function_for_name("n2_synthetic_helper", budget)
            .unwrap()
            .unwrap();
        let decision = effects.decision(helper.coordinate, budget).unwrap();
        assert_eq!(decision, Decision::CompleteEmpty);
        assert_eq!(
            require_nominal_decision(decision, budget)?,
            RequiredNominalProjectionV1::CallerCapabilitiesTensorLayoutFullWaveAndExactResults
        );
        let matrix = &inventory.operations()[helper.operations.start];
        assert!(matrix.effects.is_empty());
        assert!(matrix.compiler_ordering().is_empty());
        assert!(matches!(matrix.operation.kind, OperationKind::Matrix(_)));
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
    // The product type above is not DefinedCallableEmptyEffectDecisionV1 and
    // there is intentionally no conversion into either existing empty category.
    for decision in [Decision::Incomplete, Decision::CompleteNonempty] {
        assert!(require_nominal_decision(decision, &mut budget).is_err());
    }
}

#[test]
fn equal_inventory_contents_are_not_report_inventory_identity() {
    let (owner, retained) = synthetic();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(retained).unwrap();
    let (first, first_receipt) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(first_receipt.retained_storage())
        .unwrap();
    let (second, second_receipt) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(second_receipt.retained_storage())
        .unwrap();
    with_effect_report(&first, &mut budget, |effects, budget| {
        require_effect_inventory(&first, effects, budget)?;
        assert_eq!(
            require_effect_inventory(&second, effects, budget),
            Err(Error::Unavailable(
                "nominal effects belong to another inventory"
            ))
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn scoped_effect_report_exact_and_one_short_work_storage_preserve_prefix() {
    let (owner, retained) = synthetic();
    let mut setup_work = Work::new(LIMIT);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    setup.reserve_storage(retained).unwrap();
    let (inventory, receipt) = CanonicalKirInventoryV1::derive(&owner, &mut setup).unwrap();
    setup.reserve_storage(receipt.retained_storage()).unwrap();
    // Isolated synthetic accounting controls, not a source phase ledger.
    let incoming = FLOOR + retained + receipt.retained_storage();
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(incoming).unwrap();
        let entered = Cell::new(false);
        let result = with_effect_report(&inventory, &mut budget, |effects, budget| {
            require_effect_inventory(&inventory, effects, budget)?;
            entered.set(true);
            Ok(())
        });
        assert_eq!(budget.storage(), incoming);
        (
            result,
            entered.get(),
            budget.work(),
            budget.peak_storage(),
            budget.failed_work(),
            budget.failed_storage(),
        )
    };
    let measured = run(LIMIT, LIMIT);
    assert_eq!(measured.0, Ok(()));
    assert!(measured.1);
    assert_eq!(run(measured.2, measured.3), measured);
    let short_work = run(measured.2 - 1, measured.3);
    assert!(matches!(
        short_work.0,
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(!short_work.1);
    let short_storage = run(measured.2, measured.3 - 1);
    assert!(matches!(
        short_storage.0,
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert!(!short_storage.1);
}

#[test]
fn effect_report_callback_charges_survive_success_error_and_panic() {
    let (owner, retained) = synthetic();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(retained + FLOOR).unwrap();
    let (inventory, receipt) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    for case in 0..3 {
        let floor = budget.storage();
        let work_before = budget.work();
        let ledger = budget.work_ledger_identity_v1();
        let result = with_effect_report(&inventory, &mut budget, |_, budget| {
            budget.reserve_storage(17)?;
            budget.charge_work(11)?;
            match case {
                0 => Ok(()),
                1 => Err(Error::Unavailable("N2a callback control")),
                _ => panic!("N2a callback panic control"),
            }
        });
        match case {
            0 => assert_eq!(result, Ok(())),
            1 => assert_eq!(result, Err(Error::Unavailable("N2a callback control"))),
            _ => assert_eq!(result, Err(Error::CallbackPanicked)),
        }
        assert_eq!(budget.storage(), floor + 17);
        assert!(budget.work() >= work_before + 11);
        assert!(budget.peak_storage() >= floor + 17);
        assert!(budget.work_ledger_identity_v1() == ledger);
        budget.release_storage(17).unwrap();
    }
}

#[test]
fn effect_report_refuses_debited_or_replaced_ledger_without_repair() {
    let (owner, retained) = synthetic();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(retained + FLOOR).unwrap();
    let (inventory, receipt) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let floor = budget.storage();
    let out = with_effect_report(&inventory, &mut budget, |_, b| {
        b.release_storage(1)?;
        Ok(())
    });
    assert_eq!(out, Err(Error::Resource(Resource::Accounting)));
    assert!(
        budget.storage() > floor,
        "undercut query is not repaired by fabricated refund"
    );
    let mut other_work = Work::new(LIMIT);
    let foreign = Budget::new(&mut other_work, LIMIT);
    let out = with_effect_report(&inventory, &mut budget, move |_, b| {
        *b = foreign;
        Ok(())
    });
    assert_eq!(out, Err(Error::Resource(Resource::Accounting)));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn first_denials_are_sticky_and_typed_effect_errors_are_preserved() {
    let (owner, retained) = synthetic();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(retained).unwrap();
    let (inventory, receipt) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let floor = budget.storage();
    let out = with_effect_report(&inventory, &mut budget, |_, b| {
        let _ = b.reserve_storage(usize::MAX);
        Ok(())
    });
    assert_eq!(out, Err(Error::Resource(Resource::Accounting)));
    assert_eq!(budget.storage(), floor);
    assert!(budget.failed_storage().is_some());
    let entered = Cell::new(false);
    assert!(
        with_effect_report(&inventory, &mut budget, |_, _| {
            entered.set(true);
            Ok(())
        })
        .is_err()
    );
    assert!(!entered.get());
    for e in [
        Resource::Accounting,
        Resource::Allocation,
        Resource::Arithmetic,
    ] {
        assert_eq!(
            effect_error(CanonicalKirCallEffectErrorV1::Resource(e)),
            Error::Resource(e)
        );
    }
}
