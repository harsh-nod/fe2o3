// This source-only graph preserves selections, not a union of issuer authority.
// Emitted pointer edges and final conditional access obligations must be replayed
// separately before a selected reference can authorize a memory access.
include!("production_source_reference_selection_actual_v30.rs");
include!("production_source_reference_selection_memory_v30.rs");
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceSelectionValueV29 {
    instance: ProductionCallInstanceIdV1,
    value: SsaValueV1,
    pointer_type: SemanticTypeIdV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceReferenceSelectionStepV29 {
    Pending,
    Leaf(SourceExternalReferenceOriginV29),
    Alias {
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        source: usize,
        reborrow: bool,
        input: usize,
    },
    CallArgument {
        call: production_call_instances_v1::ProductionCallOccurrenceV1,
        local: SemanticLocalIdV1,
        source_argument: u32,
        source: usize,
        input: usize,
    },
    Parameter {
        block: SsaBlockIdV1,
        variable: fe2o3_mir_model::SsaVariableIdV1,
        ordinal: usize,
        invocation: Option<usize>,
        first: usize,
        count: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceSelectionNodeV29 {
    value: SourceReferenceSelectionValueV29,
    step: SourceReferenceSelectionStepV29,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceSelectionEdgeV29 {
    edge: SsaEdgeIdV1,
    role: SemanticEdgeRoleV1,
    input: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReferenceSelectionSubjectV29 {
    instance: ProductionCallInstanceIdV1,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
    source: usize,
}

struct SourceReferenceSelectionGraphV29 {
    subject: SourceReferenceSelectionSubjectV29,
    nodes: Vec<SourceReferenceSelectionNodeV29>,
    edges: Vec<SourceReferenceSelectionEdgeV29>,
}

struct SourceReferenceSelectionBuilderV29<'scope, 'owner, 'source> {
    plan: &'scope SourceReferencePlanV29<'owner, 'source>,
    graph: SourceReferenceSelectionGraphV29,
    indices: BTreeMap<(usize, SsaValueV1), usize>,
    originals: BTreeMap<usize, SourceIssuedSemanticV29<'scope, 'source, 'scope>>,
    node_bound: usize,
}

fn source_reference_selection_error_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29("reference selection differs from its original typed SSA edges")
}

fn source_reference_selection_headers_v29() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        source_reference_emission_headers_v29::<SourceReferenceSelectionBuilderV29<'_, '_, '_>>()?,
        source_reference_emission_headers_v29::<SourceReferenceSelectionGraphV29>()?,
        source_reference_emission_headers_v29::<SourceReferenceSelectionSubjectV29>()?,
        source_reference_emission_headers_v29::<SourceReferenceSelectionValueV29>()?,
        source_reference_emission_headers_v29::<SourceReferenceSelectionNodeV29>()?,
        source_reference_emission_headers_v29::<SourceReferenceSelectionStepV29>()?,
        source_reference_emission_headers_v29::<SourceReferenceSelectionEdgeV29>()?,
        source_reference_emission_headers_v29::<Option<SourceExternalReferenceOriginV29>>()?,
        source_reference_emission_headers_v29::<Option<SemanticLocalIdV1>>()?,
        source_reference_emission_headers_v29::<Option<usize>>()?,
        source_reference_emission_headers_v29::<&mut SourceIssuedSemanticV29<'_, '_, '_>>()?,
        source_reference_emission_headers_v29::<Vec<bool>>()?,
        source_reference_emission_headers_v29::<usize>()?,
        source_reference_emission_headers_v29::<bool>()?,
        source_reference_emission_headers_v29::<()>()?,
    ])
}

fn source_reference_selection_call_headers_v29<R>(
    consume: &impl Sized,
) -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        argument_product_v1(2, std::mem::size_of_val(consume))?,
        argument_product_v1(2, std::mem::align_of_val(consume))?,
        source_reference_emission_headers_v29::<R>()?,
        std::mem::size_of::<std::thread::Result<Result<R, ProductionSemanticKirErrorV1>>>(),
        std::mem::size_of::<Box<dyn std::any::Any + Send>>(),
        std::mem::size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
        argument_product_v1(4, std::mem::size_of::<usize>())?,
        argument_product_v1(2, std::mem::size_of::<&SourceReferencePlanV29<'_, '_>>())?,
        argument_product_v1(2, std::mem::size_of::<&SemanticPlaceV1>())?,
        argument_product_v1(2, std::mem::size_of::<&mut ArgumentBudgetV1<'_>>())?,
    ])
}

