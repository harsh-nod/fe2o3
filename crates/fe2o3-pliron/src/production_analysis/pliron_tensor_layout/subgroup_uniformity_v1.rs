include!("phi_selection_uniformity_v1.rs");
include!("forwarded_phi_equality_v1.rs");

fn analyze_pliron_subgroup_uniformity(
    context: &Context,
    function: &FuncOp,
    inventory: &BoundedPlironFunctionInventoryV1,
    layout: PlironExecutionLayoutV1,
    sparse: &SparseIndexAnalysisV1,
) -> Result<PlironSubgroupUniformityV1, PlironTensorLayoutFindingV1> {
    let entry = function.get_entry_block(context);
    let mut definitions = HashMap::<Value, SubgroupValueDefinitionV1>::new();
    let mut definition_order = Vec::new();
    let mut dependents = HashMap::<Value, Vec<Value>>::new();
    let mut block_arguments = HashMap::<Ptr<BasicBlock>, Vec<Value>>::new();
    let mut collection_work = 0_usize;
    charge_uniformity_collection(&mut collection_work, inventory.blocks().len())?;
    let phi_count = inventory
        .blocks()
        .iter()
        .filter(|block| **block != entry)
        .try_fold(0_usize, |count, block| {
            count.checked_add(block.deref(context).get_num_arguments())
        })
        .ok_or(PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    ensure_uniformity_value_capacity(0, phi_count)?;
    let mut phis = uniformity_vec_v1(phi_count)?;
    for (block_index, block) in inventory.blocks().iter().copied().enumerate() {
        let argument_count = block.deref(context).get_num_arguments();
        charge_uniformity_collection(&mut collection_work, argument_count)?;
        ensure_uniformity_value_capacity(definitions.len(), argument_count)?;
        let arguments = block.deref(context).arguments().collect::<Vec<_>>();
        if block != entry {
            charge_uniformity_collection(&mut collection_work, argument_count)?;
            phis.extend(arguments.iter().map(|argument| (block_index, *argument)));
        }
        for argument in &arguments {
            definition_order.push(*argument);
            definitions.insert(
                *argument,
                if block == entry {
                    SubgroupValueDefinitionV1::Fixed(SubgroupValueUniformityV1::Uniform)
                } else {
                    SubgroupValueDefinitionV1::Merge(Vec::new())
                },
            );
        }
        block_arguments.insert(block, arguments);
        for site in inventory.block_operations(block_index) {
            let operation = site.pointer();
            let dynamic = Operation::get_op_dyn(operation, context);
            let raw = operation.deref(context);
            let result_count = raw.get_num_results();
            charge_uniformity_collection(
                &mut collection_work,
                raw.get_num_operands().saturating_add(result_count),
            )?;
            ensure_uniformity_value_capacity(definitions.len(), result_count)?;
            for result_index in 0..result_count {
                let result = raw.get_result(result_index);
                let definition = if dynamic.downcast_ref::<IndexConstantOp>().is_some() {
                    SubgroupValueDefinitionV1::Fixed(SubgroupValueUniformityV1::Uniform)
                } else if let Some(invocation) = dynamic.downcast_ref::<InvocationIndexOp>() {
                    let uniformity = match invocation
                        .dimension(context)
                        .and_then(|dimension| usize::try_from(dimension).ok())
                    {
                        Some(dimension)
                            if invocation_axis_is_subgroup_uniform(dimension, layout) =>
                        {
                            SubgroupValueUniformityV1::Uniform
                        }
                        Some(_) => SubgroupValueUniformityV1::Varying,
                        None => SubgroupValueUniformityV1::Unknown,
                    };
                    SubgroupValueDefinitionV1::Fixed(uniformity)
                } else if let Some(binary) = dynamic.downcast_ref::<IndexBinaryOp>()
                    && raw.get_num_operands() == 2
                {
                    if invocation_quotient_is_subgroup_uniform(context, binary, layout) {
                        SubgroupValueDefinitionV1::Fixed(SubgroupValueUniformityV1::Uniform)
                    } else {
                        SubgroupValueDefinitionV1::Merge(vec![
                            binary.lhs(context),
                            binary.rhs(context),
                        ])
                    }
                } else if let Some(cast) = dynamic.downcast_ref::<IndexUnsignedCastOp>() {
                    SubgroupValueDefinitionV1::Merge(vec![cast.source(context)])
                } else if let Some(join) = dynamic.downcast_ref::<DeterministicJoinOp>() {
                    let dependencies = join.dependencies(context);
                    if dependencies.is_empty() {
                        return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                            detail: "deterministic join has no explicit dependencies".to_owned(),
                        });
                    }
                    if dependencies.len() > MAX_DETERMINISTIC_JOIN_INPUTS_V1 {
                        return Err(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
                    }
                    SubgroupValueDefinitionV1::Merge(dependencies)
                } else {
                    SubgroupValueDefinitionV1::Fixed(SubgroupValueUniformityV1::Unknown)
                };
                definition_order.push(result);
                definitions.insert(result, definition);
            }
        }
    }
    for block in inventory.blocks() {
        let Some(terminator) = block.deref(context).get_terminator(context) else {
            continue;
        };
        let dynamic = Operation::get_op_dyn(terminator, context);
        let raw = terminator.deref(context);
        if dynamic
            .downcast_ref::<IndexLessThanBranchArgsOp>()
            .is_some()
            || dynamic.downcast_ref::<IndexEqualBranchArgsOp>().is_some()
            || dynamic
                .downcast_ref::<dialect_gpu::optimization_v1::CondBranchOp>()
                .is_some()
        {
            if raw.get_num_successors() != 2 {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "typed conditional edge has a malformed successor count".to_owned(),
                });
            }
            let controls = if dynamic
                .downcast_ref::<dialect_gpu::optimization_v1::CondBranchOp>()
                .is_some()
            {
                1
            } else {
                2
            };
            let expected_operands = controls
                + raw.get_successor(0).deref(context).get_num_arguments()
                + raw.get_successor(1).deref(context).get_num_arguments();
            if raw.get_num_operands() != expected_operands {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "typed conditional edge has a malformed operand count".to_owned(),
                });
            }
        }
        if let Some(split) = dynamic.downcast_ref::<AnalysisSplitOp>() {
            if raw.get_num_successors() != 2 {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "analysis split has a malformed successor count".to_owned(),
                });
            }
            let expected_operands = split.control_dependencies(context).len()
                + raw.get_successor(0).deref(context).get_num_arguments()
                + raw.get_successor(1).deref(context).get_num_arguments();
            if raw.get_num_operands() != expected_operands {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "analysis split has a malformed operand count".to_owned(),
                });
            }
        }
        for (successor_index, successor) in raw.successors().enumerate() {
            let Some(arguments) = block_arguments.get(&successor) else {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "a branch targets a block outside the kernel".to_owned(),
                });
            };
            if arguments.is_empty() {
                continue;
            }
            charge_uniformity_collection(&mut collection_work, arguments.len())?;
            let incoming = if let Some(branch) = dynamic.downcast_ref::<BranchArgsOp>() {
                branch.arguments(context)
            } else if let Some(branch) = dynamic.downcast_ref::<IndexLessThanBranchArgsOp>() {
                if successor_index == 0 {
                    branch.true_arguments(context)
                } else {
                    branch.false_arguments(context)
                }
            } else if let Some(branch) = dynamic.downcast_ref::<IndexEqualBranchArgsOp>() {
                if successor_index == 0 {
                    branch.true_arguments(context)
                } else {
                    branch.false_arguments(context)
                }
            } else if dynamic
                .downcast_ref::<dialect_gpu::optimization_v1::CondBranchOp>()
                .is_some()
            {
                let start = if successor_index == 0 {
                    1
                } else {
                    1 + raw.get_successor(0).deref(context).get_num_arguments()
                };
                (start..start + arguments.len())
                    .map(|ordinal| raw.get_operand(ordinal))
                    .collect()
            } else if let Some(split) = dynamic.downcast_ref::<AnalysisSplitOp>() {
                if successor_index == 0 {
                    split.first_arguments(context)
                } else {
                    split.second_arguments(context)
                }
            } else {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "a block argument has a predecessor without typed edge operands"
                        .to_owned(),
                });
            };
            if incoming.len() != arguments.len() {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "typed edge operand and block argument counts differ".to_owned(),
                });
            }
            for (argument, incoming) in arguments.iter().zip(incoming) {
                let Some(SubgroupValueDefinitionV1::Merge(values)) = definitions.get_mut(argument)
                else {
                    return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                        detail: "an entry argument cannot receive a CFG edge operand".to_owned(),
                    });
                };
                values.push(incoming);
            }
        }
    }

    for value in &definition_order {
        let Some(definition) = definitions.get(value) else {
            return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                detail: "ordered subgroup-uniformity definition disappeared".to_owned(),
            });
        };
        if let SubgroupValueDefinitionV1::Merge(inputs) = definition {
            charge_uniformity_collection(&mut collection_work, inputs.len())?;
            for input in inputs {
                dependents.entry(*input).or_default().push(*value);
            }
        }
    }
    // Exact typed forwarding can preserve one value through distinct block
    // arguments. Its temporary index is dropped before controller construction.
    {
        let mut forwarding =
            SubgroupForwardingIndexV1::new(entry, phis.len(), &mut collection_work)?;
        for (_, value) in &phis {
            charge_uniformity_collection(&mut collection_work, 1)?;
            let Some(SubgroupValueDefinitionV1::Merge(inputs)) = definitions.get(value) else {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "phi selection lost its incoming definition".to_owned(),
                });
            };
            forwarding.push(*value, inputs, &mut collection_work)?;
        }
        let forwarding = forwarding.finish(context, &mut collection_work)?;
        let mut selected_phis = 0;
        for index in 0..phis.len() {
            charge_uniformity_collection(&mut collection_work, 1)?;
            if !forwarding.inputs_equal(index, &mut collection_work)? {
                charge_uniformity_collection(&mut collection_work, 1)?;
                phis[selected_phis] = phis[index];
                selected_phis += 1;
            }
        }
        phis.truncate(selected_phis);
    }
    let selection = if phis.is_empty() {
        None
    } else {
        Some(build_subgroup_phi_selection_v1(
            context,
            entry,
            inventory,
            &mut collection_work,
        )?)
    };
    charge_uniformity_collection(&mut collection_work, definition_order.len())?;
    let facts = definition_order
        .iter()
        .copied()
        .map(|value| (value, SubgroupValueUniformityV1::Uniform))
        .collect::<HashMap<_, _>>();
    let mut worklist = definition_order.into_iter().collect::<VecDeque<_>>();
    let mut uniformity = PlironSubgroupUniformityV1 { facts };
    let mut selector_facts =
        uniformity_vec_v1(selection.as_ref().map_or(0, |s| s.selectors.len()))?;
    let mut work_units = 0_usize;
    loop {
        while let Some(value) = worklist.pop_front() {
            charge_uniformity_collection(&mut work_units, 1)?;
            let Some(definition) = definitions.get(&value) else {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "queued subgroup-uniformity definition disappeared".to_owned(),
                });
            };
            let next = match definition {
                SubgroupValueDefinitionV1::Fixed(uniformity) => *uniformity,
                SubgroupValueDefinitionV1::Merge(inputs) => {
                    charge_uniformity_collection(&mut work_units, inputs.len())?;
                    SubgroupValueUniformityV1::merge(inputs.iter().map(|input| {
                        uniformity
                            .facts
                            .get(input)
                            .copied()
                            .unwrap_or(SubgroupValueUniformityV1::Unknown)
                    }))
                }
            };
            let Some(current) = uniformity.facts.get_mut(&value) else {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "subgroup-uniformity definition has no lattice fact".to_owned(),
                });
            };
            if next.rank() <= current.rank() {
                continue;
            }
            *current = next;
            charge_uniformity_collection(
                &mut work_units,
                dependents.get(&value).map_or(0, Vec::len),
            )?;
            worklist.extend(dependents.get(&value).into_iter().flatten().copied());
        }
        let Some(selection) = selection.as_ref() else {
            break;
        };
        selector_facts.clear();
        for (controller, selector) in selection.selectors.iter().enumerate() {
            charge_uniformity_collection(&mut work_units, 1)?;
            if !selection.reachable[controller] {
                selector_facts.push(SubgroupValueUniformityV1::Uniform);
                continue;
            }
            selector_facts.push(classify_subgroup_selector_v1(
                context,
                entry,
                layout,
                sparse,
                &uniformity,
                *selector,
                &mut work_units,
            )?);
        }
        let mut changed = false;
        for (block, value) in &phis {
            charge_uniformity_collection(&mut work_units, 1)?;
            let Some(SubgroupValueDefinitionV1::Merge(inputs)) = definitions.get(value) else {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "phi selection lost its exact incoming value roster".to_owned(),
                });
            };
            let selected = subgroup_phi_selection_fact_v1(
                selection,
                &selector_facts,
                *block,
                inputs,
                &mut work_units,
            )?;
            let Some(current) = uniformity.facts.get_mut(value) else {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "phi selection lost its value lattice fact".to_owned(),
                });
            };
            if selected.rank() > current.rank() {
                charge_uniformity_collection(
                    &mut work_units,
                    dependents.get(value).map_or(0, Vec::len),
                )?;
                *current = selected;
                worklist.extend(dependents.get(value).into_iter().flatten().copied());
                changed = true;
            }
        }
        // Facts only worsen twice (Uniform -> Unknown -> Varying). A sweep
        // without a worsening phi cannot create another worklist entry.
        if !changed {
            break;
        }
    }
    Ok(uniformity)
}

