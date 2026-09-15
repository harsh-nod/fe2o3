use super::super::super as adapter;
use super::*;
use crate::production::{
    ProductionSemanticMirOwnerV1, ProductionSemanticSsaOwnerV1,
    ProductionSemanticSsaSourceQueryErrorV1, ProductionSemanticSsaSourceSiteV1,
};

#[path = "bound_read_fixture.rs"]
mod fixture;
use fixture::Mutation;

fn ty(index: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(index)
}

fn root_borrow(
    view: &SemanticExpandedRootV1,
    original_local: u32,
) -> (SemanticTransparentBorrowSiteV1, &SemanticPlaceV1) {
    for (block, body) in view.body().blocks().iter().enumerate() {
        let origin = &view.block_origins()[block];
        if origin.function() != view.root() {
            continue;
        }
        for (statement, node) in body.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(a) = node.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            } = a.value().kind()
            else {
                continue;
            };
            if place.projections().is_empty()
                && view.local_origins()[place.local().index() as usize]
                    .local()
                    .index()
                    == original_local
            {
                assert_eq!(
                    origin.statements()[statement],
                    SemanticExpandedStatementOriginV1::Source {
                        statement: statement as u32
                    }
                );
                return (
                    SemanticTransparentBorrowSiteV1 {
                        block: block as u32,
                        statement: statement as u32,
                    },
                    place,
                );
            }
        }
    }
    panic!("missing original root Borrow of local {original_local}")
}

fn original_site(
    view: &SemanticExpandedRootV1,
    block: u32,
    statement: u32,
) -> SemanticTransparentBorrowSiteV1 {
    let (expanded, origin) = view
        .block_origins()
        .iter()
        .enumerate()
        .find(|(_, origin)| origin.function() == view.root() && origin.block().index() == block)
        .unwrap();
    assert_eq!(
        origin.statements()[statement as usize],
        SemanticExpandedStatementOriginV1::Source { statement }
    );
    SemanticTransparentBorrowSiteV1 {
        block: expanded as u32,
        statement,
    }
}

fn getter_site(view: &SemanticExpandedRootV1) -> SemanticTransparentBorrowSiteV1 {
    let block = view
        .block_origins()
        .iter()
        .position(|origin| {
            view.instances()[origin.instance().index() as usize].function_identity()
                == SemanticFunctionIdentityV1::from_sha256([33; 32])
                && origin.block().index() == 0
        })
        .unwrap();
    assert_eq!(
        view.block_origins()[block].statements()[0],
        SemanticExpandedStatementOriginV1::Source { statement: 0 }
    );
    SemanticTransparentBorrowSiteV1 {
        block: block as u32,
        statement: 0,
    }
}

#[test]
fn bound_read97_bind_only_registry_has_no_narrow_occurrence_or_matrix_alias() {
    for mutation in [Mutation::None, Mutation::SecondReference] {
        let source = fixture::source(mutation);
        assert!(!source.functions().iter().any(|f| matches!(
            f.defined_capability_contract(),
            Some(SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(_))
        )));
        let expansion = canonical_expansion(&source);
        let view = expansion.root(source.roots()[0]).unwrap();
        let bindings = expansion.defined_capability_bindings(&source).unwrap();
        assert_eq!(bindings.len(), 1);
        assert!(matches!(
            bindings[0].contract(),
            SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_)
        ));
        assert!(
            !view
                .instances()
                .iter()
                .any(|frame| frame.function_identity()
                    == SemanticFunctionIdentityV1::from_sha256([32; 32]))
        );
        let getter = view
            .instances()
            .iter()
            .find(|frame| {
                frame.function_identity() == SemanticFunctionIdentityV1::from_sha256([33; 32])
            })
            .unwrap();
        assert_eq!(getter.parent(), Some(bindings[0].caller_instance()));
        let facts = MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, 65_536).unwrap();
        assert_eq!(facts.pairs.get(&ty(8)), Some(&ty(7)));
        assert_eq!(facts.carrier_barriers(), &BTreeSet::from([ty(7)]));
        assert_eq!(facts.carrier_leaves(), &BTreeMap::from([(ty(5), ty(2))]));
        assert_eq!(
            facts.policy_carrier_leaves(),
            &BTreeMap::from([(ty(6), ty(3))])
        );
        assert_eq!(
            facts.pairs.len(),
            if mutation == Mutation::SecondReference {
                4
            } else {
                3
            }
        );
        if mutation == Mutation::SecondReference {
            assert_eq!(facts.pairs.get(&ty(13)), Some(&ty(7)));
        }
    }
}

