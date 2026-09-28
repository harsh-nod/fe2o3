// Movement-only correspondence for exact original typed storage. This
// does not replace source effects, alias/currentness equations or final replay.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ScopedStorageChainV29 {
    first: Option<usize>,
    last: Option<usize>,
    count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScopedStorageTypeV29 {
    Scalar(ScalarType),
    Vector(fe2o3_kernel_ir::FixedVectorTypeV12),
    Pointer(fe2o3_kernel_ir::StorageLayoutIdV1, AddressSpace, AccessMode),
    OriginalPointer(ScalarType, AddressSpace, AccessMode),
}

impl ScopedStorageTypeV29 {
    fn matches(self, ty: &Type) -> bool {
        match (self, ty) {
            (Self::Scalar(expected), Type::Scalar(actual)) => expected == *actual,
            (Self::Vector(expected), Type::Vector(actual)) => expected == *actual,
            (Self::Pointer(schema, space, access), Type::Pointer(pointer)) => {
                pointer.pointee.as_ref() == &Type::StorageObject(schema)
                    && pointer.address_space == space
                    && pointer.access == access
            }
            (Self::OriginalPointer(element, space, access), Type::Pointer(pointer)) => {
                pointer.pointee.as_ref() == &Type::Scalar(element)
                    && pointer.address_space == space
                    && pointer.access == access
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ScopedStorageSourceV29 {
    instance: ProductionCallInstanceIdV1,
    span: usize,
    source: InstanceSpanSourceV1,
    offset: u32,
    call_offset: Option<u32>,
    payload: ScopedObjectPayloadV29,
    inputs: [Option<(ValueId, ScopedStorageTypeV29)>; 2],
    result: Option<ScopedStorageTypeV29>,
    next: Option<usize>,
}

#[cfg_attr(test, derive(Clone))]
struct ScopedStorageTransportV29 {
    source_plan: usize,
    ledger: ArgumentLedgerV1,
    slot: usize,
    chains: Vec<ScopedStorageChainV29>,
    rows: Vec<ScopedStorageSourceV29>,
}

struct ScopedStorageCalleeSourceV29<'a, 'p, 's> {
    transport: &'a ScopedStorageTransportV29,
    map: &'a ProductionInstanceCorrespondenceV1<'p, 's>,
    child: ProductionCallInstanceIdV1,
}

struct ScopedStorageCalleeV29<'a> {
    function: &'a Function,
    ledger: ArgumentLedgerV1,
    slot: usize,
}

struct ScopedStorageActualV29<'a> {
    point: (BlockId, u32),
    operation: &'a Operation,
    seen: bool,
}

type ScopedStorageSpanV29 = ((usize, BlockId, u32), usize, Option<u32>);

fn scoped_storage_span_index_v29(
    rows: &[InstanceMappedSpanV1],
    root: ProductionCallInstanceIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
    scratch: &mut usize,
) -> Result<Vec<ScopedStorageSpanV29>, CallInstanceEmissionErrorV1> {
    use CallInstanceEmissionErrorV1::StorageTransport as Refused;
    budget.charge_work(rows.len())?;
    let count = rows
        .iter()
        .filter(|row| row.instance != root && row.source.coordinates().2.count != 0)
        .count();
    call_splice_charge_storage_v1(
        std::mem::size_of::<Vec<ScopedStorageSpanV29>>(),
        budget,
        scratch,
    )?;
    let mut spans = call_splice_vec_v1(count, budget, scratch)?;
    budget.charge_work(rows.len())?;
    for (ordinal, row) in rows.iter().enumerate() {
        let source = row.source.coordinates().2;
        if row.instance != root && source.count != 0 {
            spans.push((
                (row.instance.index(), source.block, source.first),
                ordinal,
                None,
            ));
        }
    }
    call_splice_sort_work_v1(spans.len(), budget)?;
    spans.sort_unstable_by_key(|row| row.0);
    budget.charge_work(spans.len())?;
    for pair in spans.windows(2) {
        let left = &rows[pair[0].1];
        let right = &rows[pair[1].1];
        let a = left.source.coordinates().2;
        let b = right.source.coordinates().2;
        let end = a
            .first
            .checked_add(a.count)
            .ok_or_else(call_splice_arithmetic_v1)?;
        if left.instance == right.instance && a.block == b.block && end > b.first {
            return Err(Refused);
        }
    }
    Ok(spans)
}

#[cfg(test)]
type ScopedStorageObserverV29 = fn(
    &ScopedStorageTransportV29,
    &ProductionInstanceCorrespondenceV1<'_, '_>,
    &[Option<LoweredFunctionResultV1>],
    &mut ArgumentBudgetV1<'_>,
) -> Result<(), CallInstanceEmissionErrorV1>;

#[cfg(test)]
thread_local! {
    static SCOPED_STORAGE_OBSERVER_V29: std::cell::Cell<Option<ScopedStorageObserverV29>> = const { std::cell::Cell::new(None) };
}

fn scoped_storage_error_v29(error: ProductionSemanticKirErrorV1) -> CallInstanceEmissionErrorV1 {
    match error {
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => error.into(),
        _ => CallInstanceEmissionErrorV1::StorageTransport,
    }
}

fn scoped_storage_actual_v29<'a>(
    function: &'a Function,
    budget: &mut ArgumentBudgetV1<'_>,
    scratch: &mut usize,
) -> Result<Vec<ScopedStorageActualV29<'a>>, CallInstanceEmissionErrorV1> {
    use CallInstanceEmissionErrorV1::StorageTransport as Refused;
    let body = function.body.as_ref().ok_or(Refused)?;
    let mut count = 0usize;
    budget.charge_work(body.blocks.len())?;
    for block in &body.blocks {
        budget.charge_work(block.operations.len())?;
        for operation in &block.operations {
            if matches!(operation.kind, OperationKind::Storage(_)) {
                count = count.checked_add(1).ok_or_else(call_splice_arithmetic_v1)?;
            }
        }
    }
    call_splice_charge_storage_v1(
        std::mem::size_of::<Vec<ScopedStorageActualV29<'_>>>(),
        budget,
        scratch,
    )?;
    let mut actual = call_splice_vec_v1(count, budget, scratch)?;
    budget.charge_work(body.blocks.len())?;
    for block in &body.blocks {
        budget.charge_work(block.operations.len())?;
        for (ordinal, operation) in block.operations.iter().enumerate() {
            if matches!(operation.kind, OperationKind::Storage(_)) {
                actual.push(ScopedStorageActualV29 {
                    point: (
                        block.id,
                        u32::try_from(ordinal).map_err(|_| call_splice_arithmetic_v1())?,
                    ),
                    operation,
                    seen: false,
                });
            }
        }
    }
    call_splice_sort_work_v1(actual.len(), budget)?;
    actual.sort_unstable_by_key(|row| row.point);
    budget.charge_work(actual.len())?;
    if actual.windows(2).any(|pair| pair[0].point == pair[1].point) {
        return Err(Refused);
    }
    Ok(actual)
}

