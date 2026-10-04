//! Separate real-source Option-first observations; no historical mode is replaced.
use super::*;
use fe2o3_lower_mir_kernel::CheckedBf16CallInstanceV1;
use fe2o3_mir_model::semantic_option_producers_with_meter_v1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Compare,
    CallbackError,
    CallbackPanic,
}
#[derive(Clone, Copy, Debug)]
struct Observation {
    producers: usize,
    allowed_pairs: usize,
    dominance_work: usize,
}
const CALLBACK_ERROR: &str = "Option-first genuine callback control";

fn comparison_frame() -> usize {
    size_of::<(
        Vec<SemanticOptionProducerV1>,
        SemanticOptionDominanceV1,
        BResult<Vec<SemanticOptionProducerV1>>,
        BResult<SemanticOptionDominanceV1>,
        SemanticEnumPayloadMeteredErrorV1<Backend>,
        Observation,
        Option<Observation>,
        BeforeEnumV1<'static>,
        &mut Prep<'static, 'static>,
        ModelMeter<'static, 'static, 'static>,
        usize,
        usize,
        usize,
        usize,
        Option<usize>,
        bool,
        Backend,
        std::slice::Iter<'static, SemanticOptionProducerV1>,
        &SemanticOptionProducerV1,
        std::ops::Range<usize>,
        Option<fe2o3_mir_model::SemanticOptionAvailabilityV1>,
        fe2o3_mir_model::SemanticOptionAvailabilityV1,
        SemanticBlockIdV1,
    )>()
}
fn compare(view: BeforeEnumV1<'_>, resources: &mut Prep<'_, '_>) -> BResult<Observation> {
    resources.reserve_storage(comparison_frame())?;
    if resources.original_ledger_v1() != Some(view.ledger) || resources.has_denial() {
        return Err(accounting());
    }
    // Independent unchanged return APIs on the SAME genuine function/callables
    // and SAME original Prep counter. No candidate output constructs expected DATA.
    let expected_producers = semantic_option_producers_with_meter_v1(
        view.function,
        view.callables,
        &mut ModelMeter(resources),
    )
    .map_err(model_error)?;
    let expected_dominance = SemanticOptionDominanceV1::analyze_with_meter_v1(
        view.function,
        &expected_producers,
        &mut ModelMeter(resources),
    )
    .map_err(model_error)?;
    // Equality visits every producer and all four complete model result arrays.
    // Analyzer work includes their initialization; pay both source-sized scans.
    let work = expected_dominance
        .work_units()
        .checked_add(view.dominance.work_units())
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| {
            view.function
                .locals()
                .len()
                .checked_mul(8)
                .and_then(|x| n.checked_add(x))
        })
        .and_then(|n| {
            expected_producers
                .len()
                .checked_mul(8)
                .and_then(|x| n.checked_add(x))
        })
        .and_then(|n| n.checked_add(64))
        .ok_or_else(arithmetic)?;
    resources.work(work)?;
    if expected_producers.as_slice() != view.producers
        || &expected_dominance != view.dominance
        || expected_producers.is_empty()
        || view.dominance.grants_authority()
    {
        return Err(Backend::Incomplete(
            "Option-first genuine original/candidate DATA differs or is empty",
        ));
    }
    let pairs = expected_producers
        .len()
        .checked_mul(view.function.blocks().len())
        .and_then(|n| n.checked_mul(2))
        .ok_or_else(arithmetic)?;
    resources.work(pairs)?;
    let mut allowed_pairs = 0usize;
    for producer in &expected_producers {
        let availability = view
            .dominance
            .availability(producer.option_local())
            .ok_or_else(accounting)?;
        for block in 0..view.function.blocks().len() {
            if view
                .dominance
                .allows(availability, SemanticBlockIdV1::from_index(block as u32))
            {
                allowed_pairs = allowed_pairs.checked_add(1).ok_or_else(arithmetic)?;
            }
        }
    }
    if allowed_pairs == 0 {
        return Err(Backend::Incomplete(
            "Option-first genuine source has no authenticated Some-region pair",
        ));
    }
    let observation = Observation {
        producers: expected_producers.len(),
        allowed_pairs,
        dominance_work: view.dominance.work_units(),
    };
    // Original comparison scratch drops here. Its accepted credits remain with
    // the shared owner until postflight and drop-before-refund. Candidate model
    // owners, unlike this temporary oracle scratch, remain physically attached.
    drop(expected_dominance);
    drop(expected_producers);
    Ok(observation)
}

