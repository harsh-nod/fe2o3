fn private_array_partition_v1<T, const N: usize, W: PrivateArrayChargeV1>(
    rows: &[T],
    key: impl Fn(&T) -> [usize; N],
    target: [usize; N],
    upper: bool,
    work: &mut W,
) -> Result<usize, W::Error> {
    let mut left = 0;
    let mut right = rows.len();
    loop {
        work.charge_private_array_work(1)?;
        if left == right {
            return Ok(left);
        }
        work.charge_private_array_work(3)?;
        let middle = left + (right - left) / 2;
        let order = private_array_compare_keys_v1(key(&rows[middle]), target, work)?;
        work.charge_private_array_work(2)?;
        if order == std::cmp::Ordering::Less || (upper && order == std::cmp::Ordering::Equal) {
            work.charge_private_array_work(1)?;
            left = middle + 1;
        } else {
            right = middle;
        }
    }
}

#[derive(Clone, Copy)]
struct PrivateArrayRankedDefinitionV1<'a> {
    value: ProductionRankedValueIdV1,
    operation: &'a ProductionRankedOperationV1,
}

fn private_array_ranked_definition_v1(
    operation: &ProductionRankedOperationV1,
) -> Option<ProductionRankedValueIdV1> {
    match operation {
        ProductionRankedOperationV1::IndexConstant { result, .. }
        | ProductionRankedOperationV1::ViewInSpace {
            result,
            memory_space: dialect_kernel::MemorySpaceAttr::Private,
            ..
        } => Some(*result),
        _ => None,
    }
}

