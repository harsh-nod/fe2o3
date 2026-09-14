//! Exact borrowed payload comparisons; SSA values are checked separately by occurrence.
use super::{Budget, Error, Result};
use fe2o3_kernel_ir::{
    AssemblyOperandKind, BarrierSemantics, BinaryOp, CastKind, InlineAssembly, MatrixOperation,
    MatrixOperationKind, MemoryIntrinsicOperation, OperationKind, TargetCapability, Type, UnaryOp,
    VerificationContractOperationV12, WaveOperationKind,
};
use std::collections::BTreeSet;

pub(super) fn bytes(a: &[u8], b: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
    budget.charge_work(a.len().checked_add(b.len()).ok_or(Error::Arithmetic)?)?;
    Ok(a == b)
}

pub(super) fn ty(mut a: &Type, mut b: &Type, budget: &mut Budget<'_>) -> Result<bool> {
    loop {
        budget.charge_work(1)?;
        match (a, b) {
            (Type::Pointer(x), Type::Pointer(y))
                if x.address_space == y.address_space && x.access == y.access =>
            {
                a = &x.pointee;
                b = &y.pointee;
            }
            (Type::Slice(x), Type::Slice(y))
                if x.address_space == y.address_space && x.access == y.access =>
            {
                a = &x.element;
                b = &y.element;
            }
            (Type::Unit, Type::Unit) => return Ok(true),
            (Type::Scalar(x), Type::Scalar(y)) => return Ok(x == y),
            (Type::Vector(x), Type::Vector(y)) => return Ok(x == y),
            _ => return Ok(false),
        }
    }
}

