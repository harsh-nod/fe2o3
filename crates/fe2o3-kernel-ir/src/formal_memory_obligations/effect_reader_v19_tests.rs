use super::*;
use crate::{
    BasicBlock, CanonicalKernelIrWorkBudgetV1 as Work, Constant, Kernel, Signature,
    StorageLayoutLimitsV1, Terminator, ValueDef, with_canonical_effects_v19,
};
use std::cell::Cell;

const LIMIT: usize = 20_000_000;
const FLOOR: usize = 37;
fn call(name: &str) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Call {
            callee: FunctionId::new(name),
            arguments: vec![],
        },
    )
}
fn block(operations: Vec<Operation>) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}
fn fixture(mode: u8) -> Module {
    let mut module = Module::new("effect_reader_v19");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block(vec![call("leaf"), call("leaf")])],
    ));
    let leaf = match mode {
        1 => vec![
            Operation::effect_free(
                ValueDef::new(
                    ValueId(0),
                    Type::pointer(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Private,
                        AccessMode::ReadWrite,
                    ),
                ),
                OperationKind::Alloca {
                    element: Type::Scalar(ScalarType::U32),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(7)),
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(0),
                    value: ValueId(1),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
            ),
        ],
        3 => vec![call("leaf")],
        _ => vec![],
    };
    module.functions.push(if mode == 2 {
        Function::declaration("leaf", Signature::new(vec![], vec![]))
    } else {
        Function::internal_helper(
            "leaf",
            Signature::new(vec![], vec![]),
            vec![],
            vec![block(leaf)],
        )
    });
    for function in &mut module.functions {
        function.required_capabilities = function.derived_capabilities();
        module
            .required_capabilities
            .extend(function.required_capabilities.iter().cloned());
    }
    module.kernels.push(Kernel::new(
        "root",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}
fn owner(module: &Module) -> (Owner, usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, receipt) = Owner::from_module_ref_with_verification_budget_v18(
        module,
        StorageLayoutLimitsV1 {
            rows: 64,
            edges: 256,
            containment_depth: 32,
            object_bytes: 4096,
        },
        &mut budget,
    )
    .unwrap();
    (owner, receipt.retained_storage())
}
fn launch() -> ExplicitLaunchExtent {
    ExplicitLaunchExtent::Exact {
        rank: 1,
        extents: [64, 1, 1],
    }
}
fn legacy(owner: &Owner, launch: ExplicitLaunchExtent) -> FormalMemoryObligationAnalysis {
    let effects = crate::analyze_interprocedural_effects_from_verified_v1(
        crate::verify_module_ref(owner.module()).unwrap(),
    )
    .unwrap();
    derive_kernel_memory_obligations_from_authenticated_module(
        owner.module(),
        &KernelId::new("root"),
        launch,
        FormalIndexWidth::Bits64,
        None,
        None,
        &effects,
    )
    .unwrap()
}

#[test]
fn actual_owner_effect_reader_preserves_pure_memory_declaration_and_recursion_reports() {
    for mode in 0..4 {
        let (owner, retained) = owner(&fixture(mode));
        let expected = legacy(&owner, launch());
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR + retained).unwrap();
        with_canonical_effects_v19(&owner, &mut budget, |scope, budget| {
            let floor = budget.storage();
            for _ in 0..3 {
                let report = derive_with_canonical_effects_legacy_remainder_v19(&owner, scope,
                    &KernelId::new("root"), launch(), FormalIndexWidth::Bits64, budget).unwrap();
                assert_eq!(report, expected);
                assert_eq!(report.is_complete(), mode == 0);
                if mode != 0 {
                    assert_eq!(report.incomplete_reasons().len(), 2);
                    assert!(report.incomplete_reasons().iter().all(|reason| matches!(reason,
                        FormalMemoryIncompleteReason::CallEffectsUnavailable { callee, .. } if callee.as_str() == "leaf")));
                }
                assert_eq!(budget.storage(), floor);
            }
            Ok(())
        }).unwrap();
        assert_eq!(budget.storage(), FLOOR + retained);
    }
}

#[test]
fn effect_reader_does_not_erase_dynamic_launch_or_missing_kernel_refusals() {
    let (owner, retained) = owner(&fixture(0));
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR + retained).unwrap();
    with_canonical_effects_v19(&owner, &mut budget, |scope, budget| {
        let report = derive_with_canonical_effects_legacy_remainder_v19(&owner, scope,
            &KernelId::new("root"), ExplicitLaunchExtent::Unknown, FormalIndexWidth::Bits64, budget).unwrap();
        assert_eq!(report.incomplete_reasons(), &[FormalMemoryIncompleteReason::LaunchExtentUnknown]);
        assert_eq!(report, legacy(&owner, ExplicitLaunchExtent::Unknown));
        assert!(matches!(derive_with_canonical_effects_legacy_remainder_v19(&owner, scope,
            &KernelId::new("absent"), launch(), FormalIndexWidth::Bits64, budget),
            Err(FormalEffectEngineErrorV19::Formal(FormalMemoryObligationError::MissingKernel { kernel })) if kernel.as_str() == "absent"));
        Ok(())
    }).unwrap();
    assert_eq!(budget.storage(), FLOOR + retained);
}

