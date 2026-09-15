//! Complete structural recipe. This alone authenticates no Rust provider.
use super::*;

fn plain(place: &SemanticPlaceV1, ty: SemanticTypeIdV1) -> Option<SemanticLocalIdV1> {
    (place.ty() == ty && place.projections().is_empty()).then_some(place.local())
}

fn operand_local(operand: &SemanticOperandV1, ty: SemanticTypeIdV1) -> Option<SemanticLocalIdV1> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => plain(place, ty),
        SemanticOperandV1::Constant(_) => None,
    }
}

fn role(
    body: &SemanticFunctionDeclV1,
    ty: SemanticTypeIdV1,
    role: SemanticLocalRoleV1,
) -> Option<SemanticLocalIdV1> {
    let mut locals = body
        .locals()
        .iter()
        .enumerate()
        .filter(|(_, l)| l.role() == role);
    let (index, local) = locals.next()?;
    (local.ty() == ty && locals.next().is_none())
        .then_some(SemanticLocalIdV1::from_index(index as u32))
}

fn shared(
    declarations: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
    pointee: SemanticTypeIdV1,
) -> bool {
    matches!(declarations.get(reference.index() as usize).map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Pointer(p)) if p.pointee() == pointee
            && p.kind() == SemanticPointerKindV1::Reference
            && p.mutability() == SemanticMutabilityV1::Immutable
            && p.address_space() == 0 && p.pointer_width_bits() == 64
            && p.metadata() == SemanticPointerMetadataV1::None)
}

pub(super) fn option(
    declarations: &[SemanticTypeDeclV1],
    id: SemanticTypeIdV1,
    payload: SemanticTypeIdV1,
) -> bool {
    matches!(declarations.get(id.index() as usize).map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Enum { variants, .. }) if variants.len() == 2
            && variants[0].discriminant() == 0 && variants[0].fields().fields().is_empty()
            && !variants[0].is_uninhabited() && !variants[1].is_uninhabited()
            && variants[1].discriminant() == 1 && variants[1].fields().fields() == [payload])
}

pub(super) fn types_match(
    declarations: &[SemanticTypeDeclV1],
    t: SemanticGuardedGridLeaderTypesV1,
) -> bool {
    let ids = t.all();
    if !ids.iter().enumerate().all(|(i, id)| {
        declarations.get(id.index() as usize).is_some_and(|ty| {
            ty.identity().as_bytes() != &[0; 32]
                && !ty.layout().is_uninhabited()
                && !ids[..i].contains(id)
                && ids[..i]
                    .iter()
                    .all(|old| declarations[old.index() as usize].identity() != ty.identity())
        })
    }) {
        return false;
    }
    let zst = |id: SemanticTypeIdV1| {
        declarations.get(id.index() as usize).is_some_and(|ty| {
            ty.layout().size_bytes() == Some(0)
                && ty.layout().alignment_bytes() == 1
                && !ty.layout().is_uninhabited()
        })
    };
    shared(declarations, t.grid_reference, t.grid)
        && matches!(declarations[t.grid.index() as usize].shape(), SemanticTypeShapeV1::Aggregate(a)
            if a.fields().len() == 4 && a.fields()[..2] == [t.rank, t.rank]
                && a.fields()[2..].iter().all(|ty| zst(*ty)))
        && declarations[t.grid.index() as usize].layout().size_bytes() == Some(16)
        && declarations[t.grid.index() as usize]
            .layout()
            .alignment_bytes()
            == 8
        && matches!(declarations[t.leader.index() as usize].shape(), SemanticTypeShapeV1::Aggregate(a)
            if a.fields().len() == 3 && a.fields().iter().all(|ty| zst(*ty)))
        && zst(t.leader)
        && matches!(
            declarations[t.rank.index() as usize].shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64
            })
        )
        && matches!(
            declarations[t.boolean.index() as usize].shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
        )
        && option(declarations, t.leader_option, t.leader)
        && option(declarations, t.grid_option, t.grid)
        && matches!(declarations[t.context_reference.index() as usize].shape(),
            SemanticTypeShapeV1::Pointer(p) if shared(declarations, t.context_reference, p.pointee())
                && zst(p.pointee()))
}

