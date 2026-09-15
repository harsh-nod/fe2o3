use super::*;
use crate::{ExecutionCapabilityRoleV1 as Role, ExecutionCapabilityTypeV1 as Cap,
    ExecutionLdsStateV1, Type, MAX_EXECUTION_CAPABILITY_RESULTS_V1};
use ReusablePhaseOperationV1 as Op;
use ReusablePhaseTokenRoleV1 as TokenRole;

fn cap<'a>(ty: &'a Type, source: ExecutionTypeIdentityV1, role: Role,
    provenance: &ExecutionCapabilityProvenanceV1) -> Option<&'a Cap> {
    match ty {
        Type::ExecutionCapability(c) if c.source_type == source && c.role == role
            && c.is_complete() && c.provenance == *provenance
            && c.workgroup_brand.is_some() && c.epoch.is_some() => Some(c),
        _ => None,
    }
}
fn token<'a>(ty: &'a Type, role: Option<TokenRole>, provenance: &ExecutionCapabilityProvenanceV1)
    -> Option<&'a ReusablePhaseTokenTypeV1> {
    match ty {
        Type::ReusablePhaseToken(t) if t.is_complete() && t.provenance == *provenance
            && role.is_none_or(|r| r == t.role) => Some(t),
        _ => None,
    }
}
fn source_type(source: ExecutionTypeIdentityV1, role: Role,
    t: &ReusablePhaseTokenTypeV1, brand: Digest, epoch: Digest) -> Type {
    Type::ExecutionCapability(Cap { source_type: source, role, provenance: t.provenance.clone(),
        workgroup_brand: Some(brand), epoch: Some(epoch) })
}
fn phase_token(t: &ReusablePhaseTokenTypeV1, role: TokenRole) -> Type {
    Type::ReusablePhaseToken(ReusablePhaseTokenTypeV1 { role, ..t.clone() })
}
fn signature(source: &PhaseOperationSourceV1, args: &[ExecutionTypeIdentityV1],
    output: ExecutionTypeIdentityV1) -> bool {
    match source {
        PhaseOperationSourceV1::Defined(d) => d.is_complete()
            && d.signature.arguments().eq(args.iter().copied()) && d.signature.output() == output,
        _ => false,
    }
}
fn key(source: &PhaseOperationSourceV1, expected: PhaseKeyV1) -> bool {
    match source {
        PhaseOperationSourceV1::ClosureReturn { phase, closure, source_protocol, .. } =>
            *phase == expected && *source_protocol != [0; 32] && closure.is_complete(),
        PhaseOperationSourceV1::WrapperDrop { phase, drop_call, drop_abi, source_protocol } =>
            *phase == expected && *source_protocol != [0; 32] && *drop_abi != [0; 32]
                && drop_call.is_complete(),
        PhaseOperationSourceV1::LeaseEnd { phase, bind, source_event, source_protocol } =>
            *phase == expected && *source_protocol != [0; 32] && bind.is_complete()
                && source_event.function != [0; 32],
        PhaseOperationSourceV1::WrapperEnd { phase, source_protocol, .. } =>
            *phase == expected && *source_protocol != [0; 32],
        PhaseOperationSourceV1::Defined(_) => false,
    }
}

