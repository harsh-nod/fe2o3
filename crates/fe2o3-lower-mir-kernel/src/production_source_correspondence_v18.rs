include!("production_source_inventory_rows_v18.rs");
include!("production_source_rvalue_results_v30.rs");

// The attachment index contains source metadata only. All graph lookups reuse
// the caller's existing canonical inventory; no graph/definition index is built.
#[derive(Clone, Copy)]
struct SourceAttachmentV18 {
    key: TileAttachmentKeyV29,
    location: TileAttachmentLocationV29,
}

// These are borrowed evidence from the retained original producer, not a new
// allocation or epoch authority. A consumer must still join the actual access.
struct SourcePhysicalBackingV18<'a> {
    instance: usize,
    row: usize,
    slot: &'a ScopedSourceSlotV29,
}

fn source_allocation_shape_headers_v18() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        size_of::<()>(),
        argument_product_v1(2, size_of::<SourceOwnedResultV18<()>>())?,
        size_of::<&ScopedSourceSlotV29>(),
        size_of::<&Operation>(),
        size_of::<&Option<ValueId>>(),
        size_of::<&Type>(),
        size_of::<&u32>(),
        size_of::<(
            ScopedAllocationIdentityV29,
            ScopedAllocationSourceV29,
            ScopedSlotRepresentationV29,
        )>(),
        size_of::<ScopedScalarArraySlotV29>(),
        size_of::<Result<ScopedScalarArraySlotV29, ProductionSemanticKirErrorV1>>(),
        size_of::<SourceOwnedResultV18<ScopedScalarArraySlotV29>>(),
        size_of::<Option<&(ValueId, PrivateArrayPhysicalLocationV1)>>(),
        size_of::<&(ValueId, PrivateArrayPhysicalLocationV1)>(),
        size_of::<&ValueId>(),
        size_of::<Option<ValueId>>(),
        size_of::<SourceCorrespondenceWorkV18<'_, '_>>(),
        size_of::<SourceOwnedResultV18<bool>>(),
        size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
    ])
}

fn source_allocation_slot_headers_v18() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        size_of::<SourcePhysicalBackingV18<'_>>(),
        argument_product_v1(
            2,
            size_of::<SourceOwnedResultV18<SourcePhysicalBackingV18<'_>>>(),
        )?,
        size_of::<&ScopedModuleRootV29>(),
        size_of::<SourceOwnedResultV18<&ScopedModuleRootV29>>(),
        size_of::<Option<&ScopedSourceSlotV29>>(),
        size_of::<&ScopedSourceSlotV29>(),
        size_of::<SourceOwnedResultV18<&ScopedSourceSlotV29>>(),
        size_of::<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>(),
        size_of::<SourceOwnedResultV18<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1>>(),
        size_of::<&fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>(),
        size_of::<SourceOwnedResultV18<&fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>>(),
        size_of::<&Operation>(),
        size_of::<SourceOwnedResultV18<()>>(),
    ])
}

struct SourcePhysicalAccessV18<'a> {
    instance: usize,
    row: usize,
    anchor: &'a ScopedMemoryAnchorV29,
}

// A checked projection of an original capture, not an expression/currentness
// proof. The consuming relation must still check the exact source payload arm.
struct SourcePhysicalPayloadV18<'a> {
    source: &'a ScopedMemoryPayloadV29,
    value: ValueId,
    store_use: Option<fe2o3_kernel_ir::CanonicalKirUseCoordinateV1>,
}

struct SourcePhysicalObjectV18<'a> {
    instance: usize,
    row: usize,
    anchor: &'a ScopedMemoryAnchorV29,
    source: &'a ScopedObjectPayloadV29,
    actual: ScopedObjectPayloadV29,
}

fn source_attachment_key_v18(key: TileAttachmentKeyV29) -> [usize; 7] {
    [
        key.root,
        key.family as usize,
        key.instance,
        key.row,
        key.field as usize,
        key.component,
        key.part,
    ]
}

struct SourceCorrespondenceWorkV18<'b, 'w>(&'b mut ArgumentBudgetV1<'w>);
impl PrivateArrayChargeV1 for SourceCorrespondenceWorkV18<'_, '_> {
    type Error = ProductionSourceOwnedViewErrorV18;
    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.0.charge_work(amount).map_err(Into::into)
    }
}

fn source_attachment_error_v18(
    error: ScopedTileFailureKindV29,
) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        ScopedTileFailureKindV29::Resource(error) => error.into(),
        _ => {
            ProductionSourceOwnedViewErrorV18::Binding("original source attachment mapping differs")
        }
    }
}

fn reserve_source_correspondence_credit_v18(
    budget: &mut ArgumentBudgetV1<'_>,
    accepted: &std::cell::Cell<usize>,
    bytes: usize,
) -> SourceOwnedResultV18<()> {
    let total = argument_sum_v1(&[accepted.get(), bytes])?;
    budget.reserve_storage(bytes)?;
    accepted.set(total);
    Ok(())
}

#[cfg(test)]
fn source_attachments_v18(
    source: &ProductionSourceOwnedViewV18<'_>,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<SourceAttachmentV18>> {
    source_attachments_owned_v18(source, inventory, budget, &std::cell::Cell::new(0))
}

fn source_attachments_owned_v18(
    source: &ProductionSourceOwnedViewV18<'_>,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    accepted: &std::cell::Cell<usize>,
) -> SourceOwnedResultV18<Vec<SourceAttachmentV18>> {
    let mut count = 0usize;
    visit_source_attachment_inventory_v18(source.owner, inventory, budget, |_, _, budget| {
        budget.charge_work(1)?;
        count = count.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
        Ok(())
    })
    .map_err(source_attachment_error_v18)?;
    let bytes = argument_product_v1(count, size_of::<SourceAttachmentV18>())?;
    reserve_source_correspondence_credit_v18(budget, accepted, bytes)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| ArgumentResourceV1::Allocation)?;
    reserve_source_correspondence_credit_v18(
        budget,
        accepted,
        argument_product_v1(
            rows.capacity()
                .checked_sub(count)
                .ok_or(ArgumentResourceV1::Accounting)?,
            size_of::<SourceAttachmentV18>(),
        )?,
    )?;
    visit_source_attachment_inventory_v18(
        source.owner,
        inventory,
        budget,
        |key, location, budget| {
            budget.charge_work(1)?;
            if rows.len() >= count || rows.len() == rows.capacity() {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            }
            rows.push(SourceAttachmentV18 { key, location });
            Ok(())
        },
    )
    .map_err(source_attachment_error_v18)?;
    if rows.len() != count {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "incomplete source attachment census",
        ));
    }
    private_array_heapsort_v1(
        &mut rows,
        |row| source_attachment_key_v18(row.key),
        &mut SourceCorrespondenceWorkV18(budget),
        || ArgumentResourceV1::Arithmetic.into(),
    )?;
    for pair in rows.windows(2) {
        budget.charge_work(15)?;
        if source_attachment_key_v18(pair[0].key) >= source_attachment_key_v18(pair[1].key) {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "duplicate source attachment coordinate",
            ));
        }
    }
    Ok(rows)
}