#[test]
fn actual_call_reader_errors_remain_structural_before_a_report_can_be_returned() {
    struct Refuse<'a> {
        expected: &'a Operation,
        reached: &'a Cell<bool>,
    }
    impl EffectReaderV19 for Refuse<'_> {
        type Error = u32;
        fn is_complete_and_pure(&mut self, operation: &Operation) -> Result<bool, u32> {
            assert!(std::ptr::eq(operation, self.expected));
            self.reached.set(true);
            Err(71)
        }
    }
    let (owner, _) = owner(&fixture(0));
    let reached = Cell::new(false);
    let expected = &owner.module().functions[0].body.as_ref().unwrap().blocks[0].operations[0];
    let mut reader = Refuse {
        expected,
        reached: &reached,
    };
    let result = derive_kernel_memory_obligations_with_effect_reader_v19(
        owner.module(),
        &KernelId::new("root"),
        launch(),
        FormalIndexWidth::Bits64,
        None,
        None,
        &mut reader,
    );
    assert!(matches!(
        result,
        Err(FormalEffectEngineErrorV19::Reader(71))
    ));
    assert!(reached.get());
}

#[test]
fn effect_reader_authenticates_scope_owner_and_foreign_budget_before_report_construction() {
    let module = fixture(0);
    let (owner, retained) = owner(&module);
    let (foreign_owner, _) = self::owner(&module);
    for foreign_owner_case in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut foreign_work = Work::new(0);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut foreign = Budget::new(&mut foreign_work, 0);
        foreign.charge_work(19).unwrap_err();
        budget.reserve_storage(FLOOR + retained).unwrap();
        let completed = Cell::new(false);
        let result = with_canonical_effects_v19(&owner, &mut budget, |scope, budget| {
            if foreign_owner_case {
                assert!(matches!(
                    derive_with_canonical_effects_legacy_remainder_v19(
                        &foreign_owner,
                        scope,
                        &KernelId::new("root"),
                        launch(),
                        FormalIndexWidth::Bits64,
                        budget
                    ),
                    Err(FormalEffectEngineErrorV19::Reader(
                        CanonicalEffectErrorV19::ForeignOwner
                    ))
                ));
            } else {
                assert!(matches!(
                    derive_with_canonical_effects_legacy_remainder_v19(
                        &owner,
                        scope,
                        &KernelId::new("root"),
                        launch(),
                        FormalIndexWidth::Bits64,
                        &mut foreign
                    ),
                    Err(FormalEffectEngineErrorV19::Reader(
                        CanonicalEffectErrorV19::Resource(Resource::Accounting)
                    ))
                ));
                assert_eq!(
                    (foreign.work(), foreign.storage(), foreign.failed_work()),
                    (0, 0, Some(19))
                );
                assert!(
                    derive_with_canonical_effects_legacy_remainder_v19(
                        &owner,
                        scope,
                        &KernelId::new("root"),
                        launch(),
                        FormalIndexWidth::Bits64,
                        budget
                    )
                    .unwrap()
                    .is_complete()
                );
            }
            completed.set(true);
            Ok(())
        });
        assert!(completed.get());
        if foreign_owner_case {
            assert_eq!(result, Err(CanonicalEffectErrorV19::ForeignOwner));
        } else {
            result.unwrap();
        }
        assert_eq!(budget.storage(), FLOOR + retained);
    }
}

#[test]
fn metered_effect_lookup_and_adapter_frames_have_cumulative_exact_and_short_boundaries() {
    let (owner, retained) = owner(&fixture(0));
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR + retained).unwrap();
        let completed = Cell::new(false);
        let result = with_canonical_effects_v19(&owner, &mut budget, |scope, budget| {
            let floor = budget.storage();
            for _ in 0..3 {
                match derive_with_canonical_effects_legacy_remainder_v19(
                    &owner,
                    scope,
                    &KernelId::new("root"),
                    launch(),
                    FormalIndexWidth::Bits64,
                    budget,
                ) {
                    Ok(report) => assert!(report.is_complete()),
                    Err(FormalEffectEngineErrorV19::Reader(error)) => return Err(error),
                    Err(FormalEffectEngineErrorV19::Formal(error)) => {
                        panic!("unexpected legacy refusal: {error:?}")
                    }
                }
                assert_eq!(budget.storage(), floor);
            }
            completed.set(true);
            Ok(())
        });
        assert_eq!(result.is_ok(), completed.get());
        assert_eq!(budget.storage(), FLOOR + retained);
        (result, budget.work(), budget.peak_storage())
    };
    // These measured boundaries cover the paid effects service and fixed reader
    // adapter only. The legacy report's other work/allocations are not credited.
    let (result, work, peak) = run(LIMIT, LIMIT);
    result.unwrap();
    run(work, peak).0.unwrap();
    assert!(matches!(
        run(work - 1, peak).0,
        Err(CanonicalEffectErrorV19::Resource(Resource::Work(_)))
    ));
    assert!(matches!(
        run(work, peak - 1).0,
        Err(CanonicalEffectErrorV19::Resource(Resource::Storage(_)))
    ));
}

#[test]
fn paid_reader_denials_cannot_be_swallowed_into_incomplete_or_complete_reports() {
    let (owner, retained) = owner(&fixture(0));
    for storage in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR + retained).unwrap();
        let expected = Cell::new(None);
        let completed = Cell::new(false);
        let result = with_canonical_effects_v19(&owner, &mut budget, |scope, budget| {
            let first = if storage {
                budget.reserve_storage(LIMIT).unwrap_err()
            } else {
                budget.charge_work(LIMIT).unwrap_err()
            };
            expected.set(Some(CanonicalEffectErrorV19::Resource(first)));
            let before = (budget.work(), budget.storage(), budget.peak_storage());
            for _ in 0..3 {
                assert!(matches!(derive_with_canonical_effects_legacy_remainder_v19(
                    &owner, scope, &KernelId::new("root"), launch(), FormalIndexWidth::Bits64, budget,
                ), Err(FormalEffectEngineErrorV19::Reader(error)) if Some(error) == expected.get()));
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    before
                );
            }
            completed.set(true);
            Ok(())
        });
        assert!(completed.get());
        assert_eq!(result, Err(expected.get().unwrap()));
        assert_eq!(budget.storage(), FLOOR + retained);
    }
}
