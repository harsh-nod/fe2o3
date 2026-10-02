// Internal enum results are logical SSA values, not Rust-layout host arguments.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarEnumResultSlotV1 {
    Tag,
    Payload { variant: u32, field: u32, leaf: u32 },
}

#[derive(Clone, Debug)]
struct ScalarEnumResultComponentV1 {
    slot: ScalarEnumResultSlotV1,
    semantic_type: SemanticTypeIdV1,
    kernel_type: Type,
}

#[derive(Clone, Debug)]
struct ScalarEnumResultShapeV1 {
    components: Vec<ScalarEnumResultComponentV1>,
}

fn scalar_enum_result_error_v1() -> ProductionSemanticKirErrorV1 {
    unsupported(
        0,
        None,
        None,
        "scalar enum helper result requires ordinary scalar-data payloads",
    )
}

fn scalar_enum_result_producer_error_v1() -> ProductionSemanticKirErrorV1 {
    unsupported(
        0,
        None,
        None,
        "scalar enum helper result requires complete acyclic SSA producers",
    )
}

struct ScalarEnumResultAnalysisBudgetV1<'a, 'work> {
    domain: SemanticEnumAnalysisBudgetV1,
    shared: Option<&'a mut (dyn SemanticEmissionBudgetV1 + 'work)>,
}

impl ScalarEnumResultAnalysisBudgetV1<'_, '_> {
    fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.domain.charge_work(amount)?;
        if let Some(shared) = self.shared.as_deref_mut() {
            shared.charge_work(amount)?;
        }
        Ok(())
    }

    fn charge_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.domain.charge_storage(amount)?;
        if let Some(shared) = self.shared.as_deref_mut() {
            shared.reserve_storage(argument_product_v1(amount, 128)?)?;
        }
        Ok(())
    }
}

fn visit_scalar_enum_result_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    mut visit: impl FnMut(
        ScalarEnumResultSlotV1,
        SemanticTypeIdV1,
        Type,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    fn data(
        types: &[SemanticTypeDeclV1],
        ty: SemanticTypeIdV1,
        variant: u32,
        field: u32,
        leaf: &mut u32,
        nodes: &mut usize,
        visit: &mut impl FnMut(
            ScalarEnumResultSlotV1,
            SemanticTypeIdV1,
            Type,
        ) -> Result<(), ProductionSemanticKirErrorV1>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        *nodes = nodes
            .checked_add(1)
            .ok_or_else(scalar_enum_result_error_v1)?;
        if *nodes > MAX_SSA_VALUE_COMPONENTS_V1 {
            return Err(scalar_enum_result_error_v1());
        }
        let declaration = types
            .get(ty.index() as usize)
            .ok_or_else(scalar_enum_result_error_v1)?;
        require_ordinary_execution_representation_v29(declaration)
            .map_err(|_| scalar_enum_result_error_v1())?;
        match declaration.shape() {
            SemanticTypeShapeV1::Unit => Ok(()),
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_) => {
                let kernel_type = lower_scalar_type(types, ty)?;
                scalar_enum_result_zero_v1(&kernel_type)?;
                visit(
                    ScalarEnumResultSlotV1::Payload {
                        variant,
                        field,
                        leaf: *leaf,
                    },
                    ty,
                    kernel_type,
                )?;
                *leaf = leaf
                    .checked_add(1)
                    .ok_or_else(scalar_enum_result_error_v1)?;
                Ok(())
            }
            SemanticTypeShapeV1::Tuple(fields) => {
                for field_type in fields.fields() {
                    data(types, *field_type, variant, field, leaf, nodes, visit)?;
                }
                Ok(())
            }
            SemanticTypeShapeV1::Array { element, length } => {
                if *length > MAX_SSA_VALUE_COMPONENTS_V1 as u64 {
                    return Err(scalar_enum_result_error_v1());
                }
                for _ in 0..*length {
                    data(types, *element, variant, field, leaf, nodes, visit)?;
                }
                Ok(())
            }
            _ => Err(scalar_enum_result_error_v1()),
        }
    }
    let declaration = types
        .get(ty.index() as usize)
        .ok_or_else(scalar_enum_result_error_v1)?;
    require_ordinary_execution_representation_v29(declaration)
        .map_err(|_| scalar_enum_result_error_v1())?;
    let SemanticTypeShapeV1::Enum {
        discriminant,
        variants,
    } = declaration.shape()
    else {
        return Err(scalar_enum_result_error_v1());
    };
    if variants.is_empty() || variants.len() > MAX_SSA_VALUE_COMPONENTS_V1 {
        return Err(scalar_enum_result_error_v1());
    }
    let tag_type = lower_scalar_type(types, *discriminant)?;
    integer_constant(&tag_type, 0)?;
    visit(ScalarEnumResultSlotV1::Tag, *discriminant, tag_type)?;
    let mut nodes = 1;
    for (variant, declaration) in variants.iter().enumerate() {
        nodes += 1;
        if nodes > MAX_SSA_VALUE_COMPONENTS_V1 {
            return Err(scalar_enum_result_error_v1());
        }
        for (field, ty) in declaration.fields().fields().iter().enumerate() {
            data(
                types,
                *ty,
                variant as u32,
                field as u32,
                &mut 0,
                &mut nodes,
                &mut visit,
            )?;
        }
    }
    Ok(nodes)
}

