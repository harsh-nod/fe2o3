// Exact source elimination/marker relation, not fabricated native source spans.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Exact elimination or marker continuation in retained semantic source.
pub enum PhysicalEntrySourceTerminatorV20 {
    /// Eliminated pure source transport.
    TransportGoto {
        /// Actual semantic successor.
        successor: SemanticBlockIdV1,
    },
    /// One exact Begin, Label or Step source occurrence; native attribution is
    /// joined through this occurrence in the verified declaration/steps.
    Marker {
        /// Actual semantic continuation.
        successor: SemanticBlockIdV1,
        /// Dense source occurrence, not a native ordinal.
        occurrence: u8,
        /// Exact admitted callable identity.
        callee: fe2o3_mir_model::semantic_mir_v1::SemanticCallableIdV1,
    },
    /// Eliminated unit source return, not a compiler-added native return.
    UnitReturn,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// One retained source block; no fabricated native source span.
pub struct PhysicalEntrySourceBlockV20 {
    /// Exact semantic block coordinate.
    pub block: SemanticBlockIdV1,
    /// Actual semantic block identity.
    pub identity: fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdentityV1,
    /// Pure transport/storage/unit statements validated before elimination.
    pub eliminated_statement_count: u32,
    /// Exact retained source terminator classification.
    pub terminator: PhysicalEntrySourceTerminatorV20,
}
#[derive(Debug, Eq, PartialEq)]
/// Immutable full source-block and logical-parameter correspondence.
pub struct PhysicalEntrySourceCorrespondenceV20 {
    semantic_sha256: [u8; 32],
    root: SemanticFunctionIdV1,
    source_blocks: Vec<PhysicalEntrySourceBlockV20>,
    parameters: [(SemanticLocalIdV1, ValueId); 5],
}
impl PhysicalEntrySourceCorrespondenceV20 {
    /// Every original semantic block, including eliminated transport.
    pub fn source_blocks(&self) -> &[PhysicalEntrySourceBlockV20] {
        &self.source_blocks
    }
    /// Actual root argument locals and canonical logical parameter IDs.
    pub const fn parameters(&self) -> &[(SemanticLocalIdV1, ValueId); 5] {
        &self.parameters
    }
    /// Retained semantic digest; not a substitute for full owner replay.
    pub const fn semantic_sha256(&self) -> &[u8; 32] {
        &self.semantic_sha256
    }
    /// Actual selected semantic root.
    pub const fn root(&self) -> SemanticFunctionIdV1 {
        self.root
    }
    fn capture(
        context: &PhysicalEntryContextV20<'_>,
        semantic_sha256: [u8; 32],
        plan: &LoweredFunctionPlanV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(Self, usize), ProductionSemanticKirErrorV1> {
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
                || !matches!(binding,PlannedParameterLocalBindingV1::Direct{local:bound,ty:bound_ty,..}
                    if bound==local&&plan.parameter_types.get(ordinal)==Some(bound_ty))
            {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            parameters[ordinal] = (
                SemanticLocalIdV1::from_index(*local as u32),
                ValueId(ordinal as u32),
            );
        }
        let bytes = argument_product_v1(
            context.function.blocks().len(),
            std::mem::size_of::<PhysicalEntrySourceBlockV20>(),
        )?;
        budget.charge_work(argument_product_v1(context.function.blocks().len(), 8)?)?;
        budget.reserve_storage(bytes)?;
        let mut source_blocks = Vec::new();
        source_blocks
            .try_reserve_exact(context.function.blocks().len())
            .map_err(|_| ArgumentResourceV1::Allocation)?;
        for (ordinal, block) in context.function.blocks().iter().enumerate() {
            let terminator = match block.terminator().kind() {
                SemanticTerminatorKindV1::Goto(edge) => {
                    PhysicalEntrySourceTerminatorV20::TransportGoto {
                        successor: edge.target(),
                    }
                }
                SemanticTerminatorKindV1::Call(call) => {
                    let source = call
                        .physical_entry_source_v37()
                        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                    PhysicalEntrySourceTerminatorV20::Marker {
                        successor: call
                            .destination()
                            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
                            .edge()
                            .target(),
                        occurrence: source.occurrence(),
                        callee: call.callee(),
                    }
                }
                SemanticTerminatorKindV1::Return => PhysicalEntrySourceTerminatorV20::UnitReturn,
                _ => return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
            };
            source_blocks.push(PhysicalEntrySourceBlockV20 {
                block: SemanticBlockIdV1::from_index(ordinal as u32),
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
