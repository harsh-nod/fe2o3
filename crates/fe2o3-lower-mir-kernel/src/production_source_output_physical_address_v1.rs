type SourceOutputAddressOpV1 = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1;
type SourceOutputAddressDefV1 = fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1;
type SourceOutputAddressUseV1 = fe2o3_kernel_ir::CanonicalKirUseCoordinateV1;

#[derive(Clone, Copy)]
struct SourceOutputPhysicalAddressRowV1 {
    access: usize,
    operation: SourceOutputAddressOpV1,
    pointer: SourceOutputAddressDefV1,
    gep: SourceOutputAddressOpV1,
    offset: SourceOutputAddressDefV1,
    allocation: SourceOutputAddressDefV1,
    extent: ProductionRankedValueV1,
    element_bytes: u32,
    alignment: u32,
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct SourceOutputIdentityPhysicalKeyV1 {
    ranked_index: ProductionRankedValueV1,
    ranked_extent: ProductionRankedValueV1,
    source_slice: SemanticLocalIdV1,
    output_slice: ValueId,
    pointer: SourceOutputAddressDefV1,
    pointer_value: Option<ValueId>,
    gep: SourceOutputAddressOpV1,
    offset: SourceOutputAddressDefV1,
    allocation: SourceOutputAddressDefV1,
}

fn source_output_identity_physical_key_v1(
    sealed: SourceOutputIdentityPhysicalKeyV1,
    actual: SourceOutputIdentityPhysicalKeyV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(16).map_err(Error::Resource)?;
    if sealed != actual {
        return Err(Error::Invalid(
            "physical identity getter own address differs",
        ));
    }
    Ok(())
}

/// Scoped correspondence of ordinary physical O addresses to the same checked
/// memory projection. This is not bounds, alias, overflow, race, formal-memory,
/// reference or native-admission proof. The current grammar is contiguous
/// scalar Global slices with one GEP and U64/INDEX leaves, plus already checked
/// private C accesses. Nonidentity integer casts and arithmetic remain refused.
///
/// No constructor, serialization, Clone or legacy receipt conversion exists.
/// Caller-owned ledger history and live owners must not be replaced in place.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionPhysicalAddressRelationV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ProductionPhysicalAddressRelationV1<'static, 'static, 'static>>();
/// ```
pub struct ProductionPhysicalAddressRelationV1<'scope, 'source, 'output> {
    analysis: &'scope ProductionScopedCanonicalStoreAnalysisV1<'scope, 'source, 'output>,
    rows: &'scope [SourceOutputPhysicalAddressRowV1],
    private_accesses: usize,
    floor: usize,
}

/// An immutable borrow of one exact physical address association. Copied
/// coordinates are descriptive, not detachable proof or runtime allocation IDs.
pub struct ProductionPhysicalAddressV1<'scope> {
    row: &'scope SourceOutputPhysicalAddressRowV1,
}

impl ProductionPhysicalAddressV1<'_> {
    /// Ordinal in the completed source/O memory partition.
    pub const fn access_ordinal(&self) -> usize {
        self.row.access
    }
    /// Exact actual O ordinary Load or Store, physical effect ordinal zero.
    pub const fn operation(&self) -> SourceOutputAddressOpV1 {
        self.row.operation
    }
    /// Actual definition consumed by the memory operation's pointer operand.
    pub const fn pointer_definition(&self) -> SourceOutputAddressDefV1 {
        self.row.pointer
    }
    /// Exact actual O GEP, not the guard Compare operation.
    pub const fn gep(&self) -> SourceOutputAddressOpV1 {
        self.row.gep
    }
    /// Exact GEP operand one occurrence, in element-index units.
    pub const fn offset_use(&self) -> SourceOutputAddressUseV1 {
        SourceOutputAddressUseV1::OperationOperand {
            operation: self.row.gep,
            operand: 1,
        }
    }
    /// Actual definition consumed by this GEP offset occurrence.
    pub const fn offset_definition(&self) -> SourceOutputAddressDefV1 {
        self.row.offset
    }
    /// Actual Slice formal defining the allocation, never a concrete allocation ID.
    pub const fn allocation(&self) -> SourceOutputAddressDefV1 {
        self.row.allocation
    }
    /// Ranked extent matched to that same Slice's length component.
    pub const fn ranked_extent(&self) -> ProductionRankedValueV1 {
        self.row.extent
    }
    /// Pointee width in bytes; ranked view widths are independently checked bits.
    pub const fn element_bytes(&self) -> u32 {
        self.row.element_bytes
    }
    /// Actual O memory payload alignment, not an authenticated runtime guarantee.
    pub const fn alignment(&self) -> u32 {
        self.row.alignment
    }
}

