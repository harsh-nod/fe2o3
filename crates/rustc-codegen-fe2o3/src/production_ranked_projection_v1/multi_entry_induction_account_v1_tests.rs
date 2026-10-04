use super::*;

// Deliberately retain the trait-default missing storage/identity custody.
struct NoCustodyFacts(usize);
impl ProjectedAssertionFactsV1 for NoCustodyFacts {
    fn charge_private_array_work(&mut self, amount: usize) -> Result<()> {
        self.0 += amount;
        Ok(())
    }
    fn private_array_initializer_count(&mut self, _: usize, _: usize) -> Result<Option<u64>> {
        Ok(None)
    }
    fn is_materialized_block(&mut self, _: usize) -> Result<bool> {
        Ok(true)
    }
    fn condition(
        &mut self,
        _: usize,
        _: bool,
        _: SemanticBlockIdV1,
    ) -> Result<ProjectedAssertionConditionV1> {
        Ok(ProjectedAssertionConditionV1::Dynamic)
    }
}

#[test]
fn multi_entry_scope_first_header_exact_and_one_byte_short() {
    let header = std::mem::size_of::<Scope>();
    for short in [0, 1] {
        let limit = 17 + header - short;
        let (result, work, peak, storage_denial, work_denial) =
            run_component(1, limit, fixture(), |_, scope, facts| {
                let result = Context { scope, facts }.charge(1);
                assert_eq!(scope.started, short == 0);
                assert_eq!(scope.retained, if short == 0 { header } else { 0 });
                result
            });
        if short == 0 {
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
            ));
            assert_eq!((work, peak, storage_denial), (1, limit, None));
        } else {
            let Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(Resource::Storage(error)),
            )) = result
            else {
                panic!("first header must require its complete reservation")
            };
            assert_eq!((error.actual(), error.limit()), (17 + header, limit));
            assert_eq!((work, peak, storage_denial), (0, 17, Some(17 + header)));
        }
        assert_eq!(work_denial, None);
    }
}

#[test]
fn multi_entry_scope_first_header_retry_after_extent_release_keeps_account_history() {
    // Measure the real zero-local extent header; the retry below uses one account.
    let extent_storage = {
        let mut work = Work::new(4);
        let mut budget = Budget::new(&mut work, usize::MAX);
        slice_extent_projection_v1::with_scope(0, &mut Facts(&mut budget), |extent| {
            extent.facts().scalar_private_storage_v1()
        })
        .unwrap()
    };
    let header = std::mem::size_of::<Scope>();
    let floor = 17;
    let attempted = floor + extent_storage + header;
    let mut work = Work::new(18);
    {
        let mut budget = Budget::new(&mut work, attempted - 1);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(13).unwrap();
        assert!(budget.charge_work(7).is_err());
        let account = Facts(&mut budget).helper_value_ledger_v1().unwrap();
        let result = with_scope(&mut Facts(&mut budget), |scope, facts| {
            slice_extent_projection_v1::with_scope(0, facts, |extent| {
                let facts = extent.facts();
                assert!(facts.helper_value_ledger_v1()? == account);
                assert_eq!(facts.scalar_private_storage_v1()?, floor + extent_storage);
                let result = Context { scope, facts }.charge(1);
                let Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(Resource::Storage(error)),
                )) = result
                else {
                    panic!("live extent scratch must deny the first induction header")
                };
                assert_eq!((error.actual(), error.limit()), (attempted, attempted - 1));
                assert!(!scope.started);
                assert_eq!(scope.retained, 0);
                Ok(())
            })?;
            assert_eq!((facts.0.storage(), facts.0.work()), (floor, 17));
            assert_eq!(facts.0.failed_storage(), Some(attempted));
            Context { scope, facts }.charge(1)?;
            assert!(scope.started);
            assert_eq!(scope.retained, header);
            assert!(facts.helper_value_ledger_v1()? == account);
            assert_eq!((facts.0.storage(), facts.0.work()), (floor + header, 18));
            let result = Context { scope, facts }.charge(1);
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(Resource::Work(_))
                ))
            ));
            Err(reject(DONE))
        });
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
        ));
        assert!(Facts(&mut budget).helper_value_ledger_v1().unwrap() == account);
        assert_eq!((budget.storage(), budget.work()), (floor, 18));
        assert_eq!(budget.peak_storage(), floor + extent_storage.max(header));
        assert_eq!(budget.failed_storage(), Some(attempted));
    }
    assert_eq!(work.failed_work(), Some(20));
}

