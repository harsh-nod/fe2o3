// Initialization follows successful original producers, never a Plain node's shape.
enum SourceReferenceConstructionV29<'a, 'source> {
    Intrinsic(&'a production_call_instances_v1::ProductionInstanceCallV1<'source>),
    CheckedBinary(&'a fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1),
}

fn source_reference_construction_headers_v29() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<SourceReferenceConstructionV29<'_, '_>>(),
        std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
        std::mem::size_of::<(&SemanticPlaceV1, usize)>(),
    ])
}

impl SourceReferenceBuilderV29<'_, '_, '_> {
    fn assign_constructed(
        &mut self,
        site: SourceReferenceSiteV29,
        construction: SourceReferenceConstructionV29<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let result = (|| {
            self.plan.check_owner(self.plan.instances, budget)?;
            budget.reserve_storage(source_reference_construction_headers_v29()?)?;
            self.plan.charge(8, budget)?;
            if self.effect_site != Some(site) {
                return Err(source_reference_construction_error_v29());
            }
            let instances = self.plan.instances;
            let declaration = instances
                .instance(site.instance)
                .ok_or_else(source_reference_construction_error_v29)?
                .declaration();
            let block = declaration
                .blocks()
                .get(site.block.index() as usize)
                .ok_or_else(source_reference_construction_error_v29)?;
            let (place, node) = match construction {
                SourceReferenceConstructionV29::Intrinsic(exact) => {
                    let calls = instances
                        .calls(site.instance)
                        .ok_or_else(source_reference_construction_error_v29)?;
                    self.plan.charge(calls.len(), budget)?;
                    let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                        return Err(source_reference_construction_error_v29());
                    };
                    if site.statement.is_some()
                        || exact.occurrence().caller != site.instance
                        || exact.occurrence().block != site.block
                        || exact.child().is_some()
                        || !calls.iter().any(|call| std::ptr::eq(call, exact))
                        || !std::ptr::eq(exact.source(), call)
                        || !instances
                            .owner()
                            .source_semantic()
                            .callables()
                            .get(call.callee().index() as usize)
                            .is_some_and(|callable| std::ptr::eq(callable, exact.callable()))
                    {
                        return Err(source_reference_construction_error_v29());
                    }
                    let destination = call
                        .destination()
                        .ok_or_else(source_reference_construction_error_v29)?;
                    let SemanticCallableDeclV1::CompilerIntrinsic {
                        operation, binding, ..
                    } = exact.callable()
                    else {
                        return Err(source_reference_construction_error_v29());
                    };
                    let output = binding.abi().source_output_type();
                    if matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
                        || matches!(operation, SemanticCompilerIntrinsicOperationV1::Trap)
                        || destination.place().ty() != output
                        || instances.owner().source_semantic().types()[output.index() as usize]
                            .layout()
                            .is_uninhabited()
                        || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
                        || declaration
                            .blocks()
                            .get(destination.edge().target().index() as usize)
                            .is_none()
                    {
                        return Err(source_reference_construction_error_v29());
                    }
                    let mut arguments =
                        source_reference_scratch_v29(call.arguments().len(), budget)?;
                    for (ordinal, operand) in call.arguments().iter().enumerate() {
                        let node = self.operand(site, operand, budget)?;
                        self.retain_boundary_value(
                            site,
                            SourceReferenceBoundaryRoleV29::Argument(
                                u32::try_from(ordinal)
                                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                            ),
                            node,
                            budget,
                        )?;
                        arguments.push(node);
                    }
                    let node = self.intrinsic(exact.callable(), &arguments, call, budget)?;
                    (destination.place(), node)
                }
                SourceReferenceConstructionV29::CheckedBinary(assignment) => {
                    let statement = site
                        .statement
                        .and_then(|index| block.statements().get(index))
                        .ok_or_else(source_reference_construction_error_v29)?;
                    let SemanticStatementKindV1::Assign(original) = statement.kind() else {
                        return Err(source_reference_construction_error_v29());
                    };
                    if !std::ptr::eq(original, assignment)
                        || !matches!(
                            assignment.value().kind(),
                            SemanticRvalueKindV1::CheckedBinary(_)
                        )
                    {
                        return Err(source_reference_construction_error_v29());
                    }
                    let node = self.rvalue(site, assignment.value(), budget)?;
                    (assignment.destination(), node)
                }
            };
            if self.plan.nodes[node].ty != place.ty() {
                return Err(source_reference_construction_error_v29());
            }
            let target = self.write_place(site, place, budget)?;
            // A normal return/checked result constructs a valid value. This does not
            // establish nominal identity, pointer provenance, or an enum selector.
            self.mutate_storage_place(
                &target,
                source_storage_v29::SourceStorageRootMutationV29::Initialize,
                budget,
            )?;
            self.install_assigned_node(&target, node, budget)?;
            self.invalidate_discriminant_values(
                Some(target.instance),
                Some(target.local),
                None,
                budget,
            )?;
            Ok(())
        })();
        if let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) = &result {
            self.plan.failure.record_resource(*error);
        }
        result
    }
}

fn source_reference_construction_error_v29() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29("source construction differs from its original successful producer")
}