fn scalar_enum_result_shape_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Result<ScalarEnumResultShapeV1, ProductionSemanticKirErrorV1> {
    let mut count = 0;
    visit_scalar_enum_result_v1(types, ty, |_, _, _| {
        count += 1;
        Ok(())
    })?;
    let mut components = argument_vec_v1(count)?;
    visit_scalar_enum_result_v1(types, ty, |slot, semantic_type, kernel_type| {
        components.push(ScalarEnumResultComponentV1 {
            slot,
            semantic_type,
            kernel_type,
        });
        Ok(())
    })?;
    Ok(ScalarEnumResultShapeV1 { components })
}

fn prepay_helper_result_shape_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if !matches!(
        types[ty.index() as usize].shape(),
        SemanticTypeShapeV1::Enum { .. }
    ) {
        return prepay_typed_shape_v1(types, ty, 0, budget);
    }
    // Pay the bounded census before traversing any variant, then its two shape walks.
    budget.charge_work(MAX_SSA_VALUE_COMPONENTS_V1)?;
    let mut count = 0;
    let nodes = visit_scalar_enum_result_v1(types, ty, |_, _, _| {
        count += 1;
        Ok(())
    })?;
    budget.charge_work(argument_product_v1(nodes, 4)?)?;
    budget.reserve_storage(argument_product_v1(
        count,
        std::mem::size_of::<ScalarEnumResultComponentV1>() + std::mem::size_of::<Type>(),
    )?)?;
    Ok(())
}

fn scalar_enum_result_zero_v1(ty: &Type) -> Result<Constant, ProductionSemanticKirErrorV1> {
    Ok(match ty.as_scalar() {
        Some(ScalarType::F16) => Constant::F16Bits(0),
        Some(ScalarType::Bf16) => Constant::Bf16Bits(0),
        Some(ScalarType::F32) => Constant::F32Bits(0),
        Some(ScalarType::F64) => Constant::F64Bits(0),
        _ => integer_constant(ty, 0)?,
    })
}

