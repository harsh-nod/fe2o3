//! Observe the exact shared-receiver adjustment in rustc's owned closure shim.

use rustc_abi::ExternAbi;
use rustc_hir::{Mutability, Safety};
use rustc_middle::mir::visit::{PlaceContext, Visitor};
use rustc_middle::mir::{
    Body, BorrowKind, Local, Location, MutBorrowKind, Operand, Place, RETURN_PLACE, Rvalue,
    Statement, StmtDebugInfo, Terminator, TerminatorKind,
};
use rustc_middle::ty::{ClosureKind, Instance, Ty, TyCtxt, TyKind, TypingEnv};

use crate::closure_profile_v1::authenticate_once_shim_v1;
use crate::rustc_semantic_plan_v1::source_signature_v1;

#[derive(Debug)]
pub(crate) enum ReceiverReborrowErrorV1<E> {
    Resource(E),
    Unsupported(&'static str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ReceiverReborrowV1<'tcx> {
    block: u32,
    receiver: Place<'tcx>,
    callee: Instance<'tcx>,
    shared_receiver_type: Ty<'tcx>,
}

impl<'tcx> ReceiverReborrowV1<'tcx> {
    pub(crate) fn block(&self) -> u32 {
        self.block
    }

    pub(crate) fn receiver(&self) -> Place<'tcx> {
        self.receiver
    }

    pub(crate) fn callee(&self) -> Instance<'tcx> {
        self.callee
    }

    pub(crate) fn shared_receiver_type(&self) -> Ty<'tcx> {
        self.shared_receiver_type
    }
}

