use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_mir_model::{SsaBlockIdV1, SsaPlannerErrorV1, SsaResolvedEventV1, plan_ssa_v1};

fn canonical_expansion(source: &AdmittedInertSemanticMirV1) -> SemanticCallExpansionV1 {
    SemanticCallExpansionV1::try_new(
        source,
        fe2o3_mir_model::SemanticCallExpansionLimitsV1::default(),
    )
    .unwrap()
}

fn bound_borrow(view: &SemanticExpandedRootV1) -> (SemanticTransparentBorrowSiteV1, u32) {
    let mut selected = Vec::new();
    for (block, origin) in view.block_origins().iter().enumerate() {
        if origin.function() != view.root() {
            continue;
        }
        for (statement, source) in view.body().blocks()[block].statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(a) = source.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            } = a.value().kind()
            else {
                continue;
            };
            if place.ty() == SemanticTypeIdV1::from_index(7) && place.projections().is_empty() {
                assert!(matches!(
                    origin.statements()[statement],
                    SemanticExpandedStatementOriginV1::Source { .. }
                ));
                selected.push((
                    SemanticTransparentBorrowSiteV1 {
                        block: block as u32,
                        statement: statement as u32,
                    },
                    place.local().index(),
                ));
            }
        }
    }
    assert!(!selected.is_empty());
    selected[0]
}

#[test]
fn canonical_narrow_registers_exact_getter_and_normal_return_not_entry() {
    let source = canonical_fixture::source(canonical_fixture::Mutation::None);
    let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        source.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.canonical_encoding(), source.canonical_encoding());
    let expansion = canonical_expansion(&decoded);
    let view = expansion.root(decoded.roots()[0]).unwrap();
    let bindings = expansion.defined_capability_bindings(&decoded).unwrap();
    let facts = MatrixBorrowSitesV1::new(&decoded, &expansion, view, &bindings, 65_536).unwrap();
    let narrow = bindings
        .iter()
        .find(|b| {
            matches!(
                b.contract(),
                SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(_)
            )
        })
        .unwrap();
    let math =
        super::super::math_borrows_v1::MathBorrowSitesV1::new(&decoded, view, &bindings, 65_536)
            .unwrap();
    assert!(
        math.pairs.is_empty(),
        "no PolicyMath route may hide the Matrix getter"
    );
    let mut counts = [0; 2];
    for (block, origin) in view.block_origins().iter().enumerate() {
        for (statement, s) in view.body().blocks()[block].statements().iter().enumerate() {
            let site = SemanticTransparentBorrowSiteV1 {
                block: block as u32,
                statement: statement as u32,
            };
            let Some(locals) = facts.captured(site, s.kind(), &mut |_| Ok(())).unwrap() else {
                continue;
            };
            assert!(!locals.is_empty());
            assert!(matches!(
                origin.statements()[statement],
                SemanticExpandedStatementOriginV1::Source { statement: 0 }
            ));
            let SemanticStatementKindV1::Assign(a) = s.kind() else {
                unreachable!()
            };
            match a.value().kind() {
                SemanticRvalueKindV1::Aggregate(_) => {
                    counts[0] += 1;
                    if origin.instance() == narrow.callee_instance() {
                        assert_ne!(block as u32, narrow.expanded_entry_block().index());
                        assert_eq!(origin.block().index(), 1);
                    }
                }
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(p)) => {
                    counts[1] += 1;
                    assert_eq!(p.projections().len(), 2);
                    assert_eq!(
                        p.projections()[1].kind(),
                        SemanticProjectionKindV1::Field(0)
                    );
                    assert_eq!(
                        view.instances()[origin.instance().index() as usize].parent(),
                        Some(narrow.callee_instance())
                    );
                }
                _ => panic!("only the recorded constructor or getter may be a capture"),
            }
            assert!(
                facts
                    .captured(site, &s.kind().clone(), &mut |_| Ok(()))
                    .unwrap()
                    .is_none()
            );
            assert!(
                facts
                    .captured(
                        SemanticTransparentBorrowSiteV1 {
                            block: u32::MAX,
                            ..site
                        },
                        s.kind(),
                        &mut |_| Ok(())
                    )
                    .unwrap()
                    .is_none()
            );
        }
    }
    assert_eq!(counts, [2, 1]);
}

