//! Ancestry of existing LDS operations; no allocation or memory event is added.
use super::*;
use crate::{ExecutionCapabilityOpV1, ExecutionCapabilityTypeV1 as Cap, ExecutionLdsStateV1 as Lds};

pub(super) fn is_shared_read(kind: &OperationKind) -> bool {
    matches!(kind, OperationKind::ExecutionCapability(e) if matches!(e.operation, Exec::LdsReadPublished { .. }))
}

fn capability<'v>(values: &[Value<'v>], id: ValueId, b: &mut Budget) -> Result<&'v Cap> {
    match values[index(values, id, b)?].ty {
        Type::ExecutionCapability(c) if c.is_complete() => Ok(c),
        _ => Err(Error::LocalContract),
    }
}
fn check(c: &Cap, source: ExecutionTypeIdentityV1, role: Role,
    e: &ExecutionCapabilityOpV1, epoch: Option<Digest>) -> Result<()> {
    if c.source_type != source || c.role != role || c.provenance != e.provenance
        || c.workgroup_brand != e.workgroup_brand || c.epoch != epoch {
        return Err(Error::LocalContract);
    }
    Ok(())
}
fn occurrence(e: &ExecutionCapabilityOpV1, f: Fact, nodes: &[Node<'_>], b: &mut Budget) -> Result<()> {
    let begin = phase(nodes, f, b)?;
    let a = recipe(begin)?.call.source.occurrence.ok_or(Error::WrongOccurrence)?;
    let z = e.source.occurrence.ok_or(Error::WrongOccurrence)?;
    if a.root_source_identity() != z.root_source_identity()
        || a.expansion_identity() != z.expansion_identity()
        || a.expanded_root_identity() != z.expanded_root_identity()
        || e.provenance != begin.provenance {
        return Err(Error::WrongOccurrence);
    }
    // This is an inert owner comparison, not proof of the original memory call.
    // The production source adapter must consume the exact replayed occurrence.
    Ok(())
}

pub(super) fn step(n: &Node<'_>, e: &ExecutionCapabilityOpV1, nodes: &[Node<'_>],
    values: &mut [Value<'_>], b: &mut Budget) -> Result<()> {
    if !e.is_complete() { return Err(Error::LocalContract); }
    let ids = &e.operands;
    let out = &n.op.results;
    let Some(first) = ids.first() else { return Err(Error::LocalContract); };
    let f = fact(values, *first, b)?;
    if f.begin.is_none() { return Ok(()); }
    occurrence(e, f, nodes, b)?;
    match e.operation {
        Exec::LdsInitializeByInvocation { input_lds, workgroup, output_lds, element, layout, elements } => {
            if ids.len() != 3 || out.len() != 1 { return Err(Error::LocalContract); }
            let w = fact(values, ids[1], b)?;
            same_phase(f, w)?;
            if f.bind.is_none() || f.allocation.is_none() || w.allocation.is_some()
                || f.barrier.is_some() || w.barrier.is_some() { return Err(Error::WrongStorage); }
            let lds = |state| Role::Lds { element, layout, elements, state };
            check(capability(values, ids[0], b)?, input_lds, lds(Lds::Uninitialized), e, e.epoch_before)?;
            check(capability(values, ids[1], b)?, workgroup, Role::Workgroup, e, e.epoch_before)?;
            check(capability(values, out[0].id, b)?, output_lds, lds(Lds::InvocationInitialized), e, e.epoch_before)?;
            consume(values, ids[0], n.at, b)?;
            put(values, out[0].id, f, b)?;
        }
        Exec::LdsPublish { input_workgroup, input_lds, output_lds, transition, element, layout, elements } => {
            if ids.len() != 2 || out.len() != 2 { return Err(Error::LocalContract); }
            let s = fact(values, ids[1], b)?;
            same_phase(f, s)?;
            if f.allocation.is_some() || s.allocation.is_none() || s.bind.is_none()
                || f.barrier.is_some() || s.barrier.is_some() { return Err(Error::WrongStorage); }
            let lds = |state| Role::Lds { element, layout, elements, state };
            check(capability(values, ids[0], b)?, input_workgroup, Role::Workgroup, e, e.epoch_before)?;
            check(capability(values, ids[1], b)?, input_lds, lds(Lds::InvocationInitialized), e, e.epoch_before)?;
            check(capability(values, out[0].id, b)?, transition, Role::Workgroup, e, e.epoch_after)?;
            check(capability(values, out[1].id, b)?, output_lds, lds(Lds::Published), e, e.epoch_after)?;
            for id in ids { consume(values, *id, n.at, b)?; }
            put(values, out[0].id, f, b)?;
            put(values, out[1].id, s, b)?;
        }
        Exec::LdsReadPublished { lds, workgroup, element, layout, elements, .. } => {
            if ids.len() != 3 || out.len() != 2 { return Err(Error::LocalContract); }
            let w = fact(values, ids[1], b)?;
            same_phase(f, w)?;
            if f.allocation.is_none() || f.bind.is_none() || w.allocation.is_some()
                || f.barrier.is_some() || w.barrier.is_some() { return Err(Error::WrongStorage); }
            check(capability(values, ids[0], b)?, lds, Role::Lds { element, layout, elements, state: Lds::Published }, e, e.epoch_before)?;
            check(capability(values, ids[1], b)?, workgroup, Role::Workgroup, e, e.epoch_before)?;
            // Neither handle is consumed. Scalar/Option results retain the old
            // exact physical type, index/bounds and numerical obligations.
        }
        _ => return Err(Error::UnsupportedPhaseConsumer),
    }
    Ok(())
}
