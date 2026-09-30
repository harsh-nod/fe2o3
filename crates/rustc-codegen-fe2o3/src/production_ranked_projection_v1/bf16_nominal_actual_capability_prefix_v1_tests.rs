//! Closed genuine ActualCapabilityPrefix observation; NOT full-root BeforeArgumentWriters.
//! Whole-root induction/projected-view/slice predecessors and ordinary routing remain absent.
use super::*;
use crate::production_pipeline::ActualRetainedRankedInputsV1;
use crate::production_ranked_projection_v1::bf16_nominal_dense_v1::{
    CompletedNominalFifoDriverV1, NominalCapabilityConsumerV1, NominalCapabilityInputsV1,
    NominalCapabilityLedgerV1, NominalCapabilityPassV1, RetainedNominalCapabilityDriverV1,
};
use crate::production_ranked_projection_v1::bf16_nominal_source_algorithms_v1::RetainedConstantLocalsV1;
use crate::production_ranked_projection_v1::canonical_assertion_facts_v1::{
    ActualSelectedInputsV1, select_actual_capability_prefix_inputs_v1,
    with_checked_nominal_facts_observation_v1, with_nominal_capability_consumer_v1,
    with_nominal_source_preparation_v1,
};
use crate::production_ranked_projection_v1::root_entry_prefix_preparation_v1::{
    RootEntryPrefixV1, prepare_root_entry_prefix_paid_v1,
};

