//! Compiler-private occurrence ownership for the closed V30 inline-ISA profile.
//!
//! Unit identity commits the live compiler's canonical preflight transcript:
//! retained MIR bodies, source origins, root contracts and helper closure. It
//! is not a raw-source-content hash, toolchain receipt or whole-build cfg receipt.
//! Helpers have static call-site identities under one authenticated kernel root;
//! these records do not distinguish dynamic invocations of a shared helper.

use std::collections::{BTreeMap, BTreeSet};

use fe2o3_mir_model::semantic_mir_v1::{
    HARD_MAX_BLOCKS_V1, HARD_MAX_CALLABLES_V1, HARD_MAX_CANONICAL_BYTES_V1, HARD_MAX_FUNCTIONS_V1,
    HARD_MAX_ROOTS_V1, SemanticBlockIdentityV1, SemanticCallableIdV1, SemanticFunctionIdV1,
    SemanticFunctionIdentityV1, SemanticInlineAssemblySourceV30,
};

use crate::collector::CollectedFunctionRole;
use crate::production_inline_assembly_v30::{input_count, source_terminal_tag};
use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1;
use crate::rustc_semantic_adapter_v1::domain_digest;
use crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1;

const MAX_OCCURRENCES_V30: usize = 4096;
const UNIT_DOMAIN: &[u8] = b"fe2o3/semantic-mir/gfx942-inline/compiler-observed-unit/v30";
const CONTRACT_DOMAIN: &[u8] = b"fe2o3/semantic-mir/gfx942-inline/root-contract/v30";
const STATEMENT_DOMAIN: &[u8] = b"fe2o3/semantic-mir/gfx942-inline/static-occurrence/v30";
type OccurrenceKey = (SemanticFunctionIdV1, u32);

struct PendingOccurrenceV30 {
    callee: SemanticCallableIdV1,
    source: SemanticInlineAssemblySourceV30,
}

/// Move-only, non-serializable ownership built only from a compiler-private plan.
#[derive(Default)]
pub(crate) struct InlineSourceOccurrencesV30 {
    pending: BTreeMap<OccurrenceKey, PendingOccurrenceV30>,
    consumed: BTreeSet<OccurrenceKey>,
}