fn abi(
    body: &SemanticFunctionDeclV1,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
    ignored: bool,
) -> bool {
    let a = body.abi();
    a.canon_abi() == SemanticCanonAbiV1::Rust
        && a.extern_abi() == SemanticExternAbiV1::Rust
        && !a.can_unwind()
        && !a.c_variadic()
        && a.hidden_arguments().is_empty()
        && a.fixed_count() as usize == inputs.len()
        && a.source_input_types() == inputs
        && a.source_output_type() == output
        && a.arguments().len() == inputs.len()
        && a.source_argument_ownership().len() == inputs.len()
        && a.source_argument_ownership()
            .iter()
            .all(|o| *o == SemanticSourceArgumentOwnershipV1::SharedBorrow)
        && a.arguments().iter().zip(inputs).all(|(a, t)| {
            a.role() == SemanticAbiArgumentRoleV1::Source
                && a.ty() == *t
                && matches!(a.value().mode(), SemanticAbiPassModeV1::Direct(_))
                && a.value().adjusted().is_none()
                && a.value().pointee_override().is_none()
        })
        && a.return_value().ty() == output
        && a.return_value().adjusted().is_none()
        && a.return_value().pointee_override().is_none()
        && if ignored {
            matches!(a.return_value().mode(), SemanticAbiPassModeV1::Ignore)
        } else {
            matches!(a.return_value().mode(), SemanticAbiPassModeV1::Direct(_))
        }
}

pub(super) fn issuer_matches(body: &SemanticFunctionDeclV1, leader: SemanticTypeIdV1) -> bool {
    body.role() == SemanticFunctionRoleV1::InternalHelper
        && body.export().is_none()
        && body.blocks().len() == 1
        && body.locals().len() == 1
        && role(body, leader, SemanticLocalRoleV1::Return).is_some()
        && abi(body, &[], leader, true)
        && body
            .blocks()
            .get(body.entry().index() as usize)
            .is_some_and(|b| {
                b.statements().is_empty()
                    && matches!(b.terminator().kind(), SemanticTerminatorKindV1::Return)
            })
}

