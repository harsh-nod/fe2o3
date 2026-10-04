//! Live-source composition admission; marker definitions are imported once,
//! while root-qualified execution occurrences remain a separate lowerer concern.
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_middle::mir::{BasicBlock, TerminatorKind};
use rustc_middle::ty::{Instance, TyCtxt};

use crate::collector::CollectedFunctionRole;
use crate::production_ordered_program_v32::{
    ActualOrderedProgramCallV32, MAX_PROFILE_BLOCKS, observe_call, parse_program_consts,
    require_unconditional_single_execution,
};
use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1;
use crate::rustc_semantic_adapter_v1::{canonical_function_identities_v1, domain_digest};
use crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1;

#[path = "production_ordered_composition_roster_v1.rs"]
mod roster;
#[path = "production_ordered_composition_shape_v1.rs"]
mod shape;
#[path = "production_ordered_composition_source_seed_v1.rs"]
mod source_seed;
#[path = "production_ordered_composition_transport_v1.rs"]
mod transport;
pub(crate) use source_seed::AuthenticatedOrderedCompositionSourceSeedV1;
use source_seed::OrderedCompositionSourceSeedV1;
#[cfg(target_os = "linux")]
#[path = "production_ordered_composition_publish_v1.rs"]
mod publish;
#[cfg(target_os = "linux")]
pub(crate) use publish::{
    OrderedCompositionSourcePublishEffectV1, OrderedCompositionSourcePublishErrorV1,
    OrderedCompositionSourcePublishRequestV1, OrderedCompositionTypedEditV1,
    PublishedOrderedCompositionSourceV1, publish_ordered_composition_source_v1,
    validate_ordered_composition_helper_name_v1,
};
use roster::{Kind, MAX_CALLS, MAX_FUNCTIONS, MAX_MARKERS, Marker, Roster, Row, Site};

const UNIT_DOMAIN: &[u8] = b"fe2o3/ordered-composition/compiler-observed-unit/v1";
const CONTRACT_DOMAIN: &[u8] = b"fe2o3/ordered-composition/root-contract/v1";
const STATEMENT_DOMAIN: &[u8] = b"fe2o3/ordered-composition/static-marker/v1";
const MAX_TYPES: usize = 4096;
const MAX_LOCALS: usize = 4096;
const MAX_STATEMENTS: usize = 65_536;

/// Mutually exclusive source custody. Existing singleton callers use exactly
/// the original factory, domains and consumption checks.
pub(crate) enum OrderedProgramSourcesV1<'tcx> {
    Singleton(crate::production_ordered_program_source_occurrences_v32::OrderedProgramSourceOccurrencesV32<'tcx>),
    Composition(OrderedCompositionSourcesV1<'tcx>),
    Completed,
}
/// Exact import completion; singleton callers cannot receive composition custody.
pub(crate) enum OrderedSourceImportCompletionV1<'tcx> {
    Singleton,
    Composition(AuthenticatedOrderedCompositionSourceSeedV1<'tcx>),
}
impl Default for OrderedProgramSourcesV1<'_> {
    fn default() -> Self {
        Self::Singleton(Default::default())
    }
}

