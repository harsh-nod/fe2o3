use std::{cell::Cell, mem::size_of};

fn accepted_shared_reader_case(
    inspect: impl for<'s, 'm, 'g> FnOnce(
        &ProductionCanonicalRankedMetadataV1<'m>,
        &fe2o3_pliron::CheckedCanonicalTrapPoliciesV1<'s, 'g>,
        &mut Budget<'_>,
    ) -> Result<(), Af>,
) {
    let owner = literal(true, true);
    let before = owner.executable().canonical().canonical_bytes().to_vec();
    run(&owner, |view, budget| {
        assert_eq!(view.assertion_count(budget)?, 2);
        let source = view.metadata(budget)?;
        let policies = view.policies(budget)?;
        assert!(std::ptr::eq(policies.owner(budget)?, owner.executable()));
        assert_eq!(policies.definition_count(budget)?, 4);
        assert_eq!(policies.pair_count(budget)?, 1);
        assert_eq!(policies.pair(0, budget)?.incoming_edges().len(), 1);
        for ordinal in 0..4 {
            let report = policies.report(ordinal, budget)?;
            assert_eq!(report.paired_stage_count(), 9);
            assert!(report.reports().is_clean());
        }
        let floor = budget.storage();
        inspect(source, policies, budget)?;
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), None);
        Ok(())
    })
    .unwrap();
    assert_eq!(owner.executable().canonical().canonical_bytes(), before);
}

#[test]
fn reader_only_missing_and_duplicate_root_aliases_are_refused() {
    accepted_shared_reader_case(|source, policies, budget| {
        canonical_assertion_v1::read_test_complete_alias_roster_v1(source, policies, budget)
    });
}

#[test]
fn reader_only_missing_and_duplicate_proved_sink_aliases_are_refused() {
    accepted_shared_reader_case(|source, policies, budget| {
        canonical_assertion_v1::read_test_proved_sink_aliases_v1(source, policies, budget)
    });
}

#[test]
fn reader_only_missing_and_extra_synthetic_counterparts_are_refused() {
    accepted_shared_reader_case(|source, policies, budget| {
        canonical_assertion_v1::read_test_synthetic_aliases_v1(source, policies, budget)
    });
}

fn empty_callable_owner(zst_assignment: bool) -> ProductionPreRankedKirOwnerV1 {
    let erased_owner = erased();
    let seed = erased_owner.original_source();
    let source = seed.semantic_ssa().source_semantic();
    assert_eq!(source.functions().len(), 2);
    let helper = &source.functions()[1];
    let unit_ty = helper.locals()[0].ty();
    assert_eq!(unit_ty, SemanticTypeIdV1::from_index(0));
    let unit = &source.types()[unit_ty.index() as usize];
    assert!(matches!(unit.shape(), SemanticTypeShapeV1::Unit));
    // Removing the private-memory body also removes its only scalar type uses.
    let mut types = vec![unit.clone()];
    let mut locals = vec![helper.locals()[0].clone()];
    let mut statements = Vec::new();
    if zst_assignment {
        let identity = SemanticTypeIdentityV1::from_sha256([203; 32]);
        assert!(types.last().unwrap().identity() < identity);
        let zst = SemanticTypeIdV1::from_index(types.len() as u32);
        let layout_identity = SemanticLayoutIdentityV1::from_sha256([204; 32]);
        assert!(
            types
                .iter()
                .all(|ty| ty.layout_identity() != layout_identity)
        );
        types.push(SemanticTypeDeclV1::new(
            identity,
            layout_identity,
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        ));
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([218; 32]),
            zst,
            SemanticLocalRoleV1::Temporary,
            helper.source(),
        ));
        statements.push(assign(
            1,
            zst,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                zst,
                SemanticConstantValueV1::ZeroSized,
            ))),
        ));
    }
    let mut functions = source.functions().to_vec();
    functions[1] = copy_function(
        helper,
        locals,
        vec![body(212, statements, SemanticTerminatorKindV1::Return)],
    );
    rebuild(seed, types, functions)
}