fn scoped_storage_operand_types_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    payload: &ScopedObjectPayloadV29,
    operation: &Operation,
    index: &CallSpliceIndexV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<
    (
        [Option<(ValueId, ScopedStorageTypeV29)>; 2],
        Option<ScopedStorageTypeV29>,
    ),
    CallInstanceEmissionErrorV1,
> {
    use CallInstanceEmissionErrorV1::StorageTransport as Refused;
    let layouts = plan
        .storage_root
        .as_ref()
        .ok_or(Refused)?
        .source_layouts(plan.instances, budget)
        .map_err(scoped_storage_error_v29)?;
    payload
        .role
        .visit_endpoints(|endpoint| {
            layouts.check_selected_schema(
                plan.instances.owner(),
                endpoint.root_type,
                endpoint.root_schema,
                budget,
            )?;
            layouts.check_selected_schema(
                plan.instances.owner(),
                endpoint.projected_type,
                endpoint.projected_schema,
                budget,
            )
        })
        .map_err(scoped_storage_error_v29)?;
    let physical = layouts
        .rows(plan.instances.owner(), budget)
        .map_err(scoped_storage_error_v29)?;
    let value_type = |endpoint: ScopedObjectEndpointV29, budget: &mut ArgumentBudgetV1<'_>| {
        Ok(
            match &physical
                .get(endpoint.projected_schema.0 as usize)
                .ok_or(Refused)?
                .kind
            {
                fe2o3_kernel_ir::StorageLayoutKindV1::Scalar(scalar) => {
                    ScopedStorageTypeV29::Scalar(*scalar)
                }
                fe2o3_kernel_ir::StorageLayoutKindV1::Vector(vector) => {
                    ScopedStorageTypeV29::Vector(*vector)
                }
                fe2o3_kernel_ir::StorageLayoutKindV1::Pointer(pointer) => {
                    budget.charge_work(4)?;
                    let original = plan
                        .instances
                        .owner()
                        .source_semantic()
                        .types()
                        .get(endpoint.projected_type.index() as usize)
                        .ok_or(Refused)?;
                    if matches!(original.shape(), SemanticTypeShapeV1::Pointer(original)
                        if original.kind() == SemanticPointerKindV1::Raw)
                        && layouts
                            .original_schema(
                                plan.instances.owner(),
                                endpoint.projected_type,
                                budget,
                            )
                            .map_err(scoped_storage_error_v29)?
                            == Some(endpoint.projected_schema)
                    {
                        // Original pointer values retain scalar pointees. A
                        // selected storage address is a different representation.
                        let Type::Pointer(value) = source_object_original_leaf_type_v29(
                            plan,
                            endpoint.projected_type,
                            endpoint.projected_schema,
                            budget,
                        )
                        .map_err(scoped_storage_error_v29)?
                        else {
                            return Err(Refused);
                        };
                        let Type::Scalar(element) = value.pointee.as_ref() else {
                            return Err(Refused);
                        };
                        ScopedStorageTypeV29::OriginalPointer(
                            *element,
                            value.address_space,
                            value.access,
                        )
                    } else {
                        ScopedStorageTypeV29::Pointer(
                            pointer.pointee,
                            pointer.value_space,
                            pointer.access,
                        )
                    }
                }
                _ => return Err(Refused),
            },
        )
    };
    let pointer_type =
        |endpoint: ScopedObjectEndpointV29, address: ValueId, budget: &mut ArgumentBudgetV1<'_>| {
            let Type::Pointer(pointer) = index.value(address, budget)? else {
                return Err(Refused);
            };
            if pointer.pointee.as_ref() != &Type::StorageObject(endpoint.projected_schema) {
                return Err(Refused);
            }
            Ok(ScopedStorageTypeV29::Pointer(
                endpoint.projected_schema,
                pointer.address_space,
                pointer.access,
            ))
        };
    budget.charge_work(8)?;
    let (inputs, result) = match (payload.role, payload.operation) {
        (
            ScopedObjectRoleV29::Project { source, projected },
            ScopedObjectOperationV29::Project { base, step },
        ) => {
            let pointer = pointer_type(source, base, budget)?;
            let ScopedStorageTypeV29::Pointer(_, space, access) = pointer else {
                return Err(Refused);
            };
            let index = match step {
                ScopedObjectProjectionV29::ArrayIndex(value) => {
                    Some((value, ScopedStorageTypeV29::Scalar(ScalarType::Index)))
                }
                _ => None,
            };
            (
                [Some((base, pointer)), index],
                Some(ScopedStorageTypeV29::Pointer(
                    projected.projected_schema,
                    space,
                    access,
                )),
            )
        }
        (
            ScopedObjectRoleV29::ReadValue { source, .. },
            ScopedObjectOperationV29::ReadValue { address, .. },
        ) => (
            [
                Some((address, pointer_type(source, address, budget)?)),
                None,
            ],
            Some(value_type(source, budget)?),
        ),
        (
            ScopedObjectRoleV29::WriteValue { destination, .. },
            ScopedObjectOperationV29::WriteValue { address, value, .. },
        ) => (
            [
                Some((address, pointer_type(destination, address, budget)?)),
                Some((value, value_type(destination, budget)?)),
            ],
            None,
        ),
        (
            ScopedObjectRoleV29::CopyObject {
                source,
                destination,
            },
            ScopedObjectOperationV29::CopyObject {
                source: read,
                destination: write,
                ..
            },
        ) => {
            if source.projected_schema != destination.projected_schema {
                return Err(Refused);
            }
            (
                [
                    Some((read, pointer_type(source, read, budget)?)),
                    Some((write, pointer_type(destination, write, budget)?)),
                ],
                None,
            )
        }
        (
            ScopedObjectRoleV29::ReadDiscriminant { source, .. },
            ScopedObjectOperationV29::ReadDiscriminant { address, .. },
        ) => {
            if !matches!(
                physical
                    .get(source.projected_schema.0 as usize)
                    .map(|row| &row.kind),
                Some(fe2o3_kernel_ir::StorageLayoutKindV1::Variants { .. })
            ) {
                return Err(Refused);
            }
            (
                [
                    Some((address, pointer_type(source, address, budget)?)),
                    None,
                ],
                Some(ScopedStorageTypeV29::Scalar(ScalarType::U128)),
            )
        }
        (
            ScopedObjectRoleV29::SetDiscriminant {
                destination,
                variant,
                ..
            },
            ScopedObjectOperationV29::SetDiscriminant {
                address,
                variant: actual,
                ..
            },
        ) => {
            if variant != actual
                || !matches!(physical.get(destination.projected_schema.0 as usize).map(|row| &row.kind),
                Some(fe2o3_kernel_ir::StorageLayoutKindV1::Variants { variants, .. }) if (variant as usize) < variants.len())
            {
                return Err(Refused);
            }
            (
                [
                    Some((address, pointer_type(destination, address, budget)?)),
                    None,
                ],
                None,
            )
        }
        _ => return Err(Refused),
    };
    for (id, ty) in inputs.iter().flatten() {
        if !ty.matches(index.value(*id, budget)?) {
            return Err(Refused);
        }
    }
    match (result, operation.results.as_slice()) {
        (Some(ty), [value]) if ty.matches(&value.ty) => {}
        (None, []) => {}
        _ => return Err(Refused),
    }
    Ok((inputs, result))
}

