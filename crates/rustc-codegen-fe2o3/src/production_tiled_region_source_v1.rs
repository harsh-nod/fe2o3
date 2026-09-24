//! Live importer sidecar for the opt-in PRE-RANKED BF16 relation.
//! No decoder, public selector, copied HIR tree, or source edit authority.
use crate::{
    collector::CollectedFunctionRole,
    production_semantic_terminal_v1::ProductionTerminalExpansionV1 as Expansion,
    rustc_semantic_adapter_v1::{
        CanonicalFunctionIdentitiesV1, borrowed_rustc_mir_body_sha256_v1,
        canonical_function_identities_v1, rustc_block_identity_v1,
    },
    rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1 as Plan,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_middle::{
    mir,
    ty::{Instance, TyCtxt},
};
use rustc_span::Span;

#[path = "production_tiled_region_file_v1.rs"]
mod file;
#[path = "production_tiled_region_hir_v1.rs"]
mod hir;
#[cfg(test)]
#[path = "production_tiled_region_source_v1_tests.rs"]
mod tests;
pub(crate) use fe2o3_lower_mir_kernel::ProductionBf16MfmaRoleV1 as Role;
pub(crate) use file::Bf16MfmaSourceFileObservationV1;
pub(crate) use hir::SourceOwnedBf16MfmaRegionV1;
type Result<T> = std::result::Result<T, &'static str>;
const ROLES: [Role; 6] = [
    Role::Context,
    Role::Lane,
    Role::Lhs,
    Role::Rhs,
    Role::Zero,
    Role::Result,
];
const CALLS: usize = 16;
const LOCALS: usize = 16;
// Sparse retained call/return mappings, not the complete source CFG.
const BLOCKS: usize = 16;
const ARGUMENTS: usize = 4;
// Declared finite P0 whole-root profile: actual direct source measured 20
// blocks. This is not a general tiled-program admission limit.
const WHOLE_ROOT_BLOCKS: usize = 32;
const WHOLE_ROOT_LOCALS: usize = 4096;
const WHOLE_ROOT_STATEMENTS: usize = 4096;
const VALIDATION_WORK_ALLOWANCE: usize = 4_000_000;
// At most 32 borrowed headers: count lookup, checked addition, result test,
// loop bookkeeping, plus fixed dimensional/total checks. This is a portion
// of the existing fixed allowance, never an additional or replacement meter.
const ROOT_SCAN_WORK_ENVELOPE: usize = 4 * WHOLE_ROOT_BLOCKS + 32;
const _: () = assert!(ROOT_SCAN_WORK_ENVELOPE <= VALIDATION_WORK_ALLOWANCE);

fn role(expansion: Expansion) -> Option<Role> {
    Some(match expansion {
        Expansion::MatrixContextCurrent => Role::Context,
        Expansion::WaveLaneCurrent => Role::Lane,
        Expansion::Bf16MatrixALoadZeroFilledV2 => Role::Lhs,
        Expansion::Bf16MatrixBLoadZeroFilledV2 => Role::Rhs,
        Expansion::F32MatrixAccumulatorZero => Role::Zero,
        Expansion::MatrixMultiplyAccumulate => Role::Result,
        _ => return None,
    })
}
fn role_index(role: Role) -> usize {
    match role {
        Role::Context => 0,
        Role::Lane => 1,
        Role::Lhs => 2,
        Role::Rhs => 3,
        Role::Zero => 4,
        Role::Result => 5,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LocalMap {
    raw: u32,
    semantic: SemanticLocalIdV1,
    identity: SemanticLocalIdentityV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BlockMap {
    raw: u32,
    semantic: SemanticBlockIdV1,
    identity: SemanticBlockIdentityV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OperandKey {
    Local {
        local: SemanticLocalIdV1,
        ty: SemanticTypeIdV1,
        moved: bool,
    },
    Scalar {
        ty: SemanticTypeIdV1,
        scalar: SemanticScalarValueV1,
    },
}
impl OperandKey {
    fn from_operand(value: &SemanticOperandV1) -> Result<Self> {
        match value {
            SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
                if place.projections().is_empty() =>
            {
                Ok(Self::Local {
                    local: place.local(),
                    ty: place.ty(),
                    moved: matches!(value, SemanticOperandV1::Move(_)),
                })
            }
            SemanticOperandV1::Constant(value) => match value.value() {
                SemanticConstantValueV1::Scalar(scalar) => Ok(Self::Scalar {
                    ty: value.ty(),
                    scalar: *scalar,
                }),
                _ => Err("BF16 source selected operand constant is unavailable"),
            },
            _ => Err("BF16 source selected operand projection is unavailable"),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CallTransport {
    arguments: [Option<OperandKey>; ARGUMENTS],
    destination: (
        SemanticLocalIdV1,
        SemanticTypeIdV1,
        SemanticControlFlowEdgeV1,
    ),
    unwind: SemanticUnwindActionV1,
}
impl CallTransport {
    fn capture(call: &SemanticDirectCallV1) -> Result<Self> {
        if call.arguments().len() > ARGUMENTS {
            return Err("BF16 source selected arity exceeds bound");
        }
        let mut arguments = [None; ARGUMENTS];
        for (slot, value) in arguments.iter_mut().zip(call.arguments()) {
            *slot = Some(OperandKey::from_operand(value)?);
        }
        let destination = call
            .destination()
            .ok_or("BF16 source selected call does not return")?;
        if !destination.place().projections().is_empty() {
            return Err("BF16 source selected destination is projected");
        }
        if !matches!(
            call.unwind(),
            SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
        ) {
            return Err("BF16 source selected call has executable unwind");
        }
        Ok(Self {
            arguments,
            destination: (
                destination.place().local(),
                destination.place().ty(),
                destination.edge(),
            ),
            unwind: call.unwind(),
        })
    }
}
struct CallRow<'tcx> {
    role: Role,
    site: BlockMap,
    callee: Instance<'tcx>,
    identities: CanonicalFunctionIdentitiesV1,
    expansion: Expansion,
    callable: SemanticCallableIdV1,
    abi: SemanticAbiIdentityV1,
    argument_count: usize,
    raw_arguments: [Option<LocalMap>; ARGUMENTS],
    raw_destination: LocalMap,
    raw_target: BlockMap,
    source_span: Span,
    function_span: Span,
    consumed: Option<CallTransport>,
}
struct Captured<'tcx> {
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    hir: &'tcx rustc_hir::Body<'tcx>,
    mir: &'tcx mir::Body<'tcx>,
    mir_sha256: [u8; 32],
    identities: CanonicalFunctionIdentitiesV1,
    abi: SemanticAbiIdentityV1,
    rows: [Option<CallRow<'tcx>>; CALLS],
    locals: [Option<LocalMap>; LOCALS],
    blocks: [Option<BlockMap>; BLOCKS],
}
/// Heap payload is fixed and fallibly allocated; ordinary disabled imports keep
/// a small handle, not a large unconditional stack/owner field.
pub(crate) struct PendingBf16MfmaSourceSeedV1<'tcx> {
    payload: Box<[Captured<'tcx>]>,
}
pub(crate) struct AuthenticatedBf16MfmaSourceSeedV1<'tcx> {
    captured: PendingBf16MfmaSourceSeedV1<'tcx>,
    semantic_sha256: [u8; 32],
}
#[derive(Default)]
pub(crate) enum Bf16MfmaImportCaptureV1<'tcx> {
    #[default]
    Disabled,
    Capturing(PendingBf16MfmaSourceSeedV1<'tcx>),
    Completed,
}
pub(crate) enum Bf16MfmaImportCompletionV1<'tcx> {
    Disabled,
    Inspected(AuthenticatedBf16MfmaSourceSeedV1<'tcx>),
}

fn insert_unique<T: Copy + Eq>(slots: &mut [Option<T>], value: T) -> Result<()> {
    if slots.iter().flatten().any(|old| *old == value) {
        return Ok(());
    }
    *slots
        .iter_mut()
        .find(|row| row.is_none())
        .ok_or("BF16 source mapping cap exceeded")? = Some(value);
    Ok(())
}
fn same_function(ids: CanonicalFunctionIdentitiesV1, f: &SemanticFunctionDeclV1) -> bool {
    ids.function() == f.identity()
        && ids.item_definition() == f.item_definition_identity()
        && ids.monomorphization() == f.monomorphization_identity()
        && ids.generic_type_arguments() == f.generic_type_arguments_identity()
        && ids.const_generic_arguments() == f.const_generic_arguments_identity()
}
// Constant-time dimensions only; no body/header traversal before prepayment.
fn validate_root_dimensions(
    blocks: usize,
    locals: usize,
    raw_block_map_rows: usize,
    raw_local_map_rows: usize,
) -> Result<()> {
    if blocks > WHOLE_ROOT_BLOCKS
        || locals > WHOLE_ROOT_LOCALS
        || raw_block_map_rows != blocks
        || raw_local_map_rows != locals
    {
        return Err("BF16 source body scan cap exceeded");
    }
    Ok(())
}

// Private counted-borrow helper. Production supplies only retained block
// header lengths after the original owner has charged ValidationWork.
fn checked_root_statement_count<T>(
    blocks: &[T],
    statement_count: impl Fn(&T) -> usize,
) -> Result<usize> {
    if blocks.len() > WHOLE_ROOT_BLOCKS {
        return Err("BF16 source body scan cap exceeded");
    }
    let count = blocks
        .iter()
        .try_fold(0usize, |n, block| n.checked_add(statement_count(block)))
        .ok_or("BF16 source body scan cap exceeded")?;
    if count > WHOLE_ROOT_STATEMENTS {
        return Err("BF16 source body scan cap exceeded");
    }
    Ok(count)
}

fn prepaid_validation_work(transcript_bytes: usize) -> Result<usize> {
    transcript_bytes
        .checked_add(VALIDATION_WORK_ALLOWANCE)
        .ok_or("BF16 source validation work overflow")
}

impl<'tcx> PendingBf16MfmaSourceSeedV1<'tcx> {
    /// Precharge in the ALREADY TRANSFERRED body-owner ValidationWork domain,
    /// before rustc queries/allocation. Not a second frontend/canonical meter.
    pub(crate) fn validation_work(plan: &Plan<'tcx>) -> Result<usize> {
        if plan.function_producers().len() != 1
            || plan.body_producers().len() != 1
            || plan.roots() != [SemanticFunctionIdV1::from_index(0)]
            || !plan.direct_call_producers().is_empty()
            || plan.terminal_expansion_producers().len() > 4096
            || plan.type_producers().len() > 4096
        {
            return Err("BF16 source requires one finite local root without helpers");
        }
        let body = &plan.body_producers()[0];
        validate_root_dimensions(
            body.blocks.len(),
            body.locals.len(),
            body.raw_to_semantic_blocks.len(),
            body.raw_to_semantic_locals.len(),
        )?;
        prepaid_validation_work(plan.canonical_transcript().len())
    }
    pub(crate) fn capture_precharged(tcx: TyCtxt<'tcx>, plan: &Plan<'tcx>) -> Result<Self> {
        // The sole caller has already debited transcript + 4M to the same
        // body owner's ValidationWork. Recheck dimensions without scanning,
        // then count borrowed headers exactly once, before rustc queries or
        // allocation. Refusal keeps that work consumed on the original owner.
        let _ = Self::validation_work(plan)?;
        let function = &plan.function_producers()[0];
        let body = &plan.body_producers()[0];
        checked_root_statement_count(&body.blocks, |block| block.statements.len())?;
        if function.role != CollectedFunctionRole::KernelEntry
            || !function.instance.args.is_empty()
            || canonical_function_identities_v1(tcx, function.instance) != function.identities
        {
            return Err("BF16 source root Instance differs");
        }
        let local = function
            .instance
            .def_id()
            .as_local()
            .ok_or("BF16 source root is not local")?;
        let hir = tcx
            .hir_maybe_body_owned_by(local)
            .ok_or("BF16 source root HIR is absent")?;
        let frontend = function
            .frontend_contract
            .as_ref()
            .ok_or("BF16 source root contract is absent")?;
        let launch = frontend
            .contract()
            .launch()
            .ok_or("BF16 source explicit launch is absent")?;
        if launch.required().map(|n| n.as_array()) != Some([64, 1, 1])
            || launch.maximum().map(|n| n.as_array()) != Some([64, 1, 1])
            || launch.max_grid().map(|n| n.as_array()) != Some([1, 1, 1])
        {
            return Err("BF16 source requires explicit WG64 and one workgroup");
        }
        let target = crate::semantic_layout_bridge::rustc_semantic_layout_target_v1(tcx)
            .map_err(|_| "BF16 source target is unavailable")?;
        if target.llvm_target() != "amdgcn-amd-amdhsa"
            || !target
                .has_exact_codegen_profile("gfx942", "-wavefrontsize32,+wavefrontsize64,-xnack")
        {
            return Err("BF16 source requires exact gfx942 xnack-off wave64");
        }
        let mir = tcx.instance_mir(function.instance.def);
        let mir_sha256 = borrowed_rustc_mir_body_sha256_v1(tcx, function.instance, mir);
        let map_local = |raw: u32| -> Result<LocalMap> {
            let semantic = *body
                .raw_to_semantic_locals
                .get(raw as usize)
                .ok_or("BF16 raw local is absent")?;
            let row = body
                .locals
                .get(semantic.index() as usize)
                .ok_or("BF16 semantic local is absent")?;
            if row.rustc_local != raw {
                return Err("BF16 local mapping differs");
            }
            Ok(LocalMap {
                raw,
                semantic,
                identity: row.identity,
            })
        };
        let map_block = |raw: u32| -> Result<BlockMap> {
            let semantic = *body
                .raw_to_semantic_blocks
                .get(raw as usize)
                .ok_or("BF16 raw block is absent")?;
            let row = body
                .blocks
                .get(semantic.index() as usize)
                .ok_or("BF16 semantic block is absent")?;
            if row.rustc_block != raw
                || row.identity
                    != rustc_block_identity_v1(function.identities.function(), mir_sha256, raw)
            {
                return Err("BF16 block mapping differs");
            }
            Ok(BlockMap {
                raw,
                semantic,
                identity: row.identity,
            })
        };
        let mut captured = Captured {
            tcx,
            instance: function.instance,
            hir,
            mir,
            mir_sha256,
            identities: function.identities,
            abi: plan.function_abi_producers()[0].identity,
            rows: [const { None }; CALLS],
            locals: [None; LOCALS],
            blocks: [None; BLOCKS],
        };
        for recipe in plan.terminal_expansion_producers() {
            let Some(selected) = role(recipe.expansion) else {
                continue;
            };
            let slot = role_index(selected);
            if captured.rows[slot].is_some() || recipe.caller.index() != 0 {
                return Err("BF16 source nominal producer is duplicated or foreign");
            }
            let terminal = plan
                .terminal_producers()
                .get(recipe.terminal as usize)
                .ok_or("BF16 terminal provider is absent")?;
            if recipe.instance != terminal.instance
                || recipe.identities != terminal.identities
                || recipe.expansion != terminal.expansion
                || canonical_function_identities_v1(tcx, terminal.instance) != terminal.identities
            {
                return Err("BF16 terminal provider identity differs");
            }
            let block = mir
                .basic_blocks
                .get(mir::BasicBlock::from_u32(recipe.block))
                .ok_or("BF16 actual raw block is absent")?;
            let mir::TerminatorKind::Call {
                func,
                args,
                destination,
                target: Some(target),
                fn_span,
                ..
            } = &block.terminator().kind
            else {
                return Err("BF16 producer is not a returning actual call");
            };
            if args.len() > ARGUMENTS
                || args.len() != recipe.arguments as usize
                || !destination.projection.is_empty()
                || crate::production_semantic_body_v1::resolve_direct_call_v1(
                    tcx,
                    function.instance,
                    mir,
                    func,
                )? != recipe.instance
            {
                return Err("BF16 actual call or Instance differs");
            }
            let mut raw_arguments = [None; ARGUMENTS];
            for (slot, arg) in raw_arguments.iter_mut().zip(args) {
                if let mir::Operand::Copy(place) | mir::Operand::Move(place) = &arg.node {
                    if !place.projection.is_empty() {
                        return Err("BF16 raw argument projection unavailable");
                    }
                    let mapped = map_local(place.local.as_u32())?;
                    insert_unique(&mut captured.locals, mapped)?;
                    *slot = Some(mapped);
                }
            }
            let site = map_block(recipe.block)?;
            let raw_destination = map_local(destination.local.as_u32())?;
            let raw_target = map_block(target.as_u32())?;
            insert_unique(&mut captured.blocks, site)?;
            insert_unique(&mut captured.blocks, raw_target)?;
            insert_unique(&mut captured.locals, raw_destination)?;
            captured.rows[role_index(selected)] = Some(CallRow {
                role: selected,
                site,
                callee: recipe.instance,
                identities: terminal.identities,
                expansion: recipe.expansion,
                callable: SemanticCallableIdV1::from_index(1 + recipe.terminal),
                abi: terminal.abi.identity,
                argument_count: args.len(),
                raw_arguments,
                raw_destination,
                raw_target,
                source_span: block.terminator().source_info.span,
                function_span: *fn_span,
                consumed: None,
            });
        }
        if captured.rows[..6].iter().any(Option::is_none) {
            return Err("BF16 source requires all six nominal producers");
        }
        let mut payload = Vec::new();
        payload
            .try_reserve_exact(1)
            .map_err(|_| "BF16 source seed allocation refused")?;
        if payload.capacity() != 1 {
            return Err("BF16 source seed allocation capacity differs");
        }
        payload.push(captured);
        Ok(Self {
            payload: payload.into_boxed_slice(),
        })
    }
}

impl<'tcx> Bf16MfmaImportCaptureV1<'tcx> {
    /// Called by the genuine body producer AFTER validated receiver restoration
    /// and semantic-call construction. Raw and rewritten semantic operand axes
    /// are retained separately; raw indices are never equated with semantic IDs.
    pub(crate) fn observe(
        &mut self,
        caller: Instance<'tcx>,
        body: &mir::Body<'tcx>,
        function: SemanticFunctionIdV1,
        raw_block: u32,
        semantic_block: SemanticBlockIdV1,
        block_identity: SemanticBlockIdentityV1,
        callee: Instance<'tcx>,
        expansion: Option<Expansion>,
        call: &SemanticDirectCallV1,
    ) -> Result<()> {
        let Self::Capturing(seed) = self else {
            return if matches!(self, Self::Disabled) {
                Ok(())
            } else {
                Err("BF16 source capture already completed")
            };
        };
        let Some(selected) = expansion.and_then(role) else {
            return Ok(());
        };
        let captured = &mut seed.payload[0];
        if caller != captured.instance || !std::ptr::eq(body, captured.mir) || function.index() != 0
        {
            return Err("BF16 imported body custody differs");
        }
        let row = captured.rows[role_index(selected)]
            .as_mut()
            .ok_or("BF16 imported selected call is absent")?;
        if row.site
            != (BlockMap {
                raw: raw_block,
                semantic: semantic_block,
                identity: block_identity,
            })
            || row.callee != callee
            || Some(row.expansion) != expansion
            || row.callable != call.callee()
            || row.argument_count != call.arguments().len()
            || row.consumed.is_some()
        {
            return Err("BF16 actual imported call relation differs");
        }
        let transport = CallTransport::capture(call)?;
        if transport.destination.0 != row.raw_destination.semantic
            || transport.destination.2.target() != row.raw_target.semantic
        {
            return Err("BF16 actual imported destination mapping differs");
        }
        row.consumed = Some(transport);
        Ok(())
    }
    pub(crate) fn complete(
        &mut self,
        semantic: &AdmittedInertSemanticMirV1,
    ) -> Result<Bf16MfmaImportCompletionV1<'tcx>> {
        match std::mem::replace(self, Self::Completed) {
            Self::Disabled => Ok(Bf16MfmaImportCompletionV1::Disabled),
            Self::Completed => Err("BF16 source capture completed twice"),
            Self::Capturing(seed) => {
                let captured = &seed.payload[0];
                let [function] = semantic.functions() else {
                    return Err("BF16 admitted source root roster differs");
                };
                if !same_function(captured.identities, function)
                    || function.abi().identity() != captured.abi
                    || semantic.roots() != [SemanticFunctionIdV1::from_index(0)]
                {
                    return Err("BF16 admitted source identity or ABI differs");
                }
                for row in captured.rows.iter().flatten() {
                    let block = function
                        .blocks()
                        .get(row.site.semantic.index() as usize)
                        .ok_or("BF16 admitted call block is absent")?;
                    let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                        return Err("BF16 admitted selected terminator differs");
                    };
                    if block.identity() != row.site.identity
                        || call.callee() != row.callable
                        || row.consumed != Some(CallTransport::capture(call)?)
                    {
                        return Err("BF16 completed actual call binding differs");
                    }
                    let callable = semantic
                        .callables()
                        .get(row.callable.index() as usize)
                        .ok_or("BF16 admitted callable is absent")?;
                    let SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } = callable else {
                        return Err("BF16 completed provider kind differs");
                    };
                    if binding.abi().identity() != row.abi
                        || binding.identity() != row.identities.function()
                    {
                        return Err("BF16 completed provider ABI differs");
                    }
                }
                for mapped in captured.locals.iter().flatten() {
                    if function
                        .locals()
                        .get(mapped.semantic.index() as usize)
                        .is_none_or(|local| local.identity() != mapped.identity)
                    {
                        return Err("BF16 completed local identity mapping differs");
                    }
                }
                Ok(Bf16MfmaImportCompletionV1::Inspected(
                    AuthenticatedBf16MfmaSourceSeedV1 {
                        captured: seed,
                        semantic_sha256: *semantic.semantic_sha256().as_bytes(),
                    },
                ))
            }
        }
    }
}
impl AuthenticatedBf16MfmaSourceSeedV1<'_> {
    pub(crate) const fn semantic_sha256(&self) -> &[u8; 32] {
        &self.semantic_sha256
    }
    pub(crate) fn retained_storage_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + std::mem::size_of::<Captured<'_>>() * self.captured.payload.len()
    }
}
