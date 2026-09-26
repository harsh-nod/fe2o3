use super::*;
use kernel_argument_abi_v18::tests::FixtureKernelAbiV18;

const FLOOR: usize = 43;

fn input() -> (ProductionSemanticSsaOwnerV1, crate::ProductionSourceLaunchRosterV1) {
    fixture(Fixture::ElidedBounds, false)
}

fn slice_space(owner: &ProductionPreRankedKirOwnerV1) -> AddressSpace {
    let Type::Slice(slice) = &owner.executable().module().functions[0].signature.parameters[0] else {
        panic!("original shared slice must retain its physical entry parameter")
    };
    slice.address_space
}

#[test]
fn pre_ranked_profile_is_explicit_and_survives_argument_and_source_replay() {
    for explicit in [false, true] {
        let (ssa, launch) = input();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let owner = if explicit {
            materialize_with_fixture_kernel_abi_v18(ssa, launch,
                ProductionSemanticKirLimitsV1::default(), &mut budget)
        } else {
            ProductionPreRankedKirOwnerV1::try_materialize_with_budget(ssa, launch,
                ProductionSemanticKirLimitsV1::default(), &mut budget)
        }.unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(slice_space(&owner), if explicit { AddressSpace::Global } else { AddressSpace::Generic });
        let retained = owner.retained_analysis_storage_v1();
        budget.reserve_storage(retained).unwrap();
        let mut completed = false;
        owner.with_checked_arguments_v1(SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(0), &mut budget, |_| {
                completed = true;
                Ok(())
            }).unwrap();
        assert!(completed);
        assert_eq!(budget.storage(), FLOOR + retained);
        source_output_replay_v1(&owner).unwrap();
        if let Some(profile) = &owner.kernel_abi {
            let expected = owner.executable_storage().retained_storage()
                + owner.assert_origin_storage().payload_storage()
                + owner.helper_memory_storage_v1().retained_storage()
                + owner.helper_memory.capture.transferred_storage()
                + std::mem::size_of_val(profile) + profile.retained_storage();
            assert_eq!(retained, expected);
        }
        drop(owner);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn pre_ranked_profile_rejects_incomplete_foreign_root_type_ownership_and_offset_proposals() {
    use fe2o3_kernel_descriptor::{DeviceLayoutDescriptorV1, DeviceLayoutRecordV1,
        LogicalArgumentV1, ScalarTypeV1, SourceTypeDescriptorV1, SourceTypeRecordV1,
        ValidName};
    for fault in 0..7 {
        let (ssa, launch) = input();
        let mut proposal = FixtureKernelAbiV18::new(&ssa);
        if fault == 2 {
            proposal.arguments_mut(0)[0].semantic_type_identity =
                fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1::from_sha256([244; 32]);
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
        let mut roots = proposal.roots();
        let foreign = [247; 32];
        match fault {
            0 => roots.clear(),
            1 => roots[0].kernel_binding = &foreign,
            5 => roots[0].export = "foreign_entry",
            6 => roots[0].explicit_argument_bytes = 8,
            _ => {},
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let result = ProductionPreRankedKirOwnerV1::try_materialize_with_kernel_abi_budget_v18(
            ssa, launch, ProductionKernelArgumentAbiInputV18 { roots: &roots },
            ProductionSemanticKirLimitsV1::default(), &mut budget);
        assert!(matches!(result, Err(ProductionPreRankedKirErrorV1::Lowering(
            ProductionSemanticKirErrorV1::Unsupported { detail:
                "kernel argument ABI profile differs from the complete original descriptor/source contract", .. }
        ))), "fault {fault}: {result:?}");
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn pre_ranked_profile_loss_or_foreign_capture_cannot_replay_a_global_entry() {
    for foreign in [false, true] {
        let (ssa, launch) = input();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let mut owner = materialize_with_fixture_kernel_abi_v18(ssa, launch,
            ProductionSemanticKirLimitsV1::default(), &mut budget).unwrap();
        let retained = owner.retained_analysis_storage_v1();
        let mut foreign_profile = None;
        if foreign {
            let (other, _) = fixture(Fixture::Literal(true), false);
            let proposal = FixtureKernelAbiV18::new(&other);
            let roots = proposal.roots();
            budget.reserve_storage(std::mem::size_of::<kernel_argument_abi_v18::CapturedKernelArgumentAbiV18>()).unwrap();
            foreign_profile = Some(kernel_argument_abi_v18::CapturedKernelArgumentAbiV18::capture(
                &other, ProductionKernelArgumentAbiInputV18 { roots: &roots }, &mut budget).unwrap());
        }
        let original_profile = std::mem::replace(&mut owner.kernel_abi, foreign_profile);
        budget.reserve_storage(retained).unwrap();
        let before = budget.storage();
        let mut invoked = false;
        let result = owner.with_checked_arguments_v1(SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(0), &mut budget, |_| {
                invoked = true;
                Ok(())
            });
        assert!(!invoked);
        if foreign {
            assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail:
                "kernel argument ABI profile differs from the complete original descriptor/source contract", .. })));
        } else {
            assert!(matches!(result, Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)));
        }
        let replay = source_output_replay_v1(&owner);
        if foreign {
            assert!(matches!(replay, Err(ProductionSemanticKirErrorV1::Unsupported { detail:
                "kernel argument ABI profile differs from the complete original descriptor/source contract", .. })));
        } else {
            assert!(matches!(replay, Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)));
        }
        assert_eq!(budget.storage(), before);
        owner.kernel_abi = original_profile;
        source_output_replay_v1(&owner).unwrap();
        drop(owner);
        budget.release_storage(budget.storage() - FLOOR).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn pre_ranked_profile_constructor_obeys_exact_and_one_short_resources_and_header_prepay() {
    #[allow(dead_code)]
    struct CaptureHeader {
        source: [u8; 32],
        ssa: ProductionSemanticSsaIdentityV1,
        target: SemanticTargetDataLayoutV1,
        layout: SemanticLayoutIdentityV1,
        roots: Vec<()>,
        arguments: Vec<()>,
        retained: usize,
    }
    assert_eq!(std::mem::size_of::<CaptureHeader>(),
        std::mem::size_of::<kernel_argument_abi_v18::CapturedKernelArgumentAbiV18>());
    let run = |work_limit, storage_limit| {
        let (ssa, launch) = input();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let result = materialize_with_fixture_kernel_abi_v18(ssa, launch,
            ProductionSemanticKirLimitsV1::default(), &mut budget);
        assert_eq!(budget.storage(), FLOOR);
        (result, budget.work(), budget.peak_storage())
    };
    let (reference, required_work, required_storage) = run(WORK, STORAGE);
    drop(reference.unwrap());
    let (exact, seen_work, seen_storage) = run(required_work, required_storage);
    drop(exact.unwrap());
    assert_eq!((seen_work, seen_storage), (required_work, required_storage));
    assert!(run(required_work - 1, required_storage).0.is_err());
    assert!(run(required_work, required_storage - 1).0.is_err());
    let (result, work, _) = run(WORK, FLOOR +
        std::mem::size_of::<CaptureHeader>() - 1);
    assert!(matches!(result, Err(ProductionPreRankedKirErrorV1::Lowering(
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(_))
    ))));
    assert_eq!(work, 0);
}

#[test]
fn pre_ranked_profile_keeps_exact_original_offsets_after_valid_capture() {
    use fe2o3_kernel_descriptor::{DeviceLayoutDescriptorV1, DeviceLayoutRecordV1,
        LogicalArgumentV1, ScalarTypeV1, SourceTypeDescriptorV1, SourceTypeRecordV1,
        ValidName};
    let (ssa, launch) = input();
    let mut proposal = FixtureKernelAbiV18::new(&ssa);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = materialize_with_fixture_kernel_abi_v18(ssa, launch,
        ProductionSemanticKirLimitsV1::default(), &mut budget).unwrap();
    let retained = owner.retained_analysis_storage_v1();
    budget.reserve_storage(retained).unwrap();
    let profile = owner.kernel_abi.as_ref().unwrap();
    profile.matches_original_input(&owner.semantic_ssa,
        ProductionKernelArgumentAbiInputV18 { roots: &proposal.roots() }, &mut budget).unwrap();
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::U32));
    let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::U32));
    let ProductionKernelArgumentAbiKindV18::Descriptor { argument, .. } =
        &mut proposal.arguments_mut(0)[0].kind else { unreachable!() };
    *argument = LogicalArgumentV1::shared_slice(0, ValidName::new("arg0".to_owned()).unwrap(),
        &source, &layout, 8).unwrap();
    let mut changed = proposal.roots();
    changed[0].explicit_argument_bytes = 24;
    let failure = profile.matches_original_input(&owner.semantic_ssa,
        ProductionKernelArgumentAbiInputV18 { roots: &changed }, &mut budget);
    assert!(matches!(failure, Err(ProductionSemanticKirErrorV1::Unsupported { detail:
        "kernel argument ABI profile differs from the complete original descriptor/source contract", .. })));
    source_output_replay_v1(&owner).unwrap();
    assert_eq!(budget.storage(), FLOOR + retained);
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