impl InlineSourceOccurrencesV30 {
    pub(crate) fn from_plan(
        plan: &ProductionSemanticPreflightPlanV1<'_>,
    ) -> Result<Self, &'static str> {
        let functions = plan.function_producers();
        let terminals = plan.terminal_producers();
        let recipes = plan.terminal_expansion_producers();
        require_bound(functions.len(), HARD_MAX_FUNCTIONS_V1)?;
        require_bound(plan.roots().len(), HARD_MAX_ROOTS_V1)?;
        require_bound(recipes.len(), HARD_MAX_BLOCKS_V1)?;
        require_bound(plan.body_producers().len(), HARD_MAX_FUNCTIONS_V1)?;
        require_bound(
            plan.canonical_transcript().len(),
            HARD_MAX_CANONICAL_BYTES_V1,
        )?;
        let callable_count = functions
            .len()
            .checked_add(terminals.len())
            .ok_or("inline source callable count overflow")?;
        require_bound(callable_count, HARD_MAX_CALLABLES_V1)?;
        if !recipes.iter().any(|recipe| {
            matches!(
                recipe.expansion,
                ProductionTerminalExpansionV1::Gfx942InlineU32(_)
            )
        }) {
            return Ok(Self::default());
        }
        let [root_id] = plan.roots() else {
            return Err("inline source requires exactly one authenticated kernel root");
        };
        let root = functions
            .get(root_id.index() as usize)
            .ok_or("inline source root identity missing")?;
        if root.role != CollectedFunctionRole::KernelEntry
            || functions.iter().enumerate().any(|(index, function)| {
                index != root_id.index() as usize
                    && function.role != CollectedFunctionRole::InternalHelper
            })
        {
            return Err("inline source requires one kernel root and internal helpers only");
        }
        let frontend = root
            .frontend_contract
            .as_ref()
            .ok_or("inline source root frontend contract missing")?;
        if plan.canonical_transcript().is_empty() || frontend.canonical_bytes().is_empty() {
            return Err("inline source canonical compiler input missing");
        }
        let unit = unit_identity(plan.canonical_transcript());
        let contract = contract_identity(
            frontend.canonical_bytes(),
            frontend.resource_canonical_bytes(),
        );
        let mut occurrences = Self::default();
        for recipe in recipes {
            let ProductionTerminalExpansionV1::Gfx942InlineU32(operation) = recipe.expansion else {
                continue;
            };
            let caller = functions
                .get(recipe.caller.index() as usize)
                .ok_or("inline source caller missing")?;
            let terminal = terminals
                .get(recipe.terminal as usize)
                .ok_or("inline source terminal missing")?;
            if terminal.instance != recipe.instance
                || terminal.identities != recipe.identities
                || terminal.expansion != recipe.expansion
                || recipe.arguments as usize != input_count(operation)
            {
                return Err("inline source terminal recipe mismatch");
            }
            let body = plan
                .body_producers()
                .get(recipe.caller.index() as usize)
                .ok_or("inline source caller body missing")?;
            if body.function != recipe.caller {
                return Err("inline source caller body mismatch");
            }
            require_bound(body.blocks.len(), HARD_MAX_BLOCKS_V1)?;
            require_bound(body.raw_to_semantic_blocks.len(), HARD_MAX_BLOCKS_V1)?;
            let semantic_block = body
                .raw_to_semantic_blocks
                .get(recipe.block as usize)
                .ok_or("inline source raw block missing")?;
            let block = body
                .blocks
                .get(semantic_block.index() as usize)
                .ok_or("inline source retained block missing")?;
            if block.rustc_block != recipe.block {
                return Err("inline source retained block mismatch");
            }
            let callable = functions
                .len()
                .checked_add(recipe.terminal as usize)
                .and_then(|index| u32::try_from(index).ok())
                .ok_or("inline source callable index overflow")?;
            let source = OccurrenceAxesV30 {
                caller: caller.identities.function(),
                raw_block: recipe.block,
                block: block.identity,
                opcode: source_terminal_tag(operation),
                callee: terminal.identities.function(),
                arguments: recipe.arguments,
            }
            .source(unit, contract)?;
            occurrences.insert(
                (recipe.caller, recipe.block),
                SemanticCallableIdV1::from_index(callable),
                source,
            )?;
        }
        Ok(occurrences)
    }

    /// Call only after the body constructor verifies the actual MIR call recipe.
    /// Unknown sites are non-assembly; consumed sites and wrong callees reject.
    pub(crate) fn take(
        &mut self,
        caller: SemanticFunctionIdV1,
        raw_block: u32,
        callee: SemanticCallableIdV1,
    ) -> Result<Option<SemanticInlineAssemblySourceV30>, &'static str> {
        let key = (caller, raw_block);
        if self.consumed.contains(&key) {
            return Err("inline source occurrence already consumed");
        }
        let Some(pending) = self.pending.get(&key) else {
            return Ok(None);
        };
        if pending.callee != callee {
            return Err("inline source occurrence callee mismatch");
        }
        let source = self
            .pending
            .remove(&key)
            .ok_or("inline source occurrence disappeared")?
            .source;
        self.consumed.insert(key);
        Ok(Some(source))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub(crate) fn require_drained(&self) -> Result<(), &'static str> {
        if self.is_empty() {
            Ok(())
        } else {
            Err("inline source occurrence was not consumed")
        }
    }

    fn insert(
        &mut self,
        key: OccurrenceKey,
        callee: SemanticCallableIdV1,
        source: SemanticInlineAssemblySourceV30,
    ) -> Result<(), &'static str> {
        if self.pending.contains_key(&key) || self.consumed.contains(&key) {
            return Err("duplicate inline source occurrence");
        }
        if self.pending.len() + self.consumed.len() >= MAX_OCCURRENCES_V30 {
            return Err("inline source occurrence limit exceeded");
        }
        self.pending
            .insert(key, PendingOccurrenceV30 { callee, source });
        Ok(())
    }
}

fn require_bound(actual: usize, maximum: u64) -> Result<(), &'static str> {
    if u64::try_from(actual).map_err(|_| "inline source table size overflow")? > maximum {
        Err("inline source plan table limit exceeded")
    } else {
        Ok(())
    }
}

fn unit_identity(canonical_plan: &[u8]) -> [u8; 32] {
    domain_digest(UNIT_DOMAIN, &[canonical_plan])
}

fn contract_identity(canonical_frontend: &[u8], resource: Option<&[u8]>) -> [u8; 32] {
    domain_digest(
        CONTRACT_DOMAIN,
        &[
            canonical_frontend,
            &[u8::from(resource.is_some())],
            resource.unwrap_or_default(),
        ],
    )
}

#[derive(Clone, Copy)]
struct OccurrenceAxesV30 {
    caller: SemanticFunctionIdentityV1,
    raw_block: u32,
    block: SemanticBlockIdentityV1,
    opcode: u8,
    callee: SemanticFunctionIdentityV1,
    arguments: u32,
}

impl OccurrenceAxesV30 {
    fn source(
        self,
        unit: [u8; 32],
        contract: [u8; 32],
    ) -> Result<SemanticInlineAssemblySourceV30, &'static str> {
        let statement = domain_digest(
            STATEMENT_DOMAIN,
            &[
                &unit,
                &contract,
                self.caller.as_bytes(),
                &self.raw_block.to_le_bytes(),
                self.block.as_bytes(),
                &[self.opcode],
                self.callee.as_bytes(),
                &self.arguments.to_le_bytes(),
            ],
        );
        SemanticInlineAssemblySourceV30::new(unit, self.caller, contract, statement)
            .map_err(|_| "inline source occurrence has an invalid identity")
    }
}

#[cfg(test)]
#[path = "production_inline_source_occurrences_v30_tests.rs"]
mod tests;