pub(super) fn observe(
    body: &SemanticFunctionDeclV1,
    t: SemanticGuardedGridLeaderTypesV1,
) -> Option<SemanticGuardedGridLeaderBodyV1> {
    if body.role() != SemanticFunctionRoleV1::InternalHelper
        || body.export().is_some()
        || body.blocks().len() != 5
        || body.locals().len() != 5
        || !abi(body, &[t.grid_reference], t.leader_option, false)
    {
        return None;
    }
    let receiver = role(body, t.grid_reference, SemanticLocalRoleV1::Argument(0))?;
    let result = role(body, t.leader_option, SemanticLocalRoleV1::Return)?;
    let entry = body.blocks().get(body.entry().index() as usize)?;
    let [read, compare] = entry.statements() else {
        return None;
    };
    let SemanticStatementKindV1::Assign(read) = read.kind() else {
        return None;
    };
    let rank = plain(read.destination(), t.rank)?;
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = read.value().kind() else {
        return None;
    };
    let [deref, field] = place.projections() else {
        return None;
    };
    if read.value().result_type() != t.rank
        || place.local() != receiver
        || place.ty() != t.rank
        || deref.kind() != SemanticProjectionKindV1::Dereference
        || deref.result_type() != t.grid
        || field.kind() != SemanticProjectionKindV1::Field(1)
        || field.result_type() != t.rank
    {
        return None;
    }
    let SemanticStatementKindV1::Assign(compare) = compare.kind() else {
        return None;
    };
    let flag = plain(compare.destination(), t.boolean)?;
    let SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::Equal,
        left,
        right,
    } = compare.value().kind()
    else {
        return None;
    };
    if compare.value().result_type() != t.boolean
        || operand_local(left, t.rank) != Some(rank)
        || !matches!(right, SemanticOperandV1::Constant(c) if c.ty() == t.rank
            && matches!(c.value(), SemanticConstantValueV1::Scalar(s) if s.bits() == 0 && s.size_bytes()==8))
    {
        return None;
    }
    let SemanticTerminatorKindV1::SwitchInt {
        discriminant,
        targets,
    } = entry.terminator().kind()
    else {
        return None;
    };
    if operand_local(discriminant, t.boolean) != Some(flag) {
        return None;
    }
    let [zero] = targets.values() else {
        return None;
    };
    if zero.value() != 0
        || zero.edge().role() != SemanticEdgeRoleV1::SwitchValue
        || targets.otherwise().role() != SemanticEdgeRoleV1::SwitchOtherwise
    {
        return None;
    }
    let none = zero.edge().target();
    let issuer = targets.otherwise().target();
    let issue_block = body.blocks().get(issuer.index() as usize)?;
    if !issue_block.statements().is_empty() {
        return None;
    }
    let SemanticTerminatorKindV1::Call(call) = issue_block.terminator().kind() else {
        return None;
    };
    if !call.arguments().is_empty() || call.unwind() != SemanticUnwindActionV1::Unreachable {
        return None;
    }
    let destination = call.destination()?;
    if destination.edge().role() != SemanticEdgeRoleV1::CallReturn {
        return None;
    }
    let issued = plain(destination.place(), t.leader)?;
    let some = destination.edge().target();
    let some_block = body.blocks().get(some.index() as usize)?;
    let none_block = body.blocks().get(none.index() as usize)?;
    let variant = |b: &SemanticBasicBlockV1, tag: u32| -> Option<()> {
        let [statement] = b.statements() else {
            return None;
        };
        let SemanticStatementKindV1::Assign(a) = statement.kind() else {
            return None;
        };
        let SemanticRvalueKindV1::Aggregate(v) = a.value().kind() else {
            return None;
        };
        if plain(a.destination(), t.leader_option) != Some(result)
            || a.value().result_type() != t.leader_option
            || *v.kind() != SemanticAggregateKindV1::EnumVariant(tag)
        {
            return None;
        }
        if tag == 0 {
            return v.operands().is_empty().then_some(());
        }
        matches!(v.operands(), [SemanticOperandV1::Constant(c)] if c.ty() == t.leader
            && matches!(c.value(), SemanticConstantValueV1::ZeroSized))
        .then_some(())
    };
    variant(some_block, 1)?;
    variant(none_block, 0)?;
    let SemanticTerminatorKindV1::Goto(a) = some_block.terminator().kind() else {
        return None;
    };
    let SemanticTerminatorKindV1::Goto(b) = none_block.terminator().kind() else {
        return None;
    };
    if a.target() != b.target()
        || a.role() != SemanticEdgeRoleV1::Goto
        || b.role() != SemanticEdgeRoleV1::Goto
    {
        return None;
    }
    let exit = a.target();
    let exit_block = body.blocks().get(exit.index() as usize)?;
    if !exit_block.statements().is_empty()
        || !matches!(
            exit_block.terminator().kind(),
            SemanticTerminatorKindV1::Return
        )
    {
        return None;
    }
    let blocks = [body.entry(), issuer, some, none, exit];
    let locals = [
        (receiver, t.grid_reference, SemanticLocalRoleV1::Argument(0)),
        (result, t.leader_option, SemanticLocalRoleV1::Return),
        (rank, t.rank, SemanticLocalRoleV1::Temporary),
        (flag, t.boolean, SemanticLocalRoleV1::Temporary),
        (issued, t.leader, SemanticLocalRoleV1::Temporary),
    ];
    if !blocks
        .iter()
        .enumerate()
        .all(|(i, b)| !blocks[..i].contains(b))
        || !locals.iter().enumerate().all(|(i, (l, t, r))| {
            !locals[..i].iter().any(|old| old.0 == *l)
                && body
                    .locals()
                    .get(l.index() as usize)
                    .is_some_and(|decl| decl.ty() == *t && decl.role() == *r)
        })
    {
        return None;
    }
    Some(SemanticGuardedGridLeaderBodyV1 {
        guard: body.entry(),
        issuer,
        some,
        none,
        exit,
        receiver,
        result,
        issued,
        issuer_callable: call.callee(),
    })
}
