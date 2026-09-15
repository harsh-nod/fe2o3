#[derive(Clone)]
enum HelperResultTransportV1 {
    Legacy(Vec<Type>),
    Ordinary(std::rc::Rc<OrdinaryHelperResultPlanV1>),
}

struct OrdinaryHelperResultPlanV1 {
    // One immutable payload is shared by planning, signature maps and call sites.
    // The fixed Rc payload/header is one logical analysis row, not a byte receipt.
    types: Vec<Type>,
    source: OrdinaryHelperResultSourceV1,
}

#[derive(Clone, Copy)]
struct OrdinaryHelperResultSourceV1 {
    ty: SemanticTypeIdV1,
    layout: fe2o3_mir_model::semantic_mir_v1::SemanticLayoutIdentityV1,
    abi: fe2o3_mir_model::semantic_mir_v1::SemanticAbiIdentityV1,
}

impl HelperResultTransportV1 {
    fn legacy(types: Vec<Type>) -> Self {
        Self::Legacy(types)
    }

    fn types(&self) -> &[Type] {
        match self {
            Self::Legacy(types) => types,
            Self::Ordinary(plan) => &plan.types,
        }
    }

    fn ordinary(&self) -> Option<OrdinaryHelperResultSourceV1> {
        match self {
            Self::Legacy(_) => None,
            Self::Ordinary(plan) => Some(plan.source),
        }
    }
}

#[derive(Clone, Copy)]
struct OrdinaryHelperResultComponentV1 {
    semantic_type: SemanticTypeIdV1,
    offset: u64,
    scalar: SemanticBackendScalarV1,
}

fn ordinary_helper_result_error_v1(detail: &'static str) -> ProductionSemanticKirErrorV1 {
    unsupported(0, None, None, detail)
}

fn charge_ordinary_helper_result_work_v1(
    work: &mut fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1,
    amount: usize,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if let Some(actual) = work.failed_work() {
        return Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            actual,
            limit: work.limit(),
        });
    }
    work.charge_work(amount)
        .map_err(|error| ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            actual: error.actual(),
            limit: error.limit(),
        })
}

struct OrdinaryHelperResultWalkV1<'a> {
    work: &'a mut dyn FnMut(usize) -> Result<(), ProductionSemanticKirErrorV1>,
    nodes: usize,
}