#[test]
fn canonical_matrix_flow_keeps_the_original_borrow_use() {
    let source = canonical_fixture::source(canonical_fixture::Mutation::None);
    let expansion = canonical_expansion(&source);
    let view = expansion.root(source.roots()[0]).unwrap();
    let sites =
        super::super::borrowed_workgroup_v1::execution_sites(&source, &expansion, view, 65_536)
            .unwrap();
    let (site, local) = bound_borrow(view);
    assert!(sites.contains(&site));
    let mut range = None;
    let (input, _, _) = super::super::semantic_function_ssa_input_with_event_origins_v1(
        view.body(),
        Some(source.types()),
        source.callables(),
        &sites,
        |block, statement, events| {
            if block == site.block && statement == Some(site.statement) {
                assert!(range.replace(events).is_none());
            }
        },
    );
    assert!(input.promotable()[local as usize]);
    let plan = plan_ssa_v1(&input).unwrap();
    let range = range.unwrap();
    let uses = plan.resolved_events(SsaBlockIdV1::new(site.block)).unwrap().iter()
        .filter(|(event, resolved)| range.contains(&(*event as usize)) && matches!(resolved, SsaResolvedEventV1::Use { variable, .. } if variable.get() == local)).count();
    assert_eq!(
        uses, 1,
        "existing source Borrow must emit its real owner Use"
    );
}

#[test]
fn canonical_capture_does_not_excuse_other_reference_uses_or_duplicates() {
    use canonical_fixture::Mutation::*;
    for mutation in [DuplicateBorrow, ExtraCapture, ProjectedCopy] {
        let source = canonical_fixture::source(mutation);
        let expansion = canonical_expansion(&source);
        let view = expansion.root(source.roots()[0]).unwrap();
        let sites =
            super::super::borrowed_workgroup_v1::execution_sites(&source, &expansion, view, 65_536)
                .unwrap();
        let (site, local) = bound_borrow(view);
        assert!(
            !sites.contains(&site),
            "{mutation:?} must invalidate the complete reference component"
        );
        let (input, _, _) = super::super::semantic_function_ssa_input_v1(
            view.body(),
            Some(source.types()),
            source.callables(),
            &sites,
        );
        assert!(
            !input.promotable()[local as usize],
            "{mutation:?}: no type-only owner promotion"
        );
    }
}

#[test]
fn canonical_capture_does_not_initialize_a_dead_or_deinitialized_owner() {
    for mutation in [
        canonical_fixture::Mutation::DeadOwner,
        canonical_fixture::Mutation::Uninitialized,
    ] {
        let source = canonical_fixture::source(mutation);
        let expansion = canonical_expansion(&source);
        let view = expansion.root(source.roots()[0]).unwrap();
        let sites =
            super::super::borrowed_workgroup_v1::execution_sites(&source, &expansion, view, 65_536)
                .unwrap();
        let (site, local) = bound_borrow(view);
        let mut borrow_events = None;
        let (input, _, _) = super::super::semantic_function_ssa_input_with_event_origins_v1(
            view.body(),
            Some(source.types()),
            source.callables(),
            &sites,
            |block, statement, events| {
                if block == site.block && statement == Some(site.statement) {
                    assert!(borrow_events.replace(events).is_none());
                }
            },
        );
        let borrow_events = borrow_events.expect("the original owner Borrow remains retained");
        if mutation == canonical_fixture::Mutation::DeadOwner {
            assert!(input.promotable()[local as usize]);
            assert!(
                matches!(plan_ssa_v1(&input), Err(SsaPlannerErrorV1::UndefinedAtUse { variable, .. }) if variable.get() == local),
                "{mutation:?}: transparent addresses do not initialize storage"
            );
        } else {
            // Deinitialize independently retains storage. Address transparency
            // cannot supply the promoted owner Use required by borrow_place_use.
            assert_eq!(mutation, canonical_fixture::Mutation::Uninitialized);
            assert!(!input.promotable()[local as usize]);
            let plan = plan_ssa_v1(&input).unwrap();
            assert!(
                !plan.promoted_variables().iter().any(|variable| variable.get() == local),
                "deinitialized storage cannot become a promoted owner"
            );
            let events = plan.resolved_events(SsaBlockIdV1::new(site.block)).unwrap();
            assert!(!events.iter().any(|(event, resolved)|
                borrow_events.contains(&(*event as usize))
                    && matches!(resolved, SsaResolvedEventV1::Use { variable, .. } if variable.get() == local)),
                "retained memory is not an SSA-value fallback at the original Borrow");
        }
    }
}

