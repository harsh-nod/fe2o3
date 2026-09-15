//! Closed semantic recipes. Source identities still require live reconstruction;
//! neither a body hash nor a same-shaped zero-size value is an issuer.
use super::*;
type Result<T> = std::result::Result<T, SemanticMirErrorV1>;
use SemanticDefinedReusablePhaseRecipeV1 as R;

fn local(
    f: &SemanticFunctionDeclV1,
    role: SemanticLocalRoleV1,
    ty: SemanticTypeIdV1,
) -> Result<SemanticLocalIdV1> {
    let mut found = None;
    for (index, l) in f.locals().iter().enumerate() {
        if l.role() == role {
            require(l.ty() == ty && found.is_none())?;
            found = Some(SemanticLocalIdV1::from_index(index as u32));
        }
    }
    found.ok_or(SemanticMirErrorV1::InvalidFunctionAbi)
}
fn whole(p: &SemanticPlaceV1, l: SemanticLocalIdV1) -> bool {
    p.local() == l && p.projections().is_empty()
}
fn temporary(f: &SemanticFunctionDeclV1, l: SemanticLocalIdV1) -> bool {
    f.locals()
        .get(l.index() as usize)
        .is_some_and(|local| local.role() == SemanticLocalRoleV1::Temporary)
}
fn moved(op: &SemanticOperandV1, l: SemanticLocalIdV1) -> bool {
    matches!(op, SemanticOperandV1::Move(p) if whole(p,l))
}
fn copied(op: &SemanticOperandV1, l: SemanticLocalIdV1) -> bool {
    matches!(op, SemanticOperandV1::Copy(p) if whole(p,l))
}
fn zst(op: &SemanticOperandV1, ty: SemanticTypeIdV1) -> bool {
    matches!(op, SemanticOperandV1::Constant(c) if c.ty() == ty && matches!(c.value(), SemanticConstantValueV1::ZeroSized))
}
fn assigned(s: &SemanticStatementV1) -> Result<&SemanticAssignmentV1> {
    match s.kind() {
        SemanticStatementKindV1::Assign(a) => Ok(a),
        _ => Err(SemanticMirErrorV1::InvalidFunctionAbi),
    }
}

fn scalar_read_type(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    let scalar = |ty: SemanticTypeIdV1| types.get(ty.index() as usize).is_some_and(|ty|
        matches!(ty.shape(), SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)));
    scalar(ty) || types.get(ty.index() as usize).is_some_and(|ty|
        matches!(ty.shape(), SemanticTypeShapeV1::Pointer(p)
            if p.kind() == SemanticPointerKindV1::Reference
                && p.mutability() == SemanticMutabilityV1::Immutable
                && p.metadata() == SemanticPointerMetadataV1::None
                && scalar(p.pointee())))
}

