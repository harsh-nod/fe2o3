//! Read-only primitive Option comparisons may materialize their exact local
//! promotion. No pointer value, comparison result, or call graph is summarized.

use super::{Operation, contract, normalized_ty, reviewed_body, shared};
use rustc_abi::ExternAbi;
use rustc_data_structures::fingerprint::Fingerprint;
use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_hir::{Mutability, Safety, def::DefKind};
use rustc_middle::mir::visit::{
    MutatingUseContext, NonMutatingUseContext, NonUseContext, PlaceContext, Visitor,
};
use rustc_middle::mir::{
    AggregateKind, BasicBlock, Body, BorrowKind, Const, ConstValue, Local, LocalDecl, Location,
    Operand, Place, ProjectionElem, Promoted, Rvalue, Statement, StatementKind, Terminator,
    TerminatorKind, UnwindAction,
};
use rustc_middle::ty::{self, EarlyBinder, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypingEnv};

const MAX_LOCALS: usize = 4096;
const MAX_BLOCKS: usize = 4096;
const MAX_STATEMENTS: usize = 16384;
const MAX_PROMOTIONS: usize = 16;
const MAX_WORK: usize = 65536;

#[derive(Debug)]
struct Materialization<'tcx> {
    block: BasicBlock,
    statement: usize,
    reference: Local,
    reborrow: Option<Local>,
    promotion: Promoted,
    value_ty: Ty<'tcx>,
    initializer: Rvalue<'tcx>,
    dependency_fingerprint: [u8; 16],
}

/// Only this module can issue an expansion; the original caller, promotion,
/// comparison bodies, ABI, and resulting MIR participate in its commitment.
#[derive(Debug)]
pub(crate) struct ReviewedPromotedOptionCompareV1<'tcx> {
    instance: Instance<'tcx>,
    source_fingerprint: [u8; 16],
    materializations: Vec<Materialization<'tcx>>,
}

impl<'tcx> ReviewedPromotedOptionCompareV1<'tcx> {
    pub(crate) fn instance(&self) -> Instance<'tcx> {
        self.instance
    }

    pub(crate) fn expand_mir(&self, tcx: TyCtxt<'tcx>) -> Body<'tcx> {
        let mut body = tcx.instance_mir(self.instance.def).clone();
        for materialization in &self.materializations {
            let source_info = body.basic_blocks[materialization.block].statements
                [materialization.statement]
                .source_info;
            let mut declaration = LocalDecl::new(materialization.value_ty, source_info.span);
            declaration.source_info = source_info;
            let value = body.local_decls.push(declaration);
            let statements = &mut body.basic_blocks_mut()[materialization.block].statements;
            statements[materialization.statement].kind = StatementKind::Assign(Box::new((
                materialization.reference.into(),
                Rvalue::Ref(tcx.lifetimes.re_erased, BorrowKind::Shared, value.into()),
            )));
            statements.insert(
                materialization.statement,
                Statement::new(
                    source_info,
                    StatementKind::Assign(Box::new((
                        value.into(),
                        materialization.initializer.clone(),
                    ))),
                ),
            );
        }
        body
    }

    pub(crate) fn expansion_fingerprint(&self, tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> [u8; 16] {
        let fingerprint: Fingerprint = tcx.with_stable_hashing_context(|mut context| {
            let mut hasher = StableHasher::new();
            "fe2o3/promoted-option-comparison/expansion/v1".hash_stable(&mut context, &mut hasher);
            self.instance.hash_stable(&mut context, &mut hasher);
            self.source_fingerprint
                .hash_stable(&mut context, &mut hasher);
            body.hash_stable(&mut context, &mut hasher);
            hasher.finish()
        });
        fingerprint.to_le_bytes()
    }
}

pub(crate) fn prove_promoted_option_comparisons_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Option<ReviewedPromotedOptionCompareV1<'tcx>> {
    // The collector's external source-expansion flag also carries source-safety
    // authority. This proof does not, so it must never select an external body.
    if !instance.def_id().is_local()
        || !matches!(instance.def, InstanceKind::Item(_))
        || !matches!(
            tcx.def_kind(instance.def_id()),
            DefKind::Fn | DefKind::AssocFn
        )
    {
        return None;
    }
    prove_body(tcx, instance, tcx.instance_mir(instance.def))
}