/// This is an observation, not a call-binding or body-custody credential. The
/// consumer must bind it to its frozen callee and exact supplied-body identity.
pub(crate) fn derive_fn_receiver_reborrow_v1<'tcx, E>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    mut charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<Option<ReceiverReborrowV1<'tcx>>, ReceiverReborrowErrorV1<E>> {
    use ReceiverReborrowErrorV1::{Resource, Unsupported};

    charge(1).map_err(Resource)?;
    let Some(callee) = authenticate_once_shim_v1(tcx, instance)
        .map_err(|_| Unsupported("unauthenticated closure once shim"))?
    else {
        return Ok(None);
    };
    if body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.arg_count != 2
        || body.spread_arg != Some(Local::from_usize(2))
    {
        return Err(Unsupported("closure once shim body source or arguments"));
    }

    charge(2).map_err(Resource)?;
    let shim_signature = source_signature_v1(tcx, instance).map_err(Unsupported)?;
    let closure_signature = source_signature_v1(tcx, callee).map_err(Unsupported)?;
    let [self_ty, tuple_ty] = shim_signature.inputs() else {
        return Err(Unsupported("closure once shim source signature"));
    };
    let [closure_receiver, closure_tuple] = closure_signature.inputs() else {
        return Err(Unsupported("closure body source signature"));
    };
    if shim_signature.abi != ExternAbi::RustCall
        || closure_signature.abi != ExternAbi::RustCall
        || shim_signature.c_variadic
        || closure_signature.c_variadic
        || shim_signature.safety != Safety::Safe
        || closure_signature.safety != Safety::Safe
        || !matches!(tuple_ty.kind(), TyKind::Tuple(_))
        || tuple_ty != closure_tuple
        || shim_signature.output() != closure_signature.output()
    {
        return Err(Unsupported("closure once shim source signature mismatch"));
    }
    let TyKind::Closure(_, closure_args) = self_ty.kind() else {
        return Err(Unsupported("closure once shim receiver is not a closure"));
    };
    let kind = closure_args.as_closure().kind_ty().to_opt_closure_kind();
    let needs_reborrow = match kind {
        Some(ClosureKind::Fn) => true,
        Some(ClosureKind::FnMut) => false,
        _ => return Err(Unsupported("closure once shim closure kind")),
    };
    let expected_mutability = if needs_reborrow {
        Mutability::Not
    } else {
        Mutability::Mut
    };
    if !matches!(closure_receiver.kind(), TyKind::Ref(_, pointee, mutability)
        if pointee == self_ty && *mutability == expected_mutability)
    {
        return Err(Unsupported("closure body receiver type"));
    }
    for (index, expected) in [shim_signature.output(), *self_ty, *tuple_ty]
        .into_iter()
        .enumerate()
    {
        if normalized_local(tcx, instance, body, Local::from_usize(index), &mut charge)? != expected
        {
            return Err(Unsupported(
                "closure once shim argument or return local type",
            ));
        }
    }

    let mut call = None;
    for (block, data) in body.basic_blocks.iter_enumerated() {
        charge(1).map_err(Resource)?;
        let terminator = data
            .terminator
            .as_ref()
            .ok_or(Unsupported("closure once shim block without terminator"))?;
        match &terminator.kind {
            TerminatorKind::Call { .. } if call.replace((block, data)).is_some() => {
                return Err(Unsupported("closure once shim has multiple calls"));
            }
            TerminatorKind::TailCall { .. } => {
                return Err(Unsupported("closure once shim tail call"));
            }
            _ => {}
        }
    }
    let (block, data) = call.ok_or(Unsupported("closure once shim receiver call is absent"))?;
    let TerminatorKind::Call {
        func,
        args,
        destination,
        ..
    } = &data.terminator().kind
    else {
        unreachable!();
    };
    if data.is_cleanup || args.len() != 2 || destination.as_local() != Some(RETURN_PLACE) {
        return Err(Unsupported("closure once shim receiver call shape"));
    }
    let Operand::Move(receiver) = &args[0].node else {
        return Err(Unsupported("closure once shim receiver is not moved"));
    };
    let Some(receiver_local) = receiver.as_local() else {
        return Err(Unsupported("closure once shim receiver is projected"));
    };
    if receiver_local.index() <= body.arg_count
        || !matches!(&args[1].node, Operand::Move(place)
            if place.as_local() == Some(Local::from_usize(2)))
    {
        return Err(Unsupported("closure once shim receiver or tuple operand"));
    }
    let receiver_ty = normalized_local(tcx, instance, body, receiver_local, &mut charge)?;
    if !matches!(receiver_ty.kind(), TyKind::Ref(_, pointee, Mutability::Mut)
        if pointee == self_ty)
    {
        return Err(Unsupported(
            "closure once shim receiver is not a mutable reference",
        ));
    }

    let Operand::Constant(function) = func else {
        return Err(Unsupported("closure once shim indirect receiver call"));
    };
    charge(1).map_err(Resource)?;
    let function_ty = super::normalize_type_v1(tcx, instance, function.const_.ty())
        .map_err(|_| Unsupported("closure once shim callable normalization"))?;
    let TyKind::FnDef(definition, arguments) = function_ty.kind() else {
        return Err(Unsupported(
            "closure once shim receiver call is not a function item",
        ));
    };
    charge(1).map_err(Resource)?;
    let fn_mut = tcx
        .lang_items()
        .fn_mut_trait()
        .ok_or(Unsupported("missing FnMut language item"))?;
    let mut method = None;
    for item in tcx.associated_items(fn_mut).in_definition_order() {
        charge(1).map_err(Resource)?;
        if item.is_fn() && method.replace(item.def_id).is_some() {
            return Err(Unsupported("ambiguous FnMut language item method"));
        }
    }
    if method != Some(*definition)
        || arguments.len() != 2
        || arguments[0].as_type() != Some(*self_ty)
        || arguments[1].as_type() != Some(*tuple_ty)
    {
        return Err(Unsupported(
            "closure once shim call is not the exact FnMut method",
        ));
    }
    charge(2).map_err(Resource)?;
    let resolved = Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        *definition,
        arguments,
    )
    .map_err(|_| Unsupported("closure once shim receiver call resolution"))?
    .ok_or(Unsupported("closure once shim receiver call is unresolved"))?;
    let call_signature =
        source_signature_v1(tcx, Instance::new_raw(*definition, arguments)).map_err(Unsupported)?;
    if resolved != callee
        || call_signature.abi != ExternAbi::RustCall
        || call_signature.c_variadic
        || call_signature.safety != Safety::Safe
        || call_signature.inputs() != [receiver_ty, *tuple_ty]
        || call_signature.output() != closure_signature.output()
    {
        return Err(Unsupported(
            "closure once shim resolved body or call signature mismatch",
        ));
    }

    let mut borrowed_owned_self = false;
    for statement in &data.statements {
        charge(1).map_err(Resource)?;
        if let Some((destination, value)) = statement.kind.as_assign()
            && destination.local == receiver_local
        {
            if borrowed_owned_self
                || destination.as_local() != Some(receiver_local)
                || !matches!(value,
                    Rvalue::Ref(_, BorrowKind::Mut { kind: MutBorrowKind::Default }, place)
                        if place.as_local() == Some(Local::from_usize(1)))
            {
                return Err(Unsupported("closure once shim receiver borrow"));
            }
            borrowed_owned_self = true;
        }
    }
    if !borrowed_owned_self {
        return Err(Unsupported(
            "closure once shim mutable receiver borrow is absent",
        ));
    }

    // Replacing the move by a reborrow must not expose any other executable use
    // of the original temporary, including uses in unreachable blocks.
    let mut uses = ReceiverUsesV1 {
        receiver: receiver_local,
        count: 0,
        local_count: body.local_decls.len(),
        charge: &mut charge,
        error: None,
    };
    for (block, data) in body.basic_blocks.iter_enumerated() {
        if !uses.work(1) {
            return Err(uses.error.take().unwrap());
        }
        for (statement_index, statement) in data.statements.iter().enumerate() {
            uses.visit_statement(
                statement,
                Location {
                    block,
                    statement_index,
                },
            );
            if let Some(error) = uses.error.take() {
                return Err(error);
            }
        }
        uses.visit_terminator(
            data.terminator(),
            Location {
                block,
                statement_index: data.statements.len(),
            },
        );
        if let Some(error) = uses.error.take() {
            return Err(error);
        }
    }
    if uses.count != 2 {
        return Err(Unsupported(
            "closure once shim receiver temporary has other uses",
        ));
    }
    if !needs_reborrow {
        return Ok(None);
    }
    Ok(Some(ReceiverReborrowV1 {
        block: u32::try_from(block.index())
            .map_err(|_| Unsupported("closure once shim block index overflow"))?,
        receiver: *receiver,
        callee,
        shared_receiver_type: *closure_receiver,
    }))
}

