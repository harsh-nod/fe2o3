//! Original ordinary semantic owners through the real receipt/handoff helpers.
//! These unit fixtures do not claim rustc callable classification authority.
use super::*;
use crate::production_semantic_body_v1::ProductionSourceCensusEncodingV1;

fn ordinary_source(roots: u32) -> AdmittedInertSemanticMirV1 {
    let original = fixture_roots(Mutation::None, u8::try_from(roots).unwrap());
    let mut functions = Vec::new();
    for root in &original.functions()[..roots as usize] {
        functions.push(
            SemanticFunctionDeclV1::new(
                root.identity(),
                root.role(),
                root.item_definition_identity(),
                root.monomorphization_identity(),
                root.generic_type_arguments_identity(),
                root.const_generic_arguments_identity(),
                root.source(),
                root.abi().clone(),
                root.locals()[..3].to_vec(),
                SemanticBlockIdV1::from_index(0),
                vec![
                    block(10, vec![], call(roots, vec![], 0, UNIT, 1)),
                    block(11, vec![unit_return()], SemanticTerminatorKindV1::Return),
                    block(
                        12,
                        vec![unit_return()],
                        SemanticTerminatorKindV1::Unreachable,
                    ),
                ],
            )
            .unwrap()
            .with_kernel_entry(root.kernel_entry().unwrap().clone()),
        );
    }
    let helper = &original.functions()[roots as usize];
    functions.push(
        SemanticFunctionDeclV1::new(
            helper.identity(),
            SemanticFunctionRoleV1::InternalHelper,
            helper.item_definition_identity(),
            helper.monomorphization_identity(),
            helper.generic_type_arguments_identity(),
            helper.const_generic_arguments_identity(),
            helper.source(),
            abi(220, false, &[], UNIT),
            vec![SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([221; 32]),
                UNIT,
                SemanticLocalRoleV1::Return,
                SemanticSourceProvenanceV1::unavailable(),
            )],
            SemanticBlockIdV1::from_index(0),
            vec![block(
                222,
                vec![unit_return()],
                SemanticTerminatorKindV1::Return,
            )],
        )
        .unwrap(),
    );
    InertSemanticMirRequestV1::new(
        original.target(),
        original.types()[..2].to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        original.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

fn ordinary_declarations(
    semantic: &AdmittedInertSemanticMirV1,
) -> (
    ProductionSourceCensusEncodingV1,
    SemanticDeclarationTablesCommitmentV1,
) {
    let encoding = ProductionSourceCensusEncodingV1::select(
        semantic.types(),
        semantic.callables(),
        |_| Ok(()),
    )
    .unwrap();
    let declarations = canonical_declaration_tables_commitment_v1(
        semantic.types(),
        semantic.callables(),
        encoding.wire_version(),
        SemanticMirLimitsV1::default(),
        &mut |_| Ok(()),
    )
    .unwrap();
    (encoding, declarations)
}

fn ordinary_pending(semantic: &AdmittedInertSemanticMirV1) -> PendingWorkgroupScopesV29 {
    let (encoding, declarations) = ordinary_declarations(semantic);
    PendingWorkgroupScopesV29::new_with_encoding(
        vec![ScopeCallableV29::Ordinary; semantic.callables().len()],
        declarations,
        semantic.target(),
        semantic.functions().len(),
        encoding,
    )
    .unwrap()
}

fn ordinary_receipt(semantic: &AdmittedInertSemanticMirV1) -> RetainedContextEntriesV29 {
    let captured = capture_with_pending(semantic, ordinary_pending(semantic), |_, _| {});
    RetainedContextEntriesV29::seal_with_scopes(
        vec![],
        Some((captured, ordinary_declarations(semantic).1)),
        semantic,
        |_| Ok(()),
    )
    .unwrap()
}

#[test]
fn ordinary_source_census_shared_helper_and_unreachable_blocks_reach_real_handoff() {
    use crate::production_pipeline::with_projected_execution_source_v29;
    use fe2o3_lower_mir_kernel::with_checked_execution_source_v29;
    for roots in [1, 3] {
        let semantic = ordinary_source(roots);
        let sha = *semantic.semantic_sha256().as_bytes();
        let profile = semantic.wire_version();
        assert_ne!(profile, SemanticMirWireVersionV1::V29);
        let receipt = ordinary_receipt(&semantic);
        let launch = launch_roster(&semantic);
        let owner = ssa_owner(semantic);
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = VisitBudget::new(&mut work, projection_storage(&receipt) + 19);
        budget.reserve_storage(19).unwrap();
        let source = receipt
            .materialization_source_v29(owner.source_semantic(), &mut budget)
            .unwrap()
            .unwrap();
        assert_eq!(source.semantic_sha256(), &sha);
        assert_eq!(source.classes().len(), roots as usize + 1);
        assert!(source.roots().is_empty());
        assert!(source.events().is_empty());
        let mut reached = false;
        with_projected_execution_source_v29(&source, &mut budget, |input, budget| {
            reached = true;
            assert_eq!(input.semantic_sha256, &sha);
            assert_eq!(input.classes.len(), roots as usize + 1);
            with_checked_execution_source_v29(&owner, &launch, input, budget, |_, _| {
                panic!("ordinary source issued a context")
            })
        })
        .unwrap();
        assert!(reached);
        assert_eq!(owner.source_semantic().wire_version(), profile);
        assert_eq!(budget.storage(), 19);
    }
}

#[test]
fn ordinary_source_census_absent_scopes_do_not_mint_an_ordinary_source() {
    let semantic = ordinary_source(1);
    let absent = RetainedContextEntriesV29::seal(vec![], &semantic, |_| Ok(())).unwrap();
    let complete = ordinary_receipt(&semantic);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = VisitBudget::new(&mut work, 0);
    assert!(
        absent
            .materialization_source_v29(&semantic, &mut budget)
            .unwrap()
            .is_none()
    );
    assert!(
        complete
            .materialization_source_v29(&semantic, &mut budget)
            .unwrap()
            .is_some()
    );
}

#[test]
fn ordinary_source_census_rejects_missing_class_incomplete_body_and_extra_event() {
    let semantic = ordinary_source(2);
    for mutation in 0..3 {
        let (encoding, declarations) = ordinary_declarations(&semantic);
        let count = semantic.callables().len() - usize::from(mutation == 0);
        let pending = PendingWorkgroupScopesV29::new_with_encoding(
            vec![ScopeCallableV29::Ordinary; count],
            declarations,
            semantic.target(),
            if mutation == 0 {
                count
            } else {
                semantic.functions().len()
            },
            encoding,
        )
        .unwrap();
        let pending = if mutation == 0 {
            pending
        } else {
            let mut pending = pending;
            for (index, function) in semantic.functions().iter().enumerate() {
                if mutation == 1 && index == semantic.functions().len() - 1 {
                    break;
                }
                let mut events = Vec::new();
                let id = SemanticFunctionIdV1::from_index(index as u32);
                for (block, data) in function.blocks().iter().enumerate() {
                    pending
                        .capture(
                            id,
                            SemanticBlockIdV1::from_index(block as u32),
                            data.statements().len(),
                            data.terminator().kind(),
                            &mut events,
                            |_| Ok(()),
                        )
                        .unwrap();
                }
                if mutation == 2 && index == 0 {
                    events.push(ScopeEventV29 {
                        function: id,
                        block: SemanticBlockIdV1::from_index(0),
                        statement_count: 0,
                        kind: ScopeEventKindV29::Return,
                    });
                }
                pending.prepare(id, events, |_| Ok(())).unwrap().publish();
            }
            pending
        };
        assert!(
            RetainedContextEntriesV29::seal_with_scopes(
                vec![],
                Some((pending, declarations)),
                &semantic,
                |_| Ok(())
            )
            .is_err()
        );
    }
    let _control = ordinary_receipt(&semantic);
}

#[test]
fn ordinary_source_census_same_layout_declaration_substitution_is_rejected() {
    let semantic = ordinary_source(1);
    let mut types = semantic.types().to_vec();
    let original = &types[1];
    types[1] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([234; 32]),
        original.layout_identity(),
        original.layout().clone(),
        original.shape().clone(),
    )
    .with_rustc_abi_properties(original.abi_properties())
    .with_rust_type_kind(original.rust_type_kind());
    let changed = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        semantic.functions().to_vec(),
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let pending = capture_with_pending(&semantic, ordinary_pending(&semantic), |_, _| {});
    assert!(
        RetainedContextEntriesV29::seal_with_scopes(
            vec![],
            Some((pending, ordinary_declarations(&changed).1)),
            &changed,
            |_| Ok(())
        )
        .is_err()
    );
    let receipt = ordinary_receipt(&semantic);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut budget = VisitBudget::new(&mut work, 0);
    assert!(
        receipt
            .materialization_source_v29(&changed, &mut budget)
            .is_err()
    );
    assert!(
        receipt
            .materialization_source_v29(&semantic, &mut budget)
            .unwrap()
            .is_some()
    );
}

#[test]
fn ordinary_source_census_borrow_exact_work_and_projection_storage() {
    let semantic = ordinary_source(2);
    let receipt = ordinary_receipt(&semantic);
    let required = 1 + semantic.callables().len();
    for maximum in [required - 1, required] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(maximum);
        let mut budget = VisitBudget::new(&mut work, 7);
        budget.reserve_storage(7).unwrap();
        assert_eq!(
            receipt
                .materialization_source_v29(&semantic, &mut budget)
                .is_ok(),
            maximum == required
        );
        assert_eq!(budget.storage(), 7);
    }
    let bytes = projection_storage(&receipt);
    for maximum in [bytes - 1, bytes] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = VisitBudget::new(&mut work, maximum + 7);
        budget.reserve_storage(7).unwrap();
        let source = receipt
            .materialization_source_v29(&semantic, &mut budget)
            .unwrap()
            .unwrap();
        let result = crate::production_pipeline::with_projected_execution_source_v29(
            &source,
            &mut budget,
            |_, _| Ok(()),
        );
        assert_eq!(result.is_ok(), maximum == bytes);
        assert_eq!(budget.storage(), 7);
    }
}
