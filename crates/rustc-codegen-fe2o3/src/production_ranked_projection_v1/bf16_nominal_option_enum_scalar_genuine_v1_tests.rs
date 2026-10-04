//! Genuine same-source Option/enum/scalar observation, separate from historical modes.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_source_algorithms_v1::assertion_definition_inventory_with_resources_v1;
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
    scalar_locals: usize,
    scalar_blocks: usize,
    scalar_definitions: usize,
    scalar_assignments: usize,
    scalar_invoked: bool,
}
const CALLBACK_ERROR: &str = "Option/enum/scalar genuine callback control";

fn comparison_frame() -> usize {
    size_of::<(
        AssertionDefinitionInventoryV1,
        BResult<AssertionDefinitionInventoryV1>,
        &AssertionDefinitionInventoryV1,
        usize,
        usize,
        (usize, usize),
        BResult<(usize, usize)>,
        Ledger,
        Option<Ledger>,
        Vec<SemanticOptionProducerV1>,
        SemanticOptionDominanceV1,
        SemanticEnumPayloadDominanceV1,
        BResult<Vec<SemanticOptionProducerV1>>,
        BResult<SemanticOptionDominanceV1>,
        BResult<SemanticEnumPayloadDominanceV1>,
        SemanticEnumPayloadMeteredErrorV1<Backend>,
        BeforeProvenanceV1<'static>,
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
fn compare(view: BeforeProvenanceV1<'_>, resources: &mut Prep<'_, '_>) -> BResult<Observation> {
    resources.reserve_storage(comparison_frame())?;
    if resources.original_ledger_v1() != Some(view.earlier.options.ledger)
        || resources.has_denial()
        || !view.enum_api_invoked()
        || !view.scalar_api_invoked()
    {
        return Err(accounting());
    }
    // These unchanged original APIs construct expectations from the actual
    // source, never from the retained candidate arrays or phase flags.
    let expected_producers = fe2o3_mir_model::semantic_option_producers_with_meter_v1(
        view.function(),
        view.earlier.options.callables,
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
        view.earlier.options.types,
        &mut ModelMeter(resources),
    )
    .map_err(model_error)?;
    let expected_scalar =
        assertion_definition_inventory_with_resources_v1(view.function(), resources)?;
    let (scalar_definitions, scalar_assignments) =
        compare_scalar(&expected_scalar, view.scalar_inventory(), resources)?;
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
        || expected_scalar.counts.is_empty()
        || expected_scalar.blocks.is_empty()
        || scalar_definitions == 0
        || scalar_assignments == 0
    {
        return Err(Backend::Incomplete(
            "Option/enum/scalar genuine original/candidate DATA differs",
        ));
    }
    let observed = Observation {
        option_producers: expected_producers.len(),
        option_work: expected_options.work_units(),
        enum_work: expected_enum.work_units(),
        enum_invoked: view.enum_api_invoked(),
        scalar_locals: expected_scalar.counts.len(),
        scalar_blocks: expected_scalar.blocks.len(),
        scalar_definitions,
        scalar_assignments,
        scalar_invoked: view.scalar_api_invoked(),
    };
    // Enum availability may legitimately be empty. Successful actual retained
    // invocation/completion plus complete original DATA equality is required;
    // a prefilled empty result or skipped analyzer cannot satisfy this entry.
    drop(expected_scalar);
    drop(expected_enum);
    drop(expected_options);
    drop(expected_producers);
    Ok(observed)
}

fn scalar_comparison_frame() -> usize {
    size_of::<(
        &AssertionDefinitionInventoryV1,
        &AssertionDefinitionInventoryV1,
        &mut Prep<'static, 'static>,
        usize,
        usize,
        usize,
        usize,
        usize,
        [&AssertionDefinitionInventoryV1; 2],
        std::array::IntoIter<&'static AssertionDefinitionInventoryV1, 2>,
        &AssertionDefinitionInventoryV1,
        &[Vec<usize>],
        std::slice::Iter<'static, Vec<usize>>,
        &Vec<usize>,
        std::slice::Iter<'static, u8>,
        &u8,
        std::slice::Iter<'static, Option<ScalarAssignmentSiteV1>>,
        &Option<ScalarAssignmentSiteV1>,
        &Vec<u8>,
        &Vec<u8>,
        &[u8],
        &[u8],
        &Vec<Vec<usize>>,
        &Vec<Vec<usize>>,
        &[Vec<usize>],
        &[Vec<usize>],
        &Vec<Option<ScalarAssignmentSiteV1>>,
        &Vec<Option<ScalarAssignmentSiteV1>>,
        &[Option<ScalarAssignmentSiteV1>],
        &[Option<ScalarAssignmentSiteV1>],
        &Vec<bool>,
        &Vec<bool>,
        &[bool],
        &[bool],
        BResult<()>,
        BResult<usize>,
        Backend,
        Option<usize>,
        BResult<(usize, usize)>,
        (usize, usize),
        bool,
    )>()
}
fn compare_scalar(
    expected: &AssertionDefinitionInventoryV1,
    actual: &AssertionDefinitionInventoryV1,
    resources: &mut Prep<'_, '_>,
) -> BResult<(usize, usize)> {
    resources.reserve_storage(scalar_comparison_frame())?;
    resources.work(64)?;
    let mut work = 0usize;
    for data in [expected, actual] {
        // Prepay length and block-header scans before traversing either table.
        resources.work(data.blocks.len())?;
        work = work
            .checked_add(data.counts.len())
            .and_then(|n| n.checked_add(data.assignments.len()))
            .and_then(|n| n.checked_add(data.address_escaped.len()))
            .and_then(|n| n.checked_add(data.blocks.len()))
            .ok_or_else(arithmetic)?;
        for block in &data.blocks {
            work = work.checked_add(block.len()).ok_or_else(arithmetic)?;
        }
    }
    resources.work(work.checked_mul(4).ok_or_else(arithmetic)?)?;
    if expected.counts != actual.counts
        || expected.blocks != actual.blocks
        || expected.assignments != actual.assignments
        || expected.address_escaped != actual.address_escaped
    {
        return Err(Backend::Incomplete(
            "Option/enum/scalar original scalar DATA differs",
        ));
    }
    let mut definitions = 0usize;
    for count in &expected.counts {
        definitions = definitions
            .checked_add(usize::from(*count))
            .ok_or_else(arithmetic)?;
    }
    let mut assignments = 0usize;
    for site in &expected.assignments {
        if site.is_some() {
            assignments = assignments.checked_add(1).ok_or_else(arithmetic)?;
        }
    }
    Ok((definitions, assignments))
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
pub(crate) fn observe_option_enum_scalar_before_provenance_for_test_v1(
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
                let result = with_nominal_option_enum_scalar_before_provenance_v1(
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
                    _ => return Err(QueryError::Unavailable("Option/enum/scalar genuine callback mapping differs")),
                }
                if budget.storage() != floor || budget.failed_work().is_some()
                    || budget.failed_storage().is_some()
                {
                    return Err(Resource::Accounting.into());
                }
                let observed = observed.ok_or(QueryError::Unavailable("Option/enum/scalar genuine callback never compared"))?;
                if !observed.enum_invoked || observed.enum_work == 0
                    || observed.option_producers == 0 || observed.option_work == 0
                    || !observed.scalar_invoked || observed.scalar_locals == 0 || observed.scalar_blocks == 0
                    || observed.scalar_definitions == 0 || observed.scalar_assignments == 0
                {
                    return Err(QueryError::Unavailable("Option/enum/scalar genuine analysis evidence absent"));
                }
                eprintln!("fe2o3-option-enum-scalar-before-provenance-v1 mode={mode:?} option_producers={} option_work={} enum_work={} enum_api_invoked=true enum_complete=true scalar_locals={} scalar_blocks={} scalar_definitions={} scalar_assignments={} scalar_api_invoked=true scalar_complete=true exact_original_data=true custody=true before_provenance=true f2=false ordinary_route=false",
                    observed.option_producers, observed.option_work, observed.enum_work, observed.scalar_locals,
                    observed.scalar_blocks, observed.scalar_definitions, observed.scalar_assignments);
            }
            let mut entered = false;
            let floor = budget.storage();
            let refusal = with_nominal_option_enum_scalar_before_provenance_v1(
                owner, inventory, source.root(), source.helper(), source.call_block(),
                source.source_call(), budget, |_, _| { entered = true; Ok(()) },
            );
            if !matches!(refusal, Err(QueryError::Unavailable(_))) || entered || budget.storage() != floor {
                return Err(QueryError::Unavailable("Option/enum/scalar source-substitution control differs"));
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
