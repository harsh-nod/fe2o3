// Replay the sparse emission trace against the admitted ABI and actual KIR
// signature. This is entry identity, not current SSA provenance or authority.

type ArgumentBudgetV1<'a> = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'a>;
type ArgumentResourceV1 = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1;

impl From<ArgumentResourceV1> for ProductionSemanticKirErrorV1 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::ArgumentCorrespondenceResource(error)
    }
}

struct ArgumentTraceV1<'a> {
    direct: &'a [SemanticKirParameterBindingV1],
    components: &'a [SemanticKirParameterComponentBindingV1],
    ignored: &'a [SemanticKirIgnoredParameterBindingV1],
}

#[derive(Clone, Copy)]
enum PhysicalArgumentTraceV1<'a> {
    Direct(&'a SemanticKirParameterBindingV1),
    Component(&'a SemanticKirParameterComponentBindingV1),
}

impl PhysicalArgumentTraceV1<'_> {
    fn value(self) -> ValueId {
        match self {
            Self::Direct(binding) => binding.kernel_ir_value,
            Self::Component(binding) => binding.kernel_ir_value,
        }
    }
}

struct IndexedArgumentTraceV1<'a> {
    trace: PhysicalArgumentTraceV1<'a>,
    used: bool,
}

// In-place radix partitioning has at most 32 visits and 32 swaps per row,
// independent of the standard library's sorting implementation. The caller
// prepays 96 units per row; recursion is bounded by the u32 identity width.
fn sort_argument_trace_v1(rows: &mut [IndexedArgumentTraceV1<'_>], bit: u32) {
    sort_correspondence_keys_v1(rows, bit, &|row| u64::from(row.trace.value().0));
}

fn sort_correspondence_keys_v1<T>(rows: &mut [T], bit: u32, key: &impl Fn(&T) -> u64) {
    if rows.len() < 2 {
        return;
    }
    let (mut left, mut right) = (0, rows.len());
    while left < right {
        if key(&rows[left]) & (1_u64 << bit) == 0 {
            left += 1;
        } else {
            right -= 1;
            rows.swap(left, right);
        }
    }
    if bit != 0 {
        let (low, high) = rows.split_at_mut(left);
        sort_correspondence_keys_v1(low, bit - 1, key);
        sort_correspondence_keys_v1(high, bit - 1, key);
    }
}

fn argument_sum_v1(values: &[usize]) -> Result<usize, ArgumentResourceV1> {
    values.iter().try_fold(0_usize, |sum, value| {
        sum.checked_add(*value)
            .ok_or(ArgumentResourceV1::Arithmetic)
    })
}

fn argument_product_v1(left: usize, right: usize) -> Result<usize, ArgumentResourceV1> {
    left.checked_mul(right)
        .ok_or(ArgumentResourceV1::Arithmetic)
}

fn argument_vec_v1<T>(length: usize) -> Result<Vec<T>, ArgumentResourceV1> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    Ok(values)
}

fn validate_parameter_correspondence_v1(
    semantic: &AdmittedInertSemanticMirV1,
    instance: &SemanticKirFunctionCorrespondenceV1,
    target: &Function,
    trace: ArgumentTraceV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_parameter_correspondence_v1(semantic, instance, target, trace, budget, |_| Ok(()))
}

fn with_parameter_correspondence_v1<'w, R>(
    semantic: &AdmittedInertSemanticMirV1,
    instance: &SemanticKirFunctionCorrespondenceV1,
    target: &Function,
    trace: ArgumentTraceV1<'_>,
    budget: &mut ArgumentBudgetV1<'w>,
    use_view: impl for<'s> FnOnce(
        &mut ProductionArgumentViewV1<'s, 'w>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let result = check_argument_trace_v1(semantic, instance, target, trace, budget, use_view);
    // All function-local owners have dropped on either Result path. Work and
    // peak history remain cumulative; this scope does not promise unwind cleanup.
    budget.release_storage(budget.storage() - floor)?;
    result
}

