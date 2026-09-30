// Sparse original allocation/leaf bindings for every actual Policy12 endpoint.
// Graphs and checked transition witnesses remain in the single retained chain.
type AggregateMemoryLeafV31 = fe2o3_kernel_analysis::CanonicalKirAggregateSsaLeafTypeV18;
type AggregateMemoryCensusV31<'i, 'g> =
    fe2o3_kernel_analysis::CanonicalKirAggregateMemoryInventoryV31<'i, 'g>;

/// Exact original private allocation and typed leaf, independent of stage-local
/// census/SSA indices. This inert key does not establish initialized storage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionAggregateMemoryKeyV31 {
    allocation: AggregateOperationV30,
    layout: fe2o3_kernel_ir::StorageLayoutIdV1,
    offset: u64,
    ty: AggregateMemoryLeafV31,
}
impl ProductionAggregateMemoryKeyV31 {
    /// Exact original canonical Alloca occurrence.
    pub const fn allocation(self) -> AggregateOperationV30 {
        self.allocation
    }
    /// Exact original typed storage leaf layout.
    pub const fn layout(self) -> fe2o3_kernel_ir::StorageLayoutIdV1 {
        self.layout
    }
    /// Checked byte offset within that original allocation.
    pub const fn offset(self) -> u64 {
        self.offset
    }
    /// Actual scalar or vector leaf payload, never inferred from byte width.
    pub const fn ty(self) -> AggregateMemoryLeafV31 {
        self.ty
    }
    fn order(self) -> [usize; 6] {
        // Preserve the complete byte-offset order on 32-bit hosts too.
        [
            self.allocation.block.function.0 as usize,
            self.allocation.block.block as usize,
            self.allocation.operation as usize,
            self.layout.0 as usize,
            (self.offset >> 32) as usize,
            (self.offset & u64::from(u32::MAX)) as usize,
        ]
    }
}

#[derive(Clone, Copy)]
struct AggregateMemoryAllocationBindingV31 {
    actual: AggregateOperationV30,
    original: AggregateOperationV30,
    closed_uses: Option<bool>,
}
#[derive(Clone, Copy)]
struct AggregateMemoryEventBindingV31 {
    actual: AggregateOperationV30,
    event: fe2o3_kernel_analysis::CanonicalKirAggregateMemoryEventV31,
    allocation: Option<AggregateOperationV30>,
}

/// Exact endpoint event expressed in the shared original allocation/leaf space.
/// These rows preserve obligations; they do not establish source semantics or
/// that a read is initialized on every actual path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionAggregateMemoryEventV31 {
    /// The actual operation still requires an independent concrete model.
    Unmodeled,
    /// Begin a new dynamic lifetime and reset the original allocation's state.
    Allocate {
        /// Exact original Alloca occurrence whose dynamic lifetime begins.
        original: AggregateOperationV30,
    },
    /// A typed projection of the exact original allocation.
    Project {
        /// Exact original allocation selected by the checked projection.
        original: AggregateOperationV30,
    },
    /// Read the shared typed leaf into the exact endpoint-local result.
    Read {
        /// Shared original typed leaf index in this retained chain.
        leaf: usize,
        /// Exact endpoint-local result of the actual read.
        output: ValueId,
    },
    /// Write the exact endpoint-local value to the shared typed leaf.
    Write {
        /// Shared original typed leaf index in this retained chain.
        leaf: usize,
        /// Exact endpoint-local operand of the actual write.
        value: ValueId,
    },
}
#[derive(Clone, Copy)]
struct AggregateMemoryLeafBindingV31 {
    original: ProductionAggregateMemoryKeyV31,
    global: usize,
    alignment: u32,
}
#[derive(Clone, Copy)]
struct AggregateMemoryPromotionV31 {
    stage: usize,
    witness_slot: usize,
    original: ProductionAggregateMemoryKeyV31,
    global: usize,
}
struct AggregateMemoryEndpointV31 {
    owner: usize,
    allocations: std::ops::Range<usize>,
    leaves: std::ops::Range<usize>,
    events: std::ops::Range<usize>,
    unmodeled_operations: usize,
    nonclosed_allocations: usize,
}

