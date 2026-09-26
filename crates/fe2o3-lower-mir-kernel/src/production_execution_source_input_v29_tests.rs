use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
use fe2o3_pliron::{
    ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
};

const LIMIT: usize = 1_000_000;

fn ordinary_owner(exact_v29: bool) -> ProductionSemanticSsaOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([10; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([11; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([12; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([13; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([14; 32]),
        provenance.clone(),
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([15; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap(),
        vec![SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([16; 32]),
            unit,
            SemanticLocalRoleV1::Return,
            provenance.clone(),
        )],
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([17; 32]),
                provenance.clone(),
                vec![],
                SemanticTerminatorV1::new(provenance, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"ordinary_census".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([18; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    None,
                )
                .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let unit_type = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Unit,
    );
    let request = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        vec![unit_type],
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(0),
        )],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap();
    let admitted = if exact_v29 {
        request.admit_exact_v29(SemanticMirLimitsV1::default())
    } else {
        request.admit_current_production(SemanticMirLimitsV1::default())
    }
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn launch(owner: &ProductionSemanticSsaOwnerV1) -> ProductionSourceLaunchRosterV1 {
    ProductionSourceLaunchRosterV1::try_new(
        owner.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "ordinary_census",
            [18; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap()
}

#[test]
fn ordinary_census_preserves_original_profile_and_needs_no_context_root() {
    for exact_v29 in [false, true] {
        let owner = ordinary_owner(exact_v29);
        let original = *owner.source_semantic_sha256();
        let version = owner.source_semantic().wire_version();
        assert_eq!(version == SemanticMirWireVersionV1::V29, exact_v29);
        let launch = launch(&owner);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut visited = false;
        with_checked_execution_source_v29(
            &owner,
            &launch,
            ProductionExecutionSourceInputV29 {
                semantic_sha256: &original,
                roots: &[],
                classes: &[ProductionScopeCallableCandidateV29::Ordinary],
                events: &[],
            },
            &mut budget,
            |_, _| {
                visited = true;
                Ok(())
            },
        )
        .unwrap();
        assert!(!visited);
        assert_eq!(owner.source_semantic_sha256(), &original);
        assert_eq!(owner.source_semantic().wire_version(), version);
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn ordinary_census_checks_full_callable_source_and_event_rows() {
    let owner = ordinary_owner(false);
    let launch = launch(&owner);
    let original = *owner.source_semantic_sha256();
    let wrong_sha = [0; 32];
    let ordinary = [ProductionScopeCallableCandidateV29::Ordinary];
    let provider = [ProductionScopeCallableCandidateV29::Provider {
        function: SemanticFunctionIdV1::from_index(0),
        identity: owner.source_semantic().functions()[0].identity(),
    }];
    let fabricated = [ProductionScopeEventCandidateV29 {
        function: SemanticFunctionIdV1::from_index(0),
        block: SemanticBlockIdV1::from_index(0),
        statement_count: 0,
        kind: ProductionScopeEventKindV29::Return,
    }];
    for fault in 0..4 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let result = check_census(
            &owner,
            &launch,
            ProductionExecutionSourceInputV29 {
                semantic_sha256: if fault == 0 { &wrong_sha } else { &original },
                roots: &[],
                classes: match fault {
                    1 => &[],
                    2 => &provider,
                    _ => &ordinary,
                },
                events: if fault == 3 { &fabricated } else { &[] },
            },
            &mut budget,
        );
        assert!(result.is_err(), "fault {fault}");
        assert_eq!(budget.storage(), 0);
        check_census(
            &owner,
            &launch,
            ProductionExecutionSourceInputV29 {
                semantic_sha256: &original,
                roots: &[],
                classes: &ordinary,
                events: &[],
            },
            &mut budget,
        )
        .unwrap();
    }
}

#[test]
fn ordinary_census_work_limit_is_checked_without_changing_owner_or_floor() {
    let owner = ordinary_owner(false);
    let launch = launch(&owner);
    let probe = |limit| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(37).unwrap();
        let result = check_census(
            &owner,
            &launch,
            ProductionExecutionSourceInputV29 {
                semantic_sha256: owner.source_semantic_sha256(),
                roots: &[],
                classes: &[ProductionScopeCallableCandidateV29::Ordinary],
                events: &[],
            },
            &mut budget,
        );
        assert_eq!(budget.storage(), 37);
        (result, budget.work())
    };
    let (result, exact) = probe(LIMIT);
    result.unwrap();
    assert!(exact >= 32 + owner.source_semantic().types().len());
    probe(exact).0.unwrap();
    assert!(matches!(
        probe(exact - 1).0,
        Err(Error::Resource(Resource::Work(_)))
    ));
}
