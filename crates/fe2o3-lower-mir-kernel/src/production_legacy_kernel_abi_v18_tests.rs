use super::*;
use kernel_argument_abi_v18::tests::FixtureKernelAbiV18;
use fe2o3_pliron::{ProductionConstructionV1, ProductionRankedBlockV1,
    ProductionRankedKernelV1, ProductionRankedOperationV1, ProductionRankedTerminatorV1,
    ProductionSessionLimitsV1, compile_ranked_kernel_for_lowering_v1};

fn input() -> ProductionSemanticSsaOwnerV1 {
    fixture_with_blocks(Fixture::ElidedBounds, false, |_, _| {
        vec![block(31, vec![], SemanticTerminatorKindV1::Return)]
    }).0
}

fn receipt(ssa: ProductionSemanticSsaOwnerV1) -> ProductionRankedSemanticProjectionModuleReceiptV1 {
    let kernel = ProductionRankedKernelV1::new("assert_root_0", 0,
        vec![ProductionRankedBlockV1::new(vec![ProductionRankedOperationV1::ExecutionLayout {
            grid_identity: 1, global_extents: [64, 1, 1], workgroup_extents: [64, 1, 1],
            subgroup_size: 64, full_physical_workgroups: true,
        }], ProductionRankedTerminatorV1::Return)]).unwrap();
    let lowering = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("legacy_profile", kernel).unwrap(),
        ProductionSessionLimitsV1::default()).unwrap();
    ProductionRankedSemanticProjectionModuleReceiptV1::from_unvalidated_projection_roster_candidate(
        ssa.into_source_owner().unwrap(), vec![ProductionRankedSemanticProjectionRootV1::new(
            SemanticFunctionIdV1::from_index(0), 1, lowering,
            "func @assert_root_0 {\n}\n".to_owned(), vec![], vec![])]).unwrap()
}