fn completion_prefix(
    locals: &[SemanticLocalDeclV1],
    types: &[SemanticTypeDeclV1],
    statements: &[SemanticStatementV1],
    pack: u32,
    result: SemanticLocalIdV1,
    completion: SemanticLocalIdV1,
    work: &mut u64,
) -> Result<()> {
    require(statements.len() == pack as usize + 1)?;
    for statement in &statements[..pack as usize] {
        spend(work, 1)?;
        let assignment = assigned(statement)?;
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(source)) = assignment.value().kind() else {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        };
        spend(work, source.projections().len())?;
        let destination = assignment.destination();
        require(destination.projections().is_empty()
            && locals.get(destination.local().index() as usize).is_some_and(|local|
                local.role() == SemanticLocalRoleV1::Temporary && local.ty() == destination.ty())
            && destination.local() != result && destination.local() != completion
            && source.local() != result && source.local() != completion
            && source.ty() == destination.ty()
            && assignment.value().result_type() == destination.ty()
            && scalar_read_type(types, destination.ty()))?;
    }
    Ok(())
}
fn block(f: &SemanticFunctionDeclV1, b: SemanticBlockIdV1) -> Result<&SemanticBasicBlockV1> {
    f.blocks()
        .get(b.index() as usize)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)
}
fn call(
    f: &SemanticFunctionDeclV1,
    b: SemanticBlockIdV1,
    c: SemanticPhaseCallableV1,
    count: usize,
) -> Result<&SemanticDirectCallV1> {
    let SemanticTerminatorKindV1::Call(call) = block(f, b)?.terminator().kind() else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    require(
        call.callee() == c.callable
            && call.arguments().len() == count
            && call.variadic_argument_abis().is_empty()
            && call.unwind() == SemanticUnwindActionV1::Unreachable,
    )?;
    let dest = call
        .destination()
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    require(
        dest.place().projections().is_empty()
            && dest.edge().role() == SemanticEdgeRoleV1::CallReturn,
    )?;
    Ok(call)
}
fn returning(f: &SemanticFunctionDeclV1, b: SemanticBlockIdV1) -> Result<()> {
    let b = block(f, b)?;
    require(
        b.statements().is_empty()
            && matches!(b.terminator().kind(), SemanticTerminatorKindV1::Return),
    )
}
fn chain(f: &SemanticFunctionDeclV1, path: &[SemanticBlockIdV1], work: &mut u64) -> Result<()> {
    require(path.first() == Some(&f.entry()) && path.len() == f.blocks().len())?;
    for (index, b) in path.iter().enumerate() {
        spend(work, index + 1)?;
        require(!path[..index].contains(b))?;
        for (from, data) in f.blocks().iter().enumerate() {
            spend(work, 1)?;
            data.terminator()
                .kind()
                .try_for_each_edge::<SemanticMirErrorV1>(|edge| {
                    spend(work, 1)?;
                    if edge.target() == *b {
                        require(
                            index != 0
                                && from == path[index - 1].index() as usize
                                && edge.role() == SemanticEdgeRoleV1::CallReturn,
                        )?;
                    }
                    Ok(())
                })?;
        }
    }
    Ok(())
}

fn signature(
    f: &SemanticFunctionDeclV1,
    inputs: &[(SemanticTypeIdV1, SemanticSourceArgumentOwnershipV1)],
    out: SemanticTypeIdV1,
    locals: usize,
    blocks: usize,
    work: &mut u64,
) -> Result<()> {
    spend(
        work,
        f.locals()
            .len()
            .checked_add(f.blocks().len())
            .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?,
    )?;
    let abi = f.abi();
    require(
        f.role() == SemanticFunctionRoleV1::InternalHelper
            && f.export().is_none()
            && f.locals().len() == locals
            && f.blocks().len() == blocks
            && abi.extern_abi() == SemanticExternAbiV1::Rust
            && !abi.can_unwind()
            && !abi.c_variadic()
            && abi.source_input_types().len() == inputs.len()
            && abi.arguments().len() == inputs.len()
            && abi.source_argument_ownership().len() == inputs.len()
            && abi.source_output_type() == out
            && abi.return_value().ty() == out
            && abi.return_value().adjusted().is_none()
            && abi.return_value().pointee_override().is_none(),
    )?;
    local(f, SemanticLocalRoleV1::Return, out)?;
    for (index, (ty, ownership)) in inputs.iter().enumerate() {
        require(
            abi.source_input_types()[index] == *ty
                && abi.source_argument_ownership()[index] == *ownership
                && abi.arguments()[index].ty() == *ty
                && abi.arguments()[index].value().adjusted().is_none(),
        )?;
        local(f, SemanticLocalRoleV1::Argument(index as u32), *ty)?;
    }
    Ok(())
}

