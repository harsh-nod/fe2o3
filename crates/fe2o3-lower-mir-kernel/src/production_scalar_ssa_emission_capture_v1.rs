use super::*;

impl Recorder {
    pub(super) fn prepare(
        owner: &ProductionSemanticSsaOwnerV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        let mut result = Self {
            expected: Vec::new(),
            functions: Vec::new(),
            definitions: Vec::new(),
            edges: Vec::new(),
        };
        let source = owner.source_semantic();
        let occurrences = owner
            .occurrences_v1()
            .ok_or(Error::Mismatch("captured source occurrences are required"))?;
        for (index, function) in source.functions().iter().enumerate() {
            budget.charge_work(2)?;
            let id = SemanticFunctionIdV1::from_index(
                u32::try_from(index).map_err(|_| Resource::Arithmetic)?,
            );
            let Some(plan) = owner.plan_for_function(id) else {
                continue;
            };
            let rows = occurrences
                .function(id)
                .ok_or(Error::Mismatch("source occurrence function"))?;
            for entry in rows.entry_definitions() {
                budget.charge_work(1)?;
                if let Some(value) = entry.value() {
                    result.prepare_one(
                        source.types(),
                        function,
                        id,
                        value,
                        entry.variable(),
                        SourceSite::Entry,
                        budget,
                    )?;
                }
            }
            for block in plan.plan().reverse_postorder() {
                budget.charge_work(1)?;
                for &variable in plan
                    .plan()
                    .transport_variables(*block)
                    .ok_or(Error::Mismatch("source transport block"))?
                {
                    result.prepare_one(
                        source.types(),
                        function,
                        id,
                        SsaValueV1::BlockArgument {
                            block: *block,
                            variable,
                        },
                        variable,
                        SourceSite::Header(block.get()),
                        budget,
                    )?;
                }
            }
            for event in rows.events() {
                budget.charge_work(1)?;
                if event.is_reachable()
                    && event.is_promoted()
                    && let Some(SsaResolvedEventV1::Define { variable, value }) = event.resolved()
                {
                    result.prepare_one(
                        source.types(),
                        function,
                        id,
                        value,
                        variable,
                        SourceSite::Event(event.site()),
                        budget,
                    )?;
                }
            }
            for edge in rows.edge_definitions() {
                budget.charge_work(1)?;
                if edge.is_reachable() && edge.is_promoted() {
                    let value = edge
                        .value()
                        .ok_or(Error::Mismatch("promoted source edge definition"))?;
                    result.prepare_one(
                        source.types(),
                        function,
                        id,
                        value,
                        edge.variable(),
                        SourceSite::Edge(edge.edge()),
                        budget,
                    )?;
                }
            }
        }
        resources::sort_work(result.expected.len(), budget)?;
        result.expected.sort_unstable_by_key(|row| row.key());
        budget.charge_work(result.expected.len())?;
        if result
            .expected
            .windows(2)
            .any(|rows| rows[0].key() == rows[1].key())
        {
            return Err(Error::Mismatch("duplicate source definition"));
        }
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_one(
        &mut self,
        types: &[SemanticTypeDeclV1],
        function: &SemanticFunctionDeclV1,
        id: SemanticFunctionIdV1,
        value: SsaValueV1,
        variable: SsaVariableIdV1,
        site: SourceSite,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        budget.charge_work(3)?;
        if let Some(shape) = source_shape(types, function, variable, site)? {
            append(
                &mut self.expected,
                Expected {
                    function: id,
                    value,
                    variable,
                    site,
                    shape,
                },
                budget,
            )?;
        }
        Ok(())
    }

    pub(super) fn expected_range(
        &self,
        function: SemanticFunctionIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<Range<usize>> {
        charge_lookup(self.expected.len(), budget)?;
        let first = self
            .expected
            .partition_point(|row| row.function.index() < function.index());
        charge_lookup(self.expected.len(), budget)?;
        let end = self
            .expected
            .partition_point(|row| row.function.index() <= function.index());
        Ok(first..end)
    }

    /// Called before the genuine lowerer's binding map is destroyed or taken by
    /// test observation. No function is lowered again and no graph is copied.
    pub(in super::super) fn record_function(
        &mut self,
        plan: &LoweredFunctionPlanV1,
        lowering: &SemanticFunctionLoweringV1<'_>,
        blocks: &[BasicBlock],
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        budget.charge_work(4)?;
        let definitions = self.definitions.len();
        let edges = self.edges.len();
        // The current scoped-execution/expanded instance relation is a separate
        // contract. Do not manufacture an unqualified root-local alias here.
        let unsupported_placement = lowering.execution.is_some()
            || lowering.emission_placement != SemanticEmissionPlacementV1::default();
        if !unsupported_placement {
            let mut block_index = Vec::new();
            for (index, block) in blocks.iter().enumerate() {
                append(&mut block_index, (block.id, index), budget)?;
            }
            resources::sort_work(block_index.len(), budget)?;
            block_index.sort_unstable_by_key(|row| row.0);
            for expected in self.expected_range(plan.semantic_function, budget)? {
                budget.charge_work(1)?;
                let row = self.expected[expected];
                charge_lookup(lowering.semantic_ssa_bindings.len(), budget)?;
                let binding =
                    lowering
                        .semantic_ssa_bindings
                        .get(&row.value)
                        .ok_or(Error::Mismatch(
                            "supported source definition has no actual emission",
                        ))?;
                let values = binding_values(binding, row.shape)?;
                append(
                    &mut self.definitions,
                    EmittedDefinition {
                        expected,
                        values,
                        definitions: [None; 2],
                    },
                    budget,
                )?;
            }
            for (source_block, source) in lowering.function.blocks().iter().enumerate() {
                budget.charge_work(2)?;
                let source_block = u32::try_from(source_block).map_err(|_| Resource::Arithmetic)?;
                charge_lookup(lowering.control_flow_ssa.edge_arguments.len(), budget)?;
                let Some(arguments) = lowering
                    .control_flow_ssa
                    .edge_arguments
                    .get(&(source_block, 0))
                else {
                    continue;
                };
                let SemanticTerminatorKindV1::Goto(target) = source.terminator().kind() else {
                    continue;
                };
                charge_lookup(block_index.len(), budget)?;
                let actual_index = block_index
                    .binary_search_by_key(&BlockId(source_block), |row| row.0)
                    .map_err(|_| Error::Mismatch("emitted Goto block"))?;
                let actual = &blocks[block_index[actual_index].1];
                let Some(Terminator::Branch {
                    target: actual_target,
                    arguments: actual_arguments,
                }) = &actual.terminator
                else {
                    return Err(Error::Mismatch("actual Goto terminator"));
                };
                if *actual_target != BlockId(target.target().index()) {
                    return Err(Error::Mismatch("actual Goto destination"));
                }
                charge_lookup(lowering.block_parameters.len(), budget)?;
                let target_parameters = lowering
                    .block_parameters
                    .get(&target.target().index())
                    .ok_or(Error::Mismatch("actual Goto parameter table"))?;
                if target_parameters.len() != arguments.len() {
                    return Err(Error::Mismatch("source target transport arity"));
                }
                let mut slot = 0usize;
                for ((&variable, components), incoming) in target_parameters.iter().zip(arguments) {
                    budget.charge_work(2)?;
                    let next = slot
                        .checked_add(components.len())
                        .ok_or(Resource::Arithmetic)?;
                    let source_variable = SsaVariableIdV1::new(variable);
                    if incoming.variable() != source_variable {
                        return Err(Error::Mismatch("source target transport ordering"));
                    }
                    let local = lowering
                        .function
                        .locals()
                        .get(variable as usize)
                        .ok_or(Error::Mismatch("edge source local"))?;
                    if let Some(ty) = fixed_scalar(lowering.types, local.ty()) {
                        if components.len() != 1 || components[0].ty != Type::Scalar(ty) {
                            return Err(Error::Mismatch("scalar edge component coverage"));
                        }
                        charge_lookup(lowering.semantic_ssa_bindings.len(), budget)?;
                        let binding = lowering
                            .semantic_ssa_bindings
                            .get(&incoming.value())
                            .ok_or(Error::Mismatch("edge value was not emitted"))?;
                        let value = binding_values(binding, Shape::Scalar(ty))?[0];
                        if actual_arguments.get(slot) != Some(&value) {
                            return Err(Error::Mismatch(
                                "actual edge value differs from source SSA binding",
                            ));
                        }
                        append(
                            &mut self.edges,
                            EmittedEdge {
                                source: SsaEdgeIdV1::new(SsaBlockIdV1::new(source_block), 0),
                                target: target.target().index(),
                                variable: source_variable,
                                incoming: incoming.value(),
                                raw_source: actual.id,
                                raw_target: *actual_target,
                                argument: u32::try_from(slot).map_err(|_| Resource::Arithmetic)?,
                                value,
                                edge: None,
                            },
                            budget,
                        )?;
                    }
                    slot = next;
                }
                if slot != actual_arguments.len() {
                    return Err(Error::Mismatch("complete emitted Goto argument roster"));
                }
            }
            let bytes = table_bytes::<(BlockId, usize)>(block_index.capacity())?;
            drop(block_index);
            budget.release_storage(bytes)?;
        }
        let name = resources::copy_name(plan.kernel_ir_function.as_str(), budget)?;
        append(
            &mut self.functions,
            EmittedFunction {
                root: plan.correspondence_owner,
                source: plan.semantic_function,
                name,
                coordinate: None,
                unsupported_placement,
                definitions: definitions..self.definitions.len(),
                edges: edges..self.edges.len(),
            },
            budget,
        )?;
        Ok(())
    }
}

pub(super) fn source_shape(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    variable: SsaVariableIdV1,
    site: SourceSite,
) -> Result<Option<Shape>> {
    let local = function
        .locals()
        .get(variable.get() as usize)
        .ok_or(Error::Mismatch("source local"))?;
    if let Some(scalar) = fixed_scalar(types, local.ty()) {
        return Ok(Some(Shape::Scalar(scalar)));
    }
    let SourceSite::Event(Site::Statement { block, statement }) = site else {
        return Ok(None);
    };
    let source = function
        .blocks()
        .get(block.get() as usize)
        .and_then(|block| block.statements().get(statement as usize))
        .ok_or(Error::Mismatch("source definition site"))?;
    let SemanticStatementKindV1::Assign(assignment) = source.kind() else {
        return Ok(None);
    };
    let SemanticRvalueKindV1::CheckedBinary(binary) = assignment.value().kind() else {
        return Ok(None);
    };
    if binary.operation() != SemanticCheckedBinaryOpV1::Add
        || assignment.destination().local().index() != variable.get()
        || !assignment.destination().projections().is_empty()
    {
        return Ok(None);
    }
    let Some(scalar) = fixed_scalar(types, binary.left().ty()) else {
        return Ok(None);
    };
    let ty =
        checked_binary_result_type(types, binary.left().ty(), assignment.value().result_type())
            .map_err(|_| Error::Mismatch("checked Add source tuple"))?;
    if ty != Type::Scalar(scalar) || binary.right().ty() != binary.left().ty() {
        return Err(Error::Mismatch("checked Add source scalar"));
    }
    Ok(Some(Shape::CheckedAdd(scalar)))
}

fn binding_values(binding: &SemanticValueBindingV1, shape: Shape) -> Result<[ValueId; 2]> {
    let scalar = |binding: &SemanticValueBindingV1, expected| match binding {
        SemanticValueBindingV1::Value { id, ty } if *ty == Type::Scalar(expected) => Ok(*id),
        _ => Err(Error::Mismatch("actual scalar binding shape")),
    };
    match shape {
        Shape::Scalar(ty) => Ok([scalar(binding, ty)?, ValueId(0)]),
        Shape::CheckedAdd(ty) => {
            let SemanticValueBindingV1::Aggregate(parts) = binding else {
                return Err(Error::Mismatch("actual checked Add aggregate binding"));
            };
            if parts.len() != 2 {
                return Err(Error::Mismatch("actual checked Add field count"));
            }
            Ok([scalar(&parts[0], ty)?, scalar(&parts[1], ScalarType::Bool)?])
        }
    }
}
