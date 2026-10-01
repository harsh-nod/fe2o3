//! Retained genuine Shared-first whole-root prefix, ending before argument writers.
//! This is source-bound DATA and partial custody, not a ranked recipe or ready token.
use super::*;
use crate::production_pipeline::ActualRetainedRankedInputsV1;
use crate::production_ranked_projection_v1::bf16_nominal_dense_v1::{
    NominalCapabilityConsumerV1, NominalCapabilityInputsV1, NominalCapabilityPassV1,
    NominalCapabilityVisitV1, RetainedNominalCapabilityDriverV1,
};
use crate::production_ranked_projection_v1::bf16_nominal_preparation_resources_v1::{
    PreparationCustodySnapshotV1 as Snapshot, retained_custody_snapshot_frame_v1,
};
use crate::production_ranked_projection_v1::bf16_nominal_source_algorithms_v1::RetainedConstantLocalsV1;
use crate::production_ranked_projection_v1::canonical_assertion_facts_v1::{
    ActualSelectedInputsV1, CanonicalSourceAssertionFactsV1,
    select_actual_capability_prefix_inputs_v1, with_nominal_capability_consumer_v1,
    with_nominal_source_preparation_v1,
};
use crate::production_ranked_projection_v1::multi_entry_induction_v1::{
    RetainedLazyScopePrefixV1, retained_lazy_prefix_frame_v1,
};
use crate::production_ranked_projection_v1::root_entry_prefix_preparation_v1::{
    RootEntryPrefixV1, prepare_root_entry_prefix_paid_v1,
};
use crate::production_ranked_projection_v1::scalar_borrow_projection_v1::RetainedScalarBorrowsV1;
use crate::production_ranked_projection_v1::scalar_singleton_projection_v1::RetainedScalarSingletonV1;
use crate::production_ranked_projection_v1::slice_extent_projection_v1::{
    RetainedSliceScopePrefixV1, retained_slice_prefix_frame_v1,
};
use fe2o3_mir_model::{
    RetainedSemanticU32InductionV1, SemanticU32InductionAnalysisLimitsV1,
    SemanticU32InductionBoundSnapshotMeterV1, SemanticU32InductionMeteredErrorV1,
    SemanticU32InductionNoOverflowReportV1,
};
use fe2o3_pliron::{ProductionSemanticSharedReadsPreparationV1, ProductionSemanticSharedReadsV1};

use crate::production_ranked_projection_v1::slice_projection_v1::{
    ProjectedViewPrefixLoanV1, RetainedProjectedViewPrefixV1,
};
#[path = "whole_root_before_argument_writers_frame_v1.rs"]
mod frames;
#[path = "retained_initial_capability_graph_v1.rs"]
mod initial_graph;
#[path = "retained_initial_strided_read_v1.rs"]
mod initial_strided_reads;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WholePhase {
    Fresh,
    Terminal,
    BeforeArgumentWriters,
    InitialStridedReadsTerminal,
    AfterInitialStridedReads,
    InitialCapabilityGraphTerminal,
    AfterInitialCapabilityGraph,
    InvocationSeedsTerminal,
    AfterInvocationSeeds,
}