impl ReusablePhaseOpV1 {
    /// Shared by the emitter and verifier. This checks local type/contract
    /// consistency, not source authenticity or linear producer/use custody.
    pub fn checked_result_types(&self, input: &[&Type]) -> Option<Vec<Type>> {
        let (inputs, results) = self.operation.arity();
        if self.operands.len() != inputs || input.len() != inputs
            || results > MAX_EXECUTION_CAPABILITY_RESULTS_V1
            || !self.provenance.is_complete() || self.provenance.root.as_str().len() > 256
            || self.obligations.bits() != self.operation.required_obligations()
            || self.operands.iter().enumerate().any(|(i, v)| self.operands[..i].contains(v)) {
            return None;
        }
        let output = match &self.operation {
            Op::OwnerConvert { workgroup, owner } => {
                if workgroup == owner || !owner.is_complete() || !signature(&self.source, &[*workgroup], *owner) { return None; }
                let w = cap(input[0], *workgroup, Role::Workgroup, &self.provenance)?;
                vec![Type::ExecutionCapability(Cap { source_type: *owner, role: Role::ReusableWorkgroup, ..w.clone() })]
            }
            Op::Begin { owner_reference, owner, phase_workgroup, outer_brand, phase_brand,
                dynamic_epoch, wrapper, invoke, source_protocol } => {
                if !signature(&self.source, &[*owner_reference], *phase_workgroup)
                    || owner_reference == owner || owner == phase_workgroup || !owner_reference.is_complete()
                    || !wrapper.is_complete() || !invoke.is_complete() || *source_protocol == [0; 32]
                    || [*outer_brand, *phase_brand, *dynamic_epoch].contains(&[0; 32]) { return None; }
                let PhaseOperationSourceV1::Defined(issue) = self.source else { return None; };
                if !issue.call.same_expansion(wrapper.call) || !issue.call.same_expansion(invoke.call)
                    || issue.call.source.occurrence?.caller_instance() != wrapper.call.callee_instance
                    || invoke.call.source.occurrence?.caller_instance() != wrapper.call.callee_instance {
                    return None;
                }
                let o = cap(input[0], *owner, Role::ReusableWorkgroup, &self.provenance)?;
                if o.workgroup_brand != Some(*outer_brand) { return None; }
                let t = ReusablePhaseTokenTypeV1 { provenance: self.provenance.clone(),
                    phase: PhaseKeyV1::for_begin(issue.call)?, owner_source: *owner,
                    owner_anchor_epoch: o.epoch?, outer_brand: *outer_brand, phase_brand: *phase_brand,
                    initial_epoch: *dynamic_epoch, role: TokenRole::OwnerLoan(PhaseLoanStateV1::Active) };
                vec![source_type(*phase_workgroup, Role::Workgroup, &t, *phase_brand, *dynamic_epoch),
                    Type::ReusablePhaseToken(t)]
            }
            Op::Bind { phase_reference, storage_reference, phase_workgroup, reusable_storage,
                phase_lds, element, layout, elements } => {
                if !signature(&self.source, &[*phase_reference, *storage_reference], *phase_lds)
                    || phase_reference == phase_workgroup || storage_reference == reusable_storage
                    || !phase_lds.is_complete() || !element.is_complete()
                    || !layout.checked_footprint(*elements).is_some_and(|n| n != 0) { return None; }
                let t = token(input[0], Some(TokenRole::OwnerLoan(PhaseLoanStateV1::Active)), &self.provenance)?;
                let p = cap(input[1], *phase_workgroup, Role::Workgroup, &self.provenance)?;
                let s = cap(input[2], *reusable_storage, Role::ReusableLds { element: *element, layout: *layout,
                    elements: *elements }, &self.provenance)?;
                if p.workgroup_brand != Some(t.phase_brand) || p.epoch != Some(t.initial_epoch)
                    || s.workgroup_brand != Some(t.outer_brand) { return None; }
                let restore = PhaseStorageRestoreV1 { reusable_source: *reusable_storage,
                    element: *element, layout: *layout, elements: *elements, allocation_anchor_epoch: s.epoch? };
                vec![Type::ReusablePhaseToken(t.clone()), source_type(*phase_lds,
                    Role::Lds { element: *element, layout: *layout, elements: *elements,
                        state: ExecutionLdsStateV1::Uninitialized }, &t, t.phase_brand, t.initial_epoch),
                    phase_token(&t, TokenRole::StorageLoan(restore))]
            }
            Op::Seal { workgroup_before_barrier, workgroup_after_barrier, completion, barrier_call } => {
                if !signature(&self.source, &[*workgroup_before_barrier], *completion)
                    || !barrier_call.is_complete() || workgroup_before_barrier == workgroup_after_barrier
                    || !completion.is_complete() { return None; }
                let PhaseOperationSourceV1::Defined(finish) = self.source else { return None; };
                if !barrier_call.same_expansion(finish.call)
                    || barrier_call.source.occurrence?.caller_instance() != finish.call.callee_instance { return None; }
                let t = token(input[0], Some(TokenRole::OwnerLoan(PhaseLoanStateV1::Active)), &self.provenance)?;
                let p = cap(input[1], *workgroup_after_barrier, Role::Workgroup, &self.provenance)?;
                if p.workgroup_brand != Some(t.phase_brand) || p.epoch == Some(t.initial_epoch) { return None; }
                vec![phase_token(&t, TokenRole::OwnerLoan(PhaseLoanStateV1::Sealed)),
                    source_type(*completion, Role::ReusablePhaseCompletion, &t, t.phase_brand, p.epoch?)]
            }
            Op::RelayClosure { completion } => {
                if !matches!(self.source, PhaseOperationSourceV1::ClosureReturn { .. }) { return None; }
                let PhaseOperationSourceV1::ClosureReturn { phase, closure, source_protocol, .. } = self.source else { return None; };
                if !key(&self.source, phase) || source_protocol == [0; 32] || !closure.is_complete() { return None; }
                vec![Type::ExecutionCapability(cap(input[0], *completion, Role::ReusablePhaseCompletion, &self.provenance)?.clone())]
            }
            Op::RelayDrop { completion } => {
                let t = token(input[0], Some(TokenRole::OwnerLoan(PhaseLoanStateV1::Sealed)), &self.provenance)?;
                if !matches!(self.source, PhaseOperationSourceV1::WrapperDrop { .. }) || !key(&self.source, t.phase) { return None; }
                let c = cap(input[1], *completion, Role::ReusablePhaseCompletion, &self.provenance)?;
                if c.workgroup_brand != Some(t.phase_brand) || c.epoch == Some(t.initial_epoch) { return None; }
                vec![phase_token(&t, TokenRole::OwnerLoan(PhaseLoanStateV1::Returned)),
                    phase_token(&t, TokenRole::CompletionReady { completion_source: *completion, barrier_epoch: c.epoch? })]
            }
            Op::CloseStorage { last_lease } => {
                let t = token(input[0], None, &self.provenance)?;
                let TokenRole::StorageLoan(s) = t.role else { return None; };
                if !matches!(self.source, PhaseOperationSourceV1::LeaseEnd { .. }) || !key(&self.source, t.phase) { return None; }
                let Type::ExecutionCapability(l) = input[1] else { return None; };
                let Role::Lds { element, layout, elements, state } = l.role else { return None; };
                if element != s.element || layout != s.layout || elements != s.elements { return None; }
                let l = cap(input[1], *last_lease, Role::Lds { element, layout, elements, state }, &self.provenance)?;
                if l.workgroup_brand != Some(t.phase_brand) { return None; }
                match state {
                    ExecutionLdsStateV1::Uninitialized | ExecutionLdsStateV1::InvocationInitialized =>
                        if l.epoch != Some(t.initial_epoch) { return None; },
                    ExecutionLdsStateV1::Published =>
                        if l.epoch == Some(t.initial_epoch) { return None; },
                    ExecutionLdsStateV1::PendingAsyncCopy => return None,
                }
                vec![phase_token(&t, TokenRole::ClosedStorage(s))]
            }
            Op::End { storage_count } => {
                if usize::from(*storage_count) >= MAX_EXECUTION_CAPABILITY_RESULTS_V1 { return None; }
                let t = token(input[0], Some(TokenRole::OwnerLoan(PhaseLoanStateV1::Returned)), &self.provenance)?;
                let c = token(input[1], None, &self.provenance)?;
                if !matches!(self.source, PhaseOperationSourceV1::WrapperEnd { .. }) || !key(&self.source, t.phase)
                    || !t.same_phase(&c) || !matches!(c.role, TokenRole::CompletionReady { .. }) { return None; }
                let mut out = Vec::new();
                out.try_reserve_exact(results).ok()?;
                out.push(source_type(t.owner_source, Role::ReusableWorkgroup, &t, t.outer_brand, t.owner_anchor_epoch));
                for ty in &input[2..] {
                    let s = token(ty, None, &self.provenance)?;
                    let TokenRole::ClosedStorage(restore) = s.role else { return None; };
                    if !t.same_phase(&s) { return None; }
                    out.push(source_type(restore.reusable_source, Role::ReusableLds { element: restore.element,
                        layout: restore.layout, elements: restore.elements }, &t, t.outer_brand, restore.allocation_anchor_epoch));
                }
                out
            }
        };
        (output.len() == results && output.iter().all(|t| match t {
            Type::ExecutionCapability(c) => c.is_complete(),
            Type::ReusablePhaseToken(t) => t.is_complete(),
            _ => false,
        })).then_some(output)
    }
}