impl ProductionPhysicalAddressRelationV1<'_, '_, '_> {
    /// Exact checked O borrowed by the completed analysis.
    pub fn output(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        self.analysis.output()
    }
    /// Number of Global scalar-slice address associations.
    pub fn global_access_count(&self) -> usize {
        self.rows.len()
    }
    /// Number of separately consumed existing private C allocation/index rules.
    pub const fn private_access_count(&self) -> usize {
        self.private_accesses
    }
    /// Paid lookup on the original live ledger; no memory projection escapes.
    pub fn access(
        &self,
        ordinal: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Option<ProductionPhysicalAddressV1<'_>>, ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        self.analysis.require_live_v1(budget)?;
        budget.charge_work(2).map_err(Error::Resource)?;
        if budget.storage() < self.floor {
            return Err(Error::Resource(AssertOriginResourceV1::Accounting));
        }
        let row = assert_origin_find_v1(self.rows, budget, |row, budget| {
            budget.charge_work(1)?;
            Ok(row.access.cmp(&ordinal))
        })
        .map_err(Error::SourceOrigin)?;
        Ok(row.map(|row| ProductionPhysicalAddressV1 {
            row: &self.rows[row],
        }))
    }
}

#[derive(Clone, Copy)]
struct SourceOutputAddressRankedDefinitionV1 {
    value: ProductionRankedValueIdV1,
    block: u32,
    operation: u32,
}

#[derive(Default)]
struct SourceOutputAddressWorkspaceV1 {
    rows: Vec<SourceOutputPhysicalAddressRowV1>,
    ranked: Vec<SourceOutputAddressRankedDefinitionV1>,
    pointers: Vec<SourceOutputAddressDefV1>,
    allowed_uses: Vec<(SourceOutputAddressDefV1, SourceOutputAddressUseV1)>,
}

fn source_output_address_u64_v1() -> ProductionSemanticScalarTypeV2 {
    ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 64,
    }
}

fn source_output_address_shape_v1(
    shape: &[u64],
    extents: &[ProductionRankedValueV1],
    width: u32,
    bytes: u32,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<ProductionRankedValueV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(7).map_err(Error::Resource)?;
    if shape != [dialect_kernel::DYNAMIC_EXTENT]
        || extents.len() != 1
        || bytes == 0
        || bytes.checked_mul(8) != Some(width)
    {
        return Err(Error::Invalid(
            "physical address requires one exact scalar slice extent and width",
        ));
    }
    Ok(extents[0])
}

