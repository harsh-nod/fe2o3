use super::*;
use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedDefinedCapabilityV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticDefinedCapabilityContractV1, SemanticKernelMathDeriveV1, SemanticPolicyMathBindV1,
};
use fe2o3_mir_model::{
    SemanticCallInstanceIdV1,
    SemanticExpandedStatementOriginV1, SemanticExpandedTerminatorOriginV1,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct MathStatementSiteV1 {
    pub(super) block: SemanticBlockIdV1,
    pub(super) statement: u32,
}

#[derive(Clone, Debug)]
pub(super) struct MathGetterOccurrenceV1 {
    pub(super) record: SemanticKernelMathDeriveV1,
    pub(super) binding: SemanticExpandedDefinedCapabilityV1,
    pub(super) return_site: MathStatementSiteV1,
    pub(super) bridge_instance: SemanticCallInstanceIdV1,
    pub(super) bridge_return_site: MathStatementSiteV1,
    pub(super) current_block: SemanticBlockIdV1,
}

#[derive(Clone, Debug)]
pub(super) struct MathBindOccurrenceV1 {
    pub(super) record: SemanticPolicyMathBindV1,
    pub(super) binding: SemanticExpandedDefinedCapabilityV1,
    pub(super) assignment_site: MathStatementSiteV1,
    pub(super) return_site: MathStatementSiteV1,
}

#[derive(Clone, Debug)]
pub(super) struct MathConsumerOccurrenceV1 {
    pub(super) contract: SemanticNumericalPolicyMathContractV1,
    pub(super) call: SemanticDirectCallV1,
    pub(super) instance: SemanticCallInstanceIdV1,
    pub(super) source_function: SemanticFunctionIdV1,
    pub(super) source_block: SemanticBlockIdV1,
    pub(super) execution_block: SemanticBlockIdV1,
}

/// Checked source coordinates only. The owner/loan resolver must still bind
/// these occurrences to actual SSA definitions before emitting capabilities.
#[derive(Debug)]
pub(super) struct MathOccurrencesV1 {
    pub(super) root: SemanticFunctionIdV1,
    pub(super) expansion_identity: [u8; 32],
    pub(super) root_identity: [u8; 32],
    pub(super) getters: BTreeMap<MathStatementSiteV1, MathGetterOccurrenceV1>,
    pub(super) binds: BTreeMap<MathStatementSiteV1, MathBindOccurrenceV1>,
    pub(super) consumers: BTreeMap<SemanticBlockIdV1, MathConsumerOccurrenceV1>,
}

struct Work {
    remaining: usize,
    limit: usize,
}

impl Work {
    fn charge(&mut self, units: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.remaining = self.remaining.checked_sub(units).ok_or(
            ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                actual: self.limit.saturating_add(1),
                limit: self.limit,
            },
        )?;
        Ok(())
    }
}

