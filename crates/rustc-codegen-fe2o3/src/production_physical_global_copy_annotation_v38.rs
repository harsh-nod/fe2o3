//! One-use live-source annotation transport. The importer retains the actual
//! preflight transaction; no decoded MIR record can construct this owner.
use crate::collector::CollectedFunctionRole;
use crate::production_physical_global_copy_call_v38::{
    ActualPhysicalGlobalCopyCallV38, is_physical,
};
use crate::production_physical_global_copy_census_v38::{observe_root, require_body_bounds};
use crate::rustc_semantic_adapter_v1::{
    borrowed_rustc_mir_body_sha256_v1, canonical_function_identities_v1,
    canonical_source_provenance_v1, rustc_block_identity_v1,
};
use crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_middle::ty::{Instance, TyCtxt};
use sha2::{Digest, Sha256};

const MAX: usize = SEMANTIC_PHYSICAL_GLOBAL_COPY_MAX_OCCURRENCES_V38;
/// Prepaid on the existing cumulative semantic-construction ledger, not a
/// reset or a compiler/RSS account. All added records use fixed stack arrays.
pub(crate) const PREPAID_CONSTRUCTION_WORK_V38: usize = 2_200_000;
const MAX_EXPANSION_DEPTH: usize = 256;

#[derive(Clone, Copy)]
struct Slot<'tcx> {
    raw_block: u32,
    semantic_block: SemanticBlockIdV1,
    block_identity: SemanticBlockIdentityV1,
    callee: SemanticCallableIdV1,
    occurrence: u8,
    actual: ActualPhysicalGlobalCopyCallV38<'tcx>,
    semantic_locals: [SemanticLocalIdV1; 2],
}
struct Common {
    axes: [[u8; 32]; 5],
    body: [u8; 32],
    signature: [u8; 32],
    abi: [u8; 32],
    frontend: [u8; 32],
}
pub(crate) struct PhysicalGlobalCopyAnnotationsV38<'tcx> {
    root: Instance<'tcx>,
    function: SemanticFunctionIdV1,
    common: Common,
    slots: [Option<Slot<'tcx>>; MAX],
    count: usize,
}