fn observation_frame() -> usize {
    size_of::<(
        [Mode; 3],
        std::array::IntoIter<Mode, 3>,
        Mode,
        Observation,
        Option<Observation>,
        Option<Observation>,
        Result<()>,
        std::result::Result<Result<()>, PanicPayload>,
        PanicPayload,
        Custody,
        Custody,
        usize,
        usize,
        usize,
        bool,
        QueryError,
        &ProductionPreRankedKirOwnerV1,
        &CheckedBf16CallInstanceV1<'static>,
        &CanonicalKirInventoryV1<'static>,
        &mut Budget<'static>,
        (&mut Option<Observation>, Mode),
        (&mut bool,),
        (
            &mut Budget<'static>,
            &ProductionPreRankedKirOwnerV1,
            &CheckedBf16CallInstanceV1<'static>,
            &CanonicalKirInventoryV1<'static>,
        ),
    )>()
}

pub(crate) fn observe_option_first_before_enum_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, |budget| {
        let before = Custody::take(budget)?;
        let bytes = observation_frame();
        budget.reserve_storage(bytes)?;
        let outcome = catch_unwind(AssertUnwindSafe(|| -> Result<()> {
            budget.charge_work(128)?;
            for mode in [Mode::Compare, Mode::CallbackError, Mode::CallbackPanic] {
                let floor = budget.storage();
                let mut observed = None;
                let result = with_nominal_option_first_before_enum_v1(
                    owner, inventory, source.root(), source.root(), source.call_block(),
                    source.source_call(), budget,
                    |view, resources| {
                        observed = Some(compare(view, resources)?);
                        match mode {
                            Mode::Compare => Ok(()),
                            Mode::CallbackError => Err(Backend::Incomplete(CALLBACK_ERROR)),
                            Mode::CallbackPanic => std::panic::panic_any(()),
                        }
                    },
                );
                match mode {
                    Mode::Compare => result?,
                    Mode::CallbackError if result == Err(QueryError::Unavailable(CALLBACK_ERROR)) => {},
                    Mode::CallbackPanic if result == Err(QueryError::CallbackPanicked) => {},
                    _ => return Err(QueryError::Unavailable("Option-first genuine callback mapping differs")),
                }
                if budget.storage() != floor || budget.failed_work().is_some()
                    || budget.failed_storage().is_some()
                {
                    return Err(Resource::Accounting.into());
                }
                let observed = observed.ok_or(QueryError::Unavailable("Option-first genuine callback never compared"))?;
                if observed.producers == 0 || observed.allowed_pairs == 0 || observed.dominance_work == 0 {
                    return Err(QueryError::Unavailable("Option-first genuine positive evidence absent"));
                }
                eprintln!("fe2o3-option-first-before-enum-v1 mode={mode:?} producers={} allowed_pairs={} dominance_work={} exact_original_data=true custody=true before_enum=true f2=false ordinary_route=false",
                    observed.producers, observed.allowed_pairs, observed.dominance_work);
            }
            // A real invalid caller selection must fail before this new callback.
            let mut entered = false;
            let floor = budget.storage();
            let refusal = with_nominal_option_first_before_enum_v1(
                owner, inventory, source.root(), source.helper(), source.call_block(),
                source.source_call(), budget, |_, _| { entered = true; Ok(()) },
            );
            if !matches!(refusal, Err(QueryError::Unavailable(_))) || entered || budget.storage() != floor {
                return Err(QueryError::Unavailable("Option-first source-substitution control differs"));
            }
            Ok(())
        }));
        let result = match outcome {
            Ok(result) => result,
            Err(payload) => { drop(payload); Err(QueryError::CallbackPanicked) },
        };
        before.check(budget, bytes)?;
        budget.release_storage(bytes)?;
        result
    })
}
