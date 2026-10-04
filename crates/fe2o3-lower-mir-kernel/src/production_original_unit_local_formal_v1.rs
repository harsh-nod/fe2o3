//! Exact original-N call gaps, discharged only inside live source deletion custody.
use super::super::{
    CheckedUnitLocalCallDeletionV1, GuardedAddressProofBudgetV1,
    ProductionUnitLocalOperationDeletionV1, ProductionUnitLocalRankedRootV1,
    canonical_call_inventory_error_v1, guarded_accesses_have_structural_bounds_result,
};
use super::{Budget, E, Graph, Resource};
use crate::ProductionFormalMemoryErrorV1 as FormalError;
use fe2o3_kernel_ir::{
    ExplicitLaunchExtent, FormalIndexWidth, FormalMemoryIncompleteReason as Reason,
    FormalMemoryObligationAnalysis as Analysis, FormalMemoryObligations, FunctionId,
    FunctionOperationLocation, Kernel, OperationKind, derive_kernel_memory_obligations_for_launch,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticTerminatorKindV1;

#[path = "production_original_unit_local_index_v1.rs"]
mod index;
use index::Index;

fn source_error(error: super::super::ProductionSemanticKirErrorV1) -> E {
    E::Source(Box::new(error))
}

fn discharge_call(
    original: &Graph,
    index: &Index<'_, '_>,
    root: &ProductionUnitLocalRankedRootV1<'_>,
    kernel: &Kernel,
    location: FunctionOperationLocation,
    callee: &FunctionId,
    budget: &mut Budget<'_>,
) -> Result<bool, E> {
    budget.charge_work(12)?;
    let deletion = index.deletion;
    let inventory = deletion.stage.source.inventory;
    if !std::ptr::eq(deletion.source().executable(), original) || !inventory.belongs_to(original) {
        return Err(E::CallRelation("original owner"));
    }
    let entry = inventory
        .function_for_name(kernel.entry.as_str(), budget)
        .map_err(canonical_call_inventory_error_v1)
        .map_err(source_error)?
        .ok_or(E::CallRelation("original entry"))?;
    let Some(block) = inventory
        .block_for_id(entry.coordinate, location.block, budget)
        .map_err(canonical_call_inventory_error_v1)
        .map_err(source_error)?
    else {
        return Ok(false);
    };
    if location.operation_index >= block.operations.len() {
        return Ok(false);
    }
    let operation_ordinal = block
        .operations
        .start
        .checked_add(location.operation_index)
        .ok_or(Resource::Arithmetic)?;
    let actual = inventory
        .operations()
        .get(operation_ordinal)
        .ok_or(E::CallRelation("original operation"))?;
    budget.charge_work(callee.as_str().len())?;
    if !matches!(&actual.operation.kind, OperationKind::Call { callee: target, arguments }
        if target == callee && arguments.is_empty())
        || !actual.operation.results.is_empty()
    {
        return Ok(false);
    }
    if deletion
        .operation(actual.coordinate, budget)
        .map_err(source_error)?
        != Some(ProductionUnitLocalOperationDeletionV1::DeletedUnitCall)
    {
        return Ok(false);
    }
    let Some(local) = index.call(root, actual.coordinate, budget)? else {
        return Ok(false);
    };
    let token = root
        .local_calls()
        .get(local)
        .ok_or(E::CallRelation("original indexed call"))?;
    budget.charge_work(8)?;
    if token.root() != root.selected_root()
        || token.caller() != root.selected_root()
        || !std::ptr::eq(token.operation(), actual.operation)
    {
        return Err(E::CallRelation("original root/call occurrence"));
    }
    let semantic = deletion.source().semantic_ssa.source_semantic();
    let caller = semantic
        .functions()
        .get(token.caller().index() as usize)
        .ok_or(E::CallRelation("original caller"))?;
    let source_block = caller
        .blocks()
        .get(token.source_block().index() as usize)
        .ok_or(E::CallRelation("original source block"))?;
    let SemanticTerminatorKindV1::Call(call) = source_block.terminator().kind() else {
        return Err(E::CallRelation("original source call"));
    };
    if !std::ptr::eq(call, token.source_call()) {
        return Err(E::CallRelation("original source call owner"));
    }
    Ok(true)
}

pub(super) fn derive(
    original: &Graph,
    deletion: &CheckedUnitLocalCallDeletionV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<Box<[FormalMemoryObligations]>, E> {
    budget.charge_work(3)?;
    if !std::ptr::eq(original, deletion.source().executable())
        || original.module().kernels.len() != deletion.stage.root_count()
    {
        return Err(E::CallRelation("original owner/roster"));
    }
    let max_operations = deletion.source().limits.max_operations;
    let index = Index::new(deletion, budget)?;
    let mut reports = Vec::with_capacity(original.module().kernels.len());
    for kernel in &original.module().kernels {
        let root = index.root(kernel, budget)?;
        let extents = crate::production_formal_memory_v1::witness_extents(&kernel.domain);
        let analysis = derive_kernel_memory_obligations_for_launch(
            original.module(),
            &kernel.id,
            ExplicitLaunchExtent::Exact {
                rank: kernel.domain.rank(),
                extents,
            },
            FormalIndexWidth::Bits64,
        )
        .map_err(|error| E::Formal(Box::new(FormalError::Analysis(error))))?;
        let report = match analysis {
            Analysis::Complete(report) => report,
            Analysis::Incomplete { partial, reasons } => {
                if reasons.is_empty() {
                    return Err(E::Formal(Box::new(FormalError::Incomplete {
                        reasons: reasons.into_boxed_slice(),
                    })));
                }
                let mut allowed = true;
                for reason in &reasons {
                    budget.charge_work(1)?;
                    allowed &= match reason {
                        Reason::GuardedAccessRequiresRankedProof { .. } => true,
                        Reason::CallEffectsUnavailable { location, callee } => discharge_call(
                            original, &index, &root, kernel, *location, callee, budget,
                        )?,
                        _ => false,
                    };
                }
                if !allowed {
                    return Err(E::Formal(Box::new(FormalError::Incomplete {
                        reasons: reasons.into_boxed_slice(),
                    })));
                }
                // This is the existing guarded-read rule, not an additional exception.
                let locations = reasons
                    .iter()
                    .filter_map(|reason| match reason {
                        Reason::GuardedAccessRequiresRankedProof { location } => Some(*location),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                if !locations.is_empty() {
                    let guarded =
                        GuardedAddressProofBudgetV1::new(max_operations).and_then(|mut guard| {
                            guarded_accesses_have_structural_bounds_result(
                                original.module(),
                                kernel,
                                &partial,
                                extents,
                                &locations,
                                max_operations,
                                &mut guard,
                            )
                        });
                    if let Err(detail) = guarded {
                        return Err(E::Formal(Box::new(FormalError::GuardedAccessDischarge {
                            reasons: reasons.into_boxed_slice(),
                            detail,
                        })));
                    }
                }
                partial
            }
        };
        if !report.inter_invocation_conflicts().is_empty() {
            return Err(E::Formal(Box::new(FormalError::InterInvocationConflicts {
                conflicts: report
                    .inter_invocation_conflicts()
                    .to_vec()
                    .into_boxed_slice(),
            })));
        }
        reports.push(report);
    }
    Ok(reports.into_boxed_slice())
}

#[cfg(test)]
pub(in crate::production_semantic_kir_v1) fn exercise_call_joins_v1(
    source: &super::super::ProductionUnitLocalErasedSourceOwnerV1,
    foreign: &Graph,
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    source
        .with_checked_erasure_v1(budget, |deletion, budget| {
            super::scoped(budget, |budget| {
                let index = Index::new(deletion, budget)?;
                let original = deletion.source().executable();
                assert_eq!(original.module().kernels.len(), 2);
                let kernel = &original.module().kernels[0];
                let root = index.root(kernel, budget).unwrap();
                let other = index.root(&original.module().kernels[1], budget).unwrap();
                let Analysis::Incomplete { reasons, .. } =
                    derive_kernel_memory_obligations_for_launch(
                        original.module(),
                        &kernel.id,
                        ExplicitLaunchExtent::Exact {
                            rank: kernel.domain.rank(),
                            extents: crate::production_formal_memory_v1::witness_extents(
                                &kernel.domain,
                            ),
                        },
                        FormalIndexWidth::Bits64,
                    )
                    .unwrap()
                else {
                    return Err(E::CallRelation("real original private-call gap"));
                };
                let (location, callee) = reasons
                    .iter()
                    .find_map(|reason| match reason {
                        Reason::CallEffectsUnavailable { location, callee } => {
                            Some((*location, callee))
                        }
                        _ => None,
                    })
                    .unwrap();
                assert!(
                    discharge_call(original, &index, &root, kernel, location, callee, budget)
                        .unwrap()
                );
                assert!(
                    !discharge_call(original, &index, &other, kernel, location, callee, budget)
                        .unwrap()
                );
                assert!(
                    !discharge_call(
                        original,
                        &index,
                        &root,
                        kernel,
                        location,
                        &kernel.entry,
                        budget
                    )
                    .unwrap()
                );
                for wrong in [
                    FunctionOperationLocation {
                        block: fe2o3_kernel_ir::BlockId(u32::MAX),
                        ..location
                    },
                    FunctionOperationLocation {
                        operation_index: usize::MAX,
                        ..location
                    },
                ] {
                    assert!(
                        !discharge_call(original, &index, &root, kernel, wrong, callee, budget)
                            .unwrap()
                    );
                }
                assert!(matches!(
                    discharge_call(foreign, &index, &root, kernel, location, callee, budget),
                    Err(E::CallRelation("original owner"))
                ));
                // A real noncall at a valid coordinate cannot borrow the call's token.
                let body = original
                    .module()
                    .function(&kernel.entry)
                    .unwrap()
                    .body
                    .as_ref()
                    .unwrap();
                let noncall =
                    body.blocks
                        .iter()
                        .find_map(|block| {
                            block.operations.iter().enumerate().find_map(
                                |(operation_index, operation)| {
                                    (!matches!(&operation.kind, OperationKind::Call { .. }))
                                        .then_some(FunctionOperationLocation {
                                            block: block.id,
                                            operation_index,
                                        })
                                },
                            )
                        })
                        .unwrap();
                assert!(
                    !discharge_call(original, &index, &root, kernel, noncall, callee, budget)
                        .unwrap()
                );
                Ok(())
            })
            .unwrap();
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), floor);
}