fn source_output_address_operation_v1<'a, 'o>(
    inventory: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV1<'o>,
    function: &fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>,
    coordinate: SourceOutputAddressOpV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<&'a fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'o>, ProductionSourceOutputErrorV1>
{
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(2).map_err(Error::Resource)?;
    if coordinate.block.function != function.coordinate {
        return Err(Error::Invalid("physical address function differs"));
    }
    let rows = inventory
        .operations()
        .get(function.operations.clone())
        .ok_or(Error::Invalid("physical operation range absent"))?;
    let index = assert_origin_find_v1(rows, budget, |row, budget| {
        budget.charge_work(2)?;
        Ok(row.coordinate.cmp(&coordinate))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid("physical operation absent"))?;
    Ok(&rows[index])
}

fn source_output_address_operand_v1<'a, 'o>(
    inventory: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV1<'o>,
    operation: &fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>,
    operand: u32,
    value: ValueId,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<&'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'o>, ProductionSourceOutputErrorV1>
{
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(7).map_err(Error::Resource)?;
    let used = inventory
        .uses()
        .get(operation.operands.clone())
        .and_then(|uses| uses.get(operand as usize))
        .ok_or(Error::Invalid("physical operand occurrence absent"))?;
    let expected = SourceOutputAddressUseV1::OperationOperand {
        operation: operation.coordinate,
        operand,
    };
    let definition = inventory
        .definitions()
        .get(used.definition)
        .ok_or(Error::Invalid("physical operand definition absent"))?;
    if used.coordinate != expected || used.value != value || definition.value != Some(value) {
        return Err(Error::Invalid(
            "physical operand occurrence or definition differs",
        ));
    }
    Ok(definition)
}

fn source_output_address_ranked_index_v1(
    work: &mut SourceOutputAddressWorkspaceV1,
    lowering: &ProductionRankedKernelLoweringInputV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    for (block, body) in lowering.kernel().blocks().iter().enumerate() {
        budget.charge_work(1).map_err(Error::Resource)?;
        for (operation, row) in body.operations().iter().enumerate() {
            budget.charge_work(3).map_err(Error::Resource)?;
            let (ProductionRankedOperationV1::ViewInSpace { result, .. }
            | ProductionRankedOperationV1::IndexConstant { result, .. }
            | ProductionRankedOperationV1::IndexUnknown { result }
            | ProductionRankedOperationV1::InvocationIndex {
                result,
                dimension: 0,
                launch_extent: 0,
            }
            | ProductionRankedOperationV1::IndexUnsignedCast { result, .. }) = row
            else {
                continue;
            };
            let block = u32::try_from(block)
                .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            let operation = u32::try_from(operation)
                .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            assert_origin_push_v1(
                &mut work.ranked,
                SourceOutputAddressRankedDefinitionV1 {
                    value: *result,
                    block,
                    operation,
                },
                budget,
            )
            .map_err(Error::SourceOrigin)?;
        }
    }
    source_output_ranked_sort_unique_v1(&mut work.ranked, budget, |a, b| a.value.cmp(&b.value))
}

fn source_output_address_ranked_operation_v1<'a>(
    rows: &[SourceOutputAddressRankedDefinitionV1],
    lowering: &'a ProductionRankedKernelLoweringInputV1,
    value: ProductionRankedValueV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<&'a ProductionRankedOperationV1, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(1).map_err(Error::Resource)?;
    let ProductionRankedValueV1::Local(value) = value else {
        return Err(Error::Invalid("physical ranked definition is not local"));
    };
    let row = assert_origin_find_v1(rows, budget, |row, budget| {
        budget.charge_work(1)?;
        Ok(row.value.cmp(&value))
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid(
        "physical ranked definition unsupported or absent",
    ))?;
    budget.charge_work(2).map_err(Error::Resource)?;
    let row = rows[row];
    lowering
        .kernel()
        .blocks()
        .get(row.block as usize)
        .and_then(|block| block.operations().get(row.operation as usize))
        .ok_or(Error::Invalid("physical ranked operation absent"))
}

fn source_output_address_leaf_node_v1(
    leaf: SourceOutputAddressLeafV1<'_>,
    mut paid: impl FnMut(usize) -> Result<(), ProductionSourceOutputErrorV1>,
) -> Result<NormalizedScalarNodeV1<usize>, ProductionSourceOutputErrorV1> {
    use NormalizedScalarNodeV1 as Node;
    use ProductionSourceOutputErrorV1 as Error;
    use SourceOutputAddressLeafV1 as Leaf;
    Ok(match leaf {
        Leaf::Formal(formal) => {
            if formal.scalar() != source_output_address_u64_v1() {
                return Err(Error::Invalid("physical index formal width unsupported"));
            }
            let SourceOutputAddressDefV1::FunctionArgument { argument, .. } = formal.output()
            else {
                return Err(Error::Invalid("physical index formal definition differs"));
            };
            paid(4)?;
            let slot = argument
                .checked_mul(2)
                .and_then(|slot| {
                    slot.checked_add(u32::from(
                        formal.component() == ProductionProjectionArgumentComponentV1::SliceLength,
                    ))
                })
                .and_then(|slot| PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2.checked_add(slot))
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            Node::Symbol {
                symbol: slot,
                scalar: source_output_address_u64_v1(),
            }
        }
        Leaf::Literal(literal) => {
            if literal.scalar() != source_output_address_u64_v1() {
                return Err(Error::Invalid("physical index literal width unsupported"));
            }
            Node::Constant {
                scalar: literal.scalar(),
                bits: literal.bits(),
            }
        }
        Leaf::Invocation(row, invocation) => {
            paid(4)?;
            if row.scalar != source_output_address_u64_v1()
                || row.component != ProductionProjectionArgumentComponentV1::Scalar
            {
                return Err(Error::Invalid("physical invocation scalar differs"));
            }
            Node::Symbol {
                symbol: PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2
                    .checked_add(invocation.symbol)
                    .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                scalar: row.scalar,
            }
        }
    })
}

fn source_output_address_ranked_leaf_v1(
    analysis: &ProductionScopedCanonicalStoreAnalysisV1<'_, '_, '_>,
    rows: &[SourceOutputAddressRankedDefinitionV1],
    mut value: ProductionRankedValueV1,
    context: &mut SourceOutputControlNormalizationV1<'_, '_, '_, '_, '_>,
) -> Result<usize, ProductionSourceOutputErrorV1> {
    use NormalizedScalarNodeV1 as Node;
    use ProductionSourceOutputErrorV1 as Error;
    for _ in 0..=MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        context
            .inner
            .paid(2)
            .ok_or_else(|| context.inner.failure())?;
        let leaf = analysis.control.address_leaf_v1(
            analysis.view,
            analysis.candidate,
            analysis.control_ordinal,
            value,
            context.inner.budget,
        )?;
        let node = match leaf {
            Some(leaf) => source_output_address_leaf_node_v1(leaf, |units| {
                context
                    .inner
                    .paid(units)
                    .ok_or_else(|| context.inner.failure())
            })?,
            None => match source_output_address_ranked_operation_v1(
                rows,
                analysis.candidate.lowering,
                value,
                context.inner.budget,
            )? {
                ProductionRankedOperationV1::IndexConstant { value, .. } => Node::Constant {
                    scalar: source_output_address_u64_v1(),
                    bits: *value,
                },
                ProductionRankedOperationV1::IndexUnsignedCast {
                    source,
                    bit_width: 64,
                    ..
                } => {
                    value = *source;
                    continue;
                }
                _ => return Err(Error::Invalid("physical ranked index grammar unsupported")),
            },
        };
        return context.emit(node).ok_or_else(|| context.inner.failure());
    }
    Err(Error::Invalid("physical ranked index depth exceeded"))
}

// Restrict the existing shared normalizer to the approved leaf/identity-cast
// subset. This inspects representation only and never evaluates arithmetic.
fn source_output_address_scalar_grammar_v1(
    mut value: ValueId,
    context: &mut SourceOutputControlNormalizationV1<'_, '_, '_, '_, '_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    for _ in 0..=MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        let definition = context
            .inner
            .inventory
            .definition_for_value(
                context.inner.function.coordinate,
                value,
                context.inner.budget,
            )
            .map_err(Error::Inventory)?
            .ok_or(Error::Invalid("physical scalar definition absent"))?;
        context
            .inner
            .budget
            .charge_work(4)
            .map_err(Error::Resource)?;
        if kir_semantic_scalar_v1(definition.ty) != Some(source_output_address_u64_v1()) {
            return Err(Error::Invalid("physical scalar is not U64 or INDEX"));
        }
        if !context.invocation_roots.is_empty()
            && assert_origin_find_v1(
                context.invocation_roots,
                context.inner.budget,
                |row, budget| {
                    budget.charge_work(4)?;
                    Ok(row.0.cmp(&definition.coordinate))
                },
            )
            .map_err(Error::SourceOrigin)?
            .is_some()
        {
            return Ok(());
        }
        let SourceOutputAddressDefV1::Result {
            operation,
            result: 0,
        } = definition.coordinate
        else {
            return if matches!(
                definition.coordinate,
                SourceOutputAddressDefV1::FunctionArgument { .. }
            ) {
                Ok(())
            } else {
                Err(Error::Invalid(
                    "physical scalar phi or result ordinal unsupported",
                ))
            };
        };
        let row = source_output_address_operation_v1(
            context.inner.inventory,
            context.inner.function,
            operation,
            context.inner.budget,
        )?;
        context
            .inner
            .budget
            .charge_work(3)
            .map_err(Error::Resource)?;
        if row.operation.results.len() != 1 || row.operation.results[0].id != value {
            return Err(Error::Invalid("physical scalar result differs"));
        }
        match &row.operation.kind {
            OperationKind::Constant(constant) => {
                return if normalize_kir_constant_v1(constant)
                    .is_some_and(|(ty, _)| ty == source_output_address_u64_v1())
                {
                    Ok(())
                } else {
                    Err(Error::Invalid("physical constant type differs"))
                };
            }
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: source,
                to,
            } if kir_semantic_scalar_v1(to) == Some(source_output_address_u64_v1()) => {
                source_output_address_operand_v1(
                    context.inner.inventory,
                    row,
                    0,
                    *source,
                    context.inner.budget,
                )?;
                value = *source;
            }
            OperationKind::SliceLength { slice } if row.operation.results[0].ty == Type::INDEX => {
                let definition = source_output_address_operand_v1(
                    context.inner.inventory,
                    row,
                    0,
                    *slice,
                    context.inner.budget,
                )?;
                return if matches!(
                    definition.coordinate,
                    SourceOutputAddressDefV1::FunctionArgument { .. }
                ) && matches!(definition.ty, Type::Slice(_))
                {
                    Ok(())
                } else {
                    Err(Error::Invalid(
                        "physical metadata lacks immutable Slice formal",
                    ))
                };
            }
            _ => {
                return Err(Error::Invalid(
                    "physical scalar arithmetic or ancestry unsupported",
                ));
            }
        }
    }
    Err(Error::Invalid("physical scalar depth exceeded"))
}