impl ScopedStorageTransportV29 {
    #[allow(clippy::too_many_arguments)]
    fn new(
        references: &SourceReferencePlanV29<'_, '_>,
        map: &ProductionInstanceCorrespondenceV1<'_, '_>,
        emitted: &[Option<LoweredFunctionResultV1>],
        budget: &mut ArgumentBudgetV1<'_>,
        scratch: &mut usize,
    ) -> Result<Self, CallInstanceEmissionErrorV1> {
        use CallInstanceEmissionErrorV1::StorageTransport as Refused;
        map.check_live_ledger_v1(budget)
            .map_err(|error| match error {
                InstanceCorrespondenceErrorV1::Resource(error) => error.into(),
                _ => Refused,
            })?;
        references
            .check_owner(map.plan, budget)
            .map_err(scoped_storage_error_v29)?;
        budget.charge_work(emitted.len())?;
        if emitted.len() != map.plan.instances().len() {
            return Err(Refused);
        }
        let root = map.plan.root();
        budget.charge_work(2)?;
        if map.plan.instance(root).is_none() || map.plan.incoming(root).is_some() {
            return Err(Refused);
        }
        let mut count = 0usize;
        budget.charge_work(emitted.len())?;
        for (ordinal, lowered) in emitted.iter().enumerate() {
            if ordinal == root.index() {
                continue;
            }
            let Some(lowered) = lowered else {
                continue;
            };
            let Some(anchors) = &lowered.scoped_memory_anchors else {
                continue;
            };
            count = count
                .checked_add(anchors.objects.len())
                .ok_or_else(call_splice_arithmetic_v1)?;
        }
        call_splice_charge_storage_v1(std::mem::size_of::<Self>(), budget, scratch)?;
        let mut result = Self {
            source_plan: std::ptr::from_ref(map.plan) as usize,
            ledger: budget.work_ledger_identity_v1(),
            slot: std::ptr::from_ref(&*budget) as usize,
            chains: call_splice_vec_v1(emitted.len(), budget, scratch)?,
            rows: call_splice_vec_v1(count, budget, scratch)?,
        };
        budget.charge_work(emitted.len())?;
        result
            .chains
            .resize(emitted.len(), ScopedStorageChainV29::default());
        // Build the original coordinate index once for the owning assembly,
        // not once per callee or per object occurrence.
        with_canonical_call_scratch_v1(budget, |budget| {
            let mut temporary = 0;
            let mut spans =
                scoped_storage_span_index_v29(&map.spans.rows, root, budget, &mut temporary)
                    .map_err(source_address_call_error_v29)?;
            let source = ExecutionCallSourceV29::from_instances(map.plan, budget)?;
            for (instance_index, lowered) in emitted.iter().enumerate() {
                budget.charge_work(2)?;
                let instance = map
                    .plan
                    .id_at(instance_index)
                    .ok_or_else(scoped_object_error_v29)?;
                if map.plan.instance_reachable(instance) == Some(false) {
                    if lowered.is_some() {
                        return Err(scoped_object_error_v29());
                    }
                    continue;
                }
                let lowered = lowered.as_ref().ok_or_else(scoped_object_error_v29)?;
                let function = &lowered.function;
                if lowered.source_call_instance != Some(instance) {
                    return Err(scoped_object_error_v29());
                }
                // Root operations never move as a callee. Their complete source
                // effects and memory obligations remain in final root admission.
                if instance == root {
                    continue;
                }
                if map.plan.incoming(instance).and_then(|call| call.child()) != Some(instance) {
                    return Err(scoped_object_error_v29());
                }
                let original = map
                    .plan
                    .instance(instance)
                    .ok_or_else(scoped_object_error_v29)?
                    .declaration();
                let occurrences = map
                    .plan
                    .occurrences(instance)
                    .ok_or_else(scoped_object_error_v29)?;
                with_canonical_call_scratch_v1(budget, |budget| {
                    let mut local_scratch = 0;
                    let parts = ScopedDeferredScalarViewV29::for_instance(
                        map.plan, instance, lowered, budget,
                    )?;
                    let index = call_splice_index_with_deferred_parts_v29(
                        function,
                        Some(&parts),
                        budget,
                        &mut local_scratch,
                    )
                    .map_err(source_address_call_error_v29)?;
                    let mut actual =
                        scoped_storage_actual_v29(function, budget, &mut local_scratch)
                            .map_err(source_address_call_error_v29)?;
                    let body = function.body.as_ref().ok_or_else(scoped_object_error_v29)?;
                    budget.charge_work(body.blocks.len())?;
                    for block in &body.blocks {
                        budget.charge_work(block.operations.len())?;
                        for (position, operation) in block.operations.iter().enumerate() {
                            if !matches!(operation.kind, OperationKind::Call { .. }) {
                                continue;
                            }
                            let position = u32::try_from(position)
                                .map_err(|_| ArgumentResourceV1::Arithmetic)?;
                            let key = (instance_index, block.id, position);
                            budget.charge_work(call_splice_search_work_v1(spans.len()))?;
                            let selected = spans
                                .partition_point(|row| row.0 <= key)
                                .checked_sub(1)
                                .ok_or_else(scoped_object_error_v29)?;
                            let mapped = &map.spans.rows[spans[selected].1];
                            let source = mapped.source.coordinates().2;
                            if mapped.instance != instance
                                || source.block != block.id
                                || position < source.first
                                || position
                                    >= source
                                        .end()
                                        .map_err(pending_scope_correspondence_error_v29)?
                                || spans[selected].2.is_some()
                            {
                                return Err(scoped_object_error_v29());
                            }
                            spans[selected].2 = Some(position - source.first);
                        }
                    }
                    let Some(anchors) = &lowered.scoped_memory_anchors else {
                        if !actual.is_empty() {
                            return Err(scoped_object_error_v29());
                        }
                        return Ok(());
                    };
                    anchors.check_object_ledger(budget)?;
                    budget.charge_work(5)?;
                    if anchors.subject.source != source
                        || anchors.subject.instance != instance
                        || anchors.subject.function
                            != map
                                .plan
                                .instance(instance)
                                .ok_or_else(scoped_object_error_v29)?
                                .function()
                        || actual.len() != anchors.objects.len()
                    {
                        return Err(scoped_object_error_v29());
                    }
                    let first = result.rows.len();
                    budget.charge_work(anchors.rows.len())?;
                    for (anchor_index, anchor) in anchors.rows.iter().enumerate() {
                        let ScopedMemoryAnchorKindV29::Object(_) = anchor.kind else {
                            continue;
                        };
                        let payload = anchors.object_payload(anchor, budget)?;
                        anchors.check_object_source(
                            original,
                            &occurrences,
                            anchor_index,
                            anchor,
                            payload,
                            budget,
                        )?;
                        let point = (
                            anchor.block,
                            u32::try_from(anchor.position)
                                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        );
                        budget.charge_work(call_splice_search_work_v1(actual.len()))?;
                        let selected = actual
                            .binary_search_by_key(&point, |row| row.point)
                            .map_err(|_| scoped_object_error_v29())?;
                        let row = &mut actual[selected];
                        if row.seen {
                            return Err(scoped_object_error_v29());
                        }
                        payload.check_operation(row.operation, budget)?;
                        let (inputs, output) = scoped_storage_operand_types_v29(
                            references,
                            payload,
                            row.operation,
                            &index,
                            budget,
                        )
                        .map_err(source_address_call_error_v29)?;
                        let key = (instance_index, point.0, point.1);
                        budget.charge_work(call_splice_search_work_v1(spans.len()))?;
                        let selected = spans
                            .partition_point(|row| row.0 <= key)
                            .checked_sub(1)
                            .ok_or_else(scoped_object_error_v29)?;
                        let span_ordinal = spans[selected].1;
                        let mapped = &map.spans.rows[span_ordinal];
                        let span = mapped.source.coordinates().2;
                        if mapped.instance != instance
                            || span.block != point.0
                            || point.1 < span.first
                            || point.1
                                >= span.end().map_err(pending_scope_correspondence_error_v29)?
                            || mapped.segments != [Some(span), None]
                            || mapped.removed_call.is_some()
                            || result.rows.len() == result.rows.capacity()
                        {
                            return Err(scoped_object_error_v29());
                        }
                        result.rows.push(ScopedStorageSourceV29 {
                            instance,
                            span: span_ordinal,
                            source: mapped.source,
                            offset: point.1 - span.first,
                            call_offset: spans[selected].2,
                            payload: *payload,
                            inputs,
                            result: output,
                            next: None,
                        });
                        row.seen = true;
                    }
                    budget.charge_work(actual.len())?;
                    if actual.iter().any(|row| !row.seen) {
                        return Err(scoped_object_error_v29());
                    }
                    let end = result.rows.len();
                    budget.charge_work(end - first)?;
                    for ordinal in first..end.saturating_sub(1) {
                        result.rows[ordinal].next = Some(ordinal + 1);
                    }
                    result.chains[instance_index] = ScopedStorageChainV29 {
                        first: (first != end).then_some(first),
                        last: end.checked_sub(1).filter(|_| first != end),
                        count: end - first,
                    };
                    Ok(())
                })?;
            }
            if result.rows.len() != count {
                return Err(scoped_object_error_v29());
            }
            Ok(())
        })
        .map_err(scoped_storage_error_v29)?;
        #[cfg(test)]
        if let Some(observer) = SCOPED_STORAGE_OBSERVER_V29.get() {
            observer(&result, map, emitted, budget)?;
        }
        Ok(result)
    }

