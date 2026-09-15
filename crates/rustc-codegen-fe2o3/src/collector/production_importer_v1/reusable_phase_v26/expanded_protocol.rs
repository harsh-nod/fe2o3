//! Consume the live source owner into exact expansion occurrences. Indexes
//! refer to one replayed binding vector; no source values or loans are minted.
use super::{
    PhaseResult,
    execution_source::CheckedSource,
    rejected,
    source_calls::{reserve, spend},
    source_protocol::Site,
};
use fe2o3_mir_model::{
    SemanticCallExpansionV1, SemanticCallInstanceIdV1,
    SemanticExpandedRootV1,
    semantic_mir_v1::{
        SemanticDefinedCapabilityContractV1 as C, SemanticDefinedReusablePhaseRecipeV1 as R, *,
    },
};
use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedDefinedCapabilityV1;

pub(super) struct Bind {
    pub call: usize,
    pub storage_conversion: usize,
}
pub(super) struct Phase {
    pub source: usize,
    pub owner: usize,
    pub wrapper: usize,
    pub issue: usize,
    pub finish: usize,
    pub closure: SemanticCallInstanceIdV1,
    pub binds: Vec<Bind>,
}

/// Still not a linear SSA proof. The retained source owner and exact view must
/// be consumed together by the SSA/ordered-borrow bridge, not replaced by IDs.
pub(super) struct CheckedExpansion<'mir, 'view> {
    pub source: CheckedSource<'mir>,
    pub expansion: &'view SemanticCallExpansionV1,
    pub view: &'view SemanticExpandedRootV1,
    pub bindings: Vec<SemanticExpandedDefinedCapabilityV1>,
    pub phases: Vec<Phase>,
}

