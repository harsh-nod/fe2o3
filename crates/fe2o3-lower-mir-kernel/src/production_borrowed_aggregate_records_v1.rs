// Untrusted locators for independent source/canonical-KIR replay. No record is
// an initialization, lifetime, invariance, access, or nonescape certificate.
// Primary retains these beside the existing correspondence, never on a wire.
#[derive(Clone, Debug, Eq, PartialEq)]
struct BorrowedAggregateFieldPathV1 {
    // Relative to the dereferenced aggregate, in source field order.
    fields: Box<[u32]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BorrowedAggregateSourceAnchorV1 {
    Entry { local: SemanticLocalIdV1 },
    Occurrence(fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BorrowedAggregateSourceOwnerV1 {
    root: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    local: SemanticLocalIdV1,
    // Candidate source initialization/lifetime start. Replay derives epochs.
    lifetime_start: BorrowedAggregateSourceAnchorV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BorrowedAggregateValueLocatorV1 {
    function: FunctionId,
    value: ValueId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BorrowedAggregateOperationLocatorV1 {
    function: FunctionId,
    location: FunctionOperationLocation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum BorrowedAggregateCarrierCandidateV1 {
    ScalarCell {
        pointer: BorrowedAggregateValueLocatorV1,
        allocation: BorrowedAggregateOperationLocatorV1,
        initialization: BorrowedAggregateOperationLocatorV1,
    },
    CapturedReference {
        value: BorrowedAggregateValueLocatorV1,
    },
    WholeSlice {
        value: BorrowedAggregateValueLocatorV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BorrowedAggregateFieldCandidateV1 {
    owner: BorrowedAggregateSourceOwnerV1,
    path: BorrowedAggregateFieldPathV1,
    source_type: SemanticTypeIdV1,
    source_definition: BorrowedAggregateSourceAnchorV1,
    carrier: BorrowedAggregateCarrierCandidateV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BorrowedAggregateCallCandidateV1 {
    root: SemanticFunctionIdV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    call: BorrowedAggregateOperationLocatorV1,
    source_argument: u32,
    tuple_field: Option<u32>,
    actual_owner: BorrowedAggregateSourceOwnerV1,
    actual_path: BorrowedAggregateFieldPathV1,
    actual: BorrowedAggregateValueLocatorV1,
    physical_argument: u32,
    callee: SemanticFunctionIdV1,
    formal_local: SemanticLocalIdV1,
    formal_path: BorrowedAggregateFieldPathV1,
    formal: BorrowedAggregateValueLocatorV1,
    physical_parameter: u32,
}

#[derive(Clone, Copy)]
struct BorrowedAggregateReplayCandidatesV1<'a> {
    fields: &'a [BorrowedAggregateFieldCandidateV1],
    calls: &'a [BorrowedAggregateCallCandidateV1],
}

#[derive(Clone)]
struct BorrowedAggregateFormalV1 {
    local: SemanticLocalIdV1,
    path: BorrowedAggregateFieldPathV1,
    value: ValueId,
}

impl BorrowedAggregateFieldPathV1 {
    fn new_in(
        fields: &[u32],
        budget: &mut dyn BorrowedAggregateBudgetV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        budget.charge_work(
            fields
                .len()
                .checked_add(2)
                .ok_or(ArgumentResourceV1::Arithmetic)?,
        )?;
        if fields.is_empty() || fields.len() > MAX_SSA_VALUE_COMPONENTS_V1 {
            return Err(borrowed_aggregate_error_v1(
                "borrowed field path is empty or too deep",
            ));
        }
        let mut copy = borrowed_aggregate_vec_v1(fields.len(), budget)?;
        copy.extend_from_slice(fields);
        Ok(Self {
            fields: copy.into_boxed_slice(),
        })
    }

    fn clone_in(
        &self,
        budget: &mut dyn BorrowedAggregateBudgetV1,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        Self::new_in(&self.fields, budget)
    }
}

fn clone_borrowed_parameter_bindings_v1(
    rows: &[SemanticKirBorrowedParameterBindingV1],
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<Vec<SemanticKirBorrowedParameterBindingV1>, ProductionSemanticKirErrorV1> {
    let mut output = borrowed_aggregate_vec_v1(rows.len(), budget)?;
    for row in rows {
        budget.charge_work(8)?;
        let projection = BorrowedAggregateFieldPathV1::new_in(&row.projection, budget)?.fields;
        output.push(SemanticKirBorrowedParameterBindingV1 { projection, ..*row });
    }
    Ok(output)
}

fn borrowed_aggregate_append_rows_v1<T>(
    target: &mut Vec<T>,
    source: Vec<T>,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let released = argument_product_v1(source.capacity(), std::mem::size_of::<T>())?;
    for row in source {
        borrowed_aggregate_push_v1(target, row, budget)?;
    }
    budget.release_storage(released)?;
    Ok(())
}

// Input payload remains caller-accounted, including on error/unwind when it is
// consumed. Success reuses its allocation; all bucket capacity is scoped scratch.
fn order_borrowed_correspondence_records_v1<T>(
    mut records: Vec<T>,
    function_ordinals: &BTreeMap<(SemanticFunctionIdV1, SemanticFunctionIdV1), usize>,
    key: impl Fn(&T) -> (SemanticFunctionIdV1, SemanticFunctionIdV1),
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<T>, ProductionSemanticKirErrorV1> {
    if records.is_empty() {
        return Ok(records);
    }
    with_canonical_call_scratch_v1(budget, move |budget| {
        let functions = function_ordinals.len();
        let lookup = argument_product_v1(functions.checked_ilog2().unwrap_or(0) as usize + 2, 24)?;
        let row_work = argument_sum_v1(&[lookup, 2])?;
        budget.charge_work(argument_sum_v1(&[functions, 1])?)?;
        let mut buckets = borrowed_aggregate_vec_v1::<Vec<T>>(functions, budget)?;
        buckets.resize_with(functions, Vec::new);

        // Drain preserves the original capacity, so flattening cannot allocate.
        for record in records.drain(..) {
            budget.charge_work(row_work)?;
            let ordinal = function_ordinals
                .get(&key(&record))
                .copied()
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let bucket = buckets
                .get_mut(ordinal)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            borrowed_aggregate_push_v1(bucket, record, budget)?;
        }
        for mut bucket in buckets {
            budget.charge_work(argument_sum_v1(&[bucket.len(), 1])?)?;
            records.append(&mut bucket);
        }
        Ok(records)
    })
}

fn check_borrowed_aggregate_root_records_v1(
    formals: &[SemanticKirBorrowedParameterBindingV1],
    fields: &[BorrowedAggregateFieldCandidateV1],
    calls: &[BorrowedAggregateCallCandidateV1],
    root: SemanticFunctionIdV1,
    functions: &BTreeMap<SemanticFunctionIdV1, FunctionId>,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    // Merging deduplicates identical function IDs without renaming SSA values.
    // Locators therefore retain IDs; root-qualified source associations are
    // rechecked here instead of incorrectly rebasing IDs by a module ordinal.
    for row in formals {
        budget.charge_work(
            functions
                .len()
                .checked_add(4)
                .ok_or(ArgumentResourceV1::Arithmetic)?,
        )?;
        if row.correspondence_owner != root || !functions.contains_key(&row.semantic_function) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
    }
    for row in fields {
        let expected = functions
            .get(&row.owner.function)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        budget.charge_work(argument_sum_v1(&[
            functions.len(),
            argument_product_v1(expected.as_str().len(), 3)?,
            8,
        ])?)?;
        let physical_matches = match &row.carrier {
            BorrowedAggregateCarrierCandidateV1::ScalarCell {
                pointer,
                allocation,
                initialization,
            } => {
                &pointer.function == expected
                    && &allocation.function == expected
                    && &initialization.function == expected
            }
            BorrowedAggregateCarrierCandidateV1::CapturedReference { value }
            | BorrowedAggregateCarrierCandidateV1::WholeSlice { value } => {
                &value.function == expected
            }
        };
        if row.owner.root != root || !physical_matches {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
    }
    for row in calls {
        let caller = functions
            .get(&row.caller)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let callee = functions
            .get(&row.callee)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        budget.charge_work(argument_sum_v1(&[
            argument_product_v1(functions.len(), 3)?,
            argument_product_v1(caller.as_str().len(), 2)?,
            callee.as_str().len(),
            12,
        ])?)?;
        if row.root != root
            || row.actual_owner.root != root
            || !functions.contains_key(&row.actual_owner.function)
            || &row.call.function != caller
            || &row.actual.function != caller
            || &row.formal.function != callee
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
    }
    Ok(())
}

fn borrowed_aggregate_correspondence_bytes_v1(
    rows: &SemanticKirCorrespondenceV1,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<usize, ProductionSemanticKirErrorV1> {
    let mut bytes = argument_sum_v1(&[
        argument_product_v1(
            rows.borrowed_parameter_bindings.len(),
            std::mem::size_of::<SemanticKirBorrowedParameterBindingV1>(),
        )?,
        argument_product_v1(
            rows.borrowed_aggregate_fields.len(),
            std::mem::size_of::<BorrowedAggregateFieldCandidateV1>(),
        )?,
        argument_product_v1(
            rows.borrowed_aggregate_calls.len(),
            std::mem::size_of::<BorrowedAggregateCallCandidateV1>(),
        )?,
    ])?;
    for row in &rows.borrowed_parameter_bindings {
        budget.charge_work(2)?;
        bytes = argument_sum_v1(&[
            bytes,
            argument_product_v1(row.projection.len(), std::mem::size_of::<u32>())?,
        ])?;
    }
    for row in &rows.borrowed_aggregate_fields {
        budget.charge_work(6)?;
        let names = match &row.carrier {
            BorrowedAggregateCarrierCandidateV1::ScalarCell {
                pointer,
                allocation,
                initialization,
            } => argument_sum_v1(&[
                pointer.function.as_str().len(),
                allocation.function.as_str().len(),
                initialization.function.as_str().len(),
            ])?,
            BorrowedAggregateCarrierCandidateV1::CapturedReference { value }
            | BorrowedAggregateCarrierCandidateV1::WholeSlice { value } => {
                value.function.as_str().len()
            }
        };
        bytes = argument_sum_v1(&[
            bytes,
            names,
            argument_product_v1(row.path.fields.len(), std::mem::size_of::<u32>())?,
        ])?;
    }
    for row in &rows.borrowed_aggregate_calls {
        budget.charge_work(8)?;
        bytes = argument_sum_v1(&[
            bytes,
            row.call.function.as_str().len(),
            row.actual.function.as_str().len(),
            row.formal.function.as_str().len(),
            argument_product_v1(
                argument_sum_v1(&[row.actual_path.fields.len(), row.formal_path.fields.len()])?,
                std::mem::size_of::<u32>(),
            )?,
        ])?;
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "production_borrowed_aggregate_records_v1_tests.rs"]
mod borrowed_correspondence_order_tests_v1;
