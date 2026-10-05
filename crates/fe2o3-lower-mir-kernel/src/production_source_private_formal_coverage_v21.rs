// Exact report occurrences joined while the completed native owner is live.
impl From<ArgumentResourceV1> for ProductionSourceNativeLifecycleErrorV18 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Source(ProductionSourceOwnedViewErrorV18::Resource(error))
    }
}

impl ProductionPrivateMemoryCheckedNativePoliciesV18<'_, '_> {
    #[cfg(test)]
    pub(in crate::production_semantic_kir_v1) fn test_private_formal_storage_pointer_v21(
        before: &fe2o3_kernel_ir::OperationKind,
        after: &fe2o3_kernel_ir::OperationKind,
        pointer: ValueId,
    ) -> Option<ValueId> {
        private_formal_storage_pointer_v21(before, after, pointer)
    }

    pub(crate) fn check_formal_report_subject_v21(
        &self,
        report: &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> NativeResult {
        self.check(budget)?;
        self.completion
            .original
            .retain_query(budget.charge_work(6).map_err(Into::into))?;
        let input = self.completion.optimized.checked.input();
        let output = self.completion.optimized.checked.output();
        let inventory = if std::ptr::eq(report.original_owner(), input.owner()) {
            input
        } else if std::ptr::eq(report.original_owner(), output.owner()) {
            output
        } else {
            return Err(self
                .recipes
                .source_failure("private formal report changed owner"));
        };
        let root = inventory
            .kernels()
            .get(report.root_index())
            .ok_or_else(|| {
                self.recipes
                    .source_failure("private formal reason root is absent")
            })?;
        if !inventory
            .functions()
            .get(root.entry.0 as usize)
            .is_some_and(|row| std::ptr::eq(row.function, report.original_function()))
        {
            return Err(self
                .recipes
                .source_failure("private formal reason function differs"));
        }
        Ok(())
    }

    pub(crate) fn check_formal_reason_v21<'work>(
        &self,
        report: &fe2o3_kernel_ir::CanonicalFormalReportViewV19<'_, '_>,
        reason: usize,
        budget: &mut ArgumentBudgetV1<'work>,
    ) -> NativeResult {
        self.check(budget)?;
        let original = self.completion.original;
        let optimized = self.completion.optimized;
        let floor = budget.storage();
        let consume = |budget: &mut ArgumentBudgetV1<'work>| {
            self.check_formal_report_subject_v21(report, budget)?;
            let headers = argument_sum_v1(&[
                size_of::<&fe2o3_kernel_ir::CanonicalFormalSourceScopeV20<'_, '_>>(),
                size_of::<&fe2o3_kernel_ir::CanonicalFormalReportViewV19<'_, '_>>(),
                size_of::<&Inventory<'_>>(),
                size_of::<&fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>(),
                size_of::<&CanonicalRankedSourceObligationV18>(),
                size_of::<Option<&CanonicalRankedSourceObligationV18>>(),
                size_of::<Option<&fe2o3_kernel_ir::FormalMemoryIncompleteReason>>(),
                size_of::<Option<ValueId>>() * 2,
                size_of::<Option<usize>>(),
                size_of::<&fe2o3_kernel_analysis::CanonicalKirPrivateMemoryAddressV1>(),
                size_of::<Option<&fe2o3_kernel_analysis::CanonicalKirPrivateMemoryAddressV1>>(),
                size_of::<(&fe2o3_kernel_ir::OperationKind, &fe2o3_kernel_ir::OperationKind, ValueId)>(),
                size_of::<(ValueId, ValueId, &fe2o3_kernel_ir::MemoryAccess, &fe2o3_kernel_ir::MemoryAccess)>(),
                size_of::<Result<Option<usize>, fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1>>(),
                size_of::<fe2o3_kernel_ir::FunctionOperationLocation>(),
                size_of::<OpCoordinate>() * 2,
                size_of::<CanonicalRankedSourceRequirementV18>(),
                size_of::<fe2o3_kernel_ir::CanonicalFormalReportErrorV19>(),
                size_of::<Result<Option<usize>, fe2o3_kernel_ir::CanonicalFormalReportErrorV19>>(),
                size_of::<NativeResult>(),
                size_of::<[usize; 16]>(),
            ])
            .map_err(ProductionSourceOwnedViewErrorV18::from)?;
            original.retain_query(budget.reserve_storage(headers).map_err(Into::into))?;
            original.retain_query(budget.charge_work(12).map_err(Into::into))?;
            let input = optimized.checked.input();
            let output = optimized.checked.output();
            let is_input = std::ptr::eq(report.original_owner(), input.owner());
            let inventory = if is_input {
                input
            } else if std::ptr::eq(report.original_owner(), output.owner()) {
                output
            } else {
                return Err(self
                    .recipes
                    .source_failure("private formal report changed owner"));
            };
            let (location, pointer) = match report.analysis().incomplete_reasons().get(reason) {
                Some(fe2o3_kernel_ir::FormalMemoryIncompleteReason::UnsupportedMemoryEffect {
                    location,
                }) => (location, None),
                Some(
                    fe2o3_kernel_ir::FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                        location,
                        pointer,
                    },
                ) => (location, Some(*pointer)),
                _ => {
                    return Err(self
                        .recipes
                        .source_failure("private formal reason lacks exact memory occurrence"));
                }
            };
            let function = inventory
                .kernels()
                .get(report.root_index())
                .ok_or_else(|| {
                    self.recipes
                        .source_failure("private formal reason root is absent")
                })?
                .entry;
            if !inventory
                .functions()
                .get(function.0 as usize)
                .is_some_and(|row| std::ptr::eq(row.function, report.original_function()))
            {
                return Err(self
                    .recipes
                    .source_failure("private formal reason function differs"));
            }
            let source = report.source_scope_v20(budget).map_err(|error| {
                NativeError::Source(optimized_source_report_refusal_v19(original, &error))
            })?;
            let block = source
                .block_ordinal(location.block, budget)
                .map_err(|error| {
                    NativeError::Source(optimized_source_report_refusal_v19(original, &error))
                })?
                .ok_or_else(|| {
                    self.recipes
                        .source_failure("private formal reason block is absent")
                })?;
            let coordinate = OpCoordinate {
                block: Block {
                    function,
                    block: u32::try_from(block).map_err(|_| {
                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Arithmetic)
                    })?,
                },
                operation: u32::try_from(location.operation_index).map_err(|_| {
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Arithmetic)
                })?,
            };
            let actual = resources::operation_index(inventory, coordinate, budget)?;
            let expected = match inventory.operations()[actual].operation.kind {
                fe2o3_kernel_ir::OperationKind::Alloca { .. }
                | fe2o3_kernel_ir::OperationKind::Storage(_) => {
                    CanonicalRankedSourceRequirementV18::Memory
                }
                fe2o3_kernel_ir::OperationKind::Execution(_) => {
                    CanonicalRankedSourceRequirementV18::Execution
                }
                _ => {
                    return Err(self
                        .recipes
                        .source_failure("private formal reason operation is not covered"));
                }
            };
            let mapped = if is_input {
                optimized
                    .index
                    .operation(optimized.checked, coordinate, budget)?
                    .ok_or_else(|| {
                        self.recipes
                            .source_failure("private formal reason lost its output operation")
                    })?
            } else {
                coordinate
            };
            let index = resources::operation_index(output, mapped, budget)?;
            original.retain_query(budget.charge_work(3).map_err(Into::into))?;
            let matching_kind = matches!(
                (
                    &inventory.operations()[actual].operation.kind,
                    &output.operations()[index].operation.kind
                ),
                (
                    fe2o3_kernel_ir::OperationKind::Alloca { .. },
                    fe2o3_kernel_ir::OperationKind::Alloca { .. }
                ) | (
                    fe2o3_kernel_ir::OperationKind::Storage(_),
                    fe2o3_kernel_ir::OperationKind::Storage(_)
                ) | (
                    fe2o3_kernel_ir::OperationKind::Execution(_),
                    fe2o3_kernel_ir::OperationKind::Execution(_)
                )
            );
            if !matching_kind {
                return Err(self
                    .recipes
                    .source_failure("private formal reason output role differs"));
            }
            if let Some(pointer) = pointer {
                original.retain_query(budget.charge_work(16).map_err(Into::into))?;
                let before = &inventory.operations()[actual].operation.kind;
                let after = &output.operations()[index].operation.kind;
                let address = private_formal_storage_pointer_v21(before, after, pointer)
                    .ok_or_else(|| {
                        self.recipes.source_failure(
                            "private formal pointer reason is not an exact typed private access",
                        )
                    })?;
                let definition = output
                    .definition_index_for_value(mapped.block.function, address, budget)
                    .map_err(|error| {
                        ProductionSourceOwnedViewErrorV18::from(
                            fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(error),
                        )
                    })?
                    .ok_or_else(|| {
                        self.recipes.source_failure(
                            "private formal pointer reason has no output definition",
                        )
                    })?;
                let proof = self.completion.physical.native_physical_v18(budget)?;
                if self.completion.coverage.get(index) != Some(&true)
                    || !proof.address(definition).is_some_and(|address| {
                        address.offset() == 0
                            && address.length() == 1
                            && self.completion.coverage.get(address.allocation()) == Some(&true)
                    })
                {
                    return Err(self.recipes.source_failure(
                        "private formal pointer reason lacks completed whole-scalar coverage",
                    ));
                }
            }
            let key = operation_key(mapped);
            let (mut lo, mut hi) = (0, self.obligations.len());
            while lo < hi {
                original.retain_query(budget.charge_work(4).map_err(Into::into))?;
                let mid = lo + (hi - lo) / 2;
                if operation_key(self.obligations[mid].coordinate()) < key {
                    lo = mid + 1;
                } else {
                    hi = mid;
                }
            }
            original.retain_query(budget.charge_work(5).map_err(Into::into))?;
            let obligation = self
                .obligations
                .get(lo)
                .filter(|row| row.coordinate() == mapped && row.requirement() == expected)
                .ok_or_else(|| {
                    self.recipes
                        .source_failure("private formal reason has no completed obligation")
                })?;
            if expected == CanonicalRankedSourceRequirementV18::Memory
                && self.completion.coverage.get(index) != Some(&true)
            {
                let proof = self.completion.physical.native_physical_v18(budget)?;
                if !proof.access_restriction(index).is_some_and(|address| {
                    self.completion.coverage.get(address.allocation()) == Some(&true)
                }) {
                    return Err(NativeError::Unresolved(*obligation));
                }
            }
            let function = mapped.block.function.0 as usize;
            if self
                .report(function, budget)?
                .is_none_or(|report| !report.is_clean())
                || self.history(function, budget)?.is_none()
            {
                return Err(self
                    .recipes
                    .source_failure("private formal reason lacks clean function history"));
            }
            self.check_source_subject_v18(original, optimized, budget)?;
            original.retain_query(budget.release_storage(headers).map_err(Into::into))?;
            Ok(())
        };
        #[cfg(test)]
        {
            // Independent test oracle: five borrowed captures, no owned input.
            assert_eq!(std::mem::size_of_val(&consume), size_of::<[usize; 5]>());
            assert_eq!(
                std::mem::align_of_val(&consume),
                std::mem::align_of::<usize>()
            );
        }
        scoped_source_attempt_v29(original.source.cleanup, budget, floor, consume).map_err(
            |error| match error {
                NativeError::Source(ProductionSourceOwnedViewErrorV18::Resource(resource)) => {
                    NativeError::Source(original.retain_query_resource_error_v18(resource))
                }
                error => error,
            },
        )
    }
}

// The legacy affine private-slot classifier conservatively records typed
// Storage operands as escapes. Only the exact address role can be discharged
// here; stored pointers, copies, projections and unrelated escapes stay pending.
fn private_formal_storage_pointer_v21(
    before: &fe2o3_kernel_ir::OperationKind,
    after: &fe2o3_kernel_ir::OperationKind,
    pointer: ValueId,
) -> Option<ValueId> {
    use fe2o3_kernel_ir::{AddressSpace, OperationKind, StorageOperationV1};
    let (original, output, first, second) = match (before, after) {
        (
            OperationKind::Storage(StorageOperationV1::ReadValue {
                address: original,
                access: first,
            }),
            OperationKind::Storage(StorageOperationV1::ReadValue {
                address: output,
                access: second,
            }),
        )
        | (
            OperationKind::Storage(StorageOperationV1::WriteValue {
                address: original,
                access: first,
                ..
            }),
            OperationKind::Storage(StorageOperationV1::WriteValue {
                address: output,
                access: second,
                ..
            }),
        ) => (*original, *output, first, second),
        _ => return None,
    };
    (original == pointer
        && first == second
        && first.address_space == AddressSpace::Private
        && !first.volatile)
        .then_some(output)
}