impl<'mir> CheckedSource<'mir> {
    pub(super) fn bind_expansion<'view>(
        mut self,
        expansion: &'view SemanticCallExpansionV1,
        view: &'view SemanticExpandedRootV1,
    ) -> PhaseResult<CheckedExpansion<'mir, 'view>> {
        if !expansion
            .root(view.root())
            .is_some_and(|root| std::ptr::eq(root, view))
        {
            return Err(rejected(
                "phase execution substituted the expanded root owner",
            ));
        }
        // This existing owner performs full replay and meters the common
        // remapping allocations. Additional vectors below use the remaining
        // live-source budget, including their actual retained capacities.
        let bindings = expansion
            .defined_capability_bindings(self.semantic)
            .map_err(|_| rejected("phase execution original expansion replay"))?;
        let work = &mut self.remaining_work;
        let mut retained = bindings.capacity()
            .checked_mul(std::mem::size_of::<SemanticExpandedDefinedCapabilityV1>())
            .ok_or_else(|| rejected("phase retained expansion binding capacity overflow"))?;
        for binding in &bindings {
            spend(work, 1)?;
            retained = retained.checked_add(binding.retained_auxiliary_bytes()
                .ok_or_else(|| rejected("phase nested expansion binding capacity overflow"))?)
                .ok_or_else(|| rejected("phase retained expansion binding capacity overflow"))?;
        }
        spend(work, retained)?;
        let mut used = reserve(bindings.len(), work)?;
        used.resize(bindings.len(), 0usize);
        spend(work, bindings.len())?;
        let count = bindings.iter().filter(|binding| binding.root() == view.root()
            && matches!(binding.contract(), C::ReusablePhase(r) if matches!(r.recipe(), R::WithPhase {..}))).count();
        let mut phases = reserve(count, work)?;
        for (index, wrapper) in bindings.iter().enumerate() {
            spend(work, 1)?;
            if wrapper.root() != view.root() {
                continue;
            }
            let C::ReusablePhase(record) = wrapper.contract() else {
                continue;
            };
            if wrapper.root_identity() != view.identity()
                || wrapper.expansion_identity() != expansion.identity()
            {
                return Err(rejected("phase execution substituted expansion identities"));
            }
            let R::WithPhase { invoke_block, .. } = record.recipe() else {
                continue;
            };
            let mut original = None;
            for (source_index, protocol) in self.protocols.iter().enumerate() {
                spend(work, 1)?;
                if site(wrapper) == protocol.wrapper && original.replace(source_index).is_some() {
                    return Err(rejected("phase execution has ambiguous original protocol"));
                }
            }
            let source = original
                .ok_or_else(|| rejected("phase execution has no complete source protocol"))?;
            let protocol = &self.protocols[source];
            let owner = lookup(
                &bindings,
                view,
                protocol.owner,
                wrapper.caller_instance(),
                work,
            )?;
            let issue = lookup(
                &bindings,
                view,
                protocol.issue,
                wrapper.callee_instance(),
                work,
            )?;
            let mut closure = None;
            for candidate in &bindings {
                spend(work, 1)?;
                if candidate.root() != view.root() || site(candidate) != protocol.finish {
                    continue;
                }
                let instance = candidate.caller_instance();
                let occurrence =
                    view.instances()
                        .get(instance.index() as usize)
                        .ok_or_else(|| {
                            rejected("phase execution closure instance outside retained roster")
                        })?;
                if occurrence.parent() == Some(wrapper.callee_instance())
                    && occurrence.call_block() == Some(invoke_block)
                {
                    if occurrence.function() != protocol.closure
                        || closure.replace(instance).is_some()
                    {
                        return Err(rejected("phase execution substituted its closure instance"));
                    }
                }
            }
            let closure =
                closure.ok_or_else(|| rejected("phase execution lost its actual closure call"))?;
            let finish = lookup(&bindings, view, protocol.finish, closure, work)?;
            if !matches!(bindings[owner].contract(), C::ReusablePhase(r) if matches!(r.recipe(), R::OwnerConvert {..}))
                || !matches!(bindings[issue].contract(), C::ReusablePhase(r) if matches!(r.recipe(), R::Issue {..}))
                || !matches!(bindings[finish].contract(), C::ReusablePhase(r) if matches!(r.recipe(), R::Finish {..}))
            {
                return Err(rejected("phase execution changed an original recipe role"));
            }
            let mut binds = reserve(protocol.binds.len(), work)?;
            for bind in &protocol.binds {
                let call = lookup(&bindings, view, bind.site, closure, work)?;
                if !matches!(bindings[call].contract(), C::ReusablePhase(r) if matches!(r.recipe(), R::Bind {..}))
                {
                    return Err(rejected("phase execution changed its Bind recipe"));
                }
                let origin = bind.storage.source();
                let storage_conversion = lookup(
                    &bindings,
                    view,
                    Site {
                        function: origin.caller,
                        block: origin.conversion_block,
                    },
                    wrapper.caller_instance(),
                    work,
                )?;
                if bindings[storage_conversion].contract() != C::ReusableLdsConversion(bind.storage)
                {
                    return Err(rejected(
                        "phase execution substituted its reusable allocation conversion",
                    ));
                }
                increment(&mut used, call, work)?;
                binds.push(Bind {
                    call,
                    storage_conversion,
                });
            }
            for value in [index, owner, issue, finish] {
                increment(&mut used, value, work)?;
            }
            if phases.len() == count {
                return Err(rejected("phase expanded protocol count changed"));
            }
            phases.push(Phase {
                source,
                owner,
                wrapper: index,
                issue,
                finish,
                closure,
                binds,
            });
        }
        if phases.is_empty() || phases.len() != count {
            return Err(rejected(
                "phase execution root has no complete original protocol",
            ));
        }
        for (index, binding) in bindings.iter().enumerate() {
            spend(work, 1)?;
            if binding.root() != view.root() {
                continue;
            }
            let C::ReusablePhase(record) = binding.contract() else {
                continue;
            };
            if used[index] == 0
                || (!matches!(record.recipe(), R::OwnerConvert { .. }) && used[index] != 1)
            {
                return Err(rejected(
                    "phase execution omitted or reused an expanded occurrence",
                ));
            }
        }
        Ok(CheckedExpansion {
            source: self,
            expansion,
            view,
            bindings,
            phases,
        })
    }
}

fn site(binding: &SemanticExpandedDefinedCapabilityV1) -> Site {
    Site {
        function: binding.caller_function(),
        block: binding.call_block(),
    }
}

fn lookup(
    bindings: &[SemanticExpandedDefinedCapabilityV1],
    view: &SemanticExpandedRootV1,
    source: Site,
    caller: SemanticCallInstanceIdV1,
    work: &mut usize,
) -> PhaseResult<usize> {
    let mut found = None;
    for (index, binding) in bindings.iter().enumerate() {
        spend(work, 1)?;
        if binding.root() == view.root()
            && site(binding) == source
            && binding.caller_instance() == caller
        {
            if binding.root_identity() != view.identity() || found.replace(index).is_some() {
                return Err(rejected(
                    "phase execution source occurrence is not unique in its exact instance",
                ));
            }
        }
    }
    found.ok_or_else(|| rejected("phase execution lost a checked original occurrence"))
}

fn increment(used: &mut [usize], index: usize, work: &mut usize) -> PhaseResult<()> {
    spend(work, 1)?;
    let value = used
        .get_mut(index)
        .ok_or_else(|| rejected("phase execution occurrence index"))?;
    *value = value
        .checked_add(1)
        .ok_or_else(|| rejected("phase execution occurrence count overflow"))?;
    Ok(())
}
