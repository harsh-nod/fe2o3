//! Genuine-observation argument initialization, not a producer/factory completion.
//! Earlier intrinsic effects and their first errors are NOT reproduced here.
//! The sole non-test entry is an authentic owning-assembly sibling; F2 is pending.
#![allow(dead_code)]
use super::*;
use crate::production_ranked_projection_v1::root_direct_comparison_preparation_v1::DirectComparisonPreparationV1;
use fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1;
type LedgerId = (usize, CanonicalKernelIrWorkLedgerIdentityV1);
type Prep<'b, 'w> = PreparationResourcesV1<'b, 'w>;
type Context = NominalRecipeResourcesV1<'static, 'static, 'static, 'static, 'static, 'static>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Phase {
    Dormant,
    Terminal,
    InitializedBeforeArgumentWriters,
}

/// Empty destinations only. No missing producer is represented as completed.
/// F2/F3 must attach their own partial/candidate storage before fallible work.
struct LaterProducerPayloads {
    ordinary_indices: Vec<Option<ProjectedOrdinaryIndexV1>>,
    direct: Option<DirectComparisonPreparationV1>,
    uniform_inductions: Vec<ProjectedUniformInductionV1>,
    switch_predicates: Vec<Option<GuardPredicateV1>>,
    deterministic_switches: Vec<Option<ProjectedDeterministicSwitchV1>>,
    read_views: Vec<Option<GuardedRankedAccessV1>>,
    direct_reads: Vec<Option<GuardedRankedAccessV1>>,
    direct_writes: Vec<Option<GuardedRankedAccessV1>>,
    pipeline_effects: Vec<Option<ProjectedPipelineEffectV1>>,
    generated_effects: Vec<Option<Vec<ProjectedGeneratedExecutableEffectV1>>>,
}
impl LaterProducerPayloads {
    const fn empty() -> Self {
        Self {
            ordinary_indices: Vec::new(),
            direct: None,
            uniform_inductions: Vec::new(),
            switch_predicates: Vec::new(),
            deterministic_switches: Vec::new(),
            read_views: Vec::new(),
            direct_reads: Vec::new(),
            direct_writes: Vec::new(),
            pipeline_effects: Vec::new(),
            generated_effects: Vec::new(),
        }
    }
    fn vacant(&self) -> bool {
        fn vacant<T>(v: &Vec<T>) -> bool {
            v.is_empty() && v.capacity() == 0
        }
        vacant(&self.ordinary_indices)
            && self.direct.is_none()
            && vacant(&self.uniform_inductions)
            && vacant(&self.switch_predicates)
            && vacant(&self.deterministic_switches)
            && vacant(&self.read_views)
            && vacant(&self.direct_reads)
            && vacant(&self.direct_writes)
            && vacant(&self.pipeline_effects)
            && vacant(&self.generated_effects)
    }
}

