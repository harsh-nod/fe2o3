#[derive(Clone, Copy)]
enum SubgroupSelectorV1 {
    Unconditional,
    LessThan(Value, Value),
    Equal(Value, Value),
    Boolean(Value),
    Split(Ptr<Operation>, usize),
}

struct SubgroupPhiSelectionV1 {
    selectors: Vec<SubgroupSelectorV1>,
    predecessors: Vec<Vec<usize>>,
    regions: Vec<Option<TensorControlRegionV1>>,
    reachable: Vec<bool>,
}

fn uniformity_vec_v1<T>(capacity: usize) -> Result<Vec<T>, PlironTensorLayoutFindingV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    Ok(values)
}

fn build_subgroup_phi_selection_v1(
    context: &Context,
    entry: Ptr<BasicBlock>,
    inventory: &BoundedPlironFunctionInventoryV1,
    work: &mut usize,
) -> Result<SubgroupPhiSelectionV1, PlironTensorLayoutFindingV1> {
    let blocks = inventory.blocks();
    // Ordinal keys, the four retained row headers, reachability, and the
    // bounded pending stack are admitted before their allocation/initialization.
    charge_uniformity_collection(
        work,
        blocks
            .len()
            .checked_mul(8)
            .ok_or(PlironTensorLayoutFindingV1::ResourceLimitExceeded)?,
    )?;
    let mut indices = HashMap::new();
    indices
        .try_reserve(blocks.len())
        .map_err(|_| PlironTensorLayoutFindingV1::ResourceLimitExceeded)?;
    for (ordinal, block) in blocks.iter().copied().enumerate() {
        if indices.insert(block, ordinal).is_some() {
            return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                detail: "phi selection has a duplicate CFG block".to_owned(),
            });
        }
    }
    let mut successors = uniformity_vec_v1(blocks.len())?;
    let mut selectors = uniformity_vec_v1(blocks.len())?;
    let mut potential_controllers = uniformity_vec_v1(blocks.len())?;
    for block in blocks {
        charge_uniformity_collection(work, 1)?;
        let terminator = block
            .deref(context)
            .get_terminator(context)
            .ok_or_else(
                || PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "phi selection block has no terminator".to_owned(),
                },
            )?;
        let dynamic = Operation::get_op_dyn(terminator, context);
        let raw = terminator.deref(context);
        charge_uniformity_collection(
            work,
            raw.get_num_successors()
                .checked_add(raw.get_num_operands())
                .ok_or(PlironTensorLayoutFindingV1::ResourceLimitExceeded)?,
        )?;
        let (selector, expected_successors) = if dynamic.downcast_ref::<ReturnOp>().is_some()
            || dynamic.downcast_ref::<TrapOp>().is_some()
        {
            (SubgroupSelectorV1::Unconditional, 0)
        } else if dynamic.downcast_ref::<BranchOp>().is_some()
            || dynamic.downcast_ref::<BranchArgsOp>().is_some()
        {
            (SubgroupSelectorV1::Unconditional, 1)
        } else if dynamic.downcast_ref::<IndexLessThanBranchOp>().is_some()
            || dynamic
                .downcast_ref::<IndexLessThanBranchArgsOp>()
                .is_some()
        {
            if raw.get_num_operands() < 2 {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "phi selection comparison has no operand pair".to_owned(),
                });
            }
            (
                SubgroupSelectorV1::LessThan(raw.get_operand(0), raw.get_operand(1)),
                2,
            )
        } else if dynamic.downcast_ref::<IndexEqualBranchOp>().is_some()
            || dynamic.downcast_ref::<IndexEqualBranchArgsOp>().is_some()
        {
            if raw.get_num_operands() < 2 {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "phi selection equality has no operand pair".to_owned(),
                });
            }
            (
                SubgroupSelectorV1::Equal(raw.get_operand(0), raw.get_operand(1)),
                2,
            )
        } else if dynamic
            .downcast_ref::<dialect_gpu::optimization_v1::CondBranchOp>()
            .is_some()
        {
            if raw.get_num_operands() == 0 {
                return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "Boolean selection has no condition".to_owned(),
                });
            }
            (SubgroupSelectorV1::Boolean(raw.get_operand(0)), 2)
        } else if let Some(split) = dynamic.downcast_ref::<AnalysisSplitOp>() {
            let count = split.control_dependencies(context).len();
            (SubgroupSelectorV1::Split(terminator, count), 2)
        } else {
            return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                detail: "phi selection has an unsupported terminator".to_owned(),
            });
        };
        if raw.get_num_successors() != expected_successors {
            return Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                detail: "phi selection has a malformed successor count".to_owned(),
            });
        }
        let mut targets = uniformity_vec_v1(expected_successors)?;
        for successor in raw.successors() {
            targets.push(*indices.get(&successor).ok_or_else(|| {
                PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                    detail: "phi selection edge leaves the kernel CFG".to_owned(),
                }
            })?);
        }
        successors.push(targets);
        selectors.push(selector);
        potential_controllers.push(if expected_successors > 1 {
            SubgroupBranchUniformityV1::Unknown
        } else {
            SubgroupBranchUniformityV1::Uniform
        });
    }
    let mut reachable = uniformity_vec_v1(blocks.len())?;
    reachable.resize(blocks.len(), false);
    let entry = *indices.get(&entry).ok_or_else(|| {
        PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
            detail: "phi selection entry is outside the kernel CFG".to_owned(),
        }
    })?;
    let mut pending = uniformity_vec_v1(blocks.len())?;
    reachable[entry] = true;
    pending.push(entry);
    while let Some(block) = pending.pop() {
        charge_uniformity_collection(work, 1)?;
        for successor in &successors[block] {
            charge_uniformity_collection(work, 1)?;
            if !reachable[*successor] {
                reachable[*successor] = true;
                pending.push(*successor);
            }
        }
    }
    let predecessors = bounded_predecessors(&successors, work)?;
    let postdominators = bounded_postdominators(&successors, &reachable, &predecessors, work)?;
    // All conditional controllers participate. Their facts may worsen during
    // the value fixed point, but their immutable control regions never change.
    let regions = bounded_control_regions(
        &successors,
        &reachable,
        &potential_controllers,
        &postdominators,
        work,
    )?;
    Ok(SubgroupPhiSelectionV1 {
        selectors,
        predecessors,
        regions,
        reachable,
    })
}

