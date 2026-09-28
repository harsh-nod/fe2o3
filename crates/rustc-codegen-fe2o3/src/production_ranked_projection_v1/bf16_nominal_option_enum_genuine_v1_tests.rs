//! Genuine same-source Option/enum observation, separate from historical modes.
use super::*;
use fe2o3_lower_mir_kernel::CheckedBf16CallInstanceV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Compare,
    CallbackError,
    CallbackPanic,
}
#[derive(Clone, Copy, Debug)]
struct Observation {
    option_producers: usize,
    option_work: usize,
    enum_work: usize,
    enum_invoked: bool,
}
const CALLBACK_ERROR: &str = "Option/enum genuine callback control";

fn comparison_frame() -> usize {
    size_of::<(
        Vec<SemanticOptionProducerV1>,
        SemanticOptionDominanceV1,
        SemanticEnumPayloadDominanceV1,
        BResult<Vec<SemanticOptionProducerV1>>,
        BResult<SemanticOptionDominanceV1>,
        BResult<SemanticEnumPayloadDominanceV1>,
        SemanticEnumPayloadMeteredErrorV1<Backend>,
        BeforeScalarV1<'static>,
        &mut Prep<'static, 'static>,
        ModelMeter<'static, 'static, 'static>,
        &SemanticFunctionDeclV1,
        &[SemanticCallableDeclV1],
        &[SemanticTypeDeclV1],
        &[SemanticOptionProducerV1],
        &SemanticOptionDominanceV1,
        &SemanticEnumPayloadDominanceV1,
        Observation,
        Option<Observation>,
        BResult<Observation>,
        usize,
        usize,
        usize,
        Option<usize>,
        bool,
        Backend,
        &Backend,
    )>()
}
fn compare(view: BeforeScalarV1<'_>, resources: &mut Prep<'_, '_>) -> BResult<Observation> {
    resources.reserve_storage(comparison_frame())?;
    if resources.original_ledger_v1() != Some(view.options.ledger)
        || resources.has_denial()
        || !view.enum_api_invoked()
    {
        return Err(accounting());
    }
    // These unchanged original APIs construct expectations from the actual
    // source, never from the retained candidate arrays or phase flags.
    let expected_producers = fe2o3_mir_model::semantic_option_producers_with_meter_v1(
        view.function(),
        view.options.callables,
        &mut ModelMeter(resources),
    )
    .map_err(model_error)?;
    let expected_options = SemanticOptionDominanceV1::analyze_with_meter_v1(
        view.function(),
        &expected_producers,
        &mut ModelMeter(resources),
    )
    .map_err(model_error)?;
    let expected_enum = SemanticEnumPayloadDominanceV1::analyze_with_meter_v1(
        view.function(),
        view.options.types,
        &mut ModelMeter(resources),
    )
    .map_err(model_error)?;
    let work = expected_options
        .work_units()
        .checked_add(expected_enum.work_units())
        .and_then(|n| n.checked_add(view.option_dominance().work_units()))
        .and_then(|n| n.checked_add(view.enum_dominance().work_units()))
        .and_then(|n| n.checked_mul(4))
        .and_then(|n| {
            view.function()
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
    if expected_producers.as_slice() != view.option_producers()
        || &expected_options != view.option_dominance()
        || &expected_enum != view.enum_dominance()
        || expected_producers.is_empty()
        || expected_options.work_units() == 0
        || expected_enum.work_units() == 0
        || view.enum_dominance().grants_authority()
    {
        return Err(Backend::Incomplete(
            "Option/enum genuine original/candidate DATA differs",
        ));
    }
    let observed = Observation {
        option_producers: expected_producers.len(),
        option_work: expected_options.work_units(),
        enum_work: expected_enum.work_units(),
        enum_invoked: view.enum_api_invoked(),
    };
    // Enum availability may legitimately be empty. Successful actual retained
    // invocation/completion plus complete original DATA equality is required;
    // a prefilled empty result or skipped analyzer cannot satisfy this entry.
    drop(expected_enum);
    drop(expected_options);
    drop(expected_producers);
    Ok(observed)
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
pub(crate) fn observe_option_enum_before_scalar_for_test_v1(
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
                let result = with_nominal_option_enum_before_scalar_v1(
                    owner, inventory, source.root(), source.root(), source.call_block(),
                    source.source_call(), budget, |view, resources| {
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
                    _ => return Err(QueryError::Unavailable("Option/enum genuine callback mapping differs")),
                }
                if budget.storage() != floor || budget.failed_work().is_some()
                    || budget.failed_storage().is_some()
                {
                    return Err(Resource::Accounting.into());
                }
                let observed = observed.ok_or(QueryError::Unavailable("Option/enum genuine callback never compared"))?;
                if !observed.enum_invoked || observed.enum_work == 0
                    || observed.option_producers == 0 || observed.option_work == 0
                {
                    return Err(QueryError::Unavailable("Option/enum genuine analysis evidence absent"));
                }
                eprintln!("fe2o3-option-enum-before-scalar-v1 mode={mode:?} option_producers={} option_work={} enum_work={} enum_api_invoked=true enum_complete=true exact_original_data=true custody=true before_scalar=true f2=false ordinary_route=false",
                    observed.option_producers, observed.option_work, observed.enum_work);
            }
            let mut entered = false;
            let floor = budget.storage();
            let refusal = with_nominal_option_enum_before_scalar_v1(
                owner, inventory, source.root(), source.helper(), source.call_block(),
                source.source_call(), budget, |_, _| { entered = true; Ok(()) },
            );
            if !matches!(refusal, Err(QueryError::Unavailable(_))) || entered || budget.storage() != floor {
                return Err(QueryError::Unavailable("Option/enum source-substitution control differs"));
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
