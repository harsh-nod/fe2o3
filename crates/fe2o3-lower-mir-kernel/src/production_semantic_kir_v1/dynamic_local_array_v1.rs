struct FixedArrayGuardAnalysisV1<'a> {
    inventory: InfallibleBoundsAssertAnalysisV1<'a>,
    predecessors: Vec<Vec<usize>>,
    guards: BTreeMap<(u32, u32, u64), u32>,
    traversal_work: usize,
    work: usize,
}

impl<'a> FixedArrayGuardAnalysisV1<'a> {
    fn new(
        types: &'a [SemanticTypeDeclV1],
        callables: &'a [SemanticCallableDeclV1],
        function: &'a SemanticFunctionDeclV1,
        limit: usize,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let mut size = function
            .locals()
            .len()
            .saturating_add(function.blocks().len());
        for block in function.blocks() {
            size = size.saturating_add(block.statements().len());
            block
                .terminator()
                .kind()
                .try_for_each_edge::<ProductionSemanticKirErrorV1>(|_| {
                    size = size.saturating_add(1);
                    Ok(())
                })?;
        }
        let work = size.saturating_mul(4);
        enforce_limit(ProductionSemanticKirResourceV1::AnalysisWork, work, limit)?;
        enforce_limit(
            ProductionSemanticKirResourceV1::AnalysisStorage,
            size,
            limit,
        )?;
        let inventory =
            InfallibleBoundsAssertAnalysisV1::source_inventory_v1(types, callables, function)?;
        let mut predecessors = vec![Vec::new(); function.blocks().len()];
        for (source, successors) in inventory.successors.iter().enumerate() {
            for &target in successors {
                predecessors[target].push(source);
            }
        }
        Ok(Self {
            inventory,
            predecessors,
            guards: BTreeMap::new(),
            traversal_work: size,
            work,
        })
    }

    fn authorize(
        &mut self,
        block: SemanticBlockIdV1,
        statement: usize,
        array: SemanticLocalIdV1,
        index: SemanticLocalIdV1,
        length: u64,
        limit: usize,
    ) -> Result<Option<u32>, ProductionSemanticKirErrorV1> {
        // Covers the bounded dominance traversals and retained cache entries.
        self.work = self
            .work
            .saturating_add(self.traversal_work.saturating_mul(8));
        enforce_limit(
            ProductionSemanticKirResourceV1::AnalysisWork,
            self.work,
            limit,
        )?;
        let use_block = block.index() as usize;
        let Some(definition) = self.inventory.stable_definition(array.index() as usize) else {
            return Ok(None);
        };
        if !self
            .inventory
            .definition_dominates_use(definition, use_block, statement)?
        {
            return Ok(None);
        }
        let key = (block.index(), index.index(), length);
        if let Some(guard) = self.guards.get(&key) {
            return Ok(Some(*guard));
        }
        let Some(&[guard]) = self.predecessors.get(use_block).map(Vec::as_slice) else {
            return Ok(None);
        };
        let function = self.inventory.function;
        let Some(index_decl) = function.locals().get(index.index() as usize) else {
            return Ok(None);
        };
        let Some(source) = function.blocks().get(guard) else {
            return Ok(None);
        };
        let SemanticTerminatorKindV1::Assert {
            condition,
            expected: true,
            message:
                SemanticAssertMessageV1::BoundsCheck {
                    length: bound,
                    index: operand,
                },
            target,
            unwind: SemanticUnwindActionV1::Unreachable,
        } = source.terminator().kind()
        else {
            return Ok(None);
        };
        if length == 0
            || target.target() != block
            || whole_semantic_operand_local_v1(operand) != Some(index.index() as usize)
            || operand.ty() != index_decl.ty()
            || exact_unsigned_semantic_constant_v1(self.inventory.types, bound, index_decl.ty())
                != Some(u128::from(length))
            || self.inventory.address_escaped.get(index.index() as usize) != Some(&false)
        {
            return Ok(None);
        }
        let Some(condition) = whole_semantic_operand_local_v1(condition) else {
            return Ok(None);
        };
        let Some(SemanticScalarDefinitionV1::Assignment {
            block: condition_block,
            statement: comparison,
        }) = self.inventory.stable_definition(condition)
        else {
            return Ok(None);
        };
        if condition_block != guard
            || !self
                .inventory
                .condition_is_exact_less_than(condition, operand, bound, guard)?
            || !self.inventory.edge_dominates(guard, use_block, use_block)?
        {
            return Ok(None);
        }
        if let Some(definition) = self.inventory.stable_definition(index.index() as usize) {
            if !self
                .inventory
                .definition_dominates_use(definition, guard, comparison)?
                || !self
                    .inventory
                    .definition_dominates_use(definition, use_block, statement)?
            {
                return Ok(None);
            }
        } else if self.inventory.definition_counts.get(index.index() as usize) != Some(&0)
            || !index_decl.role().is_entry_argument()
        {
            return Ok(None);
        }
        enforce_limit(
            ProductionSemanticKirResourceV1::AnalysisStorage,
            self.guards.len().saturating_add(1),
            limit,
        )?;
        self.guards.insert(key, guard as u32);
        Ok(Some(guard as u32))
    }
}