fn create(explicit: bool, budget: &mut AssertOriginBudgetV1<'_>)
    -> Result<ProductionSemanticKirOwnerV1, ProductionSemanticKirErrorV1>
{
    let source = input();
    let proposal = FixtureKernelAbiV18::new(&source);
    let roots = proposal.roots();
    let receipt = receipt(source);
    if explicit {
        ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks_with_kernel_abi_budget_v18(
            receipt, ProductionKernelArgumentAbiInputV18 { roots: &roots },
            ProductionSemanticKirLimitsV1::default(), budget)
    } else {
        ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks(
            receipt, ProductionSemanticKirLimitsV1::default())
    }
}

#[test]
fn legacy_profile_preserves_ranked_first_custody_default_generic_and_exact_replay() {
    for explicit in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let owner = create(explicit, &mut budget).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert!(matches!(&owner.module, RetainedProductionKirModuleV1::Legacy { .. }));
        assert!(owner.retains_mandatory_generic_checks());
        assert!(owner.mir_pliron_translation_validation().is_some());
        let Type::Slice(slice) = &owner.module().functions[0].signature.parameters[0] else {
            panic!("original shared slice remains a slice")
        };
        assert_eq!(slice.address_space, if explicit { AddressSpace::Global } else { AddressSpace::Generic });
        let retained = owner.legacy_kernel_abi_storage_v18().unwrap();
        match &owner.module {
            RetainedProductionKirModuleV1::Legacy { kernel_abi: Some(profile), .. } => {
                assert!(explicit);
                assert_eq!(retained, std::mem::size_of_val(profile) + profile.retained_storage());
            }
            _ => assert_eq!(retained, 0),
        }
        budget.reserve_storage(retained).unwrap();
        let mut completed = false;
        owner.with_checked_arguments_v1(SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(0), &mut budget, |_| {
                completed = true;
                Ok(())
            }).unwrap();
        assert!(completed);
        owner.verify_equivalence_with_budget_v1(&mut budget).unwrap();
        owner.verify_equivalence().unwrap();
        assert_eq!(budget.storage(), FLOOR + retained);
        drop(owner);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn legacy_profile_rejects_missing_foreign_source_type_ownership_and_physical_layout() {
    use fe2o3_kernel_descriptor::{DeviceLayoutDescriptorV1, DeviceLayoutRecordV1,
        LogicalArgumentV1, ScalarTypeV1, SourceTypeDescriptorV1, SourceTypeRecordV1, ValidName};
    for fault in 0..7 {
        let source = input();
        let mut proposal = FixtureKernelAbiV18::new(&source);
        if fault == 2 {
            proposal.arguments_mut(0)[0].semantic_type_identity =
                SemanticTypeIdentityV1::from_sha256([247; 32]);
        }
        if fault == 3 {
            proposal.arguments_mut(0)[0].kind =
                ProductionKernelArgumentAbiKindV18::CompilerLaidOutByValue { offset: 0 };
        }
        if fault == 4 {
            let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::U32));
            let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::U32));
            let ProductionKernelArgumentAbiKindV18::Descriptor { argument, .. } =
                &mut proposal.arguments_mut(0)[0].kind else { unreachable!() };
            *argument = LogicalArgumentV1::shared_slice(0, ValidName::new("arg0".to_owned()).unwrap(),
                &source, &layout, 8).unwrap();
        }
        let foreign = [248; 32];
        let mut roots = proposal.roots();
        match fault {
            0 => roots.clear(), 1 => roots[0].kernel_binding = &foreign,
            5 => roots[0].export = "foreign_root", 6 => roots[0].explicit_argument_bytes = 8,
            _ => {},
        }
        let receipt = receipt(source);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let result = ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks_with_kernel_abi_budget_v18(
            receipt, ProductionKernelArgumentAbiInputV18 { roots: &roots },
            ProductionSemanticKirLimitsV1::default(), &mut budget);
        assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail:
            "kernel argument ABI profile differs from the complete original descriptor/source contract", .. })),
            "fault {fault}: {result:?}");
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn legacy_profile_loss_or_foreign_capture_cannot_replay_global_parameters() {
    for foreign in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        let mut owner = create(true, &mut budget).unwrap();
        let mut replacement = None;
        if foreign {
            let (other, _) = fixture(Fixture::Literal(true), false);
            let proposal = FixtureKernelAbiV18::new(&other);
            let roots = proposal.roots();
            budget.reserve_storage(std::mem::size_of::<kernel_argument_abi_v18::CapturedKernelArgumentAbiV18>()).unwrap();
            replacement = Some(kernel_argument_abi_v18::CapturedKernelArgumentAbiV18::capture(
                &other, ProductionKernelArgumentAbiInputV18 { roots: &roots }, &mut budget).unwrap());
        }
        let original = match &mut owner.module {
            RetainedProductionKirModuleV1::Legacy { kernel_abi, .. } =>
                std::mem::replace(kernel_abi, replacement),
            _ => unreachable!(),
        };
        budget.reserve_storage(std::mem::size_of_val(original.as_ref().unwrap())
            + original.as_ref().unwrap().retained_storage()).unwrap();
        let live = budget.storage();
        let mut called = false;
        let query = owner.with_checked_arguments_v1(SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(0), &mut budget, |_| { called = true; Ok(()) });
        assert!(!called);
        let replay = owner.verify_equivalence_with_budget_v1(&mut budget);
        for result in [query, replay] {
            if foreign {
                assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail:
                    "kernel argument ABI profile differs from the complete original descriptor/source contract", .. })));
            } else {
                assert!(matches!(result, Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)));
            }
        }
        assert_eq!(budget.storage(), live);
        let RetainedProductionKirModuleV1::Legacy { kernel_abi, .. } = &mut owner.module else { unreachable!() };
        *kernel_abi = original;
        owner.verify_equivalence_with_budget_v1(&mut budget).unwrap();
        drop(owner);
        budget.release_storage(live).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn legacy_profile_capture_and_replay_require_exact_work_storage_and_live_custody() {
    let run = |work_limit, storage_limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let result = create(true, &mut budget);
        assert_eq!(budget.storage(), FLOOR);
        (result, budget.work(), budget.peak_storage())
    };
    let (owner, work, peak) = run(WORK, STORAGE);
    drop(owner.unwrap());
    drop(run(work, peak).0.unwrap());
    assert!(matches!(run(work - 1, peak).0, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
        ArgumentResourceV1::Work(_)))));
    assert!(matches!(run(work, peak - 1).0, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
        ArgumentResourceV1::Storage(_)))));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let owner = create(true, &mut budget).unwrap();
    let retained = owner.legacy_kernel_abi_storage_v18().unwrap();
    budget.reserve_storage(retained - 1).unwrap();
    let before_work = budget.work();
    let mut called = false;
    assert!(matches!(owner.with_checked_arguments_v1(SemanticFunctionIdV1::from_index(0),
        SemanticFunctionIdV1::from_index(0), &mut budget, |_| { called = true; Ok(()) }),
        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting))));
    assert!(!called);
    assert!(matches!(owner.verify_equivalence_with_budget_v1(&mut budget),
        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting))));
    assert_eq!(budget.work(), before_work);
    assert_eq!(budget.storage(), retained - 1);
    budget.reserve_storage(1).unwrap();
    owner.verify_equivalence_with_budget_v1(&mut budget).unwrap();
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 0);
}