fn callable_mapping(
    owner: &ProductionPreRankedKirOwnerV1,
    kind: ProductionCanonicalAssertionCallKindV1,
    decision: fe2o3_mir_model::SemanticCallableDecisionV1,
    memory: [usize; 2],
    has_zst_assignment: bool,
) {
    let before = owner.executable().canonical().canonical_bytes().to_vec();
    run(owner, |view, budget| {
        assert_eq!(view.memory_census(budget)?, memory);
        assert_eq!(view.assertion_count(budget)?, 0);
        let rows = view.callables(budget)?;
        let mut helpers = rows.iter().filter(|row|
            row.kind() != ProductionCanonicalAssertionCallKindV1::Root);
        let helper = helpers.next().expect("genuine retained helper association");
        assert!(helpers.next().is_none());
        assert_eq!(helper.source_function(), SemanticFunctionIdV1::from_index(1));
        assert_eq!(helper.kind(), kind);
        assert_eq!(helper.source_decision(), decision);
        let metadata = view.metadata(budget)?;
        let association = metadata.function(helper.association(), budget)?;
        assert_eq!(association.source().semantic_function(), helper.source_function());
        assert_eq!(association.canonical().coordinate, helper.function());
        let inventory = metadata.inventory(budget)?;
        assert!(inventory.calls().iter().any(|call| call.target == Some(helper.function())));
        if has_zst_assignment {
            let mut assignments = metadata.spans(budget)?.iter().filter(|span|
                span.association() == helper.association()
                    && matches!(span.site(), ProductionCanonicalRankedSourceSiteV1::Statement {
                        source, ..
                    } if matches!(source.kind(), SemanticStatementKindV1::Assign(_))));
            let assignment = assignments.next().expect("actual zero-emission source assignment");
            assert!(assignments.next().is_none());
            assert!(assignment.operations().is_empty());
            let actual = &owner.semantic_ssa().source_semantic().functions()[1];
            let SemanticStatementKindV1::Assign(assignment) = actual.blocks()[0].statements()[0].kind()
            else { panic!("actual aggregate source assignment"); };
            assert!(matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(
                SemanticOperandV1::Constant(value)) if matches!(value.value(), SemanticConstantValueV1::ZeroSized)));
            let ty = &owner.semantic_ssa().source_semantic().types()[assignment.destination().ty().index() as usize];
            assert!(matches!(ty.shape(), SemanticTypeShapeV1::Aggregate(fields) if fields.fields().is_empty()));
            assert_eq!(ty.layout().size_bytes(), Some(0));
        }
        let policies = view.policies(budget)?;
        assert_eq!(policies.definition_count(budget)?, 2);
        assert_eq!(policies.pair_count(budget)?, 0);
        let mut matched = 0;
        for ordinal in 0..2 {
            let coordinate = policies.definition_coordinate(ordinal, budget)?;
            assert_eq!(policies.history(ordinal, budget)?.function(), coordinate.0 as usize);
            let report = policies.report(ordinal, budget)?;
            assert_eq!(report.paired_stage_count(), 9);
            assert!(report.reports().is_clean());
            matched += usize::from(coordinate == helper.function());
        }
        assert_eq!(matched, 1);
        assert_eq!(policies.pending_obligations().iter().count(), 19);
        assert!(!view.ranked_verification_is_complete());
        assert!(!view.grants_artifact_or_launch_authority());
        Ok(())
    }).unwrap();
    assert_eq!(owner.executable().canonical().canonical_bytes(), before);
}

#[test]
fn public_b_deterministic_empty_helper_keeps_exact_source_decision() {
    let owner = empty_callable_owner(false);
    callable_mapping(
        &owner,
        ProductionCanonicalAssertionCallKindV1::DeterministicEmpty,
        fe2o3_mir_model::SemanticCallableDecisionV1::ExactEmptyDeterministicScalar,
        [0, 0],
        false,
    );
}

#[test]
fn public_b_empty_only_zst_helper_is_not_promoted_to_deterministic() {
    let owner = empty_callable_owner(true);
    callable_mapping(
        &owner,
        ProductionCanonicalAssertionCallKindV1::EmptyOnly,
        fe2o3_mir_model::SemanticCallableDecisionV1::ExactEmptyOnly,
        [0, 0],
        true,
    );
}

#[test]
fn public_b_private_frame_retains_rejected_source_summary() {
    let erased_owner = erased();
    callable_mapping(
        erased_owner.original_source(),
        ProductionCanonicalAssertionCallKindV1::PrivateFrame,
        fe2o3_mir_model::SemanticCallableDecisionV1::Rejected,
        [1, 2],
        false,
    );
}

