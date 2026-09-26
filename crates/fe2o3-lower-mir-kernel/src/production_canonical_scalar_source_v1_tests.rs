use super::*;
use ProductionCanonicalScalarSourceErrorV1 as CsError;
use fe2o3_mir_model::semantic_mir_v1::*;

// Genuine admitted scalar/control source, not an edited executable or maps.
fn cs_source() -> ProductionPreRankedKirOwnerV1 {
    let seed = noop_semantic_owner(&["neutral_scalar_source"]);
    let semantic = seed.semantic();
    let root = &semantic.functions()[0];
    let p = SemanticSourceProvenanceV1::unavailable();
    let unit = SemanticTypeIdV1::from_index(0);
    let scalar = SemanticTypeIdV1::from_index(1);
    let place = |i| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(i), vec![], scalar).unwrap();
    let value = |i| SemanticOperandV1::Copy(place(i));
    let constant = |n| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            scalar,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(n, 8).unwrap()),
        ))
    };
    let assign = |i, kind| {
        SemanticStatementV1::new(
            p,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(i),
                SemanticRvalueV1::new(scalar, kind),
            )),
        )
    };
    let edge = |role, n| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(n));
    let goto = |n| SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, n));
    let block = |n: u8, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([210 + n; 32]),
            p,
            statements,
            SemanticTerminatorV1::new(p, terminator),
        )
        .unwrap()
    };
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([230; 32]),
        SemanticLayoutIdentityV1::from_sha256([231; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            scalar,
            SemanticAbiPassModeV1::Direct(attributes),
        ))],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        p,
        abi,
        (0..5)
            .map(|i| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([240 + i; 32]),
                    if i == 0 { unit } else { scalar },
                    match i {
                        0 => SemanticLocalRoleV1::Return,
                        1 => SemanticLocalRoleV1::Argument(0),
                        _ => SemanticLocalRoleV1::Temporary,
                    },
                    p,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        vec![
            block(
                0,
                vec![assign(
                    2,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::BitXor,
                        left: value(1),
                        right: constant(0),
                    },
                )],
                goto(1),
            ),
            block(
                1,
                vec![assign(
                    3,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::BitOr,
                        left: value(2),
                        right: constant(0),
                    },
                )],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: value(3),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 2),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                    )
                    .unwrap(),
                },
            ),
            block(
                2,
                vec![assign(4, SemanticRvalueKindV1::Use(constant(11)))],
                goto(4),
            ),
            block(
                3,
                vec![assign(4, SemanticRvalueKindV1::Use(constant(17)))],
                goto(4),
            ),
            block(
                4,
                vec![SemanticStatementV1::new(p, SemanticStatementKindV1::Nop)],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new(
        semantic.target(),
        vec![semantic.types()[0].clone(), scalar_type()],
        vec![],
        vec![],
        vec![],
        vec![function],
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    cr_owner_from_ssa(
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap(),
    )
}

fn cs_prepare(source: ProductionPreRankedKirOwnerV1) -> ProductionCanonicalScalarFixedPointOwnerV1 {
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = source.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let (owner, additional) =
        ProductionCanonicalScalarFixedPointOwnerV1::try_prepare_v1(source, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(additional, owner.additional_storage());
    assert_eq!(
        owner.retained_storage_floor_v1(),
        owner.input_storage_floor_v1() + additional.retained_storage()
    );
    owner
}

fn cs_run<T>(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    callback: impl for<'s, 'm, 'g> FnOnce(
        &ProductionCanonicalScalarSourcePoliciesV1<'s, 'm, 'g>,
        &mut Budget<'_>,
    ) -> CsResultV1<T>,
) -> CsResultV1<T> {
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.retained_storage_floor_v1() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result = owner.with_policy_checks_v1(&mut budget, callback);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    result
}

fn cs_terminal(owner: &ProductionCanonicalScalarFixedPointOwnerV1, changed: bool) {
    let original = owner.original_source().executable();
    assert!(!std::ptr::eq(original, owner.output()));
    assert_eq!(
        original.canonical().canonical_bytes() != owner.output().canonical().canonical_bytes(),
        changed
    );
    let mut input = original;
    let rounds = owner.history().rounds();
    assert!(!rounds.is_empty());
    assert!(rounds.len() <= fe2o3_kernel_opt::SCALAR_FIXED_POINT_MAX_ROUNDS_V1);
    for (n, round) in rounds.iter().enumerate() {
        assert_eq!(usize::from(round.ordinal()), n);
        assert_eq!(
            input.canonical().canonical_bytes() != round.output().canonical().canonical_bytes(),
            n + 1 != rounds.len()
        );
        input = round.output();
    }
    assert!(std::ptr::eq(input, owner.output()));
    if changed {
        assert!(rounds.len() >= 2);
    } else {
        assert_eq!(rounds.len(), 1);
    }
}

#[test]
fn canonical_scalar_source_mutating_and_noop_run_actual_final_fixed_nine() {
    for (source, changed) in [(cs_source(), true), (cr_noop(&["zeta", "alpha"]), false)] {
        let owner = cs_prepare(source);
        cs_terminal(&owner, changed);
        cs_run(&owner, |view, budget| {
            let original = view.original_metadata(budget)?;
            assert!(std::ptr::eq(
                original.inventory(budget)?.owner(),
                owner.original_source().executable()
            ));
            assert!(
                original
                    .spans(budget)?
                    .iter()
                    .any(|r| r.operations().is_empty())
            );
            let output = view.final_inventory(budget)?;
            assert!(std::ptr::eq(output.owner(), owner.output()));
            let policies = view.policies(budget)?;
            assert!(std::ptr::eq(policies.owner(budget)?, owner.output()));
            assert_eq!(policies.function_count(budget)?, output.functions().len());
            for n in 0..policies.function_count(budget)? {
                assert_eq!(policies.report(n, budget)?.pass_order().len(), 9);
                assert!(policies.report(n, budget)?.is_clean());
                assert_eq!(policies.history(n, budget)?.function(), n);
            }
            assert_eq!(policies.pending_obligations().iter().count(), 19);
            assert!(!view.ranked_verification_is_complete());
            assert!(!view.grants_artifact_or_launch_authority());
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn canonical_scalar_source_exact_elisions_substitutions_and_control_chains() {
    let owner = cs_prepare(cs_source());
    cs_run(&owner, |view, budget| {
        let before = view.original_metadata(budget)?.inventory(budget)?;
        let after = view.final_inventory(budget)?;
        let lineage = view.lineage(budget)?;
        assert_eq!(
            lineage.original_definition_count(budget)?,
            before.definitions().len()
        );
        let (mut elided, mut substituted) = (0, 0);
        for n in 0..before.definitions().len() {
            let count = lineage.definition_descendant_count(n, budget)?;
            elided += usize::from(count == 0);
            for d in 0..count {
                let row = lineage.definition_descendant(n, d, budget)?;
                assert_eq!(row.original, before.definitions()[n].coordinate);
                cs_definition_v1(after, row.output, budget)?;
                substituted += usize::from(
                    row.kind
                        == fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1::Substituted,
                );
            }
        }
        assert!(
            elided > 0,
            "dead original values must remain in the zero-descendant roster"
        );
        assert!(
            substituted > 0,
            "live neutral identities must preserve value substitutions"
        );
        let mut merged = 0;
        for n in 0..after.blocks().len() {
            let segments = lineage.block_segments(n, budget)?;
            merged += usize::from(segments.len() > 1);
            for (s, row) in segments.iter().enumerate() {
                budget.charge_work(1)?;
                let old = cs_block_v1(before, row.original, budget)?;
                let control = lineage.original_block_control(old, budget)?;
                assert_eq!(
                    control.placement,
                    Some(fe2o3_kernel_analysis::CanonicalKirBlockPlacementV1 {
                        output: after.blocks()[n].coordinate,
                        segment: s as u32
                    })
                );
                assert_eq!(row.connector.is_none(), s + 1 == segments.len());
                if let Some(edge) = row.connector {
                    let e = cs_edge_v1(before, edge, budget)?;
                    assert!(matches!(
                        lineage.original_edge_control(e, budget)?.placement,
                        fe2o3_kernel_analysis::CanonicalKirEdgePlacementV1::InternalConnector(_)
                    ));
                }
            }
        }
        assert!(merged > 0);
        for n in 0..before.blocks().len() {
            assert_eq!(
                lineage.original_block_control(n, budget)?.original,
                before.blocks()[n].coordinate
            );
        }
        for n in 0..before.edges().len() {
            assert_eq!(
                lineage.original_edge_control(n, budget)?.original,
                before.edges()[n].coordinate
            );
        }
        for n in 0..after.uses().len() {
            cs_use_v1(before, lineage.use_origin(n, budget)?, budget)?;
        }
        for n in 0..after.edges().len() {
            cs_edge_v1(before, lineage.edge_origin(n, budget)?, budget)?;
        }
        for n in 0..after.edge_arguments().len() {
            cs_argument_v1(before, lineage.edge_argument_origin(n, budget)?, budget)?;
        }
        for n in 0..after.operations().len() {
            if let ProductionCanonicalScalarOperationOriginV1::Original(op) =
                lineage.operation_origin(n, budget)?
            {
                cs_operation_v1(before, op, budget)?;
            }
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn canonical_scalar_source_same_bytes_foreign_inventory_is_not_original_custody() {
    let owner = cs_prepare(cr_noop(&["noop"]));
    let foreign = cr_noop(&["noop"]);
    assert_eq!(
        owner
            .original_source()
            .executable()
            .canonical()
            .canonical_bytes(),
        foreign.executable().canonical().canonical_bytes()
    );
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.retained_storage_floor_v1()
        + foreign.unit_local_source_storage_floor_v1().unwrap()
        + SIBLING;
    budget.reserve_storage(floor).unwrap();
    cs_scope_v1(&mut budget, |budget| {
        let (inventory, receipt) = CanonicalKirInventoryV1::derive(foreign.executable(), budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        assert!(matches!(
            cs_lineage_v1(&owner, &inventory, budget),
            Err(CsError::InputCustody)
        ));
        foreign.with_canonical_ranked_metadata_v1(budget, |source, budget| {
            Ok(cs_scope_v1(budget, |budget| {
                let (output, receipt) = CanonicalKirInventoryV1::derive(owner.output(), budget)?;
                budget.reserve_storage(receipt.retained_storage())?;
                assert!(matches!(
                    cs_check_subjects_v1(&owner, source, &output, owner.output(), budget),
                    Err(CsError::InputCustody)
                ));
                Ok(())
            }))
        })??;
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn canonical_scalar_source_changed_adjacent_occurrence_refuses_before_final_reports() {
    let owner = cs_prepare(cs_source());
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.retained_storage_floor_v1() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    cs_scope_v1(&mut budget, |budget| {
        let input = owner.original_source().executable();
        let (inventory, receipt) = CanonicalKirInventoryV1::derive(input, budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let old = cs_identity_v1(&inventory, budget)?;
        let step = owner.history.rounds()[0].integer();
        let rows = step.occurrences().candidate();
        budget.reserve_storage(std::mem::size_of::<
            Vec<fe2o3_kernel_ir::CanonicalKirUseTransitionV1>,
        >())?;
        let mut uses = cs_vec_v1(rows.uses.len(), budget)?;
        for row in rows.uses {
            cs_push_v1(&mut uses, *row, budget)?;
        }
        assert!(!uses.is_empty());
        budget.charge_work(inventory.uses().len())?;
        let current = uses[0].input;
        uses[0].input = inventory
            .uses()
            .iter()
            .find(|row| row.coordinate != current)
            .expect("genuine input has distinct operation and control operands")
            .coordinate;
        let changed = fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1 {
            uses: &uses,
            ..rows
        };
        assert!(matches!(
            cs_scope_v1(budget, |budget| cs_pair_v1(
                &inventory,
                input,
                step.owner(),
                changed,
                &old,
                0,
                true,
                budget
            )),
            Err(CsError::Transition(_))
        ));
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn canonical_scalar_source_stale_same_byte_original_report_is_not_final_report() {
    let owner = cs_prepare(cr_noop(&["same_bytes"]));
    cs_terminal(&owner, false);
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = owner.retained_storage_floor_v1() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    owner
        .original_source()
        .with_checked_canonical_ranked_source_v1(&mut budget, |old, budget| {
            Ok(old.with_policy_checks_v1(budget, |old_reports, budget| {
                Ok(cs_scope_v1(budget, |budget| {
                    let original = old_reports.metadata(budget)?;
                    let (final_inventory, receipt) =
                        CanonicalKirInventoryV1::derive(owner.output(), budget)?;
                    budget.reserve_storage(receipt.retained_storage())?;
                    let old_report_owner = old_reports.policies(budget)?.owner(budget)?;
                    assert_eq!(
                        old_report_owner.canonical().canonical_bytes(),
                        owner.output().canonical().canonical_bytes()
                    );
                    assert!(matches!(
                        cs_check_subjects_v1(
                            &owner,
                            original,
                            &final_inventory,
                            old_report_owner,
                            budget
                        ),
                        Err(CsError::InputCustody)
                    ));
                    assert!(matches!(
                        cs_check_subjects_v1(
                            &owner,
                            original,
                            original.inventory,
                            owner.output(),
                            budget
                        ),
                        Err(CsError::InputCustody)
                    ));
                    Ok(())
                }))
            }))
        })
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn canonical_scalar_source_complete_catalog_refusal_precedes_optimizer() {
    let source = nonempty_source();
    let source = cr_owner_from_ssa(source.semantic_ssa);
    let mut work = Work::new(1 << 48);
    let mut budget = Budget::new(&mut work, S);
    let floor = source.unit_local_source_storage_floor_v1().unwrap() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        ProductionCanonicalScalarFixedPointOwnerV1::try_prepare_v1(source, &mut budget),
        Err(CsError::SourcePolicy(
            ProductionCanonicalRankedPolicyErrorV1::Unsupported {
                requirement: ProductionCanonicalRankedSourceRequirementV1::Catalog,
                ..
            }
        ))
    ));
    assert_eq!(budget.storage(), floor);
}

mod resources {
    include!("production_canonical_scalar_resources_v1_tests.rs");
}