fn prove_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
) -> Option<ReviewedPromotedOptionCompareV1<'tcx>> {
    if !instance.def_id().is_local()
        || !matches!(instance.def, InstanceKind::Item(_))
        || body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.tainted_by_errors.is_some()
        || body.coroutine.is_some()
        || body.spread_arg.is_some()
        || body.local_decls.len() > MAX_LOCALS
        || body.basic_blocks.len() > MAX_BLOCKS
        || body.source_scopes.len() > MAX_LOCALS
        || body.var_debug_info.len() > MAX_LOCALS
        || body.user_type_annotations.len() > 64
        || body
            .required_consts
            .as_ref()
            .is_some_and(|v| v.len() > MAX_LOCALS)
        || body
            .mentioned_items
            .as_ref()
            .is_some_and(|v| v.len() > MAX_LOCALS)
        || instance.args.len() > 64
        || body
            .source_scopes
            .iter()
            .any(|scope| scope.inlined.is_some() || scope.inlined_parent_scope.is_some())
    {
        return None;
    }
    let mut remaining = MAX_STATEMENTS;
    for block in body.basic_blocks.iter() {
        remaining = remaining.checked_sub(block.statements.len())?;
        remaining = remaining.checked_sub(block.after_last_stmt_debuginfos.len())?;
        for statement in &block.statements {
            remaining = remaining.checked_sub(statement.debuginfos.len())?;
        }
        block.terminator.as_ref()?;
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || signature.inputs().len() != body.arg_count
    {
        return None;
    }

    let mut materializations = Vec::new();
    for (block_id, block) in body.basic_blocks.iter_enumerated() {
        let Some((mut index, mut statement)) = block.statements.iter().enumerate().next_back()
        else {
            continue;
        };
        // The unoptimized profile retains exactly one shared reborrow. It is
        // preserved verbatim, and both reference locals must have a sole use.
        let reborrow = if let StatementKind::Assign(a) = &statement.kind
            && let Rvalue::Ref(_, BorrowKind::Shared, p) = &a.1
            && matches!(p.projection.as_ref(), [ProjectionElem::Deref])
            && a.0.projection.is_empty()
            && index != 0
        {
            let preceding = &block.statements[index - 1];
            if !matches!(&preceding.kind, StatementKind::Assign(prior)
                if prior.0.local == p.local && prior.0.projection.is_empty())
            {
                continue;
            }
            index -= 1;
            statement = preceding;
            Some(a.0.local)
        } else {
            None
        };
        let StatementKind::Assign(assignment) = &statement.kind else {
            continue;
        };
        let Rvalue::Use(Operand::Constant(constant)) = &assignment.1 else {
            continue;
        };
        let Const::Unevaluated(raw, _) = constant.const_ else {
            continue;
        };
        if raw.promoted.is_none() {
            continue;
        }
        let ty = normalized_ty(tcx, instance, constant.const_.ty())?;
        let TyKind::Ref(_, option, Mutability::Not) = *ty.kind() else {
            continue;
        };
        let TyKind::Adt(adt, _) = option.kind() else {
            continue;
        };
        if Some(adt.did()) != tcx.lang_items().option_type() {
            continue;
        }
        if materializations.len() == MAX_PROMOTIONS
            || block.is_cleanup
            || constant.user_ty.is_some()
            || !assignment.0.projection.is_empty()
            || assignment.0.local.as_usize() <= body.arg_count
            || normalized_ty(tcx, instance, body.local_decls.get(assignment.0.local)?.ty)? != ty
            || reborrow.is_some_and(|local| {
                local == assignment.0.local || local.as_usize() <= body.arg_count
            })
            || materializations.iter().any(|m: &Materialization<'_>| {
                m.reference == assignment.0.local
                    || m.reborrow == Some(assignment.0.local)
                    || reborrow
                        .is_some_and(|local| local == m.reference || m.reborrow == Some(local))
            })
        {
            return None;
        }
        if let Some(local) = reborrow
            && normalized_ty(tcx, instance, body.local_decls.get(local)?.ty)? != ty
        {
            return None;
        }
        let (comparison, comparison_fingerprint) = comparison_use(
            tcx,
            instance,
            body,
            block.terminator(),
            reborrow.unwrap_or(assignment.0.local),
            option,
        )?;
        let normalized = instance
            .try_instantiate_mir_and_normalize_erasing_regions(
                tcx,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(constant.const_),
            )
            .ok()?;
        let Const::Unevaluated(value, reference) = normalized else {
            return None;
        };
        if value.def != instance.def_id()
            || tcx.erase_and_anonymize_regions(value.args)
                != tcx.erase_and_anonymize_regions(instance.args)
            || reference != shared(tcx, option)
        {
            return None;
        }
        let promotion = value.promoted?;
        let promoted = tcx.promoted_mir(value.def).get(promotion)?;
        let initializer = promoted_initializer(tcx, instance, promotion, promoted, option)?;
        let fingerprint: Fingerprint = tcx.with_stable_hashing_context(|mut context| {
            let mut hasher = StableHasher::new();
            "fe2o3/promoted-option-comparison/dependencies/v1"
                .hash_stable(&mut context, &mut hasher);
            value.hash_stable(&mut context, &mut hasher);
            promoted.hash_stable(&mut context, &mut hasher);
            comparison.hash_stable(&mut context, &mut hasher);
            comparison_fingerprint.hash_stable(&mut context, &mut hasher);
            hasher.finish()
        });
        materializations.push(Materialization {
            block: block_id,
            statement: index,
            reference: assignment.0.local,
            reborrow,
            promotion,
            value_ty: option,
            initializer,
            dependency_fingerprint: fingerprint.to_le_bytes(),
        });
    }
    if materializations.is_empty() {
        return None;
    }
    let mut uses = SoleUses {
        slots: vec![None; body.local_decls.len()],
        remaining: MAX_WORK,
        valid: true,
    };
    for m in &materializations {
        uses.slots[m.reference.as_usize()] = Some(UseSlot {
            definition: Location {
                block: m.block,
                statement_index: m.statement,
            },
            consumption: Location {
                block: m.block,
                statement_index: m.statement + 1,
            },
            reborrow: m.reborrow.is_some(),
            stores: 0,
            reads: 0,
        });
        if let Some(local) = m.reborrow {
            uses.slots[local.as_usize()] = Some(UseSlot {
                definition: Location {
                    block: m.block,
                    statement_index: m.statement + 1,
                },
                consumption: Location {
                    block: m.block,
                    statement_index: m.statement + 2,
                },
                reborrow: false,
                stores: 0,
                reads: 0,
            });
        }
    }
    for (id, block) in body.basic_blocks.iter_enumerated() {
        uses.visit_basic_block_data(id, block);
    }
    for debug in &body.var_debug_info {
        uses.visit_var_debug_info(debug);
    }
    if !uses.valid
        || uses
            .slots
            .iter()
            .flatten()
            .any(|slot| slot.stores != 1 || slot.reads != 1)
    {
        return None;
    }
    let query = TypingEnv::fully_monomorphized().as_query_input((instance, ty::List::empty()));
    let abi = tcx.fn_abi_of_instance(query).ok()?;
    let fingerprint: Fingerprint = tcx.with_stable_hashing_context(|mut context| {
        let mut hasher = StableHasher::new();
        "fe2o3/promoted-option-comparison/source/v1".hash_stable(&mut context, &mut hasher);
        instance.hash_stable(&mut context, &mut hasher);
        body.hash_stable(&mut context, &mut hasher);
        signature.hash_stable(&mut context, &mut hasher);
        abi.hash_stable(&mut context, &mut hasher);
        for m in &materializations {
            m.block.hash_stable(&mut context, &mut hasher);
            m.statement.hash_stable(&mut context, &mut hasher);
            m.reference.hash_stable(&mut context, &mut hasher);
            m.reborrow.hash_stable(&mut context, &mut hasher);
            m.promotion.hash_stable(&mut context, &mut hasher);
            m.value_ty.hash_stable(&mut context, &mut hasher);
            m.initializer.hash_stable(&mut context, &mut hasher);
            m.dependency_fingerprint
                .hash_stable(&mut context, &mut hasher);
        }
        hasher.finish()
    });
    Some(ReviewedPromotedOptionCompareV1 {
        instance,
        source_fingerprint: fingerprint.to_le_bytes(),
        materializations,
    })
}

