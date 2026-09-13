#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SubgroupBranchUniformityV1 {
    Uniform,
    Varying,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SubgroupValueUniformityV1 {
    Uniform,
    Unknown,
    Varying,
}

impl SubgroupValueUniformityV1 {
    const fn rank(self) -> u8 {
        match self {
            Self::Uniform => 0,
            Self::Unknown => 1,
            Self::Varying => 2,
        }
    }

    fn merge(inputs: impl IntoIterator<Item = Self>) -> Self {
        inputs
            .into_iter()
            .max_by_key(|uniformity| uniformity.rank())
            .unwrap_or(Self::Unknown)
    }
}

enum SubgroupValueDefinitionV1 {
    Fixed(SubgroupValueUniformityV1),
    Merge(Vec<Value>),
}

struct PlironSubgroupUniformityV1 {
    facts: HashMap<Value, SubgroupValueUniformityV1>,
}

impl PlironSubgroupUniformityV1 {
    fn fact(&self, value: Value) -> SubgroupValueUniformityV1 {
        self.facts
            .get(&value)
            .copied()
            .unwrap_or(SubgroupValueUniformityV1::Unknown)
    }
}

struct SymbolicTensorCfgV1 {
    successors: Vec<Vec<usize>>,
    branch_uniformity: Vec<SubgroupBranchUniformityV1>,
    reachable: Vec<bool>,
}

struct TensorControlRegionV1 {
    blocks: Vec<u64>,
    has_cycle: bool,
}

impl TensorControlRegionV1 {
    fn contains(&self, block: usize) -> bool {
        self.blocks
            .get(block / u64::BITS as usize)
            .is_some_and(|word| word & (1_u64 << (block % u64::BITS as usize)) != 0)
    }
}