/// Source-owned bindings through the complete checked scalar/aggregate chain.
/// Unknown operations and nonclosed allocations remain explicit; this owner is
/// not a generated or executed concrete-memory refinement proof.
pub struct ProductionAggregateMemoryChainV31 {
    initial_owner: usize,
    final_owner: usize,
    next_stage: usize,
    endpoints: Vec<AggregateMemoryEndpointV31>,
    allocations: Vec<AggregateMemoryAllocationBindingV31>,
    leaves: Vec<AggregateMemoryLeafBindingV31>,
    events: Vec<AggregateMemoryEventBindingV31>,
    promotions: Vec<AggregateMemoryPromotionV31>,
    keys: Vec<ProductionAggregateMemoryKeyV31>,
    current: Vec<Option<AggregateOperationV30>>,
    finalized: bool,
}

fn aggregate_memory_headers_v31() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a, 'w> = (
        ProductionAggregateMemoryChainV31,
        Result<ProductionAggregateMemoryChainV31, ProductionAggregateSourceErrorV30>,
        AggregateMemoryCensusV31<'a, 'a>,
        fe2o3_kernel_analysis::CanonicalKirAggregateMemoryStorageV31,
        Result<
            (
                AggregateMemoryCensusV31<'a, 'a>,
                fe2o3_kernel_analysis::CanonicalKirAggregateMemoryStorageV31,
            ),
            fe2o3_kernel_analysis::CanonicalKirAggregateSsaErrorV18,
        >,
        &'a mut ProductionAggregateMemoryChainV31,
        &'a AggregateSourceStageV30<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a AggregateMemoryCensusV31<'a, 'a>,
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'w>,
        Vec<Option<AggregateOperationV30>>,
        Vec<bool>,
        [Result<(), ProductionAggregateSourceErrorV30>; 3],
        Result<(), ArgumentResourceV1>,
        [usize; 16],
        [ProductionAggregateMemoryKeyV31; 3],
        AggregateMemoryEndpointV31,
        AggregateMemoryAllocationBindingV31,
        AggregateMemoryEventBindingV31,
        Option<(AggregateOperationV30, ProductionAggregateMemoryEventV31)>,
        AggregateMemoryLeafBindingV31,
        AggregateMemoryPromotionV31,
        Option<fe2o3_kernel_analysis::CanonicalKirAggregateMemoryAllocationV31>,
        Option<fe2o3_kernel_analysis::CanonicalKirAggregateSsaMemorySlotV18>,
        Option<fe2o3_kernel_analysis::CanonicalKirAggregateMemoryEventV31>,
        Result<
            Option<fe2o3_kernel_analysis::CanonicalKirAggregateOperationTransportV30>,
            fe2o3_kernel_analysis::CanonicalKirAggregateSsaErrorV18,
        >,
        SourceOwnedResultV18<ProductionAggregateMemoryKeyV31>,
        SourceOwnedResultV18<&'a ProductionAggregateMemoryChainV31>,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_, '_>>(),
        std::mem::align_of::<Frame<'_, '_>>(),
    ])
}

impl AggregateStageStateV30 for ProductionAggregateMemoryChainV31 {
    fn retained_storage(&self) -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            aggregate_vector_credit_v30(&self.endpoints)?,
            aggregate_vector_credit_v30(&self.allocations)?,
            aggregate_vector_credit_v30(&self.leaves)?,
            aggregate_vector_credit_v30(&self.events)?,
            aggregate_vector_credit_v30(&self.promotions)?,
            aggregate_vector_credit_v30(&self.keys)?,
            aggregate_vector_credit_v30(&self.current)?,
        ])
    }
}

fn aggregate_memory_check_error_v31(
    error: fe2o3_kernel_analysis::CanonicalKirAggregateSsaErrorV18,
) -> ProductionAggregateSourceErrorV30 {
    ProductionAggregateSourceErrorV30::Optimization(
        fe2o3_kernel_opt::OwnedAggregateFixedpointErrorV18::Aggregate(
            fe2o3_kernel_opt::OwnedAggregateSsaErrorV18::Check(error),
        ),
    )
}

