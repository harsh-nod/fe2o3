//! Rejection inventory, never an issuer. Nominal old-scope descendants whose
//! erased marker has no stored field edge must also come from their exact
//! retained source contract. Scalar results do not inherit capability custody.
use super::super::borrowed_workgroup_01::checked_execution_source_call_v1;
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticGfx950TransposeOperationV1 as T, SemanticSubgroupPartitionOperationV1 as P,
};

pub(super) fn collect(
    owner: &ProductionSemanticSsaOwnerV1,
    view: &SemanticExpandedRootV1,
    context: &RootKernelContextLoweringV1,
    graph: &mut Graph<'_>,
    scope: SemanticExecutionCapabilityContractV1,
    initial: &[SemanticTypeIdV1],
) -> Result<Vec<SemanticTypeIdV1>> {
    graph.charge(3 + initial.len())?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(initial.len())
        .map_err(|_| allocation())?;
    result.extend_from_slice(initial);
    for (index, block) in view.body().blocks().iter().enumerate() {
        graph.charge(1)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            ..
        }) = owner
            .source_semantic()
            .callables()
            .get(call.callee().index() as usize)
        else {
            continue;
        };
        if contract.provenance() != scope.provenance()
            || contract.workgroup_brand() != scope.workgroup_brand()
            || contract.epoch_before() != scope.epoch_before()
        {
            continue;
        }
        let (_, contract) = checked_execution_source_call_v1(owner, view, context, index as u32)?;
        for ty in old_types(contract.operation()).into_iter().flatten() {
            graph.charge(1 + result.len())?;
            if !result.contains(&ty) {
                graph.charge(1)?;
                result.try_reserve_exact(1).map_err(|_| allocation())?;
                result.push(ty);
            }
        }
    }
    Ok(result)
}

fn allocation() -> ProductionSemanticKirErrorV1 {
    ProductionSemanticKirErrorV1::AllocationFailure {
        resource: ProductionSemanticKirResourceV1::AnalysisWork,
    }
}