fn normalized_local<'tcx, E>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    local: Local,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<Ty<'tcx>, ReceiverReborrowErrorV1<E>> {
    charge(1).map_err(ReceiverReborrowErrorV1::Resource)?;
    let declaration = body
        .local_decls
        .get(local)
        .ok_or(ReceiverReborrowErrorV1::Unsupported(
            "closure once shim local is absent",
        ))?;
    super::normalize_type_v1(tcx, instance, declaration.ty)
        .map_err(|_| ReceiverReborrowErrorV1::Unsupported("closure once shim local normalization"))
}

struct ReceiverUsesV1<'a, E, F> {
    receiver: Local,
    count: u8,
    local_count: usize,
    charge: &'a mut F,
    error: Option<ReceiverReborrowErrorV1<E>>,
}

impl<E, F: FnMut(usize) -> Result<(), E>> ReceiverUsesV1<'_, E, F> {
    fn work(&mut self, amount: usize) -> bool {
        if self.error.is_none() {
            self.error = (self.charge)(amount)
                .err()
                .map(ReceiverReborrowErrorV1::Resource);
        }
        self.error.is_none()
    }
}

impl<'tcx, E, F: FnMut(usize) -> Result<(), E>> Visitor<'tcx> for ReceiverUsesV1<'_, E, F> {
    fn visit_statement(&mut self, statement: &Statement<'tcx>, location: Location) {
        if self.work(statement.debuginfos.len().saturating_add(1)) {
            self.super_statement(statement, location);
        }
    }

    fn visit_statement_debuginfo(&mut self, _: &StmtDebugInfo<'tcx>, _: Location) {}

    fn visit_terminator(&mut self, terminator: &Terminator<'tcx>, location: Location) {
        let children = match &terminator.kind {
            TerminatorKind::Call { args, .. } | TerminatorKind::TailCall { args, .. } => args.len(),
            TerminatorKind::InlineAsm { operands, .. } => operands.len(),
            _ => 0,
        };
        if self.work(children.saturating_add(1)) {
            self.super_terminator(terminator, location);
        }
    }

    fn visit_local(&mut self, local: Local, _: PlaceContext, _: Location) {
        if !self.work(1) {
            return;
        }
        if local.index() >= self.local_count {
            self.error = Some(ReceiverReborrowErrorV1::Unsupported(
                "closure once shim executable local is absent",
            ));
        } else if local == self.receiver {
            self.count = self.count.saturating_add(1).min(3);
        }
    }

    fn visit_place(&mut self, place: &Place<'tcx>, context: PlaceContext, location: Location) {
        if self.work(place.projection.len().saturating_add(1)) {
            self.super_place(place, context, location);
        }
    }

    fn visit_operand(&mut self, operand: &Operand<'tcx>, location: Location) {
        if self.work(1) {
            self.super_operand(operand, location);
        }
    }

    fn visit_rvalue(&mut self, value: &Rvalue<'tcx>, location: Location) {
        let children = match value {
            Rvalue::Aggregate(_, operands) => operands.len(),
            _ => 0,
        };
        if self.work(children.saturating_add(1)) {
            self.super_rvalue(value, location);
        }
    }
}

#[cfg(test)]
mod tests;