fn aggregate_memory_operations_v31(
    owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<usize, ProductionAggregateSourceErrorV30> {
    let mut count = 0;
    for function in &owner.module().functions {
        budget.charge_work(1)?;
        if let Some(body) = &function.body {
            for block in &body.blocks {
                budget.charge_work(1)?;
                count = argument_sum_v1(&[count, block.operations.len()])?;
            }
        }
    }
    Ok(count)
}

impl ProductionAggregateMemoryChainV31 {
    fn seed(
        stage: &AggregateSourceStageV30<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionAggregateSourceErrorV30> {
        stage.check(budget)?;
        if stage.ordinal != 0 {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "aggregate memory seed stage order",
            )
            .into());
        }
        let mut capacity = stage.input.operations().len();
        for round in stage.chain.rounds() {
            capacity = argument_sum_v1(&[
                capacity,
                aggregate_memory_operations_v31(round.scalar().owner(), budget)?,
                aggregate_memory_operations_v31(round.aggregate().output(), budget)?,
            ])?;
        }
        let stages = argument_sum_v1(&[argument_product_v1(stage.chain.rounds().len(), 2)?, 1])?;
        let mut current = source_reference_emission_vec_v29(stage.input.operations().len(), budget)
            .map_err(source_argument_error_v18)?;
        for row in stage.input.operations() {
            budget.charge_work(1)?;
            current.push(
                matches!(row.operation.kind, OperationKind::Alloca { .. })
                    .then_some(row.coordinate),
            );
        }
        let mut value = Self {
            initial_owner: std::ptr::from_ref(stage.input.owner()) as usize,
            final_owner: std::ptr::from_ref(stage.input.owner()) as usize,
            next_stage: 0,
            endpoints: source_reference_emission_vec_v29(stages, budget)
                .map_err(source_argument_error_v18)?,
            allocations: source_reference_emission_vec_v29(capacity, budget)
                .map_err(source_argument_error_v18)?,
            leaves: source_reference_emission_vec_v29(capacity, budget)
                .map_err(source_argument_error_v18)?,
            events: source_reference_emission_vec_v29(capacity, budget)
                .map_err(source_argument_error_v18)?,
            promotions: source_reference_emission_vec_v29(capacity, budget)
                .map_err(source_argument_error_v18)?,
            keys: source_reference_emission_vec_v29(argument_product_v1(capacity, 2)?, budget)
                .map_err(source_argument_error_v18)?,
            current,
            finalized: false,
        };
        value.record(stage.input, stage.source, budget)?;
        Ok(value)
    }

    fn key(
        &self,
        slot: fe2o3_kernel_analysis::CanonicalKirAggregateSsaMemorySlotV18,
    ) -> SourceOwnedResultV18<ProductionAggregateMemoryKeyV31> {
        Ok(ProductionAggregateMemoryKeyV31 {
            allocation: self.current.get(slot.allocation).copied().flatten().ok_or(
                ProductionSourceOwnedViewErrorV18::Binding(
                    "aggregate memory leaf lacks exact original allocation",
                ),
            )?,
            layout: slot.layout,
            offset: slot.offset,
            ty: slot.ty,
        })
    }

    fn record(
        &mut self,
        inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
        source: &ProductionSourceOwnedViewV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionAggregateSourceErrorV30> {
        if self.current.len() != inventory.operations().len()
            || self.final_owner != std::ptr::from_ref(inventory.owner()) as usize
            || self.endpoints.len() == self.endpoints.capacity()
        {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "aggregate memory endpoint owner or census",
            )
            .into());
        }
        let (memory, receipt) = AggregateMemoryCensusV31::derive(inventory, budget)
            .map_err(aggregate_memory_check_error_v31)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let result =
            source.retain_aggregate_result_v30(self.record_census(inventory, &memory, budget));
        drop(memory);
        let settled = if source.cleanup.is_denied() {
            Ok(())
        } else {
            budget.release_storage(receipt.retained_storage())
        };
        result?;
        settled?;
        Ok(())
    }

    fn record_census(
        &mut self,
        inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
        memory: &AggregateMemoryCensusV31<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionAggregateSourceErrorV30> {
        let allocations = self.allocations.len();
        let leaves = self.leaves.len();
        let events = self.events.len();
        let mut supported_allocation = 0;
        for (at, (row, original)) in inventory.operations().iter().zip(&self.current).enumerate() {
            budget.charge_work(2)?;
            if matches!(row.operation.kind, OperationKind::Alloca { .. }) != original.is_some() {
                return Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "aggregate memory allocation occurrence changed",
                )
                .into());
            }
            if let Some(original) = original {
                let closed_uses = match memory.allocation(supported_allocation) {
                    Some(allocation) if allocation.operation() == at => {
                        supported_allocation += 1;
                        Some(allocation.has_closed_supported_uses())
                    }
                    Some(allocation) if allocation.operation() < at => {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    _ => None,
                };
                if self.allocations.len() == self.allocations.capacity() {
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                self.allocations.push(AggregateMemoryAllocationBindingV31 {
                    actual: row.coordinate,
                    original: *original,
                    closed_uses,
                });
            }
        }
        if supported_allocation != memory.allocation_count()
            || memory.event_count() != inventory.operations().len()
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        for leaf in 0..memory.leaf_count() {
            budget.charge_work(3)?;
            let original = self.key(memory.leaf(leaf).ok_or(ArgumentResourceV1::Accounting)?)?;
            if self.leaves.len() == self.leaves.capacity() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            self.leaves.push(AggregateMemoryLeafBindingV31 {
                original,
                global: usize::MAX,
                alignment: memory
                    .leaf_alignment(leaf)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            });
        }
        let mut unmodeled_operations = 0;
        for op in 0..memory.event_count() {
            use fe2o3_kernel_analysis::CanonicalKirAggregateMemoryEventV31 as Event;
            budget.charge_work(3)?;
            let event = memory.event(op).ok_or(ArgumentResourceV1::Accounting)?;
            let allocation = match event {
                Event::None => {
                    unmodeled_operations = argument_sum_v1(&[unmodeled_operations, 1])?;
                    None
                }
                Event::Allocate { allocation } | Event::Project { allocation } => {
                    Some(self.current.get(allocation).copied().flatten().ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "aggregate memory event allocation differs",
                        ),
                    )?)
                }
                Event::Read { leaf, .. } | Event::Write { leaf, .. } => {
                    if leaf >= memory.leaf_count() {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    None
                }
            };
            if self.events.len() == self.events.capacity() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            self.events.push(AggregateMemoryEventBindingV31 {
                actual: inventory.operations()[op].coordinate,
                event,
                allocation,
            });
        }
        let mut nonclosed_allocations = 0;
        for at in 0..memory.allocation_count() {
            budget.charge_work(1)?;
            if !memory
                .allocation(at)
                .ok_or(ArgumentResourceV1::Accounting)?
                .has_closed_supported_uses()
            {
                nonclosed_allocations = argument_sum_v1(&[nonclosed_allocations, 1])?;
            }
        }
        self.endpoints.push(AggregateMemoryEndpointV31 {
            owner: self.final_owner,
            allocations: allocations..self.allocations.len(),
            leaves: leaves..self.leaves.len(),
            events: events..self.events.len(),
            unmodeled_operations,
            nonclosed_allocations,
        });
        Ok(())
    }

    fn advance(
        mut self,
        stage: &AggregateSourceStageV30<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionAggregateSourceErrorV30> {
        stage.check(budget)?;
        if self.finalized
            || self.next_stage != stage.ordinal
            || self.final_owner != std::ptr::from_ref(stage.input.owner()) as usize
            || self.current.len() != stage.input.operations().len()
            || self.endpoints.len() != stage.ordinal + 1
        {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "aggregate memory stage owner or order",
            )
            .into());
        }
        let previous = self
            .endpoints
            .last()
            .ok_or(ArgumentResourceV1::Accounting)?;
        for binding in &self.allocations[previous.allocations.clone()] {
            let at = aggregate_operation_index_v30(stage.input, binding.actual, budget)?;
            budget.charge_work(1)?;
            if self.current[at] != Some(binding.original) {
                return Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "aggregate memory current allocation binding differs",
                )
                .into());
            }
        }
        let mut next = source_reference_emission_vec_v29(stage.output.operations().len(), budget)
            .map_err(source_argument_error_v18)?;
        budget.charge_work(stage.output.operations().len())?;
        next.resize(stage.output.operations().len(), None);
        match stage.relation {
            AggregateSourceStageRelationV30::Scalar(pair) => {
                let mut seen = source_reference_emission_vec_v29(self.current.len(), budget)
                    .map_err(source_argument_error_v18)?;
                budget.charge_work(self.current.len())?;
                seen.resize(self.current.len(), false);
                for row in pair.rows().operations {
                    budget.charge_work(3)?;
                    let fe2o3_kernel_ir::CanonicalKirOperationOriginV1::Retained(input) =
                        row.origin
                    else {
                        continue;
                    };
                    let input = aggregate_operation_index_v30(stage.input, input, budget)?;
                    let Some(original) = self.current[input] else {
                        continue;
                    };
                    let output = aggregate_operation_index_v30(stage.output, row.output, budget)?;
                    if seen[input] || next[output].replace(original).is_some() {
                        return Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "aggregate memory scalar allocation duplicated",
                        )
                        .into());
                    }
                    seen[input] = true;
                }
                let credit = aggregate_vector_credit_v30(&seen)?;
                drop(seen);
                budget.release_storage(credit)?;
            }
            AggregateSourceStageRelationV30::Aggregate(pair) => {
                let index =
                    stage
                        .aggregate_index
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "aggregate memory lacks checked occurrence index",
                        ))?;
                for (slot, memory) in pair.witness().memory_slots().iter().enumerate() {
                    budget.charge_work(3)?;
                    let Some(memory) = memory else {
                        continue;
                    };
                    if self.promotions.len() == self.promotions.capacity() {
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    let original = self.key(*memory)?;
                    self.promotions.push(AggregateMemoryPromotionV31 {
                        stage: stage.ordinal,
                        witness_slot: slot,
                        original,
                        global: usize::MAX,
                    });
                }
                for (input, original) in self.current.iter().enumerate() {
                    budget.charge_work(2)?;
                    let Some(original) = original else {
                        continue;
                    };
                    match index
                        .operation(input, budget)
                        .map_err(aggregate_memory_check_error_v31)?
                    {
                        Some(occurrence) if occurrence.is_retained() => {
                            let output = aggregate_operation_index_v30(
                                stage.output,
                                occurrence.output(),
                                budget,
                            )?;
                            if next[output].replace(*original).is_some() {
                                return Err(ProductionSourceOwnedViewErrorV18::Binding(
                                    "aggregate memory allocation merged",
                                )
                                .into());
                            }
                        }
                        None if matches!(pair.witness().memory_events().get(input),
                            Some(fe2o3_kernel_analysis::CanonicalKirAggregateSsaMemoryEventV18::Allocate { allocation })
                            if *allocation == input) =>
                        {
                            ()
                        }
                        _ => {
                            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                                "aggregate memory allocation has no checked retirement",
                            )
                            .into());
                        }
                    }
                }
            }
        }
        let old = std::mem::replace(&mut self.current, next);
        let credit = aggregate_vector_credit_v30(&old)?;
        drop(old);
        budget.release_storage(credit)?;
        self.final_owner = std::ptr::from_ref(stage.output.owner()) as usize;
        self.next_stage = self
            .next_stage
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        self.record(stage.output, stage.source, budget)?;
        Ok(self)
    }

    fn finish(
        &mut self,
        source: &ProductionSourceOwnedViewV18<'_>,
        chain: &fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionAggregateSourceErrorV30> {
        source.check_query_v18(budget)?;
        budget.charge_work(6)?;
        if self.finalized
            || self.initial_owner != std::ptr::from_ref(source.canonical(budget)?) as usize
            || self.final_owner != std::ptr::from_ref(chain.owner()) as usize
            || self.next_stage != argument_product_v1(chain.rounds().len(), 2)?
            || self.endpoints.len() != self.next_stage + 1
            || !self.keys.is_empty()
        {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "aggregate memory final chain identity",
            )
            .into());
        }
        if self.endpoints[0].owner != self.initial_owner {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "aggregate memory final endpoint order",
            )
            .into());
        }
        for (round, row) in chain.rounds().iter().enumerate() {
            budget.charge_work(2)?;
            if self.endpoints[round * 2 + 1].owner
                != std::ptr::from_ref(row.scalar().owner()) as usize
                || self.endpoints[round * 2 + 2].owner
                    != std::ptr::from_ref(row.aggregate().output()) as usize
            {
                return Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "aggregate memory final endpoint order",
                )
                .into());
            }
        }
        for key in self
            .leaves
            .iter()
            .map(|row| row.original)
            .chain(self.promotions.iter().map(|row| row.original))
        {
            budget.charge_work(1)?;
            if self.keys.len() == self.keys.capacity() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            self.keys.push(key);
        }
        private_array_heapsort_v1(
            &mut self.keys,
            |row| row.order(),
            &mut SourceCorrespondenceWorkV18(budget),
            || ArgumentResourceV1::Arithmetic.into(),
        )?;
        let mut retained = 0;
        for at in 0..self.keys.len() {
            budget.charge_work(3)?;
            let key = self.keys[at];
            if retained != 0 && self.keys[retained - 1].order() == key.order() {
                if self.keys[retained - 1] != key {
                    return Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "aggregate memory original leaf type changed",
                    )
                    .into());
                }
            } else {
                self.keys[retained] = key;
                retained += 1;
            }
        }
        self.keys.truncate(retained);
        let lookup_work = self.keys.len().checked_ilog2().unwrap_or(0) as usize + 3;
        for row in &mut self.leaves {
            budget.charge_work(lookup_work)?;
            row.global = self
                .keys
                .binary_search_by_key(&row.original.order(), |key| key.order())
                .map_err(|_| {
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "aggregate memory global leaf is absent",
                    )
                })?;
        }
        for row in &mut self.promotions {
            budget.charge_work(lookup_work)?;
            row.global = self
                .keys
                .binary_search_by_key(&row.original.order(), |key| key.order())
                .map_err(|_| {
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "aggregate memory promoted leaf is absent",
                    )
                })?;
        }
        self.finalized = true;
        Ok(())
    }

    /// Number of exact canonical endpoints, including original and final.
    pub fn endpoint_count(&self) -> usize {
        self.endpoints.len()
    }
    /// Number of stable original typed leaves used anywhere in this chain.
    pub fn key_count(&self) -> usize {
        self.keys.len()
    }
    /// Returns one exact original allocation/leaf key from the shared namespace.
    pub fn key_at(&self, index: usize) -> Option<ProductionAggregateMemoryKeyV31> {
        self.keys.get(index).copied()
    }
    /// Checks one actual boundary owner by identity, never by equal graph bytes.
    pub fn endpoint_belongs_to(
        &self,
        endpoint: usize,
        owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
    ) -> bool {
        self.finalized
            && self
                .endpoints
                .get(endpoint)
                .is_some_and(|row| row.owner == std::ptr::from_ref(owner) as usize)
    }
    /// Number of census leaves at the selected exact endpoint.
    pub fn endpoint_leaf_count(&self, endpoint: usize) -> Option<usize> {
        self.endpoints.get(endpoint).map(|row| row.leaves.len())
    }
    /// Number of exact original allocation bindings at one endpoint.
    pub fn endpoint_allocation_count(&self, endpoint: usize) -> Option<usize> {
        self.endpoints
            .get(endpoint)
            .map(|row| row.allocations.len())
    }
    /// Actual endpoint occurrence and the exact original Alloca it descends from.
    pub fn endpoint_allocation(
        &self,
        endpoint: usize,
        allocation: usize,
    ) -> Option<(AggregateOperationV30, AggregateOperationV30)> {
        let range = &self.endpoints.get(endpoint)?.allocations;
        if !self.finalized || allocation >= range.len() {
            return None;
        }
        let row = self.allocations.get(range.start.checked_add(allocation)?)?;
        Some((row.actual, row.original))
    }
    /// Complete-use status for an allocation: inner `None` is outside the
    /// closed census grammar, and `Some(true)` still does not prove initialization.
    pub fn endpoint_allocation_closed_uses(
        &self,
        endpoint: usize,
        allocation: usize,
    ) -> Option<Option<bool>> {
        let range = &self.endpoints.get(endpoint)?.allocations;
        if !self.finalized || allocation >= range.len() {
            return None;
        }
        Some(
            self.allocations
                .get(range.start.checked_add(allocation)?)?
                .closed_uses,
        )
    }
    /// Complete operation roster at this endpoint, including unmodeled rows.
    pub fn endpoint_event_count(&self, endpoint: usize) -> Option<usize> {
        self.endpoints.get(endpoint).map(|row| row.events.len())
    }
    /// Exact actual operation and event in the shared original memory namespace.
    /// Result/value identities remain scoped to the selected exact endpoint.
    pub fn endpoint_event(
        &self,
        endpoint: usize,
        operation: usize,
    ) -> Option<(AggregateOperationV30, ProductionAggregateMemoryEventV31)> {
        use fe2o3_kernel_analysis::CanonicalKirAggregateMemoryEventV31 as Event;
        let range = &self.endpoints.get(endpoint)?.events;
        if !self.finalized || operation >= range.len() {
            return None;
        }
        let row = self.events.get(range.start.checked_add(operation)?)?;
        let event = match row.event {
            Event::None => ProductionAggregateMemoryEventV31::Unmodeled,
            Event::Allocate { .. } => ProductionAggregateMemoryEventV31::Allocate {
                original: row.allocation?,
            },
            Event::Project { .. } => ProductionAggregateMemoryEventV31::Project {
                original: row.allocation?,
            },
            Event::Read { leaf, output } => ProductionAggregateMemoryEventV31::Read {
                leaf: self.endpoint_leaf(endpoint, leaf)?.0,
                output,
            },
            Event::Write { leaf, value } => ProductionAggregateMemoryEventV31::Write {
                leaf: self.endpoint_leaf(endpoint, leaf)?.0,
                value,
            },
        };
        Some((row.actual, event))
    }
    /// Number of selected witness slots bound to the shared original namespace.
    pub fn promotion_count(&self) -> usize {
        self.promotions.len()
    }
    /// Exact stage ordinal, witness-local slot and shared original leaf slot.
    pub fn promotion(&self, index: usize) -> Option<(usize, usize, usize)> {
        if !self.finalized {
            return None;
        }
        self.promotions
            .get(index)
            .map(|row| (row.stage, row.witness_slot, row.global))
    }
    /// Binds one endpoint-local census leaf to the shared original slot.
    pub fn endpoint_leaf(&self, endpoint: usize, leaf: usize) -> Option<(usize, u32)> {
        let range = &self.endpoints.get(endpoint)?.leaves;
        if !self.finalized || leaf >= range.len() {
            return None;
        }
        let row = self.leaves.get(range.start.checked_add(leaf)?)?;
        Some((row.global, row.alignment))
    }
    /// Explicit counts of unmodeled operations and nonclosed allocations.
    /// Zero unmodeled memory events does not assert scalar or call semantics.
    pub fn endpoint_open_census(&self, endpoint: usize) -> Option<(usize, usize)> {
        self.endpoints
            .get(endpoint)
            .map(|row| (row.unmodeled_operations, row.nonclosed_allocations))
    }
    /// Always false: exact memory identity transport is not executed refinement.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