#[test]
fn bound_read97_capture_keeps_original_getter_and_narrow_overlap() {
    for source in [
        fixture::source(Mutation::None),
        canonical_fixture::source(canonical_fixture::Mutation::None),
    ] {
        let expansion = canonical_expansion(&source);
        let view = expansion.root(source.roots()[0]).unwrap();
        let bindings = expansion.defined_capability_bindings(&source).unwrap();
        let facts = MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, 65_536).unwrap();
        let mut cursor = facts.capture_cursor(&mut |_| Ok(())).unwrap();
        let mut counts = [0, 0];
        for (block, body) in view.body().blocks().iter().enumerate() {
            for (statement, node) in body.statements().iter().enumerate() {
                let site = SemanticTransparentBorrowSiteV1 {
                    block: block as u32,
                    statement: statement as u32,
                };
                let captured = facts.captured(site, node.kind(), &mut |_| Ok(())).unwrap();
                assert_eq!(
                    cursor.captured(site, node.kind(), &mut |_| Ok(())).unwrap(),
                    captured
                );
                assert!(
                    facts
                        .captured(site, &node.kind().clone(), &mut |_| Ok(()))
                        .unwrap()
                        .is_none()
                );
                assert!(
                    facts
                        .copied_matrix_field(site, &node.kind().clone(), &mut |_| Ok(()))
                        .unwrap()
                        .is_none()
                );
                let read = if captured.is_none() {
                    facts
                        .copied_matrix_field(site, node.kind(), &mut |_| Ok(()))
                        .unwrap()
                } else {
                    None
                };
                let Some(locals) = captured.or(read.as_ref().map(|locals| locals.as_slice()))
                else {
                    continue;
                };
                assert_eq!(
                    view.block_origins()[block].statements()[statement],
                    SemanticExpandedStatementOriginV1::Source {
                        statement: statement as u32
                    }
                );
                let SemanticStatementKindV1::Assign(a) = node.kind() else {
                    panic!("assignment")
                };
                match a.value().kind() {
                    SemanticRvalueKindV1::Aggregate(_) => counts[0] += 1,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) => {
                        assert_eq!(
                            captured.is_some(),
                            bindings.len() == 2,
                            "only the checked Narrow getter is stored in the old map"
                        );
                        assert_eq!(read.is_some(), bindings.len() == 1);
                        assert_eq!(
                            view.instances()
                                [view.block_origins()[block].instance().index() as usize]
                                .function_identity(),
                            SemanticFunctionIdentityV1::from_sha256([33; 32])
                        );
                        assert_eq!(locals, &[place.local().index()]);
                        assert_eq!(place.projections().len(), 2);
                        assert_eq!(place.projections()[0].result_type(), ty(7));
                        assert_eq!(
                            place.projections()[1].kind(),
                            SemanticProjectionKindV1::Field(0)
                        );
                        assert_eq!(place.ty(), ty(5));
                        counts[1] += 1;
                    }
                    _ => panic!("unexpected capture"),
                }
            }
        }
        assert_eq!(counts, [bindings.len(), 1]);
    }
}