#[test]
fn canonical_capture_registration_and_shared_flow_respect_exact_work_boundaries() {
    let source = canonical_fixture::source(canonical_fixture::Mutation::None);
    let expansion = canonical_expansion(&source);
    let view = expansion.root(source.roots()[0]).unwrap();
    let bindings = expansion.defined_capability_bindings(&source).unwrap();
    let facts = MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, 65_536).unwrap();
    let required = bindings.len() * 32 + facts.work_units;
    assert!(MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, required).is_ok());
    assert!(matches!(
        MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, required - 1),
        Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            ..
        })
    ));
    // The caller has only the same finite allowance after registration. It must
    // not get fresh work for the common all-use scan.
    let error = super::super::borrowed_workgroup_v1::execution_sites(
        &source, &expansion, view, required,
    ).unwrap_err();
    let ProductionSemanticSsaErrorV1::BorrowFlowWork {
        phase_work_units,
        remaining_work_units,
        requested_work_units,
        error,
        ..
    } = error else {
        panic!("expected the common-flow work failure, got {error:?}");
    };
    assert_eq!(phase_work_units.iter().sum::<usize>() + remaining_work_units, required);
    assert!(requested_work_units > remaining_work_units);
    assert_eq!(*error, ProductionSemanticSsaErrorV1::AggregateResourceLimit {
        resource: SsaPlannerResourceV1::WorkUnits,
        required: required + 1,
        limit: required,
    });
    let bind = bindings
        .iter()
        .find(|b| {
            matches!(
                b.contract(),
                SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_)
            )
        })
        .unwrap();
    let narrow = bindings
        .iter()
        .find(|b| {
            matches!(
                b.contract(),
                SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(_)
            )
        })
        .unwrap();
    let first = MatrixBorrowSitesV1::new(
        &source,
        &expansion,
        view,
        std::slice::from_ref(bind),
        65_536,
    )
    .unwrap();
    let mut partial = first;
    let before = partial.scoped.work_units();
    assert!(
        partial
            .scoped
            .register(
                &source,
                &expansion,
                view,
                narrow,
                &mut partial.pairs,
                before + 1
            )
            .is_err()
    );
    assert_eq!(
        partial.scoped.work_units(),
        before,
        "failed debit cannot refund earlier real registration"
    );
}

#[test]
fn empty_matrix_capture_keeps_the_old_inventory_boundary() {
    let source = source();
    let expansion = canonical_expansion(&source);
    let view = expansion.root(source.roots()[0]).unwrap();
    let bindings = expansion.defined_capability_bindings(&source).unwrap();
    assert!(!bindings.is_empty());
    let facts = MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, bindings.len() * 32)
        .unwrap();
    assert_eq!(facts.work_units, 0);
    let mut charged = 0;
    assert!(
        facts
            .captured(
                SemanticTransparentBorrowSiteV1 {
                    block: 0,
                    statement: 0
                },
                &SemanticStatementKindV1::Nop,
                &mut |work| {
                    charged += work;
                    Ok(())
                }
            )
            .unwrap()
            .is_none()
    );
    assert_eq!(charged, 0);
}

#[test]
fn canonical_capture_rejects_foreign_source_view_and_record_instances() {
    let source = canonical_fixture::source(canonical_fixture::Mutation::None);
    let expansion = canonical_expansion(&source);
    let view = expansion.root(source.roots()[0]).unwrap();
    let bindings = expansion.defined_capability_bindings(&source).unwrap();
    let foreign = canonical_expansion(&source);
    assert!(matches!(
        MatrixBorrowSitesV1::new(
            &source,
            &expansion,
            foreign.root(source.roots()[0]).unwrap(),
            &bindings,
            65_536
        ),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
    let changed = canonical_fixture::source(canonical_fixture::Mutation::ExtraCapture);
    let changed_expansion = canonical_expansion(&changed);
    let changed_bindings = changed_expansion
        .defined_capability_bindings(&changed)
        .unwrap();
    assert!(matches!(
        MatrixBorrowSitesV1::new(&source, &expansion, view, &changed_bindings, 65_536),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
    let mut duplicated = bindings.clone();
    duplicated.push(bindings[0].clone());
    assert!(matches!(
        MatrixBorrowSitesV1::new(&source, &expansion, view, &duplicated, 65_536),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
}

#[test]
fn canonical_narrow_rejects_wrong_field_order_and_normal_edge_before_registration() {
    for mutation in [
        canonical_fixture::Mutation::WrongField,
        canonical_fixture::Mutation::ReorderedBind,
        canonical_fixture::Mutation::WrongNormalEdge,
    ] {
        assert!(
            matches!(
                canonical_fixture::try_source(mutation),
                Err(SemanticMirErrorV1::InvalidFunctionAbi)
            ),
            "{mutation:?}"
        );
    }
}
