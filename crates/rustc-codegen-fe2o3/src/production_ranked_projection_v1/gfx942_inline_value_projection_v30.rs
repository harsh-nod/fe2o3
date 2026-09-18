//! Scalar values of the six compiler-admitted, direct-root gfx942 markers.
//! This is a private source-expression projection, not call inlining, source
//! authentication, instruction rewriting, or an independent admission route.

use super::{
    GpuSemanticExpressionResolverV2, HashSet, ProductionOverflowContractV2,
    ProductionRankedProjectionErrorV1, ProductionSemanticBinaryOpV2,
    ProductionSemanticExpressionV2, ProductionSemanticScalarTypeV2, ScalarAssignmentSiteV1,
    SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1, SemanticEdgeRoleV1,
    SemanticFunctionRoleV1, SemanticPlaceV1, SemanticTerminatorKindV1, SemanticUnwindActionV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticGfx942InlineInstructionV30, SemanticGfx942InlineU32V30,
};

/// Borrowed exact roster and once-built local index; no executable body is copied.
pub(super) struct InlineCallRosterV30<'a> {
    callables: &'a [SemanticCallableDeclV1],
    blocks: Vec<Option<usize>>,
}

const U32: ProductionSemanticScalarTypeV2 = ProductionSemanticScalarTypeV2::Integer {
    signed: false,
    bits: 32,
};

