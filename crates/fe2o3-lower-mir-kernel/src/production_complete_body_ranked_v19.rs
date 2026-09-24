//! Exact analysis projection of the one retained V19 executable graph.
//! This is not another executable body. Authored integer steps keep their
//! actual SSA dependencies; their arithmetic remains in the retained KIR.
//! Only the compiler-owned guarded store is split into analysis control flow.
use super::*;
use fe2o3_kernel_ir::{OperationKind as Op, Terminator as End, VerifiedCanonicalKernelIrModuleV19};
use fe2o3_pliron::{
    ProductionRankedBlockV1 as Block, ProductionRankedKernelV1 as Recipe,
    ProductionRankedOperationV1 as OpR, ProductionRankedTerminatorV1 as EndR,
    ProductionRankedValueIdV1 as IdR, ProductionRankedValueV1 as ValueR,
};

const PROJECTION_WORK_V19: usize = 16_384;
pub(super) const PROJECTION_STORAGE_V19: usize = 256 * 1024;
const OUTPUT_VIEW_V19: IdR = IdR::new(0);

// Reviewed pre-construction maximum: six block slots, four source blocks each
// reserving 24 operation slots plus one store, 16 binary dependency vectors,
// four two-role edges, one guarded-store edge value, one view extent, one
// access index and a 128-byte name.
// The existing ranked constructor/session owns its independently bounded
// validation and preverification-transform allocation domain.
fn projection_storage_bound_v19() -> Result<usize, ProductionCompleteBodyCheckErrorV19> {
    Ok(argument_sum_v1(&[
        std::mem::size_of::<Recipe>(),
        argument_product_v1(6, std::mem::size_of::<Block>())?,
        argument_product_v1(97, std::mem::size_of::<OpR>())?,
        argument_product_v1(43, std::mem::size_of::<ValueR>())?,
        std::mem::size_of::<u64>(),
        128,
    ])?)
}
fn projection_vec_v19<T>(count: usize) -> Result<Vec<T>, ProductionCompleteBodyCheckErrorV19> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(count)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    if output.capacity() > count {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    Ok(output)
}
fn projection_copy_v19<T: Clone>(
    values: &[T],
) -> Result<Vec<T>, ProductionCompleteBodyCheckErrorV19> {
    let mut output = projection_vec_v19(values.len())?;
    output.extend_from_slice(values);
    Ok(output)
}

fn projection_refusal(detail: &'static str) -> ProductionCompleteBodyCheckErrorV19 {
    ProductionCompleteBodyCheckErrorV19::Relation(detail)
}

struct ValuesV19 {
    values: [Option<(ValueId, ValueR)>; 48],
    count: usize,
}
impl ValuesV19 {
    fn new() -> Self {
        Self {
            values: [None; 48],
            count: 0,
        }
    }
    fn insert(
        &mut self,
        source: ValueId,
        target: ValueR,
    ) -> Result<(), ProductionCompleteBodyCheckErrorV19> {
        if self.count == self.values.len()
            || self.values[..self.count]
                .iter()
                .flatten()
                .any(|(value, _)| *value == source)
        {
            return Err(projection_refusal(
                "duplicate or excessive V19 projection value",
            ));
        }
        self.values[self.count] = Some((source, target));
        self.count += 1;
        Ok(())
    }
    fn get(&self, source: ValueId) -> Result<ValueR, ProductionCompleteBodyCheckErrorV19> {
        self.values[..self.count]
            .iter()
            .flatten()
            .find_map(|(value, target)| (*value == source).then_some(*target))
            .ok_or_else(|| {
                projection_refusal("V19 projection operand has no actual SSA definition")
            })
    }
}
fn result_v19(
    operation: &fe2o3_kernel_ir::Operation,
) -> Result<ValueId, ProductionCompleteBodyCheckErrorV19> {
    let [result] = operation.results.as_slice() else {
        return Err(projection_refusal("V19 projection expected one result"));
    };
    Ok(result.id)
}
fn edge_values_v19(
    values: &ValuesV19,
    edge: &[ValueId],
) -> Result<Vec<ValueR>, ProductionCompleteBodyCheckErrorV19> {
    if edge.len() > 2 {
        return Err(projection_refusal("V19 projection edge bound"));
    }
    let mut output = projection_vec_v19(edge.len())?;
    for value in edge {
        output.push(values.get(*value)?);
    }
    Ok(output)
}

