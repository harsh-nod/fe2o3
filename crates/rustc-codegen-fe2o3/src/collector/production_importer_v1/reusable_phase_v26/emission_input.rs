//! Copy only coordinates already selected by the private source/SSA owner.
//! The resulting data is replay input, not a second source proof.
use super::*;
use fe2o3_lower_mir_kernel::{
    PhaseBoundaryKindV1 as Kind, PhaseEmissionActionV1 as Action,
    PhaseEmissionBoundaryV1 as Boundary, PhaseEmissionInputV1 as Input, PhaseEmissionRowV1 as Row,
    PhaseLeaseEmissionInputV1 as Lease, PhaseOccurrenceEmissionInputV1 as Phase,
};

pub(super) fn input(checked: &CheckedSsa<'_, '_>, work: &mut usize) -> PhaseResult<Input> {
    let expansion = &checked.expansion;
    if checked.phases.len() != expansion.phases.len() || checked.phases.is_empty() {
        return Err(rejected("phase emission changed the checked phase roster"));
    }
    let mut phases = reserve(checked.phases.len(), work)?;
    let mut bindings = reserve(expansion.bindings.len(), work)?;
    for binding in &expansion.bindings {
        spend(work, 1)?;
        spend(
            work,
            binding
                .retained_auxiliary_bytes()
                .ok_or_else(|| rejected("phase retained binding copy capacity overflow"))?,
        )?;
        bindings.push(binding.clone());
    }
    let mut row_count = 0usize;
    for (index, (source, phase)) in expansion.phases.iter().zip(&checked.phases).enumerate() {
        spend(work, 1)?;
        let instance = |binding: usize| {
            expansion
                .bindings
                .get(binding)
                .map(|binding| binding.callee_instance())
                .ok_or_else(|| rejected("phase emission left its checked expansion bindings"))
        };
        if source.binds.len() != phase.leases.len() {
            return Err(rejected(
                "phase emission changed the checked allocation roster",
            ));
        }
        let mut leases = reserve(phase.leases.len(), work)?;
        for (source, lease) in source.binds.iter().zip(&phase.leases) {
            spend(work, 1)?;
            leases.push(Lease {
                bind: instance(source.call)?,
                storage_conversion: instance(source.storage_conversion)?,
                allocation: lease.allocation,
                root_reference: lease.root_reference,
                phase_reference: lease.phase_reference,
                storage_reference: lease.storage_reference,
                result: lease.result,
                borrowed_at: (lease.borrowed_at.block, lease.borrowed_at.event),
            });
        }
        let mut first_owner = true;
        for prior in &expansion.phases[..index] {
            spend(work, 1)?;
            first_owner &= prior.owner != source.owner;
        }
        row_count = row_count
            .checked_add(5 + usize::from(first_owner))
            .and_then(|count| {
                leases
                    .len()
                    .checked_mul(2)
                    .and_then(|n| count.checked_add(n))
            })
            .ok_or_else(|| rejected("phase emission row capacity overflow"))?;
        phases.push(Phase {
            owner: instance(source.owner)?,
            wrapper: instance(source.wrapper)?,
            issue: instance(source.issue)?,
            finish: instance(source.finish)?,
            closure: source.closure,
            owner_workgroup: phase.owner_workgroup,
            converted_owner: phase.converted_owner,
            owner_reference: phase.owner_reference,
            issued_phase: phase.issued_phase,
            closure_phase: phase.closure_phase,
            relay: phase.relay,
            completion: phase.completion,
            begin: (phase.begin.block, phase.begin.event),
            leases,
        });
    }
    Ok(Input {
        root: expansion.view.root(),
        semantic: *checked.owner.source_semantic().semantic_sha256().as_bytes(),
        expansion: *expansion.expansion.identity(),
        expanded_root: *expansion.view.identity(),
        source_protocol: expansion.source.identity,
        bindings,
        phases,
        rows: reserve(row_count, work)?,
    })
}

pub(super) fn append(input: &mut Input, row: super::Row, work: &mut usize) -> PhaseResult<()> {
    spend(work, 1)?;
    if row.phase() >= input.phases.len() || input.rows.len() == input.rows.capacity() {
        return Err(rejected(
            "phase emission exceeded the complete checked row capacity",
        ));
    }
    let boundary = row.boundary();
    let kind = match boundary.kind() {
        BoundaryEvent::Define => Kind::Define,
        BoundaryEvent::Use => Kind::Use,
        BoundaryEvent::Kill => Kind::Kill,
    };
    let action = match row.action() {
        super::Action::OwnerConvert => Action::OwnerConvert,
        super::Action::Begin => Action::Begin,
        super::Action::Bind { lease } => Action::Bind { lease },
        super::Action::Seal => Action::Seal,
        super::Action::RelayClosure => Action::RelayClosure,
        super::Action::RelayDrop => Action::RelayDrop,
        super::Action::CloseStorage { lease } => Action::CloseStorage { lease },
        super::Action::End => Action::End,
    };
    input.rows.push(Row {
        phase: row.phase(),
        action,
        boundary: Boundary {
            site: boundary.site(),
            event: boundary.point().event,
            variable: boundary.variable(),
            value: boundary.value(),
            kind,
        },
    });
    Ok(())
}
