//! Inert V18 entry correspondence over the actual admitted source and KIR.
//!
//! Descriptor answers are shape proposals, not authenticated profile evidence.
//! This module creates no allocation, currentness, kernel proof or launch permit.
//! The enclosing source consumer must independently authenticate its complete
//! captured profile and perform final physical replay.

use super::*;
use crate::CanonicalAnalysisCleanupV1;

include!("source_shapes_v18.rs");

/// Inert borrowed entry coordinates, not an authenticated source association.
pub struct ArgumentEntryV18<'a> {
    pub correspondence_owner: SemanticFunctionIdV1,
    pub semantic_function: SemanticFunctionIdV1,
    pub kernel_ir_function: &'a FunctionId,
    pub role: SemanticKirFunctionRoleV1,
}

/// A narrow inert proposal for a checker-selected top-level source argument.
/// True changes only a direct semantic slice's Generic/Global representation;
/// it does not authenticate a descriptor, source owner or final memory proof.
/// A matching forged proposal can describe inert correspondence but cannot
/// substitute for the enclosing consumer's genuine captured-profile replay.
pub type DescriptorSliceProposalV18<'query, 'work> = dyn FnMut(
        u32,
        SemanticTypeIdV1,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<bool, ProductionSourceArgumentErrorV1>
    + 'query;

/// Borrowed checked correspondence data, never captured-ABI or launch authority.
/// Construction validates actual source/trace/signature data. Every query also
/// checks its original budget slot, ledger and complete live retained floor.
/// Nodes, paths and physical coverage may not escape their scoped callback.
///
/// ```
/// use fe2o3_pliron::source_argument_v1::{ArgumentBudgetV1, ProductionSourceArgumentErrorV1};
/// use fe2o3_pliron::source_argument_v1::scoped_v18::ProductionArgumentViewV18;
/// fn count(view: &ProductionArgumentViewV18<'_>, budget: &mut ArgumentBudgetV1<'_>)
///     -> Result<usize, ProductionSourceArgumentErrorV1> {
///     let mut count = 0;
///     view.visit_nodes_with_budget(budget, |_, _| { count += 1; Ok(()) })?;
///     Ok(count)
/// }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::source_argument_v1::scoped_v18::ProductionArgumentViewV18;
/// fn forge(view: &ProductionArgumentViewV18<'_>) {
///     let _ = ProductionArgumentViewV18 { data: view.data };
/// }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::source_argument_v1::{ArgumentBudgetV1, ProductionSourceArgumentErrorV1};
/// use fe2o3_pliron::source_argument_v1::scoped_v18::ProductionArgumentViewV18;
/// fn escape(view: &ProductionArgumentViewV18<'_>, budget: &mut ArgumentBudgetV1<'_>) {
///     let mut saved = None;
///     view.visit_nodes_with_budget(budget, |node, _| { saved = Some(node); Ok(()) }).unwrap();
///     assert!(saved.is_some());
/// }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::source_argument_v1::ArgumentBudgetV1;
/// use fe2o3_pliron::source_argument_v1::scoped_v18::ProductionArgumentViewV18;
/// fn escape_path(view: &ProductionArgumentViewV18<'_>, budget: &mut ArgumentBudgetV1<'_>) {
///     let mut saved = None;
///     view.visit_nodes_with_budget(budget, |node, _| { saved = Some(node.source_path()); Ok(()) }).unwrap();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::source_argument_v1::ArgumentBudgetV1;
/// use fe2o3_pliron::source_argument_v1::scoped_v18::ProductionArgumentViewV18;
/// fn escape_coverage(view: &ProductionArgumentViewV18<'_>, budget: &mut ArgumentBudgetV1<'_>) {
///     let mut saved = None;
///     view.visit_nodes_with_budget(budget, |node, _| { saved = Some(node.coverage()); Ok(()) }).unwrap();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::source_argument_v1::scoped_v18::ProductionArgumentViewV18;
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18;
/// fn no_proof(view: ProductionArgumentViewV18<'_>) -> VerifiedCanonicalKernelIrModuleV18 {
///     view.into()
/// }
/// ```
pub struct ProductionArgumentViewV18<'scope> {
    data: ArgumentViewDataV18<'scope>,
}