impl<'scope, 'owner, 'source> SourceReferenceSelectionBuilderV29<'scope, 'owner, 'source> {
    fn build(
        plan: &'scope SourceReferencePlanV29<'owner, 'source>,
        instance: ProductionCallInstanceIdV1,
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        source: &SemanticPlaceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        plan.check_owner(plan.instances, budget)?;
        budget.reserve_storage(source_reference_selection_headers_v29()?)?;
        let mut node_bound = 0;
        for instance in plan.instances.instances() {
            budget.charge_work(3)?;
            let function = instance.declaration();
            node_bound = argument_sum_v1(&[
                node_bound,
                instance.ssa().plan().definition_count(),
                argument_product_v1(function.locals().len(), function.blocks().len())?,
            ])?;
        }
        let mut builder = Self {
            plan,
            graph: SourceReferenceSelectionGraphV29 {
                subject: SourceReferenceSelectionSubjectV29 {
                    instance,
                    site,
                    role,
                    source: source as *const SemanticPlaceV1 as usize,
                },
                nodes: Vec::new(),
                edges: Vec::new(),
            },
            indices: BTreeMap::new(),
            originals: BTreeMap::new(),
            node_bound,
        };
        let value = builder.use_value(instance, site, role, source, budget)?;
        if builder.intern(value, budget)? != 0 {
            return Err(source_reference_selection_error_v29());
        }
        let mut next = 0;
        while next < builder.graph.nodes.len() {
            budget.charge_work(2)?;
            let value = builder.graph.nodes[next].value;
            let step = builder.expand(value, budget)?;
            builder.graph.nodes[next].step = step;
            next = argument_sum_v1(&[next, 1])?;
        }
        builder.check_seeded(budget)?;
        Ok(builder)
    }

    fn original(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&mut SourceIssuedSemanticV29<'scope, 'source, 'scope>, ProductionSemanticKirErrorV1>
    {
        charge_execution_cfg_lookup_v29(self.originals.len(), budget)?;
        if !self.originals.contains_key(&instance.index()) {
            let original = SourceIssuedSemanticV29::new(
                self.plan.instances,
                instance,
                SourceIssuedReplayModeV29::SourceOnly,
                budget,
            )?;
            reserve_execution_cfg_map_entry_v29::<usize, SourceIssuedSemanticV29<'_, '_, '_>>(
                self.originals.len(),
                budget,
            )?;
            self.originals.insert(instance.index(), original);
        }
        charge_execution_cfg_lookup_v29(self.originals.len(), budget)?;
        self.originals
            .get_mut(&instance.index())
            .ok_or_else(source_reference_selection_error_v29)
    }

    fn intern(
        &mut self,
        value: SourceReferenceSelectionValueV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        let source = self.plan.instances.owner().source_semantic();
        if !matches!(source.types().get(value.pointer_type.index() as usize).map(SemanticTypeDeclV1::shape),
            Some(SemanticTypeShapeV1::Pointer(pointer))
                if pointer.kind() == SemanticPointerKindV1::Reference
                    && pointer.metadata() == SemanticPointerMetadataV1::None)
        {
            return Err(source_reference_selection_error_v29());
        }
        let key = (value.instance.index(), value.value);
        charge_execution_cfg_lookup_v29(self.indices.len(), budget)?;
        if let Some(&index) = self.indices.get(&key) {
            if self.graph.nodes[index].value != value {
                return Err(source_reference_selection_error_v29());
            }
            return Ok(index);
        }
        let index = self.graph.nodes.len();
        if index >= self.node_bound {
            return Err(source_reference_selection_error_v29());
        }
        reserve_execution_cfg_map_entry_v29::<(usize, SsaValueV1), usize>(
            self.indices.len(),
            budget,
        )?;
        emission_push_v1(
            &mut self.graph.nodes,
            SourceReferenceSelectionNodeV29 {
                value,
                step: SourceReferenceSelectionStepV29::Pending,
            },
            budget,
        )?;
        self.indices.insert(key, index);
        Ok(index)
    }