/// Physically owned beside, never instead of, the existing prefix/SSA owner.
pub(super) struct PendingArgumentProducersV1 {
    pub(super) phase: Phase,
    index_arguments: Vec<Option<u32>>,
    slice_arguments: Vec<Option<u32>>,
    next_argument: usize,
    function: Option<usize>,
    ledger: Option<LedgerId>,
    initialized: bool,
    later: LaterProducerPayloads,
}
impl PendingArgumentProducersV1 {
    pub(super) const fn new() -> Self {
        Self {
            phase: Phase::Dormant,
            index_arguments: Vec::new(),
            slice_arguments: Vec::new(),
            next_argument: 0,
            function: None,
            ledger: None,
            initialized: false,
            later: LaterProducerPayloads::empty(),
        }
    }
    pub(super) fn dormant(&self) -> bool {
        self.phase == Phase::Dormant
    }
    fn vacant(&self) -> bool {
        self.dormant()
            && self.index_arguments.is_empty()
            && self.index_arguments.capacity() == 0
            && self.slice_arguments.is_empty()
            && self.slice_arguments.capacity() == 0
            && self.next_argument == 0
            && self.function.is_none()
            && self.ledger.is_none()
            && !self.initialized
            && self.later.vacant()
    }
    fn initialize(
        &mut self,
        source: &InitializationSource<'_>,
        resources: &mut Prep<'_, '_>,
    ) -> Result<()> {
        // Only the owning driver constructs source or calls this method.
        // The phase is already terminal before any authentic driver admission.
        if self.phase != Phase::Terminal
            || self.initialized
            || self.function.is_some()
            || self.ledger.is_some()
            || !self.index_arguments.is_empty()
            || self.index_arguments.capacity() != 0
            || !self.slice_arguments.is_empty()
            || self.slice_arguments.capacity() != 0
            || self.next_argument != 0
            || !self.later.vacant()
            || !resources.is_metered()
            || resources.has_denial()
            || resources.original_ledger_v1() != Some(source.ledger)
        {
            return Err(resource(Resource::Accounting));
        }
        self.function = Some(source.function as *const SemanticFunctionDeclV1 as usize);
        self.ledger = Some(source.ledger);
        self.initialize_payload(source.function.locals().len(), resources)
    }
    // Private mechanics; cfg(test) controls call this without claiming source authority.
    fn initialize_payload(&mut self, locals: usize, resources: &mut Prep<'_, '_>) -> Result<()> {
        // Exact donor order: index slots, slice slots, then counter = 1.
        // Each payload is attached BEFORE its reserve/resize, including refusal.
        initialize_rows(&mut self.index_arguments, locals, resources)?;
        initialize_rows(&mut self.slice_arguments, locals, resources)?;
        self.next_argument = 1;
        if resources.has_denial() {
            return Err(resource(Resource::Accounting));
        }
        self.initialized = true;
        Ok(())
    }
}
fn initialize_rows(
    rows: &mut Vec<Option<u32>>,
    count: usize,
    resources: &mut Prep<'_, '_>,
) -> Result<()> {
    resources.work(count)?;
    resources.reserve(rows, count)?;
    rows.resize(count, None);
    Ok(())
}

/// Private source loan, constructed only after the real assembly/source/input join.
/// Neither equal caller DATA nor a ledger pair is an alternative constructor.
struct InitializationSource<'a> {
    function: &'a SemanticFunctionDeclV1,
    source_root: ProductionSourceLaunchRootV1,
    input: &'a ProductionRankedRootInputV1,
    references: &'a [AuthenticatedReferenceEffectBindingV1],
    ledger: LedgerId,
}

/// Read-only DATA checkpoint. It is NOT pre-direct/AfterAll/bounds-ready authority.
pub(in crate::production_ranked_projection_v1) struct ActualRootArgumentInitializationV1<'a> {
    source: InitializationSource<'a>,
    prefix: &'a RootEntryPrefixV1,
    arguments: &'a PendingArgumentProducersV1,
}
impl ActualRootArgumentInitializationV1<'_> {
    pub(in crate::production_ranked_projection_v1) fn function(&self) -> &SemanticFunctionDeclV1 {
        self.source.function
    }
    pub(in crate::production_ranked_projection_v1) fn index_arguments(&self) -> &[Option<u32>] {
        &self.arguments.index_arguments
    }
    pub(in crate::production_ranked_projection_v1) fn slice_arguments(&self) -> &[Option<u32>] {
        &self.arguments.slice_arguments
    }
    pub(in crate::production_ranked_projection_v1) fn next_argument(&self) -> usize {
        self.arguments.next_argument
    }
    pub(in crate::production_ranked_projection_v1) fn entry_operations(
        &self,
    ) -> &[ProductionRankedOperationV1] {
        &self.prefix.entry_operations
    }
    pub(in crate::production_ranked_projection_v1) fn next_value(&self) -> u32 {
        self.prefix.next_value
    }
}