fn source_output_address_compare_v1(
    analysis: &ProductionScopedCanonicalStoreAnalysisV1<'_, '_, '_>,
    rows: &[SourceOutputAddressRankedDefinitionV1],
    actual: ValueId,
    projected: ProductionRankedValueV1,
    context: &mut SourceOutputControlNormalizationV1<'_, '_, '_, '_, '_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    context
        .inner
        .paid(2)
        .ok_or_else(|| context.inner.failure())?;
    context.inner.nodes.clear();
    context.inner.normalization_steps = 0;
    source_output_address_scalar_grammar_v1(actual, context)?;
    let actual = normalize_kir_expression_core_v1(actual, 0, context)
        .ok_or_else(|| context.inner.failure())?;
    let projected = source_output_address_ranked_leaf_v1(analysis, rows, projected, context)?;
    if !context
        .inner
        .equivalent(actual, projected)
        .ok_or_else(|| context.inner.failure())?
    {
        return Err(Error::Invalid(
            "physical O offset differs from ranked index",
        ));
    }
    Ok(())
}

fn source_output_address_allow_v1(
    work: &mut SourceOutputAddressWorkspaceV1,
    definition: SourceOutputAddressDefV1,
    used: SourceOutputAddressUseV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    assert_origin_push_v1(&mut work.pointers, definition, budget)
        .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?;
    assert_origin_push_v1(&mut work.allowed_uses, (definition, used), budget)
        .map_err(ProductionSourceOutputErrorV1::SourceOrigin)
}