/// One mapped occurrence of an original source statement or terminator.
/// Numeric coordinates remain local locators, not independent proof authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceOperationV18 {
    /// An operation in the exact retained canonical inventory.
    Operation(fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1),
    /// An authenticated zero-operation source span at this physical gap.
    Gap {
        /// Function/block coordinate in the original canonical graph.
        block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
        /// Gap before this operation, or the terminal gap.
        operation: u32,
    },
    /// A defined source call removed by checked instance expansion.
    RemovedCall,
    /// An explicitly retained source span with no physical output.
    NoOperations,
}

/// Scoped original-source instance attachments joined to one actual inventory.
/// This relation supplies exact coordinates to the shared translation engine;
/// it does not alone establish ranked equivalence, memory safety or execution.
pub struct ProductionSourceCorrespondenceV18<'scope> {
    source: &'scope ProductionSourceOwnedViewV18<'scope>,
    inventory: &'scope fe2o3_kernel_analysis::CanonicalKirInventoryV18<'scope>,
    attachments: &'scope [SourceAttachmentV18],
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
}

fn source_correspondence_owned_headers_v18<T, E, F>(_: &F) -> Result<usize, ArgumentResourceV1> {
    type Capture<'a, F> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        F,
    );
    type Catch<'a, 'work, F> = (
        Capture<'a, F>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a std::cell::Cell<usize>,
    );
    type Entry<F> = (Vec<SourceAttachmentV18>, usize, F);
    type Construction<'a, 'work, F> = (
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a std::cell::Cell<usize>,
        &'a F,
    );
    type Invoke<'a, 'work, F> = (
        F,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
    );
    argument_sum_v1(&[
        argument_product_v1(2, size_of::<Capture<'_, F>>())?,
        argument_product_v1(2, std::mem::align_of::<Capture<'_, F>>())?,
        size_of::<Catch<'_, '_, F>>(),
        size_of::<std::panic::AssertUnwindSafe<Catch<'_, '_, F>>>(),
        size_of::<Entry<F>>(),
        argument_product_v1(2, size_of::<SourceOwnedResultV18<Entry<F>>>())?,
        size_of::<std::thread::Result<SourceOwnedResultV18<Entry<F>>>>(),
        size_of::<std::panic::AssertUnwindSafe<Entry<F>>>(),
        size_of::<std::thread::Result<()>>(),
        size_of::<Construction<'_, '_, F>>(),
        size_of::<std::panic::AssertUnwindSafe<Construction<'_, '_, F>>>(),
        size_of::<SourceOwnedResultV18<Vec<SourceAttachmentV18>>>(),
        size_of::<std::thread::Result<SourceOwnedResultV18<Vec<SourceAttachmentV18>>>>(),
        size_of::<std::cell::Cell<usize>>(),
        argument_product_v1(8, size_of::<usize>())?,
        size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
        size_of::<SourceOwnedResultV18<()>>(),
        size_of::<Invoke<'_, '_, F>>(),
        size_of::<std::panic::AssertUnwindSafe<Invoke<'_, '_, F>>>(),
        size_of::<ProductionSourceCorrespondenceV18<'_>>(),
        size_of::<std::thread::Result<Result<T, E>>>(),
        size_of::<Result<T, E>>(),
        size_of::<std::panic::AssertUnwindSafe<Result<T, E>>>(),
        source_reference_cleanup_headers_v29()?,
        source_rvalue_headers_v30()?,
    ])
}

impl ProductionSourceOwnedViewV18<'_> {
    /// Reuses the original attachment mapper over this source and inventory.
    /// No graph/index copy or legacy correspondence owner is constructed.
    /// The scoped metadata, source and graph remain paid until the callback
    /// completes. Only known metadata credits are refunded on success.
    pub fn with_ranked_correspondence_v18<'work, T, E>(
        &self,
        inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &ProductionSourceCorrespondenceV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        let floor = budget.storage();
        let slot = std::ptr::from_ref(budget) as usize;
        let ledger = budget.work_ledger_identity_v1();
        let accepted = std::cell::Cell::new(0);
        // The owned callback stays inside the catch through early validation
        // and every construction refusal. No generic helper credit is borrowed.
        let caught = {
            let budget = &mut *budget;
            let accepted = &accepted;
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                self.query(budget)?;
                if !inventory.belongs_to(&self.owner.inner.pending.graph) {
                    return self.missing("foreign canonical inventory");
                }
                let rows = self.retain_construction(|| {
                    let headers = argument_sum_v1(&[
                        source_correspondence_owned_headers_v18::<T, E, _>(&consume)?,
                        source_owned_finish_preflight_v26::<T, E>(budget)?,
                    ])?;
                    reserve_source_correspondence_credit_v18(budget, accepted, headers)?;
                    source_attachments_owned_v18(self, inventory, budget, accepted)
                })?;
                Ok::<_, ProductionSourceOwnedViewErrorV18>((rows, accepted.get(), consume))
            }))
        };
        let custody =
            self.observe_correspondence_entry_v18(budget, floor, accepted.get(), slot, ledger);
        let (rows, storage, consume) = match caught {
            Ok(Ok(entry)) if custody.is_ok() => entry,
            Ok(Ok(entry)) => {
                let disposed =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(entry)));
                let _ = self.observe_correspondence_entry_v18(
                    budget,
                    floor,
                    accepted.get(),
                    slot,
                    ledger,
                );
                if let Err(payload) = disposed {
                    std::panic::resume_unwind(payload);
                }
                return self
                    .retain_query(Err(ArgumentResourceV1::Accounting.into()))
                    .map_err(Into::into);
            }
            Ok(Err(error)) => {
                if custody.is_ok() && budget.release_storage(accepted.get()).is_err() {
                    self.cleanup.deny_refund();
                }
                return self.retain_query(Err(error)).map_err(Into::into);
            }
            Err(payload) => {
                if custody.is_ok() && budget.release_storage(accepted.get()).is_err() {
                    self.cleanup.deny_refund();
                }
                std::panic::resume_unwind(payload);
            }
        };
        let view = ProductionSourceCorrespondenceV18 {
            source: self,
            inventory,
            attachments: &rows,
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
        };
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            view.check(budget).map_err(E::from)?;
            consume(&view, budget)
        }));
        let prior = self.guard.first.get();
        let postflight = if matches!(&caught, Ok(Ok(_))) {
            view.check(budget)
        } else {
            view.observe_custody(budget)
        };
        drop(view);
        drop(rows);
        source_owned_finish_callback_v18(caught, prior, postflight, self.cleanup, budget, storage)
    }

    fn observe_correspondence_entry_v18(
        &self,
        budget: &ArgumentBudgetV1<'_>,
        floor: usize,
        accepted: usize,
        slot: usize,
        ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    ) -> SourceOwnedResultV18<()> {
        // No public callback has run yet. Any unexplained construction residue
        // is retained, rather than being mistaken for caller-owned row credit.
        if slot != std::ptr::from_ref(budget) as usize
            || ledger != budget.work_ledger_identity_v1()
            || floor.checked_add(accepted) != Some(budget.storage())
        {
            self.cleanup.deny_refund();
        }
        self.guard.observe_custody(self.cleanup, budget)
    }
}