fn guard_tail_v19(
    index: ValueR,
    length: ValueR,
    store: u32,
    done: u32,
) -> Result<EndR, ProductionCompleteBodyCheckErrorV19> {
    Ok(EndR::IndexLessThanArgs {
        lhs: index,
        rhs: length,
        true_arguments: projection_copy_v19(&[index])?,
        false_arguments: Vec::new(),
        true_block: store,
        false_block: done,
    })
}
fn store_block_v19(store: u32, done: u32) -> Result<Block, ProductionCompleteBodyCheckErrorV19> {
    let mut operations = projection_vec_v19(1)?;
    operations.push(OpR::Access {
        kind: dialect_kernel::AccessKindAttr::Write,
        view: ValueR::Local(OUTPUT_VIEW_V19),
        indices: projection_copy_v19(&[ValueR::BlockArgument {
            block: store,
            argument: 0,
        }])?,
    });
    Ok(Block::with_index_arguments(
        1,
        operations,
        EndR::Branch { target: done },
    ))
}

/// Private constructor: only a typed canonical owner and actual source-derived
/// launch reach this analysis view. Every observable memory/control dependency
/// is reconstructed; source equivalence is required by the owning caller.
pub(super) fn complete_body_ranked_recipe_v19(
    owner: &VerifiedCanonicalKernelIrModuleV19,
    launch: &crate::ProductionSourceLaunchRosterV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Recipe, ProductionCompleteBodyCheckErrorV19> {
    budget.charge_work(PROJECTION_WORK_V19)?;
    // One small fixed profile only; reserve before any vectors/strings below.
    // The returned receipt is serviced by the caller's enclosing scope.
    if projection_storage_bound_v19()? > PROJECTION_STORAGE_V19 {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    budget.reserve_storage(PROJECTION_STORAGE_V19)?;
    let module = owner.module();
    let [function] = module.functions.as_slice() else {
        return Err(projection_refusal(
            "V19 projection requires one actual function",
        ));
    };
    let [kernel] = module.kernels.as_slice() else {
        return Err(projection_refusal(
            "V19 projection requires one actual kernel",
        ));
    };
    let [source_root] = launch.roots() else {
        return Err(projection_refusal(
            "V19 projection requires one source launch root",
        ));
    };
    let layout = source_root.layout();
    let global = layout.global_extents();
    if source_root.source_rank() != 1
        || global[0] == 0
        || global[1..] != [1, 1]
        || global[0] % 64 != 0
        || layout.workgroup_extents() != [64, 1, 1]
        || layout.subgroup_size() != 64
        || !layout.full_physical_workgroups()
        || kernel.entry != function.id
    {
        return Err(projection_refusal(
            "V19 projection source launch profile differs",
        ));
    }
    let body = function
        .body
        .as_ref()
        .ok_or_else(|| projection_refusal("V19 projection body absent"))?;
    if !matches!(body.blocks.len(), 1 | 4) || body.parameters.len() != 5 {
        return Err(projection_refusal("V19 projection actual body shape"));
    }
    let Some(fe2o3_kernel_ir::Operation {
        kind: Op::Gfx942CompleteBodyDeclaration(_),
        ..
    }) = body.blocks[0].operations.first()
    else {
        return Err(projection_refusal(
            "V19 projection has no complete-body declaration",
        ));
    };
    let mut values = ValuesV19::new();
    // Ranked argument zero names the exact output slice's runtime length,
    // not its pointer. The four u32 source scalars retain their parameter order.
    for (ordinal, value) in body.parameters.iter().enumerate().skip(1) {
        values.insert(*value, ValueR::Argument(ordinal as u32))?;
    }
    for (ordinal, block) in body.blocks.iter().enumerate() {
        if block.id.0 != ordinal as u32 || block.parameters.len() > 2 {
            return Err(projection_refusal("V19 projection block identity"));
        }
        for (parameter, value) in block.parameters.iter().enumerate() {
            values.insert(
                value.id,
                ValueR::BlockArgument {
                    block: ordinal as u32,
                    argument: parameter as u32,
                },
            )?;
        }
    }
    let mut next = 1u32;
    let mut blocks = projection_vec_v19(body.blocks.len() + 2)?;
    let mut store_value = None;
    let mut index_and_length = None;
    let mut authored = 0usize;
    for (ordinal, block) in body.blocks.iter().enumerate() {
        let mut operations = projection_vec_v19(24)?;
        if ordinal == 0 {
            operations.push(OpR::ExecutionLayout {
                grid_identity: layout.grid_identity(),
                global_extents: global,
                workgroup_extents: layout.workgroup_extents(),
                subgroup_size: layout.subgroup_size(),
                full_physical_workgroups: layout.full_physical_workgroups(),
            });
            operations.push(OpR::ViewInSpace {
                result: OUTPUT_VIEW_V19,
                element_width: 32,
                writable: true,
                shape: projection_copy_v19(&[dialect_kernel::DYNAMIC_EXTENT])?,
                dynamic_extents: projection_copy_v19(&[ValueR::Argument(0)])?,
                allocation_origin: 1,
                noalias_class: 1,
                memory_space: dialect_kernel::MemorySpaceAttr::Global,
            });
            // One exact output allocation, no pointer input or helper. This
            // asks the existing hierarchy pass to prove ownership of writes;
            // it does not assert total coverage of an unknown runtime length.
            operations.push(OpR::OwnershipContract {
                view: ValueR::Local(OUTPUT_VIEW_V19),
                coverage: dialect_kernel::OwnershipCoverageAttr::ExactEffectDomain,
                partition: dialect_kernel::OwnershipPartitionAttr::DenseRectangles,
            });
        }
        let mut comparison = None;
        for operation in &block.operations {
            match &operation.kind {
                Op::Gfx942CompleteBodyDeclaration(_) if ordinal == 0 => {}
                Op::Gfx942CompleteBodyStep(step) => {
                    authored += 1;
                    if authored > 16 {
                        return Err(projection_refusal("V19 authored-step bound"));
                    }
                    let result = IdR::new(next);
                    next += 1;
                    let mut dependencies = projection_vec_v19(2)?;
                    for value in step.operands.iter().flatten() {
                        dependencies.push(values.get(*value)?);
                    }
                    if dependencies.is_empty() || dependencies.len() > 2 {
                        return Err(projection_refusal("V19 step dependency shape"));
                    }
                    operations.push(OpR::DeterministicJoin {
                        result,
                        dependencies,
                    });
                    values.insert(result_v19(operation)?, ValueR::Local(result))?;
                }
                Op::Constant(fe2o3_kernel_ir::Constant::U32(0)) => {
                    let result = IdR::new(next);
                    next += 1;
                    operations.push(OpR::IndexConstant { result, value: 0 });
                    values.insert(result_v19(operation)?, ValueR::Local(result))?;
                }
                Op::Compare {
                    predicate,
                    lhs,
                    rhs,
                } => {
                    if comparison
                        .replace((*predicate, values.get(*lhs)?, values.get(*rhs)?))
                        .is_some()
                    {
                        return Err(projection_refusal("V19 repeated comparison"));
                    }
                }
                Op::Intrinsic(intrinsic)
                    if *intrinsic == fe2o3_kernel_ir::IntrinsicOperation::global_id_1d() =>
                {
                    let result = IdR::new(next);
                    next += 1;
                    operations.push(OpR::InvocationIndex {
                        result,
                        dimension: 0,
                        launch_extent: global[0],
                    });
                    values.insert(result_v19(operation)?, ValueR::Local(result))?;
                }
                Op::SliceLength { slice } if *slice == body.parameters[0] => {
                    let result = IdR::new(next);
                    next += 1;
                    operations.push(OpR::Dimension {
                        result,
                        view: ValueR::Local(OUTPUT_VIEW_V19),
                        dimension: 0,
                    });
                    values.insert(result_v19(operation)?, ValueR::Local(result))?;
                }
                // The typed owner's closed tail relation requires these exact
                // address operands. They add no extra analysis memory effects.
                Op::SliceData { slice } if *slice == body.parameters[0] => {}
                Op::GetElementPointer { .. } => {}
                Op::GuardedStore { value, .. } => {
                    if store_value.replace(values.get(*value)?).is_some() {
                        return Err(projection_refusal("V19 multiple guarded stores"));
                    }
                }
                _ => return Err(projection_refusal("unsupported V19 analysis operation")),
            }
        }
        let terminator = match block.terminator.as_ref() {
            Some(End::ConditionalBranch {
                then_target,
                then_arguments,
                else_target,
                else_arguments,
                ..
            }) => {
                let Some((fe2o3_kernel_ir::ComparePredicate::Equal, lhs, rhs)) = comparison else {
                    return Err(projection_refusal("V19 selector comparison differs"));
                };
                EndR::IndexEqualArgs {
                    lhs,
                    rhs,
                    true_arguments: edge_values_v19(&values, then_arguments)?,
                    false_arguments: edge_values_v19(&values, else_arguments)?,
                    true_block: then_target.0,
                    false_block: else_target.0,
                }
            }
            Some(End::Branch { target, arguments }) => EndR::BranchArgs {
                arguments: edge_values_v19(&values, arguments)?,
                target: target.0,
            },
            Some(End::Return { values: returned }) if returned.is_empty() => {
                let Some((fe2o3_kernel_ir::ComparePredicate::LessThan, index, length)) = comparison
                else {
                    return Err(projection_refusal("V19 actual tail comparison differs"));
                };
                if index_and_length.replace((index, length)).is_some() {
                    return Err(projection_refusal("V19 repeated tail"));
                }
                // The invocation index may be defined in a nonentry merge
                // block. Preserve its exact SSA value through the new edge.
                guard_tail_v19(
                    index,
                    length,
                    body.blocks.len() as u32,
                    body.blocks.len() as u32 + 1,
                )?
            }
            _ => return Err(projection_refusal("unsupported V19 analysis terminator")),
        };
        blocks.push(Block::with_index_arguments(
            block.parameters.len() as u32,
            operations,
            terminator,
        ));
    }
    index_and_length.ok_or_else(|| projection_refusal("V19 tail absent"))?;
    // A write effect, not an invented typed semantic-expression theorem. The
    // actual u32 RHS and its SSA graph remain in the immutable KIR19 owner;
    // unsupported functional-refinement bindings are refused by the compiler.
    let _value = store_value.ok_or_else(|| projection_refusal("V19 store absent"))?;
    blocks.push(store_block_v19(
        body.blocks.len() as u32,
        body.blocks.len() as u32 + 1,
    )?);
    blocks.push(Block::new(Vec::new(), EndR::Return));
    Recipe::new(function.id.as_str(), 5, blocks)
        .map_err(ProductionCompleteBodyCheckErrorV19::RankedRecipe)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewed_projection_payload_fits_prepaid_bound() {
        assert!(projection_storage_bound_v19().unwrap() <= PROJECTION_STORAGE_V19);
        assert!(projection_vec_v19::<ValueR>(2).unwrap().capacity() <= 2);
    }
    #[test]
    fn dependency_projection_uses_actual_noncontiguous_ssa_ids() {
        let mut values = ValuesV19::new();
        values.insert(ValueId(57), ValueR::Argument(2)).unwrap();
        values
            .insert(
                ValueId(1001),
                ValueR::BlockArgument {
                    block: 3,
                    argument: 1,
                },
            )
            .unwrap();
        assert_eq!(values.get(ValueId(57)).unwrap(), ValueR::Argument(2));
        assert_eq!(
            edge_values_v19(&values, &[ValueId(1001), ValueId(57)]).unwrap(),
            vec![
                ValueR::BlockArgument {
                    block: 3,
                    argument: 1
                },
                ValueR::Argument(2)
            ]
        );
    }
    #[test]
    fn undefined_and_duplicate_ssa_dependencies_refuse() {
        let mut values = ValuesV19::new();
        assert!(values.get(ValueId(1)).is_err());
        values.insert(ValueId(1), ValueR::Argument(1)).unwrap();
        assert!(values.insert(ValueId(1), ValueR::Argument(2)).is_err());
        assert!(edge_values_v19(&values, &[ValueId(1), ValueId(2)]).is_err());
    }
    #[test]
    fn fixed_projection_definition_bound_is_closed() {
        let mut values = ValuesV19::new();
        for index in 0..48 {
            values
                .insert(ValueId(index), ValueR::Argument(index))
                .unwrap();
        }
        assert!(values.insert(ValueId(48), ValueR::Argument(48)).is_err());
    }
    #[test]
    fn fixed_role_edge_bound_refuses_extra_argument() {
        let mut values = ValuesV19::new();
        values.insert(ValueId(1), ValueR::Argument(1)).unwrap();
        assert!(edge_values_v19(&values, &[ValueId(1); 3]).is_err());
    }
    // These are inert ranked-recipe SSA tests, not source/launch owner fixtures.
    // The same tail constructors are used by the live V19 projection above.
    fn nonentry_tail_fixture(direct_local: bool, omit_edge_argument: bool) -> Vec<Block> {
        let index = ValueR::Local(IdR::new(1));
        let length = ValueR::Local(IdR::new(2));
        let mut guard = guard_tail_v19(index, length, 2, 3).unwrap();
        if omit_edge_argument {
            let EndR::IndexLessThanArgs { true_arguments, .. } = &mut guard else {
                unreachable!()
            };
            true_arguments.clear();
        }
        let store = if direct_local {
            Block::with_index_arguments(
                1,
                vec![OpR::Access {
                    kind: dialect_kernel::AccessKindAttr::Write,
                    view: ValueR::Local(OUTPUT_VIEW_V19),
                    indices: vec![index],
                }],
                EndR::Branch { target: 3 },
            )
        } else {
            store_block_v19(2, 3).unwrap()
        };
        vec![
            Block::new(
                vec![
                    OpR::ExecutionLayout {
                        grid_identity: 7,
                        global_extents: [128, 1, 1],
                        workgroup_extents: [64, 1, 1],
                        subgroup_size: 64,
                        full_physical_workgroups: true,
                    },
                    OpR::ViewInSpace {
                        result: OUTPUT_VIEW_V19,
                        element_width: 32,
                        writable: true,
                        shape: vec![dialect_kernel::DYNAMIC_EXTENT],
                        dynamic_extents: vec![ValueR::Argument(0)],
                        allocation_origin: 1,
                        noalias_class: 1,
                        memory_space: dialect_kernel::MemorySpaceAttr::Global,
                    },
                ],
                EndR::Branch { target: 1 },
            ),
            Block::new(
                vec![
                    OpR::InvocationIndex {
                        result: IdR::new(1),
                        dimension: 0,
                        launch_extent: 128,
                    },
                    OpR::Dimension {
                        result: IdR::new(2),
                        view: ValueR::Local(OUTPUT_VIEW_V19),
                        dimension: 0,
                    },
                ],
                guard,
            ),
            store,
            Block::new(vec![], EndR::Return),
        ]
    }

    #[test]
    fn nonentry_guard_tail_passes_actual_index_as_store_block_argument() {
        let blocks = nonentry_tail_fixture(false, false);
        assert!(matches!(blocks[1].terminator(), EndR::IndexLessThanArgs {
            lhs: ValueR::Local(index), true_arguments, true_block: 2, false_block: 3, ..
        } if *index == IdR::new(1) && true_arguments.as_slice() == [ValueR::Local(IdR::new(1))]));
        assert_eq!(blocks[2].index_argument_count(), 1);
        assert!(Recipe::new("nonentry_guard", 1, blocks).is_ok());
    }

    #[test]
    fn nonentry_guard_tail_refuses_direct_cross_block_index() {
        assert!(matches!(
            Recipe::new("nonentry_guard", 1, nonentry_tail_fixture(true, false)),
            Err(
                fe2o3_pliron::ProductionRankedKernelErrorV1::CrossBlockDefinitionRequiresArgument {
                    definition_block: 1,
                    use_block: 2
                }
            )
        ));
    }

    #[test]
    fn nonentry_guard_tail_refuses_missing_actual_edge_argument() {
        assert!(Recipe::new("nonentry_guard", 1, nonentry_tail_fixture(false, true)).is_err());
    }
}