/// Current actual call supplied by the body producer after ordinary callee and
/// local/block binding validation. No caller outside the backend can mint it.
pub(crate) struct ImportedOrderedCallV1<'a, 'tcx> {
    pub(crate) function: SemanticFunctionIdV1,
    pub(crate) raw_block: u32,
    pub(crate) block: SemanticBlockIdV1,
    pub(crate) block_identity: SemanticBlockIdentityV1,
    pub(crate) caller: Instance<'tcx>,
    pub(crate) callee: Instance<'tcx>,
    pub(crate) callable: SemanticCallableIdV1,
    pub(crate) marker: Option<ActualOrderedProgramCallV32<'tcx>>,
    pub(crate) arguments: &'a [SemanticOperandV1],
    pub(crate) destination: Option<&'a SemanticCallDestinationV1>,
    pub(crate) unwind: SemanticUnwindActionV1,
}
impl<'tcx> OrderedProgramSourcesV1<'tcx> {
    pub(crate) fn take(
        &mut self,
        actual: ImportedOrderedCallV1<'_, 'tcx>,
    ) -> Result<Option<SemanticOrderedProgramSourceV32>, &'static str> {
        match self {
            Self::Singleton(owner) => owner.take(
                actual.function,
                actual.raw_block,
                actual.callable,
                actual.marker,
            ),
            Self::Composition(owner) => owner.roster.take(roster::Observation {
                site: Site {
                    function: actual.function,
                    raw_block: actual.raw_block,
                    block: actual.block,
                    block_identity: actual.block_identity,
                },
                caller: actual.caller,
                callee: actual.callee,
                callable: actual.callable,
                marker: actual.marker.map(|marker| Marker {
                    program: marker.program(),
                    registers: marker.registers(),
                }),
                arguments: actual.arguments,
                destination: actual.destination,
                unwind: actual.unwind,
            }),
            Self::Completed => Err("ordered composition source custody was already completed"),
        }
    }
    pub(crate) fn require_drained(&self) -> Result<(), &'static str> {
        match self {
            Self::Singleton(owner) => owner.require_drained(),
            Self::Composition(owner) => owner.roster.require_drained(),
            Self::Completed => Err("ordered composition source custody was already completed"),
        }
    }
    pub(crate) fn complete(
        &mut self,
        semantic: &AdmittedInertSemanticMirV1,
    ) -> Result<OrderedSourceImportCompletionV1<'tcx>, &'static str> {
        self.require_drained()?;
        match std::mem::replace(self, Self::Completed) {
            Self::Singleton(_) => Ok(OrderedSourceImportCompletionV1::Singleton),
            Self::Composition(owner) => owner
                .seed
                .complete(semantic)
                .map(OrderedSourceImportCompletionV1::Composition),
            Self::Completed => Err("ordered composition source custody was already completed"),
        }
    }
}

/// Move-only bounded source-definition owner; not a canonical/profile/proof owner.
pub(crate) struct OrderedCompositionSourcesV1<'tcx> {
    roster: Roster<Instance<'tcx>>,
    seed: OrderedCompositionSourceSeedV1<'tcx>,
}