fn check_argument_trace_v1<'w, R>(
    semantic: &AdmittedInertSemanticMirV1,
    instance: &SemanticKirFunctionCorrespondenceV1,
    target: &Function,
    trace: ArgumentTraceV1<'_>,
    budget: &mut ArgumentBudgetV1<'w>,
    use_view: impl for<'s> FnOnce(
        &mut ProductionArgumentViewV1<'s, 'w>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    budget.charge_work(1)?;
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
        || target.id != instance.kernel_ir_function
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
                ProductionSemanticKirErrorV1::from(ArgumentResourceV1::Allocation)
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
                match kernel_parameter_shape_v1(
                    semantic,
                    function,
                    mapped.source_argument(),
                    mapped.abi().ty(),
                )? {
                    KernelParameterShapeV1::Direct(ty) => {
                        check(&[], mapped.abi().ty(), &ty)?;
                        true
                    }
                    KernelParameterShapeV1::Components(components) => {
                        for (path, semantic_type, ty, _, _) in &components {
                            check(path, *semantic_type, ty)?;
                        }
                        false
                    }
                }
            }
            SemanticKirFunctionRoleV1::InternalHelper => {
                let (shared_slice, components) = helper_parameter_shape_v1(
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
            }
        };
        shapes.push(AdjustedArgumentShapeV1 {
            first,
            end: slot,
            atomic,
        });
        budget.release_storage(budget.storage() - shape_floor)?;
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
    let mut view = ProductionArgumentViewV1 {
        data: ArgumentViewDataV1 {
            semantic,
            instance,
            target,
            logical: &arguments,
            physical: &physical,
            ignored: &ignored,
            shapes: &shapes,
        },
        budget,
    };
    view.visit_nodes(|_| Ok(()))?;
    use_view(&mut view)
}

fn prepay_argument_shape_v1(
    semantic: &AdmittedInertSemanticMirV1,
    ty: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    prepay_typed_shape_v1(semantic.types(), ty, semantic.callables().len(), budget)
}

fn prepay_typed_shape_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    callable_count: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    // Existing pure shape selection has a 256-node structural cap. Prepay its
    // worst-case path copies, range sorting, vector relocation, and transparent
    // carrier walk before calling it. This deliberately conservative allowance
    // is scratch per adjusted argument, never retained correspondence evidence.
    let shape = types[ty.index() as usize].shape();
    let mut nodes = 0;
    count_argument_shape_nodes_v1(types, ty, &mut nodes, budget)?;
    let fields = match shape {
        SemanticTypeShapeV1::Aggregate(fields) | SemanticTypeShapeV1::Tuple(fields) => {
            fields.fields().len()
        }
        _ => 0,
    };
    let paths = argument_product_v1(nodes, nodes)?;
    budget.charge_work(argument_sum_v1(&[
        argument_product_v1(paths, 8)?,
        argument_product_v1(nodes, 64)?,
        argument_product_v1(callable_count, 4)?,
        argument_product_v1(fields, 8)?,
    ])?)?;
    budget.reserve_storage(argument_sum_v1(&[
        argument_product_v1(
            paths,
            std::mem::size_of::<SemanticKirParameterProjectionV1>()
                + std::mem::size_of::<ProductionArgumentProjectionV1>(),
        )?,
        argument_product_v1(nodes, 512)?,
        2048,
    ])?)?;
    Ok(())
}

// Allocation-free sizing only, not representation or ABI validation. Stop at
// the representation walk's existing cap and retain its conservative quota.
// Following pointer carriers also bounds transparent pointee traversal.
fn count_argument_shape_nodes_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    nodes: &mut usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ArgumentResourceV1> {
    if *nodes == MAX_SSA_VALUE_COMPONENTS_V1 {
        return Ok(());
    }
    budget.charge_work(1)?;
    *nodes += 1;
    match types[ty.index() as usize].shape() {
        SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
            for field in fields.fields() {
                if *nodes == MAX_SSA_VALUE_COMPONENTS_V1 {
                    break;
                }
                count_argument_shape_nodes_v1(types, *field, nodes, budget)?;
            }
        }
        SemanticTypeShapeV1::Array { element, length } => {
            for _ in 0..(*length).min(MAX_SSA_VALUE_COMPONENTS_V1 as u64) {
                if *nodes == MAX_SSA_VALUE_COMPONENTS_V1 {
                    break;
                }
                count_argument_shape_nodes_v1(types, *element, nodes, budget)?;
            }
        }
        SemanticTypeShapeV1::Pointer(pointer) => {
            count_argument_shape_nodes_v1(types, pointer.pointee(), nodes, budget)?
        }
        SemanticTypeShapeV1::Slice { element } => {
            count_argument_shape_nodes_v1(types, *element, nodes, budget)?
        }
        _ => {}
    }
    Ok(())
}