impl<'scope> ProductionArgumentViewV18<'scope> {
    /// Rechecks this inert view's budget slot, ledger and live owner floor.
    /// This does not authenticate a source profile or grant execution authority,
    /// and neither charges work nor reserves or refunds storage.
    pub fn check_custody(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSourceArgumentErrorV1> {
        self.data.check(budget)
    }

    /// Shortens an inert borrow; each subsequent metered query rechecks custody.
    pub fn reborrow(&self) -> ProductionArgumentViewV18<'_> {
        ProductionArgumentViewV18 { data: self.data }
    }

    /// Returns the original semantic function locator of this checked inert view.
    /// This number is not an owner, authenticated profile, or execution authority.
    pub fn semantic_function(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SemanticFunctionIdV1, ProductionSourceArgumentErrorV1> {
        self.data.check(budget)?;
        budget.charge_work(1)?;
        Ok(self.data.semantic_function)
    }

    pub fn physical(
        &self,
        slot: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<ProductionPhysicalArgumentV1<'scope>>, ProductionSourceArgumentErrorV1> {
        self.data.physical(slot, budget)
    }

    pub fn visit_nodes_with_budget<'work>(
        &self,
        budget: &mut ArgumentBudgetV1<'work>,
        visit: impl for<'node, 'borrow> FnMut(
            ProductionArgumentNodeV1<'node>,
            &'borrow mut ArgumentBudgetV1<'work>,
        ) -> Result<(), ProductionSourceArgumentErrorV1>,
    ) -> Result<(), ProductionSourceArgumentErrorV1> {
        self.data.visit_nodes_scoped(budget, visit)
    }
}

include!("scoped_v18_view.rs");
include!("scoped_v18_cleanup.rs");

#[cfg(test)]
#[path = "scoped_v18_tests.rs"]
mod tests;

/// Replays inert trace and shape proposals against the actual source and body.
/// The returned scope is not an authenticated captured descriptor or a proof.
/// The HRTB callback cannot return its view, nodes or original budget borrow.
/// Selected errors and raw panics retain precedence; observed custody loss
/// permanently denies all linked enclosing refunds.

/// ```compile_fail
/// use fe2o3_pliron::source_argument_v1::{ArgumentBudgetV1, ArgumentTraceV1};
/// use fe2o3_pliron::source_argument_v1::scoped_v18::{ArgumentEntryV18, with_parameter_correspondence_v18};
/// use fe2o3_pliron::CanonicalAnalysisCleanupV1;
/// use fe2o3_kernel_ir::Function;
/// use fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1;
/// fn escape(source: &AdmittedInertSemanticMirV1, entry: ArgumentEntryV18<'_>, target: &Function,
///     rows: ArgumentTraceV1<'_>, cleanup: &CanonicalAnalysisCleanupV1<'_>, budget: &mut ArgumentBudgetV1<'_>) {
///     let view = with_parameter_correspondence_v18(source, entry, target, rows, cleanup,
///         budget, None, |view, _| Ok(view)).unwrap();
///     let _ = view.reborrow();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::source_argument_v1::{ArgumentBudgetV1, ArgumentTraceV1};
/// use fe2o3_pliron::source_argument_v1::scoped_v18::{ArgumentEntryV18, with_parameter_correspondence_v18};
/// use fe2o3_pliron::CanonicalAnalysisCleanupV1;
/// use fe2o3_kernel_ir::Function;
/// use fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1;
/// fn alias(source: &AdmittedInertSemanticMirV1, entry: ArgumentEntryV18<'_>, target: &Function,
///     rows: ArgumentTraceV1<'_>, cleanup: &CanonicalAnalysisCleanupV1<'_>, budget: &mut ArgumentBudgetV1<'_>) {
///     with_parameter_correspondence_v18(source, entry, target, rows, cleanup, budget, None,
///         |_, _| { budget.release_storage(1)?; Ok(()) }).unwrap();
/// }
/// ```
pub fn with_parameter_correspondence_v18<'w, R>(
    semantic: &AdmittedInertSemanticMirV1,
    entry: ArgumentEntryV18<'_>,
    target: &Function,
    trace: ArgumentTraceV1<'_>,
    cleanup: &CanonicalAnalysisCleanupV1<'_>,
    budget: &mut ArgumentBudgetV1<'w>,
    descriptor_slice: Option<&mut DescriptorSliceProposalV18<'_, 'w>>,
    use_data: impl for<'s> FnOnce(
        ProductionArgumentViewV18<'s>,
        &'s mut ArgumentBudgetV1<'w>,
    ) -> Result<R, ProductionSourceArgumentErrorV1>,
) -> Result<R, ProductionSourceArgumentErrorV1> {
    budget.charge_work(2)?;
    with_argument_scratch_v18(cleanup, budget, |budget| {
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<ArgumentEntryV18<'_>>(),
            std::mem::size_of::<ArgumentQueryCustodyV18>(),
        ])?)?;
        check_argument_trace_v18(
            semantic,
            entry,
            target,
            trace,
            cleanup,
            budget,
            descriptor_slice,
            use_data,
        )
    })
}

