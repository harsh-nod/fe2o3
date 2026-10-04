// Exact source/output/native correspondence, deliberately not memory completion.
#[derive(Debug)]
enum PendingGlobalNativeErrorV18 {
    Source(ProductionSourceOwnedViewErrorV18),
    Native(fe2o3_pliron::CanonicalRankedPolicyFailureV1),
}
impl From<ProductionSourceOwnedViewErrorV18> for PendingGlobalNativeErrorV18 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}
impl From<ArgumentResourceV1> for PendingGlobalNativeErrorV18 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Source(error.into())
    }
}

struct PendingGlobalSourceNativeAccessV18<'s, 'g> {
    pair: &'s GlobalSourceAccessPairV18,
    native: &'s fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'s, 'g>,
}
impl PendingGlobalSourceNativeAccessV18<'_, '_> {
    const fn grants_memory_or_launch_authority(&self) -> bool {
        false
    }
}

fn global_native_pair_headers_v18() -> Result<usize, ArgumentResourceV1> {
    use fe2o3_kernel_analysis::{
        CanonicalKirBlockRefV1, CanonicalKirInventoryV18 as Inventory, CanonicalKirOperationRefV1,
    };
    // The four carrier borrows coexist. Query return slots, edge zip and the
    // value-lookup closure have their own fixed slots, not surplus Result bytes.
    type PairFrame<'a> = (
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'a, 'a>,
        &'a GlobalSourceAccessPairV18,
        &'a mut ArgumentBudgetV1<'a>,
        &'a Inventory<'a>,
        &'a GlobalSourceAccessEndpointV18,
        std::array::IntoIter<SliceOperation, 4>,
        Option<SliceOperation>,
        SliceOperation,
        Option<&'a fe2o3_kernel_ir::Operation>,
        &'a fe2o3_kernel_ir::Operation,
        &'a CanonicalKirOperationRefV1<'a>,
        Result<&'a CanonicalKirOperationRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        Option<&'a Terminator>,
        Option<(&'a Terminator, &'a Terminator)>,
        &'a CanonicalKirBlockRefV1<'a>,
        Result<&'a CanonicalKirBlockRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        (&'a Inventory<'a>,),
        &'a CanonicalKirDefinitionRefV1<'a>,
        Result<&'a CanonicalKirDefinitionRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        Result<ValueId, ProductionSourceOwnedViewErrorV18>,
        Result<&'a Inventory<'a>, ProductionSourceOwnedViewErrorV18>,
        (
            fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
            &'a mut ArgumentBudgetV1<'a>,
        ),
        Option<ValueId>,
        [ValueId; 4],
        [&'a fe2o3_kernel_ir::Operation; 4],
        [&'a fe2o3_kernel_ir::OperationKind; 2],
        [&'a ValueId; 2],
        &'a fe2o3_kernel_ir::MemoryAccess,
        [bool; 2],
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        [usize; 3],
        Option<&'a GlobalSourceAccessPairV18>,
        Option<&'a GlobalSourceAccessPairV18>,
        &'a GlobalSourceAccessPairV18,
        SourceOwnedResultV18<Option<&'a GlobalSourceAccessPairV18>>,
        (
            &'a Option<&'a GlobalSourceAccessPairV18>,
            &'a Option<&'a GlobalSourceAccessPairV18>,
        ),
        (&'a GlobalSourceAccessPairV18, &'a GlobalSourceAccessPairV18),
        Result<(), ArgumentResourceV1>,
        SourceOwnedResultV18<()>,
        bool,
    );
    argument_sum_v1(&[
        size_of::<PairFrame<'_>>(),
        argument_product_v1(
            2,
            size_of::<Result<PairFrame<'_>, PendingGlobalNativeErrorV18>>(),
        )?,
    ])
}

fn global_native_headers_v18(capture: usize) -> Result<usize, ArgumentResourceV1> {
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<Result<T, PendingGlobalNativeErrorV18>>())?,
        ])
    }
    argument_sum_v1(&[
        capture,
        global_native_pair_headers_v18()?,
        h::<PendingGlobalSourceNativeAccessV18<'_, '_>>()?,
        h::<&GlobalSourceAccessPairV18>()?,
        h::<Option<&GlobalSourceAccessPairV18>>()?,
        h::<Option<&PendingGlobalSourceNativeAccessV18<'_, '_>>>()?,
        h::<&fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>>()?,
        h::<&fe2o3_kernel_ir::Operation>()?,
        h::<Option<&fe2o3_kernel_ir::Operation>>()?,
        h::<&Terminator>()?,
        h::<Option<&Terminator>>()?,
        h::<&CanonicalKirDefinitionRefV1<'_>>()?,
        h::<[SliceOperation; 4]>()?,
        h::<[ValueId; 3]>()?,
        h::<[usize; 4]>()?,
        h::<Result<(), PendingGlobalNativeErrorV18>>()?,
        h::<std::thread::Result<Result<(), PendingGlobalNativeErrorV18>>>()?,
        h::<(
            &PendingGlobalSourceAccessesV18<'_>,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<(
            &PendingGlobalSourceNativeAccessV18<'_, '_>,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
    ])
}

