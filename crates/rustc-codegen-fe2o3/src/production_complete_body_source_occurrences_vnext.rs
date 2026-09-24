//! Actual-source custody for the registered complete-body terminal.
//! This is NOT a diagnostic/KIR importer and grants no executable owner.
//! The full current preflight plan remains borrowed until the one-use source
//! observation is consumed by the registered semantic producer.

use fe2o3_kernel_ir::Gfx942CompleteBodyPackedV1;
use fe2o3_mir_model::semantic_mir_v1::{
    HARD_MAX_CANONICAL_BYTES_V1, SemanticBlockIdentityV1, SemanticCallableIdV1,
    SemanticFunctionIdV1, SemanticSourceProvenanceV1,
};
use rustc_middle::mir::BasicBlock;
use rustc_middle::ty::{Instance, TyCtxt};

use crate::collector::CollectedFunctionRole;
use crate::production_complete_body_call_vnext::{
    ActualCompleteBodyCallVNext, parse_complete_body_consts,
};
use crate::production_complete_body_census_vnext::{observe_root, require_body_bounds};
use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1;
use crate::rustc_semantic_adapter_v1::{
    CanonicalFunctionIdentitiesV1, borrowed_rustc_mir_body_sha256_v1,
    canonical_function_identities_v1, canonical_source_provenance_v1, rustc_block_identity_v1,
};
use crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1;

const MAX_WORK: usize = 1_048_576;
const MAX_EXPANSION_DEPTH: usize = 256;
// Includes both fixed array initializations, then bounded provenance/grammar.
const FIXED_REPLAY_WORK: usize = crate::production_complete_body_census_vnext::MAX_SOURCE_BLOCKS
    + crate::production_complete_body_census_vnext::MAX_SOURCE_LOCALS
    + 2 * MAX_EXPANSION_DEPTH
    + 64;