fn source_output_address_check_consumers_v1(
    work: &mut SourceOutputAddressWorkspaceV1,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
    function: &fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    assert_origin_sort_v1(&mut work.pointers, budget, |a, b, budget| {
        budget.charge_work(1)?;
        Ok(a.cmp(b))
    })
    .map_err(Error::SourceOrigin)?;
    assert_origin_sort_v1(&mut work.allowed_uses, budget, |a, b, budget| {
        budget.charge_work(2)?;
        Ok(a.cmp(b))
    })
    .map_err(Error::SourceOrigin)?;
    for used in &inventory.uses()[function.uses.clone()] {
        budget.charge_work(2).map_err(Error::Resource)?;
        let definition = inventory
            .definitions()
            .get(used.definition)
            .ok_or(Error::Invalid("physical consumer definition absent"))?
            .coordinate;
        if assert_origin_find_v1(&work.pointers, budget, |row, budget| {
            budget.charge_work(1)?;
            Ok(row.cmp(&definition))
        })
        .map_err(Error::SourceOrigin)?
        .is_none()
        {
            continue;
        }
        let key = (definition, used.coordinate);
        if assert_origin_find_v1(&work.allowed_uses, budget, |row, budget| {
            budget.charge_work(2)?;
            Ok(row.cmp(&key))
        })
        .map_err(Error::SourceOrigin)?
        .is_none()
        {
            return Err(Error::Invalid(
                "physical pointer has an unaccounted or escaping use",
            ));
        }
    }
    Ok(())
}