    fn join(
        &mut self,
        caller: ProductionCallInstanceIdV1,
        child: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), CallInstanceEmissionErrorV1> {
        self.check_custody(budget)?;
        budget.charge_work(5)?;
        use CallInstanceEmissionErrorV1::StorageTransport as Refused;
        if caller == child {
            return Err(Refused);
        }
        let left = *self.chains.get(caller.index()).ok_or(Refused)?;
        let right = *self.chains.get(child.index()).ok_or(Refused)?;
        let count = left
            .count
            .checked_add(right.count)
            .ok_or_else(call_splice_arithmetic_v1)?;
        if let Some(last) = left.last {
            let tail = self.rows.get_mut(last).ok_or(Refused)?;
            if tail.next.is_some() {
                return Err(Refused);
            }
            tail.next = right.first;
        }
        self.chains[caller.index()] = ScopedStorageChainV29 {
            first: left.first.or(right.first),
            last: right.last.or(left.last),
            count,
        };
        self.chains[child.index()] = ScopedStorageChainV29::default();
        Ok(())
    }

    fn check_custody(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> Result<(), CallInstanceEmissionErrorV1> {
        if self.ledger != budget.work_ledger_identity_v1()
            || self.slot != std::ptr::from_ref(budget) as usize
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }
}

impl ScopedStorageCalleeSourceV29<'_, '_, '_> {
    fn permit<'f>(
        &self,
        function: &'f Function,
        index: &CallSpliceIndexV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
        scratch: &mut usize,
    ) -> Result<ScopedStorageCalleeV29<'f>, CallInstanceEmissionErrorV1> {
        use CallInstanceEmissionErrorV1::StorageTransport as Refused;
        self.transport.check_custody(budget)?;
        self.map
            .check_live_ledger_v1(budget)
            .map_err(|error| match error {
                InstanceCorrespondenceErrorV1::Resource(error) => error.into(),
                _ => Refused,
            })?;
        budget.charge_work(5)?;
        if self.transport.source_plan != std::ptr::from_ref(self.map.plan) as usize
            || self.child == self.map.plan.root()
            || self
                .map
                .plan
                .incoming(self.child)
                .and_then(|call| call.child())
                != Some(self.child)
        {
            return Err(Refused);
        }
        let seed = self
            .map
            .seed_index(self.child, budget)
            .map_err(|error| match error {
                InstanceCorrespondenceErrorV1::Resource(error) => error.into(),
                _ => Refused,
            })?;
        budget.charge_work(function.id.as_str().len())?;
        if self.map.seeds.rows[seed].container != self.child
            || self.map.seeds.rows[seed].function_name != function.id.as_str()
        {
            return Err(Refused);
        }
        let chain = *self
            .transport
            .chains
            .get(self.child.index())
            .ok_or(Refused)?;
        let mut actual = scoped_storage_actual_v29(function, budget, scratch)?;
        if actual.len() != chain.count
            || (chain.count == 0) != chain.first.is_none()
            || (chain.count == 0) != chain.last.is_none()
        {
            return Err(Refused);
        }
        let mut next = chain.first;
        let mut last = None;
        for _ in 0..chain.count {
            budget.charge_work(10)?;
            let ordinal = next.ok_or(Refused)?;
            let source = self.transport.rows.get(ordinal).ok_or(Refused)?;
            let mapped = self.map.spans.rows.get(source.span).ok_or(Refused)?;
            if mapped.instance != source.instance || mapped.source != source.source {
                return Err(Refused);
            }
            let point = scoped_storage_mapped_point_v29(mapped, source.offset, source.call_offset)?;
            budget.charge_work(call_splice_search_work_v1(actual.len()))?;
            let row = actual
                .binary_search_by_key(&point, |row| row.point)
                .map_err(|_| Refused)?;
            let row = &mut actual[row];
            if row.seen {
                return Err(Refused);
            }
            source
                .payload
                .check_operation(row.operation, budget)
                .map_err(scoped_storage_error_v29)?;
            for (value, ty) in source.inputs.iter().flatten() {
                budget.charge_work(1)?;
                if !ty.matches(index.value(*value, budget)?) {
                    return Err(Refused);
                }
            }
            budget.charge_work(1)?;
            if let Some(ty) = source.result {
                if !matches!(row.operation.results.as_slice(), [value] if ty.matches(&value.ty)) {
                    return Err(Refused);
                }
            } else if !row.operation.results.is_empty() {
                return Err(Refused);
            }
            row.seen = true;
            last = Some(ordinal);
            next = source.next;
        }
        budget.charge_work(actual.len())?;
        if next.is_some() || last != chain.last || actual.iter().any(|row| !row.seen) {
            return Err(Refused);
        }
        call_splice_charge_storage_v1(
            std::mem::size_of::<ScopedStorageCalleeV29<'_>>(),
            budget,
            scratch,
        )?;
        Ok(ScopedStorageCalleeV29 {
            function,
            ledger: self.transport.ledger,
            slot: self.transport.slot,
        })
    }
}