    fn use_value(
        &mut self,
        instance: ProductionCallInstanceIdV1,
        site: ExecutionSiteV29,
        role: ExecutionOperandV29,
        place: &SemanticPlaceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceSelectionValueV29, ProductionSemanticKirErrorV1> {
        let pointer_type = self
            .plan
            .instances
            .instance(instance)
            .and_then(|row| {
                row.declaration()
                    .locals()
                    .get(place.local().index() as usize)
            })
            .ok_or_else(source_reference_selection_error_v29)?
            .ty();
        let value = self
            .original(instance, budget)?
            .use_value(site, role, place, budget)?;
        Ok(SourceReferenceSelectionValueV29 {
            instance,
            value,
            pointer_type,
        })
    }

    fn parameter(
        &mut self,
        value: SourceReferenceSelectionValueV29,
        block: SsaBlockIdV1,
        variable: fe2o3_mir_model::SsaVariableIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceSelectionStepV29, ProductionSemanticKirErrorV1> {
        let instances = self.plan.instances;
        let instance = instances
            .instance(value.instance)
            .ok_or_else(source_reference_selection_error_v29)?;
        let ssa = instance.ssa().plan();
        let variables = ssa
            .transport_variables(block)
            .ok_or_else(source_reference_selection_error_v29)?;
        charge_execution_cfg_lookup_v29(variables.len(), budget)?;
        let ordinal = variables
            .binary_search(&variable)
            .map_err(|_| source_reference_selection_error_v29())?;
        if instance
            .declaration()
            .locals()
            .get(variable.get() as usize)
            .is_none_or(|local| local.ty() != value.pointer_type)
        {
            return Err(source_reference_selection_error_v29());
        }
        let occurrences = instances
            .occurrences(value.instance)
            .ok_or_else(source_reference_selection_error_v29)?;
        let invocation = if block.get() == instance.declaration().entry().index() {
            let arguments = ssa.entry_arguments();
            budget.charge_work(arguments.len())?;
            let argument = arguments
                .get(ordinal)
                .ok_or_else(source_reference_selection_error_v29)?;
            if arguments.len() != variables.len() || argument.variable() != variable {
                return Err(source_reference_selection_error_v29());
            }
            Some(self.intern(
                SourceReferenceSelectionValueV29 {
                    instance: value.instance,
                    value: argument.value(),
                    pointer_type: value.pointer_type,
                },
                budget,
            )?)
        } else {
            None
        };
        let first = self.graph.edges.len();
        // Occurrence order includes parallel successors. Only original dead
        // source blocks and checked no-normal-return continuations are omitted.
        for incoming in occurrences.successors() {
            budget.charge_work(6)?;
            let edge = incoming.id();
            if incoming.edge().target().index() != block.get()
                || instances.block_reachable(
                    value.instance,
                    SemanticBlockIdV1::from_index(edge.source().get()),
                ) != Some(true)
            {
                continue;
            }
            if incoming.edge().role() == SemanticEdgeRoleV1::CallReturn
                && !source_reference_call_returns_v29(
                    instances,
                    value.instance,
                    SemanticBlockIdV1::from_index(edge.source().get()),
                    budget,
                )?
            {
                continue;
            }
            let arguments = ssa
                .edge_arguments(edge)
                .ok_or_else(source_reference_selection_error_v29)?;
            let argument = arguments
                .get(ordinal)
                .ok_or_else(source_reference_selection_error_v29)?;
            if arguments.len() != variables.len() || argument.variable() != variable {
                return Err(source_reference_selection_error_v29());
            }
            let input = self.intern(
                SourceReferenceSelectionValueV29 {
                    instance: value.instance,
                    value: argument.value(),
                    pointer_type: value.pointer_type,
                },
                budget,
            )?;
            emission_push_v1(
                &mut self.graph.edges,
                SourceReferenceSelectionEdgeV29 {
                    edge,
                    role: incoming.edge().role(),
                    input,
                },
                budget,
            )?;
        }
        let count = self.graph.edges.len() - first;
        if count == 0 && invocation.is_none() {
            return Err(source_reference_selection_error_v29());
        }
        Ok(SourceReferenceSelectionStepV29::Parameter {
            block,
            variable,
            ordinal,
            invocation,
            first,
            count,
        })
    }

    fn expand(
        &mut self,
        value: SourceReferenceSelectionValueV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SourceReferenceSelectionStepV29, ProductionSemanticKirErrorV1> {
        let plan = self.plan;
        budget.charge_work(4)?;
        if let Some(recipe) = self
            .original(value.instance, budget)?
            .resolve(value.value, budget)?
        {
            if recipe.form != SourceIssuedFormV29::Pointer
                || recipe.pointer_type != value.pointer_type
            {
                return Err(source_reference_selection_error_v29());
            }
            return Ok(SourceReferenceSelectionStepV29::Leaf(
                SourceExternalReferenceOriginV29::Issued {
                    instance: value.instance,
                    recipe,
                },
            ));
        }
        let original = self.original(value.instance, budget)?;
        charge_execution_cfg_lookup_v29(original.definitions.len(), budget)?;
        let definition = original.definitions.get(&value.value).copied();
        let function = plan
            .instances
            .instance(value.instance)
            .ok_or_else(source_reference_selection_error_v29)?
            .declaration();
        let occurrences = plan
            .instances
            .occurrences(value.instance)
            .ok_or_else(source_reference_selection_error_v29)?;
        if let Some(SourceIssuedDefinitionV29::Statement(definition)) = definition {
            let (site, rvalue) = source_descriptor_assignment_v29(
                function,
                &occurrences,
                definition,
                value.value,
                budget,
            )?;
            let Some(SemanticStatementKindV1::Assign(assignment)) =
                scoped_source_statement_v29(function, site)
            else {
                return Err(source_reference_selection_error_v29());
            };
            if assignment.destination().ty() != value.pointer_type
                || assignment.value().result_type() != value.pointer_type
            {
                return Err(source_reference_selection_error_v29());
            }
            if let SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            } = rvalue
            {
                if let Some(origin) = source_external_descriptor_origin_v29(
                    plan,
                    value.instance,
                    site,
                    place,
                    budget,
                )? {
                    return Ok(SourceReferenceSelectionStepV29::Leaf(origin));
                }
            }
            let Some((place, role, reborrow)) = source_reference_pointer_alias_v29(
                plan.instances.owner().source_semantic().types(),
                function,
                site,
                rvalue,
                budget,
            )?
            else {
                return Err(source_reference_selection_error_v29());
            };
            if !reborrow && (!place.projections().is_empty() || place.ty() != value.pointer_type) {
                return Err(source_reference_selection_error_v29());
            }
            let dependency = self.use_value(value.instance, site, role, place, budget)?;
            if dependency.pointer_type != value.pointer_type {
                return Err(source_reference_selection_error_v29());
            }
            let input = self.intern(dependency, budget)?;
            return Ok(SourceReferenceSelectionStepV29::Alias {
                site,
                role,
                source: place as *const SemanticPlaceV1 as usize,
                reborrow,
                input,
            });
        }
        if definition.is_some() {
            return Err(source_reference_selection_error_v29());
        }
        if let Some(local) = self
            .original(value.instance, budget)?
            .entry_dependency_v26(value.value, budget)?
        {
            let incoming = plan
                .instances
                .incoming(value.instance)
                .ok_or_else(source_reference_selection_error_v29)?;
            let call = incoming.occurrence();
            if incoming.child() != Some(value.instance)
                || call.caller.index() >= value.instance.index()
            {
                return Err(source_reference_selection_error_v29());
            }
            let selector = plan
                .instances
                .parameter_source(value.instance, local, budget)
                .map_err(|error| match error {
                    production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(
                        error,
                    ) => error.into(),
                    _ => source_reference_selection_error_v29(),
                })?;
            let operand = incoming
                .source()
                .arguments()
                .get(selector.source_argument as usize)
                .ok_or_else(source_reference_selection_error_v29)?;
            let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
                return Err(source_reference_selection_error_v29());
            };
            if selector.tuple_field.is_some()
                || !std::ptr::eq(selector.operand, operand)
                || selector.ty != value.pointer_type
                || place.ty() != value.pointer_type
                || !place.projections().is_empty()
            {
                return Err(source_reference_selection_error_v29());
            }
            let dependency = self.use_value(
                call.caller,
                ExecutionSiteV29::Terminator {
                    block: SsaBlockIdV1::new(call.block.index()),
                },
                ExecutionOperandV29::CallArgument(selector.source_argument),
                place,
                budget,
            )?;
            if dependency.pointer_type != value.pointer_type {
                return Err(source_reference_selection_error_v29());
            }
            let input = self.intern(dependency, budget)?;
            return Ok(SourceReferenceSelectionStepV29::CallArgument {
                call,
                local,
                source_argument: selector.source_argument,
                source: place as *const SemanticPlaceV1 as usize,
                input,
            });
        }
        let SsaValueV1::BlockArgument { block, variable } = value.value else {
            return Err(source_reference_selection_error_v29());
        };
        self.parameter(value, block, variable, budget)
    }

    fn check_seeded(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let mut seeded = emission_vec_v1(self.graph.nodes.len(), budget)?;
        for node in &self.graph.nodes {
            budget.charge_work(1)?;
            seeded.push(matches!(
                node.step,
                SourceReferenceSelectionStepV29::Leaf(_)
            ));
        }
        // The graph is finite and the bitset only grows. Every node must reach
        // an authenticated leaf; an uninitialized, self-supporting SCC fails.
        for _ in 0..self.graph.nodes.len() {
            let mut changed = false;
            for (index, node) in self.graph.nodes.iter().enumerate() {
                budget.charge_work(4)?;
                if seeded[index] {
                    continue;
                }
                let has_seed = match node.step {
                    SourceReferenceSelectionStepV29::Pending => {
                        return Err(source_reference_selection_error_v29());
                    }
                    SourceReferenceSelectionStepV29::Leaf(_) => true,
                    SourceReferenceSelectionStepV29::Alias { input, .. }
                    | SourceReferenceSelectionStepV29::CallArgument { input, .. } => seeded[input],
                    SourceReferenceSelectionStepV29::Parameter {
                        first,
                        count,
                        invocation,
                        ..
                    } => {
                        let end = argument_sum_v1(&[first, count])?;
                        let inputs = self
                            .graph
                            .edges
                            .get(first..end)
                            .ok_or_else(source_reference_selection_error_v29)?;
                        budget.charge_work(inputs.len())?;
                        invocation.is_some_and(|input| seeded[input])
                            || inputs.iter().any(|edge| seeded[edge.input])
                    }
                };
                if has_seed {
                    seeded[index] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        budget.charge_work(seeded.len())?;
        if seeded.iter().any(|seeded| !*seeded) {
            return Err(source_reference_selection_error_v29());
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn with_source_reference_selection_v29<R>(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
    source: &SemanticPlaceV1,
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl FnOnce(
        &SourceReferenceSelectionGraphV29,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    plan.check_owner(plan.instances, budget)?;
    let result = with_canonical_call_scratch_v1(budget, |budget| {
        budget.reserve_storage(source_reference_selection_call_headers_v29::<R>(&consume)?)?;
        let builder =
            SourceReferenceSelectionBuilderV29::build(plan, instance, site, role, source, budget)?;
        let retained = budget.storage();
        let result = consume(&builder.graph, budget);
        if !plan.retains_custody(plan.instances, budget) || budget.storage() != retained {
            plan.failure.record_resource(ArgumentResourceV1::Accounting);
            return Err(ArgumentResourceV1::Accounting.into());
        }
        result
    });
    result.inspect_err(|error| source_reference_record_failure_v29(plan, error))
}

#[allow(clippy::too_many_arguments)]
fn check_source_reference_selection_rows_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    instance: ProductionCallInstanceIdV1,
    site: ExecutionSiteV29,
    role: ExecutionOperandV29,
    source: &SemanticPlaceV1,
    graph: &SourceReferenceSelectionGraphV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_source_reference_selection_v29(
        plan,
        instance,
        site,
        role,
        source,
        budget,
        |expected, budget| {
            budget.charge_work(argument_sum_v1(&[
                3,
                expected.nodes.len(),
                expected.edges.len(),
            ])?)?;
            if expected.subject != graph.subject
                || expected.nodes != graph.nodes
                || expected.edges != graph.edges
            {
                return Err(source_reference_selection_error_v29());
            }
            Ok(())
        },
    )
}
