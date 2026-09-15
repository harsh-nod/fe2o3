//! Same-owner receipt for the original guarded issuer and erased Some payload.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateKindV1, SemanticAssignmentV1, SemanticBorrowKindV1, SemanticConstantValueV1,
    SemanticDefinedCapabilityContractV1, SemanticGuardedGridLeaderV1, SemanticMutabilityV1,
    SemanticPointerKindV1,
};
use fe2o3_mir_model::{
    SemanticCallInstanceIdV1, SemanticExpandedStatementOriginV1, SemanticOptionProducerV1,
};
#[path = "guarded_grid_results/borrow_uses.rs"]
mod borrow_uses;

pub(super) fn initialized_shared_owner(
    function: &SemanticFunctionDeclV1,
    owner: SemanticLocalIdV1,
    charge: &mut impl FnMut(usize, usize) -> Result<(), ProductionSemanticSsaErrorV1>,
) -> Result<bool, ProductionSemanticSsaErrorV1> {
    borrow_uses::initialized_shared(function, owner, charge)
}


#[path = "guarded_grid_results/checked_source.rs"]
mod checked_source;
pub use checked_source::{ProductionGuardedGridActionKindV1, ProductionGuardedGridActionV1, ProductionGuardedGridIndexV1, ProductionGuardedGridSourceV1};

#[cfg(test)]
#[path = "guarded_grid_results/roster_accounting_tests.rs"]
mod roster_accounting_tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    Issuer,
    Some,
    Borrow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Event {
    block: u32,
    statement: u32,
    kind: Kind,
    source: SemanticLocalIdV1,
    destination: SemanticLocalIdV1,
}