#[test]
fn public_b_nested_payload_destructor_panic_keeps_exact_observations() {
    use std::{
        panic::panic_any,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };
    struct Trace {
        next: AtomicUsize,
        counts: [AtomicUsize; 3],
        order: [AtomicUsize; 3],
        tokens: [AtomicUsize; 3],
    }
    struct Payload {
        trace: Arc<Trace>,
        token: usize,
        generation: usize,
        terminal: usize,
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            let generation = self.generation;
            self.trace.counts[generation].fetch_add(1, Ordering::SeqCst);
            self.trace.tokens[generation].store(self.token, Ordering::SeqCst);
            let order = self.trace.next.fetch_add(1, Ordering::SeqCst) + 1;
            self.trace.order[generation].store(order, Ordering::SeqCst);
            if generation < self.terminal {
                panic_any(Payload {
                    trace: Arc::clone(&self.trace),
                    token: self.token,
                    generation: generation + 1,
                    terminal: self.terminal,
                });
            }
        }
    }
    struct PaidProbe<'a, 'w> {
        budget: &'a Budget<'w>,
        storage: &'a Cell<usize>,
        work: &'a Cell<usize>,
        drops: &'a Cell<usize>,
    }
    impl Drop for PaidProbe<'_, '_> {
        fn drop(&mut self) {
            self.storage.set(self.budget.storage());
            self.work.set(self.budget.work());
            self.drops.set(self.drops.get() + 1);
        }
    }
    for terminal in [1, 2] {
        let owner = literal(true, false);
        run(&owner, |view, budget| {
            assert_eq!(view.assertion_count(budget)?, 1);
            assert_eq!(
                view.policies(budget)?
                    .report(0, budget)?
                    .paired_stage_count(),
                9
            );
            Ok(())
        })
        .unwrap();
        let trace = Arc::new(Trace {
            next: AtomicUsize::new(0),
            counts: std::array::from_fn(|_| AtomicUsize::new(0)),
            order: std::array::from_fn(|_| AtomicUsize::new(0)),
            tokens: std::array::from_fn(|_| AtomicUsize::new(0)),
        });
        let token = 0x271_b11 + terminal;
        let entry_floor = Cell::new(0);
        let callback = Cell::new(None);
        let native = Cell::new(None);
        let probe_storage = Cell::new(0);
        let probe_work = Cell::new(0);
        let probe_drops = Cell::new(0);
        let mut work = Work::new(1 << 48);
        let mut budget = Budget::new(&mut work, S);
        // Payload/probe credit is caller-owned; Arc/atomic instrumentation and
        // Rust's panic allocator are not a production allocation oracle.
        let floor = owner.unit_local_source_storage_floor_v1().unwrap()
            + (terminal + 1) * size_of::<Payload>()
            + size_of::<PaidProbe<'_, '_>>()
            + SIBLING;
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let slot = std::ptr::from_ref(&budget) as usize;
        let error = owner
            .with_checked_canonical_ranked_source_v1(&mut budget, |source, budget| {
                entry_floor.set(budget.storage());
                Ok(source.with_assertion_policy_checks_v1(
                    budget,
                    |view, budget| -> Result<(), Af> {
                        assert_eq!(view.assertion_count(budget)?, 1);
                        let policies = view.policies(budget)?;
                        assert_eq!(policies.report(0, budget)?.paired_stage_count(), 9);
                        native.set(Some(policies.observation(budget)?));
                        callback.set(Some((
                            budget.work(),
                            budget.storage(),
                            budget.peak_storage(),
                        )));
                        let _probe = PaidProbe {
                            budget,
                            storage: &probe_storage,
                            work: &probe_work,
                            drops: &probe_drops,
                        };
                        panic_any(Payload {
                            trace: Arc::clone(&trace),
                            token,
                            generation: 0,
                            terminal,
                        });
                    },
                ))
            })
            .unwrap()
            .unwrap_err();
        // Keep assertions outside production catches so incidental test panics
        // cannot be mistaken for the deliberately nested payload sequence.
        let (callback_work, paid, peak) = callback.get().expect("genuine public B callback");
        assert!(matches!(error.failure, Af::Panicked));
        let native = native.get().unwrap();
        assert_eq!(error.policies, Some(native));
        assert_eq!(native.first_denial(), None);
        assert!(!native.caught_panic());
        let observed = error.source.expect("original source ledger observation");
        // B's exact postflight performs one B, one source and one C-owner query.
        // This local recurrence is not a whole-pipeline resource oracle.
        assert_eq!(observed.work, callback_work + 3);
        assert_eq!(observed.storage, entry_floor.get());
        assert_eq!(observed.peak, peak);
        assert_eq!(observed.failed_storage, None);
        assert_eq!(probe_drops.get(), 1);
        assert_eq!(probe_storage.get(), paid);
        assert_eq!(probe_work.get(), callback_work);
        assert_eq!(budget.work(), callback_work + 3);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), peak);
        assert_eq!(budget.failed_storage(), None);
        assert_eq!(std::ptr::from_ref(&budget) as usize, slot);
        assert!(budget.work_ledger_identity_v1() == ledger);
        for generation in 0..3 {
            let reached = generation <= terminal;
            assert_eq!(
                trace.counts[generation].load(Ordering::SeqCst),
                usize::from(reached)
            );
            assert_eq!(
                trace.order[generation].load(Ordering::SeqCst),
                if reached { generation + 1 } else { 0 }
            );
            assert_eq!(
                trace.tokens[generation].load(Ordering::SeqCst),
                if reached { token } else { 0 }
            );
        }
        assert_eq!(trace.next.load(Ordering::SeqCst), terminal + 1);
        drop(owner);
        drop(trace);
        budget.release_storage(floor - SIBLING).unwrap();
        assert_eq!(budget.storage(), SIBLING);
        assert_eq!(work.failed_work(), None);
    }
}