fn classify_subgroup_selector_v1(
    context: &Context,
    entry: Ptr<BasicBlock>,
    layout: PlironExecutionLayoutV1,
    sparse: &SparseIndexAnalysisV1,
    uniformity: &PlironSubgroupUniformityV1,
    selector: SubgroupSelectorV1,
    work: &mut usize,
) -> Result<SubgroupValueUniformityV1, PlironTensorLayoutFindingV1> {
    charge_uniformity_collection(work, 1)?;
    let kind = match selector {
        SubgroupSelectorV1::Unconditional => return Ok(SubgroupValueUniformityV1::Uniform),
        SubgroupSelectorV1::Boolean(condition) => {
            charge_uniformity_collection(work, 1)?;
            return Ok(uniformity.fact(condition));
        }
        SubgroupSelectorV1::Split(operation, count) => {
            charge_uniformity_collection(work, count)?;
            return Ok(SubgroupValueUniformityV1::merge((0..count).map(|index| {
                uniformity.fact(operation.deref(context).get_operand(index))
            })));
        }
        SubgroupSelectorV1::LessThan(lhs, rhs) | SubgroupSelectorV1::Equal(lhs, rhs) => {
            // Fixed key/fact observations, two uniform-axis scans, affine
            // cancellation and totality, and two coordinate-cutoff proofs.
            // Axis stride scans are rank-bounded; sparse facts are borrowed.
            const RANK: usize = dialect_kernel::MAX_RANKED_MEMORY_RANK;
            const VALUE_OBSERVATIONS: usize = 32;
            const UNIFORM_AXES: usize = 2 * RANK * (8 + RANK);
            const AFFINE_CANCELLATION: usize = RANK * (8 + RANK);
            const AFFINE_TOTALITY: usize = 2 * RANK * 8;
            const COORDINATE_CUTOFFS: usize = 2 * (16 + RANK * (8 + RANK));
            charge_uniformity_collection(
                work,
                VALUE_OBSERVATIONS
                    + UNIFORM_AXES
                    + AFFINE_CANCELLATION
                    + AFFINE_TOTALITY
                    + COORDINATE_CUTOFFS,
            )?;
            if matches!(selector, SubgroupSelectorV1::LessThan(..)) {
                classify_subgroup_predicate(entry, layout, sparse, uniformity, lhs, rhs)
            } else {
                classify_subgroup_equality(entry, layout, sparse, uniformity, lhs, rhs)
            }
        }
    };
    Ok(match kind {
        SubgroupBranchUniformityV1::Uniform => SubgroupValueUniformityV1::Uniform,
        SubgroupBranchUniformityV1::Unknown => SubgroupValueUniformityV1::Unknown,
        SubgroupBranchUniformityV1::Varying => SubgroupValueUniformityV1::Varying,
    })
}

