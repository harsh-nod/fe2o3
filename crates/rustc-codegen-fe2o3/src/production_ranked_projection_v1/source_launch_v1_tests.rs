// Source-launch adapter and row-use regressions on the real production path.

fn source_launch_test_semantic_v1(
    first_identity: u8,
    first_binding: u8,
) -> AdmittedInertSemanticMirV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let types = vec![neutral_semantic_types_v1().into_iter().next().unwrap()];
    let mut functions = Vec::new();
    for (ordinal, identity, symbol, binding) in [
        (0_u8, first_identity, "source_launch_first", first_binding),
        (1_u8, 80_u8, "source_launch_second", 0x7a_u8),
    ] {
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(100 + ordinal)),
            SemanticLayoutIdentityV1::from_sha256(bytes(250)),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
        let contract = SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap(),
            ),
            None,
            None,
        )
        .unwrap();
        let blocks = vec![block(
            90 + ordinal,
            vec![],
            SemanticTerminatorKindV1::Return,
        )];
        functions.push(
            SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256(bytes(identity)),
                SemanticFunctionRoleV1::KernelRoot,
                SemanticItemDefinitionIdentityV1::from_sha256(bytes(120 + ordinal)),
                SemanticMonomorphizationIdentityV1::from_sha256(bytes(130 + ordinal)),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(140 + ordinal)),
                SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(150 + ordinal)),
                SemanticSourceProvenanceV1::unavailable(),
                abi,
                vec![local(160 + ordinal, unit, SemanticLocalRoleV1::Return)],
                SemanticBlockIdV1::from_index(0),
                blocks,
            )
            .unwrap()
            .with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(symbol.as_bytes().to_vec()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256(bytes(binding)),
                contract,
            )),
        );
    }
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(1),
        ],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