pub(crate) fn prepare<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    budget: &mut Budget<'_>,
) -> Result<PhysicalGlobalCopyAnnotationsV38<'tcx>, String> {
    // Charge fixed tables and the bounded cross-roster comparisons up front.
    budget
        .charge_work(2 * 4096 + MAX * MAX + MAX * (2 * MAX_EXPANSION_DEPTH + 128))
        .map_err(|e| e.to_string())?;
    let [function] = plan.roots() else {
        return Err("physical-global-copy requires one root".into());
    };
    let [root] = plan.function_producers() else {
        return Err("physical-global-copy excludes helpers".into());
    };
    let [retained] = plan.body_producers() else {
        return Err("physical-global-copy retained body absent".into());
    };
    if function.index() != 0
        || retained.function != *function
        || root.role != CollectedFunctionRole::KernelEntry
        || !plan.direct_call_producers().is_empty()
        || !plan.normalized_intrinsic_producers().is_empty()
    {
        return Err("physical-global-copy actual root/call roster differs".into());
    }
    let recipes = plan.terminal_expansion_producers();
    let terminals = plan.terminal_producers();
    if recipes.is_empty()
        || recipes.len() > MAX
        || terminals.is_empty()
        || terminals.len() > MAX
        || recipes.iter().any(|r| !is_physical(r.expansion))
        || terminals.iter().any(|t| !is_physical(t.expansion))
    {
        return Err(
            "physical-global-copy terminal roster contains a foreign or excessive marker".into(),
        );
    }
    let target = crate::semantic_layout_bridge::rustc_semantic_layout_target_v1(tcx)
        .map_err(|_| "physical-global-copy active target absent")?;
    if target.llvm_target() != "amdgcn-amd-amdhsa"
        || !target.has_exact_codegen_profile("gfx942", "-wavefrontsize32,+wavefrontsize64,-xnack")
    {
        return Err("physical-global-copy requires exact gfx942 xnack-off Wave64".into());
    }
    let frontend = root
        .frontend_contract
        .as_ref()
        .ok_or("physical-global-copy frontend contract absent")?;
    let launch = frontend
        .contract()
        .launch()
        .ok_or("physical-global-copy explicit source launch absent")?;
    if launch.required().map(|n| n.as_array()) != Some([64, 1, 1])
        || launch.maximum().map(|n| n.as_array()) != Some([64, 1, 1])
        || launch.max_grid().map(|n| n.as_array()) != Some([2, 1, 1])
    {
        return Err("physical-global-copy requires exact authored launch64 and max_grid2".into());
    }
    if frontend.resource_contract().is_some_and(|r| {
        r.static_shared_memory_bytes() != 0 || r.max_dynamic_shared_memory_bytes() != 0
    }) {
        return Err("physical-global-copy source cannot declare workgroup storage".into());
    }
    let export = root
        .export_name
        .as_deref()
        .ok_or("physical-global-copy source export absent")?;
    if export.is_empty()
        || export.len() > 128
        || !export
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i != 0 && b.is_ascii_digit()))
    {
        return Err(
            "physical-global-copy export is outside the closed ASCII symbol grammar".into(),
        );
    }
    let transcript = plan.canonical_transcript();
    if transcript.is_empty()
        || frontend.canonical_bytes().is_empty()
        || transcript.len() as u64 > HARD_MAX_CANONICAL_BYTES_V1
        || frontend.canonical_bytes().len() as u64 > HARD_MAX_CANONICAL_BYTES_V1
        || frontend
            .resource_canonical_bytes()
            .is_some_and(|b| b.len() as u64 > HARD_MAX_CANONICAL_BYTES_V1)
    {
        return Err("physical-global-copy current source bytes absent or oversized".into());
    }
    for amount in [
        transcript.len(),
        frontend.canonical_bytes().len(),
        frontend.resource_canonical_bytes().map_or(0, <[u8]>::len),
    ] {
        budget.charge_work(amount).map_err(|e| e.to_string())?;
    }
    let body = tcx.instance_mir(root.instance.def);
    let items = require_body_bounds(body).map_err(str::to_owned)?;
    budget
        .charge_work(
            items
                .checked_mul(4)
                .ok_or("physical-global-copy source work overflow")?,
        )
        .map_err(|e| e.to_string())?;
    let census = observe_root(tcx, root.instance, body).map_err(str::to_owned)?;
    if census.len() != recipes.len() {
        return Err(
            "physical-global-copy retained recipes do not cover the complete source census".into(),
        );
    }
    let identities = canonical_function_identities_v1(tcx, root.instance);
    if identities != root.identities {
        return Err("physical-global-copy current full root Instance identity differs".into());
    }
    let body_hash = borrowed_rustc_mir_body_sha256_v1(tcx, root.instance, body);
    if retained.blocks.len() != body.basic_blocks.len()
        || retained.raw_to_semantic_blocks.len() != body.basic_blocks.len()
        || retained.locals.len() != body.local_decls.len()
        || retained.raw_to_semantic_locals.len() != body.local_decls.len()
    {
        return Err("physical-global-copy retained body coordinate census differs".into());
    }
    for (raw, semantic) in retained.raw_to_semantic_blocks.iter().enumerate() {
        let row = retained
            .blocks
            .get(semantic.index() as usize)
            .ok_or("physical-global-copy block mapping leaves roster")?;
        if row.rustc_block != raw as u32
            || row.identity != rustc_block_identity_v1(identities.function(), body_hash, raw as u32)
        {
            return Err(
                "physical-global-copy current raw-to-semantic block identity differs".into(),
            );
        }
    }
    for (raw, semantic) in retained.raw_to_semantic_locals.iter().enumerate() {
        if retained
            .locals
            .get(semantic.index() as usize)
            .is_none_or(|row| row.rustc_local != raw as u32)
        {
            return Err("physical-global-copy raw-to-semantic local mapping differs".into());
        }
    }
    let (signature, abi) = crate::production_physical_global_copy_source_abi_v38::check(plan)
        .map_err(str::to_owned)?;
    let common = Common {
        axes: [
            *identities.function().as_bytes(),
            *identities.item_definition().as_bytes(),
            *identities.monomorphization().as_bytes(),
            *identities.generic_type_arguments().as_bytes(),
            *identities.const_generic_arguments().as_bytes(),
        ],
        body: body_hash,
        signature,
        abi,
        frontend: Sha256::digest(frontend.canonical_bytes()).into(),
    };
    let mut slots = [None; MAX];
    let mut used_terminals = [false; MAX];
    let mut used_recipes = [false; MAX];
    for (occurrence, observed) in census.calls().enumerate() {
        let mut matching = recipes
            .iter()
            .enumerate()
            .filter(|(_, r)| r.caller == *function && r.block == observed.raw_block);
        let (recipe_index, recipe) = matching
            .next()
            .ok_or("physical-global-copy current occurrence recipe absent")?;
        if matching.next().is_some() || used_recipes[recipe_index] {
            return Err("physical-global-copy source occurrence recipe is ambiguous".into());
        }
        let terminal = terminals
            .get(recipe.terminal as usize)
            .ok_or("physical-global-copy source terminal association absent")?;
        let actual = observed.actual;
        let actual_identities = canonical_function_identities_v1(tcx, actual.instance());
        let count = if matches!(
            actual.operation(),
            SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyBegin
        ) {
            2
        } else {
            0
        };
        if recipe.instance != actual.instance()
            || terminal.instance != actual.instance()
            || recipe.identities != actual_identities
            || terminal.identities != actual_identities
            || recipe.expansion != terminal.expansion
            || recipe.arguments != count
        {
            return Err(
                "physical-global-copy current exact callee/const/ABI occurrence differs".into(),
            );
        }
        used_recipes[recipe_index] = true;
        used_terminals[recipe.terminal as usize] = true;
        let semantic_block = retained.raw_to_semantic_blocks[observed.raw_block as usize];
        let row = &retained.blocks[semantic_block.index() as usize];
        let raw = &body.basic_blocks[rustc_middle::mir::BasicBlock::from_u32(observed.raw_block)];
        let source = canonical_source_provenance_v1(
            tcx,
            raw.terminator().source_info.span,
            MAX_EXPANSION_DEPTH,
        )
        .map_err(|_| "physical-global-copy source provenance is unavailable or overbound")?;
        if source.provenance() != row.terminator.provenance {
            return Err(
                "physical-global-copy retained source occurrence provenance differs".into(),
            );
        }
        let mut semantic_locals = [SemanticLocalIdV1::from_index(0); 2];
        if count == 2 {
            for (index, raw) in actual.argument_locals().into_iter().enumerate() {
                semantic_locals[index] = *retained
                    .raw_to_semantic_locals
                    .get(raw as usize)
                    .ok_or("physical-global-copy actual begin local mapping absent")?;
            }
        }
        let callee = recipe
            .terminal
            .checked_add(1)
            .map(SemanticCallableIdV1::from_index)
            .ok_or("physical-global-copy callable index overflow")?;
        slots[occurrence] = Some(Slot {
            raw_block: observed.raw_block,
            semantic_block,
            block_identity: row.identity,
            callee,
            occurrence: occurrence as u8,
            actual,
            semantic_locals,
        });
    }
    if used_recipes[..recipes.len()].iter().any(|v| !*v)
        || used_terminals[..terminals.len()].iter().any(|v| !*v)
    {
        return Err("physical-global-copy contains an unconsumed terminal or call recipe".into());
    }
    Ok(PhysicalGlobalCopyAnnotationsV38 {
        root: root.instance,
        function: *function,
        common,
        slots,
        count: census.len(),
    })
}