/// A retained relationship, not a KIR leader operation or artifact authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionGuardedGridLeaderResultV1 {
    getter: SemanticCallInstanceIdV1,
    issuer: SemanticCallInstanceIdV1,
    receiver: SemanticLocalIdV1,
    option: SemanticLocalIdV1,
    contract: SemanticGuardedGridLeaderV1,
}
impl ProductionGuardedGridLeaderResultV1 {
    pub const fn getter(self) -> SemanticCallInstanceIdV1 {
        self.getter
    }
    pub const fn issuer(self) -> SemanticCallInstanceIdV1 {
        self.issuer
    }
    pub const fn receiver(self) -> SemanticLocalIdV1 {
        self.receiver
    }
    pub const fn option(self) -> SemanticLocalIdV1 {
        self.option
    }
    pub const fn contract(self) -> SemanticGuardedGridLeaderV1 {
        self.contract
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct GuardedGridResultsV1 {
    view: Option<[u8; 32]>,
    entries: Box<[ProductionGuardedGridLeaderResultV1]>,
    events: Box<[Event]>,
    resources: SemanticSsaAuxiliaryResourcesV1,
}

fn mismatch() -> ProductionSemanticSsaErrorV1 {
    ProductionSemanticSsaErrorV1::ReplayMismatch
}

fn append<T>(
    values: &mut Vec<T>,
    value: T,
    charge: &mut impl FnMut(usize, usize) -> Result<(), ProductionSemanticSsaErrorV1>,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    if values.len() == values.capacity() {
        let requested = values
            .capacity()
            .max(2)
            .checked_mul(2)
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        let words = |n: usize| {
            n.checked_mul(std::mem::size_of::<T>())
                .map(|b| b.div_ceil(std::mem::size_of::<usize>()))
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)
        };
        // Cumulative accounting includes old and replacement allocations. Charge
        // the request before reserve; never refund work or capacity after growth.
        charge(values.len(), words(requested)?)?;
        values
            .try_reserve_exact(requested - values.len())
            .map_err(|_| ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        charge(0, words(values.capacity().saturating_sub(requested))?)?;
    }
    charge(1, 0)?;
    values.push(value);
    Ok(())
}

impl GuardedGridResultsV1 {
    pub(super) fn derive(
        semantic: &AdmittedInertSemanticMirV1,
        expansion: &SemanticCallExpansionV1,
        view: &SemanticExpandedRootV1,
        limits: ProductionSemanticSsaLimitsV1,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        if !expansion
            .root(view.root())
            .is_some_and(|v| std::ptr::eq(v, view))
        {
            return Err(mismatch());
        }
        let mut resources = SemanticSsaAuxiliaryResourcesV1::default();
        let mut charge = |work: usize, words: usize| -> Result<(), ProductionSemanticSsaErrorV1> {
            resources.work_units = resources
                .work_units
                .checked_add(work)
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            resources.storage_words = resources
                .storage_words
                .checked_add(words)
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            enforce_function_resource_limit_v1(view.source_body(), resources, limits)
        };
        // The absent-family check also walks the function roster. Retain that
        // charge even when no Grid receipt or transient binding roster is built.
        charge(semantic.functions().len(), 0)?;
        if !semantic.functions().iter().any(|f|matches!(f.defined_capability_contract(),
            Some(SemanticDefinedCapabilityContractV1::GuardedGridLeader(r)) if r.provenance().root()==view.root())) {
            return Ok(Self {
                resources,
                ..Self::default()
            });
        }
        charge(0, 16)?;
        let bindings = expansion
            .defined_capability_bindings(semantic)
            .map_err(ProductionSemanticSsaErrorV1::CallExpansion)?;
        // Include the complete transient replay roster, not just matching rows.
        let roster_bytes=bindings.capacity().checked_mul(std::mem::size_of::<fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedDefinedCapabilityV1>())
            .and_then(|n|n.checked_mul(2)).ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        charge(0, roster_bytes.div_ceil(std::mem::size_of::<usize>()))?;
        for binding in &bindings {
            let bytes = binding
                .retained_auxiliary_bytes()
                .and_then(|n| n.checked_mul(2))
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            charge(
                1 + binding.arguments().len(),
                bytes.div_ceil(std::mem::size_of::<usize>()),
            )?;
            for operand in binding.arguments() {
                if let SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p) = operand {
                    charge(p.projections().len(), 0)?;
                }
            }
        }
        let mut entries = Vec::new();
        let mut events = Vec::new();
        for binding in bindings.iter().filter(|b| b.root() == view.root()) {
            let SemanticDefinedCapabilityContractV1::GuardedGridLeader(record) = binding.contract()
            else {
                continue;
            };
            if binding.caller_function() != record.source().caller
                || binding.call_block() != record.source().call_block
                || binding.callee_arguments().len() != 1
                || binding.arguments().len() != 1
            {
                return Err(mismatch());
            }
            let getter = binding.callee_instance();
            let mut issuer = None;
            charge(view.instances().len(), 0)?;
            for (i, instance) in view.instances().iter().enumerate() {
                if instance.parent() == Some(getter) {
                    if instance.function() != record.issuer().function
                        || instance.function_identity() != record.issuer().source
                        || instance.call_block() != Some(record.roles().issuer)
                    {
                        return Err(mismatch());
                    }
                    let origin = view
                        .block_origins()
                        .get(instance.block_start() as usize)
                        .ok_or_else(mismatch)?;
                    if origin.instance().index() as usize != i {
                        return Err(mismatch());
                    }
                    if issuer.replace(origin.instance()).is_some() {
                        return Err(mismatch());
                    }
                }
            }
            let issuer = issuer.ok_or_else(mismatch)?;
            let mut issuer_site = None;
            let mut some_site = None;
            let mut getter_site = None;
            charge(view.body().blocks().len(), 0)?;
            for (bi, (b, origin)) in view
                .body()
                .blocks()
                .iter()
                .zip(view.block_origins())
                .enumerate()
            {
                charge(b.statements().len(), 0)?;
                for (si, marker) in origin.statements().iter().enumerate() {
                    if *marker
                        == (SemanticExpandedStatementOriginV1::ReturnTransfer { callee: issuer })
                    {
                        if origin.instance() != issuer
                            || issuer_site.replace((bi as u32, si as u32)).is_some()
                        {
                            return Err(mismatch());
                        }
                    }
                    if *marker
                        == (SemanticExpandedStatementOriginV1::ReturnTransfer { callee: getter })
                    {
                        if origin.instance() != getter
                            || getter_site.replace((bi as u32, si as u32)).is_some()
                        {
                            return Err(mismatch());
                        }
                    }
                    if origin.instance() == getter
                        && origin.block() == record.roles().some
                        && *marker == (SemanticExpandedStatementOriginV1::Source { statement: 0 })
                    {
                        if some_site.replace((bi as u32, si as u32)).is_some() {
                            return Err(mismatch());
                        }
                    }
                }
            }
            let issuer_site = issuer_site.ok_or_else(mismatch)?;
            let some_site = some_site.ok_or_else(mismatch)?;
            let getter_site = getter_site.ok_or_else(mismatch)?;
            let transfer = assignment(view.body(), issuer_site)?;
            let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(returned)) =
                transfer.value().kind()
            else {
                return Err(mismatch());
            };
            let issued = transfer.destination();
            if !returned.projections().is_empty()
                || returned.ty() != record.types().leader
                || !issued.projections().is_empty()
                || issued.ty() != record.types().leader
            {
                return Err(mismatch());
            }
            let ret_origin = view
                .local_origins()
                .get(returned.local().index() as usize)
                .ok_or_else(mismatch)?;
            let dst_origin = view
                .local_origins()
                .get(issued.local().index() as usize)
                .ok_or_else(mismatch)?;
            if ret_origin.instance() != issuer
                || ret_origin.function() != record.issuer().function
                || semantic.functions()[record.issuer().function.index() as usize].locals()
                    [ret_origin.local().index() as usize]
                    .role()
                    != SemanticLocalRoleV1::Return
                || dst_origin.instance() != getter
                || dst_origin.local() != record.roles().issued
            {
                return Err(mismatch());
            }
            let some = assignment(view.body(), some_site)?;
            let SemanticRvalueKindV1::Aggregate(a) = some.value().kind() else {
                return Err(mismatch());
            };
            if *a.kind() != SemanticAggregateKindV1::EnumVariant(1)
                || !matches!(a.operands(),[SemanticOperandV1::Constant(c)] if c.ty()==record.types().leader && matches!(c.value(),SemanticConstantValueV1::ZeroSized))
                || some.destination().local() != binding.callee_return()
                || some.destination().ty() != record.types().leader_option
            {
                return Err(mismatch());
            }
            let out = assignment(view.body(), getter_site)?;
            if out.destination() != binding.destination()
                || !matches!(out.value().kind(),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p)) if p.local()==binding.callee_return() && p.ty()==record.types().leader_option && p.projections().is_empty())
            {
                return Err(mismatch());
            }
            let receiver = binding.callee_arguments()[0];
            append(
                &mut events,
                Event {
                    block: issuer_site.0,
                    statement: issuer_site.1,
                    kind: Kind::Issuer,
                    source: receiver,
                    destination: returned.local(),
                },
                &mut charge,
            )?;
            append(
                &mut events,
                Event {
                    block: some_site.0,
                    statement: some_site.1,
                    kind: Kind::Some,
                    source: issued.local(),
                    destination: binding.callee_return(),
                },
                &mut charge,
            )?;
            append(
                &mut entries,
                ProductionGuardedGridLeaderResultV1 {
                    getter,
                    issuer,
                    receiver,
                    option: binding.destination().local(),
                    contract: record,
                },
                &mut charge,
            )?;
            derive_borrows(semantic, view, binding, record, &mut events, &mut charge)?;
        }
        charge(
            events
                .len()
                .saturating_mul((usize::BITS - events.len().leading_zeros()) as usize + 1),
            0,
        )?;
        events.sort_by_key(|e| (e.block, e.statement));
        if events
            .windows(2)
            .any(|p| (p[0].block, p[0].statement) == (p[1].block, p[1].statement))
        {
            return Err(mismatch());
        }
        charge(
            entries.len() + events.len(),
            std::mem::size_of_val(entries.as_slice()).div_ceil(std::mem::size_of::<usize>())
                + std::mem::size_of_val(events.as_slice()).div_ceil(std::mem::size_of::<usize>()),
        )?;
        let result = Self {
            view: Some(*view.identity()),
            entries: entries.into_boxed_slice(),
            events: events.into_boxed_slice(),
            resources,
        };
        result.verify_view(view)?;
        Ok(result)
    }
    pub(super) fn entries(&self) -> &[ProductionGuardedGridLeaderResultV1] {
        &self.entries
    }
    pub(super) fn resources(&self) -> SemanticSsaAuxiliaryResourcesV1 {
        self.resources
    }
    pub(super) fn borrow_sites(
        &self,
    ) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.events
            .iter()
            .filter(|e| e.kind == Kind::Borrow)
            .map(|e| (e.block, e.statement))
    }
    pub(super) fn verify_view(
        &self,
        view: &SemanticExpandedRootV1,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        if self.view.is_none() && self.entries.is_empty() && self.events.is_empty() {
            return Ok(());
        }
        if self.view != Some(*view.identity()) {
            return Err(mismatch());
        }
        self.verify_markers(view.body())
    }
    pub(super) fn verify_markers(
        &self,
        f: &SemanticFunctionDeclV1,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        for e in &self.events {
            let a = assignment(f, (e.block, e.statement))?;
            match e.kind {
                Kind::Issuer if matches!(a.value().kind(),SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p)) if p.local()==e.destination && p.projections().is_empty()) =>
                    {}
                Kind::Some
                    if a.destination().local() == e.destination
                        && matches!(a.value().kind(),SemanticRvalueKindV1::Aggregate(v) if *v.kind()==SemanticAggregateKindV1::EnumVariant(1)) =>
                    {}
                Kind::Borrow if matches!(a.value().kind(),SemanticRvalueKindV1::Borrow {kind:SemanticBorrowKindV1::Shared,place} if place.local()==e.destination && place.projections().is_empty()) =>
                    {}
                _ => return Err(mismatch()),
            }
        }
        Ok(())
    }
    pub(super) fn append_result_events(
        &self,
        block: u32,
        statement: u32,
        events: &mut Vec<SsaEventV1>,
    ) {
        if let Ok(i) = self
            .events
            .binary_search_by_key(&(block, statement), |e| (e.block, e.statement))
        {
            let e = self.events[i];
            events.push(SsaEventV1::Use(SsaVariableIdV1::new(e.source.index())));
            if e.kind == Kind::Some {
                events.push(SsaEventV1::Kill(SsaVariableIdV1::new(e.source.index())));
            } else {
                events.push(SsaEventV1::Define(SsaVariableIdV1::new(
                    e.destination.index(),
                )));
            }
        }
    }
    pub(super) fn hash_into(&self, digest: &mut Sha256) {
        if self.entries.is_empty() {
            return;
        }
        digest.update(b"fe2o3.execution-guarded-grid-leader.v1\0");
        digest.update(self.view.expect("nonempty relation has a view"));
        digest.update((self.events.len() as u64).to_le_bytes());
        for e in &self.events {
            for v in [
                e.block,
                e.statement,
                e.kind as u32,
                e.source.index(),
                e.destination.index(),
            ] {
                digest.update(v.to_le_bytes());
            }
        }
        digest.update((self.entries.len() as u64).to_le_bytes());
        for e in &self.entries {
            for v in [
                e.getter.index(),
                e.issuer.index(),
                e.receiver.index(),
                e.option.index(),
            ] {
                digest.update(v.to_le_bytes());
            }
        }
    }
}