fn check_argument_trace_v18<'w, R>(
    semantic: &AdmittedInertSemanticMirV1,
    instance: ArgumentEntryV18<'_>,
    target: &Function,
    trace: ArgumentTraceV1<'_>,
    cleanup: &CanonicalAnalysisCleanupV1<'_>,
    budget: &mut ArgumentBudgetV1<'w>,
    mut descriptor_slice: Option<&mut DescriptorSliceProposalV18<'_, 'w>>,
    use_data: impl for<'s> FnOnce(
        ProductionArgumentViewV18<'s>,
        &'s mut ArgumentBudgetV1<'w>,
    ) -> Result<R, ProductionSourceArgumentErrorV1>,
) -> Result<R, ProductionSourceArgumentErrorV1> {
    let mismatch = || ProductionSourceArgumentErrorV1::CorrespondenceMismatch;
    budget.charge_work(1)?;
    if descriptor_slice.is_some() && instance.role != SemanticKirFunctionRoleV1::KernelEntry {
        return Err(mismatch());
    }
    let function = semantic
        .functions()
        .get(instance.semantic_function.index() as usize)
        .ok_or_else(mismatch)?;
    let body = target.body.as_ref().ok_or_else(mismatch)?;
    let abi = function.abi();
    check_argument_function_abi_v1(function, instance.semantic_function, instance.role)?;
    let count = argument_sum_v1(&[trace.direct.len(), trace.components.len()])?;
    if count != body.parameters.len()
        || count != target.signature.parameters.len()
        || &target.id != instance.kernel_ir_function
    {
        return Err(mismatch());
    }
    let locals = function.locals().len();
    let sources = abi.source_input_types().len();
    let expanded =
        if abi.extern_abi() == fe2o3_mir_model::semantic_mir_v1::SemanticExternAbiV1::RustCall {
            abi.adjusted_arguments()
                .len()
                .checked_sub(abi.fixed_count() as usize)
                .ok_or_else(mismatch)?
        } else {
            0
        };
    // Logical requested payload, excluding allocator excess and borrowed owners.
    // Packed RustCall conservatively reserves the expanded-map upper bound too.
    let storage = argument_sum_v1(&[
        argument_product_v1(sources, std::mem::size_of::<Option<SemanticLocalIdV1>>())?,
        argument_product_v1(expanded, std::mem::size_of::<SemanticLocalIdV1>())?,
        argument_product_v1(
            locals,
            std::mem::size_of::<Option<&SemanticKirIgnoredParameterBindingV1>>()
                + std::mem::size_of::<bool>(),
        )?,
        argument_product_v1(count, std::mem::size_of::<IndexedArgumentTraceV1<'_>>())?,
        argument_product_v1(
            abi.adjusted_arguments().len(),
            std::mem::size_of::<AdjustedArgumentShapeV1>(),
        )?,
    ])?;
    budget.charge_work(argument_sum_v1(&[
        argument_product_v1(locals, 4)?,
        sources,
        expanded,
        argument_product_v1(count, 100)?,
        trace.ignored.len(),
    ])?)?;
    budget.reserve_storage(storage)?;
    let arguments = semantic
        .logical_arguments_v1(instance.semantic_function)
        .map_err(|error| match error {
            fe2o3_mir_model::SemanticLogicalArgumentErrorV1::AllocationFailure => {
                ProductionSourceArgumentErrorV1::from(ArgumentResourceV1::Allocation)
            }
            fe2o3_mir_model::SemanticLogicalArgumentErrorV1::UnknownFunction => mismatch(),
        })?;
    let mut physical_locals = argument_vec_v1(locals)?;
    physical_locals.resize(locals, false);
    let mut ignored = argument_vec_v1(locals)?;
    ignored.resize(locals, None);
    let mut physical = argument_vec_v1(count)?;
    let mut shapes = argument_vec_v1(abi.adjusted_arguments().len())?;
    let exact_local = |owner, function_id, local: SemanticLocalIdV1| {
        owner == instance.correspondence_owner
            && function_id == instance.semantic_function
            && function
                .locals()
                .get(local.index() as usize)
                .is_some_and(|local| local.role().is_entry_argument())
    };
    for binding in trace.direct {
        if !exact_local(
            binding.correspondence_owner,
            binding.semantic_function,
            binding.semantic_local,
        ) {
            return Err(mismatch());
        }
        physical.push(IndexedArgumentTraceV1 {
            trace: PhysicalArgumentTraceV1::Direct(binding),
            used: false,
        });
    }
    for binding in trace.components {
        if !exact_local(
            binding.correspondence_owner,
            binding.semantic_function,
            binding.semantic_local,
        ) || binding.projection.is_empty()
        {
            return Err(mismatch());
        }
        physical.push(IndexedArgumentTraceV1 {
            trace: PhysicalArgumentTraceV1::Component(binding),
            used: false,
        });
    }
    for binding in trace.ignored {
        if !exact_local(
            binding.correspondence_owner,
            binding.semantic_function,
            binding.semantic_local,
        ) {
            return Err(mismatch());
        }
        let local = binding.semantic_local.index() as usize;
        if binding.semantic_type != function.locals()[local].ty()
            || ignored[local].replace(binding).is_some()
        {
            return Err(mismatch());
        }
    }
    sort_argument_trace_v1(&mut physical, 31);
    if physical
        .windows(2)
        .any(|rows| rows[0].trace.value() == rows[1].trace.value())
    {
        return Err(mismatch());
    }
    for argument in arguments.source_arguments() {
        budget.charge_work(1)?;
        if matches!(
            argument.binding(),
            fe2o3_mir_model::SemanticSourceArgumentBindingV1::ExpandedTuple([])
        ) && argument.source_ownership() != SemanticSourceArgumentOwnershipV1::ByValue
        {
            return Err(mismatch());
        }
    }
    let mut slot = 0;
    for mapped in arguments.adjusted_arguments() {
        let shape_floor = budget.storage();
        let first = slot;
        prepay_argument_shape_v1(semantic, mapped.abi().ty(), budget)?;
        let global_descriptor = match descriptor_slice.as_deref_mut() {
            Some(propose) => {
                let floor = budget.storage();
                argument_attempt_v18(cleanup, budget, floor, |budget| {
                    propose(mapped.source_argument(), mapped.abi().ty(), budget)
                })?
            }
            None => false,
        };
        if global_descriptor
            && (!shared_slice_leaf_v1(semantic.types(), mapped.abi().ty())
                || abi
                    .source_argument_ownership()
                    .get(mapped.source_argument() as usize)
                    != Some(&SemanticSourceArgumentOwnershipV1::SharedBorrow))
        {
            return Err(mismatch());
        }
        let mut check = |path: &[SemanticKirParameterProjectionV1], semantic_type, ty: &Type| {
            budget.charge_work(argument_sum_v1(&[40, path.len()])?)?;
            let value = *body.parameters.get(slot).ok_or_else(mismatch)?;
            if target.signature.parameters.get(slot) != Some(ty) {
                return Err(mismatch());
            }
            let index = physical
                .binary_search_by_key(&value, |row| row.trace.value())
                .map_err(|_| mismatch())?;
            let row = &mut physical[index];
            if row.used {
                return Err(mismatch());
            }
            let expected = mapped
                .local_field()
                .map(SemanticKirParameterProjectionV1::Field)
                .into_iter()
                .chain(path.iter().copied());
            let exact = match row.trace {
                PhysicalArgumentTraceV1::Direct(binding) => {
                    mapped.local_field().is_none()
                        && path.is_empty()
                        && binding.semantic_local == mapped.local()
                }
                PhysicalArgumentTraceV1::Component(binding) => {
                    binding.semantic_local == mapped.local()
                        && binding.semantic_component_type == semantic_type
                        && binding.projection.len()
                            == path.len() + usize::from(mapped.local_field().is_some())
                        && binding.projection.iter().copied().eq(expected)
                }
            };
            if !exact {
                return Err(mismatch());
            }
            physical_locals[mapped.local().index() as usize] = true;
            row.used = true;
            slot += 1;
            Ok(())
        };
        let atomic = match instance.role {
            SemanticKirFunctionRoleV1::KernelEntry => {
                if mapped.tuple_field().is_some() {
                    return Err(mismatch());
                }
                match source_kernel_parameter_shape_v18(
                    semantic,
                    function,
                    mapped.source_argument(),
                    mapped.abi().ty(),
                )? {
                    KernelParameterShapeV1::Direct(mut ty) => {
                        if global_descriptor {
                            let Type::Slice(slice) = &mut ty else {
                                return Err(mismatch());
                            };
                            slice.address_space = AddressSpace::Global;
                        }
                        check(&[], mapped.abi().ty(), &ty)?;
                        true
                    }
                    KernelParameterShapeV1::Components(components) => {
                        if global_descriptor {
                            return Err(mismatch());
                        }
                        for (path, semantic_type, ty, _, _) in &components {
                            check(path, *semantic_type, ty)?;
                        }
                        false
                    }
                }
            }
            SemanticKirFunctionRoleV1::InternalHelper => {
                let (shared_slice, components) = source_helper_parameter_shape_v18(
                    semantic.types(),
                    function,
                    instance.semantic_function,
                    mapped,
                )?;
                for (path, semantic_type, ty) in &components {
                    check(path, *semantic_type, ty)?;
                }
                shared_slice
                    || matches!(
                        semantic.types()[mapped.abi().ty().index() as usize].shape(),
                        SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
                    )
                    || matches!(
                        semantic.types()[mapped.abi().ty().index() as usize].shape(),
                        SemanticTypeShapeV1::Pointer(pointer)
                            // The exact helper shape above already validated
                            // ownership, unadjusted ABI, width and access mode.
                            // A thin reference is one leaf, not an aggregate.
                            if matches!(pointer.kind(),
                                SemanticPointerKindV1::Raw | SemanticPointerKindV1::Reference)
                                && pointer.metadata() == SemanticPointerMetadataV1::None
                    )
            }
        };
        shapes.push(AdjustedArgumentShapeV1 {
            first,
            end: slot,
            atomic,
            policy: match instance.role {
                // Match exact original ByValue component lowering; nested
                // shared slices retain Generic representation, never Global.
                SemanticKirFunctionRoleV1::KernelEntry => ParameterLeafPolicyV1::SharedSliceLeaves,
                SemanticKirFunctionRoleV1::InternalHelper => {
                    ParameterLeafPolicyV1::SharedSliceLeaves
                }
            },
        });
        budget.release_storage(
            budget
                .storage()
                .checked_sub(shape_floor)
                .ok_or(ArgumentResourceV1::Accounting)?,
        )?;
    }
    budget.charge_work(argument_sum_v1(&[locals, count])?)?;
    for (index, local) in function.locals().iter().enumerate() {
        if !local.role().is_entry_argument() {
            continue;
        }
        if physical_locals[index] {
            if ignored[index].is_some() {
                return Err(mismatch());
            }
        } else {
            let source = match local.role() {
                SemanticLocalRoleV1::Argument(source)
                | SemanticLocalRoleV1::RustCallTupleField {
                    argument: source, ..
                } => source,
                _ => return Err(mismatch()),
            };
            if ignored[index].is_none()
                || semantic.types()[local.ty().index() as usize]
                    .layout()
                    .size_bytes()
                    != Some(0)
                || abi.source_argument_ownership().get(source as usize)
                    != Some(&SemanticSourceArgumentOwnershipV1::ByValue)
            {
                return Err(mismatch());
            }
        }
    }
    if slot != count || physical.iter().any(|row| !row.used) {
        return Err(mismatch());
    }
    let data = ArgumentViewDataV18 {
        semantic,
        semantic_function: instance.semantic_function,
        target,
        logical: &arguments,
        physical: &physical,
        ignored: &ignored,
        shapes: &shapes,
        cleanup,
        custody: ArgumentQueryCustodyV18 {
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
        },
    };
    data.visit_nodes_scoped(budget, |_, _| Ok(()))?;
    let floor = budget.storage();
    argument_attempt_v18(cleanup, budget, floor, |budget| {
        use_data(ProductionArgumentViewV18 { data }, budget)
    })
}
