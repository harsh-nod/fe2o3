use super::*;

type Site = SemanticTransparentBorrowSiteV1;

// Reconstruct only this admitted fixture's reference tree. Production still
// performs its complete candidate/use classification before calling prove.
struct Facts {
    candidates: Vec<SemanticBorrowCandidateV1>,
    children: Vec<Vec<usize>>,
    uses: Vec<Vec<Site>>,
    mutable: BTreeSet<usize>,
    root: usize,
}

impl Facts {
    fn new(semantic: &AdmittedInertSemanticMirV1, view: &SemanticExpandedRootV1) -> Self {
        let body = view.body();
        let mut candidates = Vec::new();
        let mut destinations = BTreeMap::new();
        let mut mutable = BTreeSet::new();
        for (b, block) in body.blocks().iter().enumerate() {
            for (s, statement) in block.statements().iter().enumerate() {
                let SemanticStatementKindV1::Assign(a) = statement.kind() else {
                    continue;
                };
                let SemanticTypeShapeV1::Pointer(reference) =
                    semantic.types()[a.destination().ty().index() as usize].shape()
                else {
                    continue;
                };
                if reference.kind() != SemanticPointerKindV1::Reference {
                    continue;
                }
                assert_eq!(reference.pointee(), ty(1));
                let (from, alias) = match a.value().kind() {
                    SemanticRvalueKindV1::Borrow { place, .. } => (place, false),
                    SemanticRvalueKindV1::Use(
                        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
                    ) => (place, true),
                    _ => panic!("unexpected fixture reference assignment"),
                };
                let source_reference =
                    (alias || !from.projections().is_empty()).then_some(from.local().index());
                if source_reference.is_none() {
                    assert_eq!(from.local().index(), 1);
                }
                let index = candidates.len();
                assert!(
                    destinations
                        .insert(a.destination().local().index(), index)
                        .is_none()
                );
                if reference.mutability() == SemanticMutabilityV1::Mutable {
                    mutable.insert(index);
                }
                candidates.push(SemanticBorrowCandidateV1 {
                    site: site(b as u32, s as u32),
                    source_local: 1,
                    source_type: ty(1),
                    source_reference,
                    value_alias: alias, source_kind: SemanticBorrowCandidateSourceV1::Direct,
                    valid: true,
                    consumers: 0,
                    intrinsic_consumer: false,
                });
            }
        }
        let mut children = vec![Vec::new(); candidates.len()];
        let mut uses = children.iter().map(|_| Vec::new()).collect::<Vec<_>>();
        let mut roots = Vec::new();
        for (index, c) in candidates.iter().enumerate() {
            if let Some(from) = c.source_reference {
                children[*destinations.get(&from).unwrap()].push(index);
            } else {
                roots.push(index);
            }
        }
        assert_eq!(roots.len(), 1);
        for (b, block) in body.blocks().iter().enumerate() {
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
                assert!(matches!(
                    semantic.callables()[call.callee().index() as usize],
                    SemanticCallableDeclV1::CompilerIntrinsic { .. }
                ));
                for operand in call.arguments() {
                    let (SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p)) = operand else {
                        panic!("unexpected fixture call operand");
                    };
                    let index = *destinations.get(&p.local().index()).unwrap();
                    uses[index].push(site(b as u32, block.statements().len() as u32));
                    candidates[index].intrinsic_consumer = true;
                }
            }
        }
        for (i, c) in candidates.iter_mut().enumerate() {
            c.consumers = (children[i].len() + uses[i].len()) as u32;
        }
        Self {
            candidates,
            children,
            uses,
            mutable,
            root: roots[0],
        }
    }
}

fn run(
    body: &SemanticFunctionDeclV1,
    facts: &Facts,
    transfers: Option<&CheckedTransfers<'_>>,
    dfs: bool,
    limit: usize,
    prefix: usize,
) -> (
    Result<bool, ProductionSemanticSsaErrorV1>,
    usize,
    [usize; 11],
) {
    let mut work = budget(limit);
    work.charge(prefix).unwrap();
    let mut action = || {
        ordered_context_loans::prove(
            body,
            &facts.candidates,
            &facts.children,
            &facts.uses,
            &facts.mutable,
            facts.root,
            transfers,
            &mut work,
        )
    };
    let result = if dfs {
        ordered_context_loans::with_dfs_reference(action)
    } else {
        action()
    };
    assert_eq!(work.profile.proof_calls, 1);
    let ProductionSemanticSsaErrorV1::BorrowFlowWork {
        phase_work_units, ..
    } = work.profile.failure(
        work.remaining,
        0,
        ProductionSemanticSsaErrorV1::ReplayMismatch,
    )
    else {
        panic!("expected fixed profile")
    };
    (result, limit - work.remaining, phase_work_units)
}

fn equivalent(
    body: &SemanticFunctionDeclV1,
    facts: &Facts,
    transfers: Option<&CheckedTransfers<'_>>,
    expected: bool,
) -> (usize, usize) {
    let old = run(body, facts, transfers, true, MAX_FLOW_WORK, 127);
    let new = run(body, facts, transfers, false, MAX_FLOW_WORK, 127);
    assert_eq!(old.0.unwrap(), expected);
    assert_eq!(new.0.unwrap(), expected);
    for stage in 0..11 {
        if stage != FlowWorkStage::Paths as usize {
            assert_eq!(old.2[stage], new.2[stage], "non-path stage {stage}");
        }
    }
    (old.1, new.1)
}