fn comparison_use<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    body: &Body<'tcx>,
    terminator: &Terminator<'tcx>,
    reference: Local,
    option: Ty<'tcx>,
) -> Option<(Instance<'tcx>, [u8; 16])> {
    let TerminatorKind::Call {
        func: Operand::Constant(c),
        args,
        destination,
        target: Some(target),
        unwind,
        ..
    } = &terminator.kind
    else {
        return None;
    };
    if c.user_ty.is_some()
        || !matches!(c.const_, Const::Val(ConstValue::ZeroSized, _))
        || args.len() != 2
        || destination.local == reference
        || body.basic_blocks.get(*target)?.is_cleanup
        || !match unwind {
            UnwindAction::Continue => tcx.sess.panic_strategy().unwinds(),
            UnwindAction::Unreachable => !tcx.sess.panic_strategy().unwinds(),
            _ => false,
        }
        || args
            .iter()
            .filter(|arg| {
                matches!(&arg.node, Operand::Move(p) | Operand::Copy(p)
            if p.local == reference && p.projection.is_empty())
            })
            .count()
            != 1
    {
        return None;
    }
    for arg in args {
        if normalized_ty(tcx, caller, arg.node.ty(body, tcx))? != shared(tcx, option) {
            return None;
        }
    }
    if normalized_ty(tcx, caller, destination.ty(body, tcx).ty)? != tcx.types.bool {
        return None;
    }
    let call_ty = normalized_ty(tcx, caller, c.const_.ty())?;
    let TyKind::FnDef(definition, args) = *call_ty.kind() else {
        return None;
    };
    let instance =
        Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), definition, args).ok()??;
    let comparison = contract(tcx, instance)?;
    if comparison.option != option
        || !primitive(comparison.payload)
        || !reviewed_body(tcx, instance, tcx.instance_mir(instance.def), &comparison)
    {
        return None;
    }
    let eq = if comparison.operation == Operation::Ne {
        comparison.callee
    } else {
        instance
    };
    let eq_contract = contract(tcx, eq)?;
    if eq_contract.operation != Operation::Eq || eq_contract.option != option
        || eq_contract.payload != comparison.payload
        || !reviewed_body(tcx, eq, tcx.instance_mir(eq.def), &eq_contract)
        || !super::super::core_primitive_value_v1::authenticate_reviewed_safe_core_primitive_value_helper_v1(
            tcx, eq_contract.callee,
        )
    { return None; }
    let fingerprint: Fingerprint = tcx.with_stable_hashing_context(|mut context| {
        let mut hasher = StableHasher::new();
        "fe2o3/promoted-option-comparison/read-only-core/v1".hash_stable(&mut context, &mut hasher);
        for callee in [instance, eq, eq_contract.callee] {
            callee.hash_stable(&mut context, &mut hasher);
            tcx.instance_mir(callee.def)
                .hash_stable(&mut context, &mut hasher);
        }
        hasher.finish()
    });
    Some((instance, fingerprint.to_le_bytes()))
}