#[path = "bf16_nominal_actual_capability_oracle_v1_tests.rs"]
mod original;
use original::OriginalCapabilityOracleV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrefixMode {
    Compare,
    CallbackError,
    CallbackPanic,
    QueryError,
    QueryPanic,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrefixPhase {
    Fresh,
    Terminal,
    Complete,
}
#[derive(Clone, Copy, Debug)]
struct PrefixObservation {
    constants: usize,
    nonempty_constants: usize,
    blocks: usize,
    layouts: usize,
    bound_reads: usize,
    first_visits: usize,
    repeated_visits: usize,
    original_work: usize,
    query_visits: [usize; 3],
}
struct PendingActualCapabilityPrefixV1 {
    phase: PrefixPhase,
    constants: RetainedConstantLocalsV1,
    prefix: RootEntryPrefixV1,
    earlier: PendingBeforeCapabilitiesV1,
    driver: RetainedNominalCapabilityDriverV1,
    oracle: OriginalCapabilityOracleV1,
    preparation_observation: Option<Observation>,
    observation: Option<PrefixObservation>,
    visited: [usize; 3],
    failure: Option<Backend>,
}
impl PendingActualCapabilityPrefixV1 {
    fn new() -> Self {
        Self {
            phase: PrefixPhase::Fresh,
            constants: RetainedConstantLocalsV1::new(),
            prefix: RootEntryPrefixV1::empty(),
            earlier: PendingBeforeCapabilitiesV1::new(),
            driver: RetainedNominalCapabilityDriverV1::new(),
            oracle: OriginalCapabilityOracleV1::new(),
            preparation_observation: None,
            observation: None,
            visited: [0; 3],
            failure: None,
        }
    }
}
const PREFIX_CALLBACK_ERROR: &str = "actual capability prefix callback control";
const PREFIX_QUERY_ERROR: &str = "actual capability prefix actual query control";

#[allow(clippy::too_many_arguments)]
fn run_prefix(
    owner: &ProductionPreRankedKirOwnerV1,
    source_call: &CheckedBf16CallInstanceV1<'_>,
    actual_inputs: &ActualRetainedRankedInputsV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
    caller: SemanticFunctionIdV1,
    mode: PrefixMode,
) -> Result<(Option<PrefixObservation>, [usize; 3])> {
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, |budget| {
        let before = Custody::take(budget)?;
        let mut owned = 0usize;
        Prep::new(budget, &mut owned).reserve_storage(prefix_frame()?).map_err(query_error)?;
        let mut pending = PendingActualCapabilityPrefixV1::new();
        let result = with_checked_nominal_facts_observation_v1(
            owner, inventory, source_call.root(), caller, source_call.call_block(),
            source_call.source_call(), budget, &mut owned, |checked, facts, owned| {
                pending.phase = PrefixPhase::Terminal;
                if !std::ptr::eq(checked.emission().owner(), owner)
                    || !checked.belongs_to(inventory)
                    || !std::ptr::eq(checked.source_call(), source_call.source_call())
                    || !actual_inputs.belongs_to(owner)
                {
                    return Err(QueryError::Unavailable("actual capability prefix checked source differs"));
                }
                let semantic = checked.emission().owner().semantic_ssa().source_semantic();
                let function = semantic.functions().get(caller.index() as usize)
                    .ok_or(QueryError::Unavailable("actual capability prefix source function absent"))?;
                if semantic.functions().len() != 2 || semantic.types().len() > 4096
                    || semantic.callables().len() > 4096 || function.blocks().len() > 32
                    || function.locals().len() > 4096
                {
                    return Err(QueryError::Unavailable("actual capability prefix exceeds closed source profile"));
                }
                let source = Source { function, callables: semantic.callables(), types: semantic.types(), ledger: before.ledger };
                let prepared = with_nominal_source_preparation_v1(facts, function, before.ledger, owned, |resources| {
                    // Constants occur BEFORE the actual prefix and Option-first chain.
                    pending.constants.prepare_into(function, resources)?;
                    let selected = select_actual_capability_prefix_inputs_v1(owner, checked, function, actual_inputs, resources)?;
                    prepare_root_entry_prefix_paid_v1(selected.source_root(), selected.references(), &mut pending.prefix, resources)?;
                    pending.earlier.prepare(&source, resources)?;
                    let view = pending.earlier.view(&source, resources)?;
                    pending.preparation_observation = Some(super::compare(view, resources)?);
                    Ok(())
                });
                if let Err(error) = prepared {
                    let mapped = saved_query_error(&error); pending.failure = Some(error); return Err(mapped);
                }
                let borrowed = with_nominal_source_preparation_v1(facts, function, before.ledger, owned, |resources| {
                    Ok((pending.constants.completed_for(function, resources)?, pending.earlier.view(&source, resources)?))
                });
                let (constants, preparation) = match borrowed {
                    Ok(value) => value,
                    Err(error) => { let mapped=saved_query_error(&error); pending.failure=Some(error); return Err(mapped); }
                };
                let inputs = NominalCapabilityInputsV1::from_borrowed_source_v1(
                    function, preparation.enum_dominance(), preparation.allocation_contracts(), constants,
                );
                with_nominal_capability_consumer_v1(facts, |site, consumer| {
                    let mut visit = |pass, authenticated: &crate::production_ranked_projection_v1::bf16_nominal_capabilities_v1::AuthenticatedNominalCallerV1<'_>, query_budget: &mut Budget<'_>| {
                        query_budget.charge_work(64)?;
                        let index = match pass { NominalCapabilityPassV1::Initial=>0, NominalCapabilityPassV1::Repeated=>1, NominalCapabilityPassV1::Final=>2 };
                        pending.visited[index] = pending.visited[index].checked_add(1).ok_or(Resource::Arithmetic)?;
                        let actual = authenticated.candidate().call();
                        if !std::ptr::eq(actual.emission().owner(), owner)
                            || !actual.belongs_to(inventory) || !std::ptr::eq(actual.source_call(), source_call.source_call())
                            || actual.emission().root() != source_call.root()
                            || actual.emission().helper() != source_call.helper()
                            || actual.emission().return_permutation() != source_call.return_permutation()
                            || authenticated.site().caller() != caller
                        {
                            return Err(QueryError::Unavailable("actual capability prefix query association differs"));
                        }
                        match mode {
                            PrefixMode::QueryError => Err(QueryError::Unavailable(PREFIX_QUERY_ERROR)),
                            PrefixMode::QueryPanic => std::panic::panic_any(()),
                            _ => Ok(()),
                        }
                    };
                    pending.driver.prepare_into(site, &inputs, consumer, owned, &mut visit)?;
                    Ok(())
                })?;
                // Original expected DATA uses the actual source and independent
                // old analysis results, never any candidate state or cached answer.
                let expected = with_nominal_source_preparation_v1(facts, function, before.ledger, owned, |resources| {
                    pending.oracle.prepare(function, semantic.callables(), semantic.types(),
                        checked, owner, actual_inputs, &pending.prefix, constants, resources)
                });
                if let Err(error) = expected {
                    let mapped=saved_query_error(&error); pending.failure=Some(error); return Err(mapped);
                }
                pending.observation = Some(with_nominal_capability_consumer_v1(facts, |site, consumer| {
                    let actual = pending.driver.completed_for(site, &inputs, consumer)?;
                    pending.oracle.compare(function, actual, pending.visited, consumer)
                })?);
                pending.phase = PrefixPhase::Complete;
                match mode {
                    PrefixMode::CallbackError => Err(QueryError::Unavailable(PREFIX_CALLBACK_ERROR)),
                    PrefixMode::CallbackPanic => std::panic::panic_any(()),
                    PrefixMode::Compare => Ok(()),
                    _ => Err(QueryError::Unavailable("actual query control failed to stop driver")),
                }
            },
        );
        let custody = before.check(budget, owned);
        let observation = pending.observation;
        let visited = pending.visited;
        let phase = pending.phase;
        let denied = budget.failed_work().is_some() || budget.failed_storage().is_some();
        // Facts report cleanup AND outer original checked-call postflight have
        // completed. Every actual and completed expected payload is still here.
        drop(pending);
        custody?;
        budget.release_storage(owned)?;
        if denied { return Err(Resource::Accounting.into()); }
        match mode {
            PrefixMode::Compare => { result?; if phase != PrefixPhase::Complete { return Err(Resource::Accounting.into()); } },
            PrefixMode::CallbackError if result == Err(QueryError::Unavailable(PREFIX_CALLBACK_ERROR)) && phase == PrefixPhase::Complete => {},
            PrefixMode::CallbackPanic if result == Err(QueryError::CallbackPanicked) && phase == PrefixPhase::Complete => {},
            PrefixMode::QueryError if result == Err(QueryError::Unavailable(PREFIX_QUERY_ERROR)) && phase == PrefixPhase::Terminal && visited[0] > 0 => {},
            PrefixMode::QueryPanic if result == Err(QueryError::CallbackPanicked) && phase == PrefixPhase::Terminal && visited[0] > 0 => {},
            _ => return Err(QueryError::Unavailable("actual capability prefix callback/query mapping differs")),
        }
        Ok((observation, visited))
    })
}