fn scoped_storage_mapped_point_v29(
    mapped: &InstanceMappedSpanV1,
    offset: u32,
    call_offset: Option<u32>,
) -> Result<(BlockId, u32), CallInstanceEmissionErrorV1> {
    use CallInstanceEmissionErrorV1::StorageTransport as Refused;
    let original = mapped.source.coordinates().2;
    if offset >= original.count {
        return Err(Refused);
    }
    let removed = if let Some(call) = mapped.removed_call {
        let InstanceSpanSourceV1::Terminator(source) = mapped.source else {
            return Err(Refused);
        };
        if call.caller != mapped.instance || call.block != source.semantic_block {
            return Err(Refused);
        }
        let call = call_offset.ok_or(Refused)?;
        if call >= original.count || call == offset {
            return Err(Refused);
        }
        Some(call)
    } else {
        None
    };
    let mut remaining = offset - u32::from(removed.is_some_and(|call| offset > call));
    let mut total = 0u32;
    let mut selected = None;
    if mapped.segments[0].is_none() && mapped.segments[1].is_some() {
        return Err(Refused);
    }
    for segment in &mapped.segments {
        let Some(segment) = segment else {
            continue;
        };
        total = total
            .checked_add(segment.count)
            .ok_or_else(call_splice_arithmetic_v1)?;
        if selected.is_none() {
            if remaining < segment.count {
                selected = Some((
                    segment.block,
                    segment
                        .first
                        .checked_add(remaining)
                        .ok_or_else(call_splice_arithmetic_v1)?,
                ));
            } else {
                remaining -= segment.count;
            }
        }
    }
    if total != original.count - u32::from(removed.is_some()) {
        return Err(Refused);
    }
    selected.ok_or(Refused)
}

impl ScopedStorageCalleeV29<'_> {
    fn check(
        &self,
        function: &Function,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), CallInstanceEmissionErrorV1> {
        if self.ledger != budget.work_ledger_identity_v1()
            || self.slot != std::ptr::from_ref(&*budget) as usize
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.charge_work(1)?;
        if !std::ptr::eq(self.function, function) {
            return Err(CallInstanceEmissionErrorV1::StorageTransport);
        }
        Ok(())
    }
}