impl MathOccurrencesV1 {
    pub(super) fn new(
        owner: &ProductionSemanticSsaOwnerV1,
        context: &RootKernelContextLoweringV1,
        max_work: usize,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        owner
            .verify_replay()
            .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
        let view = owner
            .execution_view_for_root(context.selected_root)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let source = owner.source_semantic();
        let expansion = owner.execution_expansion();
        let bindings = expansion
            .defined_capability_bindings(source)
            .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let mut work = Work {
            remaining: max_work,
            limit: max_work,
        };
        work.charge(view.body().blocks().len())?;
        work.charge(view.instances().len())?;
        work.charge(bindings.len())?;
        let mut result = Self {
            root: view.root(),
            expansion_identity: *expansion.identity(),
            root_identity: *view.identity(),
            getters: BTreeMap::new(),
            binds: BTreeMap::new(),
            consumers: BTreeMap::new(),
        };
        let mut returns = BTreeMap::<SemanticCallInstanceIdV1, Vec<MathStatementSiteV1>>::new();
        let mut calls = BTreeMap::<SemanticCallInstanceIdV1, Vec<SemanticBlockIdV1>>::new();
        let mut children =
            BTreeMap::<SemanticCallInstanceIdV1, Vec<SemanticCallInstanceIdV1>>::new();
        for (index, instance) in view.instances().iter().enumerate() {
            if let Some(parent) = instance.parent() {
                let id = view
                    .block_origins()
                    .get(instance.block_start() as usize)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
                    .instance();
                if id.index() as usize != index {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                children.entry(parent).or_default().push(id);
            }
        }
        for (index, (block, origin)) in view
            .body()
            .blocks()
            .iter()
            .zip(view.block_origins())
            .enumerate()
        {
            let id = SemanticBlockIdV1::from_index(index as u32);
            work.charge(block.statements().len())?;
            if block.statements().len() != origin.statements().len() {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            for (statement, marker) in origin.statements().iter().enumerate() {
                if let SemanticExpandedStatementOriginV1::ReturnTransfer { callee } = marker {
                    returns
                        .entry(*callee)
                        .or_default()
                        .push(MathStatementSiteV1 {
                            block: id,
                            statement: statement as u32,
                        });
                }
            }
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            if origin.terminator() != SemanticExpandedTerminatorOriginV1::Source {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            calls.entry(origin.instance()).or_default().push(id);
            let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation: SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { contract },
                ..
            }) = source.callables().get(call.callee().index() as usize)
            else {
                continue;
            };
            let original = source
                .functions()
                .get(origin.function().index() as usize)
                .and_then(|function| function.blocks().get(origin.block().index() as usize))
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let SemanticTerminatorKindV1::Call(original_call) = original.terminator().kind() else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            if original_call.callee() != call.callee()
                || binding.identity() != contract.source_identity()
                || !global_capability_provenance_matches_v1(context, contract.provenance())
                || call
                    .arguments()
                    .iter()
                    .map(SemanticOperandV1::ty)
                    .ne(contract.signature().arguments())
                || call
                    .destination()
                    .map(|destination| destination.place().ty())
                    != Some(contract.types().element)
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            result.consumers.insert(
                id,
                MathConsumerOccurrenceV1 {
                    contract: *contract,
                    call: call.clone(),
                    instance: origin.instance(),
                    source_function: origin.function(),
                    source_block: origin.block(),
                    execution_block: id,
                },
            );
        }
        for binding in bindings {
            if binding.root() != result.root {
                continue;
            }
            if binding.expansion_identity() != &result.expansion_identity
                || binding.root_identity() != &result.root_identity
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            match binding.contract() {
                SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(_)
                | SemanticDefinedCapabilityContractV1::KernelMatrixDerive(_)
                | SemanticDefinedCapabilityContractV1::ReusableLdsConversion(_)
                | SemanticDefinedCapabilityContractV1::GuardedGridLeader(_)
                | SemanticDefinedCapabilityContractV1::ReusablePhase(_)
                | SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_)
                | SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(_) => continue,
                SemanticDefinedCapabilityContractV1::KernelMathDerive(record) => {
                    if !global_capability_provenance_matches_v1(context, record.provenance())
                        || record.types().context != context.semantic_type
                        || binding.arguments().len() != 1
                        || binding.callee_arguments().len() != 1
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    let return_site = one_return(&returns, binding.callee_instance())?;
                    check_return(view, return_site, &binding)?;
                    let nested = children
                        .get(&binding.callee_instance())
                        .map(Vec::as_slice)
                        .unwrap_or(&[]);
                    work.charge(nested.len())?;
                    let [bridge_instance] = nested else {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    };
                    let bridge = &view.instances()[bridge_instance.index() as usize];
                    if bridge.function() != record.bridge().function()
                        || bridge.function_identity() != record.bridge().source_identity()
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    let current_calls =
                        calls.get(bridge_instance).map(Vec::as_slice).unwrap_or(&[]);
                    let [current_block] = current_calls else {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    };
                    let SemanticTerminatorKindV1::Call(current) = view.body().blocks()
                        [current_block.index() as usize]
                        .terminator()
                        .kind()
                    else {
                        unreachable!()
                    };
                    if current.callee() != record.current_callable()
                        || !current.arguments().is_empty()
                        || current.destination().map(|d| d.place().ty())
                            != Some(record.types().unbranded_math)
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    let occurrence = MathGetterOccurrenceV1 {
                        record,
                        return_site,
                        bridge_instance: *bridge_instance,
                        bridge_return_site: one_return(&returns, *bridge_instance)?,
                        current_block: *current_block,
                        binding,
                    };
                    if result.getters.insert(return_site, occurrence).is_some() {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                }
                SemanticDefinedCapabilityContractV1::PolicyMathBind(record) => {
                    if !global_capability_provenance_matches_v1(context, record.provenance())
                        || binding.arguments().len() != 2
                        || binding.callee_arguments().len() != 2
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    let assignment_site = MathStatementSiteV1 {
                        block: binding.expanded_entry_block(),
                        statement: 0,
                    };
                    let origin = &view.block_origins()[assignment_site.block.index() as usize];
                    let assignment = assignment(view, assignment_site)?;
                    let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind()
                    else {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    };
                    if origin.instance() != binding.callee_instance() || origin.function() != record.function()
                        || origin.statements().first() != Some(&SemanticExpandedStatementOriginV1::Source { statement: 0 })
                        || assignment.destination().local() != binding.callee_return()
                        || !assignment.destination().projections().is_empty() || assignment.destination().ty() != record.types().bound
                        || aggregate.operands().len() != 3
                        || !aggregate.operands()[..2].iter().zip(binding.callee_arguments()).all(|(operand, local)|
                            matches!(operand, SemanticOperandV1::Copy(place) if place.local() == *local && place.projections().is_empty()))
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                    let return_site = one_return(&returns, binding.callee_instance())?;
                    check_return(view, return_site, &binding)?;
                    if result
                        .binds
                        .insert(
                            assignment_site,
                            MathBindOccurrenceV1 {
                                record,
                                assignment_site,
                                return_site,
                                binding,
                            },
                        )
                        .is_some()
                    {
                        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                    }
                }
            }
        }
        Ok(result)
    }
}

fn one_return(
    returns: &BTreeMap<SemanticCallInstanceIdV1, Vec<MathStatementSiteV1>>,
    instance: SemanticCallInstanceIdV1,
) -> Result<MathStatementSiteV1, ProductionSemanticKirErrorV1> {
    match returns.get(&instance).map(Vec::as_slice) {
        Some([site]) => Ok(*site),
        _ => Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
    }
}

fn assignment(
    view: &SemanticExpandedRootV1,
    site: MathStatementSiteV1,
) -> Result<&fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1, ProductionSemanticKirErrorV1> {
    match view
        .body()
        .blocks()
        .get(site.block.index() as usize)
        .and_then(|block| block.statements().get(site.statement as usize))
        .map(|statement| statement.kind())
    {
        Some(SemanticStatementKindV1::Assign(assignment)) => Ok(assignment),
        _ => Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
    }
}

fn check_return(
    view: &SemanticExpandedRootV1,
    site: MathStatementSiteV1,
    binding: &SemanticExpandedDefinedCapabilityV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let assignment = assignment(view, site)?;
    if assignment.destination() != binding.destination()
        || !matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place))
            if place.local() == binding.callee_return() && place.projections().is_empty() && place.ty() == binding.destination().ty())
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(())
}