fn private_array_ranked_index_v1<'a>(
    lowering: &'a ProductionRankedKernelLoweringInputV1,
    max_operations: usize,
    work: &mut PrivateArrayCorrelationWorkV1<'_>,
) -> Result<Vec<PrivateArrayRankedDefinitionV1<'a>>, ProductionMirPlironTranslationErrorV1> {
    let mut count = 0usize;
    let mut operations = 0usize;
    for block in lowering.kernel().blocks() {
        work.charge_private_array_work(1)?;
        for operation in block.operations() {
            work.charge_private_array_work(3)?;
            operations = operations
                .checked_add(1)
                .ok_or(ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
            if operations > max_operations {
                return Err(ProductionMirPlironTranslationErrorV1::ResourceLimit);
            }
            if private_array_ranked_definition_v1(operation).is_some() {
                work.charge_private_array_work(1)?;
                count = count
                    .checked_add(1)
                    .ok_or(ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
            }
        }
    }
    work.charge_private_array_work(2)?;
    count
        .checked_mul(std::mem::size_of::<PrivateArrayRankedDefinitionV1<'a>>())
        .ok_or(ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
    for block in lowering.kernel().blocks() {
        work.charge_private_array_work(1)?;
        for operation in block.operations() {
            work.charge_private_array_work(1)?;
            if let Some(value) = private_array_ranked_definition_v1(operation) {
                work.charge_private_array_work(2)?;
                if rows.len() >= count {
                    return Err(ProductionMirPlironTranslationErrorV1::ResourceLimit);
                }
                rows.push(PrivateArrayRankedDefinitionV1 { value, operation });
            }
        }
    }
    work.charge_private_array_work(1)?;
    if rows.len() != count {
        return Err(ProductionMirPlironTranslationErrorV1::ResourceLimit);
    }
    private_array_heapsort_v1(
        &mut rows,
        |row| [row.value.get() as usize],
        work,
        || ProductionMirPlironTranslationErrorV1::ResourceLimit,
    )?;
    for pair in rows.windows(2) {
        work.charge_private_array_work(2)?;
        if pair[0].value >= pair[1].value {
            return Err(ProductionMirPlironTranslationErrorV1::KernelShape);
        }
    }
    Ok(rows)
}

struct PrivateArrayFinalRelationV1<'a> {
    owner: SemanticFunctionIdV1,
    function_id: SemanticFunctionIdV1,
    semantic: &'a AdmittedInertSemanticMirV1,
    function: &'a SemanticFunctionDeclV1,
    body: &'a FunctionBody,
    slots: &'a [PrivateArraySlotV1],
    effects: &'a [PrivateArrayEffectV1],
    ranked_definitions: Vec<PrivateArrayRankedDefinitionV1<'a>>,
    max_operations: usize,
}

impl<'a> PrivateArrayFinalRelationV1<'a> {
    fn new(
        module: &'a Module,
        correspondence: &'a SemanticKirCorrespondenceV1,
        semantic: Option<&'a AdmittedInertSemanticMirV1>,
        owner: SemanticFunctionIdV1,
        function_id: SemanticFunctionIdV1,
        actual: &'a Function,
        lowering: &'a ProductionRankedKernelLoweringInputV1,
        max_operations: usize,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
    ) -> Result<Option<Self>, ProductionMirPlironTranslationErrorV1> {
        if !correspondence.private_arrays.active {
            return Ok(None);
        }
        let mut work = PrivateArrayCorrelationWorkV1 { budget };
        let rows = &correspondence.private_arrays;
        let Some(instance) = private_array_instance_v1(rows, owner, function_id, &mut work)? else {
            return Ok(None);
        };
        work.charge_private_array_work(8)?;
        let semantic = semantic.ok_or(ProductionMirPlironTranslationErrorV1::KernelShape)?;
        let function = semantic
            .functions()
            .get(function_id.index() as usize)
            .ok_or(ProductionMirPlironTranslationErrorV1::KernelShape)?;
        let entry = correspondence
            .lowered_functions
            .get(instance.lowered_function_ordinal)
            .ok_or(ProductionMirPlironTranslationErrorV1::KernelShape)?;
        let physical = module
            .functions
            .get(instance.module_function_ordinal)
            .ok_or(ProductionMirPlironTranslationErrorV1::KernelShape)?;
        if entry.correspondence_owner != owner
            || entry.semantic_function != function_id
            || !std::ptr::eq(physical, actual)
        {
            return Err(ProductionMirPlironTranslationErrorV1::KernelShape);
        }
        if !private_array_equal_bytes_v1(
            entry.kernel_ir_function.as_str().as_bytes(),
            actual.id.as_str().as_bytes(),
            &mut work,
        )? {
            return Err(ProductionMirPlironTranslationErrorV1::KernelShape);
        }
        work.charge_private_array_work(3)?;
        let body = actual
            .body
            .as_ref()
            .ok_or(ProductionMirPlironTranslationErrorV1::KernelShape)?;
        let slots = rows
            .slots
            .get(instance.slot_start..instance.slot_end)
            .ok_or(ProductionMirPlironTranslationErrorV1::KernelShape)?;
        let effects = rows
            .effects
            .get(instance.effect_start..instance.effect_end)
            .ok_or(ProductionMirPlironTranslationErrorV1::KernelShape)?;
        let ranked_definitions =
            private_array_ranked_index_v1(lowering, max_operations, &mut work)?;
        Ok(Some(Self {
            owner,
            function_id,
            semantic,
            function,
            body,
            slots,
            effects,
            ranked_definitions,
            max_operations,
        }))
    }

    fn statement_range(
        &self,
        site: SemanticAccessSiteV1,
        work: &mut PrivateArrayCorrelationWorkV1<'_>,
    ) -> Result<&[PrivateArrayEffectV1], ProductionMirPlironTranslationErrorV1> {
        let Some(statement) = site.statement else {
            return Ok(&[]);
        };
        let key = [site.block as usize, statement as usize];
        let start = private_array_partition_v1(
            self.effects,
            |row| [row.semantic_block as usize, row.semantic_statement as usize],
            key,
            false,
            work,
        )?;
        let end = private_array_partition_v1(
            self.effects,
            |row| [row.semantic_block as usize, row.semantic_statement as usize],
            key,
            true,
            work,
        )?;
        work.charge_private_array_work(1)?;
        self.effects
            .get(start..end)
            .ok_or(ProductionMirPlironTranslationErrorV1::KernelShape)
    }

    fn definition(
        &self,
        value: ProductionRankedValueV1,
        work: &mut PrivateArrayCorrelationWorkV1<'_>,
    ) -> Result<Option<&ProductionRankedOperationV1>, ProductionMirPlironTranslationErrorV1> {
        work.charge_private_array_work(1)?;
        let ProductionRankedValueV1::Local(value) = value else {
            return Ok(None);
        };
        let found = private_array_binary_search_v1(
            &self.ranked_definitions,
            |row| [row.value.get() as usize],
            [value.get() as usize],
            work,
        )?;
        match found {
            Ok(index) => {
                work.charge_private_array_work(1)?;
                Ok(Some(self.ranked_definitions[index].operation))
            }
            Err(_) => Ok(None),
        }
    }

    fn check(
        &self,
        lowering: &ProductionRankedKernelLoweringInputV1,
        source: &IndexedRankedAccessSourceV1,
        consumer: KirMemoryConsumerV1,
        site: SemanticAccessSiteV1,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
    ) -> Result<(), ProductionMirPlironTranslationErrorV1> {
        let mismatch = || ProductionMirPlironTranslationErrorV1::AllocationOriginMismatch {
            location: consumer.location,
        };
        let mut work = PrivateArrayCorrelationWorkV1 { budget };
        let effects = self.statement_range(site, &mut work)?;
        // This is the physical axis, not a reinterpretation of the dense ranked ordinal.
        let key = [
            consumer.location.block.0 as usize,
            consumer.location.operation_index,
        ];
        let index = private_array_binary_search_v1(
            effects,
            |row| {
                [
                    row.memory_location.block.0 as usize,
                    row.memory_location.operation,
                ]
            },
            key,
            &mut work,
        )?
        .map_err(|_| mismatch())?;
        work.charge_private_array_work(2)?;
        let effect = &effects[index];
        if consumer.operation_access_ordinal != 0 {
            return Err(mismatch());
        }
        let slot_index = private_array_binary_search_v1(
            self.slots,
            |row| [row.local as usize],
            [effect.local as usize],
            &mut work,
        )?
        .map_err(|_| mismatch())?;
        work.charge_private_array_work(1)?;
        let slot = &self.slots[slot_index];
        let offset = private_array_exact_relation_v1(
            self.semantic.types(),
            self.function,
            self.body,
            self.owner,
            self.function_id,
            slot,
            effect,
            self.max_operations,
            &mut work,
        )
        .map_err(|error| match error {
            PrivateArrayRelationErrorV1::Work(error) => error,
            _ => mismatch(),
        })?;
        work.charge_private_array_work(3)?;
        let operation = lowering
            .kernel()
            .blocks()
            .get(source.ranked_block as usize)
            .and_then(|block| block.operations().get(source.ranked_operation as usize))
            .ok_or_else(mismatch)?;
        let ProductionRankedOperationV1::Access {
            view,
            indices,
            kind,
        } = operation
        else {
            return Err(mismatch());
        };
        work.charge_private_array_work(4)?;
        if effect.access != PrivateArrayAccessV1::Write
            || *kind != dialect_kernel::AccessKindAttr::Write
            || consumer.access != *kind
            || source.atomic.is_some()
        {
            return Err(mismatch());
        }
        work.charge_private_array_work(1)?;
        let [ranked_index] = indices.as_slice() else {
            return Err(mismatch());
        };
        let Some(ProductionRankedOperationV1::ViewInSpace {
            element_width,
            writable,
            shape,
            dynamic_extents,
            memory_space,
            allocation_origin,
            noalias_class,
            ..
        }) = self.definition(*view, &mut work)?
        else {
            return Err(mismatch());
        };
        work.charge_private_array_work(1)?;
        let [extent] = shape.as_slice() else {
            return Err(mismatch());
        };
        // Two origin additions, width multiplication, and seven scalar predicates.
        work.charge_private_array_work(10)?;
        let origin = (1u64 << 63)
            .checked_add(u64::from(slot.local))
            .and_then(|n| n.checked_add(1))
            .ok_or_else(mismatch)?;
        let width = slot
            .element_facts
            .size
            .checked_mul(8)
            .ok_or_else(mismatch)?;
        if *memory_space != dialect_kernel::MemorySpaceAttr::Private
            || !*writable
            || *extent != slot.length
            || !dynamic_extents.is_empty()
            || u64::from(*element_width) != width
            || *allocation_origin != origin
            || *noalias_class != origin
        {
            return Err(mismatch());
        }
        let Some(ProductionRankedOperationV1::IndexConstant { value, .. }) =
            self.definition(*ranked_index, &mut work)?
        else {
            return Err(mismatch());
        };
        work.charge_private_array_work(1)?;
        if *value != offset {
            return Err(mismatch());
        }
        Ok(())
    }

    fn requires_consumption(
        &self,
        site: SemanticAccessSiteV1,
        source: &IndexedRankedAccessSourceV1,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
    ) -> Result<bool, ProductionMirPlironTranslationErrorV1> {
        let mut work = PrivateArrayCorrelationWorkV1 { budget };
        if !self.statement_range(site, &mut work)?.is_empty() {
            return Ok(true);
        }
        work.charge_private_array_work(1)?;
        let IndexedRankedAllocationV1::View(view) = source.allocation else {
            return Ok(false);
        };
        let Some(ProductionRankedOperationV1::ViewInSpace {
            allocation_origin, ..
        }) = self.definition(view, &mut work)?
        else {
            return Ok(false);
        };
        work.charge_private_array_work(2)?;
        let Some(local) = allocation_origin
            .checked_sub((1u64 << 63) + 1)
            .and_then(|n| u32::try_from(n).ok())
        else {
            return Ok(false);
        };
        Ok(private_array_binary_search_v1(
            self.slots,
            |row| [row.local as usize],
            [local as usize],
            &mut work,
        )?
        .is_ok())
    }
}