fn primitive(ty: Ty<'_>) -> bool {
    matches!(
        ty.kind(),
        TyKind::Bool | TyKind::Char | TyKind::Int(_) | TyKind::Uint(_) | TyKind::Float(_)
    )
}

fn promoted_initializer<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    promotion: Promoted,
    body: &Body<'tcx>,
    option: Ty<'tcx>,
) -> Option<Rvalue<'tcx>> {
    if body.source.instance != instance.def
        || body.source.promoted != Some(promotion)
        || body.arg_count != 0
        || body.local_decls.len() != 2
        || body.basic_blocks.len() != 1
        || body.tainted_by_errors.is_some()
        || body.coroutine.is_some()
        || body.spread_arg.is_some()
        || body.source_scopes.len() > 4
        || !body.var_debug_info.is_empty()
        || !body.user_type_annotations.is_empty()
        || body.required_consts.as_ref().is_some_and(|v| v.len() > 1)
        || body.mentioned_items.as_ref().is_some_and(|v| !v.is_empty())
        || body
            .source_scopes
            .iter()
            .any(|s| s.inlined.is_some() || s.inlined_parent_scope.is_some())
        || normalized_ty(tcx, instance, body.local_decls[Local::from_usize(0)].ty)?
            != shared(tcx, option)
        || normalized_ty(tcx, instance, body.local_decls[Local::from_usize(1)].ty)? != option
    {
        return None;
    }
    let block = &body.basic_blocks[BasicBlock::from_usize(0)];
    let [initialize, borrow] = block.statements.as_slice() else {
        return None;
    };
    if block.is_cleanup
        || !matches!(block.terminator.as_ref()?.kind, TerminatorKind::Return)
        || !block.after_last_stmt_debuginfos.is_empty()
        || !initialize.debuginfos.is_empty()
        || !borrow.debuginfos.is_empty()
    {
        return None;
    }
    let StatementKind::Assign(value) = &initialize.kind else {
        return None;
    };
    let StatementKind::Assign(reference) = &borrow.kind else {
        return None;
    };
    if !super::plain(value.0, 1)
        || !super::plain(reference.0, 0)
        || !matches!(&reference.1, Rvalue::Ref(_, BorrowKind::Shared, p) if super::plain(*p, 1))
    {
        return None;
    }
    let initializer = instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(value.1.clone()),
        )
        .ok()?;
    let Rvalue::Aggregate(kind, operands) = &initializer else {
        return None;
    };
    let AggregateKind::Adt(definition, variant, args, None, None) = **kind else {
        return None;
    };
    let TyKind::Adt(adt, option_args) = *option.kind() else {
        return None;
    };
    if Some(definition) != tcx.lang_items().option_type()
        || definition != adt.did()
        || args != option_args
        || args.len() != 1
        || !primitive(args.type_at(0))
    {
        return None;
    }
    let variant_definition = adt.variants().get(variant)?.def_id;
    if Some(variant_definition) == tcx.lang_items().option_none_variant() {
        return (operands.is_empty() && body.required_consts.as_ref().is_none_or(|v| v.is_empty()))
            .then_some(initializer);
    }
    if Some(variant_definition) != tcx.lang_items().option_some_variant() {
        return None;
    }
    let [Operand::Constant(value)] = operands.raw.as_slice() else {
        return None;
    };
    if value.user_ty.is_some() || value.const_.ty() != args.type_at(0) {
        return None;
    }
    // A literal or fully instantiated const-generic leaf, not a new CTFE call
    // to arbitrary user code or an allocation-backed value.
    match value.const_ {
        Const::Val(ConstValue::Scalar(_), _) => {}
        Const::Ty(_, constant) if matches!(constant.kind(), ty::ConstKind::Value(_)) => {}
        _ => return None,
    }
    for required in body.required_consts.iter().flatten() {
        if required.user_ty.is_some()
            || instance
                .try_instantiate_mir_and_normalize_erasing_regions(
                    tcx,
                    TypingEnv::fully_monomorphized(),
                    EarlyBinder::bind(required.const_),
                )
                .ok()?
                != value.const_
        {
            return None;
        }
    }
    let scalar = value
        .const_
        .try_eval_scalar_int(tcx, TypingEnv::fully_monomorphized())?;
    let layout = tcx
        .layout_of(TypingEnv::fully_monomorphized().as_query_input(args.type_at(0)))
        .ok()?;
    if scalar.size() != layout.size {
        return None;
    }
    let bits = scalar.to_bits(scalar.size());
    if (args.type_at(0).is_bool() && bits > 1)
        || (args.type_at(0).is_char()
            && u32::try_from(bits).ok().and_then(char::from_u32).is_none())
    {
        return None;
    }
    Some(initializer)
}