/// Separate prepaid logical traversal budget, not rustc query/RSS accounting.
pub(crate) struct CompleteBodySourceWorkVNext {
    limit: usize,
    used: usize,
}
impl CompleteBodySourceWorkVNext {
    pub(crate) fn new(limit: usize) -> Result<Self, &'static str> {
        if limit == 0 || limit > MAX_WORK {
            return Err("complete body source work limit invalid");
        }
        Ok(Self { limit, used: 0 })
    }
    fn charge(&mut self, amount: usize) -> Result<(), &'static str> {
        let next = self
            .used
            .checked_add(amount)
            .filter(|value| *value <= self.limit)
            .ok_or("complete body source work exhausted")?;
        self.used = next;
        Ok(())
    }
    #[cfg(test)]
    pub(crate) fn used(&self) -> usize {
        self.used
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CallBinding<I> {
    instance: I,
    packed: Gfx942CompleteBodyPackedV1,
    registers: [u8; 5],
    argument_locals: [u32; 5],
    moved_arguments: u8,
}
impl<'tcx> CallBinding<Instance<'tcx>> {
    fn from_actual(actual: &ActualCompleteBodyCallVNext<'tcx>) -> Self {
        Self {
            instance: actual.instance(),
            packed: actual.packed(),
            registers: actual.registers(),
            argument_locals: actual.argument_locals(),
            moved_arguments: actual.moved_arguments(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Key {
    caller: SemanticFunctionIdV1,
    block: u32,
    callee: SemanticCallableIdV1,
}

/// Testable generic state machine. Production uses only actual rustc Instances.
struct OneUse<I> {
    key: Key,
    pending: Option<CallBinding<I>>,
}
impl<I: Eq> OneUse<I> {
    fn check(&self, key: Key, actual: &CallBinding<I>) -> Result<(), &'static str> {
        if self.key != key {
            return Err("complete body source occurrence key differs");
        }
        let Some(expected) = self.pending.as_ref() else {
            return Err("complete body source occurrence was already consumed");
        };
        if expected != actual {
            return Err("complete body actual source call binding differs");
        }
        Ok(())
    }
    fn take(&mut self, key: Key, actual: &CallBinding<I>) -> Result<CallBinding<I>, &'static str> {
        self.check(key, actual)?;
        self.pending
            .take()
            .ok_or("complete body source occurrence disappeared")
    }
    fn require_drained(&self) -> Result<(), &'static str> {
        if self.pending.is_none() {
            Ok(())
        } else {
            Err("complete body source occurrence unconsumed")
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BodyBinding {
    root: CanonicalFunctionIdentitiesV1,
    terminal: CanonicalFunctionIdentitiesV1,
    mir_body: [u8; 32],
    block_identity: SemanticBlockIdentityV1,
    source: SemanticSourceProvenanceV1,
    expansion_chain: [u8; 32],
    expansion_depth: usize,
}

/// Move-only, no public constructor/Clone/Deserialize. The original sealed plan
/// is retained by borrow; a detached JSON/packed descriptor cannot create it.
pub(crate) struct CompleteBodySourceOccurrencesVNext<'plan, 'tcx> {
    plan: &'plan ProductionSemanticPreflightPlanV1<'tcx>,
    root: Instance<'tcx>,
    body: BodyBinding,
    slot: OneUse<Instance<'tcx>>,
}
pub(crate) struct CompleteBodySourceVNext<'plan, 'tcx> {
    plan: &'plan ProductionSemanticPreflightPlanV1<'tcx>,
    body: BodyBinding,
    call: CallBinding<Instance<'tcx>>,
    key: Key,
}
impl<'plan, 'tcx> CompleteBodySourceVNext<'plan, 'tcx> {
    pub(crate) fn plan(&self) -> &'plan ProductionSemanticPreflightPlanV1<'tcx> {
        self.plan
    }
    pub(crate) fn packed(&self) -> Gfx942CompleteBodyPackedV1 {
        self.call.packed
    }
    pub(crate) fn registers(&self) -> [u8; 5] {
        self.call.registers
    }
    pub(crate) fn root_identities(&self) -> CanonicalFunctionIdentitiesV1 {
        self.body.root
    }
    pub(crate) fn terminal_instance(&self) -> Instance<'tcx> {
        self.call.instance
    }
    pub(crate) fn mir_body_sha256(&self) -> [u8; 32] {
        self.body.mir_body
    }
    pub(crate) fn semantic_block_identity(&self) -> SemanticBlockIdentityV1 {
        self.body.block_identity
    }
    pub(crate) fn raw_block(&self) -> u32 {
        self.key.block
    }
    pub(crate) fn argument_locals(&self) -> [u32; 5] {
        self.call.argument_locals
    }
    /// Exact retained semantic occurrence coordinates; these are positions,
    /// not caller-supplied identities or a constructor for source custody.
    pub(crate) fn occurrence_key_vnext(&self) -> (SemanticFunctionIdV1, u32, SemanticCallableIdV1) {
        (self.key.caller, self.key.block, self.key.callee)
    }
    pub(crate) fn moved_arguments_vnext(&self) -> u8 {
        self.call.moved_arguments
    }
}

impl<'plan, 'tcx> CompleteBodySourceOccurrencesVNext<'plan, 'tcx> {
    /// Absent for legacy profiles. No schema/tag numeric allocation in this leaf.
    pub(crate) fn from_plan(
        tcx: TyCtxt<'tcx>,
        plan: &'plan ProductionSemanticPreflightPlanV1<'tcx>,
        work: &mut CompleteBodySourceWorkVNext,
    ) -> Result<Option<Self>, &'static str> {
        let recipes = plan.terminal_expansion_producers();
        if recipes.len() > crate::production_complete_body_census_vnext::MAX_SOURCE_BLOCKS {
            return Err("complete body source recipe bound exceeded");
        }
        work.charge(recipes.len())?;
        let mut selected = recipes.iter().filter(|recipe| {
            recipe.expansion == ProductionTerminalExpansionV1::Gfx942CompleteBodyE32
        });
        let Some(recipe) = selected.next() else {
            return Ok(None);
        };
        if selected.next().is_some()
            || recipes.len() != 1
            || !plan.direct_call_producers().is_empty()
            || !plan.normalized_intrinsic_producers().is_empty()
        {
            return Err("complete body requires one source marker and no extra calls");
        }
        let [root_id] = plan.roots() else {
            return Err("complete body requires one root");
        };
        let [root] = plan.function_producers() else {
            return Err("complete body excludes helper bodies");
        };
        let [terminal] = plan.terminal_producers() else {
            return Err("complete body terminal roster differs");
        };
        if root_id.index() != 0
            || recipe.caller != *root_id
            || root.role != CollectedFunctionRole::KernelEntry
            || recipe.terminal != 0
            || recipe.arguments != 10
            || terminal.instance != recipe.instance
            || terminal.identities != recipe.identities
            || terminal.expansion != recipe.expansion
        {
            return Err("complete body authenticated root/terminal recipe differs");
        }
        let frontend = root
            .frontend_contract
            .as_ref()
            .ok_or("complete body current frontend contract absent")?;
        let launch = frontend
            .contract()
            .launch()
            .ok_or("complete body explicit launch bounds absent")?;
        if launch.required().map(|value| value.as_array()) != Some([64, 1, 1])
            || launch.maximum().map(|value| value.as_array()) != Some([64, 1, 1])
        {
            return Err("complete body requires required and maximum 64x1x1");
        }
        let target = crate::semantic_layout_bridge::rustc_semantic_layout_target_v1(tcx)
            .map_err(|_| "complete body active target absent")?;
        if target.llvm_target() != "amdgcn-amd-amdhsa"
            || !target
                .has_exact_codegen_profile("gfx942", "-wavefrontsize32,+wavefrontsize64,-xnack")
        {
            return Err("complete body requires exact gfx942 xnack-off wave64");
        }
        let transcript = plan.canonical_transcript();
        if transcript.is_empty()
            || transcript.len() as u64 > HARD_MAX_CANONICAL_BYTES_V1
            || frontend.canonical_bytes().is_empty()
            || frontend.canonical_bytes().len() as u64 > HARD_MAX_CANONICAL_BYTES_V1
            || frontend
                .resource_canonical_bytes()
                .is_some_and(|bytes| bytes.len() as u64 > HARD_MAX_CANONICAL_BYTES_V1)
        {
            return Err("complete body current canonical source contract absent or oversized");
        }
        // Actual bytes remain borrowed through plan. These charges are separate
        // logical work; no new domain digest or old source identity is relabeled.
        work.charge(transcript.len())?;
        work.charge(frontend.canonical_bytes().len())?;
        work.charge(frontend.resource_canonical_bytes().map_or(0, <[u8]>::len))?;
        let actual_body = tcx.instance_mir(root.instance.def);
        let items = require_body_bounds(actual_body)?;
        work.charge(
            items
                .checked_mul(3)
                .ok_or("complete body source work overflow")?,
        )?;
        work.charge(FIXED_REPLAY_WORK)?;
        let actual = observe_root(tcx, root.instance, actual_body, recipe.block)?;
        if actual.instance() != terminal.instance
            || actual.packed() != parse_complete_body_consts(tcx, terminal.instance)?
        {
            return Err("complete body current call differs from authenticated terminal");
        }
        let body = bind_body(tcx, plan, root.instance, &actual, recipe.block)?;
        let callee_index = plan
            .function_producers()
            .len()
            .checked_add(recipe.terminal as usize)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or("complete body semantic callable index overflow")?;
        let callee = SemanticCallableIdV1::from_index(callee_index);
        Ok(Some(Self {
            plan,
            root: root.instance,
            body,
            slot: OneUse {
                key: Key {
                    caller: *root_id,
                    block: recipe.block,
                    callee,
                },
                pending: Some(CallBinding::from_actual(&actual)),
            },
        }))
    }

    /// A fresh body-producer observation must match an independent root replay.
    /// Refusal never consumes the pending slot; successful take is one-shot.
    pub(crate) fn take(
        &mut self,
        tcx: TyCtxt<'tcx>,
        caller: SemanticFunctionIdV1,
        block: u32,
        callee: SemanticCallableIdV1,
        actual: ActualCompleteBodyCallVNext<'tcx>,
        work: &mut CompleteBodySourceWorkVNext,
    ) -> Result<CompleteBodySourceVNext<'plan, 'tcx>, &'static str> {
        let key = Key {
            caller,
            block,
            callee,
        };
        let binding = CallBinding::from_actual(&actual);
        self.slot.check(key, &binding)?;
        let current_body = tcx.instance_mir(self.root.def);
        let items = require_body_bounds(current_body)?;
        work.charge(
            items
                .checked_mul(3)
                .ok_or("complete body source work overflow")?,
        )?;
        work.charge(FIXED_REPLAY_WORK)?;
        let replay = observe_root(tcx, self.root, current_body, block)?;
        if CallBinding::from_actual(&replay) != binding
            || bind_body(tcx, self.plan, self.root, &replay, block)? != self.body
        {
            return Err("complete body source changed or current replay differs");
        }
        let call = self.slot.take(key, &binding)?;
        Ok(CompleteBodySourceVNext {
            plan: self.plan,
            body: self.body,
            call,
            key,
        })
    }

    pub(crate) fn require_drained(&self) -> Result<(), &'static str> {
        self.slot.require_drained()
    }
}

fn bind_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    root: Instance<'tcx>,
    actual: &ActualCompleteBodyCallVNext<'tcx>,
    raw_block: u32,
) -> Result<BodyBinding, &'static str> {
    let [root_producer] = plan.function_producers() else {
        return Err("complete body root producer absent");
    };
    let [terminal] = plan.terminal_producers() else {
        return Err("complete body terminal producer absent");
    };
    let [retained] = plan.body_producers() else {
        return Err("complete body retained root body absent");
    };
    let root_identities = canonical_function_identities_v1(tcx, root);
    let terminal_identities = canonical_function_identities_v1(tcx, actual.instance());
    if root_producer.instance != root
        || root_producer.identities != root_identities
        || terminal.instance != actual.instance()
        || terminal.identities != terminal_identities
        || retained.function.index() != 0
    {
        return Err("complete body current full Instance identities differ");
    }
    let body = tcx.instance_mir(root.def);
    require_body_bounds(body)?;
    let mir_body = borrowed_rustc_mir_body_sha256_v1(tcx, root, body);
    if retained.blocks.len() != body.basic_blocks.len()
        || retained.raw_to_semantic_blocks.len() != body.basic_blocks.len()
    {
        return Err("complete body retained source block roster differs");
    }
    for (raw, semantic) in retained.raw_to_semantic_blocks.iter().enumerate() {
        let row = retained
            .blocks
            .get(semantic.index() as usize)
            .ok_or("complete body source block mapping leaves retained roster")?;
        if row.rustc_block != raw as u32
            || row.identity
                != rustc_block_identity_v1(root_identities.function(), mir_body, raw as u32)
        {
            return Err("complete body source block mapping is stale, lost or ambiguous");
        }
    }
    let block_identity = rustc_block_identity_v1(root_identities.function(), mir_body, raw_block);
    let semantic = retained
        .raw_to_semantic_blocks
        .get(raw_block as usize)
        .ok_or("complete body raw-to-semantic block association absent")?;
    let retained_block = retained
        .blocks
        .get(semantic.index() as usize)
        .ok_or("complete body retained semantic block absent")?;
    if retained_block.rustc_block != raw_block || retained_block.identity != block_identity {
        return Err("complete body source block identity or association differs");
    }
    let raw = body
        .basic_blocks
        .get(BasicBlock::from_u32(raw_block))
        .ok_or("complete body actual source block absent")?;
    let source =
        canonical_source_provenance_v1(tcx, raw.terminator().source_info.span, MAX_EXPANSION_DEPTH)
            .map_err(|_| "complete body source provenance unavailable or overbound")?;
    if source.provenance() != retained_block.terminator.provenance {
        return Err("complete body current source provenance differs from retained producer");
    }
    Ok(BodyBinding {
        root: root_identities,
        terminal: terminal_identities,
        mir_body,
        block_identity,
        source: source.provenance(),
        expansion_chain: source.expansion_chain_sha256(),
        expansion_depth: source.expansion_depth(),
    })
}

#[cfg(test)]
#[path = "production_complete_body_source_occurrences_vnext_tests.rs"]
mod tests;