fn arithmetic() -> Error {
    resource(Resource::Arithmetic)
}
fn call_frame<T>(locals: usize) -> Result<usize> {
    locals
        .checked_add(size_of::<T>().checked_mul(2).ok_or_else(arithmetic)?)
        .and_then(|n| {
            size_of::<Result<T>>()
                .checked_mul(2)
                .and_then(|r| n.checked_add(r))
        })
        .ok_or_else(arithmetic)
}
fn sum(rows: &[usize]) -> Result<usize> {
    rows.iter()
        .try_fold(0usize, |n, row| n.checked_add(*row).ok_or_else(arithmetic))
}
const FRAME_ROWS: usize = 25;
/// New selected-source slots only; NO newly invented fixed padding.
/// Existing assembly/graph/prefix callees retain their separate exact contracts.
/// Vec allocator/resize implementation stack and generated native frames excluded.
fn frame_rows<R, F>() -> Result<[usize; FRAME_ROWS]> {
    type P = PendingActualRootPrefixIndicesV1;
    type A = PendingArgumentProducersV1;
    type S = InitializationSource<'static>;
    type V = ActualRootArgumentInitializationV1<'static>;
    type Resources = Prep<'static, 'static>;
    Ok([
        // Entry caller, explicit parameters, closure/result transfers.
        call_frame::<R>(size_of::<(
            &mut Context,
            &CheckedBf16NominalCallV1<'static>,
            &RichNominalSourceTablesV1<'static>,
            &crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
            &mut P,
            F,
            Result<R>,
            LedgerId,
            bool,
            &ProductionPreRankedKirOwnerV1,
        )>())?,
        // Source-graph continuation; the graph constructor ALSO admits its concrete closure.
        call_frame::<R>(size_of::<(
            F,
            &mut Context,
            NominalCompleteForProfileGraphV1<'static>,
            &mut RootEntryPrefixV1,
            &mut A,
            &ProductionPreRankedKirOwnerV1,
            ActualSelectedInputsV1<'static>,
            S,
            V,
            Result<R>,
        )>())?,
        // Admission resource callback, including all captured argument borrows.
        call_frame::<()>(size_of::<(
            &mut Resources,
            &mut P,
            &ProductionPreRankedKirOwnerV1,
            &CheckedBf16NominalCallV1<'static>,
            &RichNominalSourceTablesV1<'static>,
            &crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
            LedgerId,
            Option<LedgerId>,
            usize,
            Result<usize>,
            bool,
        )>())?,
        // New prefix+initializer resource callback, not the unchanged prefix body.
        call_frame::<()>(size_of::<(
            &mut Resources,
            &mut RootEntryPrefixV1,
            &mut A,
            &S,
        )>())?,
        // Attached initializer.
        call_frame::<()>(size_of::<(
            &mut A,
            &S,
            &mut Resources,
            usize,
            Option<LedgerId>,
            bool,
        )>())?,
        // Two distinct reached row initializer calls and their simultaneous transfers.
        call_frame::<()>(size_of::<(
            &mut Vec<Option<u32>>,
            usize,
            &mut Resources,
            Option<u32>,
        )>())?
        .checked_mul(2)
        .ok_or_else(arithmetic)?,
        // Original preparation reserve<Option<u32>> reached by the new initializer.
        call_frame::<()>(size_of::<(
            &mut Resources,
            &mut Vec<Option<u32>>,
            usize,
            usize,
            Option<usize>,
            bool,
            std::result::Result<(), std::collections::TryReserveError>,
        )>())?,
        call_frame::<()>(size_of::<(&mut Resources, &mut &mut Budget<'static>, usize)>())?, // work
        call_frame::<()>(size_of::<(
            &mut Resources,
            usize,
            &mut &mut Budget<'static>,
            &mut &mut usize,
            usize,
            Option<usize>,
        )>())?, // storage
        call_frame::<Option<LedgerId>>(size_of::<(
            &Resources,
            &&mut Budget<'static>,
            &Budget<'static>,
            LedgerId,
        )>())?,
        call_frame::<bool>(size_of::<(
            &Resources,
            &&mut Budget<'static>,
            Option<usize>,
            Option<usize>,
        )>())?
        .checked_add(call_frame::<bool>(size_of::<&Resources>())?)
        .ok_or_else(arithmetic)?,
        call_frame::<&[SemanticLocalDeclV1]>(size_of::<&SemanticFunctionDeclV1>())?,
        // Three separate source methods, each with its actual receiver.
        sum(&[
            call_frame::<bool>(size_of::<(&A, bool)>())?,
            call_frame::<bool>(size_of::<(&A, Phase)>())?,
            call_frame::<bool>(size_of::<(&LaterProducerPayloads, bool)>())?,
        ])?,
        // Nine reached concrete Vec<T> vacant callees; headers only, never T traversal.
        sum(&[
            call_frame::<bool>(size_of::<(&Vec<Option<ProjectedOrdinaryIndexV1>>, bool)>())?,
            call_frame::<bool>(size_of::<(&Vec<ProjectedUniformInductionV1>, bool)>())?,
            call_frame::<bool>(size_of::<(&Vec<Option<GuardPredicateV1>>, bool)>())?,
            call_frame::<bool>(size_of::<(
                &Vec<Option<ProjectedDeterministicSwitchV1>>,
                bool,
            )>())?,
            call_frame::<bool>(size_of::<(&Vec<Option<GuardedRankedAccessV1>>, bool)>())?
                .checked_mul(3)
                .ok_or_else(arithmetic)?,
            call_frame::<bool>(size_of::<(&Vec<Option<ProjectedPipelineEffectV1>>, bool)>())?,
            call_frame::<bool>(size_of::<(
                &Vec<Option<Vec<ProjectedGeneratedExecutableEffectV1>>>,
                bool,
            )>())?,
        ])?,
        // Immutable DATA getters: function, two slots, count, operations, SSA.
        sum(&[
            call_frame::<&SemanticFunctionDeclV1>(size_of::<&V>())?,
            call_frame::<&[Option<u32>]>(size_of::<&V>())?
                .checked_mul(2)
                .ok_or_else(arithmetic)?,
            call_frame::<usize>(size_of::<&V>())?,
            call_frame::<&[ProductionRankedOperationV1]>(size_of::<&V>())?,
            call_frame::<u32>(size_of::<&V>())?,
        ])?,
        // Ending state/result transfer after all nested postflights have returned.
        call_frame::<R>(size_of::<(&mut A, Phase, Result<R>, bool)>())?,
        call_frame::<[usize; FRAME_ROWS]>(size_of::<(
            [usize; FRAME_ROWS],
            usize,
            Option<usize>,
            Result<usize>,
        )>())?,
        call_frame::<usize>(size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            usize,
            &usize,
            Option<usize>,
        )>())?,
        // Arithmetic/resource mapper plus error-only return paths (no arbitrary closure).
        call_frame::<Error>(size_of::<(Resource, Option<usize>)>())?
            .checked_mul(3)
            .ok_or_else(arithmetic)?,
        // New private payload helper, distinct from source authentication.
        call_frame::<()>(size_of::<(&mut A, usize, &mut Resources)>())?,
        // Nested row-array constructors passed to sum above: actual lengths.
        call_frame::<[usize; 3]>(size_of::<[usize; 3]>())?,
        call_frame::<[usize; 7]>(size_of::<[usize; 7]>())?,
        call_frame::<[usize; 5]>(size_of::<[usize; 5]>())?,
        // additional_frame helper caller/return and whole roster receiver.
        call_frame::<usize>(size_of::<([usize; FRAME_ROWS], Result<[usize; FRAME_ROWS]>)>())?,
        // Generic call_frame's executable arithmetic has scalar locals only.
        call_frame::<usize>(size_of::<(usize, usize, usize, Option<usize>, Result<usize>)>())?,
    ])
}
fn additional_frame<R, F>() -> Result<usize> {
    sum(&frame_rows::<R, F>()?)
}

/// Added to the pre-existing assembly envelope as an explicit delta: the new
/// inline owners are constructed even when an OLD prefix observer is selected.
/// No old residual allowance is claimed to cover these new source vertices.
pub(super) fn construction_frame_v1() -> Result<usize> {
    sum(&[
        call_frame::<PendingArgumentProducersV1>(size_of::<(
            Phase,
            Option<usize>,
            Option<LedgerId>,
            bool,
        )>())?,
        call_frame::<LaterProducerPayloads>(0)?,
        call_frame::<bool>(size_of::<(&PendingArgumentProducersV1, Phase, bool)>())?,
        call_frame::<[usize; 8]>(size_of::<[usize; 8]>())?,
        call_frame::<usize>(size_of::<(
            &[usize],
            std::slice::Iter<'static, usize>,
            usize,
            &usize,
            Option<usize>,
        )>())?,
        call_frame::<usize>(size_of::<(usize, Option<usize>, Result<usize>)>())?,
        call_frame::<usize>(size_of::<(usize, usize, usize, Option<usize>, Result<usize>)>())?,
        call_frame::<Error>(size_of::<(Resource, Option<usize>)>())?,
    ])
}

impl NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_> {
    /// NEW observation checkpoint: initializes authentic owned slots before ANY
    /// argument writer. It does NOT reproduce earlier intrinsic-effect errors.
    /// Existing prefix/index entry cannot resume this started owner.
    pub(in crate::production_ranked_projection_v1) fn with_actual_root_argument_initialization_v1<
        R,
        F,
    >(
        &mut self,
        checked: &CheckedBf16NominalCallV1<'_>,
        rich: &RichNominalSourceTablesV1<'_>,
        actual_inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
        pending: &mut PendingActualRootPrefixIndicesV1,
        inspect: F,
    ) -> Result<R>
    where
        F: for<'a> FnOnce(ActualRootArgumentInitializationV1<'a>, &mut Self) -> Result<R>,
    {
        let fresh = !pending.started
            && !pending.completed
            && pending.ledger.is_none()
            && pending.frame_credits == 0
            && pending.arguments.vacant();
        // Failure and unwind cannot leave an initializer or old-factory retry.
        pending.arguments.phase = Phase::Terminal;
        if !fresh {
            return Err(Error::Incomplete(
                "actual argument initialization cannot be replaced or retried",
            ));
        }
        let facts_owner = self.facts.owner;
        let expected = (self.state.slot, self.state.ledger);
        self.with_resources(|resources| {
            resources.work(64)?;
            if !actual_inputs.belongs_to(facts_owner)
                || !actual_inputs.belongs_to(checked.emission().owner())
                || !rich.belongs_to_original_ledger_v1(expected)
                || resources.original_ledger_v1() != Some(expected)
                || resources.has_denial()
            {
                return Err(resource(Resource::Accounting));
            }
            // The old assembly envelope covers unchanged source-selection helpers;
            // all added slots/callees are named separately above, not residual slack.
            let bytes = assembly_frame::<R, F>()?
                .checked_add(additional_frame::<R, F>()?)
                .ok_or_else(arithmetic)?;
            resources.work(bytes)?;
            resources.reserve_storage(bytes)?;
            pending.frame_credits = bytes;
            pending.ledger = Some(expected);
            pending.started = true;
            Ok(())
        })?;
        let result = {
            let PendingActualRootPrefixIndicesV1 {
                graph,
                prefix,
                arguments,
                ..
            } = pending;
            self.with_complete_for_profile_graph_v1(checked, rich, graph, |graph, context| {
                let owner = context.facts.owner;
                let function = graph.function();
                require_same_source_v1(function, rich.function())?;
                let selected = context.with_resources(|resources| {
                    actual_selected_inputs_v1(
                        owner,
                        checked,
                        function,
                        actual_inputs.inputs(),
                        actual_inputs.bindings(),
                        resources,
                    )
                })?;
                let source = InitializationSource {
                    function,
                    source_root: selected.source_root,
                    input: selected.input,
                    references: selected.references,
                    ledger: expected,
                };
                context.with_resources(|resources| {
                    prepare_root_entry_prefix_paid_v1(
                        source.source_root,
                        source.references,
                        prefix,
                        resources,
                    )?;
                    arguments.initialize(&source, resources)
                })?;
                // Honest stop before read views, invocation/ordinary/access/direct
                // writers, inductions, switches, pipelines or extent finalization.
                inspect(
                    ActualRootArgumentInitializationV1 {
                        source,
                        prefix,
                        arguments,
                    },
                    context,
                )
            })
        };
        if result.is_ok() {
            pending.arguments.phase = Phase::InitializedBeforeArgumentWriters;
        }
        result
    }
}

#[cfg(test)]
#[path = "bf16_nominal_root_argument_initialization_genuine_v1_tests.rs"]
mod genuine;
#[cfg(test)]
#[path = "bf16_nominal_root_argument_initialization_v1_tests.rs"]
mod tests;
#[cfg(test)]
pub(crate) use genuine::observe_actual_root_argument_initialization_for_test_v1;