#[test]
fn multi_entry_scope_cleanup_refuses_lost_floor_and_foreign_ledger_on_return_and_unwind() {
    let header = std::mem::size_of::<Scope>();
    for foreign in [false, true] {
        for unwind in [false, true] {
            let mut work = Work::new(1);
            let mut other_work = Work::new(1);
            let mut budget = Budget::new(&mut work, 17 + header);
            let mut other = Budget::new(&mut other_work, 17 + header);
            budget.reserve_storage(17).unwrap();
            other.reserve_storage(17 + header).unwrap();
            let account = budget.work_ledger_identity_v1();
            let other_account = other.work_ledger_identity_v1();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                with_scope(&mut Facts(&mut budget), |scope, facts| {
                    Context { scope, facts }.charge(1)?;
                    if foreign {
                        std::mem::swap(facts.0, &mut other);
                    } else {
                        facts.0.release_storage(1).unwrap();
                    }
                    if unwind {
                        panic!("injected after custody loss");
                    }
                    Err(reject(DONE))
                })
            }));
            // Cleanup accounting refusal takes precedence even over a caught panic.
            assert!(matches!(
                result,
                Ok(Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(Resource::Accounting)
                )))
            ));
            if foreign {
                assert!(budget.work_ledger_identity_v1() == other_account);
                assert!(other.work_ledger_identity_v1() == account);
                std::mem::swap(&mut budget, &mut other);
            }
            assert!(budget.work_ledger_identity_v1() == account);
            assert!(other.work_ledger_identity_v1() == other_account);
            assert_eq!(
                (budget.storage(), budget.work(), budget.failed_storage()),
                (if foreign { 17 + header } else { 16 + header }, 1, None)
            );
            assert_eq!(
                (other.storage(), other.work(), other.failed_storage()),
                (17 + header, 0, None)
            );
        }
    }
}

#[test]
fn multi_entry_scope_keeps_trait_default_custody_optional_without_induction() {
    let mut facts = NoCustodyFacts(0);
    let mut called = false;
    let result = with_scope(&mut facts, |_, _| {
        called = true;
        Err(ProductionRankedProjectionErrorV1::Unsupported(DONE))
    });
    assert!(called);
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Unsupported(_))
    ));
    assert_eq!(facts.0, 0);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_scope(&mut facts, |_, _| panic!("historical callback unwind"))
    }));
    assert!(panic.is_err());
    assert_eq!(facts.0, 0);
}

#[test]
fn multi_entry_scope_requires_entry_custody_on_use_and_cannot_retry_with_a_new_floor() {
    let mut facts = NoCustodyFacts(0);
    let result = with_scope(&mut facts, |scope, facts| {
        let result = make(&fixture(), scope, facts);
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Incomplete(_))
        ));
        assert_eq!(facts.0, 0);
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(17).unwrap();
        for _ in 0..2 {
            let result = make(&fixture(), scope, &mut Facts(&mut budget));
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(Resource::Accounting)
                ))
            ));
            assert_eq!((budget.storage(), budget.work()), (17, 0));
        }
        Err(ProductionRankedProjectionErrorV1::Unsupported(DONE))
    });
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Unsupported(_))
    ));
    assert_eq!(facts.0, 0);
}

#[test]
fn multi_entry_scope_refuses_foreign_account_before_first_induction_use() {
    let mut work = Work::new(usize::MAX);
    let mut other_work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let mut other = Budget::new(&mut other_work, usize::MAX);
    budget.reserve_storage(17).unwrap();
    other.reserve_storage(17).unwrap();
    let account = budget.work_ledger_identity_v1();
    for moved in [false, true] {
        let result = with_scope(&mut Facts(&mut budget), |scope, facts| {
            std::mem::swap(facts.0, &mut other);
            // Same address/foreign ledger, then same ledger/foreign address.
            let result = if moved {
                make(&fixture(), scope, &mut Facts(&mut other))
            } else {
                make(&fixture(), scope, facts)
            };
            std::mem::swap(facts.0, &mut other);
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(Resource::Accounting)
                ))
            ));
            assert_eq!((other.storage(), other.work()), (17, 0));
            Err(ProductionRankedProjectionErrorV1::Unsupported(DONE))
        });
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Unsupported(_))
        ));
        assert_eq!((budget.storage(), budget.work()), (17, 0));
        assert!(budget.work_ledger_identity_v1() == account);
    }
}

