mod execution_source_occurrence_01 {
    use super::*;
    use fe2o3_kernel_ir::ExecutionCapabilitySourceOccurrenceV1;
    use fe2o3_mir_model::SemanticExpandedTerminatorOriginV1;
    use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionAbiV1;

    include!("execution_source_occurrence_01/defined.rs");
    #[cfg(test)]
    include!("execution_source_occurrence_01/abi_tests.rs");

    /// Constructed once from the replayed owner, never from caller-selected
    /// occurrence records. Exact execution calls stay inseparable from origins.
    pub(super) struct CheckedExecutionSourceCarrierV1 {
        root: SemanticFunctionIdV1,
        function: SemanticFunctionIdentityV1,
        expansion: Option<[u8; 32]>,
        expanded_root: Option<[u8; 32]>,
        calls: BTreeMap<u32, CheckedSourceCallV1>,
    }

    struct CheckedSourceCallV1 {
        execution: SemanticDirectCallV1,
        source: ExecutionCapabilitySourceV1,
    }

    impl CheckedExecutionSourceCarrierV1 {
        pub(super) fn new(
            owner: &ProductionSemanticSsaOwnerV1,
            root: SemanticFunctionIdV1,
            function: &SemanticFunctionDeclV1,
        ) -> Result<Self, ProductionSemanticKirErrorV1> {
            owner
                .verify_replay()
                .map_err(ProductionSemanticKirErrorV1::SemanticSsa)?;
            let semantic = owner.source_semantic();
            let view = owner
                .execution_view_for_root(root)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let original_root = semantic
                .functions()
                .get(root.index() as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if function != execution_function_for_root_v1(owner, root)?
                || view.root() != root
                || view.block_origins().len() != function.blocks().len()
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            let expanded_root = view.has_expanded_calls().then(|| *view.identity());
            let mut calls = BTreeMap::new();
            for (block, execution_block) in function.blocks().iter().enumerate() {
                let SemanticTerminatorKindV1::Call(execution) = execution_block.terminator().kind()
                else {
                    continue;
                };
                let origin = &view.block_origins()[block];
                // Defined call entries/returns are synthetic transfers, not
                // surviving compiler intrinsic source terminators.
                if origin.terminator() != SemanticExpandedTerminatorOriginV1::Source {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                let instance = view
                    .instances()
                    .get(origin.instance().index() as usize)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                let original = semantic
                    .functions()
                    .get(origin.function().index() as usize)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                if instance.function() != origin.function()
                    || instance.function_identity() != original.identity()
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                let original_block = original
                    .blocks()
                    .get(origin.block().index() as usize)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                let SemanticTerminatorKindV1::Call(original_call) =
                    original_block.terminator().kind()
                else {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                };
                if original_call.callee() != execution.callee() {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                let callable = semantic
                    .callables()
                    .get(original_call.callee().index() as usize)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                let SemanticCallableDeclV1::CompilerIntrinsic {
                    binding, operation, ..
                } = callable
                else {
                    // No source carrier for device FFI or surviving defined
                    // calls. Their existing lowering boundaries still reject.
                    continue;
                };
                if !matches!(
                    operation,
                    SemanticCompilerIntrinsicOperationV1::ExecutionCapability { .. }
                        | SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { .. }
                ) {
                    continue;
                }
                require_abi(original_call, binding.abi())?;
                require_abi(execution, binding.abi())?;
                let expanded_block = u32::try_from(block)
                    .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                let occurrence = if view.has_expanded_calls() {
                    Some(
                        ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                            *original_root.identity().as_bytes(),
                            *owner.execution_expansion().identity(),
                            *view.identity(),
                            origin.instance().index(),
                            expanded_block,
                        )
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
                    )
                } else {
                    None
                };
                let source = ExecutionCapabilitySourceV1 {
                    function: *original.identity().as_bytes(),
                    operation: *binding.identity().as_bytes(),
                    block: origin.block().index(),
                    occurrence,
                };
                if !source.is_complete()
                    || calls
                        .insert(
                            expanded_block,
                            CheckedSourceCallV1 {
                                execution: execution.clone(),
                                source,
                            },
                        )
                        .is_some()
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
            }
            Ok(Self {
                root,
                function: function.identity(),
                expansion: execution_expansion_identity_v1(owner),
                expanded_root,
                calls,
            })
        }

        pub(super) fn source_for_call(
            &self,
            root: SemanticFunctionIdV1,
            function: &SemanticFunctionDeclV1,
            expansion: Option<[u8; 32]>,
            expanded_root: Option<[u8; 32]>,
            block: SemanticBlockIdV1,
            call: &SemanticDirectCallV1,
            callee: SemanticFunctionIdentityV1,
        ) -> Result<ExecutionCapabilitySourceV1, ProductionSemanticKirErrorV1> {
            let record = self
                .calls
                .get(&block.index())
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if root != self.root
                || function.identity() != self.function
                || expansion != self.expansion
                || expanded_root != self.expanded_root
                || call != &record.execution
                || record.source.operation != *callee.as_bytes()
                || !matches!(function.blocks().get(block.index() as usize).map(|b| b.terminator().kind()), Some(SemanticTerminatorKindV1::Call(actual)) if actual == call)
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            Ok(record.source)
        }
    }

    fn require_abi(
        call: &SemanticDirectCallV1,
        abi: &SemanticFunctionAbiV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if abi.c_variadic()
            || call
                .arguments()
                .iter()
                .map(semantic_operand_type)
                .ne(abi.source_input_types().iter().copied())
            || call.destination().map(|d| d.place().ty()) != Some(abi.source_output_type())
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(())
    }
}

use execution_source_occurrence_01::CheckedExecutionSourceCarrierV1;