pub(super) fn validate(f: &SemanticFunctionDeclV1, recipe: R, work: &mut u64) -> Result<()> {
    use SemanticSourceArgumentOwnershipV1::{ByValue, SharedBorrow, UniqueBorrow};
    match recipe {
        R::OwnerConvert {
            workgroup, owner, ..
        } => {
            signature(f, &[(workgroup, ByValue)], owner, 4, 1, work)?;
            require(
                matches!(
                    f.abi().arguments()[0].value().mode(),
                    SemanticAbiPassModeV1::Pair { .. }
                ) && matches!(
                    f.abi().return_value().mode(),
                    SemanticAbiPassModeV1::Pair { .. }
                ),
            )?;
            constructor(f, workgroup, owner, false, work)
        }
        R::Issue {
            owner_reference,
            phase_workgroup,
            ..
        } => {
            signature(
                f,
                &[(owner_reference.reference, UniqueBorrow)],
                phase_workgroup,
                4,
                1,
                work,
            )?;
            require(
                matches!(
                    f.abi().arguments()[0].value().mode(),
                    SemanticAbiPassModeV1::Direct(..)
                ) && matches!(
                    f.abi().return_value().mode(),
                    SemanticAbiPassModeV1::Pair { .. }
                ),
            )?;
            constructor(f, owner_reference.reference, phase_workgroup, true, work)
        }
        R::Bind {
            phase_reference,
            storage_reference,
            phase_lds,
            ..
        } => {
            signature(
                f,
                &[
                    (phase_reference.reference, SharedBorrow),
                    (storage_reference.reference, UniqueBorrow),
                ],
                phase_lds,
                3,
                1,
                work,
            )?;
            require(
                f.abi()
                    .arguments()
                    .iter()
                    .all(|a| matches!(a.value().mode(), SemanticAbiPassModeV1::Direct(..)))
                    && matches!(f.abi().return_value().mode(), SemanticAbiPassModeV1::Ignore),
            )?;
            returning(f, f.entry())
        }
        R::Finish {
            workgroup_before_barrier,
            workgroup_after_barrier,
            completion,
            barrier,
            barrier_block,
            return_block,
            ..
        } => {
            signature(
                f,
                &[(workgroup_before_barrier, ByValue)],
                completion,
                3,
                2,
                work,
            )?;
            require(
                f.entry() == barrier_block
                    && block(f, barrier_block)?.statements().is_empty()
                    && matches!(
                        f.abi().arguments()[0].value().mode(),
                        SemanticAbiPassModeV1::Pair { .. }
                    )
                    && matches!(f.abi().return_value().mode(), SemanticAbiPassModeV1::Ignore),
            )?;
            let input = local(
                f,
                SemanticLocalRoleV1::Argument(0),
                workgroup_before_barrier,
            )?;
            let c = call(f, barrier_block, barrier, 1)?;
            let dst = c.destination().unwrap();
            require(
                (moved(&c.arguments()[0], input) || copied(&c.arguments()[0], input))
                    && dst.place().ty() == workgroup_after_barrier
                    && dst.edge().target() == return_block
                    && temporary(f, dst.place().local()),
            )?;
            returning(f, return_block)?;
            chain(f, &[barrier_block, return_block], work)
        }
        R::WithPhase {
            owner_reference,
            closure,
            call_tuple,
            phase_workgroup,
            completion,
            result_pair,
            result,
            drop_result,
            issue,
            invoke,
            drop_completion,
            issue_block,
            invoke_block,
            drop_block,
            relay,
            ..
        } => {
            signature(
                f,
                &[
                    (owner_reference.reference, UniqueBorrow),
                    (closure, ByValue),
                ],
                result,
                7,
                4,
                work,
            )?;
            require(f.entry() == issue_block && block(f, issue_block)?.statements().is_empty())?;
            let owner = local(
                f,
                SemanticLocalRoleV1::Argument(0),
                owner_reference.reference,
            )?;
            let environment = local(f, SemanticLocalRoleV1::Argument(1), closure)?;
            let ret = local(f, SemanticLocalRoleV1::Return, result)?;
            let a = call(f, issue_block, issue, 1)?;
            let b = call(f, invoke_block, invoke, 2)?;
            let c = call(f, drop_block, drop_completion, 1)?;
            let ad = a.destination().unwrap();
            let bd = b.destination().unwrap();
            let cd = c.destination().unwrap();
            require(
                copied(&a.arguments()[0], owner)
                    && ad.place().ty() == phase_workgroup
                    && ad.edge().target() == invoke_block
                    && (copied(&b.arguments()[0], environment)
                        || moved(&b.arguments()[0], environment))
                    && bd.place().ty() == result_pair
                    && bd.edge().target() == drop_block
                    && cd.place().ty() == drop_result,
            )?;
            let [tuple] = block(f, invoke_block)?.statements() else {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            };
            let tuple = assigned(tuple)?;
            let SemanticRvalueKindV1::Aggregate(aggregate) = tuple.value().kind() else {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            };
            require(
                tuple.destination().projections().is_empty()
                    && tuple.destination().ty() == call_tuple
                    && aggregate.kind() == &SemanticAggregateKindV1::Tuple
                    && aggregate.operands().len() == 1
                    && moved(&aggregate.operands()[0], ad.place().local())
                    && moved(&b.arguments()[1], tuple.destination().local()),
            )?;
            let [result_assignment] = block(f, drop_block)?.statements() else {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            };
            let result_assignment = assigned(result_assignment)?;
            require(
                whole(result_assignment.destination(), ret)
                    && matches!(result_assignment.value().kind(),SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p))
                    if field(p,bd.place().local(),1,result)),
            )?;
            match relay.wrapper_drop {
                SemanticPhaseCompletionDropV1::RetainedPairField => require(
                    matches!(&c.arguments()[0],SemanticOperandV1::Move(p) if field(p,bd.place().local(),0,completion)),
                )?,
                SemanticPhaseCompletionDropV1::ErasedZstConstant { canonical_operand } => require(
                    zst(&c.arguments()[0], completion)
                        && operand_digest(&c.arguments()[0], work)? == canonical_operand,
                )?,
            }
            let roles = [
                ad.place().local(),
                tuple.destination().local(),
                bd.place().local(),
                cd.place().local(),
            ];
            for (i, id) in roles.iter().enumerate() {
                spend(work, i + 1)?;
                require(!roles[..i].contains(id) && temporary(f, *id))?;
            }
            returning(f, cd.edge().target())?;
            chain(
                f,
                &[issue_block, invoke_block, drop_block, cd.edge().target()],
                work,
            )
        }
    }
}

