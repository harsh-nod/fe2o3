use fe2o3_mir_model::semantic_mir_v1::SemanticDefinedCapabilityContractV1;
use fe2o3_mir_model::SemanticExpandedStatementOriginV1;
use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedDefinedCapabilityV1;

impl CheckedExecutionSourceCarrierV1 {
    /// Source attribution only. The Math plan must separately prove exact SSA
    /// receiver/issuer custody; this cannot issue Math or discharge FP policy.
    /// Only the retained getter return transfer and Bind source assignment are
    /// accepted. In particular, a generic CallEntry cannot obtain a record.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn source_for_defined_statement(
        owner: &ProductionSemanticSsaOwnerV1,
        root: SemanticFunctionIdV1,
        function: &SemanticFunctionDeclV1,
        expansion_identity: [u8; 32],
        expanded_root_identity: [u8; 32],
        expected: &SemanticExpandedDefinedCapabilityV1,
        block: SemanticBlockIdV1,
        statement: u32,
    ) -> Result<ExecutionCapabilitySourceV1, ProductionSemanticKirErrorV1> {
        owner
            .verify_replay()
            .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
        let source = owner.source_semantic();
        let view = owner
            .execution_view_for_root(root)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let bindings = owner
            .execution_expansion()
            .defined_capability_bindings(source)
            .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let mut matches = bindings.iter().filter(|binding| *binding == expected);
        let binding = matches
            .next()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if matches.next().is_some()
            || binding.root() != root
            || binding.contract().provenance().root() != root
            || !view.has_expanded_calls()
            || function != execution_function_for_root_v1(owner, root)?
            || expansion_identity != *owner.execution_expansion().identity()
            || expansion_identity != *binding.expansion_identity()
            || expanded_root_identity != *view.identity()
            || expanded_root_identity != *binding.root_identity()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let caller = source
            .functions()
            .get(binding.caller_function().index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let caller_instance = view
            .instances()
            .get(binding.caller_instance().index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let callee = source
            .functions()
            .get(binding.contract().function().index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let callee_instance = view
            .instances()
            .get(binding.callee_instance().index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let original_call = match caller
            .blocks()
            .get(binding.call_block().index() as usize)
            .map(|block| block.terminator().kind())
        {
            Some(SemanticTerminatorKindV1::Call(call)) => call,
            _ => return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
        };
        if caller_instance.function() != binding.caller_function()
            || caller_instance.function_identity() != caller.identity()
            || callee_instance.function() != binding.contract().function()
            || callee_instance.function_identity() != callee.identity()
            || callee_instance.parent() != Some(binding.caller_instance())
            || callee_instance.call_block() != Some(binding.call_block())
            || callee.identity() != binding.contract().source_identity()
            || callee.defined_capability_contract() != Some(&binding.contract())
            || !matches!(source.callables().get(original_call.callee().index() as usize),
                Some(SemanticCallableDeclV1::Defined { function }) if *function == binding.contract().function())
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        require_abi(original_call, callee.abi())?;
        let entry_origin = view
            .block_origins()
            .get(binding.expanded_call_block().index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if entry_origin.instance() != binding.caller_instance()
            || entry_origin.function() != binding.caller_function()
            || entry_origin.block() != binding.call_block()
            || entry_origin.terminator()
                != (SemanticExpandedTerminatorOriginV1::CallEntry {
                    callee: binding.callee_instance(),
                })
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let origin = view
            .block_origins()
            .get(block.index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let actual = function
            .blocks()
            .get(block.index() as usize)
            .and_then(|block| block.statements().get(statement as usize))
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if origin.instance() != binding.callee_instance()
            || origin.function() != binding.contract().function()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let valid_site = match binding.contract() {
            // No guarded-source-to-KIR correspondence is implemented by this packet.
            SemanticDefinedCapabilityContractV1::GuardedGridLeader(_) => false,
            SemanticDefinedCapabilityContractV1::ReusablePhase(_) => false,
            SemanticDefinedCapabilityContractV1::KernelMathDerive(_)
            | SemanticDefinedCapabilityContractV1::KernelMatrixDerive(_)
            | SemanticDefinedCapabilityContractV1::ReusableLdsConversion(_) => {
                origin.terminator()
                    == (SemanticExpandedTerminatorOriginV1::CallReturn {
                        callee: binding.callee_instance(),
                    })
                    && origin.statements().get(statement as usize)
                        == Some(&SemanticExpandedStatementOriginV1::ReturnTransfer {
                            callee: binding.callee_instance(),
                        })
                    && matches!(actual.kind(), SemanticStatementKindV1::Assign(assignment)
                        if assignment.destination() == binding.destination()
                            && matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place))
                                if place.local() == binding.callee_return() && place.projections().is_empty()
                                    && place.ty() == binding.destination().ty()))
            }
            SemanticDefinedCapabilityContractV1::PolicyMathBind(record) => {
                // The admitted closed Bind recipe has exactly one source
                // assignment at entry/statement zero. Its remapped operands
                // must still be the exact argument locals of this instance.
                block == binding.expanded_entry_block()
                    && statement == 0
                    && origin.block() == callee.entry()
                    && origin.statements().first()
                        == Some(&SemanticExpandedStatementOriginV1::Source { statement: 0 })
                    && matches!(actual.kind(), SemanticStatementKindV1::Assign(assignment)
                        if assignment.destination().local() == binding.callee_return()
                            && assignment.destination().projections().is_empty()
                            && assignment.destination().ty() == record.types().bound
                            && matches!(assignment.value().kind(), SemanticRvalueKindV1::Aggregate(aggregate)
                                if aggregate.operands().len() == 3 && binding.callee_arguments().len() == 2
                                    && aggregate.operands()[..2].iter().zip(binding.callee_arguments()).all(|(operand, local)|
                                        matches!(operand, SemanticOperandV1::Copy(place) if place.local() == *local && place.projections().is_empty()))))
            }
            SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(_)
            | SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_)
            | SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(_) => false,
        };
        if !valid_site {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(ExecutionCapabilitySourceV1 {
            function: *caller.identity().as_bytes(),
            operation: *callee.identity().as_bytes(),
            block: binding.call_block().index(),
            occurrence: Some(
                ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                    *source
                        .functions()
                        .get(root.index() as usize)
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
                        .identity()
                        .as_bytes(),
                    expansion_identity,
                    expanded_root_identity,
                    binding.caller_instance().index(),
                    block.index(),
                )
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
            ),
        })
    }
}