impl ProductionSourceCorrespondenceV18<'_> {
    fn observe_custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            self.source.cleanup.deny_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        }
        self.source
            .guard
            .observe_custody(self.source.cleanup, budget)
    }

    fn check(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        if self.observe_custody(budget).is_err() {
            return self
                .source
                .guard
                .reject(SourceOwnedQueryFailureV18::Resource(
                    ArgumentResourceV1::Accounting,
                ));
        }
        self.source
            .guard
            .check(self.source.owner, self.source.cleanup, budget)
    }

    fn query(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.check(budget)?;
        self.source.query(budget)
    }

    fn retain_query<T>(&self, result: SourceOwnedResultV18<T>) -> SourceOwnedResultV18<T> {
        self.source.retain_query(result)
    }

    // This classifies the historical source access-ordinal census only. It is
    // not permission to omit a local effect from semantic/formal obligations.
    fn retained_scalar_allocation(
        &self,
        root: usize,
        operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<bool> {
        let Some(backing) = self.retained_allocation(root, operation, budget)? else {
            return Ok(false);
        };
        budget.charge_work(1)?;
        Ok(match backing.slot.representation {
            ScopedSlotRepresentationV29::ScalarArray(scalar) => {
                scalar.count.is_none() && scalar.length == 1
            }
            ScopedSlotRepresentationV29::Object { .. } => false,
        })
    }

    fn retained_allocation(
        &self,
        root: usize,
        operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<SourcePhysicalBackingV18<'_>>> {
        self.retain_query((|| {
            self.query(budget)?;
            let root_row = self.source.root_row(root)?;
            budget.charge_work(4)?;
            if operation.block.function.0 as usize != root_row.function_ordinal {
                return self.source.missing("retained allocation changed root");
            }
            let actual = self.inventory.functions().get(root_row.function_ordinal)
                .and_then(|row| row.function.body.as_ref())
                .and_then(|body| body.blocks.get(operation.block.block as usize))
                .and_then(|block| block.operations.get(operation.operation as usize))
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding("retained allocation coordinate"))?;
            let OperationKind::Alloca { address_space: AddressSpace::Private, .. } = &actual.kind else {
                return Ok(None);
            };
            let [result] = actual.results.as_slice() else {
                return self.source.missing("retained allocation result census");
            };
            let mut matched = None;
            if root_row.source_slots.instances.len() != root_row.sidecars.rows.len() {
                return self.source.missing("retained allocation active instance census");
            }
            for (ordinal, owner) in root_row.source_slots.instances.iter().enumerate() {
                budget.charge_work(3)?;
                let instance = owner.instance.index();
                if self.source.active_ordinal(root, instance, budget)? != Some(ordinal) {
                    return self.source.missing("retained allocation source instance");
                }
                let sidecar = self.source.sidecar(root, instance, budget)?;
                let first = sidecar.scoped_slot_origins.as_ref().map_or(0, |rows| rows.len());
                let slots = root_row.source_slots.slots.get(owner.slots.clone())
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding("retained allocation slot interval"))?;
                for (offset, slot) in slots.iter().enumerate() {
                    budget.charge_work(3)?;
                    if slot.origin.pointer != result.id { continue; }
                    if matched.is_some() || slot.instance != owner.instance {
                        return self.source.missing("retained allocation has ambiguous or changed backing");
                    }
                    self.retained_allocation_shape_v18(slot, actual, budget)?;
                    let rows = self.attachment_range(TileAttachmentKeyV29 {
                        root, family: TileAttachmentFamilyV29::SourceSlot, instance,
                        row: first.checked_add(offset).ok_or(ArgumentResourceV1::Arithmetic)?,
                        field: TileAttachmentFieldV29::SlotAllocation, component: 0, part: 0,
                    }, budget)?;
                    let [row] = rows else { return self.source.missing("retained allocation attachment census"); };
                    if !matches!(row.location, TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(point))
                        if point.function == operation.block.function.0 as usize
                        && point.block == operation.block.block as usize
                        && point.operation == operation.operation as usize) {
                        return self.source.missing("retained allocation attachment differs from current definition");
                    }
                    matched = Some(SourcePhysicalBackingV18 {
                        instance,
                        row: owner.slots.start.checked_add(offset).ok_or(ArgumentResourceV1::Arithmetic)?,
                        slot,
                    });
                }
            }
            Ok(matched)
        })())
    }

    // The existing whole-inventory locator and the slot-directed caller share
    // the same representation check. No schema/identity is inferred from bits.
    fn retained_allocation_shape_v18(
        &self,
        slot: &ScopedSourceSlotV29,
        actual: &Operation,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        budget.reserve_storage(source_allocation_shape_headers_v18()?)?;
        budget.charge_work(3)?;
        let OperationKind::Alloca {
            address_space: AddressSpace::Private,
            count,
            element: element_type,
            alignment,
        } = &actual.kind
        else {
            return self
                .source
                .missing("retained allocation changed source kind");
        };
        match (
            slot.origin.identity,
            slot.origin.source,
            slot.representation,
        ) {
            (
                ScopedAllocationIdentityV29::LegacyLocal(_),
                ScopedAllocationSourceV29::Legacy | ScopedAllocationSourceV29::OriginalArray { .. },
                ScopedSlotRepresentationV29::ScalarArray(scalar),
            ) => {
                slot.scalar_array()
                    .map_err(|error| source_attachment_error_v18(error.into()))?;
                if scalar.count.as_ref().map(|(value, _)| *value) != *count
                    || scalar.element.alignment != *alignment
                    || !scalar
                        .element
                        .element
                        .matches_borrowed(element_type, &mut SourceCorrespondenceWorkV18(budget))?
                {
                    return self
                        .source
                        .missing("retained scalar allocation changed representation");
                }
            }
            (
                ScopedAllocationIdentityV29::OriginalObject { .. },
                ScopedAllocationSourceV29::OriginalObject {
                    schema: original, ..
                },
                ScopedSlotRepresentationV29::Object {
                    schema, alignment, ..
                },
            ) if original == schema => {
                check_scoped_object_alloca_v29(
                    actual,
                    slot.origin.pointer,
                    schema,
                    alignment,
                    budget,
                )
                .map_err(|error| source_attachment_error_v18(error.into()))?;
            }
            _ => {
                return self
                    .source
                    .missing("retained allocation changed source kind");
            }
        }
        Ok(())
    }

    // Only an authentic retained slot ordinal selects this path. The attachment
    // helper rejoins its exact active instance interval and original operation.
    // The sole production slot constructor appends to one root-wide vector and
    // rejects duplicate pointers before each push. Assembly moves that owner
    // unchanged; relocation only borrows it. This authenticated invariant, not
    // a caller-supplied table, preserves the full locator's uniqueness check.
    // This avoids the allocation-by-all-slots scan in a complete slot traversal.
    fn retained_allocation_for_slot_v18(
        &self,
        root: usize,
        slot_ordinal: usize,
        operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<SourcePhysicalBackingV18<'_>> {
        self.retain_query((|| {
            self.query(budget)?;
            budget.reserve_storage(source_allocation_slot_headers_v18()?)?;
            let owner = self.source.root_row(root)?;
            budget.charge_work(4)?;
            if owner.source_slots.instances.len() != owner.sidecars.rows.len() {
                return self
                    .source
                    .missing("retained allocation active instance census");
            }
            let slot = owner.source_slots.slots.get(slot_ordinal).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("retained allocation slot ordinal"),
            )?;
            let input =
                scoped_raw_admission_v29::source_slot_input_v18(self, root, slot_ordinal, budget)?;
            if input != operation {
                return self
                    .source
                    .missing("retained allocation slot changed original operation");
            }
            let actual = source_operation_row_v18(self.inventory, operation, budget)?.operation;
            self.retained_allocation_shape_v18(slot, actual, budget)?;
            Ok(SourcePhysicalBackingV18 {
                instance: slot.instance.index(),
                row: slot_ordinal,
                slot,
            })
        })())
    }

    fn attachment_value(
        &self,
        root: usize,
        location: TileAttachmentLocationV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ValueId> {
        self.retain_query((|| {
            self.query(budget)?;
            budget.charge_work(4)?;
            let expected = self.source.root_row(root)?.function_ordinal;
            let body = self
                .inventory
                .functions()
                .get(expected)
                .and_then(|row| row.function.body.as_ref())
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source value function",
                ))?;
            let value = match location {
                TileAttachmentLocationV29::Origin(TileScalarSourceV29::FunctionParameter {
                    function,
                    parameter,
                }) if function == expected => body.parameters.get(parameter).copied(),
                TileAttachmentLocationV29::Origin(TileScalarSourceV29::BlockParameter {
                    function,
                    block,
                    parameter,
                }) if function == expected => body
                    .blocks
                    .get(block)
                    .and_then(|row| row.parameters.get(parameter))
                    .map(|row| row.id),
                TileAttachmentLocationV29::Origin(TileScalarSourceV29::Result {
                    operation,
                    result,
                }) if operation.function == expected => body
                    .blocks
                    .get(operation.block)
                    .and_then(|row| row.operations.get(operation.operation))
                    .and_then(|row| row.results.get(result))
                    .map(|row| row.id),
                _ => None,
            };
            value.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source value attachment is not this root's definition",
            ))
        })())
    }

    // The source mapper has already replayed relocation and synthetic insertion.
    // Join both the operation and pointer attachments, not raw pre-splice IDs.
    fn retained_memory_access(
        &self,
        root: usize,
        operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        pointer: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<SourcePhysicalAccessV18<'_>>> {
        self.retain_query((|| {
            self.query(budget)?;
            let root_row = self.source.root_row(root)?;
            if operation.block.function.0 as usize != root_row.function_ordinal {
                return self.source.missing("source access changed root");
            }
            let mut found = None;
            for source in &root_row.coordinates.sources.rows {
                budget.charge_work(1)?;
                let instance = source.instance.index();
                let Some(sidecar) = self.source.optional_sidecar(root, instance, budget)? else {
                    continue;
                };
                let Some(anchors) = &sidecar.scoped_memory_anchors else {
                    return self
                        .source
                        .missing("source instance lacks memory anchor census");
                };
                if anchors.subject.instance != source.instance
                    || anchors.subject.function != source.function
                    || anchors.subject.ledger != self.ledger
                {
                    return self.source.missing("source memory anchor owner differs");
                }
                for (row, anchor) in anchors.rows.iter().enumerate() {
                    budget.charge_work(1)?;
                    if !matches!(
                        anchor.kind,
                        ScopedMemoryAnchorKindV29::Access { .. }
                            | ScopedMemoryAnchorKindV29::Object(_)
                    ) {
                        continue;
                    }
                    let key = TileAttachmentKeyV29 {
                        root,
                        family: TileAttachmentFamilyV29::MemoryAnchor,
                        instance,
                        row,
                        field: TileAttachmentFieldV29::MemoryPosition,
                        component: 0,
                        part: 0,
                    };
                    let [mapped] = self.attachment_range(key, budget)? else {
                        return self.source.missing("source memory access position census");
                    };
                    if self.mapped_source_operation(mapped.location, budget)?
                        != ProductionSourceOperationV18::Operation(operation)
                    {
                        continue;
                    }
                    if matches!(anchor.kind, ScopedMemoryAnchorKindV29::Object(_)) {
                        return self.source.missing(
                            "typed object requires the complete object correspondence query",
                        );
                    }
                    let [definition] = self.attachment_range(
                        TileAttachmentKeyV29 {
                            field: TileAttachmentFieldV29::MemoryPointer,
                            ..key
                        },
                        budget,
                    )?
                    else {
                        return self.source.missing("source memory access pointer census");
                    };
                    if found.is_some()
                        || self.attachment_value(root, definition.location, budget)? != pointer
                    {
                        return self
                            .source
                            .missing("source memory access pointer or occurrence differs");
                    }
                    found = Some(SourcePhysicalAccessV18 {
                        instance,
                        row,
                        anchor,
                    });
                }
            }
            Ok(found)
        })())
    }

    fn retained_object_payload_v29(
        &self,
        root: usize,
        operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<SourcePhysicalObjectV18<'_>>> {
        use TileAttachmentFieldV29 as Field;
        self.retain_query((|| {
            self.query(budget)?;
            let root_row = self.source.root_row(root)?;
            if operation.block.function.0 as usize != root_row.function_ordinal {
                return self.source.missing("typed object changed physical root");
            }
            let actual = self
                .inventory
                .functions()
                .get(root_row.function_ordinal)
                .and_then(|row| row.function.body.as_ref())
                .and_then(|body| body.blocks.get(operation.block.block as usize))
                .and_then(|block| block.operations.get(operation.operation as usize))
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "typed object actual operation",
                ))?;
            if !matches!(actual.kind, OperationKind::Storage(_)) {
                return Ok(None);
            }
            let mut found = None;
            for source in &root_row.coordinates.sources.rows {
                budget.charge_work(1)?;
                let instance = source.instance.index();
                let Some(sidecar) = self.source.optional_sidecar(root, instance, budget)? else {
                    continue;
                };
                let anchors = sidecar.scoped_memory_anchors.as_ref().ok_or(
                    ProductionSourceOwnedViewErrorV18::Binding("typed object original census"),
                )?;
                if anchors.subject.instance != source.instance
                    || anchors.subject.function != source.function
                    || anchors.subject.ledger != self.ledger
                {
                    return self.source.missing("typed object changed original owner");
                }
                for (row, anchor) in anchors.rows.iter().enumerate() {
                    budget.charge_work(1)?;
                    if !matches!(anchor.kind, ScopedMemoryAnchorKindV29::Object(_)) {
                        continue;
                    }
                    let key = TileAttachmentKeyV29 {
                        root,
                        family: TileAttachmentFamilyV29::MemoryAnchor,
                        instance,
                        row,
                        field: Field::MemoryPosition,
                        component: 0,
                        part: 0,
                    };
                    let [position] = self.attachment_range(key, budget)? else {
                        return self.source.missing("typed object position census");
                    };
                    if self.mapped_source_operation(position.location, budget)?
                        != ProductionSourceOperationV18::Operation(operation)
                    {
                        continue;
                    }
                    if found.is_some() {
                        return self.source.missing("duplicate typed object correspondence");
                    }
                    found =
                        Some(self.retained_object_payload_at_v29(
                            root, instance, row, operation, budget,
                        )?);
                }
            }
            if found.is_none() {
                return self
                    .source
                    .missing("actual typed operation lacks original correspondence");
            }
            Ok(found)
        })())
    }

    // A batch consumer visits original anchors once and uses this exact row
    // query. Completeness/duplicate checks stay with that existing census.
    fn retained_object_payload_at_v29(
        &self,
        root: usize,
        instance: usize,
        row: usize,
        operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<SourcePhysicalObjectV18<'_>> {
        use TileAttachmentFieldV29 as Field;
        self.retain_query((|| {
            self.query(budget)?;
            let root_row = self.source.root_row(root)?;
            if operation.block.function.0 as usize != root_row.function_ordinal {
                return self.source.missing("typed object changed physical root");
            }
            let (function, _) = self.source.instance(root, instance, budget)?;
            let sidecar = self.source.sidecar(root, instance, budget)?;
            let anchors = sidecar.scoped_memory_anchors.as_ref().ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("typed object original census"),
            )?;
            budget.charge_work(5)?;
            if anchors.subject.instance.index() != instance
                || anchors.subject.function != function
                || anchors.subject.ledger != self.ledger
            {
                return self.source.missing("typed object changed original owner");
            }
            let anchor =
                anchors
                    .rows
                    .get(row)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "typed object original row",
                    ))?;
            if !matches!(anchor.kind, ScopedMemoryAnchorKindV29::Object(_)) {
                return self
                    .source
                    .missing("typed object row is not an Object anchor");
            }
            let key = TileAttachmentKeyV29 {
                root,
                family: TileAttachmentFamilyV29::MemoryAnchor,
                instance,
                row,
                field: Field::MemoryPosition,
                component: 0,
                part: 0,
            };
            let one =
                |field, component, budget: &mut ArgumentBudgetV1<'_>| -> SourceOwnedResultV18<_> {
                    let [row] = self.attachment_range(
                        TileAttachmentKeyV29 {
                            field,
                            component,
                            ..key
                        },
                        budget,
                    )?
                    else {
                        return self.source.missing("typed object attachment census");
                    };
                    budget.charge_work(1)?;
                    if row.key.part != 0 {
                        return self.source.missing("typed object attachment census");
                    }
                    Ok(row.location)
                };
            let position = one(Field::MemoryPosition, 0, budget)?;
            if self.mapped_source_operation(position, budget)?
                != ProductionSourceOperationV18::Operation(operation)
            {
                return self
                    .source
                    .missing("typed object row changed its exact operation");
            }
            let actual = self
                .inventory
                .functions()
                .get(root_row.function_ordinal)
                .and_then(|row| row.function.body.as_ref())
                .and_then(|body| body.blocks.get(operation.block.block as usize))
                .and_then(|block| block.operations.get(operation.operation as usize))
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "typed object actual operation",
                ))?;
            if !matches!(actual.kind, OperationKind::Storage(_)) {
                return self
                    .source
                    .missing("typed object row does not name an actual Storage operation");
            }
            let payload = anchors
                .object_payload(anchor, budget)
                .map_err(|error| source_attachment_error_v18(error.into()))?;
            for field in [
                Field::MemoryPointer,
                Field::MemoryLoadResult,
                Field::MemoryStoreValue,
                Field::MemoryStoreUse,
            ] {
                if one(field, 0, budget)? != TileAttachmentLocationV29::NoOutput {
                    return self
                        .source
                        .missing("typed object acquired scalar payload authority");
                }
            }
            let mut values = [None; 2];
            let mut operand_count = 0;
            for (component, original) in payload.operands().into_iter().enumerate() {
                let definition = one(Field::ObjectOperand, component, budget)?;
                let usage = one(Field::ObjectOperandUse, component, budget)?;
                match original {
                    Some(_) => {
                        let expected =
                            fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                                operation,
                                operand: u32::try_from(component)
                                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                            };
                        if usage != TileAttachmentLocationV29::Use(expected) {
                            return self
                                .source
                                .missing("typed object changed exact operand use");
                        }
                        values[component] = Some(self.attachment_value(root, definition, budget)?);
                        operand_count += 1;
                    }
                    None if definition == TileAttachmentLocationV29::NoOutput
                        && usage == TileAttachmentLocationV29::NoOutput => {}
                    None => {
                        return self
                            .source
                            .missing("absent typed operand acquired authority");
                    }
                }
            }
            let result = one(Field::ObjectResult, 0, budget)?;
            let result = match payload.result {
                Some(_) => Some(self.attachment_value(root, result, budget)?),
                None if result == TileAttachmentLocationV29::NoOutput => None,
                None => return self.source.missing("typed effect acquired a result"),
            };
            let mut mapped = *payload;
            let mut ordinal = 0;
            mapped
                .try_map_values(|_| {
                    let value = if ordinal < operand_count {
                        values[ordinal]
                    } else {
                        result
                    };
                    ordinal += 1;
                    value.ok_or_else(scoped_object_error_v29)
                })
                .map_err(|error| source_attachment_error_v18(error.into()))?;
            mapped
                .check_operation(actual, budget)
                .map_err(|error| source_attachment_error_v18(error.into()))?;
            Ok(SourcePhysicalObjectV18 {
                instance,
                row,
                anchor,
                source: payload,
                actual: mapped,
            })
        })())
    }

    fn retained_scalar_payload_v18<'a>(
        &'a self,
        root: usize,
        operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        access: &SourcePhysicalAccessV18<'a>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<SourcePhysicalPayloadV18<'a>>> {
        use TileAttachmentFieldV29 as Field;
        self.retain_query((|| {
            self.query(budget)?;
            let anchors = self
                .source
                .sidecar(root, access.instance, budget)?
                .scoped_memory_anchors
                .as_ref()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "scalar payload original anchor census",
                ))?;
            if !anchors
                .rows
                .get(access.row)
                .is_some_and(|anchor| std::ptr::eq(anchor, access.anchor))
            {
                return self
                    .source
                    .missing("scalar payload changed original owner or instance");
            }
            let key = TileAttachmentKeyV29 {
                root,
                family: TileAttachmentFamilyV29::MemoryAnchor,
                instance: access.instance,
                row: access.row,
                field: Field::MemoryPosition,
                component: 0,
                part: 0,
            };
            let [position] = self.attachment_range(key, budget)? else {
                return self.source.missing("scalar payload position census");
            };
            if self.mapped_source_operation(position.location, budget)?
                != ProductionSourceOperationV18::Operation(operation)
            {
                return self
                    .source
                    .missing("scalar payload changed physical operation");
            }
            let one = |field,
                       budget: &mut ArgumentBudgetV1<'_>|
             -> SourceOwnedResultV18<TileAttachmentLocationV29> {
                let [row] = self.attachment_range(TileAttachmentKeyV29 { field, ..key }, budget)?
                else {
                    return self.source.missing("scalar payload projection census");
                };
                Ok(row.location)
            };
            let load = one(Field::MemoryLoadResult, budget)?;
            let store = one(Field::MemoryStoreValue, budget)?;
            let store_use = one(Field::MemoryStoreUse, budget)?;
            let ScopedMemoryAnchorKindV29::Access { payload, .. } = &access.anchor.kind else {
                return self.source.missing("scalar payload is not an access");
            };
            let Some(source) = payload else {
                if [load, store, store_use]
                    .iter()
                    .any(|location| *location != TileAttachmentLocationV29::NoOutput)
                {
                    return self
                        .source
                        .missing("absent scalar payload acquired value authority");
                }
                return Ok(None);
            };
            budget.charge_work(4)?;
            let expected_function = self.source.root_row(root)?.function_ordinal;
            if operation.block.function.0 as usize != expected_function {
                return self.source.missing("scalar payload physical root differs");
            }
            let actual = self
                .inventory
                .functions()
                .get(operation.block.function.0 as usize)
                .and_then(|row| row.function.body.as_ref())
                .and_then(|body| body.blocks.get(operation.block.block as usize))
                .and_then(|block| block.operations.get(operation.operation as usize))
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "scalar payload actual operation",
                ))?;
            match source {
                ScopedMemoryPayloadV29::Load { .. } | ScopedMemoryPayloadV29::IndexLoad { .. } => {
                    if store != TileAttachmentLocationV29::NoOutput
                        || store_use != TileAttachmentLocationV29::NoOutput
                        || !matches!(
                            actual.kind,
                            OperationKind::Load { .. } | OperationKind::GuardedLoad { .. }
                        )
                    {
                        return self
                            .source
                            .missing("load payload acquired a Store use or changed operation");
                    }
                    let value = self.attachment_value(root, load, budget)?;
                    let [result] = actual.results.as_slice() else {
                        return self.source.missing("load payload result census");
                    };
                    if result.id != value {
                        return self.source.missing("load payload changed actual result");
                    }
                    Ok(Some(SourcePhysicalPayloadV18 {
                        source,
                        value,
                        store_use: None,
                    }))
                }
                ScopedMemoryPayloadV29::Store { .. } => {
                    if load != TileAttachmentLocationV29::NoOutput {
                        return self
                            .source
                            .missing("Store payload acquired a load definition");
                    }
                    let value = self.attachment_value(root, store, budget)?;
                    let [pointer] = self.attachment_range(
                        TileAttachmentKeyV29 {
                            field: Field::MemoryPointer,
                            ..key
                        },
                        budget,
                    )?
                    else {
                        return self.source.missing("Store payload pointer census");
                    };
                    let pointer = self.attachment_value(root, pointer.location, budget)?;
                    let operand = tile_store_payload_operand_v18(actual, pointer, value, budget)
                        .map_err(source_attachment_error_v18)?;
                    let expected = fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                        operation,
                        operand,
                    };
                    if store_use != TileAttachmentLocationV29::Use(expected) {
                        return self
                            .source
                            .missing("Store payload is not the exact actual RHS use");
                    }
                    Ok(Some(SourcePhysicalPayloadV18 {
                        source,
                        value,
                        store_use: Some(expected),
                    }))
                }
            }
        })())
    }

    fn attachment_range(
        &self,
        mut key: TileAttachmentKeyV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[SourceAttachmentV18]> {
        self.retain_query((|| {
            self.query(budget)?;
            key.part = 0;
            let start = private_array_partition_v1(
                self.attachments,
                |row| source_attachment_key_v18(row.key),
                source_attachment_key_v18(key),
                false,
                &mut SourceCorrespondenceWorkV18(budget),
            )?;
            key.part = usize::MAX;
            let end = private_array_partition_v1(
                self.attachments,
                |row| source_attachment_key_v18(row.key),
                source_attachment_key_v18(key),
                true,
                &mut SourceCorrespondenceWorkV18(budget),
            )?;
            let rows = self.attachments.get(start..end).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding("source attachment interval"),
            )?;
            if rows.is_empty() {
                return self.source.missing("missing source attachment");
            }
            Ok(rows)
        })())
    }

    fn generated_recipe_values_v18(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<GeneratedRecipeValuesV18> {
        self.retain_query((|| {
            self.query(budget)?;
            let original_root = self.source.root(root, budget)?.0;
            let function = self.source.instance(root, instance, budget)?.0;
            let sidecar = self.source.sidecar(root, instance, budget)?;
            budget.charge_work(sidecar.generated_terminator_values.len())?;
            let mut matching = sidecar
                .generated_terminator_values
                .iter()
                .enumerate()
                .filter(|(_, row)| {
                    row.correspondence_owner == original_root
                        && row.semantic_function == function
                        && row.semantic_block == block
                });
            let (ordinal, values) =
                matching
                    .next()
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "generated recipe has no original value row",
                    ))?;
            if matching.next().is_some() {
                return self
                    .source
                    .missing("generated recipe has ambiguous original value rows");
            }
            let invocation_rows = match &sidecar.invocation_entry {
                Some(entry) => argument_sum_v1(&[
                    1,
                    entry.arguments.len(),
                    entry.inputs.len(),
                    entry.components.len(),
                ])?,
                None => 0,
            };
            let row = argument_sum_v1(&[
                invocation_rows,
                sidecar.blocks.len(),
                sidecar.statement_operation_spans.len(),
                sidecar.terminator_operation_spans.len(),
                ordinal,
            ])?;
            let key = TileAttachmentKeyV29 {
                root,
                family: TileAttachmentFamilyV29::RawSidecar,
                instance,
                row,
                field: TileAttachmentFieldV29::RawGeneratedInput,
                component: 0,
                part: 0,
            };
            let [input] = self.attachment_range(key, budget)? else {
                return self
                    .source
                    .missing("generated recipe input attachment census");
            };
            let [output] = self.attachment_range(
                TileAttachmentKeyV29 {
                    field: TileAttachmentFieldV29::RawGeneratedOutput,
                    ..key
                },
                budget,
            )?
            else {
                return self
                    .source
                    .missing("generated recipe output attachment census");
            };
            Ok(GeneratedRecipeValuesV18 {
                destination_local: values.destination_local,
                input: self.attachment_value(root, input.location, budget)?,
                output: self.attachment_value(root, output.location, budget)?,
            })
        })())
    }

    /// Checks this relation's existing custody and source latch without another debit.
    pub fn check_query_v18(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.check(budget)
    }

    /// Retains a delegated query's resource refusal in the original source owner.
    pub fn retain_query_resource_error_v18(
        &self,
        error: ArgumentResourceV1,
    ) -> ProductionSourceOwnedViewErrorV18 {
        self.source.retain_query_resource_error_v18(error)
    }

    /// Borrows the same original-source view retained by this relation.
    pub fn source(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&ProductionSourceOwnedViewV18<'_>> {
        self.query(budget)?;
        Ok(self.source)
    }

    /// Borrows the one already paid canonical inventory.
    pub fn inventory(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>> {
        self.query(budget)?;
        Ok(self.inventory)
    }

    /// Selects a source function only when it has one active instance in this root.
    /// Repeated active invocations are an ambiguity. Original inactive IDs remain
    /// queryable but cannot stand in for a physical selected body.
    pub fn unique_source_instance(
        &self,
        root: usize,
        function: SemanticFunctionIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<usize>> {
        self.retain_query((|| {
            self.query(budget)?;
            let rows = &self.source.root_row(root)?.coordinates.sources.rows;
            budget.charge_work(rows.len())?;
            let mut first = None;
            for row in rows {
                if row.function != function
                    || !self
                        .source
                        .instance_active(root, row.instance.index(), budget)?
                {
                    continue;
                }
                if first.replace(row.instance.index()).is_some() {
                    return self
                        .source
                        .missing("source function has multiple call instances");
                }
            }
            Ok(first)
        })())
    }

    /// Resolves an original defined call to its exact child instance. Repeated
    /// helper functions remain distinct; a missing physical function is unused.
    pub fn defined_call_instance(
        &self,
        root: usize,
        caller: usize,
        block: SemanticBlockIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        self.retain_query((|| {
            self.query(budget)?;
            let semantic = self.source.source_semantic(budget)?;
            let function = self.source.instance(root, caller, budget)?.0;
            let terminator = semantic
                .functions()
                .get(function.index() as usize)
                .and_then(|function| function.blocks().get(block.index() as usize))
                .map(|block| block.terminator().kind())
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "original defined call site",
                ))?;
            let callee = match terminator {
                SemanticTerminatorKindV1::Call(call) => call.callee(),
                SemanticTerminatorKindV1::TailCall(call) => call.callee(),
                _ => return self.source.missing("source site is not a defined call"),
            };
            let Some(SemanticCallableDeclV1::Defined { function: target }) =
                semantic.callables().get(callee.index() as usize)
            else {
                return self.source.missing("source call has no defined child");
            };
            let rows = &self.source.root_row(root)?.coordinates.sources.rows;
            budget.charge_work(rows.len())?;
            let mut found = rows.iter().filter(|row| {
                row.incoming
                    .is_some_and(|call| call.caller.index() == caller && call.block == block)
            });
            let child = found
                .next()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "missing original defined call instance",
                ))?;
            if found.next().is_some() || child.function != *target {
                return self
                    .source
                    .missing("substituted or ambiguous defined call instance");
            }
            Ok(child.instance.index())
        })())
    }

    fn instance_descends_from_v18(
        &self,
        root: usize,
        ancestor: usize,
        mut candidate: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<bool> {
        let rows = &self.source.root_row(root)?.coordinates.sources.rows;
        for _ in 0..rows.len() {
            budget.charge_work(1)?;
            let row = rows
                .get(candidate)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "call-instance ancestor",
                ))?;
            if row.instance.index() != candidate {
                return self.source.missing("call-instance ancestor identity");
            }
            if candidate == ancestor {
                return Ok(true);
            }
            let Some(incoming) = row.incoming else {
                return Ok(false);
            };
            candidate = incoming.caller.index();
        }
        self.source
            .missing("cyclic original call-instance ancestry")
    }

    /// Classifies all normalized emitted spans, lifecycle insertions, assertion
    /// failure blocks and nested calls belonging to an actual source instance.
    /// This reuses the existing effect engine and does not prove determinism,
    /// termination, aliasing, initializedness or execution-role safety.
    pub fn instance_effect_decision(
        &self,
        root: usize,
        instance: usize,
        report: &fe2o3_kernel_analysis::CanonicalKirCallEffectsV18<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<fe2o3_kernel_analysis::CanonicalKirCallEffectDecisionV1> {
        use TileAttachmentFamilyV29 as Family;
        use TileAttachmentFieldV29 as Field;
        use fe2o3_kernel_analysis::CanonicalKirCallEffectDecisionV1 as Decision;
        self.retain_query((|| {
            self.query(budget)?;
            self.source.instance(root, instance, budget)?;
            if !report.belongs_to(self.inventory) {
                return self
                    .source
                    .missing("foreign instance call-effect inventory");
            }
            let mut result = Decision::CompleteEmpty;
            let mut checked = 0usize;
            let mut classify = |operation,
                                budget: &mut ArgumentBudgetV1<'_>|
             -> SourceOwnedResultV18<()> {
                let decision = report
                    .operation_decision(operation, budget)
                    .map_err(|error| match error {
                        fe2o3_kernel_analysis::CanonicalKirCallEffectErrorV1::Resource(error) => {
                            ProductionSourceOwnedViewErrorV18::Resource(error)
                        }
                        _ => ProductionSourceOwnedViewErrorV18::Binding(
                            "instance effect occurrence inventory",
                        ),
                    })?;
                result = match (result, decision) {
                    (Decision::Incomplete, _) | (_, Decision::Incomplete) => Decision::Incomplete,
                    (Decision::CompleteNonempty, _) | (_, Decision::CompleteNonempty) => {
                        Decision::CompleteNonempty
                    }
                    _ => Decision::CompleteEmpty,
                };
                Ok(())
            };
            for row in self.attachments {
                budget.charge_work(1)?;
                if row.key.root != root
                    || !matches!(
                        (row.key.family, row.key.field),
                        (Family::InstanceSpans, Field::Span)
                            | (Family::Lifecycle, Field::LifecycleOperation)
                            | (Family::Assertion, Field::AssertFailureBlock)
                            | (
                                Family::TerminalFailure,
                                Field::FailureCleanup | Field::FailureDiagnostic
                            )
                    )
                {
                    continue;
                }
                if !self.instance_descends_from_v18(root, instance, row.key.instance, budget)? {
                    continue;
                }
                checked = checked
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                if matches!(
                    (row.key.family, row.key.field),
                    (Family::Assertion, Field::AssertFailureBlock)
                ) {
                    let TileAttachmentLocationV29::Origin(TileScalarSourceV29::Block {
                        function,
                        block,
                    }) = row.location
                    else {
                        return self
                            .source
                            .missing("instance assertion failure block attachment");
                    };
                    if function != self.source.root_row(root)?.function_ordinal {
                        return self
                            .source
                            .missing("instance failure belongs to another root");
                    }
                    let function = self.inventory.functions().get(function).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding("instance failure function"),
                    )?;
                    let coordinate = fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                        function: function.coordinate,
                        block: u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    };
                    let ordinal = function
                        .blocks
                        .start
                        .checked_add(block)
                        .filter(|ordinal| *ordinal < function.blocks.end)
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "instance failure block",
                        ))?;
                    let failure = self.inventory.blocks().get(ordinal).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "instance failure block inventory",
                        ),
                    )?;
                    if failure.coordinate != coordinate {
                        return self.source.missing("instance failure block coordinate");
                    }
                    for operation in self
                        .inventory
                        .operations()
                        .get(failure.operations.clone())
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "instance failure operations",
                        ))?
                    {
                        budget.charge_work(1)?;
                        classify(operation.coordinate, budget)?;
                    }
                } else if let ProductionSourceOperationV18::Operation(operation) =
                    self.mapped_source_operation(row.location, budget)?
                {
                    classify(operation, budget)?;
                }
            }
            if checked == 0 {
                return self
                    .source
                    .missing("instance has no complete emitted span census");
            }
            Ok(result)
        })())
    }

    /// Returns the original source block's physical entry when it was emitted.
    /// A valid unreachable block has no entry; an invalid source locator refuses.
    /// Call splitting does not replace this entry with a continuation block.
    pub fn source_block_entry(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1>> {
        self.retain_query((|| {
            self.query(budget)?;
            let function = self.source.instance(root, instance, budget)?.0;
            let source = self.source.owner.inner.source.owner.source_semantic();
            if source
                .functions()
                .get(function.index() as usize)
                .and_then(|row| row.blocks().get(block.index() as usize))
                .is_none()
            {
                return self.source.missing("source block locator");
            }
            let Some(sidecar) = self.source.optional_sidecar(root, instance, budget)? else {
                return Ok(None);
            };
            let rows = &sidecar.blocks;
            budget.charge_work(rows.len())?;
            let mut found = rows
                .iter()
                .filter(|row| row.semantic_function == function && row.semantic_block == block);
            let Some(row) = found.next() else {
                return Ok(None);
            };
            if found.next().is_some()
                || row.correspondence_owner != self.source.root_row(root)?.coordinates.root
            {
                return self.source.missing("source block instance binding");
            }
            let function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                u32::try_from(self.source.root_row(root)?.function_ordinal)
                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            let row = self
                .inventory
                .block_for_id(function, row.kernel_ir_block, budget)
                .map_err(|error| match error {
                    fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(error) => {
                        ProductionSourceOwnedViewErrorV18::Resource(error)
                    }
                    _ => ProductionSourceOwnedViewErrorV18::Binding("source block inventory"),
                })?;
            match row {
                Some(row) => Ok(Some(row.coordinate)),
                None => self
                    .source
                    .missing("missing source block in canonical inventory"),
            }
        })())
    }

    /// Returns one original Assert's actual occurrence after instance relocation.
    /// The source polarity, source success edge and physical root are rejoined;
    /// this binding does not itself prove that the condition succeeds.
    pub fn assertion(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<SemanticKirAssertConditionBindingV1> {
        self.retain_query((|| {
            self.query(budget)?;
            let root_row = self.source.root_row(root)?;
            let function = self.source.instance(root, instance, budget)?.0;
            let source = self.source.owner.inner.source.owner.source_semantic();
            let Some(source_block) = source
                .functions()
                .get(function.index() as usize)
                .and_then(|row| row.blocks().get(block.index() as usize))
            else {
                return self.source.missing("source assertion block");
            };
            let SemanticTerminatorKindV1::Assert {
                expected, target, ..
            } = source_block.terminator().kind()
            else {
                return self.source.missing("source site is not an assertion");
            };
            let site = SemanticKirAssertSiteV1::new(root_row.coordinates.root, function, block);
            budget.charge_work(self.source.owner.inner.assertions.len())?;
            let mut matches = self.source.owner.inner.assertions.iter().filter(|row| {
                row.instance.index() == instance
                    && row.site == site
                    && row.binding.block().function.0 as usize == root_row.function_ordinal
            });
            let Some(row) = matches.next() else {
                return self.source.missing("missing instance assertion attachment");
            };
            if matches.next().is_some()
                || row.binding.expected() != *expected
                || row.binding.semantic_success() != target.target()
            {
                return self
                    .source
                    .missing("source assertion polarity or success edge");
            }
            Ok(row.binding)
        })())
    }

    fn source_operation_rows(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&[SourceAttachmentV18]> {
        self.retain_query((|| {
            self.query(budget)?;
            let source = self.source.instance(root, instance, budget)?.0;
            let root_row = self.source.root_row(root)?;
            budget.charge_work(root_row.coordinates.spans.rows.len())?;
            let mut matching =
                root_row
                    .coordinates
                    .spans
                    .rows
                    .iter()
                    .enumerate()
                    .filter(|(_, row)| {
                        row.instance.index() == instance
                            && match row.source {
                                InstanceSpanSourceV1::Statement(site) => {
                                    site.semantic_function == source
                                        && site.semantic_block == block
                                        && Some(site.statement_ordinal) == statement
                                }
                                InstanceSpanSourceV1::Terminator(site) => {
                                    site.semantic_function == source
                                        && site.semantic_block == block
                                        && statement.is_none()
                                }
                                _ => false,
                            }
                    });
            let Some((row, _)) = matching.next() else {
                return self.source.missing("source site span");
            };
            if matching.next().is_some() {
                return self.source.missing("ambiguous source site span");
            }
            self.attachment_range(
                TileAttachmentKeyV29 {
                    root,
                    family: TileAttachmentFamilyV29::InstanceSpans,
                    instance,
                    row,
                    field: TileAttachmentFieldV29::Span,
                    component: 0,
                    part: 0,
                },
                budget,
            )
        })())
    }

    fn mapped_source_operation(
        &self,
        location: TileAttachmentLocationV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionSourceOperationV18> {
        self.retain_query((|| {
            self.query(budget)?;
            budget.charge_work(1)?;
            Ok(match location {
                TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(point)) => {
                    ProductionSourceOperationV18::Operation(
                        fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                            block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                                function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                                    u32::try_from(point.function)
                                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                                ),
                                block: u32::try_from(point.block)
                                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                            },
                            operation: u32::try_from(point.operation)
                                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        },
                    )
                }
                TileAttachmentLocationV29::Gap(point) => ProductionSourceOperationV18::Gap {
                    block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                            u32::try_from(point.function)
                                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        ),
                        block: u32::try_from(point.block)
                            .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    },
                    operation: u32::try_from(point.operation)
                        .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                },
                TileAttachmentLocationV29::Tombstone => ProductionSourceOperationV18::RemovedCall,
                TileAttachmentLocationV29::NoOutput => ProductionSourceOperationV18::NoOperations,
                _ => {
                    return self
                        .source
                        .missing("source span maps to a non-operation attachment");
                }
            })
        })())
    }

    /// Visits the complete mapped output of one original source site in order.
    /// `statement == None` selects its terminator, not a synthetic entry block.
    /// Repeated helper sites must always retain both root and instance ordinals.
    pub fn visit_source_operations(
        &self,
        root: usize,
        instance: usize,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        budget: &mut ArgumentBudgetV1<'_>,
        mut visit: impl FnMut(
            ProductionSourceOperationV18,
            &mut ArgumentBudgetV1<'_>,
        ) -> SourceOwnedResultV18<()>,
    ) -> SourceOwnedResultV18<()> {
        let rows = self.source_operation_rows(root, instance, block, statement, budget)?;
        for row in rows {
            let operation = self.mapped_source_operation(row.location, budget)?;
            visit(operation, budget)?;
        }
        Ok(())
    }
}