fn source_launch_test_inputs_v1() -> [ProductionRankedRootInputV1; 2] {
    [
        ranked_root_input_1d("logical_first", 0xa1, 64),
        ranked_root_input_1d("logical_second", 0x7a, 64),
    ]
    .map(|mut root| {
        root.source_launch = LaunchContract::new(
            1,
            BlockSize::Exact(fe2o3_artifacts::Dimensions::new(64, 1, 1).unwrap()),
            fe2o3_artifacts::Dimensions::new(3, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap();
        root
    })
}

fn source_launch_test_ssa_owner_v1() -> ProductionSemanticSsaOwnerV1 {
    let semantic = source_launch_test_semantic_v1(70, 0xa1);
    let owner = ProductionSemanticMirOwnerV1::try_new(
        semantic,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        owner,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn source_launch_production_any_and_at_most_are_exactly_incomplete() {
    for block_size in [
        BlockSize::Any,
        BlockSize::AtMost(fe2o3_artifacts::Dimensions::new(64, 1, 1).unwrap()),
    ] {
        let mut inputs = source_launch_test_inputs_v1();
        inputs[0].source_launch = LaunchContract::new(
            1,
            block_size,
            fe2o3_artifacts::Dimensions::new(3, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap();
        assert_eq!(inputs[0].source_launch.block_size(), block_size);
        assert_eq!(
            source_launch_input_v1(&inputs[0].source_launch),
            ProductionSourceLaunchInputV1::new(1, None, [3, 1, 1])
        );
        let result = materialize_ranked_fixture_v1(source_launch_test_ssa_owner_v1(), &inputs);
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "concurrency verification requires an exact authenticated LaunchContract workgroup"
            ))
        ));
    }
}

#[test]
fn source_launch_production_row_guard_rejects_substituted_grid_identity_and_root() {
    let program = neutral_ranked_program_v1();
    let semantic = program.materialized.semantic_ssa().source_semantic();
    let mut inputs = [ranked_root_input_1d("neutral_generated_hostile", 247, 64)];
    let [finite_launch, _] = source_launch_test_inputs_v1();
    inputs[0].source_launch = finite_launch.source_launch;
    let detached = inputs
        .iter()
        .map(source_launch_root_input_v1)
        .collect::<Vec<_>>();
    let roster = ProductionSourceLaunchRosterV1::try_new(semantic, &detached).unwrap();
    let effects = derive_defined_callable_empty_effect_summaries_v1(
        semantic.types(),
        semantic.functions(),
        semantic.callables(),
    )
    .unwrap();
    let selected = semantic
        .select_kernel_body_for_root_v1(semantic.roots()[0])
        .unwrap();
    let references = crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
    let project = |launch: &LaunchContract, row: ProductionSourceLaunchRootV1| {
        let input = ProductionRankedRootInputV1::new(
            &inputs[0].logical_name,
            inputs[0].kernel_binding,
            launch,
        );
        project_and_verify_ranked_root_v1(
            program.materialized.semantic_ssa(),
            &effects,
            selected,
            &input,
            row,
            &references,
            &mut ComponentDynamicAssertionFactsV1,
        )
    };

    let exact = project(&inputs[0].source_launch, roster.roots()[0]).unwrap();
    assert_eq!(exact.semantic_root, semantic.roots()[0]);
    assert_eq!(
        exact.semantic_root_identity,
        semantic.functions()[0].identity()
    );
    assert_eq!(exact.kernel_binding, bytes(247));
    assert_eq!(exact.source_rank, 1);
    let layouts = exact
        .lowering
        .kernel()
        .blocks()
        .iter()
        .flat_map(|block| {
            block.operations().iter().filter(|operation| {
                matches!(
                    operation,
                    ProductionRankedOperationV1::ExecutionLayout { .. }
                )
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        layouts,
        vec![&ProductionRankedOperationV1::ExecutionLayout {
            grid_identity: u64::from_le_bytes([247; 8]),
            global_extents: [192, 1, 1],
            workgroup_extents: [64, 1, 1],
            subgroup_size: 64,
            full_physical_workgroups: true,
        }]
    );

    let changed_grid = LaunchContract::new(
        1,
        inputs[0].source_launch.block_size(),
        fe2o3_artifacts::Dimensions::new(4, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap();
    assert_eq!(changed_grid.rank(), inputs[0].source_launch.rank());
    assert_eq!(
        changed_grid.block_size(),
        inputs[0].source_launch.block_size()
    );
    assert_ne!(changed_grid.max_grid(), inputs[0].source_launch.max_grid());

    let foreign_semantic = source_launch_test_semantic_v1(71, 247);
    let foreign_inputs = [
        ProductionSourceLaunchRootInputV1::new(
            "foreign_first",
            bytes(247),
            source_launch_input_v1(&inputs[0].source_launch),
        ),
        ProductionSourceLaunchRootInputV1::new(
            "foreign_second",
            bytes(0x7a),
            source_launch_input_v1(&inputs[0].source_launch),
        ),
    ];
    let foreign_roster =
        ProductionSourceLaunchRosterV1::try_new(&foreign_semantic, &foreign_inputs).unwrap();
    let exact_row = roster.roots()[0];
    let foreign_row = foreign_roster.roots()[0];
    assert_ne!(roster.semantic_sha256(), foreign_roster.semantic_sha256());
    assert_eq!(foreign_row.selected_root(), exact_row.selected_root());
    assert_eq!(foreign_row.kernel_binding(), exact_row.kernel_binding());
    assert_eq!(foreign_row.source_launch(), exact_row.source_launch());
    assert_eq!(foreign_row.layout(), exact_row.layout());
    assert_ne!(
        foreign_row.semantic_root_identity(),
        exact_row.semantic_root_identity()
    );

    for (launch, row) in [
        (&changed_grid, exact_row),
        (&inputs[0].source_launch, foreign_row),
        (&inputs[0].source_launch, foreign_roster.roots()[1]),
    ] {
        let result = project(launch, row);
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "source launch roster root changed before ranked projection"
            ))
        ));
    }
    assert_eq!(roster.roots()[0], exact_row);
    let retry = project(&inputs[0].source_launch, exact_row).unwrap();
    assert_eq!(retry.ranked_ir, exact.ranked_ir);
}

#[test]
fn source_launch_production_checks_later_geometry_before_earlier_body_failure() {
    let inputs = source_launch_test_inputs_v1();
    let individual = materialize_ranked_fixture_v1(source_launch_test_ssa_owner_v1(), &inputs)
        .and_then(|materialized| {
            project_and_verify_ranked_materialized_semantic_mir_v1(
                materialized,
                &inputs,
                &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
            )
        });
    assert!(matches!(
        individual,
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a kernel without a statically ranked indexed memory access"
        ))
    ));

    let mut conflicting = source_launch_test_inputs_v1();
    conflicting[1].source_launch = LaunchContract::new(
        1,
        BlockSize::Exact(fe2o3_artifacts::Dimensions::new(128, 1, 1).unwrap()),
        fe2o3_artifacts::Dimensions::new(3, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap();
    assert_eq!(conflicting[0].source_launch, inputs[0].source_launch);
    let combined = materialize_ranked_fixture_v1(source_launch_test_ssa_owner_v1(), &conflicting)
        .and_then(|materialized| {
            project_and_verify_ranked_materialized_semantic_mir_v1(
                materialized,
                &conflicting,
                &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
            )
        });
    assert!(matches!(
        combined,
        Err(ProductionRankedProjectionErrorV1::Unsupported(
            "authenticated LaunchContract workgroup disagrees with semantic source workgroup"
        ))
    ));
}