fn field(p: &SemanticPlaceV1, base: SemanticLocalIdV1, index: u32, ty: SemanticTypeIdV1) -> bool {
    p.local() == base
        && p.ty() == ty
        && p.projections().len() == 1
        && p.projections()[0].kind() == SemanticProjectionKindV1::Field(index)
}

fn constructor(
    f: &SemanticFunctionDeclV1,
    input: SemanticTypeIdV1,
    output: SemanticTypeIdV1,
    borrowed: bool,
    work: &mut u64,
) -> Result<()> {
    let arg = local(f, SemanticLocalRoleV1::Argument(0), input)?;
    let ret = local(f, SemanticLocalRoleV1::Return, output)?;
    let entry = block(f, f.entry())?;
    require(matches!(
        entry.terminator().kind(),
        SemanticTerminatorKindV1::Return
    ))?;
    let [first, second, result] = entry.statements() else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    let mut values = [ret; 2];
    for (index, s) in [first, second].iter().enumerate() {
        spend(work, 1)?;
        let a = assigned(s)?;
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(p)) = a.value().kind() else {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        };
        require(
            p.local() == arg
                && p.projections().len() == usize::from(borrowed) + 1
                && p.projections()[usize::from(borrowed)].kind()
                    == SemanticProjectionKindV1::Field(index as u32)
                && (!borrowed
                    || p.projections()[0].kind() == SemanticProjectionKindV1::Dereference)
                && a.destination().projections().is_empty()
                && temporary(f, a.destination().local()),
        )?;
        values[index] = a.destination().local();
    }
    require(values[0] != values[1])?;
    let a = assigned(result)?;
    let SemanticRvalueKindV1::Aggregate(v) = a.value().kind() else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    require(
        whole(a.destination(), ret)
            && v.kind() == &SemanticAggregateKindV1::Aggregate
            && v.operands().len() == 4
            && moved(&v.operands()[0], values[0])
            && moved(&v.operands()[1], values[1])
            && zst(&v.operands()[2], v.operands()[2].ty())
            && zst(&v.operands()[3], v.operands()[3].ty()),
    )
}

pub(super) fn operand_digest(op: &SemanticOperandV1, work: &mut u64) -> Result<[u8; 32]> {
    use sha2::{Digest, Sha256};
    let mut writer = CanonicalWriterV1::new((*work / 2).min(HARD_MAX_CANONICAL_BYTES_V1));
    encode_operand(&mut writer, op)?;
    let bytes = writer.finish();
    spend(
        work,
        bytes
            .len()
            .checked_mul(2)
            .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?,
    )?;
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/SEMANTIC-REUSABLE-PHASE-OPERAND/V25\0");
    hash.update(&bytes);
    Ok(hash.finalize().into())
}