fn walk_ordinary_helper_result_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    offset: u64,
    walk: &mut OrdinaryHelperResultWalkV1<'_>,
    leaf: &mut dyn FnMut(
        OrdinaryHelperResultComponentV1,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    (walk.work)(1)?;
    walk.nodes += 1;
    if walk.nodes > MAX_SSA_VALUE_COMPONENTS_V1 {
        return Err(ordinary_helper_result_error_v1(
            "ordinary helper result exceeds the structural limit",
        ));
    }
    let declaration = types
        .get(ty.index() as usize)
        .ok_or_else(|| ordinary_helper_result_error_v1("ordinary helper result type is missing"))?;
    let layout = declaration.layout();
    let size = layout
        .size_bytes()
        .filter(|_| !layout.is_uninhabited())
        .ok_or_else(|| {
            ordinary_helper_result_error_v1("ordinary helper result is unsized or uninhabited")
        })?;
    if declaration.abi_properties().first_pointee().is_some()
        || declaration.abi_properties().second_pointee().is_some()
        || declaration.abi_properties().has_unsized_foreign_tail()
    {
        return Err(ordinary_helper_result_error_v1(
            "ordinary helper result has pointee or foreign-tail evidence",
        ));
    }
    match declaration.shape() {
        SemanticTypeShapeV1::Unit if size == 0 => Ok(()),
        SemanticTypeShapeV1::Scalar(_) => {
            let SemanticBackendReprV1::Scalar(scalar) = layout.backend_repr() else {
                return Err(ordinary_helper_result_error_v1(
                    "ordinary helper result leaf lacks exact scalar layout",
                ));
            };
            if scalar.primitive().size_bytes() != Some(size)
                || matches!(
                    scalar.primitive(),
                    SemanticBackendPrimitiveV1::Pointer { .. }
                )
            {
                return Err(ordinary_helper_result_error_v1(
                    "ordinary helper result scalar layout changed",
                ));
            }
            lower_scalar_type(types, ty)?;
            leaf(OrdinaryHelperResultComponentV1 {
                semantic_type: ty,
                offset,
                scalar: *scalar,
            })
        }
        SemanticTypeShapeV1::Array { element, length } => {
            let SemanticFieldsShapeV1::Array {
                stride_bytes,
                count,
            } = layout.fields()
            else {
                return Err(ordinary_helper_result_error_v1(
                    "ordinary helper result array has no exact stride",
                ));
            };
            if count != length
                || *length > MAX_SSA_VALUE_COMPONENTS_V1 as u64
                || stride_bytes.checked_mul(*length) != Some(size)
            {
                return Err(ordinary_helper_result_error_v1(
                    "ordinary helper result array extent changed",
                ));
            }
            let element_size = types
                .get(element.index() as usize)
                .and_then(|element| element.layout().size_bytes())
                .ok_or_else(|| {
                    ordinary_helper_result_error_v1(
                        "ordinary helper result array element is unsized",
                    )
                })?;
            if element_size > *stride_bytes {
                return Err(ordinary_helper_result_error_v1(
                    "ordinary helper result array elements overlap",
                ));
            }
            if *length == 0 {
                return walk_ordinary_helper_result_v1(types, *element, offset, walk, &mut |_| {
                    Ok(())
                });
            }
            for index in 0..*length {
                let next = stride_bytes
                    .checked_mul(index)
                    .and_then(|next| offset.checked_add(next))
                    .ok_or_else(|| {
                        ordinary_helper_result_error_v1("ordinary helper result offset overflows")
                    })?;
                walk_ordinary_helper_result_v1(types, *element, next, walk, leaf)?;
            }
            Ok(())
        }
        SemanticTypeShapeV1::Tuple(fields) => {
            let SemanticTypeLayoutDetailsV1::Aggregate(aggregate) = layout.details() else {
                return Err(ordinary_helper_result_error_v1(
                    "ordinary helper result tuple has no exact offsets",
                ));
            };
            if fields.fields().len() != aggregate.field_offsets().len()
                || fields.fields().len() > MAX_SSA_VALUE_COMPONENTS_V1
            {
                return Err(ordinary_helper_result_error_v1(
                    "ordinary helper result tuple field count changed",
                ));
            }
            for (field, relative) in fields.fields().iter().zip(aggregate.field_offsets()) {
                let field_size = types
                    .get(field.index() as usize)
                    .and_then(|field| field.layout().size_bytes())
                    .ok_or_else(|| {
                        ordinary_helper_result_error_v1(
                            "ordinary helper result tuple field is unsized",
                        )
                    })?;
                if relative
                    .checked_add(field_size)
                    .is_none_or(|end| end > size)
                {
                    return Err(ordinary_helper_result_error_v1(
                        "ordinary helper result tuple field exceeds its layout",
                    ));
                }
                let next = offset.checked_add(*relative).ok_or_else(|| {
                    ordinary_helper_result_error_v1("ordinary helper result offset overflows")
                })?;
                walk_ordinary_helper_result_v1(types, *field, next, walk, leaf)?;
            }
            Ok(())
        }
        _ => Err(ordinary_helper_result_error_v1(
            "helper result requires arrays or tuples of plain scalar and unit leaves; nominal, pointer, capability, enum and drop-bearing results are unsupported",
        )),
    }
}

fn ordinary_helper_result_scalar_attributes_v1(
    attributes: &fe2o3_mir_model::semantic_mir_v1::SemanticAbiValueAttributesV1,
) -> bool {
    let regular = attributes.regular();
    !regular.no_alias()
        && regular.pointer_capture().is_none()
        && !regular.non_null()
        && !regular.read_only()
        && !regular.in_register()
        && attributes.pointee_size_bytes() == 0
        && attributes.pointee_alignment_bytes().is_none()
}

