/// Inert result for one original source occurrence and its checked O placement.
/// Absence of a retained source slot and omission of an unreachable effect are
/// different outcomes; neither is a replacement source or executable proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceOutputPrivateArrayAccessV1 {
    /// The unchanged source query proved that this occurrence is not retained.
    ProvenUnretained,
    /// A validated retained N write has no O placement and is checked unreachable.
    OmittedUnreachable,
    /// The actual checked O write, with reachability independent of placement.
    Retained {
        /// Original exact source index, also checked against the actual O offset.
        index: u64,
        /// Actual O physical access coordinate, never an old N span ordinal.
        coordinate: fe2o3_kernel_ir::CanonicalKirAccessCoordinateV1,
        /// Whether checked execution can reach the original source effect.
        executable: bool,
    },
}

#[derive(Clone, Copy, Debug)]
struct SourceOutputArrayAnchorsV1 {
    allocation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    gep: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    memory: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    uses: [fe2o3_kernel_analysis::CanonicalKirOutputUseV1; 5],
    executable: bool,
}

#[derive(Clone, Copy, Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "Fixed-size Copy rows keep the prepaid contiguous payload free of per-row allocations"
)]
enum SourceOutputArrayPlacementV1 {
    Unsupported,
    OmittedUnreachable,
    Retained(SourceOutputArrayAnchorsV1),
}

#[derive(Clone, Copy, Debug)]
struct SourceOutputArrayRowV1 {
    key: [u32; 7],
    original_effect: usize,
    original_slot: usize,
    placement: SourceOutputArrayPlacementV1,
}