fn fields(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> Result<&[SemanticTypeIdV1]> {
    match types
        .get(ty.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    {
        Some(SemanticTypeShapeV1::Aggregate(a) | SemanticTypeShapeV1::Tuple(a)) => Ok(a.fields()),
        _ => Err(SemanticMirErrorV1::InvalidFunctionAbi),
    }
}
fn empty(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    types.get(ty.index() as usize).is_some_and(|t| {
        t.layout().size_bytes() == Some(0)
            && t.layout().alignment_bytes() == 1
            && !t.layout().is_uninhabited()
            && matches!(t.shape(), SemanticTypeShapeV1::Unit)
            || t.layout().size_bytes() == Some(0)
                && t.layout().alignment_bytes() == 1
                && !t.layout().is_uninhabited()
                && matches!(t.shape(),SemanticTypeShapeV1::Aggregate(a) if a.fields().is_empty())
    })
}
fn reference(
    types: &[SemanticTypeDeclV1],
    r: SemanticPhaseReferenceV1,
    pointee: SemanticTypeIdV1,
    kind: SemanticPhaseReferenceKindV1,
) -> Result<()> {
    require(r.pointee == pointee && r.kind == kind)?;
    let Some(SemanticTypeShapeV1::Pointer(p)) = types
        .get(r.reference.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    require(
        p.pointee() == pointee
            && p.kind() == SemanticPointerKindV1::Reference
            && p.metadata() == SemanticPointerMetadataV1::None
            && p.mutability()
                == match kind {
                    SemanticPhaseReferenceKindV1::Shared => SemanticMutabilityV1::Immutable,
                    SemanticPhaseReferenceKindV1::Unique => SemanticMutabilityV1::Mutable,
                },
    )
}
pub(super) fn validate_types(types: &[SemanticTypeDeclV1], r: R, work: &mut u64) -> Result<()> {
    r.try_visit_types(|id| {
        spend(work, 1)?;
        require((id.index() as usize) < types.len())
    })?;
    let physical = |ty| -> Result<()> {
        let f = fields(types, ty)?;
        require(
            f.len() == 4
                && f[0] == f[1]
                && types.get(f[0].index() as usize).is_some_and(|t| {
                    matches!(
                        t.shape(),
                        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                            signed: false,
                            bits: 64
                        })
                    )
                })
                && empty(types, f[3]),
        )
    };
    match r {
        R::OwnerConvert {
            workgroup, owner, ..
        } => {
            physical(workgroup)?;
            physical(owner)
        }
        R::Issue {
            owner_reference,
            owner,
            phase_workgroup,
            ..
        } => {
            reference(
                types,
                owner_reference,
                owner,
                SemanticPhaseReferenceKindV1::Unique,
            )?;
            physical(owner)?;
            physical(phase_workgroup)
        }
        R::WithPhase {
            owner_reference,
            owner,
            call_tuple,
            phase_workgroup,
            completion,
            result_pair,
            result,
            drop_result,
            ..
        } => {
            reference(
                types,
                owner_reference,
                owner,
                SemanticPhaseReferenceKindV1::Unique,
            )?;
            physical(owner)?;
            physical(phase_workgroup)?;
            require(
                fields(types, call_tuple)? == [phase_workgroup]
                    && fields(types, result_pair)? == [completion, result]
                    && fields(types, completion)?.len() == 3
                    && fields(types, completion)?
                        .iter()
                        .all(|id| empty(types, *id))
                    && matches!(
                        types[drop_result.index() as usize].shape(),
                        SemanticTypeShapeV1::Unit
                    ),
            )
        }
        R::Bind {
            phase_reference,
            storage_reference,
            phase_workgroup,
            reusable_storage,
            phase_lds,
            element,
            uninitialized_marker,
            storage_marker,
            thread_marker,
            elements,
            ..
        } => {
            reference(
                types,
                phase_reference,
                phase_workgroup,
                SemanticPhaseReferenceKindV1::Shared,
            )?;
            reference(
                types,
                storage_reference,
                reusable_storage,
                SemanticPhaseReferenceKindV1::Unique,
            )?;
            physical(phase_workgroup)?;
            let s = fields(types, reusable_storage)?;
            let l = fields(types, phase_lds)?;
            require(
                s.len() == 3
                    && l.len() == 5
                    && s[0] == storage_marker
                    && l[0] == storage_marker
                    && l[1] == uninitialized_marker
                    && s[2] == thread_marker
                    && l[4] == thread_marker
                    && s.iter().chain(l).all(|id| empty(types, *id))
                    && elements != 0
                    && types[element.index() as usize]
                        .layout()
                        .size_bytes()
                        .is_some_and(|size| size != 0 && size.checked_mul(elements).is_some()),
            )
        }
        R::Finish {
            workgroup_before_barrier,
            workgroup_after_barrier,
            completion,
            input_epoch,
            advanced_epoch,
            ..
        } => {
            physical(workgroup_before_barrier)?;
            physical(workgroup_after_barrier)?;
            require(
                input_epoch != advanced_epoch
                    && fields(types, completion)?.len() == 3
                    && fields(types, completion)?
                        .iter()
                        .all(|id| empty(types, *id)),
            )
        }
    }
}

pub(super) fn validate_calls(
    functions: &[SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    types: &[SemanticTypeDeclV1],
    provenance: SemanticKernelCapabilityProvenanceV1,
    r: R,
    dependencies: &[SemanticDefinedReusablePhaseV1],
    work: &mut u64,
) -> Result<()> {
    r.try_visit_callables(|c| {
        spend(work, 1)?;
        let (identity, abi) = match callables.get(c.callable.index() as usize) {
            Some(SemanticCallableDeclV1::Defined { function }) => {
                let f = functions
                    .get(function.index() as usize)
                    .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
                (f.identity(), f.abi().identity())
            }
            Some(c) => {
                let b = c.binding().ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
                (b.identity(), b.abi().identity())
            }
            None => return Err(SemanticMirErrorV1::InvalidFunctionAbi),
        };
        require(identity == c.identity && abi == c.abi)
    })?;
    match r {
        R::Finish {
            barrier,
            workgroup_before_barrier,
            workgroup_after_barrier,
            brands,
            input_epoch,
            advanced_epoch,
            ..
        } => {
            let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
                ..
            }) = callables.get(barrier.callable.index() as usize)
            else {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            };
            let SemanticExecutionCapabilityOperationV1::WorkgroupBarrier {
                input_workgroup,
                output_workgroup,
                semantics,
            } = contract.operation()
            else {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            };
            require(
                input_workgroup == workgroup_before_barrier
                    && output_workgroup == workgroup_after_barrier
                    && semantics
                        == SemanticExecutionMemorySemanticsV1::new(
                            SemanticExecutionMemoryScopeV1::Workgroup,
                            SemanticExecutionMemoryOrderingV1::AcquireRelease,
                            SemanticExecutionMemorySpacesV1::Workgroup,
                        )
                    && contract.provenance() == provenance
                    && contract.workgroup_brand()
                        == Some(types[brands.phase_brand.index() as usize].identity())
                    && contract.epoch_before()
                        == Some(types[input_epoch.index() as usize].identity())
                    && contract.epoch_after()
                        == Some(types[advanced_epoch.index() as usize].identity()),
            )
        }
        R::WithPhase {
            relay,
            issue,
            invoke,
            owner_reference,
            owner,
            phase_workgroup,
            brands,
            completion,
            result,
            result_pair,
            ..
        } => {
            let issue_record =
                defined_recipe(functions, callables, types, issue, dependencies, work)?;
            require(
                issue_record.provenance() == provenance
                    && issue_record.recipe()
                        == R::Issue {
                            owner_reference,
                            owner,
                            phase_workgroup,
                            brands,
                        },
            )?;
            let finish_record = defined_recipe(
                functions,
                callables,
                types,
                relay.finish,
                dependencies,
                work,
            )?;
            require(
                finish_record.provenance() == provenance
                    && matches!(finish_record.recipe(),R::Finish{completion:c,brands:b,..} if c==completion && b==brands),
            )?;
            require(
                callables.get(invoke.callable.index() as usize)
                    == Some(&SemanticCallableDeclV1::Defined {
                        function: relay.closure_function,
                    }),
            )?;
            let closure = functions
                .get(relay.closure_function.index() as usize)
                .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
            let (hash, bytes) = canonical_semantic_source_body_sha256_v25(closure, *work / 2)?;
            spend(
                work,
                bytes
                    .checked_mul(2)
                    .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?,
            )?;
            require(hash == relay.closure_body)?;
            let finish = call(closure, relay.finish_call_block, relay.finish, 1)?;
            let fd = finish.destination().unwrap();
            require(
                fd.place().ty() == completion && fd.edge().target() == relay.finish_normal_target,
            )?;
            let statements = block(closure, relay.closure_pack_block)?.statements();
            let ret = local(closure, SemanticLocalRoleV1::Return, result_pair)?;
            completion_prefix(closure.locals(), types, statements, relay.closure_pack_statement,
                ret, fd.place().local(), work)?;
            let pack = statements
                .get(relay.closure_pack_statement as usize)
                .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
            let pack = assigned(pack)?;
            let SemanticRvalueKindV1::Aggregate(a) = pack.value().kind() else {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            };
            require(
                whole(pack.destination(), ret)
                    && a.kind() == &SemanticAggregateKindV1::Tuple
                    && a.operands().len() == 2
                    && a.operands()[0].ty() == completion
                    && a.operands()[1].ty() == result,
            )?;
            match relay.completion_field {
                SemanticPhaseCompletionFieldV1::RetainedFinishResult => {
                    require(moved(&a.operands()[0], fd.place().local()))?
                }
                SemanticPhaseCompletionFieldV1::ErasedZstConstant { canonical_operand } => require(
                    zst(&a.operands()[0], completion)
                        && operand_digest(&a.operands()[0], work)? == canonical_operand,
                )?,
            }
            require(
                relay.closure_pack_block == relay.finish_normal_target
                    && relay.closure_return_block == relay.closure_pack_block
                    && matches!(
                        block(closure, relay.closure_return_block)?
                            .terminator()
                            .kind(),
                        SemanticTerminatorKindV1::Return
                    ),
            )?;
            // No other edge may enter the completion pack without this finish.
            require(closure.entry() != relay.closure_pack_block)?;
            for (index, b) in closure.blocks().iter().enumerate() {
                spend(work, 1)?;
                b.terminator()
                    .kind()
                    .try_for_each_edge::<SemanticMirErrorV1>(|edge| {
                        spend(work, 1)?;
                        if edge.target() == relay.closure_pack_block {
                            require(
                                index == relay.finish_call_block.index() as usize
                                    && edge.role() == SemanticEdgeRoleV1::CallReturn,
                            )?;
                        }
                        Ok(())
                    })?;
            }
            Ok(())
        }
        R::OwnerConvert { .. } | R::Issue { .. } | R::Bind { .. } => Ok(()),
    }
}