impl SemanticFunctionLoweringV1<'_> {
    #[allow(clippy::too_many_arguments)]
    fn lower_dynamic_local_array_v1(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        place: &SemanticPlaceV1,
        index_local: SemanticLocalIdV1,
        array_type: SemanticTypeIdV1,
        fields: &[SemanticValueBindingV1],
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let refuse = |detail| {
            unsupported(
                self.semantic_function.index(),
                Some(block.index()),
                statement,
                detail,
            )
        };
        let Some(SemanticTypeShapeV1::Array { element, length }) = self
            .types
            .get(array_type.index() as usize)
            .map(SemanticTypeDeclV1::shape)
        else {
            return Err(refuse("dynamic local indexing requires a fixed-size array"));
        };
        let (element, length) = (*element, *length);
        if length == 0
            || usize::try_from(length) != Ok(fields.len())
            || place.projections().len() != 1
            || self.function.locals()[place.local().index() as usize]
                .role()
                .is_entry_argument()
            || self
                .retained_local_slots
                .contains_key(&place.local().index())
            || !matches!(
                self.types[element.index() as usize].shape(),
                SemanticTypeShapeV1::Scalar(_)
            )
        {
            return Err(refuse(
                "dynamic local indexing requires an immutable local scalar array",
            ));
        }
        let element_type = lower_scalar_type(self.types, element)?;
        if fields
            .iter()
            .any(|field| !field.value().is_ok_and(|(_, ty)| ty == element_type))
        {
            return Err(refuse(
                "dynamic local array components do not match their scalar type",
            ));
        }
        let index_type = self.function.locals()[index_local.index() as usize].ty();
        if !matches!(
            semantic_unsigned_integer_bits_v1(self.types, index_type),
            Some(8 | 16 | 32 | 64)
        ) {
            return Err(refuse(
                "dynamic local array index requires an exact unsigned width up to 64 bits",
            ));
        }
        let (index, index_ty) = self.locals[index_local.index() as usize]
            .as_ref()
            .ok_or_else(|| refuse("dynamic local array index is undefined"))?
            .value()
            .map_err(refuse)?;
        let expected_index_type = lower_scalar_type(self.types, index_type)?;
        if index_ty != expected_index_type
            && !(expected_index_type == Type::Scalar(ScalarType::U64) && index_ty == Type::INDEX)
        {
            return Err(refuse(
                "dynamic local array index transport changes its finite width",
            ));
        }
        let use_statement = statement.map_or(
            self.function.blocks()[block.index() as usize]
                .statements()
                .len(),
            |value| value as usize,
        );
        if self.fixed_array_analysis.is_none() {
            self.fixed_array_analysis = Some(FixedArrayGuardAnalysisV1::new(
                self.types,
                self.callables,
                self.function,
                self.max_operations,
            )?);
        }
        let guard = self
            .fixed_array_analysis
            .as_mut()
            .expect("initialized analysis")
            .authorize(
                block,
                use_statement,
                place.local(),
                index_local,
                length,
                self.max_operations,
            )?;
        if guard.is_none_or(|guard| self.infallible_asserts.contains(&guard))
            || self.assert_failure_block.is_none()
        {
            return Err(refuse(
                "dynamic local array access lacks its retained exact Rust bounds guard",
            ));
        }
        self.emit_local_array_selection_v1(operations, index, &index_ty, fields, &element_type, 0)
    }

    fn emit_local_array_selection_v1(
        &mut self,
        operations: &mut Vec<Operation>,
        index: ValueId,
        index_type: &Type,
        fields: &[SemanticValueBindingV1],
        element_type: &Type,
        start: usize,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let extra = fields
            .len()
            .checked_sub(1)
            .and_then(|value| value.checked_mul(3))
            .ok_or_else(|| {
                unsupported(
                    0,
                    None,
                    None,
                    "dynamic local array selection is empty or too large",
                )
            })?;
        enforce_limit(
            ProductionSemanticKirResourceV1::Operations,
            self.emitted_operations.saturating_add(extra),
            self.max_operations,
        )?;
        enforce_limit(
            ProductionSemanticKirResourceV1::Operations,
            operations.len().saturating_add(extra),
            MAX_BLOCK_OPERATIONS_V1,
        )?;
        operations.try_reserve(extra).map_err(|_| {
            ProductionSemanticKirErrorV1::AllocationFailure {
                resource: ProductionSemanticKirResourceV1::Operations,
            }
        })?;
        if fields.len() == 1 {
            return Ok(fields[0].clone());
        }
        let midpoint = fields.len() / 2;
        let split = start
            .checked_add(midpoint)
            .ok_or_else(|| unsupported(0, None, None, "dynamic local array midpoint overflow"))?;
        let left = self
            .emit_local_array_selection_v1(
                operations,
                index,
                index_type,
                &fields[..midpoint],
                element_type,
                start,
            )?
            .value()
            .map_err(|detail| unsupported(0, None, None, detail))?
            .0;
        let right = self
            .emit_local_array_selection_v1(
                operations,
                index,
                index_type,
                &fields[midpoint..],
                element_type,
                split,
            )?
            .value()
            .map_err(|detail| unsupported(0, None, None, detail))?
            .0;
        let bound = self.emit_id(
            operations,
            index_type.clone(),
            OperationKind::Constant(integer_constant(index_type, split as u128)?),
        )?;
        let condition = self.emit_compare(operations, ComparePredicate::LessThan, index, bound)?;
        self.emit(
            operations,
            element_type.clone(),
            OperationKind::Select {
                condition,
                true_value: left,
                false_value: right,
            },
        )
    }
}