#[test]
fn bound_read97_real_owner_queries_only_the_retained_borrow_use() {
    let source = fixture::source(Mutation::None);
    let bytes = source.canonical_encoding().to_vec();
    let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        &bytes,
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.canonical_encoding(), bytes);
    let mir = ProductionSemanticMirOwnerV1::try_new(decoded, Default::default()).unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(mir, Default::default()).unwrap();
    owner.verify_replay().unwrap();
    assert!(!owner.grants_proof_or_artifact_authority());
    let source = owner.source_semantic();
    let root = source.roots()[0];
    let view = owner.execution_view_for_root(root).unwrap();
    let (site, place) = root_borrow(view, 7);
    let sites = adapter::borrowed_workgroup_v1::execution_sites(
        source,
        owner.execution_expansion(),
        view,
        65_536,
    )
    .unwrap();
    assert!(sites.contains(&site));
    let mut range = None;
    let (input, _, _) = adapter::semantic_function_ssa_input_with_event_origins_v1(
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
    let plain = adapter::semantic_function_ssa_input_v1(
        view.body(),
        Some(source.types()),
        source.callables(),
        &BTreeSet::new(),
    )
    .0;
    assert_eq!(
        input.blocks(),
        plain.blocks(),
        "no synthetic Use or transport event"
    );
    assert!(input.promotable()[place.local().index() as usize]);
    let plan = plan_ssa_v1(&input).unwrap();
    let range = range.unwrap();
    let uses = plan
        .resolved_events(SsaBlockIdV1::new(site.block))
        .unwrap()
        .iter()
        .filter_map(|(event, resolved)| match resolved {
            SsaResolvedEventV1::Use { variable, value }
                if range.contains(&(*event as usize))
                    && variable.get() == place.local().index() =>
            {
                Some(*value)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(uses.len(), 1);
    let query = owner.source_query_for_root(root, view.body()).unwrap();
    let query_site = ProductionSemanticSsaSourceSiteV1::new(
        SemanticBlockIdV1::from_index(site.block),
        Some(site.statement),
    );
    assert_eq!(
        query
            .borrow_place_use(query_site, place, &mut || true)
            .unwrap(),
        uses[0]
    );
    assert_eq!(
        query.borrow_place_use(query_site, &place.clone(), &mut || true),
        Err(ProductionSemanticSsaSourceQueryErrorV1::OperandOutsideSite)
    );
    assert!(
        query
            .borrow_place_use(query_site, place, &mut || false)
            .is_err()
    );
    assert!(
        owner
            .source_query_for_root(root, &view.body().clone())
            .is_err()
    );
    assert_eq!(source.canonical_encoding(), bytes);
}

#[test]
fn bound_read97_alias_escape_duplicate_and_whole_copy_still_demote() {
    for mutation in [
        Mutation::DuplicateBorrow,
        Mutation::ExtraCapture,
        Mutation::ProjectedCopy,
        Mutation::AliasEscape,
    ] {
        let source = fixture::source(mutation);
        let expansion = canonical_expansion(&source);
        let view = expansion.root(source.roots()[0]).unwrap();
        let (site, place) = root_borrow(view, 7);
        let sites =
            adapter::borrowed_workgroup_v1::execution_sites(&source, &expansion, view, 65_536)
                .unwrap();
        assert!(!sites.contains(&site), "{mutation:?}");
        let input = adapter::semantic_function_ssa_input_v1(
            view.body(),
            Some(source.types()),
            source.callables(),
            &sites,
        )
        .0;
        let plain = adapter::semantic_function_ssa_input_v1(
            view.body(),
            Some(source.types()),
            source.callables(),
            &BTreeSet::new(),
        )
        .0;
        assert_eq!(input.blocks(), plain.blocks());
        assert!(
            !input.promotable()[place.local().index() as usize],
            "{mutation:?}"
        );
    }
}

#[test]
fn bound_read97_dead_and_deinitialized_owner_keep_distinct_rejections() {
    for mutation in [Mutation::DeadOwner, Mutation::Deinitialize] {
        let source = fixture::source(mutation);
        let expansion = canonical_expansion(&source);
        let view = expansion.root(source.roots()[0]).unwrap();
        let (site, place) = root_borrow(view, 7);
        let local = place.local().index();
        let sites =
            adapter::borrowed_workgroup_v1::execution_sites(&source, &expansion, view, 65_536)
                .unwrap();
        let mut range = None;
        let input = adapter::semantic_function_ssa_input_with_event_origins_v1(
            view.body(),
            Some(source.types()),
            source.callables(),
            &sites,
            |block, statement, events| {
                if block == site.block && statement == Some(site.statement) {
                    range = Some(events);
                }
            },
        )
        .0;
        let range = range.unwrap();
        if mutation == Mutation::DeadOwner {
            assert!(input.promotable()[local as usize]);
            assert!(
                matches!(plan_ssa_v1(&input), Err(SsaPlannerErrorV1::UndefinedAtUse { variable, .. }) if variable.get() == local)
            );
        } else {
            assert!(!input.promotable()[local as usize]);
            let plan = plan_ssa_v1(&input).unwrap();
            assert!(!plan.resolved_events(SsaBlockIdV1::new(site.block)).unwrap().iter().any(|(event, resolved)|
                range.contains(&(*event as usize)) && matches!(resolved, SsaResolvedEventV1::Use { variable, .. } if variable.get() == local)));
        }
    }
}

#[test]
fn bound_read97_field_policy_move_nested_write_unknown_and_narrow_are_not_reads() {
    for mutation in [
        Mutation::PolicyField,
        Mutation::MoveField,
        Mutation::NestedRead,
        Mutation::ProjectedDestination,
        Mutation::UnknownWrapper,
        Mutation::NarrowField,
    ] {
        let source = fixture::source(mutation);
        let expansion = canonical_expansion(&source);
        let view = expansion.root(source.roots()[0]).unwrap();
        let bindings = expansion.defined_capability_bindings(&source).unwrap();
        let facts = MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, 65_536).unwrap();
        let (block, statement, original_local) = match mutation {
            Mutation::UnknownWrapper => (4, 3, 18),
            Mutation::NarrowField => (5, 1, 9),
            _ => (4, 1, 7),
        };
        let site = original_site(view, block, statement);
        let node = &view.body().blocks()[site.block as usize].statements()[site.statement as usize];
        assert!(
            facts
                .captured(site, node.kind(), &mut |_| Ok(()))
                .unwrap()
                .is_none(),
            "{mutation:?}"
        );
        assert!(
            facts
                .copied_matrix_field(site, node.kind(), &mut |_| Ok(()))
                .unwrap()
                .is_none(),
            "{mutation:?}"
        );
        if mutation == Mutation::UnknownWrapper {
            assert!(!facts.pairs.contains_key(&ty(14)));
            assert!(!facts.carrier_barriers().contains(&ty(13)));
            // An ordinary aggregate may have an independent carrier route.
            // This negative is specifically absence of checked-wrapper facts.
            continue;
        }
        if mutation == Mutation::NarrowField {
            assert_eq!(facts.pairs.get(&ty(13)), Some(&ty(9)));
            assert!(facts.carrier_barriers().contains(&ty(9)));
        }
        let (borrow, place) = root_borrow(view, original_local);
        let sites =
            adapter::borrowed_workgroup_v1::execution_sites(&source, &expansion, view, 65_536)
                .unwrap();
        assert!(!sites.contains(&borrow), "{mutation:?}");
        let input = adapter::semantic_function_ssa_input_v1(
            view.body(),
            Some(source.types()),
            source.callables(),
            &sites,
        )
        .0;
        assert!(
            !input.promotable()[place.local().index() as usize],
            "{mutation:?}"
        );
    }
}

#[test]
fn bound_read97_missing_foreign_and_duplicate_bindings_do_not_seed_reads() {
    let source = fixture::source(Mutation::None);
    let expansion = canonical_expansion(&source);
    let view = expansion.root(source.roots()[0]).unwrap();
    let bindings = expansion.defined_capability_bindings(&source).unwrap();
    let missing = MatrixBorrowSitesV1::new(&source, &expansion, view, &[], 0).unwrap();
    assert!(missing.pairs.is_empty());
    assert!(missing.carrier_barriers().is_empty());
    assert_eq!(missing.work_units, 0);
    let site = getter_site(view);
    let kind = view.body().blocks()[site.block as usize].statements()[0].kind();
    assert!(
        missing
            .copied_matrix_field(site, kind, &mut |_| panic!(
                "empty registry has no lazy-read debit"
            ))
            .unwrap()
            .is_none()
    );
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
    let other = fixture::source(Mutation::SecondReference);
    assert!(matches!(
        MatrixBorrowSitesV1::new(&other, &expansion, view, &bindings, 65_536),
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
fn bound_read97_independent_small_trace_and_every_short_constructor_budget() {
    let source = fixture::source(Mutation::None);
    let expansion = canonical_expansion(&source);
    let view = expansion.root(source.roots()[0]).unwrap();
    let bindings = expansion.defined_capability_bindings(&source).unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(source.types().len(), 13);
    assert_eq!(
        source
            .types()
            .iter()
            .filter(|t| matches!(t.shape(), SemanticTypeShapeV1::Pointer(_)))
            .count(),
        4
    );
    // One Bind: 32 + 32 + pair inserts(3,4) + capture9 + leaves(5,5) + barrier3.
    // Type scan: 13 visits, four pointer checks10, one wrapper pair insert5.
    // Retain the type slice and view: three words, no body scan or new capture.
    let registration = 32 + 32 + 3 + 4 + 9 + 5 + 5 + 3;
    let type_scan = 13 + 4 * 10 + 5;
    let work = registration + type_scan + 3;
    let required = 32 + work;
    let facts = MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, required).unwrap();
    assert_eq!(facts.work_units, work);
    for limit in 0..required {
        assert!(
            matches!(
                MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, limit),
                Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                    resource: SsaPlannerResourceV1::WorkUnits,
                    ..
                })
            ),
            "limit={limit}"
        );
    }
    assert!(MatrixBorrowSitesV1::new(&source, &expansion, view, &bindings, required + 1).is_ok());

    let site = getter_site(view);
    let kind = view.body().blocks()[site.block as usize].statements()[0].kind();
    let SemanticStatementKindV1::Assign(assignment) = kind else {
        panic!("getter assignment")
    };
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = assignment.value().kind()
    else {
        panic!("original field Copy")
    };
    let expected = Some([place.local().index()]);
    let mut trace = Vec::new();
    assert_eq!(
        facts
            .copied_matrix_field(site, kind, &mut |amount| {
                trace.push(amount);
                Ok(())
            })
            .unwrap(),
        expected
    );
    assert_eq!(trace, [1, 4, 2, 18]);
    let mut clone_trace = Vec::new();
    assert!(
        facts
            .copied_matrix_field(site, &kind.clone(), &mut |amount| {
                clone_trace.push(amount);
                Ok(())
            })
            .unwrap()
            .is_none()
    );
    assert_eq!(clone_trace, trace);
    // Independent lazy-read budget, not a new production allowance or cache.
    for limit in 0..=25 {
        let mut remaining = limit;
        let result = facts.copied_matrix_field(site, kind, &mut |amount| {
            if amount > remaining {
                return Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                    resource: SsaPlannerResourceV1::WorkUnits,
                    required: amount,
                    limit: remaining,
                });
            }
            remaining -= amount;
            Ok(())
        });
        if limit == 25 {
            assert_eq!(result.unwrap(), expected);
            assert_eq!(remaining, 0);
        } else {
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit { .. })
                ),
                "lazy limit={limit}"
            );
        }
    }
    assert_eq!(facts.work_units, work);
    assert!(
        facts
            .captured(site, kind, &mut |_| Ok(()))
            .unwrap()
            .is_none(),
        "lazy reads never allocate or publish a stored capture"
    );
}