fn source_output_address_access_v1(
    analysis: &ProductionScopedCanonicalStoreAnalysisV1<'_, '_, '_>,
    ordinal: usize,
    work: &mut SourceOutputAddressWorkspaceV1,
    context: &mut SourceOutputControlNormalizationV1<'_, '_, '_, '_, '_>,
) -> Result<bool, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use SourceOutputAddressDefV1 as Def;
    use SourceOutputAddressUseV1 as Use;
    let access = analysis
        .access(ordinal, context.inner.budget)?
        .ok_or(Error::Invalid("physical access partition absent"))?;
    let row = source_output_address_operation_v1(
        context.inner.inventory,
        context.inner.function,
        access.operation(),
        context.inner.budget,
    )?;
    context
        .inner
        .budget
        .charge_work(5)
        .map_err(Error::Resource)?;
    let (pointer, payload, store) = match &row.operation.kind {
        OperationKind::Load { pointer, access } => (*pointer, *access, false),
        OperationKind::Store {
            pointer, access, ..
        } => (*pointer, *access, true),
        _ => {
            return Err(Error::Invalid(
                "physical address requires ordinary memory operation",
            ));
        }
    };
    if payload.address_space == AddressSpace::Private {
        return Ok(false);
    }
    if payload.address_space != AddressSpace::Global || payload.volatile {
        return Err(Error::Invalid(
            "physical address space or volatile access unsupported",
        ));
    }
    let memory_pointer = source_output_address_operand_v1(
        context.inner.inventory,
        row,
        0,
        pointer,
        context.inner.budget,
    )?;
    let Type::Pointer(pointer_type) = memory_pointer.ty else {
        return Err(Error::Invalid("physical memory pointer type absent"));
    };
    context
        .inner
        .budget
        .charge_work(5)
        .map_err(Error::Resource)?;
    let scalar = pointer_type
        .pointee
        .as_scalar()
        .ok_or(Error::Invalid("physical pointee is not scalar"))?;
    let bits = scalar
        .bit_width()
        .ok_or(Error::Invalid("physical scalar width absent"))?;
    if bits == 0
        || bits % 8 != 0
        || pointer_type.address_space != AddressSpace::Global
        || (store && pointer_type.access == AccessMode::ReadOnly)
        || (!store && pointer_type.access == AccessMode::WriteOnly)
    {
        return Err(Error::Invalid("physical pointer width or access differs"));
    }
    let bytes = u32::from(bits / 8);
    let ranked = analysis
        .candidate
        .lowering
        .kernel()
        .blocks()
        .get(access.source().ranked_block() as usize)
        .and_then(|block| {
            block
                .operations()
                .get(access.source().ranked_operation() as usize)
        })
        .ok_or(Error::Invalid("physical ranked access absent"))?;
    context
        .inner
        .budget
        .charge_work(4)
        .map_err(Error::Resource)?;
    let (kind, view, indices) = match ranked {
        ProductionRankedOperationV1::Access {
            kind,
            view,
            indices,
        }
        | ProductionRankedOperationV1::ValueAccess {
            kind,
            view,
            indices,
            ..
        } => (*kind, *view, indices),
        _ => return Err(Error::Invalid("physical ranked access grammar unsupported")),
    };
    if indices.len() != 1
        || kind
            != if store {
                dialect_kernel::AccessKindAttr::Write
            } else {
                dialect_kernel::AccessKindAttr::Read
            }
    {
        return Err(Error::Invalid(
            "physical ranked access kind or rank differs",
        ));
    }
    let ProductionRankedOperationV1::ViewInSpace {
        element_width,
        writable,
        shape,
        dynamic_extents,
        memory_space: dialect_kernel::MemorySpaceAttr::Global,
        ..
    } = source_output_address_ranked_operation_v1(
        &work.ranked,
        analysis.candidate.lowering,
        view,
        context.inner.budget,
    )?
    else {
        return Err(Error::Invalid("physical ranked Global view absent"));
    };
    if store && !writable {
        return Err(Error::Invalid("physical ranked view is read-only"));
    }
    let extent = source_output_address_shape_v1(
        shape,
        dynamic_extents,
        *element_width,
        bytes,
        context.inner.budget,
    )?;
    let Some(SourceOutputAddressLeafV1::Formal(length)) = analysis.control.address_leaf_v1(
        analysis.view,
        analysis.candidate,
        analysis.control_ordinal,
        extent,
        context.inner.budget,
    )?
    else {
        return Err(Error::Invalid(
            "physical extent is not checked Slice metadata",
        ));
    };
    context
        .inner
        .budget
        .charge_work(3)
        .map_err(Error::Resource)?;
    if length.component() != ProductionProjectionArgumentComponentV1::SliceLength
        || length.scalar() != source_output_address_u64_v1()
    {
        return Err(Error::Invalid("physical extent component differs"));
    }
    let mut current = memory_pointer;
    let mut used = Use::OperationOperand {
        operation: row.coordinate,
        operand: 0,
    };
    let mut gep = None;
    for _ in 0..=MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        context
            .inner
            .budget
            .charge_work(4)
            .map_err(Error::Resource)?;
        let Type::Pointer(ty) = current.ty else {
            return Err(Error::Invalid("physical chain is not a pointer"));
        };
        if ty.pointee.as_ref() != pointer_type.pointee.as_ref()
            || ty.address_space != pointer_type.address_space
        {
            return Err(Error::Invalid(
                "physical chain changes pointee or address space",
            ));
        }
        source_output_address_allow_v1(work, current.coordinate, used, context.inner.budget)?;
        let Def::Result {
            operation,
            result: 0,
        } = current.coordinate
        else {
            return Err(Error::Invalid("physical chain has unsupported origin"));
        };
        let definition = source_output_address_operation_v1(
            context.inner.inventory,
            context.inner.function,
            operation,
            context.inner.budget,
        )?;
        context
            .inner
            .budget
            .charge_work(2)
            .map_err(Error::Resource)?;
        if definition.operation.results.len() != 1
            || Some(definition.operation.results[0].id) != current.value
        {
            return Err(Error::Invalid("physical pointer result differs"));
        }
        match &definition.operation.kind {
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value,
                to,
            } if to == current.ty => {
                current = source_output_address_operand_v1(
                    context.inner.inventory,
                    definition,
                    0,
                    *value,
                    context.inner.budget,
                )?;
                used = Use::OperationOperand {
                    operation,
                    operand: 0,
                };
            }
            OperationKind::GetElementPointer { base, offset } => {
                if gep.is_some() {
                    return Err(Error::Invalid("physical nested GEP unsupported"));
                }
                let offset_definition = source_output_address_operand_v1(
                    context.inner.inventory,
                    definition,
                    1,
                    *offset,
                    context.inner.budget,
                )?;
                source_output_address_compare_v1(
                    analysis,
                    &work.ranked,
                    *offset,
                    indices[0],
                    context,
                )?;
                gep = Some((operation, offset_definition.coordinate));
                current = source_output_address_operand_v1(
                    context.inner.inventory,
                    definition,
                    0,
                    *base,
                    context.inner.budget,
                )?;
                used = Use::OperationOperand {
                    operation,
                    operand: 0,
                };
            }
            OperationKind::SliceData { slice } => {
                let allocation = source_output_address_operand_v1(
                    context.inner.inventory,
                    definition,
                    0,
                    *slice,
                    context.inner.budget,
                )?;
                context
                    .inner
                    .budget
                    .charge_work(7)
                    .map_err(Error::Resource)?;
                let Type::Slice(slice_type) = allocation.ty else {
                    return Err(Error::Invalid("physical SliceData operand is not Slice"));
                };
                if !matches!(allocation.coordinate, Def::FunctionArgument { .. })
                    || allocation.coordinate != length.output()
                    || allocation.value != Some(length.output_value())
                    || slice_type.element.as_ref() != pointer_type.pointee.as_ref()
                    || slice_type.address_space != pointer_type.address_space
                    || (store && slice_type.access == AccessMode::ReadOnly)
                    || (!store && slice_type.access == AccessMode::WriteOnly)
                {
                    return Err(Error::Invalid(
                        "physical allocation or extent source differs",
                    ));
                }
                let (gep, offset) =
                    gep.ok_or(Error::Invalid("physical slice access lacks one GEP"))?;
                if analysis.control.candidates[analysis.control_ordinal]
                    .identity_address
                    .is_some()
                {
                    let (identity, own) = analysis
                        .control
                        .identity_address_use_v1(
                            analysis.view,
                            analysis.candidate,
                            analysis.control_ordinal,
                            *access.source(),
                            row.coordinate,
                            context.inner.budget,
                        )?
                        .ok_or(Error::Invalid("physical identity own-use seal absent"))?;
                    source_output_identity_physical_key_v1(
                        SourceOutputIdentityPhysicalKeyV1 {
                            ranked_index: identity.ranked_index,
                            ranked_extent: identity.ranked_extent,
                            source_slice: identity.slice,
                            output_slice: identity.output_slice,
                            pointer: own.pointer.definition,
                            pointer_value: Some(own.pointer.value),
                            gep: own.gep,
                            offset: identity.invocation.output,
                            allocation: own.allocation,
                        },
                        SourceOutputIdentityPhysicalKeyV1 {
                            ranked_index: indices[0],
                            ranked_extent: extent,
                            source_slice: length.source_local(),
                            output_slice: length.output_value(),
                            pointer: memory_pointer.coordinate,
                            pointer_value: memory_pointer.value,
                            gep,
                            offset,
                            allocation: allocation.coordinate,
                        },
                        context.inner.budget,
                    )?;
                }
                assert_origin_push_v1(
                    &mut work.rows,
                    SourceOutputPhysicalAddressRowV1 {
                        access: ordinal,
                        operation: row.coordinate,
                        pointer: memory_pointer.coordinate,
                        gep,
                        offset,
                        allocation: allocation.coordinate,
                        extent,
                        element_bytes: bytes,
                        alignment: payload.alignment,
                    },
                    context.inner.budget,
                )
                .map_err(Error::SourceOrigin)?;
                return Ok(true);
            }
            _ => return Err(Error::Invalid("physical pointer derivation unsupported")),
        }
    }
    Err(Error::Invalid("physical pointer depth exceeded"))
}