fn ordinary_helper_result_abi_v1(
    function: &SemanticFunctionDeclV1,
    declaration: &SemanticTypeDeclV1,
    physical: &[&OrdinaryHelperResultComponentV1],
) -> bool {
    let abi = function.abi();
    if abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != fe2o3_mir_model::semantic_mir_v1::SemanticExternAbiV1::Rust
        || abi.return_value().pointee_override().is_some()
    {
        return false;
    }
    match abi.return_value().mode() {
        SemanticAbiPassModeV1::Direct(attributes) => {
            ordinary_helper_result_scalar_attributes_v1(attributes)
                && matches!((declaration.layout().backend_repr(), physical),
                    (SemanticBackendReprV1::Scalar(scalar), [component])
                        if component.offset == 0 && &component.scalar == scalar)
        }
        SemanticAbiPassModeV1::Pair { first, second } => {
            if !ordinary_helper_result_scalar_attributes_v1(first)
                || !ordinary_helper_result_scalar_attributes_v1(second)
            {
                return false;
            }
            let SemanticBackendReprV1::ScalarPair { first, second } =
                declaration.layout().backend_repr()
            else {
                return false;
            };
            let Some(first_size) = first.primitive().size_bytes() else {
                return false;
            };
            let align = second.primitive().alignment_bytes();
            let second_offset = first_size
                .checked_add(align.saturating_sub(1))
                .map(|offset| offset & !align.saturating_sub(1));
            matches!(physical, [a, b] if a.offset == 0 && &a.scalar == first
                && Some(b.offset) == second_offset && &b.scalar == second)
        }
        SemanticAbiPassModeV1::Cast { pad_i32, cast } => {
            let layout = declaration.layout();
            matches!(
                layout.backend_repr(),
                SemanticBackendReprV1::Memory { sized: true }
            ) && layout
                .size_bytes()
                .is_some_and(|size| (1..=8).contains(&size))
                && !pad_i32
                && cast.prefix().iter().all(Option::is_none)
                && cast.rest_offset_bytes().is_none()
                && cast.rest().unit().kind() == SemanticAbiRegisterKindV1::Integer
                && layout.size_bytes() == Some(cast.rest().unit().size_bytes())
                && layout.size_bytes() == Some(cast.rest_total_bytes())
                && !cast.rest_consecutive()
                && ordinary_helper_result_scalar_attributes_v1(&cast.attributes())
                && cast.attributes().extension() == SemanticAbiExtensionV1::None
                && !physical.is_empty()
        }
        SemanticAbiPassModeV1::Ignore | SemanticAbiPassModeV1::Indirect { .. } => false,
    }
}

fn plan_ordinary_helper_result_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    work: &mut fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1,
    storage_limit: usize,
) -> Result<HelperResultTransportV1, ProductionSemanticKirErrorV1> {
    let plan = validate_ordinary_helper_result_transport_inner_v1(
        types,
        function,
        &mut |amount| charge_ordinary_helper_result_work_v1(work, amount),
        storage_limit,
    )?;
    // Temporary component/physical scratch has dropped; the retained plan is
    // still covered by the same conservative peak bound before Rc publication.
    charge_ordinary_helper_result_work_v1(work, 1)?;
    Ok(HelperResultTransportV1::Ordinary(std::rc::Rc::new(plan)))
}

/// Checks the ordinary result transport subset of an already admitted semantic
/// catalog. This diagnostic issues no source, effect, control or ABI authority.
/// Charges use the caller's existing phase budget; callback errors are returned
/// unchanged. Scratch uses the planner's logical-row limit, not a byte ledger.
pub fn check_ordinary_helper_result_transport_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    charge: &mut dyn FnMut(usize) -> Result<(), ProductionSemanticKirErrorV1>,
    storage_limit: usize,
) -> Result<(), ProductionSemanticKirErrorV1> {
    validate_ordinary_helper_result_transport_inner_v1(types, function, charge, storage_limit)
        .map(|_| ())
}

/// Checks one ordinary value shape in an already admitted catalog using the
/// result planner's bounded walker. No allocation or authority receipt is made.
/// Every expanded node is charged to the caller before inspection.
pub fn check_ordinary_helper_value_type_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    charge: &mut dyn FnMut(usize) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    walk_ordinary_helper_result_v1(
        types,
        ty,
        0,
        &mut OrdinaryHelperResultWalkV1 {
            work: charge,
            nodes: 0,
        },
        &mut |_| Ok(()),
    )
}

