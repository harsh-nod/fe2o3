//! Retained-core masked shifts across primitive integer widths and directions.
//! Only the safe wrapping entry can issue this proof; the unchecked helper and
//! its complete guarded failure path are evidence, never standalone terminals.

use super::*;
use rustc_middle::mir::{AssertMessage, ProjectionElem};

#[path = "general_shift/precondition.rs"]
mod precondition;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ShiftSpec<'tcx> {
    element: Ty<'tcx>,
    bits: u32,
    left: bool,
}

impl<'tcx> ShiftSpec<'tcx> {
    fn from_root(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<Self> {
        if !core_item(tcx, instance) || tcx.def_kind(instance.def_id()) != DefKind::AssocFn {
            return None;
        }
        let left = match tcx.item_name(instance.def_id()).as_str() {
            "wrapping_shl" => true,
            "wrapping_shr" => false,
            _ => return None,
        };
        let implementation = tcx.impl_of_assoc(instance.def_id())?;
        let element = tcx.type_of(implementation).instantiate_identity();
        let bits = match element.kind() {
            TyKind::Int(integer) => integer.bit_width(),
            TyKind::Uint(integer) => integer.bit_width(),
            _ => return None,
        }
        .unwrap_or(tcx.data_layout.pointer_size().bits());
        if !matches!(bits, 8 | 16 | 32 | 64 | 128) {
            return None;
        }
        let spec = Self {
            element,
            bits: bits as u32,
            left,
        };
        spec.identity(tcx, instance, Part::Wrapping).then_some(spec)
    }

    fn operation(self, unchecked: bool) -> BinOp {
        match (self.left, unchecked) {
            (true, false) => BinOp::Shl,
            (true, true) => BinOp::ShlUnchecked,
            (false, false) => BinOp::Shr,
            (false, true) => BinOp::ShrUnchecked,
        }
    }

    fn identity(self, tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, part: Part) -> bool {
        if !core_item(tcx, instance) {
            return false;
        }
        let definition = instance.def_id();
        let sig = signature(tcx, instance);
        if sig.abi != ExternAbi::Rust
            || sig.c_variadic
            || sig.safety
                != if part == Part::Unchecked {
                    Safety::Unsafe
                } else {
                    Safety::Safe
                }
        {
            return false;
        }
        if part == Part::Precondition {
            return tcx.def_kind(definition) == DefKind::Fn
                && tcx.item_name(definition).as_str() == "precondition_check"
                && self.identity(
                    tcx,
                    Instance::mono(tcx, tcx.parent(definition)),
                    Part::Unchecked,
                )
                && sig.inputs() == [tcx.types.u32]
                && sig.output() == tcx.types.unit;
        }
        let name = match (self.left, part) {
            (true, Part::Wrapping) => "wrapping_shl",
            (false, Part::Wrapping) => "wrapping_shr",
            (true, Part::Unchecked) => "unchecked_shl",
            (false, Part::Unchecked) => "unchecked_shr",
            (_, Part::Precondition) => unreachable!(),
        };
        let Some(implementation) = tcx.impl_of_assoc(definition) else {
            return false;
        };
        tcx.def_kind(definition) == DefKind::AssocFn
            && tcx.item_name(definition).as_str() == name
            && tcx
                .opt_associated_item(definition)
                .is_some_and(|item| item.is_fn())
            && implementation.krate == definition.krate
            && !tcx.impl_is_of_trait(implementation)
            && tcx.type_of(implementation).instantiate_identity() == self.element
            && sig.inputs() == [self.element, tcx.types.u32]
            && sig.output() == self.element
    }

    fn message(self) -> String {
        format!(
            "unsafe precondition(s) violated: {}::unchecked_sh{} cannot overflow\n\nThis indicates a bug in the program. This Undefined Behavior check is optional, and cannot be relied on for safety.",
            self.element,
            if self.left { 'l' } else { 'r' }
        )
    }

    fn precondition_element(self, tcx: TyCtxt<'tcx>) -> Option<Ty<'tcx>> {
        use rustc_middle::ty::{IntTy, UintTy};
        // The pinned macro uses <$ActualT>::BITS only inside the UB checker.
        // The wrapping entry still requires the original Self::BITS identity.
        Some(match (self.element.kind(), self.bits) {
            (TyKind::Int(IntTy::Isize), 16) => tcx.types.i16,
            (TyKind::Int(IntTy::Isize), 32) => tcx.types.i32,
            (TyKind::Int(IntTy::Isize), 64) => tcx.types.i64,
            (TyKind::Uint(UintTy::Usize), 16) => tcx.types.u16,
            (TyKind::Uint(UintTy::Usize), 32) => tcx.types.u32,
            (TyKind::Uint(UintTy::Usize), 64) => tcx.types.u64,
            (TyKind::Int(IntTy::Isize) | TyKind::Uint(UintTy::Usize), _) => return None,
            _ => self.element,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Part {
    Wrapping,
    Unchecked,
    Precondition,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ReviewedCoreWrappingShiftV1<'tcx> {
    instance: Instance<'tcx>,
    spec: ShiftSpec<'tcx>,
    closure_fingerprint: [u8; 16],
}

impl<'tcx> ReviewedCoreWrappingShiftV1<'tcx> {
    pub(crate) fn instance(self) -> Instance<'tcx> {
        self.instance
    }

    pub(crate) fn expansion_fingerprint(self, tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> [u8; 16] {
        let fingerprint: Fingerprint = tcx.with_stable_hashing_context(|mut context| {
            let mut hasher = StableHasher::new();
            "fe2o3/core-primitive-wrapping-shift/masked-mir/v1"
                .hash_stable(&mut context, &mut hasher);
            self.instance.hash_stable(&mut context, &mut hasher);
            self.spec.element.hash_stable(&mut context, &mut hasher);
            self.spec.bits.hash_stable(&mut context, &mut hasher);
            self.spec.left.hash_stable(&mut context, &mut hasher);
            self.closure_fingerprint
                .hash_stable(&mut context, &mut hasher);
            body.hash_stable(&mut context, &mut hasher);
            hasher.finish()
        });
        fingerprint.to_le_bytes()
    }

    pub(crate) fn expand_mir(self, tcx: TyCtxt<'tcx>) -> Body<'tcx> {
        let mut body = tcx.instance_mir(self.instance.def).clone();
        let source_info = SourceInfo {
            span: body.span,
            scope: SourceScope::from_usize(0),
        };
        let mask = Operand::Constant(Box::new(ConstOperand {
            span: body.span,
            user_ty: None,
            const_: Const::Val(
                ConstValue::Scalar(rustc_middle::mir::interpret::Scalar::from_uint(
                    self.spec.bits - 1,
                    rustc_abi::Size::from_bytes(4),
                )),
                tcx.types.u32,
            ),
        }));
        let binary = |dest: usize, operation, left, right| {
            Statement::new(
                source_info,
                StatementKind::Assign(Box::new((
                    Local::from_usize(dest).into(),
                    Rvalue::BinaryOp(operation, Box::new((left, right))),
                ))),
            )
        };
        body.basic_blocks_mut().raw.clear();
        body.basic_blocks_mut().push(BasicBlockData::new_stmts(
            vec![
                binary(
                    3,
                    BinOp::BitAnd,
                    Operand::Copy(Local::from_usize(2).into()),
                    mask,
                ),
                binary(
                    0,
                    self.spec.operation(false),
                    Operand::Copy(Local::from_usize(1).into()),
                    Operand::Move(Local::from_usize(3).into()),
                ),
            ],
            Some(Terminator {
                source_info,
                kind: TerminatorKind::Return,
            }),
            false,
        ));
        body.local_decls.truncate(3);
        body.local_decls
            .push(LocalDecl::new(tcx.types.u32, body.span));
        for local in &mut body.local_decls {
            local.source_info = source_info;
        }
        body.source_scopes.truncate(1);
        body.var_debug_info.clear();
        body.required_consts = Some(Vec::new());
        body
    }
}

pub(crate) fn prove_core_wrapping_shift_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Option<ReviewedCoreWrappingShiftV1<'tcx>> {
    // Keep the older u32-right-shift profile and its proof namespace unchanged.
    let spec = ShiftSpec::from_root(tcx, instance)?;
    if spec.element == tcx.types.u32 && !spec.left {
        return None;
    }
    prove_with_budget(
        tcx,
        instance,
        &|instance| tcx.instance_mir(instance.def),
        &mut 16,
    )
}

fn prove_with_budget<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
    budget: &mut usize,
) -> Option<ReviewedCoreWrappingShiftV1<'tcx>>
where
    'tcx: 'a,
{
    let spec = ShiftSpec::from_root(tcx, instance)?;
    let visited = std::cell::RefCell::new(Vec::new());
    let fetch = |instance| {
        let source = body(instance);
        visited.borrow_mut().push((instance, source));
        source
    };
    if !check(tcx, instance, spec, Part::Wrapping, &fetch, budget) {
        return None;
    }
    let fingerprint: Fingerprint = tcx.with_stable_hashing_context(|mut context| {
        let mut hasher = StableHasher::new();
        for (instance, source) in visited.into_inner() {
            instance.hash_stable(&mut context, &mut hasher);
            source.hash_stable(&mut context, &mut hasher);
        }
        hasher.finish()
    });
    Some(ReviewedCoreWrappingShiftV1 {
        instance,
        spec,
        closure_fingerprint: fingerprint.to_le_bytes(),
    })
}

fn check<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    spec: ShiftSpec<'tcx>,
    part: Part,
    fetch: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
    budget: &mut usize,
) -> bool
where
    'tcx: 'a,
{
    if *budget == 0 || !spec.identity(tcx, instance, part) {
        return false;
    }
    *budget -= 1;
    let body = fetch(instance);
    if body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.coroutine.is_some()
        || body.spread_arg.is_some()
        || body.tainted_by_errors.is_some()
        || body.is_polymorphic
        || body.injection_phase.is_some()
        || body.var_debug_info.len() > 32
        || !body.user_type_annotations.is_empty()
        || body.basic_blocks.len() > 5
        || body.local_decls.len() > 16
        || body.source_scopes.len() > 8
        || body.local_decls.iter().any(|decl| decl.user_ty.is_some())
        || body
            .basic_blocks
            .iter()
            .any(|block| block.is_cleanup || block.terminator.is_none())
        || body
            .basic_blocks
            .iter()
            .map(|block| block.statements.len())
            .sum::<usize>()
            > 64
    {
        return false;
    }
    match part {
        Part::Wrapping => wrapping(tcx, body, spec, fetch, budget),
        Part::Unchecked => unchecked(tcx, body, spec, fetch, budget),
        Part::Precondition => precondition::check(tcx, body, spec, fetch, budget),
    }
}

#[allow(clippy::too_many_arguments)]
fn call<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    block: usize,
    dest: usize,
    next: usize,
    operands: &[(usize, bool)],
    spec: ShiftSpec<'tcx>,
    part: Part,
    fetch: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
    budget: &mut usize,
) -> bool
where
    'tcx: 'a,
{
    let TerminatorKind::Call {
        func,
        args,
        destination,
        target,
        unwind,
        ..
    } = &retained::bb(body, block).terminator().kind
    else {
        return false;
    };
    let Some(callee) = resolve(tcx, func) else {
        return false;
    };
    args.len() == operands.len()
        && args
            .iter()
            .zip(operands)
            .all(|(arg, &(local, moved))| retained::operand(&arg.node, local, moved))
        && local(*destination, dest)
        && target.map(|target| target.as_usize()) == Some(next)
        && matches!(unwind, UnwindAction::Unreachable)
        && check(tcx, callee, spec, part, fetch, budget)
}

fn wrapping<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    spec: ShiftSpec<'tcx>,
    fetch: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
    budget: &mut usize,
) -> bool
where
    'tcx: 'a,
{
    if !retained::shape(
        body,
        2,
        &[
            spec.element,
            spec.element,
            tcx.types.u32,
            tcx.types.u32,
            tcx.types.u32,
            Ty::new_tup(tcx, &[tcx.types.u32, tcx.types.bool]),
        ],
        &[1, 2, 0],
        1,
    ) {
        return false;
    }
    let Some(Rvalue::BinaryOp(BinOp::SubWithOverflow, operands)) =
        assignment(&retained::bb(body, 0).statements[0], 5)
    else {
        return false;
    };
    if !precondition::bits(tcx, &operands.0, spec)
        || scalar(tcx, &operands.1, tcx.types.u32) != Some(1)
        || !precondition::required_bits(tcx, body, 2, spec)
    {
        return false;
    }
    let TerminatorKind::Assert {
        cond,
        expected,
        msg,
        target,
        unwind,
    } = &retained::bb(body, 0).terminator().kind
    else {
        return false;
    };
    // Exact BITS CTFE plus width >= 8 proves this subtraction nonoverflowing.
    // Both the assertion and its failure operands remain part of the proof.
    !*expected
        && moved_field(cond, 5, 1, tcx.types.bool)
        && target.as_usize() == 1
        && matches!(unwind, UnwindAction::Unreachable)
        && matches!(&**msg, AssertMessage::Overflow(BinOp::Sub, left, right) if left == &operands.0 && right == &operands.1)
        && matches!(assignment(&retained::bb(body, 1).statements[0], 4), Some(Rvalue::Use(value)) if moved_field(value, 5, 0, tcx.types.u32))
        && retained::binary(body, 1, 1, 3, BinOp::BitAnd, 2, false, 4, true)
        && retained::returns(body, 2)
        && call(
            tcx,
            body,
            1,
            0,
            2,
            &[(1, false), (3, true)],
            spec,
            Part::Unchecked,
            fetch,
            budget,
        )
}

fn unchecked<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    spec: ShiftSpec<'tcx>,
    fetch: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
    budget: &mut usize,
) -> bool
where
    'tcx: 'a,
{
    retained::shape(
        body,
        2,
        &[
            spec.element,
            spec.element,
            tcx.types.u32,
            tcx.types.bool,
            tcx.types.unit,
        ],
        &[0, 0, 0, 1],
        1,
    ) && retained::no_required_consts(body)
        && retained::switch_local(body, 1, 3, 3, 2)
        && retained::binary(body, 3, 0, 0, spec.operation(true), 1, false, 2, false)
        && retained::returns(body, 3)
        && retained::call(tcx, body, 0, 3, 1, &[], Helper::LanguageUb, fetch, budget)
        && call(
            tcx,
            body,
            2,
            4,
            3,
            &[(2, false)],
            spec,
            Part::Precondition,
            fetch,
            budget,
        )
}

fn moved_field<'tcx>(operand: &Operand<'tcx>, local: usize, field: usize, ty: Ty<'tcx>) -> bool {
    matches!(operand, Operand::Move(place) if place.local.as_usize() == local
        && matches!(place.projection.as_ref(), [ProjectionElem::Field(actual, actual_ty)]
            if actual.as_usize() == field && *actual_ty == ty))
}

#[cfg(test)]
#[path = "general_shift/tests.rs"]
mod tests;