#[test]
fn scc_complete_owner_proof_matches_dfs_with_real_copy_transfer_custody() {
    for (case, change, expected) in [
        (0, Change::None, true),
        (1, Change::MoveArgument, true),
        (2, Change::ByValueHelper, false),
        (3, Change::OrdinaryCopy, false),
        (4, Change::LateSharedUse, false),
        (5, Change::DeadOwner, false),
        (6, Change::Backedge, false),
    ] {
        let semantic = admitted(change);
        let source_identity = semantic.semantic_sha256().clone();
        let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
        expansion.verify_replay(&semantic).unwrap();
        let view = expansion.root(semantic.roots()[0]).unwrap();
        let before = view.body().clone();
        let transfers = CheckedTransfers::new(&semantic, &expansion, view).unwrap();
        let facts = Facts::new(&semantic, view);
        let (old, new) = equivalent(view.body(), &facts, Some(&transfers), expected);
        if expected {
            let observed = run(
                view.body(),
                &facts,
                Some(&transfers),
                false,
                MAX_FLOW_WORK,
                0,
            );
            assert!(observed.2[FlowWorkStage::Transfers as usize] >= 96);
        }
        assert_eq!(semantic.semantic_sha256(), source_identity);
        assert_eq!(view.body(), &before);
        eprintln!("admitted owner case {case}: DFS work {old}, SCC work {new}");
    }
}

#[test]
fn scc_full_proof_rejects_missing_substituted_and_cloned_transfer_owners() {
    let semantic = admitted(Change::None);
    let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
    let view = expansion.root(semantic.roots()[0]).unwrap();
    let facts = Facts::new(&semantic, view);
    let transfers = CheckedTransfers::new(&semantic, &expansion, view).unwrap();
    equivalent(view.body(), &facts, None, false);
    equivalent(&view.body().clone(), &facts, Some(&transfers), false);
    let other_expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
    let other_view = other_expansion.root(semantic.roots()[0]).unwrap();
    let other = CheckedTransfers::new(&semantic, &other_expansion, other_view).unwrap();
    equivalent(view.body(), &facts, Some(&other), false);
    assert!(CheckedTransfers::new(&semantic, &expansion, other_view).is_err());
    let other_source = admitted(Change::MoveArgument);
    assert!(CheckedTransfers::new(&other_source, &expansion, view).is_err());
}

#[test]
fn scc_full_owner_proof_retains_all_root_type_use_and_mutability_checks() {
    let semantic = admitted(Change::None);
    let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
    let view = expansion.root(semantic.roots()[0]).unwrap();
    let transfers = CheckedTransfers::new(&semantic, &expansion, view).unwrap();
    for mutation in 0..9 {
        let mut facts = Facts::new(&semantic, view);
        let root = facts.root;
        let child = facts.children[root][0];
        match mutation {
            0 => facts.candidates[child].source_type = ty(3),
            1 => facts.candidates[child].consumers += 1,
            2 => facts.candidates[child].valid = false,
            3 => {
                facts.mutable.remove(&root);
            }
            4 => {
                facts.uses[child].push(site(0, 0));
                facts.candidates[child].consumers += 1;
            }
            5 => {
                facts.children[child].push(root);
                facts.candidates[child].consumers += 1;
            }
            6 | 7 => {
                // Another root of this SAME owner must be checked, even if it
                // is not in the requested root's descendant tree.
                let mut extra = facts.candidates[root];
                if mutation == 6 {
                    extra.source_type = ty(3);
                } else {
                    extra.valid = false;
                }
                facts.candidates.push(extra);
                facts.children.push(vec![]);
                facts.uses.push(vec![]);
            }
            8 => facts.candidates[child].source_reference = Some(0),
            _ => unreachable!(),
        }
        equivalent(view.body(), &facts, Some(&transfers), false);
    }
}

#[test]
fn scc_complete_owner_proof_shares_spent_prefix_and_exact_exhaustion_error() {
    let semantic = admitted(Change::None);
    let expansion = SemanticCallExpansionV1::try_new(&semantic, Default::default()).unwrap();
    let view = expansion.root(semantic.roots()[0]).unwrap();
    let facts = Facts::new(&semantic, view);
    let transfers = CheckedTransfers::new(&semantic, &expansion, view).unwrap();
    for dfs in [false, true] {
        let prefix = 127;
        let complete = run(
            view.body(),
            &facts,
            Some(&transfers),
            dfs,
            MAX_FLOW_WORK,
            prefix,
        );
        assert!(complete.0.unwrap());
        for limit in prefix..=complete.1 {
            let observed = run(view.body(), &facts, Some(&transfers), dfs, limit, prefix);
            assert!(observed.1 <= limit);
            if limit == complete.1 {
                assert!(observed.0.unwrap());
                assert_eq!(observed.1, limit);
            } else {
                let ProductionSemanticSsaErrorV1::BorrowFlowWork {
                    remaining_work_units,
                    requested_work_units,
                    error,
                    ..
                } = observed.0.unwrap_err()
                else {
                    panic!("expected shared work failure")
                };
                assert!(requested_work_units > remaining_work_units);
                assert_eq!(limit - observed.1, remaining_work_units);
                assert!(
                    matches!(*error, ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                    resource: SsaPlannerResourceV1::WorkUnits, required, limit: actual
                } if actual == limit && required == limit + 1)
                );
            }
        }
    }
}