impl<'source, 'output> ProductionScopedCanonicalStoreAnalysisV1<'_, 'source, 'output> {
    /// Checks a closed correspondence-only address subset on the same immutable
    /// O and the SAME source-anchored Control leaf map. Callback success is not
    /// formal discharge, safety, termination or final attachment. Scratch drops
    /// before the caller floor is restored on success, error and unwind.
    ///
    /// Neither the relation nor an access borrow can leave its query scope.
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
    /// use fe2o3_lower_mir_kernel::{
    ///     ProductionPhysicalAddressRelationV1, ProductionScopedCanonicalStoreAnalysisV1,
    ///     ProductionSourceOutputErrorV1,
    /// };
    /// fn escape<'a>(
    ///     analysis: &'a ProductionScopedCanonicalStoreAnalysisV1<'a, 'a, 'a>,
    ///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    /// ) -> Result<&'a ProductionPhysicalAddressRelationV1<'a, 'a, 'a>, ProductionSourceOutputErrorV1> {
    ///     analysis.with_physical_address_relation_v1(budget, |relation, _| Ok(relation))
    /// }
    /// ```
    ///
    /// ```compile_fail
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
    /// use fe2o3_lower_mir_kernel::{
    ///     ProductionPhysicalAddressV1, ProductionScopedCanonicalStoreAnalysisV1,
    ///     ProductionSourceOutputErrorV1,
    /// };
    /// fn escape<'a>(
    ///     analysis: &'a ProductionScopedCanonicalStoreAnalysisV1<'a, 'a, 'a>,
    ///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    /// ) -> Result<Option<ProductionPhysicalAddressV1<'a>>, ProductionSourceOutputErrorV1> {
    ///     analysis.with_physical_address_relation_v1(budget, |relation, budget| {
    ///         relation.access(0, budget)
    ///     })
    /// }
    /// ```
    pub fn with_physical_address_relation_v1<T>(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
        body: impl for<'scope> FnOnce(
            &ProductionPhysicalAddressRelationV1<'scope, 'source, 'output>,
            &mut AssertOriginBudgetV1<'_>,
        ) -> Result<T, ProductionSourceOutputErrorV1>,
    ) -> Result<T, ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        self.require_live_v1(budget)?;
        source_output_global_scratch_scope_v1(budget, |budget| {
            let inventory = self.control.address_inventory_v1(
                self.view,
                self.candidate,
                self.control_ordinal,
                budget,
            )?;
            budget.charge_work(2).map_err(Error::Resource)?;
            let function = inventory
                .functions()
                .get(self.output_function.0 as usize)
                .ok_or(Error::Invalid("physical function absent"))?;
            budget.charge_work(4).map_err(Error::Resource)?;
            let header =
                std::mem::size_of::<SourceOutputControlNormalizationV1<'_, '_, '_, '_, '_>>()
                    .checked_sub(std::mem::size_of::<SourceOutputAllocationScratchV1>())
                    .and_then(|n| {
                        n.checked_add(std::mem::size_of::<SourceOutputAddressWorkspaceV1>())
                    })
                    .and_then(|n| {
                        n.checked_add(std::mem::size_of::<
                            ProductionPhysicalAddressRelationV1<'_, '_, '_>,
                        >())
                    })
                    .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            budget.reserve_storage(header).map_err(Error::Resource)?;
            let ssa = source_output_allocation_scratch_v1(inventory, budget)?;
            let mut context = SourceOutputControlNormalizationV1 {
                literal_uses: &[],
                guard: None,
                invocation_roots: self.control.invocation_roots,
                inner: SourceOutputScalarNormalizationV1 {
                    inventory,
                    function,
                    lowering: self.candidate.lowering,
                    sources: Vec::new(),
                    locations: Vec::new(),
                    sites: Vec::new(),
                    views: Vec::new(),
                    expressions: Vec::new(),
                    nodes: Vec::new(),
                    comparison: [(0, 0); 2 * MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
                    visiting: [None; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
                    visiting_len: 0,
                    normalization_steps: 0,
                    ssa,
                    error: None,
                    budget,
                },
            };
            let mut work = SourceOutputAddressWorkspaceV1::default();
            source_output_address_ranked_index_v1(
                &mut work,
                self.candidate.lowering,
                context.inner.budget,
            )?;
            let mut private_accesses = 0usize;
            for ordinal in 0..self.access_count() {
                context
                    .inner
                    .budget
                    .charge_work(2)
                    .map_err(Error::Resource)?;
                if !source_output_address_access_v1(self, ordinal, &mut work, &mut context)? {
                    private_accesses = private_accesses
                        .checked_add(1)
                        .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
                }
            }
            source_output_address_check_consumers_v1(
                &mut work,
                inventory,
                function,
                context.inner.budget,
            )?;
            let relation = ProductionPhysicalAddressRelationV1 {
                analysis: self,
                rows: &work.rows,
                private_accesses,
                floor: context.inner.budget.storage(),
            };
            body(&relation, context.inner.budget)
        })
    }
}