fn source_output_array_key_v1(
    owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    block: u32,
    statement: u32,
    role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
    component: u32,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<[u32; 7], ProductionSourceOutputErrorV1> {
    budget
        .charge_work(2)
        .map_err(ProductionSourceOutputErrorV1::Resource)?;
    let (class, ordinal) = private_array_role_key_v1(role).ok_or(
        ProductionSourceOutputErrorV1::Invalid("unsupported array source role"),
    )?;
    Ok([
        owner.index(),
        function.index(),
        block,
        statement,
        u32::from(class),
        ordinal,
        component,
    ])
}

fn source_output_array_coordinate_v1(
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    location: PrivateArrayPhysicalLocationV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1, ProductionSourceOutputErrorV1> {
    use fe2o3_kernel_ir::{CanonicalKirBlockCoordinateV1, CanonicalKirOperationCoordinateV1};
    budget
        .charge_work(2)
        .map_err(ProductionSourceOutputErrorV1::Resource)?;
    Ok(CanonicalKirOperationCoordinateV1 {
        block: CanonicalKirBlockCoordinateV1 {
            function,
            block: u32::try_from(location.block_ordinal).map_err(|_| {
                ProductionSourceOutputErrorV1::Invalid("array block ordinal overflow")
            })?,
        },
        operation: u32::try_from(location.operation).map_err(|_| {
            ProductionSourceOutputErrorV1::Invalid("array operation ordinal overflow")
        })?,
    })
}

fn source_output_array_use_operation_v1(
    used: fe2o3_kernel_analysis::CanonicalKirOutputUseV1,
    expected_operand: u32,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1, ProductionSourceOutputErrorV1> {
    budget
        .charge_work(2)
        .map_err(ProductionSourceOutputErrorV1::Resource)?;
    match used.coordinate {
        fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand { operation, operand }
            if operand == expected_operand =>
        {
            Ok(operation)
        }
        _ => Err(ProductionSourceOutputErrorV1::Invalid(
            "array output operand occurrence changed",
        )),
    }
}

// The header must be live before row allocation and remain reserved across the
// following catalog phase. The enclosing constructor owns failure cleanup.
fn source_output_array_rows_with_header_v1(
    source: &ProductionPreRankedKirOwnerV1,
    control: &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(Vec<SourceOutputArrayRowV1>, usize, usize), ProductionSourceOutputErrorV1> {
    budget
        .charge_work(1)
        .map_err(ProductionSourceOutputErrorV1::Resource)?;
    let header = std::mem::size_of::<Vec<SourceOutputArrayRowV1>>();
    budget
        .reserve_storage(header)
        .map_err(ProductionSourceOutputErrorV1::Resource)?;
    let (rows, payload) = source_output_array_rows_v1(source, control, budget)?;
    Ok((rows, payload, header))
}

// Called only inside the source-qualified constructor after exact N/B custody
// and the independently checked B/O control index. No inert rows enter here.
fn source_output_array_rows_v1(
    source: &ProductionPreRankedKirOwnerV1,
    control: &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(Vec<SourceOutputArrayRowV1>, usize), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand;
    budget.charge_work(2).map_err(Error::Resource)?;
    let original = &source.correspondence.private_arrays;
    let count = original.effects.len();
    let payload = count
        .checked_mul(std::mem::size_of::<SourceOutputArrayRowV1>())
        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
    budget.reserve_storage(payload).map_err(Error::Resource)?;
    let mut rows = Vec::new();
    budget.charge_work(1).map_err(Error::Resource)?;
    rows.try_reserve_exact(count)
        .map_err(|_| Error::Resource(AssertOriginResourceV1::Allocation))?;
    for instance in &original.instances {
        budget.charge_work(4).map_err(Error::Resource)?;
        let function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
            u32::try_from(instance.module_function_ordinal)
                .map_err(|_| Error::Invalid("array function ordinal overflow"))?,
        );
        let effects = original
            .effects
            .get(instance.effect_start..instance.effect_end)
            .ok_or(Error::Invalid("array effect range changed"))?;
        let slots = original
            .slots
            .get(instance.slot_start..instance.slot_end)
            .ok_or(Error::Invalid("array slot range changed"))?;
        for (ordinal, effect) in effects.iter().enumerate() {
            budget.charge_work(5).map_err(Error::Resource)?;
            if effect.owner != instance.owner || effect.function != instance.function {
                return Err(Error::Invalid("array source instance changed"));
            }
            let original_effect = instance
                .effect_start
                .checked_add(ordinal)
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            if rows.len() >= count || rows.len() == rows.capacity() {
                return Err(Error::Invalid("array placement row admission changed"));
            }
            let key = source_output_array_key_v1(
                effect.owner,
                effect.function,
                effect.semantic_block,
                effect.semantic_statement,
                effect.role,
                effect.original_index.component(),
                budget,
            )?;
            let slot_index = private_array_binary_search_v1(
                slots,
                |slot| [slot.local as usize],
                [effect.local as usize],
                &mut PrivateArrayQueryWorkV1 { budget },
            )
            .map_err(Error::PrivateArray)?
            .map_err(|_| Error::Invalid("array placement source slot is absent"))?;
            budget.charge_work(4).map_err(Error::Resource)?;
            let slot = &slots[slot_index];
            let original_slot = instance
                .slot_start
                .checked_add(slot_index)
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            if slot.owner != effect.owner || slot.function != effect.function {
                return Err(Error::Invalid(
                    "array placement source slot identity changed",
                ));
            }
            budget.charge_work(1).map_err(Error::Resource)?;
            let placement = if effect.access != PrivateArrayAccessV1::Write {
                SourceOutputArrayPlacementV1::Unsupported
            } else {
                let allocation =
                    source_output_array_coordinate_v1(function, slot.alloca_location, budget)?;
                let gep = source_output_array_coordinate_v1(function, effect.gep_location, budget)?;
                let memory =
                    source_output_array_coordinate_v1(function, effect.memory_location, budget)?;
                let state = control
                    .block(memory.block, budget)
                    .map_err(Error::Transition)?;
                let mut uses = [None; 5];
                for (index, (operation, operand)) in [
                    (allocation, 0),
                    (gep, 0),
                    (gep, 1),
                    (memory, 0),
                    (memory, 1),
                ]
                .into_iter()
                .enumerate()
                {
                    budget.charge_work(1).map_err(Error::Resource)?;
                    uses[index] = control
                        .operand(OperationOperand { operation, operand }, budget)
                        .map_err(Error::Transition)?;
                }
                budget.charge_work(6).map_err(Error::Resource)?;
                match uses {
                    [
                        Some(count),
                        Some(base),
                        Some(offset),
                        Some(pointer),
                        Some(value),
                    ] => {
                        let allocation = source_output_array_use_operation_v1(count, 0, budget)?;
                        let gep = source_output_array_use_operation_v1(base, 0, budget)?;
                        let offset_operation =
                            source_output_array_use_operation_v1(offset, 1, budget)?;
                        let memory = source_output_array_use_operation_v1(pointer, 0, budget)?;
                        let value_operation =
                            source_output_array_use_operation_v1(value, 1, budget)?;
                        // Two operation triples, two function IDs, and the
                        // placement option plus its output block pair.
                        budget.charge_work(11).map_err(Error::Resource)?;
                        if gep != offset_operation
                            || memory != value_operation
                            || allocation.block.function != memory.block.function
                            || gep.block.function != memory.block.function
                            || state
                                .placement
                                .is_none_or(|placement| placement.output != memory.block)
                        {
                            return Err(Error::Invalid("array output anchor placement changed"));
                        }
                        SourceOutputArrayPlacementV1::Retained(SourceOutputArrayAnchorsV1 {
                            allocation,
                            gep,
                            memory,
                            uses: [count, base, offset, pointer, value],
                            executable: state.reachable,
                        })
                    }
                    [_, None, None, None, None] if !state.reachable => {
                        SourceOutputArrayPlacementV1::OmittedUnreachable
                    }
                    _ => {
                        return Err(Error::Invalid(
                            "required array output operand placement is absent",
                        ));
                    }
                }
            };
            budget.charge_work(1).map_err(Error::Resource)?;
            rows.push(SourceOutputArrayRowV1 {
                key,
                original_effect,
                original_slot,
                placement,
            });
        }
    }
    budget.charge_work(1).map_err(Error::Resource)?;
    if rows.len() != count {
        return Err(Error::Invalid("array source placement census changed"));
    }
    source_output_array_check_keys_v1(&mut rows, budget)?;
    Ok((rows, payload))
}

fn source_output_array_check_keys_v1(
    rows: &mut [SourceOutputArrayRowV1],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    assert_origin_sort_v1(rows, budget, |a, b, budget| {
        budget.charge_work(7)?;
        Ok(a.key.cmp(&b.key))
    })
    .map_err(Error::SourceOrigin)?;
    for pair in rows.windows(2) {
        budget.charge_work(7).map_err(Error::Resource)?;
        if pair[0].key == pair[1].key {
            return Err(Error::Invalid("duplicate array source occurrence"));
        }
    }
    Ok(())
}

fn source_output_operation_v1<'o>(
    output: &'o fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<&'o Operation, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(4).map_err(Error::Resource)?;
    output
        .module()
        .functions
        .get(coordinate.block.function.0 as usize)
        .and_then(|function| function.body.as_ref())
        .and_then(|body| body.blocks.get(coordinate.block.block as usize))
        .and_then(|block| block.operations.get(coordinate.operation as usize))
        .ok_or(Error::Invalid(
            "array output operation coordinate is absent",
        ))
}

fn source_output_definition_v1<'o>(
    output: &'o fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    coordinate: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(ValueId, &'o Type), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
    budget.charge_work(1).map_err(Error::Resource)?;
    match coordinate {
        Definition::Result { operation, result } => {
            let operation = source_output_operation_v1(output, operation, budget)?;
            budget.charge_work(1).map_err(Error::Resource)?;
            let result = operation
                .results
                .get(result as usize)
                .ok_or(Error::Invalid("array output definition result is absent"))?;
            Ok((result.id, &result.ty))
        }
        Definition::FunctionArgument { function, argument } => {
            budget.charge_work(4).map_err(Error::Resource)?;
            let function = output
                .module()
                .functions
                .get(function.0 as usize)
                .ok_or(Error::Invalid("array output argument function is absent"))?;
            let body = function
                .body
                .as_ref()
                .ok_or(Error::Invalid("array output argument body is absent"))?;
            let value = body
                .parameters
                .get(argument as usize)
                .ok_or(Error::Invalid("array output argument value is absent"))?;
            let ty = function
                .signature
                .parameters
                .get(argument as usize)
                .ok_or(Error::Invalid("array output argument type is absent"))?;
            Ok((*value, ty))
        }
        Definition::BlockArgument { block, argument } => {
            budget.charge_work(4).map_err(Error::Resource)?;
            let value = output
                .module()
                .functions
                .get(block.function.0 as usize)
                .and_then(|function| function.body.as_ref())
                .and_then(|body| body.blocks.get(block.block as usize))
                .and_then(|block| block.parameters.get(argument as usize))
                .ok_or(Error::Invalid("array output block argument is absent"))?;
            Ok((value.id, &value.ty))
        }
    }
}