impl<'a> GpuSemanticExpressionResolverV2<'a> {
    pub(super) fn with_gfx942_inline_callables_v30(
        mut self,
        callables: &'a [SemanticCallableDeclV1],
    ) -> Result<Self, ProductionRankedProjectionErrorV1> {
        // Charge before allocation/scanning using the existing cumulative graph
        // ledger. Repeated result queries do not rescan all function bodies.
        self.definitions.charge(callables.len())?;
        if !callables.iter().any(|callable| {
            matches!(
                callable,
                SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(_),
                    ..
                }
            )
        }) {
            // Ordinary source retains no additional local index or fallback.
            return Ok(self);
        }
        self.definitions.charge(self.function.locals().len())?;
        let mut blocks = Vec::new();
        blocks
            .try_reserve_exact(self.function.locals().len())
            .map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "GPU typed ISA call index storage cannot be reserved",
                )
            })?;
        blocks.resize(self.function.locals().len(), None);
        for (index, block) in self.function.blocks().iter().enumerate() {
            self.definitions.charge(1)?;
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                continue;
            };
            if !matches!(
                callables.get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(_),
                    ..
                })
            ) {
                continue;
            }
            let Some(destination) = call.destination() else {
                continue;
            };
            if !destination.place().projections().is_empty() {
                continue;
            }
            let slot = blocks
                .get_mut(destination.place().local().index() as usize)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "GPU typed ISA call destination is outside the local table",
                ))?;
            // The query checks the global definition census, including ordinary
            // and projected writes. Never select one of several definitions.
            if slot.is_none() {
                *slot = Some(index);
            }
        }
        self.inline_calls_v30 = Some(InlineCallRosterV30 { callables, blocks });
        Ok(self)
    }

    pub(super) fn resolve_gfx942_inline_call_result_v30(
        &mut self,
        place: &'a SemanticPlaceV1,
        depth: usize,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        Self::require_depth_v2(depth)?;
        self.charge_v2()?;
        let local = place.local().index() as usize;
        if !place.projections().is_empty() {
            return Err("GPU typed ISA call result cannot have aggregate projections");
        }
        let Some(roster) = self.inline_calls_v30.as_ref() else {
            return Err("GPU semantic local has no exact reaching assignment");
        };
        let Some(call_block) = roster.blocks.get(local).copied().flatten() else {
            return Err("GPU semantic local has no exact reaching assignment");
        };
        if self.function.role() != SemanticFunctionRoleV1::KernelRoot {
            return Err("GPU typed ISA scalar projection supports direct kernel roots only");
        }
        if self.definitions.definition_counts.get(local).copied() != Some(1) {
            return Err("GPU typed ISA call result has no single global definition");
        }
        if self.borrowed.contains(&(local as u32))
            || self.definitions.address_escaped.get(local).copied() != Some(false)
        {
            return Err("GPU typed ISA call result has an escaped or borrowed address");
        }
        let block = &self.function.blocks()[call_block];
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            return Err("GPU typed ISA indexed definition is not an exact call");
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(assembly),
            ..
        }) = roster.callables.get(call.callee().index() as usize)
        else {
            return Err("GPU typed ISA result is not an admitted gfx942 marker");
        };
        let source = call
            .inline_assembly_source_v30()
            .ok_or("GPU typed ISA call has no retained source occurrence")?;
        if source.function() != self.function.identity()
            || [
                source.frontend_unit(),
                *source.function().as_bytes(),
                source.contract(),
                source.statement(),
            ]
            .contains(&[0; 32])
        {
            return Err("GPU typed ISA source occurrence belongs to another caller");
        }
        // Gfx942InlineU32's closed source grammar fixes target and VGPR32 class.
        // The independent lowerer relation checks actual KIR target/operands.
        if assembly.option_bits() != SemanticGfx942InlineU32V30::NOMEM_OPTION_BITS {
            return Err("GPU typed ISA option contract is unsupported");
        }
        let destination = call
            .destination()
            .ok_or("GPU typed ISA call has no result")?;
        let ty = place.ty();
        if destination.place() != place
            || self
                .function
                .locals()
                .get(local)
                .is_none_or(|declaration| declaration.ty() != ty)
            || self.scalar_v2(ty)? != U32
            || call.arguments().len() != assembly.input_count()
            || call.arguments().iter().any(|argument| argument.ty() != ty)
            || !call.variadic_argument_abis().is_empty()
            || binding.abi().c_variadic()
            || binding.abi().source_output_type() != ty
            || binding.abi().source_input_types().len() != assembly.input_count()
            || binding
                .abi()
                .source_input_types()
                .iter()
                .any(|input| *input != ty)
        {
            return Err("GPU typed ISA call operands and result require the exact u32 ABI");
        }
        if binding.abi().can_unwind()
            || !matches!(
                call.unwind(),
                SemanticUnwindActionV1::Unreachable | SemanticUnwindActionV1::Continue
            )
            || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
        {
            return Err("GPU typed ISA call requires a non-unwinding normal return edge");
        }
        let use_site = self
            .use_site
            .ok_or("GPU typed ISA result has no exact use site")?;
        let mut edge = HashSet::new();
        edge.try_reserve(1)
            .map_err(|_| "GPU typed ISA return-edge storage cannot be reserved")?;
        edge.insert((call_block, destination.edge().target().index() as usize));
        if !self
            .definitions
            .edge_set_dominates(&edge, use_site.block)
            .map_err(|_| "GPU typed ISA return-edge dominance exceeds its analysis budget")?
        {
            return Err("GPU typed ISA normal return edge does not dominate its use");
        }
        let definition = ScalarAssignmentSiteV1 {
            block: call_block,
            statement: block.statements().len(),
        };
        let key = (local as u32, definition.block, definition.statement);
        if !self.visiting.insert(key) {
            return Err("GPU typed ISA scalar result has a cyclic definition");
        }
        // Inputs are values at the call, not values of mutated locals at the store.
        let previous = self.use_site.replace(definition);
        let resolved = match assembly.instruction() {
            SemanticGfx942InlineInstructionV30::VMovB32 => {
                self.resolve_operand_v2(&call.arguments()[0], depth + 1)
            }
            instruction => {
                let operation = match instruction {
                    SemanticGfx942InlineInstructionV30::VAddU32 => {
                        ProductionSemanticBinaryOpV2::Add
                    }
                    SemanticGfx942InlineInstructionV30::VSubU32 => {
                        ProductionSemanticBinaryOpV2::Subtract
                    }
                    SemanticGfx942InlineInstructionV30::VAndB32 => {
                        ProductionSemanticBinaryOpV2::BitAnd
                    }
                    SemanticGfx942InlineInstructionV30::VOrB32 => {
                        ProductionSemanticBinaryOpV2::BitOr
                    }
                    SemanticGfx942InlineInstructionV30::VXorB32 => {
                        ProductionSemanticBinaryOpV2::BitXor
                    }
                    SemanticGfx942InlineInstructionV30::VMovB32 => unreachable!(),
                };
                self.binary_expression_v2(
                    operation,
                    ProductionOverflowContractV2::Wrapping,
                    U32,
                    &call.arguments()[0],
                    &call.arguments()[1],
                    depth,
                )
            }
        };
        self.use_site = previous;
        self.visiting.remove(&key);
        resolved
    }
}