include!("production_semantic_kir_v1/tests/production_source_output_physical_address_v1_tests.rs");

include!("production_source_output_functional_address_v1.rs");

#[cfg(test)]
mod identity_physical_key_components_v1 {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirBlockCoordinateV1 as Block,
        CanonicalKirFunctionCoordinateV1 as Function,
    };

    // Inert equality keys only. These are not source/control/address owners.
    fn key() -> SourceOutputIdentityPhysicalKeyV1 {
        let operation = SourceOutputAddressOpV1 {
            block: Block {
                function: Function(2),
                block: 3,
            },
            operation: 4,
        };
        let definition = SourceOutputAddressDefV1::Result {
            operation,
            result: 0,
        };
        SourceOutputIdentityPhysicalKeyV1 {
            ranked_index: ProductionRankedValueV1::Argument(0),
            ranked_extent: ProductionRankedValueV1::Argument(1),
            source_slice: SemanticLocalIdV1::from_index(2),
            output_slice: ValueId(3),
            pointer: definition,
            pointer_value: Some(ValueId(4)),
            gep: operation,
            offset: definition,
            allocation: definition,
        }
    }

    #[test]
    fn identity_physical_key_rejects_each_own_use_axis_independently() {
        let expected = key();
        for axis in 0..10 {
            let mut actual = expected;
            let foreign = SourceOutputAddressDefV1::FunctionArgument {
                function: Function(7),
                argument: 0,
            };
            match axis {
                0 => actual.ranked_index = ProductionRankedValueV1::Argument(9),
                1 => actual.ranked_extent = ProductionRankedValueV1::Argument(9),
                2 => actual.source_slice = SemanticLocalIdV1::from_index(9),
                3 => actual.output_slice = ValueId(9),
                4 => actual.pointer = foreign,
                5 => actual.pointer_value = Some(ValueId(9)),
                6 => actual.gep.block.function = Function(9),
                7 => actual.offset = foreign,
                8 => actual.allocation = foreign,
                9 => actual.pointer_value = None,
                _ => unreachable!(),
            }
            let mut work = Work::new(32);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
            assert!(matches!(
                source_output_identity_physical_key_v1(expected, actual, &mut budget),
                Err(ProductionSourceOutputErrorV1::Invalid(
                    "physical identity getter own address differs"
                ))
            ));
            assert_eq!(budget.work(), 16);
            source_output_identity_physical_key_v1(expected, expected, &mut budget).unwrap();
            assert_eq!(budget.work(), 32);
            assert_eq!(budget.storage(), 0);
        }
    }

    #[test]
    fn identity_physical_key_prepays_exact_fixed_query_work_before_comparing() {
        for limit in [15, 16] {
            let mut work = Work::new(limit);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
            let result = source_output_identity_physical_key_v1(key(), key(), &mut budget);
            if limit == 16 {
                result.unwrap();
                assert_eq!(budget.work(), 16);
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Resource(
                        AssertOriginResourceV1::Work(error)
                    )) if error.actual() == 16 && error.limit() == 15
                ));
                assert_eq!(budget.work(), 0);
            }
            assert_eq!(budget.storage(), 0);
        }
    }
}