fn extent_then_induction_replay(
    function: &SemanticFunctionDeclV1,
    scope: &mut Scope,
    facts: &mut Facts<'_, '_>,
) -> Result<()> {
    let floor = facts.0.storage();
    let count = function.locals().len();
    let proof = slice_extent_projection_v1::with_scope(count, facts, |extent| {
        let scratch = slice_extent_projection_v1::Scratch {
            origins: vec![None; count],
            arguments: vec![None; count],
            definitions: vec![0; count],
            escaped: vec![false; count],
        };
        extent.retain(&scratch)?;
        make(function, scope, extent.facts())
    })?;
    // The actual extent scope is dead; only the induction's own rows/header
    // remain. Its outer floor must never include the released inner scratch.
    assert_eq!(facts.0.storage(), floor + scope.retained);
    replay(
        &proof,
        function,
        &graph(function),
        3,
        4,
        SemanticLocalIdV1::from_index(1),
        &mut Context { scope, facts },
    )
}

#[test]
fn multi_entry_outer_account_survives_extent_scratch_with_exact_and_short_budgets() {
    let run = |work, storage| run_component(work, storage, fixture(), extent_then_induction_replay);
    let (result, work, peak, a, b) = run(usize::MAX, usize::MAX);
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
    ));
    assert_eq!((a, b), (None, None));
    let (result, w, p, a, b) = run(work, peak);
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
    ));
    assert_eq!((w, p, a, b), (work, peak, None, None));
    let (result, _, _, a, b) = run(work - 1, peak);
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
            CanonicalAssertionErrorV1::Resource(Resource::Work(_))
        ))
    ));
    assert_eq!((a, b), (None, Some(work)));
    let (result, _, _, a, b) = run(work, peak - 1);
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
            CanonicalAssertionErrorV1::Resource(Resource::Storage(_))
        ))
    ));
    assert_eq!((a, b), (Some(peak), None));
}

#[test]
fn multi_entry_outer_account_refuses_foreign_after_extent_and_initial_floor_loss() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(17).unwrap();
    for foreign in [true, false] {
        let before = budget.work();
        let result = with_scope(&mut Facts(&mut budget), |scope, facts| {
            if foreign {
                extent_then_induction_replay(&fixture(), scope, facts)?;
                let mut other_work = Work::new(usize::MAX);
                let mut other = Budget::new(&mut other_work, usize::MAX);
                let foreign_floor = facts.0.storage();
                other.reserve_storage(foreign_floor).unwrap();
                let result = make(&fixture(), scope, &mut Facts(&mut other));
                assert_eq!((other.storage(), other.work()), (foreign_floor, 0));
                result?;
            } else {
                facts.0.release_storage(1).unwrap();
                make(&fixture(), scope, facts)?;
            }
            Err(reject(DONE))
        });
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(Resource::Accounting)
            ))
        ));
        assert_eq!(budget.storage(), if foreign { 17 } else { 16 });
        if foreign {
            assert!(budget.work() > before);
        } else {
            assert_eq!(budget.work(), before);
        }
    }
}

#[test]
fn multi_entry_outer_account_unwind_after_extent_release_keeps_original_floor() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(17).unwrap();
    budget.charge_work(13).unwrap();
    let account = budget.work_ledger_identity_v1();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_scope(&mut Facts(&mut budget), |scope, facts| {
            extent_then_induction_replay(&fixture(), scope, facts)?;
            panic!("injected after shorter-lived extent scope");
        })
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), 17);
    assert!(budget.work_ledger_identity_v1() == account);
    assert!(budget.work() > 13);
}