fn prefix_frame() -> Result<usize> {
    let rows = [
        size_of::<PendingActualCapabilityPrefixV1>(),
        size_of::<(PrefixPhase, RetainedConstantLocalsV1, RootEntryPrefixV1,
            PendingBeforeCapabilitiesV1, RetainedNominalCapabilityDriverV1,
            OriginalCapabilityOracleV1, Option<Observation>, Option<PrefixObservation>,
            [usize; 3], Option<Backend>)>(),
        size_of::<(&ProductionPreRankedKirOwnerV1, &CheckedBf16CallInstanceV1<'static>,
            &ActualRetainedRankedInputsV1<'static>, &CanonicalKirInventoryV1<'static>,
            &mut Budget<'static>, SemanticFunctionIdV1, PrefixMode)>(),
        size_of::<(Custody, usize, &mut usize, &mut PendingActualCapabilityPrefixV1,
            &CheckedBf16NominalCallV1<'static>, &AdmittedInertSemanticMirV1,
            &SemanticFunctionDeclV1, Option<&SemanticFunctionDeclV1>, Source<'static>)>(),
        size_of::<(Prep<'static, 'static>, &mut Prep<'static, 'static>, Ledger,
            BeforeCapabilitiesV1<'static>, BResult<BeforeCapabilitiesV1<'static>>,
            ActualSelectedInputsV1<'static>, BResult<ActualSelectedInputsV1<'static>>,
            &ActualSelectedInputsV1<'static>, ProductionSourceLaunchRootV1,
            &[crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1],
            &[Option<u64>], BResult<&[Option<u64>]>)>(),
        size_of::<(NominalCapabilityInputsV1<'static>, &NominalCapabilityInputsV1<'static>,
            CompletedNominalFifoDriverV1<'static>, Result<CompletedNominalFifoDriverV1<'static>>)>(),
        size_of::<(&mut RetainedNominalCapabilityDriverV1,
            &crate::production_ranked_projection_v1::bf16_nominal_capabilities_v1::NominalCallerSiteV1<'static>,
            &mut dyn NominalCapabilityConsumerV1, &mut usize)>(),
        size_of::<(&crate::production_ranked_projection_v1::bf16_nominal_capabilities_v1::AuthenticatedNominalCallerV1<'static>,
            &mut Budget<'static>, NominalCapabilityPassV1, usize,
            &CheckedBf16NominalCallV1<'static>, Result<()>)>(),
        size_of::<(&mut [usize; 3], PrefixMode, &ProductionPreRankedKirOwnerV1,
            &CanonicalKirInventoryV1<'static>, &CheckedBf16CallInstanceV1<'static>, SemanticFunctionIdV1)>(),
        size_of::<(&mut OriginalCapabilityOracleV1, &RootEntryPrefixV1, &SemanticFunctionDeclV1,
            &[SemanticCallableDeclV1], &[SemanticTypeDeclV1], &[Option<u64>], &mut Prep<'static, 'static>)>(),
        size_of::<(Result<()>, Result<()>, BResult<()>, Option<Backend>, Backend, QueryError,
            BResult<(&[Option<u64>], BeforeCapabilitiesV1<'static>)>,
            (&[Option<u64>], BeforeCapabilitiesV1<'static>))>(),
        size_of::<(Option<PrefixObservation>, [usize; 3], PrefixPhase, bool,
            Result<(Option<PrefixObservation>, [usize; 3])>, Result<()>)>(),
        size_of::<(PrefixObservation, Option<PrefixObservation>, Observation, Option<Observation>)>(),
        size_of::<(&mut RetainedConstantLocalsV1, &mut RootEntryPrefixV1,
            &mut PendingBeforeCapabilitiesV1, &mut Option<Observation>,
            &mut OriginalCapabilityOracleV1, &mut Option<Backend>)>(),
        size_of::<([usize; 16], std::array::IntoIter<usize, 16>, usize, usize, Option<usize>, Resource, QueryError)>(),
        allocation_frame::<(), ()>().map_err(query_error)?,
    ];
    rows.into_iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row)
            .ok_or(QueryError::Resource(Resource::Arithmetic))
    })
}
pub(crate) fn observe_actual_capability_prefix_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    actual_inputs: &ActualRetainedRankedInputsV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    super::super::whole_root_before_argument_writers::observe_before_writers_for_test_v1(
        owner,
        source,
        actual_inputs,
        inventory,
        budget,
    )?;
    super::super::whole_root_before_argument_writers::observe_initial_empty_reads_for_test_v1(
        owner,
        source,
        actual_inputs,
        inventory,
        budget,
    )?;
    super::super::whole_root_before_argument_writers::observe_initial_graph_for_test_v1(
        owner,
        source,
        actual_inputs,
        inventory,
        budget,
    )?;
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, |budget| {
        let before = Custody::take(budget)?;
        let bytes = size_of::<(
            [PrefixMode; 5], std::array::IntoIter<PrefixMode, 5>, PrefixMode,
            &ProductionPreRankedKirOwnerV1, &CheckedBf16CallInstanceV1<'static>,
            &ActualRetainedRankedInputsV1<'static>, &CanonicalKirInventoryV1<'static>,
            &mut Budget<'static>, Custody, usize, usize,
            Option<PrefixObservation>, PrefixObservation, [usize; 3],
            Result<()>, Result<(Option<PrefixObservation>, [usize; 3])>,
            std::result::Result<Result<()>, PanicPayload>, PanicPayload,
        )>();
        budget.reserve_storage(bytes)?;
        let outcome = catch_unwind(AssertUnwindSafe(|| -> Result<()> {
            budget.charge_work(128)?;
            for mode in [PrefixMode::Compare, PrefixMode::CallbackError, PrefixMode::CallbackPanic,
                PrefixMode::QueryError, PrefixMode::QueryPanic]
            {
                let floor = budget.storage();
                let (observed, visits) = run_prefix(owner, source, actual_inputs, inventory, budget, source.root(), mode)?;
                if budget.storage() != floor { return Err(Resource::Accounting.into()); }
                if matches!(mode, PrefixMode::QueryError | PrefixMode::QueryPanic) {
                    if observed.is_some() || visits[0] == 0 {
                        return Err(QueryError::Unavailable("actual capability prefix query failure evidence differs"));
                    }
                    eprintln!("fe2o3-actual-capability-prefix-v1 mode={mode:?} actual_query_entered=true completed=false custody=true ordinary_route=false whole_root_before_argument_writers=false");
                    continue;
                }
                let observed = observed.ok_or(QueryError::Unavailable("actual capability prefix did not compare complete DATA"))?;
                if observed.constants == 0 || observed.blocks == 0
                    || observed.bound_reads == 0 || observed.first_visits == 0 || observed.repeated_visits == 0
                    || observed.original_work == 0 || observed.query_visits[0] == 0
                    || observed.query_visits[1] == 0 || observed.query_visits[2] != 1
                {
                    return Err(QueryError::Unavailable("actual capability prefix positive source evidence absent"));
                }
                eprintln!("fe2o3-actual-capability-prefix-v1 mode={mode:?} constants={} nonempty_constants={} blocks={} layouts={} bound_reads={} first_visits={} repeated_visits={} original_work={} queries={:?} constants_before_prefix=true actual_prefix=true option_enum_scalar_provenance_allocation=true both_fifo_passes=true final_replay=true read_binding=true independent_original_data=true actual_query_authentication=true custody=true ordinary_route=false whole_root_before_argument_writers=false f2=false",
                    observed.constants, observed.nonempty_constants, observed.blocks, observed.layouts,
                    observed.bound_reads, observed.first_visits, observed.repeated_visits,
                    observed.original_work, observed.query_visits);
            }
            let floor = budget.storage();
            let refused = run_prefix(owner, source, actual_inputs, inventory, budget, source.helper(), PrefixMode::Compare);
            if !matches!(refused, Err(QueryError::Unavailable(_))) || budget.storage() != floor {
                return Err(QueryError::Unavailable("actual capability prefix helper/root substitution control differs"));
            }
            Ok(())
        }));
        let result = match outcome {
            Ok(value) => value,
            Err(payload) => { drop(payload); Err(QueryError::CallbackPanicked) },
        };
        before.check(budget, bytes)?;
        budget.release_storage(bytes)?;
        result
    })
}