fn old_types(operation: SemanticExecutionCapabilityOperationV1) -> [Option<SemanticTypeIdV1>; 3] {
    use SemanticExecutionCapabilityOperationV1 as O;
    let one = |a| [Some(a), None, None];
    let two = |a, b| [Some(a), Some(b), None];
    match operation {
        O::Gfx950Transpose(t) => match t.operation() {
            T::Issue {
                partition, tile, ..
            } => two(partition, tile),
            T::Stage {
                input_tile,
                output_tile,
                view,
                ..
            } => [Some(input_tile), Some(output_tile), Some(view)],
            T::Publish {
                input_tile,
                input_workgroup,
                ..
            } => two(input_tile, input_workgroup),
            T::Read {
                tile,
                lane,
                fragment,
                ..
            } => [Some(tile), Some(lane), Some(fragment)],
        },
        O::SubgroupPartition(p) => match p {
            P::Derive {
                subgroup,
                partition,
                ..
            } => two(subgroup, partition),
            P::ReduceSumF32 { partition, .. }
            | P::ReduceMaxF32 { partition, .. }
            | P::BroadcastF32 { partition, .. } => one(partition),
        },
        O::WorkgroupDerive { workgroup, .. } => one(workgroup),
        O::SubgroupDerive {
            workgroup,
            subgroup,
            ..
        }
        | O::SubgroupDeriveBorrowed {
            workgroup,
            subgroup,
            ..
        } => two(workgroup, subgroup),
        O::LdsAllocate { workgroup, lds, .. } => two(workgroup, lds),
        O::LdsInitializeByInvocation {
            workgroup,
            input_lds,
            output_lds,
            ..
        } => [Some(workgroup), Some(input_lds), Some(output_lds)],
        O::LdsPublish {
            input_workgroup,
            input_lds,
            ..
        } => two(input_workgroup, input_lds),
        O::LdsReadPublished { lds, workgroup, .. } => two(lds, workgroup),
        O::WorkgroupBarrier {
            input_workgroup, ..
        } => one(input_workgroup),
        O::SubgroupBarrier {
            input_workgroup,
            subgroup,
            ..
        } => two(input_workgroup, subgroup),
        O::WorkgroupFence { workgroup, .. } => one(workgroup),
        O::SubgroupFence {
            subgroup, epoch, ..
        }
        | O::SubgroupCollective {
            subgroup, epoch, ..
        } => two(subgroup, epoch),
        O::Atomic { location, .. } => one(location),
        O::WorkgroupCollective {
            input_workgroup,
            scratch,
            ..
        } => two(input_workgroup, scratch),
        O::MatrixAccess {
            subgroup,
            epoch,
            matrix,
            ..
        } => [Some(subgroup), Some(epoch), Some(matrix)],
        O::AsyncCopy {
            workgroup,
            destination,
            pending,
            ..
        } => [Some(workgroup), Some(destination), Some(pending)],
        O::AsyncWait {
            input_workgroup,
            pending,
            ..
        } => two(input_workgroup, pending),
        O::RawMemoryBind {
            view, index_space, ..
        } => [Some(view), index_space, None],
        O::WorkgroupMemoryIndex { workgroup, witness }
        | O::WorkgroupMemoryIndexV2 {
            workgroup, witness, ..
        } => two(workgroup, witness),
        O::WorkgroupMemoryIndexIntoDisjoint {
            input_witness,
            output_witness,
        } => two(input_witness, output_witness),
        O::WorkgroupMemoryAllocate {
            workgroup,
            view,
            index_space,
            ..
        } => [Some(workgroup), Some(view), Some(index_space)],
        O::WorkgroupMemoryPublish {
            input_workgroup,
            input_view,
            ..
        } => two(input_workgroup, input_view),
        O::MemoryLoad {
            view, workgroup, ..
        }
        | O::MemoryStore {
            view, workgroup, ..
        } => [Some(view), workgroup, None],
        O::NumericalPolicyIssue { .. } | O::PrivateMemoryAllocate { .. } => [None; 3],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(index: u32) -> SemanticTypeIdV1 {
        SemanticTypeIdV1::from_index(index)
    }

    #[test]
    fn erased_index_witness_is_an_old_scope_seed_but_loaded_scalar_data_is_not() {
        let index = SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexV2 {
            workgroup_reference: id(0),
            workgroup: id(1),
            option: id(2),
            witness: id(3),
        };
        assert_eq!(old_types(index), [Some(id(1)), Some(id(3)), None]);
        let load = SemanticExecutionCapabilityOperationV1::MemoryLoad {
            view: id(4),
            workgroup: Some(id(1)),
            index: id(5),
            option: id(6),
            element: id(7),
            space:
                fe2o3_mir_model::semantic_mir_v1::SemanticExecutionMemoryAddressSpaceV1::Workgroup,
            access: fe2o3_mir_model::semantic_mir_v1::SemanticExecutionMemoryAccessV1::ReadOnly,
        };
        assert_eq!(old_types(load), [Some(id(4)), Some(id(1)), None]);
        assert!(!old_types(load).contains(&Some(id(7))));
        assert!(!old_types(load).contains(&Some(id(6))));
    }

    #[test]
    fn epoch_transition_outputs_are_not_mislabeled_as_old_capabilities() {
        let publish = SemanticExecutionCapabilityOperationV1::LdsPublish {
            input_workgroup: id(1),
            input_lds: id(2),
            output_lds: id(3),
            transition: id(4),
            element: id(5),
            elements: 64,
        };
        assert_eq!(old_types(publish), [Some(id(1)), Some(id(2)), None]);
        assert!(!old_types(publish).contains(&Some(id(3))));
        assert!(!old_types(publish).contains(&Some(id(4))));
    }
}