#[cfg(test)]
#[path = "completion_prefix_tests.rs"]
mod completion_prefix_tests;

fn defined_recipe(
    functions: &[SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    types: &[SemanticTypeDeclV1],
    c: SemanticPhaseCallableV1,
    dependencies: &[SemanticDefinedReusablePhaseV1],
    work: &mut u64,
) -> Result<SemanticDefinedReusablePhaseV1> {
    let Some(SemanticCallableDeclV1::Defined { function }) =
        callables.get(c.callable.index() as usize)
    else {
        return Err(SemanticMirErrorV1::InvalidFunctionAbi);
    };
    let attached = functions
        .get(function.index() as usize)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?
        .defined_capability_contract();
    let mut selected = None;
    for candidate in dependencies {
        spend(work, 1)?;
        if candidate.function() != *function {
            continue;
        }
        require(
            selected.is_none() && matches!(candidate.recipe(), R::Issue { .. } | R::Finish { .. }),
        )?;
        let fresh = SemanticDefinedReusablePhaseV1::for_defined_function(
            *function,
            functions,
            callables,
            types,
            candidate.provenance(),
            *candidate.source_binding(),
            candidate.recipe(),
            work,
        )?;
        require(
            fresh == *candidate
                && attached.is_none_or(|value| {
                    value == &SemanticDefinedCapabilityContractV1::ReusablePhase(*candidate)
                }),
        )?;
        selected = Some(*candidate);
    }
    if let Some(record) = selected {
        return Ok(record);
    }
    match attached {
        Some(SemanticDefinedCapabilityContractV1::ReusablePhase(record)) => Ok(*record),
        _ => Err(SemanticMirErrorV1::InvalidFunctionAbi),
    }
}