#[derive(Clone, Copy)]
struct UseSlot {
    definition: Location,
    consumption: Location,
    reborrow: bool,
    stores: u8,
    reads: u8,
}
struct SoleUses {
    slots: Vec<Option<UseSlot>>,
    remaining: usize,
    valid: bool,
}
impl SoleUses {
    fn charge(&mut self, work: usize) -> bool {
        if !self.valid {
            return false;
        }
        match self.remaining.checked_sub(work) {
            Some(remaining) => {
                self.remaining = remaining;
                true
            }
            None => {
                self.valid = false;
                false
            }
        }
    }
}
impl<'tcx> Visitor<'tcx> for SoleUses {
    fn visit_statement(&mut self, statement: &Statement<'tcx>, location: Location) {
        if self.charge(1) {
            self.super_statement(statement, location);
        }
    }
    fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
        let size = match &terminator.kind {
            TerminatorKind::SwitchInt { targets, .. } => targets.all_targets().len(),
            TerminatorKind::Call { args, .. } => args.len(),
            TerminatorKind::InlineAsm { .. } => {
                self.valid = false;
                return;
            }
            _ => 1,
        };
        if self.charge(size) {
            self.super_terminator(terminator, location);
        }
    }
    fn visit_rvalue(&mut self, value: &Rvalue<'tcx>, location: Location) {
        let size = if let Rvalue::Aggregate(_, values) = value {
            values.len()
        } else {
            1
        };
        if self.charge(size) {
            self.super_rvalue(value, location);
        }
    }
    fn visit_place(&mut self, place: &Place<'tcx>, context: PlaceContext, location: Location) {
        if !self.charge(place.projection.len() + 1) {
            return;
        }
        if self.slots.get(place.local.as_usize()).is_none() {
            self.valid = false;
            return;
        }
        if let Some(slot) = self.slots[place.local.as_usize()]
            && !place.projection.is_empty()
        {
            if slot.reborrow
                && location == slot.consumption
                && context == PlaceContext::NonMutatingUse(NonMutatingUseContext::SharedBorrow)
                && matches!(place.projection.as_ref(), [ProjectionElem::Deref])
            {
                self.visit_local(place.local, context, location);
                return;
            }
            self.valid = false;
            return;
        }
        self.super_place(place, context, location);
    }
    fn visit_local(&mut self, local: Local, context: PlaceContext, location: Location) {
        if !self.charge(1) {
            return;
        }
        let Some(slot) = self.slots.get_mut(local.as_usize()) else {
            self.valid = false;
            return;
        };
        let Some(slot) = slot else { return };
        // Storage markers do not consume the pointer; adjacency prevents any
        // marker from invalidating the definition-to-use interval itself.
        if matches!(
            context,
            PlaceContext::NonUse(NonUseContext::StorageLive | NonUseContext::StorageDead)
        ) {
            return;
        }
        if location == slot.definition
            && context == PlaceContext::MutatingUse(MutatingUseContext::Store)
            && slot.stores == 0
        {
            slot.stores = 1;
        } else if location == slot.consumption
            && (if slot.reborrow {
                context == PlaceContext::NonMutatingUse(NonMutatingUseContext::SharedBorrow)
            } else {
                matches!(
                    context,
                    PlaceContext::NonMutatingUse(
                        NonMutatingUseContext::Move | NonMutatingUseContext::Copy
                    )
                )
            })
            && slot.reads == 0
        {
            slot.reads = 1;
        } else {
            self.valid = false;
        }
    }
}

#[cfg(test)]
#[path = "promoted/tests.rs"]
mod tests;