fn assignment(
    f: &SemanticFunctionDeclV1,
    site: (u32, u32),
) -> Result<&SemanticAssignmentV1, ProductionSemanticSsaErrorV1> {
    match f
        .blocks()
        .get(site.0 as usize)
        .and_then(|b| b.statements().get(site.1 as usize))
        .map(|s| s.kind())
    {
        Some(SemanticStatementKindV1::Assign(a)) => Ok(a),
        _ => Err(mismatch()),
    }
}

fn derive_borrows(
    semantic: &AdmittedInertSemanticMirV1,
    view: &SemanticExpandedRootV1,
    binding:&fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedDefinedCapabilityV1,
    record: SemanticGuardedGridLeaderV1,
    events: &mut Vec<Event>,
    charge: &mut impl FnMut(usize, usize) -> Result<(), ProductionSemanticSsaErrorV1>,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    let source = &semantic.functions()[record.source().caller.index() as usize];
    let SemanticTerminatorKindV1::Call(call) = source.blocks()
        [record.source().call_block.index() as usize]
        .terminator()
        .kind()
    else {
        return Err(mismatch());
    };
    let dest = call.destination().ok_or_else(mismatch)?;
    let dominance = SemanticOptionDominanceV1::analyze(
        source,
        &[SemanticOptionProducerV1::new(
            dest.place().local(),
            dest.edge().target(),
        )],
    )
    .map_err(|_| mismatch())?;
    charge(
        dominance.work_units(),
        4 * source.blocks().len() + 2 * source.locals().len(),
    )?;
    let availability = dominance
        .availability(dest.place().local())
        .ok_or_else(mismatch)?;
    let definitions = super::adapter::direct_definition_or_lifetime_locals_v1(source);
    let instance = &view.instances()[binding.caller_instance().index() as usize];
    charge(
        source
            .blocks()
            .iter()
            .map(|b| b.statements().len() + 1)
            .sum(),
        8 * definitions.len(),
    )?;
    let mut checked_owner = None;
    for (bi, origin) in view.block_origins().iter().enumerate() {
        charge(1 + origin.statements().len(), 0)?;
        if origin.instance() != binding.caller_instance() {
            continue;
        }
        for (si, marker) in origin.statements().iter().enumerate() {
            let SemanticExpandedStatementOriginV1::Source { statement } = marker else {
                continue;
            };
            let SemanticStatementKindV1::Assign(a) =
                source.blocks()[origin.block().index() as usize].statements()[*statement as usize]
                    .kind()
            else {
                continue;
            };
            let SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            } = a.value().kind()
            else {
                continue;
            };
            if place.ty() != record.types().leader || !place.projections().is_empty() {
                continue;
            }
            if definitions.contains(&place.local()) {
                continue;
            }
            if let Some(owner) = checked_owner {
                if owner != place.local() {
                    return Err(mismatch());
                }
            } else {
                if !borrow_uses::only_shared(source, place.local(), charge)? {
                    return Err(mismatch());
                }
                checked_owner = Some(place.local());
            }
            if !dominance.allows(availability, origin.block()) {
                return Err(mismatch());
            }
            let Some(SemanticTypeShapeV1::Pointer(p)) = semantic
                .types()
                .get(a.value().result_type().index() as usize)
                .map(SemanticTypeDeclV1::shape)
            else {
                return Err(mismatch());
            };
            if p.pointee() != record.types().leader
                || p.kind() != SemanticPointerKindV1::Reference
                || p.mutability() != SemanticMutabilityV1::Immutable
            {
                return Err(mismatch());
            }
            let local = SemanticLocalIdV1::from_index(
                instance
                    .local_start()
                    .checked_add(place.local().index())
                    .ok_or_else(mismatch)?,
            );
            let expanded = assignment(view.body(), (bi as u32, si as u32))?;
            if !matches!(expanded.value().kind(),SemanticRvalueKindV1::Borrow {kind:SemanticBorrowKindV1::Shared,place} if place.local()==local && place.ty()==record.types().leader && place.projections().is_empty())
            {
                return Err(mismatch());
            }
            // The caller extends the existing transparent-borrow BTreeSet with
            // this exact site. Include its node allowance in this same receipt.
            charge(1, 12)?;
            append(
                events,
                Event {
                    block: bi as u32,
                    statement: si as u32,
                    kind: Kind::Borrow,
                    source: binding.destination().local(),
                    destination: local,
                },
                charge,
            )?;
        }
    }
    Ok(())
}
