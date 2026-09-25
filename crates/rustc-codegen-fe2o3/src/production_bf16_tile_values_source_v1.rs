//! Separate live one-helper BF16 import custody; never a P0 fallback.
//! This seed proves source transport only. It does not authorize a Matrix splice.
use crate::{
    collector::CollectedFunctionRole,
    production_semantic_terminal_v1::ProductionTerminalExpansionV1 as Expansion,
    rustc_semantic_adapter_v1::{
        CanonicalFunctionIdentitiesV1, borrowed_rustc_mir_body_sha256_v1,
        canonical_function_identities_v1, rustc_block_identity_v1,
    },
    rustc_semantic_plan_v1::{
        ProductionSemanticPreflightPlanV1 as Plan, RetainedSemanticBodyProducerV1 as BodyPlan,
    },
};
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_middle::{
    mir,
    ty::{self, Instance, TyCtxt},
};
use rustc_span::Span;
#[path = "production_tiled_region_file_v1.rs"]
mod file;
#[path = "production_bf16_tile_values_hir_v1.rs"]
mod hir;
#[cfg(test)]
#[path = "production_bf16_tile_values_source_v1_tests.rs"]
mod tests;
pub(crate) use hir::SourceOwnedBf16TileValuesRegionV1;
type Result<T> = std::result::Result<T, &'static str>;
const FUNCTIONS: usize = 2;
const ROWS: usize = 8;
const ARGUMENTS: usize = 4;
const WHOLE_BLOCKS: usize = 32;
const WHOLE_LOCALS: usize = 4096;
const WHOLE_STATEMENTS: usize = 4096;
const VALIDATION_WORK: usize = 4_000_000;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Role {
    Context,
    Lane,
    Lhs,
    Rhs,
    Zero,
    Matrix,
    Values,
    HelperCall,
}
const ROLES: [Role; ROWS] = [
    Role::Context,
    Role::Lane,
    Role::Lhs,
    Role::Rhs,
    Role::Zero,
    Role::Matrix,
    Role::Values,
    Role::HelperCall,
];
fn role(expansion: Expansion) -> Option<Role> {
    Some(match expansion {
        Expansion::MatrixContextCurrent => Role::Context,
        Expansion::WaveLaneCurrent => Role::Lane,
        Expansion::Bf16MatrixALoadZeroFilledV2 => Role::Lhs,
        Expansion::Bf16MatrixBLoadZeroFilledV2 => Role::Rhs,
        Expansion::F32MatrixAccumulatorZero => Role::Zero,
        Expansion::MatrixMultiplyAccumulate => Role::Matrix,
        Expansion::F32MatrixAccumulatorIntoValues => Role::Values,
        _ => return None,
    })
}
fn slot(role: Role) -> usize {
    match role {
        Role::Context => 0,
        Role::Lane => 1,
        Role::Lhs => 2,
        Role::Rhs => 3,
        Role::Zero => 4,
        Role::Matrix => 5,
        Role::Values => 6,
        Role::HelperCall => 7,
    }
}
fn root_role(role: Role) -> bool {
    !matches!(role, Role::Matrix | Role::Values)
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
        value: SemanticScalarValueV1,
    },
}
impl OperandKey {
    fn capture(value: &SemanticOperandV1) -> Result<Self> {
        match value {
            SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p)
                if p.projections().is_empty() =>
            {
                Ok(Self::Local {
                    local: p.local(),
                    ty: p.ty(),
                    moved: matches!(value, SemanticOperandV1::Move(_)),
                })
            }
            SemanticOperandV1::Constant(c) => match c.value() {
                SemanticConstantValueV1::Scalar(v) => Ok(Self::Scalar {
                    ty: c.ty(),
                    value: *v,
                }),
                _ => Err("BF16 helper selected constant is not scalar"),
            },
            _ => Err("BF16 helper selected operand is projected"),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Transport {
    arguments: [Option<OperandKey>; ARGUMENTS],
    destination: (
        SemanticLocalIdV1,
        SemanticTypeIdV1,
        SemanticControlFlowEdgeV1,
    ),
    unwind: SemanticUnwindActionV1,
}
impl Transport {
    fn capture(call: &SemanticDirectCallV1) -> Result<Self> {
        if call.arguments().len() > ARGUMENTS {
            return Err("BF16 helper selected arity exceeds bound");
        }
        let mut arguments = [None; ARGUMENTS];
        for (to, from) in arguments.iter_mut().zip(call.arguments()) {
            *to = Some(OperandKey::capture(from)?);
        }
        let d = call
            .destination()
            .ok_or("BF16 helper selected call does not return")?;
        if !d.place().projections().is_empty()
            || !matches!(
                call.unwind(),
                SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
            )
        {
            return Err("BF16 helper projected destination or executable unwind");
        }
        Ok(Self {
            arguments,
            destination: (d.place().local(), d.place().ty(), d.edge()),
            unwind: call.unwind(),
        })
    }
}
struct CallRow<'tcx> {
    role: Role,
    function: SemanticFunctionIdV1,
    site: BlockMap,
    callee: Instance<'tcx>,
    identities: CanonicalFunctionIdentitiesV1,
    expansion: Option<Expansion>,
    callable: SemanticCallableIdV1,
    abi: SemanticAbiIdentityV1,
    raw_arguments: [Option<LocalMap>; ARGUMENTS],
    raw_moved: [Option<bool>; ARGUMENTS],
    arity: usize,
    destination: LocalMap,
    target: BlockMap,
    source_span: Span,
    function_span: Span,
    consumed: Option<Transport>,
}
// Named Captured deliberately permits reuse of the unchanged P0 source-file reader.
// Each record retains actual compiler borrows, not a reconstructed HIR or MIR body.
struct Captured<'tcx> {
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    hir: &'tcx rustc_hir::Body<'tcx>,
    mir: &'tcx mir::Body<'tcx>,
    mir_sha256: [u8; 32],
    identities: CanonicalFunctionIdentitiesV1,
    abi: SemanticAbiIdentityV1,
    fn_abi: &'tcx rustc_target::callconv::FnAbi<'tcx, ty::Ty<'tcx>>,
    source_inputs: [Option<ty::Ty<'tcx>>; ARGUMENTS],
    source_output: ty::Ty<'tcx>,
    signature_sha256: [u8; 32],
    fn_abi_sha256: [u8; 32],
}
struct Pair<'tcx> {
    functions: [Captured<'tcx>; FUNCTIONS],
    root: SemanticFunctionIdV1,
    helper: SemanticFunctionIdV1,
    rows: [Option<CallRow<'tcx>>; ROWS],
}
pub(crate) struct PendingBf16TileValuesSourceSeedV1<'tcx> {
    payload: Box<[Pair<'tcx>]>,
}
pub(crate) struct AuthenticatedBf16TileValuesSourceSeedV1<'tcx> {
    pending: PendingBf16TileValuesSourceSeedV1<'tcx>,
    semantic_sha256: [u8; 32],
}
#[derive(Default)]
pub(crate) enum Bf16TileValuesImportCaptureV1<'tcx> {
    #[default]
    Disabled,
    Capturing(PendingBf16TileValuesSourceSeedV1<'tcx>),
    Completed,
}
pub(crate) enum Bf16TileValuesImportCompletionV1<'tcx> {
    Disabled,
    Inspected(AuthenticatedBf16TileValuesSourceSeedV1<'tcx>),
}
fn same_function(ids: CanonicalFunctionIdentitiesV1, f: &SemanticFunctionDeclV1) -> bool {
    ids.function() == f.identity()
        && ids.item_definition() == f.item_definition_identity()
        && ids.monomorphization() == f.monomorphization_identity()
        && ids.generic_type_arguments() == f.generic_type_arguments_identity()
        && ids.const_generic_arguments() == f.const_generic_arguments_identity()
}
fn dimensions(
    functions: usize,
    bodies: usize,
    roots: usize,
    calls: usize,
    blocks: usize,
    locals: usize,
) -> Result<()> {
    if functions != 2
        || bodies != 2
        || roots != 1
        || calls != 1
        || blocks > WHOLE_BLOCKS
        || locals > WHOLE_LOCALS
    {
        return Err("BF16 helper requires exactly two finite bodies and one call");
    }
    Ok(())
}
fn map_local(body: &BodyPlan, raw: u32) -> Result<LocalMap> {
    let semantic = *body
        .raw_to_semantic_locals
        .get(raw as usize)
        .ok_or("BF16 helper raw local absent")?;
    let row = body
        .locals
        .get(semantic.index() as usize)
        .ok_or("BF16 helper semantic local absent")?;
    if row.rustc_local != raw {
        return Err("BF16 helper local mapping differs");
    }
    Ok(LocalMap {
        raw,
        semantic,
        identity: row.identity,
    })
}
fn map_block(body: &BodyPlan, f: &Captured<'_>, raw: u32) -> Result<BlockMap> {
    let semantic = *body
        .raw_to_semantic_blocks
        .get(raw as usize)
        .ok_or("BF16 helper raw block absent")?;
    let row = body
        .blocks
        .get(semantic.index() as usize)
        .ok_or("BF16 helper semantic block absent")?;
    if row.rustc_block != raw
        || row.identity != rustc_block_identity_v1(f.identities.function(), f.mir_sha256, raw)
    {
        return Err("BF16 helper block mapping differs");
    }
    Ok(BlockMap {
        raw,
        semantic,
        identity: row.identity,
    })
}
impl<'tcx> PendingBf16TileValuesSourceSeedV1<'tcx> {
    pub(crate) fn validation_work(plan: &Plan<'tcx>) -> Result<usize> {
        // Only constant-sized table headers before the existing body-owner debit.
        if plan.function_producers().len() != 2
            || plan.body_producers().len() != 2
            || plan.function_abi_producers().len() != 2
        {
            return Err("BF16 helper requires exactly two finite bodies and one call");
        }
        let [a, b] = plan.body_producers() else {
            return Err("BF16 helper body roster");
        };
        let blocks = a
            .blocks
            .len()
            .checked_add(b.blocks.len())
            .ok_or("BF16 helper block overflow")?;
        let locals = a
            .locals
            .len()
            .checked_add(b.locals.len())
            .ok_or("BF16 helper local overflow")?;
        dimensions(
            2,
            2,
            plan.roots().len(),
            plan.direct_call_producers().len(),
            blocks,
            locals,
        )?;
        for body in [a, b] {
            if body.raw_to_semantic_blocks.len() != body.blocks.len()
                || body.raw_to_semantic_locals.len() != body.locals.len()
            {
                return Err("BF16 helper raw map dimensions differ");
            }
        }
        if plan.terminal_expansion_producers().len() > 4096
            || plan.terminal_producers().len() > 4096
            || plan.type_producers().len() > 4096
        {
            return Err("BF16 helper provider census exceeds bound");
        }
        plan.canonical_transcript()
            .len()
            .checked_add(VALIDATION_WORK)
            .ok_or("BF16 helper validation work overflow")
    }
    pub(crate) fn capture_precharged(tcx: TyCtxt<'tcx>, plan: &Plan<'tcx>) -> Result<Self> {
        let _ = Self::validation_work(plan)?;
        let mut statements = 0usize;
        for body in plan.body_producers() {
            for block in &body.blocks {
                statements = statements
                    .checked_add(block.statements.len())
                    .ok_or("BF16 helper statement overflow")?;
                if statements > WHOLE_STATEMENTS {
                    return Err("BF16 helper statement cap");
                }
            }
        }
        let root = plan.roots()[0];
        let direct = plan.direct_call_producers()[0];
        let helper = direct.callee;
        if direct.caller != root || helper == root || root.index() >= 2 || helper.index() >= 2 {
            return Err("BF16 helper actual call/root roster differs");
        }
        let make = |index: usize| -> Result<Captured<'tcx>> {
            let f = &plan.function_producers()[index];
            let abi = &plan.function_abi_producers()[index];
            let body = &plan.body_producers()[index];
            if body.function.index() as usize != index
                || abi.function.index() as usize != index
                || canonical_function_identities_v1(tcx, f.instance) != f.identities
                || abi.source_inputs.len() != 4
            {
                return Err("BF16 helper function/ABI producer differs");
            }
            if (index == root.index() as usize && f.role != CollectedFunctionRole::KernelEntry)
                || (index == helper.index() as usize
                    && f.role != CollectedFunctionRole::InternalHelper)
            {
                return Err("BF16 helper compiler function role differs");
            }
            if f.instance.args.len() > 1
                || f.instance
                    .args
                    .iter()
                    .any(|a| !matches!(a.kind(), ty::GenericArgKind::Lifetime(_)))
            {
                return Err("BF16 helper type/const substitution unavailable");
            }
            let local = f
                .instance
                .def_id()
                .as_local()
                .ok_or("BF16 helper function must be local")?;
            let hir = tcx
                .hir_maybe_body_owned_by(local)
                .ok_or("BF16 helper HIR body absent")?;
            let mir = tcx.instance_mir(f.instance.def);
            if mir.arg_count != 4 {
                return Err("BF16 helper actual argument count differs");
            }
            let mut inputs = [None; 4];
            for (to, from) in inputs.iter_mut().zip(abi.source_inputs.iter()) {
                *to = Some(*from);
            }
            Ok(Captured {
                tcx,
                instance: f.instance,
                hir,
                mir,
                mir_sha256: borrowed_rustc_mir_body_sha256_v1(tcx, f.instance, mir),
                identities: f.identities,
                abi: abi.identity,
                fn_abi: abi.fn_abi,
                source_inputs: inputs,
                source_output: abi.source_output,
                signature_sha256: abi.rustc_source_signature_sha256,
                fn_abi_sha256: abi.rustc_fn_abi_sha256,
            })
        };
        let mut pair = Pair {
            functions: [make(0)?, make(1)?],
            root,
            helper,
            rows: [const { None }; ROWS],
        };
        let root_contract = plan.function_producers()[root.index() as usize]
            .frontend_contract
            .as_ref()
            .ok_or("BF16 helper root contract absent")?;
        let launch = root_contract
            .contract()
            .launch()
            .ok_or("BF16 helper explicit launch absent")?;
        if launch.required().map(|v| v.as_array()) != Some([64, 1, 1])
            || launch.maximum().map(|v| v.as_array()) != Some([64, 1, 1])
            || launch.max_grid().map(|v| v.as_array()) != Some([1, 1, 1])
        {
            return Err("BF16 helper requires explicit WG64 and one workgroup");
        }
        let target = crate::semantic_layout_bridge::rustc_semantic_layout_target_v1(tcx)
            .map_err(|_| "BF16 helper target unavailable")?;
        if target.llvm_target() != "amdgcn-amd-amdhsa"
            || !target
                .has_exact_codegen_profile("gfx942", "-wavefrontsize32,+wavefrontsize64,-xnack")
        {
            return Err("BF16 helper requires exact gfx942 xnack-off wave64");
        }
        for recipe in plan.terminal_expansion_producers() {
            let Some(selected) = role(recipe.expansion) else {
                continue;
            };
            let expected = if root_role(selected) { root } else { helper };
            if recipe.caller != expected || pair.rows[slot(selected)].is_some() {
                return Err("BF16 helper selected producer is foreign or repeated");
            }
            let terminal = plan
                .terminal_producers()
                .get(recipe.terminal as usize)
                .ok_or("BF16 helper terminal absent")?;
            if recipe.instance != terminal.instance
                || recipe.identities != terminal.identities
                || recipe.expansion != terminal.expansion
                || canonical_function_identities_v1(tcx, terminal.instance) != terminal.identities
            {
                return Err("BF16 helper terminal identity differs");
            }
            pair.rows[slot(selected)] = Some(capture_call(
                &pair,
                plan,
                selected,
                recipe.caller,
                recipe.block,
                terminal.instance,
                terminal.identities,
                Some(recipe.expansion),
                SemanticCallableIdV1::from_index(2 + recipe.terminal),
                terminal.abi.identity,
                recipe.arguments as usize,
            )?);
        }
        let f = &pair.functions[helper.index() as usize];
        pair.rows[7] = Some(capture_call(
            &pair,
            plan,
            Role::HelperCall,
            root,
            direct.block,
            f.instance,
            f.identities,
            None,
            SemanticCallableIdV1::from_index(helper.index()),
            f.abi,
            4,
        )?);
        if ROLES.iter().any(|role| pair.rows[slot(*role)].is_none()) {
            return Err("BF16 helper requires all seven terminals and the actual Defined call");
        }
        // Trust comes from exact retained terminal providers, not structural fragment layout.
        let terminal_for = |r: Role| -> Result<
            &crate::rustc_semantic_plan_v1::RetainedSemanticTerminalProducerV1<'tcx>,
        > {
            let row = pair.rows[slot(r)]
                .as_ref()
                .ok_or("BF16 helper role absent")?;
            plan.terminal_producers()
                .get((row.callable.index() - 2) as usize)
                .ok_or("BF16 helper terminal index")
        };
        let helper_abi = &plan.function_abi_producers()[helper.index() as usize];
        let mfma = &terminal_for(Role::Matrix)?.abi;
        let values = &terminal_for(Role::Values)?.abi;
        if mfma.source_inputs.len() != 4
            || values.source_inputs.len() != 1
            || helper_abi.source_inputs.as_ref() != mfma.source_inputs.as_ref()
            || helper_abi.source_inputs[1] != terminal_for(Role::Lhs)?.abi.source_output
            || helper_abi.source_inputs[2] != terminal_for(Role::Rhs)?.abi.source_output
            || helper_abi.source_inputs[3] != terminal_for(Role::Zero)?.abi.source_output
            || values.source_inputs[0] != mfma.source_output
            || helper_abi.source_output != values.source_output
        {
            return Err("BF16 helper nominal source signature differs from actual terminals");
        }
        let ty::Ref(_, matrix, mutability) = *helper_abi.source_inputs[0].kind() else {
            return Err("BF16 helper matrix is not a shared source borrow");
        };
        if mutability != mir::Mutability::Not
            || matrix != terminal_for(Role::Context)?.abi.source_output
        {
            return Err("BF16 helper matrix borrow source differs");
        }
        let mut payload = Vec::new();
        payload
            .try_reserve_exact(1)
            .map_err(|_| "BF16 helper source seed allocation refused")?;
        if payload.capacity() != 1 {
            return Err("BF16 helper source seed capacity differs");
        }
        payload.push(pair);
        Ok(Self {
            payload: payload.into_boxed_slice(),
        })
    }
}
fn capture_call<'tcx>(
    pair: &Pair<'tcx>,
    plan: &Plan<'tcx>,
    role: Role,
    function: SemanticFunctionIdV1,
    raw: u32,
    callee: Instance<'tcx>,
    identities: CanonicalFunctionIdentitiesV1,
    expansion: Option<Expansion>,
    callable: SemanticCallableIdV1,
    abi: SemanticAbiIdentityV1,
    arity: usize,
) -> Result<CallRow<'tcx>> {
    let f = &pair.functions[function.index() as usize];
    let body = &plan.body_producers()[function.index() as usize];
    let block = f
        .mir
        .basic_blocks
        .get(mir::BasicBlock::from_u32(raw))
        .ok_or("BF16 helper raw call block absent")?;
    let mir::TerminatorKind::Call {
        func,
        args,
        destination,
        target: Some(target),
        fn_span,
        ..
    } = &block.terminator().kind
    else {
        return Err("BF16 helper producer is not a returning raw call");
    };
    if arity > 4
        || args.len() != arity
        || !destination.projection.is_empty()
        || crate::production_semantic_body_v1::resolve_direct_call_v1(
            f.tcx, f.instance, f.mir, func,
        )? != callee
    {
        return Err("BF16 helper actual call Instance/shape differs");
    }
    let mut raw_arguments = [None; 4];
    let mut raw_moved = [None; 4];
    for (index, arg) in args.iter().enumerate() {
        if let mir::Operand::Copy(p) | mir::Operand::Move(p) = &arg.node {
            if !p.projection.is_empty() {
                return Err("BF16 helper raw argument projection unavailable");
            }
            raw_arguments[index] = Some(map_local(body, p.local.as_u32())?);
            raw_moved[index] = Some(matches!(&arg.node, mir::Operand::Move(_)));
        }
    }
    Ok(CallRow {
        role,
        function,
        site: map_block(body, f, raw)?,
        callee,
        identities,
        expansion,
        callable,
        abi,
        raw_arguments,
        raw_moved,
        arity,
        destination: map_local(body, destination.local.as_u32())?,
        target: map_block(body, f, target.as_u32())?,
        source_span: block.terminator().source_info.span,
        function_span: *fn_span,
        consumed: None,
    })
}
fn defined_argument(
    index: usize,
    expected_local: Option<SemanticLocalIdV1>,
    expected_move: Option<bool>,
    actual: Option<OperandKey>,
) -> Result<()> {
    if index >= ARGUMENTS {
        return Err("BF16 helper actual argument index exceeds bound");
    }
    let Some(OperandKey::Local { local, moved, .. }) = actual else {
        return Err("BF16 helper actual argument is not a whole local");
    };
    if expected_local != Some(local) || expected_move != Some(moved) || (index > 0 && !moved) {
        return Err("BF16 helper actual Move argument transport differs");
    }
    Ok(())
}
impl<'tcx> Bf16TileValuesImportCaptureV1<'tcx> {
    pub(crate) fn observe(
        &mut self,
        caller: Instance<'tcx>,
        body: &mir::Body<'tcx>,
        function: SemanticFunctionIdV1,
        raw: u32,
        semantic: SemanticBlockIdV1,
        identity: SemanticBlockIdentityV1,
        callee: Instance<'tcx>,
        expansion: Option<Expansion>,
        call: &SemanticDirectCallV1,
    ) -> Result<()> {
        let Self::Capturing(seed) = self else {
            return if matches!(self, Self::Disabled) {
                Ok(())
            } else {
                Err("BF16 helper source capture already completed")
            };
        };
        let p = &mut seed.payload[0];
        let selected =
            if expansion.is_none() && callee == p.functions[p.helper.index() as usize].instance {
                Some(Role::HelperCall)
            } else {
                expansion.and_then(role)
            };
        let Some(selected) = selected else {
            return Ok(());
        };
        let row = p.rows[slot(selected)]
            .as_mut()
            .ok_or("BF16 helper selected importer row absent")?;
        let f = &p.functions[row.function.index() as usize];
        if function != row.function
            || caller != f.instance
            || !std::ptr::eq(body, f.mir)
            || row.site
                != (BlockMap {
                    raw,
                    semantic,
                    identity,
                })
            || row.callee != callee
            || row.expansion != expansion
            || row.callable != call.callee()
            || row.arity != call.arguments().len()
            || row.consumed.is_some()
        {
            return Err("BF16 helper live imported call relation differs");
        }
        let transport = Transport::capture(call)?;
        if transport.destination.0 != row.destination.semantic
            || transport.destination.2.target() != row.target.semantic
        {
            return Err("BF16 helper live destination relation differs");
        }
        if selected == Role::HelperCall {
            for (index, arg) in transport.arguments.iter().enumerate() {
                defined_argument(
                    index,
                    row.raw_arguments[index].map(|p| p.semantic),
                    row.raw_moved[index],
                    *arg,
                )?;
            }
        }
        row.consumed = Some(transport);
        Ok(())
    }
    pub(crate) fn complete(
        &mut self,
        semantic: &AdmittedInertSemanticMirV1,
    ) -> Result<Bf16TileValuesImportCompletionV1<'tcx>> {
        match std::mem::replace(self, Self::Completed) {
            Self::Disabled => Ok(Bf16TileValuesImportCompletionV1::Disabled),
            Self::Completed => Err("BF16 helper source capture completed twice"),
            Self::Capturing(seed) => {
                let p = &seed.payload[0];
                if semantic.functions().len() != 2 || semantic.roots() != [p.root] {
                    return Err("BF16 helper admitted function roster differs");
                }
                for (index, f) in semantic.functions().iter().enumerate() {
                    let raw = &p.functions[index];
                    if !same_function(raw.identities, f) || f.abi().identity() != raw.abi {
                        return Err("BF16 helper admitted Instance/ABI differs");
                    }
                }
                for row in p.rows.iter().flatten() {
                    let f = &semantic.functions()[row.function.index() as usize];
                    let block = f
                        .blocks()
                        .get(row.site.semantic.index() as usize)
                        .ok_or("BF16 helper admitted block absent")?;
                    let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                        return Err("BF16 helper admitted terminator differs");
                    };
                    if block.identity() != row.site.identity
                        || call.callee() != row.callable
                        || row.consumed != Some(Transport::capture(call)?)
                    {
                        return Err("BF16 helper completed call transport differs");
                    }
                    let callable = semantic
                        .callables()
                        .get(row.callable.index() as usize)
                        .ok_or("BF16 helper callable absent")?;
                    match callable {
                        SemanticCallableDeclV1::Defined { function }
                            if row.role == Role::HelperCall && *function == p.helper => {}
                        SemanticCallableDeclV1::CompilerIntrinsic { binding, .. }
                            if row.role != Role::HelperCall
                                && binding.identity() == row.identities.function()
                                && binding.abi().identity() == row.abi => {}
                        _ => return Err("BF16 helper completed callable kind/ABI differs"),
                    }
                    for mapped in row
                        .raw_arguments
                        .iter()
                        .flatten()
                        .chain(std::iter::once(&row.destination))
                    {
                        if f.locals()
                            .get(mapped.semantic.index() as usize)
                            .is_none_or(|local| local.identity() != mapped.identity)
                        {
                            return Err("BF16 helper completed local provenance differs");
                        }
                    }
                }
                let abi = semantic.functions()[p.helper.index() as usize].abi();
                if abi.source_argument_ownership()
                    != [
                        SemanticSourceArgumentOwnershipV1::SharedBorrow,
                        SemanticSourceArgumentOwnershipV1::ByValue,
                        SemanticSourceArgumentOwnershipV1::ByValue,
                        SemanticSourceArgumentOwnershipV1::ByValue,
                    ]
                {
                    return Err("BF16 helper source ownership differs");
                }
                let output = semantic
                    .types()
                    .get(abi.source_output_type().index() as usize)
                    .ok_or("BF16 helper output type absent")?;
                let SemanticTypeShapeV1::Array { element, length: 4 } = output.shape() else {
                    return Err("BF16 helper return is not four ordinary values");
                };
                if !matches!(
                    semantic
                        .types()
                        .get(element.index() as usize)
                        .map(|t| t.shape()),
                    Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float {
                        bits: 32
                    }))
                ) {
                    return Err("BF16 helper return component differs");
                }
                Ok(Bf16TileValuesImportCompletionV1::Inspected(
                    AuthenticatedBf16TileValuesSourceSeedV1 {
                        pending: seed,
                        semantic_sha256: *semantic.semantic_sha256().as_bytes(),
                    },
                ))
            }
        }
    }
}
impl AuthenticatedBf16TileValuesSourceSeedV1<'_> {
    pub(crate) const fn semantic_sha256(&self) -> &[u8; 32] {
        &self.semantic_sha256
    }
    pub(crate) fn retained_storage_bytes(&self) -> usize {
        std::mem::size_of::<Self>() + std::mem::size_of::<Pair<'_>>() * self.pending.payload.len()
    }
}
