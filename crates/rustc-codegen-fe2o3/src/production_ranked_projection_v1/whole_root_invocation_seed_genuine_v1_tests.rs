//! Separate genuine seed observation. Old prewriter/empty/graph markers remain independent.
use super::*;
use initial_graph::genuine as graph;
use initial_graph::genuine_seed as seed;
const CALLBACK_ERROR: &str = "retained invocation seed callback control";
fn callback_matches(mode: Mode, result: &Result<()>, entered: bool) -> bool {
    entered
        && match mode {
            Mode::Compare => result.is_ok(),
            Mode::CallbackError => *result == Err(QueryError::Unavailable(CALLBACK_ERROR)),
            Mode::CallbackPanic => *result == Err(QueryError::CallbackPanicked),
        }
}
fn run(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    actual: &ActualRetainedRankedInputsV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
    mode: Mode,
) -> Result<seed::SeedObservation> {
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, |budget| {
        let before = Custody::take(budget)?;
        let mut owned = 0usize;
        Prep::new(budget, &mut owned)
            .reserve_storage(frame()?)
            .map_err(query_error)?;
        // Four distinct outer owners, all outside actual checked/canonical
        // callback and postflight; no old terminal owner is resumed.
        let mut pending = PendingWholeRootBeforeArgumentWritersV1::new();
        let mut expected = Originals::new();
        let mut graph_oracle = graph::GraphOracle::new();
        let mut oracle = seed::SeedOracle::new();
        let mut callback_observation = None;
        let mut control_entered = false;
        let result = with_checked_nominal_facts_observation_v1(
            owner,
            inventory,
            source.root(),
            source.root(),
            source.call_block(),
            source.source_call(),
            budget,
            &mut owned,
            |checked, facts, owned| {
                if !checked.belongs_to(inventory)
                    || !std::ptr::eq(checked.source_call(), source.source_call())
                    || !actual.belongs_to(owner)
                {
                    return Err(QueryError::Unavailable(
                        "genuine seed checked source differs",
                    ));
                }
                pending.prepare_into(owner, checked, actual, facts, owned)?;
                // Existing full before-writer comparisons remain independently
                // evaluated; no old marker is emitted or reclassified here.
                if let Err(error) = super::compare(
                    &pending,
                    &mut expected,
                    owner,
                    checked,
                    actual,
                    facts,
                    owned,
                ) {
                    let mapped = saved_query_error(&error);
                    pending.failure = Some(error);
                    return Err(mapped);
                }
                let writer = initial_strided_reads::genuine_empty::observe_before_reentry_in_scope(
                    &mut pending,
                    &mut expected.empty_reads,
                    &mut expected.indices,
                    owner,
                    source.root(),
                    facts,
                    owned,
                )
                .map_err(|error| {
                    let mapped = saved_query_error(&error);
                    pending.failure = Some(error);
                    mapped
                })?;
                expected
                    .observation
                    .as_mut()
                    .ok_or(QueryError::Unavailable(
                        "genuine seed before-writer observation absent",
                    ))?
                    .writer = Some(writer);
                graph::observe_before_reentry_in_scope(
                    &mut pending,
                    &mut graph_oracle,
                    owner,
                    source.root(),
                    facts,
                    owned,
                )
                .map_err(|error| {
                    let mapped = saved_query_error(&error);
                    pending.failure = Some(error);
                    mapped
                })?;
                let observed = seed::observe_in_scope(
                    &mut pending,
                    &mut oracle,
                    &graph_oracle,
                    owner,
                    source.root(),
                    facts,
                    owned,
                )
                .map_err(|error| {
                    let mapped = saved_query_error(&error);
                    pending.failure = Some(error);
                    mapped
                })?;
                callback_observation = Some(observed);
                control_entered = true;
                match mode {
                    Mode::Compare => Ok(()),
                    Mode::CallbackError => Err(QueryError::Unavailable(CALLBACK_ERROR)),
                    Mode::CallbackPanic => std::panic::panic_any(()),
                }
            },
        );
        let expected_control = callback_matches(mode, &result, control_entered);
        // This second comparison was paid inside the successful callback and
        // reads the still-live source/destination/writer payload after postflight.
        let postflight = if expected_control {
            seed::postflight(
                &pending,
                &mut oracle,
                &graph_oracle,
                owner,
                source.root(),
                &Prep::new(budget, &mut owned),
            )
            .map_err(query_error)
        } else {
            Err(QueryError::Unavailable(
                "genuine seed controlled callback did not complete",
            ))
        };
        let custody = before.check(budget, owned);
        let retained = owned;
        let denied = budget.failed_work().is_some() || budget.failed_storage().is_some();
        drop(oracle);
        drop(graph_oracle);
        drop(expected);
        drop(pending);
        custody?;
        budget.release_storage(retained)?;
        // Never turn an unexpected donor/bridge refusal or panic into a
        // successful observation, nor relabel it as an unavailable profile.
        if !expected_control {
            return match result {
                Err(error) => Err(error),
                Ok(()) => Err(QueryError::Unavailable("genuine seed observation absent")),
            };
        }
        if denied {
            return Err(Resource::Accounting.into());
        }
        let observed = postflight?;
        if callback_observation != Some(observed) {
            return Err(Resource::Accounting.into());
        }
        Ok(observed)
    })
}
fn frame() -> Result<usize> {
    const ROWS: usize = 7;
    let rows = [
        super::frame()?,
        graph::frame().map_err(query_error)?,
        seed::frame().map_err(query_error)?,
        size_of::<(
            graph::GraphOracle,
            &mut graph::GraphOracle,
            &graph::GraphOracle,
            seed::SeedOracle,
            seed::SeedObservation,
            Option<seed::SeedObservation>,
            &mut seed::SeedOracle,
            &seed::SeedOracle,
            Mode,
            [Mode; 3],
            std::array::IntoIter<Mode, 3>,
            Custody,
            usize,
            usize,
            bool,
            bool,
        )>(),
        size_of::<(
            &ProductionPreRankedKirOwnerV1,
            &CheckedBf16CallInstanceV1<'static>,
            &ActualRetainedRankedInputsV1<'static>,
            &CanonicalKirInventoryV1<'static>,
            &mut Budget<'static>,
            &mut usize,
            &mut PendingWholeRootBeforeArgumentWritersV1<'static>,
            &mut Originals<'static>,
            &CheckedBf16NominalCallV1<'static>,
            &mut CanonicalSourceAssertionFactsV1<'static, 'static, 'static, 'static, 'static>,
            Prep<'static, 'static>,
            &Prep<'static, 'static>,
        )>(),
        size_of::<(
            Result<()>,
            Result<seed::SeedObservation>,
            BResult<seed::SeedObservation>,
            Backend,
            QueryError,
            Resource,
            Option<Backend>,
            PanicPayload,
            std::result::Result<Result<()>, PanicPayload>,
            bool,
            Option<usize>,
        )>(),
        size_of::<(
            [usize; ROWS],
            std::array::IntoIter<usize, ROWS>,
            usize,
            usize,
            Option<usize>,
            Result<usize>,
        )>(),
    ];
    rows.into_iter().try_fold(0usize, |n, row| {
        n.checked_add(row)
            .ok_or(QueryError::Resource(Resource::Arithmetic))
    })
}
pub(in crate::production_ranked_projection_v1) fn observe(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    actual: &ActualRetainedRankedInputsV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, |budget| {
        let before = Custody::take(budget)?;
        let bytes = frame()?;
        budget.reserve_storage(bytes)?;
        let outcome = catch_unwind(AssertUnwindSafe(|| -> Result<()> {
            for mode in [Mode::Compare, Mode::CallbackError, Mode::CallbackPanic] {
                let floor = budget.storage();
                let observed = run(owner, source, actual, inventory, budget, mode)?;
                if budget.storage() != floor { return Err(Resource::Accounting.into()); }
                eprintln!("fe2o3-whole-root-invocation-seeds-v1 mode={mode:?} locals={} blocks={} seeds={} prefix_operations={} next_value={} next_argument={} seeds_invoked=true original_metered_donor=true same_source_ledger_counter=true retained_postflight=true postflight_rows_compared=true original_namespace_preserved=true graph_rows_preserved=true before_writer_comparison=true empty_read_predecessor=true graph_predecessor=true nonempty_read_coverage=false recovery_started=false propagation_started=false old_completions_refused=true foreign_counter_refused=true reentry_terminal=true private_component=true ordinary_route=false",
                    observed.locals, observed.blocks, observed.seeds, observed.prefix_operations,
                    observed.next_value, observed.next_argument);
            }
            Ok(())
        }));
        let result = match outcome {
            Ok(result) => result,
            Err(payload) => { drop(payload); Err(QueryError::CallbackPanicked) }
        };
        before.check(budget, bytes)?;
        budget.release_storage(bytes)?;
        result
    })
}
#[test]
fn genuine_seed_callback_control_requires_completed_observation() {
    for mode in [Mode::Compare, Mode::CallbackError, Mode::CallbackPanic] {
        let result = match mode {
            Mode::Compare => Ok(()),
            Mode::CallbackError => Err(QueryError::Unavailable(CALLBACK_ERROR)),
            Mode::CallbackPanic => Err(QueryError::CallbackPanicked),
        };
        assert!(callback_matches(mode, &result, true));
        assert!(!callback_matches(mode, &result, false));
    }
}
#[test]
fn genuine_seed_callback_control_preserves_unexpected_refusal() {
    let refusal = Err(QueryError::Resource(Resource::Accounting));
    for mode in [Mode::Compare, Mode::CallbackError, Mode::CallbackPanic] {
        assert!(!callback_matches(mode, &refusal, true));
    }
    assert!(!callback_matches(
        Mode::CallbackError,
        &Err(QueryError::Unavailable("donor refused")),
        true
    ));
    assert!(!callback_matches(Mode::CallbackPanic, &Ok(()), true));
}