fn symbolic_subgroup_convergence(
    context: &Context,
    function: &FuncOp,
    inventory: &BoundedPlironFunctionInventoryV1,
    layout: PlironExecutionLayoutV1,
    analyses: &mut PlironAnalysisManagerV1,
    tensor_sites: &[(usize, usize)],
) -> Result<(), PlironTensorLayoutFindingV1> {
    let workgroup_size = layout
        .workgroup_extents
        .into_iter()
        .try_fold(1_u64, u64::checked_mul)
        .ok_or(PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    if !workgroup_size.is_multiple_of(layout.subgroup_size) {
        return Err(
            PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                detail: "symbolic tensor convergence cannot establish full subgroup participation for a partial subgroup"
                    .to_owned(),
            },
        );
    }
    let potentially_partial_workgroup = layout
        .global_extents
        .iter()
        .zip(layout.workgroup_extents)
        .any(|(global, workgroup)| *global == 0 || !global.is_multiple_of(workgroup));
    if potentially_partial_workgroup
        && layout.execution_domain != dialect_gpu::ExecutionDomainAttr::FullPhysicalWorkgroups
    {
        return Err(
            PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                detail: "symbolic tensor convergence cannot establish full subgroup participation for a partial workgroup"
                    .to_owned(),
            },
        );
    }
    analyses.prepare_sparse_indices(context, function);
    let sparse = analyses.sparse_indices().map_err(|failure| match failure {
        SparseIndexFailureV1::ResourceLimit { .. } => {
            PlironTensorLayoutFindingV1::ResourceLimitExceeded
        }
        failure => PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
            detail: format!("sparse predicate analysis failed: {failure:?}"),
        },
    })?;
    let uniformity =
        analyze_pliron_subgroup_uniformity(context, function, inventory, layout, sparse)?;
    let blocks = inventory.blocks();
    if blocks.is_empty() || blocks.len() > MAX_PLIRON_TENSOR_LAYOUT_OPERATIONS_V1 {
        return Err(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
    }
    let block_indices = blocks
        .iter()
        .copied()
        .enumerate()
        .map(|(index, block)| (block, index))
        .collect::<HashMap<_, _>>();
    let entry = function.get_entry_block(context);
    let mut successors = Vec::with_capacity(blocks.len());
    let mut branch_uniformity = Vec::with_capacity(blocks.len());
    for (block_index, block) in blocks.iter().copied().enumerate() {
        let terminator = block
            .deref(context)
            .get_terminator(context)
            .ok_or_else(
                || PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: format!("block {block_index} has no terminator"),
                },
            )?;
        let terminator = Operation::get_op_dyn(terminator, context);
        let raw = terminator.get_operation().deref(context);
        let (kind, expected_successors) = if terminator.downcast_ref::<ReturnOp>().is_some()
            || terminator.downcast_ref::<TrapOp>().is_some()
        {
            (SubgroupBranchUniformityV1::Uniform, 0)
        } else if terminator.downcast_ref::<BranchOp>().is_some()
            || terminator.downcast_ref::<BranchArgsOp>().is_some()
        {
            (SubgroupBranchUniformityV1::Uniform, 1)
        } else if let Some(branch) = terminator.downcast_ref::<IndexLessThanBranchOp>() {
            if raw.get_num_operands() != 2 {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: format!(
                        "block {block_index} index comparison has a malformed operand count"
                    ),
                });
            }
            (
                classify_subgroup_predicate(
                    entry,
                    layout,
                    sparse,
                    &uniformity,
                    branch.lhs(context),
                    branch.rhs(context),
                ),
                2,
            )
        } else if let Some(branch) = terminator.downcast_ref::<IndexLessThanBranchArgsOp>() {
            if raw.get_num_successors() != 2 {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: format!(
                        "block {block_index} typed index comparison has a malformed successor count"
                    ),
                });
            }
            let expected_operands = 2
                + raw.get_successor(0).deref(context).get_num_arguments()
                + raw.get_successor(1).deref(context).get_num_arguments();
            if raw.get_num_operands() != expected_operands {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: format!(
                        "block {block_index} typed index comparison has a malformed operand count"
                    ),
                });
            }
            (
                classify_subgroup_predicate(
                    entry,
                    layout,
                    sparse,
                    &uniformity,
                    branch.lhs(context),
                    branch.rhs(context),
                ),
                2,
            )
        } else if let Some(branch) = terminator.downcast_ref::<IndexEqualBranchOp>() {
            if raw.get_num_operands() != 2 {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: format!(
                        "block {block_index} equality comparison has a malformed operand count"
                    ),
                });
            }
            (
                classify_subgroup_equality(
                    entry,
                    layout,
                    sparse,
                    &uniformity,
                    branch.lhs(context),
                    branch.rhs(context),
                ),
                2,
            )
        } else if let Some(branch) = terminator.downcast_ref::<IndexEqualBranchArgsOp>() {
            if raw.get_num_successors() != 2 {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: format!(
                        "block {block_index} typed equality comparison has a malformed successor count"
                    ),
                });
            }
            let expected_operands = 2
                + raw.get_successor(0).deref(context).get_num_arguments()
                + raw.get_successor(1).deref(context).get_num_arguments();
            if raw.get_num_operands() != expected_operands {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: format!(
                        "block {block_index} typed equality comparison has a malformed operand count"
                    ),
                });
            }
            (
                classify_subgroup_equality(
                    entry,
                    layout,
                    sparse,
                    &uniformity,
                    branch.lhs(context),
                    branch.rhs(context),
                ),
                2,
            )
        } else if terminator
            .downcast_ref::<dialect_gpu::optimization_v1::CondBranchOp>()
            .is_some()
        {
            if raw.get_num_operands() == 0 {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: format!("block {block_index} Boolean branch has no condition"),
                });
            }
            let kind = match uniformity.fact(raw.get_operand(0)) {
                SubgroupValueUniformityV1::Uniform => SubgroupBranchUniformityV1::Uniform,
                SubgroupValueUniformityV1::Varying => SubgroupBranchUniformityV1::Varying,
                SubgroupValueUniformityV1::Unknown => SubgroupBranchUniformityV1::Unknown,
            };
            (kind, 2)
        } else if let Some(split) = terminator.downcast_ref::<AnalysisSplitOp>() {
            let dependencies = split.control_dependencies(context);
            let kind = if dependencies.is_empty() {
                SubgroupBranchUniformityV1::Unknown
            } else {
                match SubgroupValueUniformityV1::merge(
                    dependencies
                        .into_iter()
                        .map(|dependency| uniformity.fact(dependency)),
                ) {
                    SubgroupValueUniformityV1::Uniform => SubgroupBranchUniformityV1::Uniform,
                    SubgroupValueUniformityV1::Varying => SubgroupBranchUniformityV1::Varying,
                    SubgroupValueUniformityV1::Unknown => SubgroupBranchUniformityV1::Unknown,
                }
            };
            (kind, 2)
        } else {
            return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                detail: format!("block {block_index} has an unsupported terminator"),
            });
        };
        if raw.get_num_successors() != expected_successors {
            return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                detail: format!(
                    "block {block_index} terminator has {} successors, expected {expected_successors}",
                    raw.get_num_successors()
                ),
            });
        }
        let targets = raw
            .successors()
            .map(|successor| {
                block_indices.get(&successor).copied().ok_or_else(|| {
                    PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                        detail: format!("block {block_index} targets a block outside the kernel"),
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        successors.push(targets);
        branch_uniformity.push(kind);
    }

    let mut reachable = vec![false; blocks.len()];
    let entry_index = block_indices.get(&entry).copied().ok_or_else(|| {
        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
            detail: "the kernel entry block is outside its body region".to_owned(),
        }
    })?;
    let mut worklist = VecDeque::from([entry_index]);
    while let Some(block) = worklist.pop_front() {
        if reachable[block] {
            continue;
        }
        reachable[block] = true;
        worklist.extend(successors[block].iter().copied());
    }
    let cfg = SymbolicTensorCfgV1 {
        successors,
        branch_uniformity,
        reachable,
    };

    let tensor_blocks = tensor_sites.iter().copied().fold(
        BTreeMap::<usize, usize>::new(),
        |mut blocks, (block, operation)| {
            blocks.entry(block).or_insert(operation);
            blocks
        },
    );
    if tensor_blocks.len() > MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1 {
        return Err(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
    }
    let mut convergence_work = 0_usize;
    let predecessors = bounded_predecessors(&cfg.successors, &mut convergence_work)?;
    let postdominators = bounded_postdominators(
        &cfg.successors,
        &cfg.reachable,
        &predecessors,
        &mut convergence_work,
    )?;
    let control_regions = bounded_control_regions(
        &cfg.successors,
        &cfg.reachable,
        &cfg.branch_uniformity,
        &postdominators,
        &mut convergence_work,
    )?;
    let tensor_blocks = tensor_blocks.into_iter().collect::<Vec<_>>();
    let mut tensor_block_ids = Vec::new();
    tensor_block_ids
        .try_reserve_exact(tensor_blocks.len())
        .map_err(|_| PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    tensor_block_ids.extend(tensor_blocks.iter().map(|(block, _)| *block));
    let tensor_reachability = bounded_tensor_reachability(
        &cfg.successors,
        &predecessors,
        &tensor_block_ids,
        &mut convergence_work,
    )?;
    let edge_count = cfg
        .successors
        .iter()
        .try_fold(0_usize, |count, targets| count.checked_add(targets.len()));
    let Some(edge_count) = edge_count else {
        return Err(PlironTensorLayoutFindingV1::ResourceLimitExceeded);
    };
    let controller_query_work = cfg
        .successors
        .len()
        .checked_mul(2)
        .and_then(|blocks| blocks.checked_add(edge_count))
        .and_then(|per_tensor| per_tensor.checked_mul(tensor_blocks.len()))
        .ok_or(PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    charge_convergence_work(&mut convergence_work, controller_query_work)?;
    for (tensor_index, (tensor_block, tensor_operation)) in tensor_blocks.into_iter().enumerate() {
        if tensor_block >= cfg.successors.len() || !cfg.reachable[tensor_block] {
            continue;
        }
        for (controller, region) in control_regions
            .iter()
            .enumerate()
            .take(cfg.successors.len())
        {
            let kind = cfg.branch_uniformity[controller];
            let mut controls_future_tensor = false;
            for successor in cfg.successors[controller].iter().copied() {
                if tensor_reachability.block_reaches(successor, tensor_index)? {
                    controls_future_tensor = true;
                    break;
                }
            }
            if !cfg.reachable[controller]
                || !tensor_reachability.block_reaches(controller, tensor_index)?
                || !controls_future_tensor
                || kind == SubgroupBranchUniformityV1::Uniform
            {
                continue;
            }
            let Some(region) = region else {
                continue;
            };
            if !region.contains(tensor_block) && !region.has_cycle {
                continue;
            }
            return Err(match kind {
                SubgroupBranchUniformityV1::Varying => {
                    PlironTensorLayoutFindingV1::DivergentSubgroupControl {
                        block: tensor_block,
                        operation: tensor_operation,
                        controller,
                    }
                }
                SubgroupBranchUniformityV1::Unknown => {
                    PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                        detail: format!(
                            "tensor instruction at block {tensor_block} op {tensor_operation} is control-dependent on unresolved branch block {controller}"
                        ),
                    }
                }
                SubgroupBranchUniformityV1::Uniform => {
                    PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                        detail: format!(
                            "uniform controller block {controller} reached the divergent-control rejection boundary"
                        ),
                    }
                }
            });
        }
    }
    Ok(())
}

fn classify_subgroup_predicate(
    entry: Ptr<BasicBlock>,
    layout: PlironExecutionLayoutV1,
    sparse: &SparseIndexAnalysisV1,
    uniformity: &PlironSubgroupUniformityV1,
    lhs: Value,
    rhs: Value,
) -> SubgroupBranchUniformityV1 {
    if lhs == rhs {
        return SubgroupBranchUniformityV1::Uniform;
    }
    let lhs_is_entry_argument = value_is_entry_argument(lhs, entry);
    let rhs_is_entry_argument = value_is_entry_argument(rhs, entry);
    let lhs_uniformity = uniformity.fact(lhs);
    let rhs_uniformity = uniformity.fact(rhs);
    let lhs = sparse.fact_ref(lhs);
    let rhs = sparse.fact_ref(rhs);
    if (lhs_is_entry_argument
        || lhs_uniformity == SubgroupValueUniformityV1::Uniform
        || sparse_fact_is_subgroup_uniform(lhs, layout))
        && (rhs_is_entry_argument
            || rhs_uniformity == SubgroupValueUniformityV1::Uniform
            || sparse_fact_is_subgroup_uniform(rhs, layout))
    {
        return SubgroupBranchUniformityV1::Uniform;
    }
    if let (Some(lhs), Some(rhs)) = (lhs.affine(), rhs.affine()) {
        let differing_lane_coefficient = lhs
            .coefficients()
            .iter()
            .zip(rhs.coefficients())
            .enumerate()
            .any(|(dimension, (lhs, rhs))| {
                lhs != rhs && !invocation_axis_is_subgroup_uniform(dimension, layout)
            });
        if !differing_lane_coefficient
            && affine_is_total_over_layout(lhs, layout)
            && affine_is_total_over_layout(rhs, layout)
        {
            return SubgroupBranchUniformityV1::Uniform;
        }
    }
    if let Some(classification) = classify_coordinate_cutoff(lhs, rhs, layout, true) {
        return classification;
    }
    if let Some(classification) = classify_coordinate_cutoff(rhs, lhs, layout, false) {
        return classification;
    }
    if matches!(
        (lhs_uniformity, rhs_uniformity),
        (
            SubgroupValueUniformityV1::Varying,
            SubgroupValueUniformityV1::Uniform
        ) | (
            SubgroupValueUniformityV1::Uniform,
            SubgroupValueUniformityV1::Varying
        )
    ) {
        return SubgroupBranchUniformityV1::Varying;
    }
    SubgroupBranchUniformityV1::Unknown
}

fn classify_subgroup_equality(
    entry: Ptr<BasicBlock>,
    layout: PlironExecutionLayoutV1,
    sparse: &SparseIndexAnalysisV1,
    uniformity: &PlironSubgroupUniformityV1,
    lhs: Value,
    rhs: Value,
) -> SubgroupBranchUniformityV1 {
    if lhs == rhs {
        return SubgroupBranchUniformityV1::Uniform;
    }
    let lhs_uniformity = uniformity.fact(lhs);
    let rhs_uniformity = uniformity.fact(rhs);
    let lhs_fact = sparse.fact_ref(lhs);
    let rhs_fact = sparse.fact_ref(rhs);
    let lhs_is_uniform = value_is_entry_argument(lhs, entry)
        || lhs_uniformity == SubgroupValueUniformityV1::Uniform
        || sparse_fact_is_subgroup_uniform(lhs_fact, layout);
    let rhs_is_uniform = value_is_entry_argument(rhs, entry)
        || rhs_uniformity == SubgroupValueUniformityV1::Uniform
        || sparse_fact_is_subgroup_uniform(rhs_fact, layout);
    if lhs_is_uniform && rhs_is_uniform {
        return SubgroupBranchUniformityV1::Uniform;
    }
    if let (Some(lhs), Some(rhs)) = (lhs_fact.affine(), rhs_fact.affine())
        && lhs
            .coefficients()
            .iter()
            .zip(rhs.coefficients())
            .enumerate()
            .all(|(dimension, (lhs, rhs))| {
                lhs == rhs || invocation_axis_is_subgroup_uniform(dimension, layout)
            })
        && affine_is_total_over_layout(lhs, layout)
        && affine_is_total_over_layout(rhs, layout)
    {
        return SubgroupBranchUniformityV1::Uniform;
    }
    if matches!(
        (lhs_uniformity, rhs_uniformity),
        (
            SubgroupValueUniformityV1::Varying,
            SubgroupValueUniformityV1::Uniform
        ) | (
            SubgroupValueUniformityV1::Uniform,
            SubgroupValueUniformityV1::Varying
        )
    ) {
        return SubgroupBranchUniformityV1::Varying;
    }
    SubgroupBranchUniformityV1::Unknown
}

fn affine_is_total_over_layout(
    affine: &crate::SparseAffineIndexV1,
    layout: PlironExecutionLayoutV1,
) -> bool {
    let mut maximum = affine.constant_term();
    for (dimension, coefficient) in affine.coefficients().iter().copied().enumerate() {
        if coefficient == 0 {
            continue;
        }
        let Some(extent) = layout.global_extents.get(dimension).copied() else {
            return false;
        };
        let Some(coordinate) = extent.checked_sub(1) else {
            return false;
        };
        let Some(term) = coefficient.checked_mul(coordinate) else {
            return false;
        };
        let Some(next) = maximum.checked_add(term) else {
            return false;
        };
        maximum = next;
    }
    true
}