impl<'tcx> PhysicalGlobalCopyAnnotationsV38<'tcx> {
    /// The actual call is freshly observed by the normal body producer.
    /// Semantic block traversal may be identity-sorted, not source-order; every
    /// prepared source occurrence nevertheless must be consumed exactly once.
    pub(crate) fn attach(
        &mut self,
        current_root: Instance<'tcx>,
        key: (SemanticFunctionIdV1, u32, SemanticCallableIdV1),
        semantic_block: SemanticBlockIdV1,
        actual: ActualPhysicalGlobalCopyCallV38<'tcx>,
        call: SemanticDirectCallV1,
    ) -> Result<SemanticDirectCallV1, &'static str> {
        let index = self.slots[..self.count]
            .iter()
            .position(|slot| slot.is_some_and(|s| s.raw_block == key.1))
            .ok_or("physical-global-copy source occurrence missing or already consumed")?;
        let slot = self.slots[index]
            .as_ref()
            .ok_or("physical-global-copy source occurrence disappeared")?;
        let count = if matches!(
            actual.operation(),
            SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyBegin
        ) {
            2
        } else {
            0
        };
        if current_root != self.root
            || key.0 != self.function
            || key.2 != slot.callee
            || call.callee() != slot.callee
            || semantic_block != slot.semantic_block
            || actual.instance() != slot.actual.instance()
            || actual.operation() != slot.actual.operation()
            || actual.argument_locals() != slot.actual.argument_locals()
            || actual.moved_arguments() != slot.actual.moved_arguments()
            || call.arguments().len() != count
            || !call.variadic_argument_abis().is_empty()
            || call.inline_assembly_source_v30().is_some()
            || call.ordered_region_source_v31().is_some()
            || call.ordered_program_source_v32().is_some()
            || call.complete_body_source_vnext().is_some()
            || call.physical_global_copy_source_v38().is_some()
            || call.physical_entry_source_v37().is_some()
        {
            return Err("physical-global-copy current raw/semantic occurrence join differs");
        }
        for (index, operand) in call.arguments().iter().enumerate() {
            let (place, moved) = match operand {
                SemanticOperandV1::Copy(p) => (p, false),
                SemanticOperandV1::Move(p) => (p, true),
                _ => {
                    return Err("physical-global-copy emitted begin input is not direct transport");
                }
            };
            if !place.projections().is_empty()
                || place.local() != slot.semantic_locals[index]
                || moved != (actual.moved_arguments() & (1 << index) != 0)
            {
                return Err(
                    "physical-global-copy emitted begin input changed identity or move mode",
                );
            }
        }
        let source = SemanticPhysicalGlobalCopySourceV38::new(
            self.common.axes,
            self.common.body,
            *slot.block_identity.as_bytes(),
            self.common.signature,
            self.common.abi,
            self.common.frontend,
            (slot.raw_block, slot.occurrence),
        )?;
        self.slots[index] = None;
        Ok(call.with_physical_global_copy_source_v38(source))
    }
    pub(crate) fn require_drained(&self) -> Result<(), &'static str> {
        if self.slots[..self.count].iter().any(Option::is_some) {
            Err("physical-global-copy source occurrence was not consumed")
        } else {
            Ok(())
        }
    }
}