/// Must be physically outside the actual checked/canonical factory and its
/// postflight. No component field can be extracted or replaced after entry.
pub(in crate::production_ranked_projection_v1) struct PendingWholeRootBeforeArgumentWritersV1<'s> {
    phase: WholePhase,
    owner: Option<&'s ProductionPreRankedKirOwnerV1>,
    function: Option<SemanticFunctionIdV1>,
    entry: Option<Snapshot>,
    held: Option<Snapshot>,
    failure: Option<Backend>,
    shared: ProductionSemanticSharedReadsPreparationV1<'s>,
    singleton: RetainedScalarSingletonV1,
    borrows: RetainedScalarBorrowsV1<'s>,
    lazy: RetainedLazyScopePrefixV1,
    induction: RetainedSemanticU32InductionV1<'s, Resource>,
    selected: Option<ActualSelectedInputsV1<'s>>,
    constants: RetainedConstantLocalsV1,
    prefix: RootEntryPrefixV1,
    incomplete: Option<&'static str>,
    views: RetainedProjectedViewPrefixV1<'s>,
    discarded_ir: String,
    slice: RetainedSliceScopePrefixV1,
    earlier: PendingBeforeCapabilitiesV1,
    driver: RetainedNominalCapabilityDriverV1,
    query_visits: [usize; 3],
    arguments: RetainedBeforeArgumentWritersV1,
    initial_reads: initial_strided_reads::RetainedInitialStridedReadV1,
    initial_graph: initial_graph::RetainedInitialCapabilityGraphV1,
    invocation_seeds: initial_graph::RetainedInvocationSeedV1,
}
impl<'s> PendingWholeRootBeforeArgumentWritersV1<'s> {
    pub(in crate::production_ranked_projection_v1) fn new() -> Self {
        Self {
            phase: WholePhase::Fresh,
            owner: None,
            function: None,
            entry: None,
            held: None,
            failure: None,
            shared: ProductionSemanticSharedReadsPreparationV1::new(),
            singleton: RetainedScalarSingletonV1::new(),
            borrows: RetainedScalarBorrowsV1::new(),
            lazy: RetainedLazyScopePrefixV1::new(),
            induction: RetainedSemanticU32InductionV1::new(),
            selected: None,
            constants: RetainedConstantLocalsV1::new(),
            prefix: RootEntryPrefixV1::empty(),
            incomplete: None,
            views: RetainedProjectedViewPrefixV1::new(),
            discarded_ir: String::new(),
            slice: RetainedSliceScopePrefixV1::new(),
            earlier: PendingBeforeCapabilitiesV1::new(),
            driver: RetainedNominalCapabilityDriverV1::new(),
            query_visits: [0; 3],
            arguments: RetainedBeforeArgumentWritersV1::new(),
            initial_reads: initial_strided_reads::RetainedInitialStridedReadV1::new(),
            initial_graph: initial_graph::RetainedInitialCapabilityGraphV1::new(),
            invocation_seeds: initial_graph::RetainedInvocationSeedV1::new(),
        }
    }
    /// Sealed real-facts entry. The input constructor and genuine observation
    /// factories remain outside this component; there is no raw source fallback.
    pub(in crate::production_ranked_projection_v1) fn prepare_into(
        &mut self,
        owner: &'s ProductionPreRankedKirOwnerV1,
        checked: &CheckedBf16NominalCallV1<'_>,
        actual: &'s ActualRetainedRankedInputsV1<'_>,
        facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
        owned: &mut usize,
    ) -> Result<()> {
        if self.phase != WholePhase::Fresh {
            return Err(self
                .failure
                .as_ref()
                .map(saved_query_error)
                .unwrap_or(QueryError::Resource(Resource::Accounting)));
        }
        self.phase = WholePhase::Terminal;
        self.owner = Some(owner);
        self.function = Some(checked.emission().root());
        let result = self.prepare_original(owner, checked, actual, facts, owned);
        match result {
            Ok(()) => {
                self.phase = WholePhase::BeforeArgumentWriters;
                Ok(())
            }
            Err(error) => {
                let mapped = saved_query_error(&error);
                self.failure = Some(error);
                Err(mapped)
            }
        }
    }
    fn prepare_original(
        &mut self,
        owner: &'s ProductionPreRankedKirOwnerV1,
        checked: &CheckedBf16NominalCallV1<'_>,
        actual: &'s ActualRetainedRankedInputsV1<'_>,
        facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
        owned: &mut usize,
    ) -> BResult<()> {
        let function_id = self.function.ok_or_else(accounting)?;
        if !std::ptr::eq(owner, checked.emission().owner()) || !actual.belongs_to(owner) {
            return Err(Backend::Incomplete(
                "whole-root actual source owner differs",
            ));
        }
        self.entry =
            Some(facts.retained_whole_root_snapshot_v1(owner, function_id, owned, None)?);
        // New whole-owner frame only. No source query is moved ahead of Shared.
        let frame = frames::frame()?;
        let next_owned = owned.checked_add(frame).ok_or_else(arithmetic)?;
        facts.reserve_scalar_private_storage_v1(frame)?;
        *owned = next_owned;
        // Original Shared-first backend prefix and unchanged canonical source seam.
        facts.charge_private_array_work(4)?;
        let before = self.entry.ok_or_else(accounting)?;
        if facts.helper_value_ledger_v1()? != (before.budget_slot, before.work_ledger)
            || facts.scalar_private_storage_v1()? < before.storage
        {
            return Err(accounting());
        }
        facts.prepare_retained_shared_reads_v1(
            &mut self.shared,
            owner.semantic_ssa(),
            function_id,
            owned,
        )?;
        facts.check_retained_shared_reads_v1(
            &self.shared,
            owner.semantic_ssa(),
            function_id,
            owned,
        )?;
        self.shared.view().ok_or_else(accounting)?;
        let semantic = owner.semantic_ssa().source_semantic();
        let function = semantic
            .functions()
            .get(function_id.index() as usize)
            .ok_or(Backend::Unsupported("an out-of-range semantic kernel body"))?;
        // Explicit bounded component enrollment, not an ordinary-route widening.
        if semantic.functions().len() != 2
            || semantic.types().len() > 4096
            || semantic.callables().len() > 4096
            || function.blocks().len() > 32
            || function.locals().len() > 4096
        {
            return Err(Backend::Incomplete(
                "whole-root pre-writer source exceeds closed profile",
            ));
        }
        let source = Source {
            function,
            callables: semantic.callables(),
            types: semantic.types(),
            ledger: (before.budget_slot, before.work_ledger),
        };
        with_nominal_source_preparation_v1(facts, function, source.ledger, owned, |resources| {
            self.singleton
                .prepare_into(semantic.types(), function, resources)?;
            self.borrows
                .prepare_into(semantic.types(), function, semantic.target(), resources)
        })?;
        // Snapshot before full-CFG induction and the shorter-lived slice reservation.
        self.lazy.capture_into(facts)?;
        with_nominal_source_preparation_v1(facts, function, source.ledger, owned, |resources| {
            if self
                .induction
                .prepare_into(
                    semantic,
                    function_id,
                    SemanticU32InductionAnalysisLimitsV1::default(),
                    &mut InductionMeter(resources),
                )
                .is_err()
            {
                return Err(induction_error(&self.induction));
            }
            self.induction
                .completed_for(semantic, function_id)
                .map_err(|_| induction_error(&self.induction))?;
            // The exact actual root/body/launch validation precedes constants.
            if function.role() != SemanticFunctionRoleV1::KernelRoot {
                return Err(Backend::Unsupported("a root without the KernelRoot role"));
            }
            self.selected = Some(select_actual_capability_prefix_inputs_v1(
                owner, checked, function, actual, resources,
            )?);
            self.constants.prepare_into(function, resources)?;
            let selected = self.selected.as_ref().ok_or_else(accounting)?;
            prepare_root_entry_prefix_paid_v1(
                selected.source_root(),
                selected.references(),
                &mut self.prefix,
                resources,
            )?;
            self.views
                .prepare_into(function, semantic.types(), semantic.target(), resources)
        })?;
        let (singletons, borrows) = with_nominal_source_preparation_v1(
            facts,
            function,
            source.ledger,
            owned,
            |resources| {
                Ok((
                    self.singleton
                        .completed_for(semantic.types(), function, resources)?,
                    self.borrows.completed_for(
                        semantic.types(),
                        function,
                        semantic.target(),
                        resources,
                    )?,
                ))
            },
        )?;
        facts.check_retained_shared_reads_v1(
            &self.shared,
            owner.semantic_ssa(),
            function_id,
            owned,
        )?;
        let shared = self.shared.view().ok_or_else(accounting)?;
        {
            let Self {
                views,
                slice,
                earlier,
                constants,
                driver,
                query_visits,
                arguments,
                ..
            } = self;
            let mut loan = views.loan(singletons, shared, borrows, semantic.target(), facts)?;
            // Original initial destinations are still attached and unmodified.
            let extent_work = function
                .blocks()
                .len()
                .checked_mul(16)
                .ok_or_else(arithmetic)?;
            loan.charge_private_array_work(extent_work)?;
            loan.with_assertion_facts_v1(|facts| {
                with_nominal_source_preparation_v1(facts, function, source.ledger, owned, |resources| {
                    slice.prepare_into(function.locals().len(), resources)?;
                    earlier.prepare(&source, resources)
                })?;
                let (constant_values, preparation) = with_nominal_source_preparation_v1(
                    facts, function, source.ledger, owned, |resources| {
                        Ok((constants.completed_for(function, resources)?, earlier.view(&source, resources)?))
                    })?;
                let inputs = NominalCapabilityInputsV1::from_borrowed_source_v1(
                    function, preparation.enum_dominance(),
                    preparation.allocation_contracts(), constant_values);
                with_nominal_capability_consumer_v1(facts, |site, consumer| {
                    let mut visit = |pass, authenticated: &crate::production_ranked_projection_v1::bf16_nominal_capabilities_v1::AuthenticatedNominalCallerV1<'_>,
                        _query_budget: &mut Budget<'_>| {
                        let index = match pass { NominalCapabilityPassV1::Initial=>0,
                            NominalCapabilityPassV1::Repeated=>1, NominalCapabilityPassV1::Final=>2 };
                        query_visits[index] = query_visits[index].checked_add(1)
                            .ok_or(QueryError::Resource(Resource::Arithmetic))?;
                        let current = authenticated.candidate().call();
                        if !std::ptr::eq(current.emission().owner(), owner)
                            || !std::ptr::eq(current.source_call(), checked.source_call())
                            || current.emission().root() != checked.emission().root()
                            || current.emission().helper() != checked.emission().helper()
                            || current.emission().return_permutation() != checked.emission().return_permutation()
                            || authenticated.site().caller() != function_id {
                            return Err(QueryError::Unavailable("whole-root genuine capability query differs"));
                        }
                        Ok(())
                    };
                    driver.prepare_into(site, &inputs, consumer, owned, &mut visit)?;
                    driver.completed_for(site, &inputs, consumer)?;
                    Ok(())
                }).map_err(nominal_error)?;
                with_nominal_source_preparation_v1(facts, function, source.ledger, owned, |resources| {
                    arguments.prepare_into(function.locals().len(), resources)
                })
            })?;
        }
        self.check_before_writers(owner, function_id, facts, owned)?;
        self.held =
            Some(facts.retained_whole_root_snapshot_v1(owner, function_id, owned, self.entry)?);
        Ok(())
    }
    fn check_before_writers(
        &self,
        owner: &ProductionPreRankedKirOwnerV1,
        function_id: SemanticFunctionIdV1,
        facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
        owned: &mut usize,
    ) -> BResult<()> {
        if !self.owner.is_some_and(|bound| std::ptr::eq(bound, owner))
            || self.function != Some(function_id)
            || self.failure.is_some()
        {
            return Err(accounting());
        }
        let before = self.entry.ok_or_else(accounting)?;
        let now = facts.retained_whole_root_snapshot_v1(
            owner,
            function_id,
            owned,
            self.held.or(self.entry),
        )?;
        let expected = before
            .storage
            .checked_add(now.owned.checked_sub(before.owned).ok_or_else(accounting)?)
            .ok_or_else(arithmetic)?;
        if now.storage != expected {
            return Err(accounting());
        }
        facts.check_retained_shared_reads_v1(
            &self.shared,
            owner.semantic_ssa(),
            function_id,
            owned,
        )?;
        let semantic = owner.semantic_ssa().source_semantic();
        let function = semantic
            .functions()
            .get(function_id.index() as usize)
            .ok_or_else(accounting)?;
        self.induction
            .completed_for(semantic, function_id)
            .map_err(|_| induction_error(&self.induction))?;
        self.lazy.before_writers()?;
        if self.selected.is_none()
            || self.incomplete.is_some()
            || !self.discarded_ir.is_empty()
            || self.discarded_ir.capacity() != 0
            || self.query_visits[0] == 0
            || self.query_visits[1] == 0
            || self.query_visits[2] != 1
        {
            return Err(accounting());
        }
        with_nominal_source_preparation_v1(
            facts,
            function,
            (before.budget_slot, before.work_ledger),
            owned,
            |resources| {
                self.views.before_writers(
                    function,
                    semantic.types(),
                    semantic.target(),
                    resources,
                )?;
                self.slice
                    .before_writers(function.locals().len(), resources)?;
                self.arguments
                    .before_writers(function.locals().len(), resources)
            },
        )
    }
    /// Fixed completion check, not a consuming finish or producer/writer loan.
    pub(in crate::production_ranked_projection_v1) fn completed_for(
        &self,
        owner: &ProductionPreRankedKirOwnerV1,
        function: SemanticFunctionIdV1,
        facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
        owned: &mut usize,
    ) -> BResult<()> {
        if self.phase != WholePhase::BeforeArgumentWriters {
            return Err(accounting());
        }
        self.check_before_writers(owner, function, facts, owned)
    }
}
fn nominal_error(error: QueryError) -> Backend {
    match error {
        QueryError::Resource(error) => resource(error),
        error => Backend::CanonicalAssertions(CanonicalAssertionErrorV1::NominalCall(error)),
    }
}
struct InductionMeter<'r, 'b, 'w>(&'r mut Prep<'b, 'w>);
impl SemanticU32InductionBoundSnapshotMeterV1 for InductionMeter<'_, '_, '_> {
    type Error = Resource;
    fn charge_work(&mut self, amount: usize) -> std::result::Result<(), Resource> {
        self.0.work(amount).map_err(preparation_resource)
    }
    fn reserve_storage(&mut self, amount: usize) -> std::result::Result<(), Resource> {
        self.0.reserve_storage(amount).map_err(preparation_resource)
    }
}
fn preparation_resource(error: Backend) -> Resource {
    match error {
        Backend::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(error)) => error,
        _ => Resource::Accounting, // Original Prep work/reserve have only Resource errors.
    }
}
fn induction_error(induction: &RetainedSemanticU32InductionV1<'_, Resource>) -> Backend {
    match induction.failure() {
        Some(SemanticU32InductionMeteredErrorV1::Analysis(error)) => {
            Backend::SemanticU32Induction(*error)
        }
        Some(SemanticU32InductionMeteredErrorV1::Meter(error)) => resource(*error),
        Some(SemanticU32InductionMeteredErrorV1::Allocation) => resource(Resource::Allocation),
        Some(SemanticU32InductionMeteredErrorV1::Arithmetic) => arithmetic(),
        None => accounting(),
    }
}
/// Original post-capability scalar/Vec initialization only. No later argument writer.
struct RetainedBeforeArgumentWritersV1 {
    entered: bool,
    initialized: bool,
    edge_count: usize,
    borrowed_locals: Vec<(usize, usize)>,
    runtime_index_arguments: Vec<Option<u32>>,
    runtime_slice_extent_arguments: Vec<Option<u32>>,
    next_runtime_argument: usize,
}
impl RetainedBeforeArgumentWritersV1 {
    fn new() -> Self {
        Self {
            entered: false,
            initialized: false,
            edge_count: 0,
            borrowed_locals: Vec::new(),
            runtime_index_arguments: Vec::new(),
            runtime_slice_extent_arguments: Vec::new(),
            next_runtime_argument: 0,
        }
    }
    fn prepare_into(&mut self, count: usize, resources: &mut Prep<'_, '_>) -> BResult<()> {
        if self.entered {
            return Err(accounting());
        }
        self.entered = true;
        // Exact original order: edge_count, borrowed locals, index rows, slice rows, 1.
        self.edge_count = 0;
        resources.work(count)?;
        resources.reserve(&mut self.runtime_index_arguments, count)?;
        self.runtime_index_arguments.resize(count, None);
        resources.work(count)?;
        resources.reserve(&mut self.runtime_slice_extent_arguments, count)?;
        self.runtime_slice_extent_arguments.resize(count, None);
        self.next_runtime_argument = 1;
        self.initialized = true;
        self.before_writers(count, resources)
    }
    fn before_writers(&self, count: usize, resources: &mut Prep<'_, '_>) -> BResult<()> {
        if !self.entered
            || !self.initialized
            || self.edge_count != 0
            || !self.borrowed_locals.is_empty()
            || self.borrowed_locals.capacity() != 0
            || self.runtime_index_arguments.len() != count
            || self.runtime_slice_extent_arguments.len() != count
            || self.next_runtime_argument != 1
        {
            return Err(accounting());
        }
        resources.work(count.checked_mul(2).ok_or_else(arithmetic)?)?;
        if self.runtime_index_arguments.iter().any(Option::is_some)
            || self
                .runtime_slice_extent_arguments
                .iter()
                .any(Option::is_some)
        {
            return Err(accounting());
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "whole_root_before_argument_writers_genuine_v1_tests.rs"]
mod genuine;
#[cfg(test)]
pub(in crate::production_ranked_projection_v1) use genuine::observe as observe_before_writers_for_test_v1;

#[cfg(test)]
#[path = "whole_root_before_argument_writers_v1_tests.rs"]
mod tests;

#[cfg(test)]
pub(in crate::production_ranked_projection_v1) use genuine::observe_initial_reads as observe_initial_empty_reads_for_test_v1;

#[cfg(test)]
pub(in crate::production_ranked_projection_v1) use genuine::observe_initial_graph as observe_initial_graph_for_test_v1;

#[cfg(test)]
pub(in crate::production_ranked_projection_v1) use genuine::observe_invocation_seeds as observe_invocation_seeds_for_test_v1;