fn subgroup_phi_selection_fact_v1(
    topology: &SubgroupPhiSelectionV1,
    selector_facts: &[SubgroupValueUniformityV1],
    block: usize,
    inputs: &[Value],
    work: &mut usize,
) -> Result<SubgroupValueUniformityV1, PlironTensorLayoutFindingV1> {
    charge_uniformity_collection(
        work,
        inputs
            .len()
            .checked_add(1)
            .ok_or(PlironTensorLayoutFindingV1::ResourceLimitExceeded)?,
    )?;
    if !topology.reachable[block] || inputs.iter().all(|input| Some(input) == inputs.first()) {
        return Ok(SubgroupValueUniformityV1::Uniform);
    }
    let mut fact = SubgroupValueUniformityV1::Uniform;
    for (controller, selector) in selector_facts.iter().copied().enumerate() {
        charge_uniformity_collection(work, 1)?;
        if selector.rank() <= fact.rank() || !topology.reachable[controller] {
            continue;
        }
        for predecessor in &topology.predecessors[block] {
            charge_uniformity_collection(work, 1)?;
            // The controller's own edge is essential when both physical
            // successors are the same block but carry distinct phi operands.
            if controller == *predecessor
                || topology.regions[controller]
                    .as_ref()
                    .is_some_and(|region| region.contains(*predecessor))
            {
                fact = selector;
                break;
            }
        }
    }
    Ok(fact)
}

#[cfg(test)]
mod phi_selection_resource_tests {
    use super::*;

    #[test]
    fn non_exiting_irreducible_regions_keep_their_divergent_selector() {
        let context = &mut Context::new();
        dialect_kernel::register_dialect(
            context,
            &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let left = IndexConstantOp::new(context, 0).result(context);
        let right = IndexConstantOp::new(context, 1).result(context);
        let successors = vec![vec![1, 2], vec![2], vec![1]];
        let reachable = vec![true; 3];
        let mut work = 0;
        let predecessors = bounded_predecessors(&successors, &mut work).unwrap();
        let postdominators =
            bounded_postdominators(&successors, &reachable, &predecessors, &mut work).unwrap();
        assert!(postdominators.iter().all(Option::is_none));
        let regions = bounded_control_regions(
            &successors,
            &reachable,
            &[
                SubgroupBranchUniformityV1::Unknown,
                SubgroupBranchUniformityV1::Uniform,
                SubgroupBranchUniformityV1::Uniform,
            ],
            &postdominators,
            &mut work,
        )
        .unwrap();
        assert!(regions[0].as_ref().unwrap().has_cycle);
        let topology = SubgroupPhiSelectionV1 {
            selectors: vec![SubgroupSelectorV1::Unconditional; 3],
            predecessors,
            regions,
            reachable,
        };
        assert_eq!(
            subgroup_phi_selection_fact_v1(
                &topology,
                &[
                    SubgroupValueUniformityV1::Varying,
                    SubgroupValueUniformityV1::Uniform,
                    SubgroupValueUniformityV1::Uniform
                ],
                1,
                &[left, right],
                &mut work
            )
            .unwrap(),
            SubgroupValueUniformityV1::Varying
        );
    }

    #[test]
    fn phi_selection_query_charges_exact_identity_and_controller_visits() {
        let context = &mut Context::new();
        dialect_kernel::register_dialect(
            context,
            &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let left = IndexConstantOp::new(context, 0).result(context);
        let right = IndexConstantOp::new(context, 1).result(context);
        let topology = SubgroupPhiSelectionV1 {
            selectors: vec![SubgroupSelectorV1::Unconditional; 2],
            predecessors: vec![vec![], vec![0, 0]],
            regions: vec![None, None],
            reachable: vec![true; 2],
        };
        let selectors = [
            SubgroupValueUniformityV1::Varying,
            SubgroupValueUniformityV1::Uniform,
        ];
        // Two identity comparisons + header, two controller visits, and the
        // first direct predecessor query. The second duplicate is not queried.
        const DISTINCT_WORK: usize = 2 + 1 + 2 + 1;
        let mut work = MAX_PLIRON_TENSOR_UNIFORMITY_WORK_UNITS_V1 - DISTINCT_WORK;
        assert_eq!(
            subgroup_phi_selection_fact_v1(&topology, &selectors, 1, &[left, right], &mut work)
                .unwrap(),
            SubgroupValueUniformityV1::Varying
        );
        assert_eq!(work, MAX_PLIRON_TENSOR_UNIFORMITY_WORK_UNITS_V1);
        let mut work = MAX_PLIRON_TENSOR_UNIFORMITY_WORK_UNITS_V1 - DISTINCT_WORK + 1;
        assert!(matches!(
            subgroup_phi_selection_fact_v1(&topology, &selectors, 1, &[left, right], &mut work),
            Err(PlironTensorLayoutFindingV1::ResourceLimitExceeded)
        ));
        let mut work = 0;
        assert_eq!(
            subgroup_phi_selection_fact_v1(&topology, &selectors, 1, &[left, left], &mut work)
                .unwrap(),
            SubgroupValueUniformityV1::Uniform
        );
        assert_eq!(work, 2 + 1);
    }
}