fn scalar_enum_result_values_v1(
    types: &[SemanticTypeDeclV1],
    semantic_type: SemanticTypeIdV1,
    binding: &SemanticValueBindingV1,
    expected: &[Type],
) -> Result<Vec<(ValueId, Type)>, &'static str> {
    fn append(
        types: &[SemanticTypeDeclV1],
        ty: SemanticTypeIdV1,
        binding: &SemanticValueBindingV1,
        values: &mut Vec<(ValueId, Type)>,
        nodes: &mut usize,
    ) -> Result<(), &'static str> {
        *nodes = nodes
            .checked_add(1)
            .ok_or("scalar enum result binding exceeds its structural bound")?;
        if *nodes > MAX_SSA_VALUE_COMPONENTS_V1 {
            return Err("scalar enum result binding exceeds its structural bound");
        }
        let declaration = types
            .get(ty.index() as usize)
            .ok_or("scalar enum result type is missing")?;
        match (declaration.shape(), binding) {
            (SemanticTypeShapeV1::Unit, SemanticValueBindingV1::Unit) => Ok(()),
            (
                SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_),
                SemanticValueBindingV1::Value { id, ty: actual },
            ) if *actual
                == lower_scalar_type(types, ty)
                    .map_err(|_| "scalar enum result type changed")? =>
            {
                if values.len() == values.capacity() {
                    return Err("scalar enum result component bound changed");
                }
                values.push((*id, actual.clone()));
                Ok(())
            }
            (SemanticTypeShapeV1::Tuple(shape), SemanticValueBindingV1::Aggregate(fields))
                if shape.fields().len() == fields.len() =>
            {
                for (ty, field) in shape.fields().iter().zip(fields) {
                    append(types, *ty, field, values, nodes)?;
                }
                Ok(())
            }
            (
                SemanticTypeShapeV1::Array { element, length },
                SemanticValueBindingV1::Aggregate(fields),
            ) if *length == fields.len() as u64 => {
                for field in fields {
                    append(types, *element, field, values, nodes)?;
                }
                Ok(())
            }
            _ => Err("scalar enum result contains a non-data binding"),
        }
    }
    visit_scalar_enum_result_v1(types, semantic_type, |_, _, _| Ok(()))
        .map_err(|_| "scalar enum result shape changed")?;
    let SemanticValueBindingV1::Enum {
        discriminant,
        discriminant_ty,
        semantic_type: actual,
        payloads,
        ..
    } = binding
    else {
        return Err("scalar enum result binding is not an enum");
    };
    let (tag, variants) = semantic_enum_shape(types, semantic_type)
        .map_err(|_| "scalar enum result shape changed")?;
    if *actual != semantic_type
        || *discriminant_ty
            != lower_scalar_type(types, tag).map_err(|_| "scalar enum result tag type changed")?
        || payloads.len() != variants.len()
        || expected.is_empty()
        || expected.len() > MAX_SSA_VALUE_COMPONENTS_V1
    {
        return Err("scalar enum result variant roster changed");
    }
    let mut values =
        argument_vec_v1(expected.len()).map_err(|_| "scalar enum result allocation refused")?;
    values.push((*discriminant, discriminant_ty.clone()));
    let mut nodes = 1;
    for (ordinal, variant) in variants.iter().enumerate() {
        let fields = payloads
            .get(&(ordinal as u32))
            .ok_or("scalar enum result variant roster changed")?;
        if fields.len() != variant.fields().fields().len() {
            return Err("scalar enum result field roster changed");
        }
        for (ty, field) in variant.fields().fields().iter().zip(fields) {
            append(types, *ty, field, &mut values, &mut nodes)?;
        }
    }
    if values.len() != expected.len()
        || values
            .iter()
            .zip(expected)
            .any(|((_, actual), expected)| actual != expected)
    {
        return Err("scalar enum result lost a variant-qualified SSA component");
    }
    Ok(values)
}