fn charge_uniformity_collection(
    work: &mut usize,
    amount: usize,
) -> Result<(), PlironTensorLayoutFindingV1> {
    *work = work
        .checked_add(amount)
        .ok_or(PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    if *work > MAX_PLIRON_TENSOR_UNIFORMITY_WORK_UNITS_V1 {
        return Err(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
    }
    Ok(())
}

fn ensure_uniformity_value_capacity(
    current: usize,
    additional: usize,
) -> Result<(), PlironTensorLayoutFindingV1> {
    if current
        .checked_add(additional)
        .is_none_or(|count| count > MAX_PLIRON_TENSOR_UNIFORMITY_VALUES_V1)
    {
        return Err(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
    }
    Ok(())
}

fn invocation_quotient_is_subgroup_uniform(
    context: &Context,
    binary: &IndexBinaryOp,
    layout: PlironExecutionLayoutV1,
) -> bool {
    if binary.kind(context) != Some(IndexBinaryKindAttr::Divide) || layout.subgroup_size == 0 {
        return false;
    }
    let Some(divisor) = binary.rhs(context).defining_op().and_then(|operation| {
        Operation::get_op_dyn(operation, context)
            .downcast_ref::<IndexConstantOp>()
            .and_then(|constant| constant.value(context))
    }) else {
        return false;
    };
    if divisor == 0 || !divisor.is_multiple_of(layout.subgroup_size) {
        return false;
    }
    let Some(invocation) = binary.lhs(context).defining_op().and_then(|operation| {
        Operation::get_op_dyn(operation, context)
            .downcast_ref::<InvocationIndexOp>()
            .cloned()
    }) else {
        return false;
    };
    if invocation.dimension(context) != Some(0) {
        return false;
    }
    let workgroup_extent = layout.workgroup_extents[0];
    workgroup_extent >= layout.subgroup_size
        && workgroup_extent.is_multiple_of(layout.subgroup_size)
}

fn value_is_entry_argument(value: Value, entry: Ptr<BasicBlock>) -> bool {
    value.defining_block() == Some(entry)
}

fn sparse_fact_is_subgroup_uniform(
    fact: &SparseIndexFactV1,
    layout: PlironExecutionLayoutV1,
) -> bool {
    match fact {
        SparseIndexFactV1::Affine(affine) => {
            affine
                .coefficients()
                .iter()
                .enumerate()
                .all(|(dimension, coefficient)| {
                    *coefficient == 0 || invocation_axis_is_subgroup_uniform(dimension, layout)
                })
        }
        SparseIndexFactV1::Remainder { dividend, modulus } => {
            *modulus == 1
                || dividend
                    .coefficients()
                    .iter()
                    .enumerate()
                    .all(|(dimension, coefficient)| {
                        *coefficient == 0 || invocation_axis_is_subgroup_uniform(dimension, layout)
                    })
        }
        SparseIndexFactV1::Unknown
        | SparseIndexFactV1::MachineOverflow(_)
        | SparseIndexFactV1::CheckedTiled2D(_)
        | SparseIndexFactV1::CheckedRowStriped2D(_) => false,
    }
}

fn invocation_axis_is_subgroup_uniform(dimension: usize, layout: PlironExecutionLayoutV1) -> bool {
    let Some(&extent) = layout.workgroup_extents.get(dimension) else {
        return false;
    };
    if extent == 1 {
        return true;
    }
    let Some(stride) = layout.workgroup_extents[..dimension]
        .iter()
        .try_fold(1_u64, |stride, extent| stride.checked_mul(*extent))
    else {
        return false;
    };
    stride >= layout.subgroup_size && stride.is_multiple_of(layout.subgroup_size)
}

fn classify_coordinate_cutoff(
    coordinate: &SparseIndexFactV1,
    cutoff: &SparseIndexFactV1,
    layout: PlironExecutionLayoutV1,
    coordinate_is_lhs: bool,
) -> Option<SubgroupBranchUniformityV1> {
    let affine = coordinate.affine()?;
    if affine.constant_term() != 0 {
        return None;
    }
    let mut dimensions = affine
        .coefficients()
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, coefficient)| *coefficient != 0);
    let (dimension, coefficient) = dimensions.next()?;
    if coefficient != 1 || dimensions.next().is_some() {
        return None;
    }
    let cutoff = cutoff.constant_value()?;
    let global_extent = layout.global_extents.get(dimension).copied()?;
    if (coordinate_is_lhs && cutoff == 0)
        || (!coordinate_is_lhs
            && (cutoff == u64::MAX
                || (global_extent != 0 && cutoff >= global_extent.saturating_sub(1))))
        || (coordinate_is_lhs && global_extent != 0 && cutoff >= global_extent)
        || invocation_axis_is_subgroup_uniform(dimension, layout)
    {
        return Some(SubgroupBranchUniformityV1::Uniform);
    }
    let boundary = if coordinate_is_lhs {
        cutoff
    } else {
        cutoff.checked_add(1)?
    };
    let extent = *layout.workgroup_extents.get(dimension)?;
    let stride = layout.workgroup_extents[..dimension]
        .iter()
        .try_fold(1_u64, |stride, extent| stride.checked_mul(*extent))?;
    let period = extent.checked_mul(stride)?;
    let transition = (boundary % extent).checked_mul(stride)?;
    if period.is_multiple_of(layout.subgroup_size) {
        return Some(if transition.is_multiple_of(layout.subgroup_size) {
            SubgroupBranchUniformityV1::Uniform
        } else {
            SubgroupBranchUniformityV1::Varying
        });
    }
    None
}