impl<'tcx> OrderedCompositionSourcesV1<'tcx> {
    /// Call only after charging validation_work() in the existing semantic body
    /// construction ledger. Type/ABI construction and rustc queries retain their
    /// existing independent limits; bounded row storage is not an RSS promise.
    pub(crate) fn from_plan(
        tcx: TyCtxt<'tcx>,
        plan: &ProductionSemanticPreflightPlanV1<'tcx>,
        types: &[SemanticTypeDeclV1],
        abis: &[SemanticFunctionAbiV1],
    ) -> Result<Self, &'static str> {
        let _ = Self::validation_work(plan)?;
        let functions = plan.function_producers();
        let [root_id] = plan.roots() else {
            return Err("ordered composition requires one authenticated kernel root");
        };
        let root_index = root_id.index() as usize;
        let root = functions
            .get(root_index)
            .filter(|root| root.role == CollectedFunctionRole::KernelEntry)
            .ok_or("ordered composition actual root role differs")?;
        if abis.len() != functions.len() || types.len() != plan.type_producers().len() {
            return Err("ordered composition actual type/ABI roster differs");
        }
        let frontend = root
            .frontend_contract
            .as_ref()
            .ok_or("ordered composition authenticated root contract missing")?;
        let launch = frontend
            .contract()
            .launch()
            .ok_or("ordered composition explicit source launch missing")?;
        if launch.required().map(|v| v.as_array()) != Some([64, 1, 1])
            || launch.maximum().map(|v| v.as_array()) != Some([64, 1, 1])
        {
            return Err(
                "ordered composition requires required and maximum 64x1x1 workgroup bounds",
            );
        }
        let target = crate::semantic_layout_bridge::rustc_semantic_layout_target_v1(tcx)
            .map_err(|_| "ordered composition active target is not admitted")?;
        if target.llvm_target() != "amdgcn-amd-amdhsa"
            || !target
                .has_exact_codegen_profile("gfx942", "-wavefrontsize32,+wavefrontsize64,-xnack")
        {
            return Err("ordered composition requires exact gfx942 xnack-off wave64");
        }
        for (index, function) in functions.iter().enumerate() {
            if canonical_function_identities_v1(tcx, function.instance) != function.identities {
                return Err("ordered composition actual function Instance identity differs");
            }
            if index != root_index {
                if function.role != CollectedFunctionRole::InternalHelper
                    || function.frontend_contract.is_some()
                    || function.export_name.is_some()
                    || !transport::helper_abi_is_direct(&abis[index], types)
                {
                    return Err("ordered composition helper requires exact Direct Rust u32 ABI");
                }
                transport::require_helper_signature(tcx, function.instance)?;
                shape::require_scalar_helper(
                    tcx,
                    plan,
                    SemanticFunctionIdV1::from_index(index as u32),
                )?;
            }
        }
        let unit = domain_digest(UNIT_DOMAIN, &[plan.canonical_transcript()]);
        let resource = frontend.resource_canonical_bytes();
        let contract = domain_digest(
            CONTRACT_DOMAIN,
            &[
                frontend.canonical_bytes(),
                &[u8::from(resource.is_some())],
                resource.unwrap_or_default(),
                &[1, 2, 1, 8, 8, 16, 128],
                root.identities.function().as_bytes(),
                b"gfx942:xnack-",
                &[64, 1, 1],
            ],
        );
        let mut roster = Roster::new(functions.len())?;
        let mut markers = [0_usize; MAX_FUNCTIONS];
        let mut steps = [0_usize; MAX_FUNCTIONS];
        for recipe in plan.terminal_expansion_producers() {
            if matches!(
                recipe.expansion,
                ProductionTerminalExpansionV1::Gfx942InlineU32(_)
                    | ProductionTerminalExpansionV1::Gfx942OrderedXorAddE32
                    | ProductionTerminalExpansionV1::Gfx942CompleteBodyE32
                    | ProductionTerminalExpansionV1::Gfx942PhysicalEntryBegin
                    | ProductionTerminalExpansionV1::Gfx942PhysicalEntryLabel
                    | ProductionTerminalExpansionV1::Gfx942PhysicalEntryStep
                    | ProductionTerminalExpansionV1::Gfx942PhysicalGlobalCopyBegin
                    | ProductionTerminalExpansionV1::Gfx942PhysicalGlobalCopyLabel
                    | ProductionTerminalExpansionV1::Gfx942PhysicalGlobalCopyStep
                    | ProductionTerminalExpansionV1::Gfx942PhysicalLdsExchangeBegin
                    | ProductionTerminalExpansionV1::Gfx942PhysicalLdsExchangeLabel
                    | ProductionTerminalExpansionV1::Gfx942PhysicalLdsExchangeStep
            ) {
                return Err("ordered composition mixes another assembly source family");
            }
            if recipe.expansion != ProductionTerminalExpansionV1::Gfx942OrderedProgramE32 {
                if recipe.caller != *root_id {
                    return Err("ordered composition helper contains another terminal family");
                }
                continue;
            }
            let caller_index = recipe.caller.index() as usize;
            let caller = functions
                .get(caller_index)
                .ok_or("ordered composition marker caller missing")?;
            let terminal = plan
                .terminal_producers()
                .get(recipe.terminal as usize)
                .ok_or("ordered composition marker terminal missing")?;
            if terminal.instance != recipe.instance
                || terminal.identities != recipe.identities
                || terminal.expansion != recipe.expansion
                || recipe.arguments != 8
                || canonical_function_identities_v1(tcx, terminal.instance) != terminal.identities
            {
                return Err("ordered composition authenticated marker recipe differs");
            }
            let body = tcx.instance_mir(caller.instance.def);
            require_unconditional_single_execution(body, recipe.block)?;
            let raw = body
                .basic_blocks
                .get(BasicBlock::from_u32(recipe.block))
                .ok_or("ordered composition marker raw block missing")?;
            let TerminatorKind::Call { func, args, .. } = &raw.terminator().kind else {
                return Err("ordered composition marker is not an actual call");
            };
            if args.len() != 8 {
                return Err("ordered composition marker arity differs");
            }
            let observed = observe_call(
                tcx,
                caller.instance,
                body,
                func,
                [
                    &args[0].node,
                    &args[1].node,
                    &args[2].node,
                    &args[3].node,
                    &args[4].node,
                    &args[5].node,
                    &args[6].node,
                    &args[7].node,
                ],
            )?;
            if observed.instance() != terminal.instance
                || observed.program() != parse_program_consts(tcx, terminal.instance)?
            {
                return Err("ordered composition marker Instance or program differs");
            }
            let (site, binding) =
                transport::capture_call(tcx, plan, recipe.caller, recipe.block, 8)?;
            if binding.callee != observed.instance()
                || binding.callable.index() as usize != functions.len() + recipe.terminal as usize
            {
                return Err("ordered composition marker semantic callee binding differs");
            }
            let registers = observed.registers();
            let [a, b, c] = registers.inputs();
            let roles = [registers.scratch(), registers.output(), a, b, c];
            let mut packed_bytes = [0_u8; 32];
            for (index, word) in observed.program().packed_words().iter().enumerate() {
                packed_bytes[index * 8..index * 8 + 8].copy_from_slice(&word.to_le_bytes());
            }
            let statement = domain_digest(
                STATEMENT_DOMAIN,
                &[
                    &unit,
                    &contract,
                    caller.identities.function().as_bytes(),
                    caller.identities.item_definition().as_bytes(),
                    caller.identities.monomorphization().as_bytes(),
                    caller.identities.generic_type_arguments().as_bytes(),
                    caller.identities.const_generic_arguments().as_bytes(),
                    &recipe.block.to_le_bytes(),
                    &site.block.index().to_le_bytes(),
                    site.block_identity.as_bytes(),
                    &[134, 0],
                    terminal.identities.function().as_bytes(),
                    terminal.identities.item_definition().as_bytes(),
                    terminal.identities.monomorphization().as_bytes(),
                    terminal.identities.generic_type_arguments().as_bytes(),
                    terminal.identities.const_generic_arguments().as_bytes(),
                    &roles,
                    &[observed.program().count()],
                    &packed_bytes,
                ],
            );
            let source = SemanticOrderedProgramSourceV32::new(
                unit,
                caller.identities.function(),
                contract,
                statement,
            )
            .map_err(|_| "ordered composition incomplete source identity")?;
            markers[caller_index] += 1;
            steps[caller_index] += usize::from(observed.program().count());
            roster.insert(Row {
                site,
                binding,
                consumed: false,
                kind: Kind::Marker {
                    actual: Marker {
                        program: observed.program(),
                        registers,
                    },
                    source,
                },
            })?;
        }
        let mut calls = [(0_usize, 0_usize); MAX_CALLS];
        for (index, recipe) in plan.direct_call_producers().iter().enumerate() {
            let caller = functions
                .get(recipe.caller.index() as usize)
                .ok_or("ordered composition Defined caller missing")?;
            let callee = functions
                .get(recipe.callee.index() as usize)
                .ok_or("ordered composition Defined callee missing")?;
            require_unconditional_single_execution(
                tcx.instance_mir(caller.instance.def),
                recipe.block,
            )?;
            let (site, binding) =
                transport::capture_call(tcx, plan, recipe.caller, recipe.block, 3)?;
            if binding.callee != callee.instance
                || binding.callable.index() != recipe.callee.index()
            {
                return Err("ordered composition Defined call Instance or semantic callee differs");
            }
            calls[index] = (
                recipe.caller.index() as usize,
                recipe.callee.index() as usize,
            );
            roster.insert(Row {
                site,
                binding,
                kind: Kind::Defined,
                consumed: false,
            })?;
        }
        roster::validate_census(
            root_index,
            functions.len(),
            &markers,
            &steps,
            &calls[..plan.direct_call_producers().len()],
        )?;
        let seed = OrderedCompositionSourceSeedV1::capture_precharged(tcx, plan)?;
        Ok(Self { roster, seed })
    }

    /// Conservative precharge for bounded source scans, per-site CFG replay,
    /// operand type-table probes and digest input bytes. Called before rustc body
    /// queries/roster allocation by the request owner. It is not heap accounting.
    pub(crate) fn validation_work(
        plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    ) -> Result<usize, &'static str> {
        let functions = plan.function_producers();
        if !(1..=MAX_FUNCTIONS).contains(&functions.len())
            || plan.body_producers().len() != functions.len()
            || plan.type_producers().len() > MAX_TYPES
            || plan.direct_call_producers().len() > MAX_CALLS
            || plan.terminal_expansion_producers().len() > MAX_FUNCTIONS * MAX_PROFILE_BLOCKS
            || plan.normalized_intrinsic_producers().len() > MAX_FUNCTIONS * MAX_PROFILE_BLOCKS
            || plan.canonical_transcript().is_empty()
            || plan.canonical_transcript().len() as u64 > HARD_MAX_CANONICAL_BYTES_V1
        {
            return Err("ordered composition source roster bounds exceeded");
        }
        let marker_count = plan
            .terminal_expansion_producers()
            .iter()
            .filter(|recipe| {
                recipe.expansion == ProductionTerminalExpansionV1::Gfx942OrderedProgramE32
            })
            .count();
        if !(1..=MAX_MARKERS).contains(&marker_count) {
            return Err("ordered composition requires one to eight static marker definitions");
        }
        for body in plan.body_producers() {
            if body.blocks.is_empty()
                || body.blocks.len() > MAX_PROFILE_BLOCKS
                || body.raw_to_semantic_blocks.len() != body.blocks.len()
                || body.locals.len() > MAX_LOCALS
                || body.raw_to_semantic_locals.len() != body.locals.len()
                || body
                    .blocks
                    .iter()
                    .try_fold(0_usize, |n, block| n.checked_add(block.statements.len()))
                    .is_none_or(|n| n > MAX_STATEMENTS)
            {
                return Err("ordered composition source body bounds exceeded");
            }
        }
        let mut bytes = plan.canonical_transcript().len();
        for function in functions {
            if let Some(frontend) = &function.frontend_contract {
                for input in [
                    frontend.canonical_bytes(),
                    frontend.resource_canonical_bytes().unwrap_or_default(),
                ] {
                    if input.len() as u64 > HARD_MAX_CANONICAL_BYTES_V1 {
                        return Err("ordered composition source contract bounds exceeded");
                    }
                    bytes = bytes
                        .checked_add(input.len())
                        .ok_or("ordered composition source work overflow")?;
                }
            }
        }
        // <=16 CFG traversals, each <=4096 blocks/16384 edges; <=128 operand
        // type probes across <=4096 rows; <=3*65536 scalar helper statements.
        // Twice that subtotal covers map joins/shape checks; fixed hash rows
        // and bounded metadata scans fit the remaining conservative margin.
        bytes
            .checked_add(4_000_000)
            .ok_or("ordered composition source work overflow")
    }
}