fn validate_ordinary_helper_result_transport_inner_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    work: &mut dyn FnMut(usize) -> Result<(), ProductionSemanticKirErrorV1>,
    storage_limit: usize,
) -> Result<OrdinaryHelperResultPlanV1, ProductionSemanticKirErrorV1> {
    // This bounded planning scratch retains the legacy lowering analysis scope;
    // it is not included in the graph/assertion-origin canonical storage receipt.
    work(16)?;
    work(function.locals().len())?;
    let abi = function.abi();
    let mut returns = function
        .locals()
        .iter()
        .filter(|local| local.role() == SemanticLocalRoleV1::Return);
    if function.role() != SemanticFunctionRoleV1::InternalHelper
        || function.export().is_some()
        || abi.can_unwind()
        || abi.c_variadic()
        || !abi.hidden_arguments().is_empty()
        || abi.return_value().ty() != abi.source_output_type()
        || abi.return_value().adjusted().is_some()
        || returns.next().map(|local| local.ty()) != Some(abi.source_output_type())
        || returns.next().is_some()
    {
        return Err(ordinary_helper_result_error_v1(
            "ordinary helper result lacks exact internal source return custody",
        ));
    }
    let ty = function.abi().source_output_type();
    let declaration = types
        .get(ty.index() as usize)
        .ok_or_else(|| ordinary_helper_result_error_v1("ordinary helper result type is missing"))?;
    if !matches!(
        declaration.shape(),
        SemanticTypeShapeV1::Array { .. } | SemanticTypeShapeV1::Tuple(_)
    ) {
        return Err(ordinary_helper_result_error_v1(
            "ordinary helper result is not an array or tuple",
        ));
    }
    let mut count = 0_usize;
    walk_ordinary_helper_result_v1(
        types,
        ty,
        0,
        &mut OrdinaryHelperResultWalkV1 { work, nodes: 0 },
        &mut |_| {
            count += 1;
            Ok(())
        },
    )?;
    if count == 0 {
        return Err(ordinary_helper_result_error_v1(
            "non-ignored helper result has no scalar components",
        ));
    }
    let needed = count
        .checked_mul(3)
        .and_then(|rows| rows.checked_add(1))
        .ok_or_else(|| {
            ordinary_helper_result_error_v1("ordinary helper result storage overflows")
        })?;
    enforce_limit(
        ProductionSemanticKirResourceV1::AnalysisStorage,
        needed,
        storage_limit,
    )?;
    work(3)?;
    let mut components = Vec::new();
    let mut physical: Vec<&OrdinaryHelperResultComponentV1> = Vec::new();
    let mut result_types = Vec::new();
    let allocation_error = |_| ProductionSemanticKirErrorV1::AllocationFailure {
        resource: ProductionSemanticKirResourceV1::AnalysisStorage,
    };
    components
        .try_reserve_exact(count)
        .map_err(allocation_error)?;
    ordinary_helper_result_storage_v1([components.capacity(), count, count], storage_limit)?;
    physical
        .try_reserve_exact(count)
        .map_err(allocation_error)?;
    ordinary_helper_result_storage_v1(
        [components.capacity(), physical.capacity(), count],
        storage_limit,
    )?;
    result_types
        .try_reserve_exact(count)
        .map_err(allocation_error)?;
    ordinary_helper_result_storage_v1(
        [
            components.capacity(),
            physical.capacity(),
            result_types.capacity(),
        ],
        storage_limit,
    )?;
    walk_ordinary_helper_result_v1(
        types,
        ty,
        0,
        &mut OrdinaryHelperResultWalkV1 { work, nodes: 0 },
        &mut |component| {
            if components.len() == count {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            components.push(component);
            Ok(())
        },
    )?;
    if components.len() != count {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let logarithm = usize::BITS as usize - count.leading_zeros() as usize;
    // This charges bounded planning volume, not individual sort comparisons.
    work(count * (logarithm + 2))?;
    physical.extend(components.iter());
    physical.sort_unstable_by_key(|component| component.offset);
    let mut previous_end = 0;
    let source_size = declaration
        .layout()
        .size_bytes()
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    for component in &physical {
        let end = component
            .scalar
            .primitive()
            .size_bytes()
            .and_then(|width| component.offset.checked_add(width))
            .ok_or_else(|| {
                ordinary_helper_result_error_v1("ordinary helper result range overflows")
            })?;
        if component.offset < previous_end || end > source_size {
            return Err(ordinary_helper_result_error_v1(
                "ordinary helper result scalar ranges overlap or exceed the source layout",
            ));
        }
        previous_end = end;
    }
    if !ordinary_helper_result_abi_v1(function, declaration, &physical) {
        return Err(ordinary_helper_result_error_v1(
            "ordinary helper result ABI is not an exact direct, pair, or simple Rust integer transport",
        ));
    }
    for component in &components {
        result_types.push(lower_scalar_type(types, component.semantic_type)?);
    }
    Ok(OrdinaryHelperResultPlanV1 {
        types: result_types,
        source: OrdinaryHelperResultSourceV1 {
            ty,
            layout: declaration.layout_identity(),
            abi: function.abi().identity(),
        },
    })
}

fn ordinary_helper_result_storage_v1(
    capacities: [usize; 3],
    limit: usize,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let actual = capacities
        .into_iter()
        .try_fold(1_usize, usize::checked_add)
        .ok_or(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisStorage,
            actual: usize::MAX,
            limit,
        })?;
    enforce_limit(
        ProductionSemanticKirResourceV1::AnalysisStorage,
        actual,
        limit,
    )
}

fn ordinary_helper_result_binding_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    binding: &SemanticValueBindingV1,
    nodes: &mut usize,
) -> Result<(), ProductionSemanticKirErrorV1> {
    *nodes += 1;
    if *nodes > MAX_SSA_VALUE_COMPONENTS_V1 {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let declaration = types
        .get(ty.index() as usize)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    match (declaration.shape(), binding) {
        (SemanticTypeShapeV1::Unit, SemanticValueBindingV1::Unit)
        | (SemanticTypeShapeV1::Scalar(_), SemanticValueBindingV1::Value { .. }) => Ok(()),
        (
            SemanticTypeShapeV1::Array { element, length },
            SemanticValueBindingV1::Aggregate(fields),
        ) if usize::try_from(*length) == Ok(fields.len()) => {
            for field in fields {
                ordinary_helper_result_binding_v1(types, *element, field, nodes)?;
            }
            Ok(())
        }
        (SemanticTypeShapeV1::Tuple(types2), SemanticValueBindingV1::Aggregate(fields))
            if types2.fields().len() == fields.len() =>
        {
            for (ty, field) in types2.fields().iter().zip(fields) {
                ordinary_helper_result_binding_v1(types, *ty, field, nodes)?;
            }
            Ok(())
        }
        _ => Err(ordinary_helper_result_error_v1(
            "ordinary helper result binding has a changed shape or capability",
        )),
    }
}

impl SemanticFunctionLoweringV1<'_> {
    fn lower_ordinary_helper_return_v1(
        &mut self,
        block: SemanticBlockIdV1,
        return_local: usize,
        source: OrdinaryHelperResultSourceV1,
        operations: &mut Vec<Operation>,
    ) -> Result<Terminator, ProductionSemanticKirErrorV1> {
        if self.function.abi().identity() != source.abi
            || self.function.abi().source_output_type() != source.ty
            || self.function.locals()[return_local].ty() != source.ty
            || self.types[source.ty.index() as usize].layout_identity() != source.layout
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let binding = self
            .locals
            .get(return_local)
            .and_then(Option::as_ref)
            .ok_or(ProductionSemanticKirErrorV1::MissingLocalDefinition {
                function: self.semantic_function.index(),
                block: block.index(),
                statement: None,
                local: return_local as u32,
            })?;
        ordinary_helper_result_binding_v1(self.types, source.ty, binding, &mut 0)?;
        let components = binding.values().map_err(ordinary_helper_result_error_v1)?;
        let transport = self.result_transport.clone();
        let expected = transport.types();
        if components.len() != expected.len() {
            return Err(ordinary_helper_result_error_v1(
                "ordinary helper return component count changed",
            ));
        }
        let mut values = Vec::new();
        values.try_reserve_exact(expected.len()).map_err(|_| {
            ProductionSemanticKirErrorV1::AllocationFailure {
                resource: ProductionSemanticKirResourceV1::AnalysisStorage,
            }
        })?;
        for ((value, actual), expected) in components.into_iter().zip(expected.iter().cloned()) {
            values.push(self.coerce_transport_value_v1(
                operations,
                block,
                None,
                value,
                actual,
                expected,
                "ordinary helper return component type changed",
            )?);
        }
        Ok(Terminator::Return { values })
    }

    fn emit_ordinary_helper_call_v1(
        &mut self,
        operations: &mut Vec<Operation>,
        callee: FunctionId,
        arguments: Vec<ValueId>,
        source: OrdinaryHelperResultSourceV1,
        result_types: &[Type],
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        if self
            .types
            .get(source.ty.index() as usize)
            .map(SemanticTypeDeclV1::layout_identity)
            != Some(source.layout)
            || result_types.is_empty()
            || result_types.len() > MAX_SSA_VALUE_COMPONENTS_V1
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        self.reserve_operation(operations)?;
        let count = u32::try_from(result_types.len())
            .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let next_value = self.next_value.checked_add(count).ok_or_else(|| {
            ordinary_helper_result_error_v1("ordinary helper call SSA identity overflows")
        })?;
        let mut results = Vec::new();
        results.try_reserve_exact(result_types.len()).map_err(|_| {
            ProductionSemanticKirErrorV1::AllocationFailure {
                resource: ProductionSemanticKirResourceV1::AnalysisStorage,
            }
        })?;
        for (ordinal, ty) in result_types.iter().enumerate() {
            results.push(ValueDef::new(
                ValueId(self.next_value + ordinal as u32),
                ty.clone(),
            ));
        }
        let binding = binding_from_value_defs(self.types, source.ty, &results)?;
        self.next_value = next_value;
        operations.push(Operation::new(
            results,
            OperationKind::Call { callee, arguments },
        ));
        Ok(binding)
    }
}