fn source_output_array_index_literal_v1(
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    coordinate: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    expected: u64,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<ValueId, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(2).map_err(Error::Resource)?;
    let fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::Result {
        operation,
        result: 0,
    } = coordinate
    else {
        return Err(Error::Invalid(
            "array output index is not an exact constant result",
        ));
    };
    let operation = source_output_operation_v1(output, operation, budget)?;
    // Result shape, fixed scalar type, constant kind and literal equality.
    budget.charge_work(4).map_err(Error::Resource)?;
    let [result] = operation.results.as_slice() else {
        return Err(Error::Invalid("array output index result count changed"));
    };
    if result.ty != Type::Scalar(ScalarType::Index)
        || !matches!(operation.kind, OperationKind::Constant(Constant::Index(value)) if value == expected)
    {
        return Err(Error::Invalid("array output index type or literal changed"));
    }
    Ok(result.id)
}

impl ProductionSourceOutputOccurrencesV1<'_, '_> {
    /// Checks original N source provenance and actual O array-write operands.
    /// Keep source G+A, bound B, checked O, and this view's transfer reserved on
    /// the same caller ledger. No original source index is replaced by an O ID.
    /// The result is inert and does not authorize final ranked/formal attachment.
    pub fn private_array_write(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<ProductionSourceOutputPrivateArrayAccessV1, ProductionSourceOutputErrorV1> {
        self.private_array_write_with_offset_v1(owner, function, site, role, budget)
            .map(|(outcome, _)| outcome)
    }

    // The offset was already proved by the unchanged source query. It is inert,
    // and retained internally so omitted writes do not require a second query.
    fn private_array_write_with_offset_v1(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<
        (ProductionSourceOutputPrivateArrayAccessV1, Option<u64>),
        ProductionSourceOutputErrorV1,
    > {
        use ProductionSourceOutputErrorV1 as Error;
        use ProductionSourceOutputPrivateArrayAccessV1 as Outcome;
        // Preserve the three-sum/floor precharge; source's immutable graph,
        // origin and helper subtotal was already checked at sealing. B's
        // separate owner receipt remains a caller precondition, as in B0.
        budget.charge_work(4).map_err(Error::Resource)?;
        let minimum = self
            .source
            .retained_analysis_storage_v1()
            .checked_add(self.checked_output.storage().retained_storage())
            .and_then(|n| n.checked_add(self.storage.retained_storage()))
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        if budget.storage() < minimum {
            return Err(Error::Resource(AssertOriginResourceV1::Accounting));
        }
        let Some(index) = self
            .source
            .materialized_private_array_constant_index(owner, function, site, role, budget)
            .map_err(Error::PrivateArray)?
        else {
            return Ok((Outcome::ProvenUnretained, None));
        };
        budget.charge_work(1).map_err(Error::Resource)?;
        let fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement { block, statement } =
            site
        else {
            return Err(Error::Invalid("array source statement is absent"));
        };
        let key =
            source_output_array_key_v1(owner, function, block.get(), statement, role, 0, budget)?;
        let ordinal = private_array_binary_search_v1(
            &self.private_arrays,
            |row| row.key.map(|value| value as usize),
            key.map(|value| value as usize),
            &mut PrivateArrayQueryWorkV1 { budget },
        )
        .map_err(Error::PrivateArray)?
        .map_err(|_| Error::Invalid("required source-qualified array output row is absent"))?;
        budget.charge_work(3).map_err(Error::Resource)?;
        let row = &self.private_arrays[ordinal];
        let original = &self.source.correspondence.private_arrays;
        let effect = original
            .effects
            .get(row.original_effect)
            .ok_or(Error::Invalid("array original effect is absent"))?;
        let slot = original
            .slots
            .get(row.original_slot)
            .ok_or(Error::Invalid("array original slot is absent"))?;
        let actual_key = source_output_array_key_v1(
            effect.owner,
            effect.function,
            effect.semantic_block,
            effect.semantic_statement,
            effect.role,
            effect.original_index.component(),
            budget,
        )?;
        budget.charge_work(10).map_err(Error::Resource)?;
        if actual_key != key
            || slot.owner != owner
            || slot.function != function
            || slot.local != effect.local
        {
            return Err(Error::Invalid("array original occurrence identity changed"));
        }
        budget.charge_work(1).map_err(Error::Resource)?;
        let anchors = match row.placement {
            SourceOutputArrayPlacementV1::Unsupported => {
                return Err(Error::PrivateArray(
                    SemanticKirPrivateArrayQueryErrorV1::Incomplete(
                        "checked output currently requires an ordinary private-array write",
                    ),
                ));
            }
            SourceOutputArrayPlacementV1::OmittedUnreachable => {
                return Ok((Outcome::OmittedUnreachable, Some(index)));
            }
            SourceOutputArrayPlacementV1::Retained(anchors) => anchors,
        };
        self.private_array_retained_write_v1(slot, anchors, index, budget)
            .map(|outcome| (outcome, Some(index)))
    }

    fn private_array_retained_write_v1(
        &self,
        slot: &PrivateArraySlotV1,
        anchors: SourceOutputArrayAnchorsV1,
        index: u64,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<ProductionSourceOutputPrivateArrayAccessV1, ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        use ProductionSourceOutputPrivateArrayAccessV1 as Outcome;
        use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
        // Two result definitions (tag, operation triple, result ordinal),
        // one block pair, and the checked adjacency addition/comparison.
        budget.charge_work(14).map_err(Error::Resource)?;
        if anchors.uses[1].definition
            != (Definition::Result {
                operation: anchors.allocation,
                result: 0,
            })
            || anchors.uses[3].definition
                != (Definition::Result {
                    operation: anchors.gep,
                    result: 0,
                })
            || anchors.gep.block != anchors.memory.block
            || anchors.gep.operation.checked_add(1) != Some(anchors.memory.operation)
        {
            return Err(Error::Invalid(
                "array output pointer ancestry or adjacency changed",
            ));
        }
        let output = self.output();
        let count = source_output_array_index_literal_v1(
            output,
            anchors.uses[0].definition,
            slot.length,
            budget,
        )?;
        let offset = source_output_array_index_literal_v1(
            output,
            anchors.uses[2].definition,
            index,
            budget,
        )?;
        let (base, _) = source_output_definition_v1(output, anchors.uses[1].definition, budget)?;
        let (pointer, _) = source_output_definition_v1(output, anchors.uses[3].definition, budget)?;
        let (value, value_type) =
            source_output_definition_v1(output, anchors.uses[4].definition, budget)?;
        let allocation = source_output_operation_v1(output, anchors.allocation, budget)?;
        private_array_check_allocation_operation_v1(
            allocation,
            base,
            count,
            slot.element_facts,
            &mut PrivateArrayQueryWorkV1 { budget },
        )
        .map_err(private_array_query_error_v1)
        .map_err(Error::PrivateArray)?;
        let gep = source_output_operation_v1(output, anchors.gep, budget)?;
        let mut work = PrivateArrayQueryWorkV1 { budget };
        work.charge_private_array_work(5)
            .map_err(Error::PrivateArray)?;
        let [result] = gep.results.as_slice() else {
            return Err(Error::Invalid("array output GEP result count changed"));
        };
        if result.id != pointer
            || !matches!(gep.kind, OperationKind::GetElementPointer { base: actual_base, offset: actual_offset }
                if actual_base == base && actual_offset == offset)
            || !private_array_pointer_matches_v1(&result.ty, slot.element_facts.element, &mut work)
                .map_err(Error::PrivateArray)?
        {
            return Err(Error::Invalid(
                "array output GEP pointer, index or type changed",
            ));
        }
        let memory = source_output_operation_v1(output, anchors.memory, &mut *work.budget)?;
        private_array_check_memory_operation_v1(
            memory,
            PrivateArrayAccessV1::Write,
            pointer,
            slot.element_facts,
            &mut work,
        )
        .map_err(private_array_query_error_v1)
        .map_err(Error::PrivateArray)?;
        work.charge_private_array_work(2)
            .map_err(Error::PrivateArray)?;
        if !matches!(memory.kind, OperationKind::Store { value: actual, .. } if actual == value)
            || !slot
                .element_facts
                .element
                .matches_borrowed(value_type, &mut work)
                .map_err(Error::PrivateArray)?
        {
            return Err(Error::Invalid(
                "array output stored-value ancestry or type changed",
            ));
        }
        Ok(Outcome::Retained {
            index,
            coordinate: fe2o3_kernel_ir::CanonicalKirAccessCoordinateV1 {
                operation: anchors.memory,
                effect: 0,
            },
            executable: anchors.executable,
        })
    }
}

include!("production_source_output_initializer_v1.rs");

include!("production_source_output_private_array_ranked_v1.rs");
