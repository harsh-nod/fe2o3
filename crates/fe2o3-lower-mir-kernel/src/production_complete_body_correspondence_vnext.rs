// Explicit many-to-many expansion/elimination attribution; no legacy fake spans.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Exact attribution of each admitted source terminator.
pub enum CompleteBodySourceTerminatorVNext {
    /// Source-only transport eliminated from executable KIR.
    TransportGoto {
        /// Exact source successor.
        successor: SemanticBlockIdV1,
    },
    /// One marker expands into the complete authored graph and fixed tail.
    ExpandedMarker {
        /// Exact source continuation after the marker.
        successor: SemanticBlockIdV1,
        /// Number of authored blocks.
        authored_blocks: u8,
        /// Number of authored arithmetic instructions.
        authored_steps: u8,
    },
    /// The source root's unit return.
    UnitReturn,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Exact source block attribution, including explicit eliminated statements.
pub struct CompleteBodySourceBlockVNext {
    /// Dense source block identifier.
    pub block: SemanticBlockIdV1,
    /// Full retained source block identity.
    pub identity: fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdentityV1,
    /// All statement ordinals 0..count were checked as transport/unit/Nop.
    /// They are eliminated, not mapped onto an invented emitted instruction.
    pub eliminated_statement_count: u32,
    /// Exact terminator expansion or elimination.
    pub terminator: CompleteBodySourceTerminatorVNext,
}
#[derive(Debug, Eq, PartialEq)]
/// Readonly actual source-to-canonical expansion relation.
pub struct CompleteBodySourceCorrespondenceVNext {
    semantic_sha256: [u8; 32],
    root: SemanticFunctionIdV1,
    source_blocks: Vec<CompleteBodySourceBlockVNext>,
    /// Exact ABI source local and newly assigned canonical parameter ID.
    parameters: [(SemanticLocalIdV1, ValueId); 5],
}
impl CompleteBodySourceCorrespondenceVNext {
    /// All source blocks in their original order.
    pub fn source_blocks(&self) -> &[CompleteBodySourceBlockVNext] {
        &self.source_blocks
    }
    /// Exact source ABI-local to canonical parameter mapping.
    pub const fn parameters(&self) -> &[(SemanticLocalIdV1, ValueId); 5] {
        &self.parameters
    }
    /// Identity of the retained admitted source owner.
    pub const fn semantic_sha256(&self) -> &[u8; 32] {
        &self.semantic_sha256
    }
    /// Actual source root function.
    pub const fn root(&self) -> SemanticFunctionIdV1 {
        self.root
    }
    fn capture(
        context: &CompleteBodyContextVNext<'_>,
        semantic_sha256: [u8; 32],
        plan: &LoweredFunctionPlanV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(Self, usize), ProductionSemanticKirErrorV1> {
        // Plan comes only from the unchanged kernel_entry_plan_v1 on this owner.
        if plan.correspondence_owner != context.root
            || plan.semantic_function != context.root
            || plan.role != SemanticKirFunctionRoleV1::KernelEntry
            || plan.parameter_declarations.len() != 5
            || plan.parameter_local_bindings.len() != 5
            || !plan.parameter_component_bindings.is_empty()
            || !plan.ignored_parameter_bindings.is_empty()
            || !plan.result_types.is_empty()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let mut parameters = [(SemanticLocalIdV1::from_index(0), ValueId(0)); 5];
        for (ordinal, ((argument, local, ty), binding)) in plan
            .parameter_declarations
            .iter()
            .zip(&plan.parameter_local_bindings)
            .enumerate()
        {
            if *argument != ordinal as u32
                || *local != context.argument_locals[ordinal]
                || *ty != context.function.abi().source_input_types()[ordinal]
                || !matches!(binding, PlannedParameterLocalBindingV1::Direct { local: bound, ty: bound_ty, .. }
                    if bound == local && plan.parameter_types.get(ordinal) == Some(bound_ty))
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            // These ValueIds are definitions allocated by the sole importer.
            // Numeric local indexes never stand in for source/owner identities.
            parameters[ordinal] = (
                SemanticLocalIdV1::from_index(*local as u32),
                ValueId(ordinal as u32),
            );
        }
        let bytes = argument_product_v1(
            context.function.blocks().len(),
            std::mem::size_of::<CompleteBodySourceBlockVNext>(),
        )?;
        budget.charge_work(argument_product_v1(context.function.blocks().len(), 8)?)?;
        budget.reserve_storage(bytes)?;
        let mut source_blocks = Vec::new();
        source_blocks
            .try_reserve_exact(context.function.blocks().len())
            .map_err(|_| ArgumentResourceV1::Allocation)?;
        for (ordinal, block) in context.function.blocks().iter().enumerate() {
            let id = SemanticBlockIdV1::from_index(ordinal as u32);
            let terminator = match block.terminator().kind() {
                SemanticTerminatorKindV1::Goto(edge) => {
                    CompleteBodySourceTerminatorVNext::TransportGoto {
                        successor: edge.target(),
                    }
                }
                SemanticTerminatorKindV1::Call(call) if id == context.marker => {
                    let edge = call
                        .destination()
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
                        .edge();
                    CompleteBodySourceTerminatorVNext::ExpandedMarker {
                        successor: edge.target(),
                        authored_blocks: context.input.packed.block_count(),
                        authored_steps: context.input.packed.instruction_count(),
                    }
                }
                SemanticTerminatorKindV1::Return => CompleteBodySourceTerminatorVNext::UnitReturn,
                _ => return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
            };
            source_blocks.push(CompleteBodySourceBlockVNext {
                block: id,
                identity: block.identity(),
                eliminated_statement_count: u32::try_from(block.statements().len())
                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                terminator,
            });
        }
        Ok((
            Self {
                semantic_sha256,
                root: context.root,
                source_blocks,
                parameters,
            },
            bytes,
        ))
    }
}