pub(super) fn types(a: &[Type], b: &[Type], budget: &mut Budget<'_>) -> Result<bool> {
    budget.charge_work(1)?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.iter().zip(b) {
        if !ty(a, b, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn set<T: Eq>(a: &BTreeSet<T>, b: &BTreeSet<T>, budget: &mut Budget<'_>) -> Result<bool> {
    budget.charge_work(a.len().checked_add(b.len()).ok_or(Error::Arithmetic)?)?;
    Ok(a == b)
}

pub(super) fn capabilities(
    a: &BTreeSet<TargetCapability>,
    b: &BTreeSet<TargetCapability>,
    budget: &mut Budget<'_>,
) -> Result<bool> {
    budget.charge_work(1)?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.iter().zip(b) {
        budget.charge_work(1)?;
        if let TargetCapability::Extension { namespace, name } = a {
            let TargetCapability::Extension {
                namespace: other_namespace,
                name: other_name,
            } = b
            else {
                return Ok(false);
            };
            if !bytes(namespace.as_bytes(), other_namespace.as_bytes(), budget)?
                || !bytes(name.as_bytes(), other_name.as_bytes(), budget)?
            {
                return Ok(false);
            }
        } else if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn operation(
    a: &OperationKind,
    b: &OperationKind,
    budget: &mut Budget<'_>,
) -> Result<bool> {
    use OperationKind as K;
    budget.charge_work(1)?;
    Ok(match (a, b) {
        (
            K::VerificationContract(VerificationContractOperationV12::WorkgroupPipelineEvent {
                contract: ac,
                kind: ak,
                ..
            }),
            K::VerificationContract(VerificationContractOperationV12::WorkgroupPipelineEvent {
                contract: bc,
                kind: bk,
                ..
            }),
        ) => ac == bc && ak == bk,
        (K::VectorLoad(a), K::VectorLoad(b)) => a.access == b.access,
        (K::VectorStore(a), K::VectorStore(b)) => a.access == b.access,
        (K::VectorLayoutConvert(a), K::VectorLayoutConvert(b)) => a.to == b.to,
        (K::Constant(a), K::Constant(b)) => a == b,
        (K::Intrinsic(a), K::Intrinsic(b)) => {
            a.kind == b.kind && ty(&a.result_type, &b.result_type, budget)?
        }
        (K::MemoryIntrinsic(a), K::MemoryIntrinsic(b)) => memory_intrinsic(a, b),
        (K::Unary { op: a, .. }, K::Unary { op: b, .. }) => a == b,
        (K::Binary { op: a, .. }, K::Binary { op: b, .. }) => a == b,
        (K::Compare { predicate: a, .. }, K::Compare { predicate: b, .. }) => a == b,
        (
            K::Cast {
                kind: ak, to: a, ..
            },
            K::Cast {
                kind: bk, to: b, ..
            },
        ) => ak == bk && ty(a, b, budget)?,
        (K::Select { .. }, K::Select { .. })
        | (K::SliceLength { .. }, K::SliceLength { .. })
        | (K::SliceData { .. }, K::SliceData { .. })
        | (K::GetElementPointer { .. }, K::GetElementPointer { .. }) => true,
        (
            K::Call {
                callee: a,
                arguments: aa,
            },
            K::Call {
                callee: b,
                arguments: ba,
            },
        ) => aa.len() == ba.len() && bytes(a.as_str().as_bytes(), b.as_str().as_bytes(), budget)?,
        (
            K::Alloca {
                element: ae,
                count: ac,
                address_space: aa,
                alignment: al,
            },
            K::Alloca {
                element: be,
                count: bc,
                address_space: ba,
                alignment: bl,
            },
        ) => ac.is_some() == bc.is_some() && aa == ba && al == bl && ty(ae, be, budget)?,
        (K::Load { access: a, .. }, K::Load { access: b, .. })
        | (K::GuardedLoad { access: a, .. }, K::GuardedLoad { access: b, .. })
        | (K::GuardedStore { access: a, .. }, K::GuardedStore { access: b, .. })
        | (K::Store { access: a, .. }, K::Store { access: b, .. }) => a == b,
        (K::Barrier(a), K::Barrier(b)) => {
            a.execution_scope == b.execution_scope
                && a.memory_scope == b.memory_scope
                && semantics(&a.semantics, &b.semantics, budget)?
        }
        (K::Fence(a), K::Fence(b)) => {
            a.memory_scope == b.memory_scope && semantics(&a.semantics, &b.semantics, budget)?
        }
        (K::WorkgroupBarrier(a), K::WorkgroupBarrier(b)) => {
            a.memory_scope == b.memory_scope
                && a.convergence == b.convergence
                && semantics(&a.semantics, &b.semantics, budget)?
        }
        (K::WorkgroupMemory(a), K::WorkgroupMemory(b)) => {
            a.extent == b.extent
                && a.alignment == b.alignment
                && ty(&a.element, &b.element, budget)?
        }
        (K::Atomic(a), K::Atomic(b)) => {
            a.kind == b.kind
                && a.value.is_some() == b.value.is_some()
                && a.compare.is_some() == b.compare.is_some()
                && a.access == b.access
                && a.scope == b.scope
                && a.ordering == b.ordering
                && a.failure_ordering == b.failure_ordering
        }
        (K::Matrix(a), K::Matrix(b)) => matrix(a, b, budget)?,
        (K::Gfx950LdsTranspose(a), K::Gfx950LdsTranspose(b)) => {
            std::mem::discriminant(&a.kind) == std::mem::discriminant(&b.kind)
                && a.kind.format() == b.kind.format()
                && a.width == b.width
                && a.active_lanes == b.active_lanes
                && a.convergence == b.convergence
        }
        (K::Wave(a), K::Wave(b)) => {
            a.width == b.width
                && a.active_lanes == b.active_lanes
                && a.convergence == b.convergence
                && wave(a.kind, b.kind)
        }
        (K::InlineAssembly(a), K::InlineAssembly(b)) => assembly(a, b, budget)?,
        _ => false,
    })
}

fn semantics(a: &BarrierSemantics, b: &BarrierSemantics, budget: &mut Budget<'_>) -> Result<bool> {
    Ok(a.ordering == b.ordering && set(&a.address_spaces, &b.address_spaces, budget)?)
}

fn memory_intrinsic(a: &MemoryIntrinsicOperation, b: &MemoryIntrinsicOperation) -> bool {
    use MemoryIntrinsicOperation as M;
    match (a, b) {
        (
            M::PointerDistance {
                kind: ak,
                unit: au,
                element: ae,
                address_space: aa,
                layout: al,
                contract: ac,
                ..
            },
            M::PointerDistance {
                kind: bk,
                unit: bu,
                element: be,
                address_space: ba,
                layout: bl,
                contract: bc,
                ..
            },
        ) => (ak, au, ae, aa, al, ac) == (bk, bu, be, ba, bl, bc),
        (
            M::VolatileLoad {
                element: ae,
                address_space: aa,
                layout: al,
                contract: ac,
                ..
            },
            M::VolatileLoad {
                element: be,
                address_space: ba,
                layout: bl,
                contract: bc,
                ..
            },
        )
        | (
            M::VolatileStore {
                element: ae,
                address_space: aa,
                layout: al,
                contract: ac,
                ..
            },
            M::VolatileStore {
                element: be,
                address_space: ba,
                layout: bl,
                contract: bc,
                ..
            },
        ) => (ae, aa, al, ac) == (be, ba, bl, bc),
        (
            M::CopyNonOverlapping {
                element: ae,
                source_address_space: ass,
                destination_address_space: ads,
                layout: al,
                contract: ac,
                ..
            },
            M::CopyNonOverlapping {
                element: be,
                source_address_space: bs,
                destination_address_space: bd,
                layout: bl,
                contract: bc,
                ..
            },
        ) => (ae, ass, ads, al, ac) == (be, bs, bd, bl, bc),
        _ => false,
    }
}

fn matrix(a: &MatrixOperation, b: &MatrixOperation, budget: &mut Budget<'_>) -> Result<bool> {
    use MatrixOperationKind as M;
    let shape = match (&a.kind, &b.kind) {
        (M::MultiplyAccumulate { profile: a, .. }, M::MultiplyAccumulate { profile: b, .. })
        | (
            M::ScaledMultiplyAccumulate { profile: a, .. },
            M::ScaledMultiplyAccumulate { profile: b, .. },
        ) => a == b,
        (M::LdsLoad { profile: a, .. }, M::LdsLoad { profile: b, .. })
        | (M::LdsStore { profile: a, .. }, M::LdsStore { profile: b, .. }) => a == b,
        _ => false,
    };
    if !shape
        || a.active_lanes != b.active_lanes
        || a.convergence != b.convergence
        || a.tensor_layout != b.tensor_layout
    {
        return Ok(false);
    }
    for binding in [&a.frontend_binding, &b.frontend_binding]
        .into_iter()
        .flatten()
    {
        let observed = &binding.observed_source;
        let work = observed
            .provider
            .crate_name
            .len()
            .checked_add(observed.canonical_record.len())
            .and_then(|n| {
                observed
                    .provider
                    .definition_identities
                    .len()
                    .checked_mul(16)
                    .and_then(|ids| n.checked_add(ids))
            })
            .ok_or(Error::Arithmetic)?;
        budget.charge_work(work)?;
    }
    Ok(a.frontend_binding == b.frontend_binding)
}

fn wave(a: WaveOperationKind, b: WaveOperationKind) -> bool {
    use WaveOperationKind as W;
    match (a, b) {
        (W::LaneId, W::LaneId)
        | (W::Ballot { .. }, W::Ballot { .. })
        | (W::Any { .. }, W::Any { .. })
        | (W::All { .. }, W::All { .. }) => true,
        (W::ShuffleIndex { tile_width: a, .. }, W::ShuffleIndex { tile_width: b, .. })
        | (W::BroadcastF32 { tile_width: a, .. }, W::BroadcastF32 { tile_width: b, .. }) => a == b,
        (
            W::ReduceF32 {
                tile_width: a,
                kind: ak,
                ..
            },
            W::ReduceF32 {
                tile_width: b,
                kind: bk,
                ..
            },
        ) => a == b && ak == bk,
        _ => false,
    }
}

fn assembly(a: &InlineAssembly, b: &InlineAssembly, budget: &mut Budget<'_>) -> Result<bool> {
    if a.target != b.target
        || a.source != b.source
        || a.operands.len() != b.operands.len()
        || !bytes(a.mnemonic.as_bytes(), b.mnemonic.as_bytes(), budget)?
        || !set(&a.options, &b.options, budget)?
        || !set(&a.declared_effects, &b.declared_effects, budget)?
    {
        return Ok(false);
    }
    for (a, b) in a.operands.iter().zip(&b.operands) {
        budget.charge_work(1)?;
        if a.constraint != b.constraint {
            return Ok(false);
        }
        let same = match (&a.kind, &b.kind) {
            (AssemblyOperandKind::Input(_), AssemblyOperandKind::Input(_)) => true,
            (
                AssemblyOperandKind::Output { result_index: a },
                AssemblyOperandKind::Output { result_index: b },
            )
            | (
                AssemblyOperandKind::InOut {
                    result_index: a, ..
                },
                AssemblyOperandKind::InOut {
                    result_index: b, ..
                },
            ) => a == b,
            (AssemblyOperandKind::ImmediateI32(a), AssemblyOperandKind::ImmediateI32(b)) => a == b,
            _ => false,
        };
        if !same {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Closed native DCE/CSE allowance. In particular, no memory-only purity query
/// can classify calls, compiler events, allocations, convergence or opaque ops.
pub(super) fn pure(kind: &OperationKind) -> bool {
    matches!(
        kind,
        OperationKind::Constant(_)
            | OperationKind::Compare { .. }
            | OperationKind::Select { .. }
            | OperationKind::SliceLength { .. }
            | OperationKind::SliceData { .. }
            | OperationKind::Unary {
                op: UnaryOp::Not,
                ..
            }
            | OperationKind::Binary {
                op: BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor | BinaryOp::Checked(_),
                ..
            }
            | OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess
                    | CastKind::Truncate
                    | CastKind::ZeroExtend
                    | CastKind::SignExtend
                    | CastKind::Bitcast,
                ..
            }
    )
}