impl PendingGlobalSourceAccessesV18<'_> {
    fn with_native_access_v18<'work>(
        &self,
        native: &fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>,
        operation: SliceOperation,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'s, 'g> FnOnce(
            Option<&PendingGlobalSourceNativeAccessV18<'s, 'g>>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(), PendingGlobalNativeErrorV18>,
    ) -> Result<(), PendingGlobalNativeErrorV18> {
        let mut consume = SourceCallbackCustodyV29::new(consume);
        self.roles.original.check(budget)?;
        self.roles.observe_custody(budget)?;
        let inventory = self
            .roles
            .optimized
            .pending_global_output_v18(self.roles.original, budget)?;
        native
            .check_owner(inventory.owner(), budget)
            .map_err(PendingGlobalNativeErrorV18::Native)?;
        let prepared = self.roles.original.retain_query((|| {
            let storage = argument_sum_v1(&[
                global_native_headers_v18(std::mem::size_of_val(&consume))?,
                source_owned_finish_preflight_v26::<(), PendingGlobalNativeErrorV18>(budget)?,
            ])?;
            budget.reserve_storage(storage)?;
            Ok(storage)
        })());
        let storage = match prepared {
            Ok(storage) => storage,
            Err(error) => {
                source_reference_discard_v29(consume.take());
                return Err(error.into());
            }
        };
        let ledger = budget.work_ledger_identity_v1();
        let slot = std::ptr::from_ref(&*budget) as usize;
        let retained = budget.storage();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let pair = self.access(operation, budget)?;
            match pair {
                None => consume
                    .take()
                    .expect("native access callback is invoked once")(
                    None, budget
                )?,
                Some(pair) => {
                    self.check_native_pair_v18(native, pair, budget)?;
                    consume
                        .take()
                        .expect("native access callback is invoked once")(
                        Some(&PendingGlobalSourceNativeAccessV18 { pair, native }),
                        budget,
                    )?;
                }
            }
            native
                .check_owner(inventory.owner(), budget)
                .map_err(PendingGlobalNativeErrorV18::Native)
        }));
        drop(consume);
        let prior = self.roles.original.source.guard.first.get();
        let custody = ledger == budget.work_ledger_identity_v1()
            && slot == std::ptr::from_ref(&*budget) as usize
            && budget.storage() >= retained;
        if !custody {
            self.roles.original.source.cleanup.deny_refund();
            let _ = native.refuse_retained_custody();
        }
        let postflight = if !custody || budget.storage() != retained {
            Err(ArgumentResourceV1::Accounting.into())
        } else {
            self.roles.observe_custody(budget).and_then(|()| {
                self.roles
                    .optimized
                    .pending_global_output_v18(self.roles.original, budget)
                    .map(|_| ())
            })
        };
        source_owned_finish_callback_v18(
            caught,
            prior,
            postflight,
            self.roles.original.source.cleanup,
            budget,
            storage,
        )
    }

    fn check_native_pair_v18(
        &self,
        native: &fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>,
        pair: &GlobalSourceAccessPairV18,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), PendingGlobalNativeErrorV18> {
        // Authenticate every logical and physical field, including normalized
        // source index identity, before checking the separate native occurrence.
        let authentic = self.access(pair.output.logical.access.operation, budget)?;
        self.roles
            .original
            .retain_query(budget.charge_work(160).map_err(Into::into))?;
        if authentic != Some(pair) {
            return self
                .roles
                .original
                .source
                .missing::<()>("pending global native substituted checked source pair")
                .map_err(Into::into);
        }
        let inventory = self
            .roles
            .optimized
            .pending_global_output_v18(self.roles.original, budget)?;
        let endpoint = &pair.output;
        for coordinate in [
            endpoint.logical.access.operation,
            endpoint.logical.data,
            endpoint.logical.length,
            endpoint.logical.address,
        ] {
            let actual = native
                .operation(inventory.owner(), coordinate, budget)
                .map_err(PendingGlobalNativeErrorV18::Native)?;
            let expected = source_operation_row_v18(inventory, coordinate, budget)?;
            if !actual.is_some_and(|actual| std::ptr::eq(actual, expected.operation)) {
                return Err(self
                    .roles
                    .original
                    .source
                    .missing::<()>("pending global native lost actual carrier occurrence")
                    .unwrap_err()
                    .into());
            }
        }
        let guard = native
            .guard_terminator(
                inventory.owner(),
                endpoint.logical.guard_edge.source,
                budget,
            )
            .map_err(PendingGlobalNativeErrorV18::Native)?;
        let expected = source_block_row_v18(inventory, endpoint.logical.guard_edge.source, budget)?;
        if !guard
            .zip(expected.block.terminator.as_ref())
            .is_some_and(|(actual, expected)| std::ptr::eq(actual, expected))
        {
            return Err(self
                .roles
                .original
                .source
                .missing::<()>("pending global native lost actual guard terminator")
                .unwrap_err()
                .into());
        }
        self.roles.original.retain_query((|| {
            budget.charge_work(48)?;
            let value = |coordinate, budget: &mut ArgumentBudgetV1<'_>| {
                optimized_source_definition_row_v18(inventory, coordinate, budget)?.value
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding("pending global native declaration value"))
            };
            let root = value(endpoint.logical.root, budget)?;
            let index = value(endpoint.address_index, budget)?;
            let condition = value(endpoint.logical.guard_condition, budget)?;
            let data = source_operation_row_v18(inventory, endpoint.logical.data, budget)?.operation;
            let length = source_operation_row_v18(inventory, endpoint.logical.length, budget)?.operation;
            let address = source_operation_row_v18(inventory, endpoint.logical.address, budget)?.operation;
            let access = source_operation_row_v18(inventory, endpoint.logical.access.operation, budget)?.operation;
            let data_value = match data.results.as_slice() { [value] => value.id,
                _ => return self.roles.original.source.missing("pending global native data result") };
            let access_matches = match (&access.kind, endpoint.writing) {
                (OperationKind::Load { pointer, access }, false) =>
                    *pointer == endpoint.pointer && *access == endpoint.memory,
                (OperationKind::Store { pointer, value, access }, true) =>
                    (*pointer, *value, *access) == (endpoint.pointer, endpoint.value, endpoint.memory),
                _ => false,
            };
            let guard_matches = match guard {
                Some(Terminator::ConditionalBranch { condition: actual, .. }) =>
                    *actual == condition && endpoint.logical.guard_edge.successor < 2,
                Some(Terminator::Switch { selector, cases, .. }) =>
                    *selector == condition && (endpoint.logical.guard_edge.successor as usize) <= cases.len(),
                Some(Terminator::IntegerSwitch { selector, cases, .. }) =>
                    *selector == condition && (endpoint.logical.guard_edge.successor as usize) <= cases.len(),
                _ => false,
            };
            if !access_matches || !guard_matches
                || !matches!(data.kind, OperationKind::SliceData { slice } if slice == root)
                || !matches!(length.kind, OperationKind::SliceLength { slice } if slice == root)
                || !matches!(address.kind, OperationKind::GetElementPointer { base, offset }
                    if base == data_value && offset == index)
                // Exact formation and actual access are joined independently.
                // The authenticated private pair already replayed every issued
                // Global/Generic cast and ancestor call edge between them.
                || !matches!(address.results.as_slice(), [value] if value.id == endpoint.formation_pointer)
                || (!endpoint.writing && !matches!(access.results.as_slice(), [value]
                    if value.id == endpoint.value && value.ty == Type::Scalar(endpoint.scalar)))
            { return self.roles.original.source.missing("pending global native changed exact access recipe"); }
            Ok(())
        })()).map_err(Into::into)
    }
}

#[cfg(test)]
include!("production_optimized_source_global_native_v18_tests.rs");