fn scalar_enum_result_binding_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    values: &[ValueDef],
) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
    let shape = scalar_enum_result_shape_v1(types, ty)?;
    if shape.components.len() != values.len()
        || shape
            .components
            .iter()
            .zip(values)
            .any(|(component, value)| component.kernel_type != value.ty)
    {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    let (_, variants) = semantic_enum_shape(types, ty)?;
    let mut payloads = BTreeMap::new();
    let mut next = 1;
    for (variant, declaration) in variants.iter().enumerate() {
        let mut fields = argument_vec_v1(declaration.fields().fields().len())?;
        for field_type in declaration.fields().fields() {
            let count = lower_ssa_value_components_v1(types, *field_type)?.len();
            let end = next + count;
            // Inactive slots are physical words only; downcast authentication is unchanged.
            fields.push(binding_from_value_defs_with_validation(
                types,
                *field_type,
                &values[next..end],
                false,
            )?);
            next = end;
        }
        payloads.insert(variant as u32, fields);
    }
    if next != values.len() {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(SemanticValueBindingV1::Enum {
        discriminant: values[0].id,
        discriminant_ty: values[0].ty.clone(),
        semantic_type: ty,
        variant: None,
        payloads,
    })
}

fn scalar_enum_result_locals_v1<'work>(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    semantic_ssa: &ProductionSemanticSsaFunctionPlanV1,
    max_work: usize,
    max_storage: usize,
    emission: Option<&mut (dyn SemanticEmissionBudgetV1 + 'work)>,
) -> Result<BTreeSet<u32>, ProductionSemanticKirErrorV1> {
    fn reject_escape(
        operand: &SemanticOperandV1,
        selected: &BTreeSet<u32>,
        budget: &mut ScalarEnumResultAnalysisBudgetV1<'_, '_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        if let SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) = operand
            && place.projections().is_empty()
            && selected.contains(&place.local().index())
        {
            return Err(scalar_enum_result_producer_error_v1());
        }
        Ok(())
    }
    let mut budget = ScalarEnumResultAnalysisBudgetV1 {
        domain: SemanticEnumAnalysisBudgetV1::new(max_work, max_storage),
        shared: emission,
    };
    let mut selected = BTreeSet::new();
    let is_enum = |ty: SemanticTypeIdV1| {
        matches!(
            types[ty.index() as usize].shape(),
            SemanticTypeShapeV1::Enum { .. }
        )
    };
    let insert = |local: u32,
                  selected: &mut BTreeSet<u32>,
                  budget: &mut ScalarEnumResultAnalysisBudgetV1<'_, '_>|
     -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        if selected.contains(&local) {
            return Ok(false);
        }
        budget.charge_storage(1)?;
        selected.insert(local);
        Ok(true)
    };
    for (local, declaration) in function.locals().iter().enumerate() {
        budget.charge_work(1)?;
        if declaration.role() == SemanticLocalRoleV1::Return && is_enum(declaration.ty()) {
            insert(local as u32, &mut selected, &mut budget)?;
        }
    }
    for block in function.blocks() {
        budget.charge_work(1)?;
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && matches!(
                callables.get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::Defined { .. })
            )
            && let Some(destination) = call.destination()
            && is_enum(destination.place().ty())
        {
            if !destination.place().projections().is_empty() {
                return Err(scalar_enum_result_producer_error_v1());
            }
            insert(
                destination.place().local().index(),
                &mut selected,
                &mut budget,
            )?;
        }
    }
    if selected.is_empty() {
        return Ok(selected);
    }
    loop {
        let mut changed = false;
        for block in function.blocks() {
            for statement in block.statements() {
                budget.charge_work(1)?;
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                let SemanticRvalueKindV1::Use(
                    SemanticOperandV1::Copy(source) | SemanticOperandV1::Move(source),
                ) = assignment.value().kind()
                else {
                    continue;
                };
                let destination = assignment.destination();
                if !source.projections().is_empty() || !destination.projections().is_empty() {
                    continue;
                }
                if selected.contains(&source.local().index())
                    || selected.contains(&destination.local().index())
                {
                    if source.ty() != destination.ty() {
                        return Err(scalar_enum_result_producer_error_v1());
                    }
                    changed |= insert(source.local().index(), &mut selected, &mut budget)?;
                    changed |= insert(destination.local().index(), &mut selected, &mut budget)?;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let shared = semantic_ssa.plan();
    for local in &selected {
        budget.charge_work(shared.promoted_variables().len())?;
        let declaration = &function.locals()[*local as usize];
        if declaration.role().is_entry_argument()
            || !shared
                .promoted_variables()
                .iter()
                .any(|variable| variable.get() == *local)
        {
            return Err(scalar_enum_result_producer_error_v1());
        }
        budget.charge_work(MAX_SSA_VALUE_COMPONENTS_V1)?;
        let nodes = visit_scalar_enum_result_v1(types, declaration.ty(), |_, _, _| Ok(()))?;
        budget.charge_storage(nodes)?;
        if let Some(shared) = budget.shared.as_deref_mut() {
            shared.charge_work(argument_product_v1(nodes, function.blocks().len())?)?;
            shared.reserve_storage(argument_product_v1(
                argument_product_v1(nodes, function.blocks().len().max(1))?,
                512,
            )?)?;
        }
    }
    // RPO is a topological order exactly when every reachable edge points forward.
    for (position, block_id) in shared.reverse_postorder().iter().enumerate() {
        let block = &function.blocks()[block_id.get() as usize];
        block.terminator().kind().try_for_each_edge(|edge| {
            budget.charge_work(shared.reverse_postorder().len())?;
            let target = shared
                .reverse_postorder()
                .iter()
                .position(|id| id.get() == edge.target().index());
            if target.is_some_and(|target| target <= position) {
                return Err(scalar_enum_result_producer_error_v1());
            }
            Ok(())
        })?;
        for statement in block.statements() {
            budget.charge_work(1)?;
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    let destination = assignment.destination();
                    let value = assignment.value().kind();
                    if let SemanticRvalueKindV1::Borrow { place, .. }
                    | SemanticRvalueKindV1::AddressOf { place, .. } = value
                        && selected.contains(&place.local().index())
                    {
                        return Err(scalar_enum_result_producer_error_v1());
                    }
                    let whole_copy = matches!(value, SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place))
                        if place.projections().is_empty() && selected.contains(&place.local().index()))
                        && destination.projections().is_empty()
                        && selected.contains(&destination.local().index());
                    if !whole_copy {
                        value.try_visit_operands(|operand| {
                            reject_escape(operand, &selected, &mut budget)
                        })?;
                    }
                    if !selected.contains(&destination.local().index()) {
                        continue;
                    }
                    if !destination.projections().is_empty() {
                        return Err(scalar_enum_result_producer_error_v1());
                    }
                    match value {
                        SemanticRvalueKindV1::Aggregate(aggregate) => {
                            let SemanticAggregateKindV1::EnumVariant(variant) = aggregate.kind()
                            else {
                                return Err(scalar_enum_result_producer_error_v1());
                            };
                            let (_, variants) = semantic_enum_shape(types, destination.ty())?;
                            let fields = variants
                                .get(*variant as usize)
                                .ok_or_else(scalar_enum_result_producer_error_v1)?
                                .fields()
                                .fields();
                            if fields.len() != aggregate.operands().len() {
                                return Err(scalar_enum_result_producer_error_v1());
                            }
                        }
                        SemanticRvalueKindV1::Use(
                            SemanticOperandV1::Copy(source) | SemanticOperandV1::Move(source),
                        ) if source.projections().is_empty()
                            && source.ty() == destination.ty()
                            && selected.contains(&source.local().index()) => {}
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(constant))
                            if constant.ty() == destination.ty() => {}
                        _ => return Err(scalar_enum_result_producer_error_v1()),
                    }
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place)
                    if selected.contains(&place.local().index()) =>
                {
                    return Err(scalar_enum_result_producer_error_v1());
                }
                SemanticStatementKindV1::Store(store) => {
                    reject_escape(store.value(), &selected, &mut budget)?
                }
                SemanticStatementKindV1::AtomicRmw(atomic) => {
                    reject_escape(atomic.value(), &selected, &mut budget)?
                }
                SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                    reject_escape(atomic.expected(), &selected, &mut budget)?;
                    reject_escape(atomic.replacement(), &selected, &mut budget)?;
                }
                SemanticStatementKindV1::Assume(operand) => {
                    reject_escape(operand, &selected, &mut budget)?
                }
                // StorageLive/Dead and whole-local Move kills are enforced by the shared SSA plan.
                _ => {}
            }
        }
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => {
                for operand in call.arguments() {
                    reject_escape(operand, &selected, &mut budget)?;
                }
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                for operand in call.arguments() {
                    reject_escape(operand, &selected, &mut budget)?;
                }
            }
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
                reject_escape(discriminant, &selected, &mut budget)?
            }
            SemanticTerminatorKindV1::Assert { condition, .. } => {
                reject_escape(condition, &selected, &mut budget)?
            }
            _ => {}
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(destination) = call.destination()
            && selected.contains(&destination.place().local().index())
            && (!destination.place().projections().is_empty()
                || !matches!(
                    callables.get(call.callee().index() as usize),
                    Some(SemanticCallableDeclV1::Defined { .. })
                ))
        {
            return Err(scalar_enum_result_producer_error_v1());
        }
    }
    Ok(selected)
}
