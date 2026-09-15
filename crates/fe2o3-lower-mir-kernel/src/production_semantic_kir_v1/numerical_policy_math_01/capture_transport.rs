fn math_reference_binding_matches(
    actual: &SemanticValueBindingV1,
    original: &SemanticValueBindingV1,
) -> bool {
    match (actual, original) {
        (SemanticValueBindingV1::MathContext, SemanticValueBindingV1::MathContext) => true,
        (
            SemanticValueBindingV1::KernelContext {
                value: a,
                context: at,
            },
            SemanticValueBindingV1::KernelContext {
                value: b,
                context: bt,
            },
        ) => a == b && at == bt,
        (
            SemanticValueBindingV1::Value { id: a, ty: at },
            SemanticValueBindingV1::Value { id: b, ty: bt },
        ) => a == b && at == bt,
        _ => false,
    }
}

fn math_capture_shape_work(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    path: &[u32],
) -> Result<usize, ProductionSemanticKirErrorV1> {
    fn visit(
        types: &[SemanticTypeDeclV1],
        ty: SemanticTypeIdV1,
        path: Option<&[u32]>,
        work: &mut usize,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        *work += 1;
        if *work > MAX_SSA_VALUE_COMPONENTS_V1 {
            return Err(unsupported(
                0,
                None,
                None,
                "Math capture exceeds the existing SSA structural limit",
            ));
        }
        if path.is_some_and(|path| path.is_empty()) {
            return Ok(());
        }
        let shape = types
            .get(ty.index() as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .shape();
        match shape {
            SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields) => {
                let selected = path.and_then(|path| path.split_first());
                if selected.is_some_and(|(field, _)| *field as usize >= fields.fields().len()) {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                if fields.fields().len() > MAX_SSA_VALUE_COMPONENTS_V1 - *work {
                    return Err(unsupported(
                        0,
                        None,
                        None,
                        "Math capture exceeds the existing SSA structural limit",
                    ));
                }
                for (index, field) in fields.fields().iter().enumerate() {
                    visit(
                        types,
                        *field,
                        selected
                            .filter(|(field, _)| **field as usize == index)
                            .map(|(_, rest)| rest),
                        work,
                    )?;
                }
            }
            SemanticTypeShapeV1::Array { element, length } if path.is_none() => {
                let length = usize::try_from(*length)
                    .ok()
                    .filter(|length| *length <= MAX_SSA_VALUE_COMPONENTS_V1 - *work)
                    .ok_or_else(|| {
                        unsupported(
                            0,
                            None,
                            None,
                            "Math capture exceeds the existing SSA structural limit",
                        )
                    })?;
                for _ in 0..length {
                    visit(types, *element, None, work)?;
                }
            }
            _ if path.is_some() => {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            _ => {}
        }
        Ok(())
    }
    let mut work = 0;
    visit(types, ty, Some(path), &mut work)?;
    Ok(work)
}

fn math_capture_binding_field<'a>(
    mut binding: &'a SemanticValueBindingV1,
    fields: impl IntoIterator<Item = u32>,
) -> Result<&'a SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
    for (depth, field) in fields.into_iter().enumerate() {
        if depth >= 16 {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let SemanticValueBindingV1::Aggregate(values) = binding else {
            return Err(unsupported(
                0,
                None,
                None,
                "Math live capture is not the retained aggregate binding",
            ));
        };
        binding = values
            .get(field as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    }
    Ok(binding)
}

impl MathTransportV1 {
    fn kernel_types(
        self,
        types: &[SemanticTypeDeclV1],
        semantic_type: SemanticTypeIdV1,
        context: Option<&KernelContextTypeV1>,
    ) -> Result<Vec<Type>, ProductionSemanticKirErrorV1> {
        if self.semantic_type() != semantic_type {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let Some(path) = self.capture else {
            return Ok(vec![self.kernel_type(types, semantic_type, context)?]);
        };
        math_capture_shape_work(types, path.carrier, path.fields())?;
        let leaf = Self {
            capture: None,
            ..self
        };
        let expected_reference = if self.bound {
            self.contract.types().bound_reference
        } else {
            self.contract.types().math_reference
        };
        let mut output = Vec::new();
        fn append(
            types: &[SemanticTypeDeclV1],
            ty: SemanticTypeIdV1,
            path: &[u32],
            reference: SemanticTypeIdV1,
            leaf_type: &Type,
            output: &mut Vec<Type>,
        ) -> Result<(), ProductionSemanticKirErrorV1> {
            if let Some((&selected, rest)) = path.split_first() {
                let fields = numerical_policy_math_custody_01::capture_fields(types, ty)?;
                if selected as usize >= fields.len() || fields.len() > MAX_SSA_VALUE_COMPONENTS_V1 {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                for (index, field) in fields.iter().enumerate() {
                    if index == selected as usize {
                        append(types, *field, rest, reference, leaf_type, output)?;
                    } else {
                        let ordinary = lower_ssa_value_types(types, *field)?;
                        if ordinary.len() > MAX_SSA_VALUE_COMPONENTS_V1.saturating_sub(output.len())
                        {
                            return Err(unsupported(
                                0,
                                None,
                                None,
                                "Math capture exceeds the existing SSA component limit",
                            ));
                        }
                        output.extend(ordinary);
                    }
                }
            } else {
                if ty != reference {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                output.push(leaf_type.clone());
            }
            if output.len() > MAX_SSA_VALUE_COMPONENTS_V1 {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "Math capture exceeds the existing SSA component limit",
                ));
            }
            Ok(())
        }
        append(
            types,
            path.carrier,
            path.fields(),
            expected_reference,
            &leaf.kernel_type(types, leaf.semantic_type(), context)?,
            &mut output,
        )?;
        Ok(output)
    }

    fn capture_values(
        self,
        binding: &SemanticValueBindingV1,
        expected: &[Type],
    ) -> Result<Vec<(ValueId, Type)>, &'static str> {
        let path = self.capture.ok_or("Math capture descriptor is absent")?;
        let selected = math_capture_binding_field(binding, path.fields().iter().copied())
            .map_err(|_| "Math capture field differs from its checked SSA route")?;
        let mut capability = expected.iter().filter(|ty| {
            matches!(ty,
            Type::ExecutionCapability(c) if matches!(c.role,
                ExecutionCapabilityRoleV1::NumericalPolicyMathBound(_)
                | ExecutionCapabilityRoleV1::NumericalPolicyMathSource(_)))
        });
        let one = capability
            .next()
            .ok_or("Math capture lacks its expected capability")?;
        if capability.next().is_some() {
            return Err("Math capture transport does not admit an untracked second capability");
        }
        Self {
            capture: None,
            ..self
        }
        .values(selected, std::slice::from_ref(one))?;
        let values = binding.values()?;
        if values.len() > MAX_SSA_VALUE_COMPONENTS_V1
            || values.iter().map(|(_, ty)| ty).ne(expected.iter())
        {
            return Err("Math capture changed its exact SSA component types");
        }
        Ok(values)
    }

    fn capture_from_values(
        self,
        types: &[SemanticTypeDeclV1],
        semantic_type: SemanticTypeIdV1,
        values: &[ValueDef],
        expected: &[Type],
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let path = self
            .capture
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        math_capture_shape_work(types, path.carrier, path.fields())?;
        if semantic_type != path.carrier
            || values.len() > MAX_SSA_VALUE_COMPONENTS_V1
            || values.iter().map(|value| &value.ty).ne(expected.iter())
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        fn restore(
            leaf: MathTransportV1,
            types: &[SemanticTypeDeclV1],
            ty: SemanticTypeIdV1,
            path: &[u32],
            values: &[ValueDef],
            expected: &[Type],
            cursor: &mut usize,
        ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
            if let Some((&selected, rest)) = path.split_first() {
                let fields = numerical_policy_math_custody_01::capture_fields(types, ty)?;
                if selected as usize >= fields.len() || fields.len() > MAX_SSA_VALUE_COMPONENTS_V1 {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                let mut bindings = Vec::with_capacity(fields.len());
                for (index, field) in fields.iter().enumerate() {
                    let binding = if index == selected as usize {
                        restore(leaf, types, *field, rest, values, expected, cursor)?
                    } else {
                        let ordinary = lower_ssa_value_types(types, *field)?;
                        let end = cursor
                            .checked_add(ordinary.len())
                            .filter(|end| *end <= values.len())
                            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                        if expected[*cursor..end] != ordinary {
                            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                        }
                        let result = binding_from_value_defs(types, *field, &values[*cursor..end])?;
                        *cursor = end;
                        result
                    };
                    bindings.push(binding);
                }
                Ok(SemanticValueBindingV1::Aggregate(bindings))
            } else {
                let reference = if leaf.bound {
                    leaf.contract.types().bound_reference
                } else {
                    leaf.contract.types().math_reference
                };
                if ty != reference || *cursor >= values.len() {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
                let result = leaf.from_values(
                    types,
                    leaf.semantic_type(),
                    &values[*cursor..*cursor + 1],
                    &expected[*cursor..*cursor + 1],
                )?;
                *cursor += 1;
                Ok(result)
            }
        }
        let mut cursor = 0;
        let binding = restore(
            Self {
                capture: None,
                ..self
            },
            types,
            path.carrier,
            path.fields(),
            values,
            expected,
            &mut cursor,
        )?;
        if cursor != values.len() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        self.capture_values(&binding, expected)
            .map_err(|detail| unsupported(0, None, None, detail))?;
        Ok(binding)
    }
}
