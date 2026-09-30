// Local read conditions only. Runtime allocation, initialized contents,
// alias/read-from/concurrency, native completion and launch stay independent.
type GlobalReadFactsV18<'s, 'g> = fe2o3_kernel_ir::CheckedCanonicalGuardedGlobalReadsV18<'s, 'g>;
type GlobalReadFactV18<'s, 'g> = fe2o3_kernel_ir::CanonicalGuardedGlobalReadFactV18<'s, 'g>;
include!("production_source_global_domain_join_v30.rs");

#[derive(Debug)]
enum PendingGlobalReadConditionErrorV18 {
    Source(ProductionSourceOwnedViewErrorV18),
    Native(fe2o3_pliron::CanonicalRankedPolicyFailureV1),
    Formal {
        error: fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
        source_refusal: ProductionSourceOwnedViewErrorV18,
    },
    Unproved {
        operation: SliceOperation,
        reason: fe2o3_kernel_ir::CanonicalGuardedGlobalReadReasonV1,
        source_refusal: ProductionSourceOwnedViewErrorV18,
    },
}
impl From<ProductionSourceOwnedViewErrorV18> for PendingGlobalReadConditionErrorV18 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}
impl From<ArgumentResourceV1> for PendingGlobalReadConditionErrorV18 {
    fn from(error: ArgumentResourceV1) -> Self {
        Self::Source(error.into())
    }
}
impl From<PendingGlobalNativeErrorV18> for PendingGlobalReadConditionErrorV18 {
    fn from(error: PendingGlobalNativeErrorV18) -> Self {
        match error {
            PendingGlobalNativeErrorV18::Source(error) => Self::Source(error),
            PendingGlobalNativeErrorV18::Native(error) => Self::Native(error),
        }
    }
}

// A borrowed conjunction, not a copied row's authority or a completion token.
struct PendingGlobalReadConditionsV18<'s, 'g> {
    pair: &'s GlobalSourceAccessPairV18,
    native: &'s fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'s, 'g>,
    fact: GlobalReadFactV18<'s, 'g>,
}
impl PendingGlobalReadConditionsV18<'_, '_> {
    const fn requires_runtime_allocation_binding(&self) -> bool {
        self.fact.requires_runtime_allocation_binding()
    }
    const fn initialized_readable_region_is_proved(&self) -> bool {
        false
    }
    const fn grants_memory_or_launch_authority(&self) -> bool {
        false
    }
}

fn global_read_condition_headers_v18(
    capture: usize,
    alignment: usize,
) -> Result<usize, ArgumentResourceV1> {
    use fe2o3_kernel_analysis::{
        CanonicalKirBlockRefV1, CanonicalKirEdgeRefV1, CanonicalKirInventoryV18,
        CanonicalKirOperationRefV1,
    };
    type Frame<'a> = (
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'a, 'a>,
        &'a GlobalReadFactsV18<'a, 'a>,
        SliceOperation,
        &'a mut ArgumentBudgetV1<'a>,
    );
    type Capture<'a> = (
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'a, 'a>,
        &'a GlobalReadFactsV18<'a, 'a>,
        &'a CanonicalKirInventoryV18<'a>,
        &'a SliceOperation,
        &'a mut ArgumentBudgetV1<'a>,
        &'a mut (),
    );
    type DomainFrame<'a> = (
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a CanonicalKirInventoryV18<'a>,
        &'a GlobalReadFactsV18<'a, 'a>,
        &'a GlobalSourceAccessPairV18,
        &'a GlobalReadFactV18<'a, 'a>,
        &'a mut ArgumentBudgetV1<'a>,
    );
    type Join<'a> = (
        &'a CanonicalKirInventoryV18<'a>,
        &'a GlobalSourceAccessPairV18,
        &'a GlobalSourceAccessEndpointV18,
        &'a GlobalReadFactV18<'a, 'a>,
        &'a CanonicalKirDefinitionRefV1<'a>,
        &'a CanonicalKirDefinitionRefV1<'a>,
        &'a CanonicalKirOperationRefV1<'a>,
        &'a CanonicalKirBlockRefV1<'a>,
        &'a CanonicalKirEdgeRefV1<'a>,
        Option<&'a CanonicalKirEdgeRefV1<'a>>,
        &'a fe2o3_kernel_ir::Operation,
        &'a [fe2o3_kernel_ir::ValueDef],
        &'a fe2o3_kernel_ir::ValueDef,
        &'a Type,
        &'a fe2o3_kernel_ir::SliceType,
        &'a [CanonicalKirEdgeRefV1<'a>],
        &'a std::ops::Range<usize>,
        fe2o3_kernel_ir::FormalRuntimeSliceReadDomainV1,
        fe2o3_kernel_ir::FormalRuntimeSliceReadDomainV1,
        fe2o3_kernel_ir::FormalAllocationIdentity,
        Type,
        fe2o3_kernel_ir::FormalGuardedPathV1,
        (fe2o3_kernel_ir::BlockId, usize, fe2o3_kernel_ir::BlockId),
        [ValueId; 4],
        [usize; 3],
        [u32; 2],
        Option<u16>,
        u16,
        u64,
        bool,
        (
            &'a CanonicalKirInventoryV18<'a>,
            SliceDefinition,
            &'a mut ArgumentBudgetV1<'a>,
        ),
        SliceDefinition,
        fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
        Result<&'a CanonicalKirDefinitionRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        Result<&'a CanonicalKirDefinitionRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        Result<&'a CanonicalKirOperationRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        Result<&'a CanonicalKirBlockRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        Option<usize>,
        &'a usize,
        &'a std::ops::Range<usize>,
    );
    type NormalizedJoin<'a> = (
        [&'a CanonicalKirDefinitionRefV1<'a>; 2],
        Result<&'a CanonicalKirDefinitionRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        Result<&'a CanonicalKirDefinitionRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        SliceOperation,
        &'a CanonicalKirOperationRefV1<'a>,
        Result<&'a CanonicalKirOperationRefV1<'a>, ProductionSourceOwnedViewErrorV18>,
        &'a fe2o3_kernel_ir::Operation,
        &'a fe2o3_kernel_ir::OperationKind,
        &'a [fe2o3_kernel_ir::ValueDef],
        &'a fe2o3_kernel_ir::ValueDef,
        [&'a ValueId; 2],
        [&'a Type; 2],
        fe2o3_kernel_ir::CanonicalGuardedReadIndexOriginV1,
        (ValueId, ValueId),
        (ValueId, ValueId),
        [ValueId; 4],
        [Option<ValueId>; 2],
        [bool; 2],
        [Type; 3],
        SliceDefinition,
        fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
        [fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1; 3],
    );
    type ControlJoin<'a> = (
        &'a CanonicalKirDefinitionRefV1<'a>,
        SliceOperation,
        SliceDefinition,
        fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
        &'a CanonicalKirOperationRefV1<'a>,
        &'a fe2o3_kernel_ir::Operation,
        &'a fe2o3_kernel_ir::OperationKind,
        &'a [fe2o3_kernel_ir::ValueDef],
        &'a fe2o3_kernel_ir::ValueDef,
        &'a Option<Terminator>,
        &'a Terminator,
        Option<&'a Terminator>,
        [&'a ValueId; 3],
        [&'a Type; 3],
        [Type; 2],
        [Option<ValueId>; 2],
        bool,
        Option<&'a CanonicalKirDefinitionRefV1<'a>>,
        Result<
            Option<&'a CanonicalKirDefinitionRefV1<'a>>,
            fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
        >,
        SourceOwnedResultV18<Option<&'a CanonicalKirDefinitionRefV1<'a>>>,
        SourceOwnedResultV18<&'a CanonicalKirDefinitionRefV1<'a>>,
        SourceOwnedResultV18<&'a CanonicalKirOperationRefV1<'a>>,
        Result<(), ArgumentResourceV1>,
        (
            &'a CanonicalKirInventoryV18<'a>,
            SliceOperation,
            &'a mut ArgumentBudgetV1<'a>,
        ),
        (
            &'a CanonicalKirInventoryV18<'a>,
            fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
            ValueId,
            &'a mut ArgumentBudgetV1<'a>,
        ),
    );
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(
                2,
                size_of::<Result<T, PendingGlobalReadConditionErrorV18>>(),
            )?,
        ])
    }
    argument_sum_v1(&[
        capture,
        alignment,
        global_native_pair_headers_v18()?,
        source_global_domain_join_headers_v30()?,
        h::<Frame<'_>>()?,
        h::<Capture<'_>>()?,
        h::<Capture<'_>>()?,
        h::<DomainFrame<'_>>()?,
        h::<DomainFrame<'_>>()?,
        h::<Join<'_>>()?,
        h::<NormalizedJoin<'_>>()?,
        h::<ControlJoin<'_>>()?,
        h::<PendingGlobalReadConditionsV18<'_, '_>>()?,
        h::<&PendingGlobalReadConditionsV18<'_, '_>>()?,
        h::<Option<&PendingGlobalReadConditionsV18<'_, '_>>>()?,
        h::<GlobalReadFactV18<'_, '_>>()?,
        h::<fe2o3_kernel_ir::CanonicalGuardedGlobalReadOutcomeV18<'_, '_>>()?,
        h::<fe2o3_kernel_ir::CanonicalGuardedPredicateFactV18<'_, '_>>()?,
        h::<Option<fe2o3_kernel_ir::CanonicalGuardedPredicateFactV18<'_, '_>>>()?,
        h::<Option<&fe2o3_kernel_ir::CanonicalGuardedPredicateFactV18<'_, '_>>>()?,
        h::<(
            &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
            &GlobalReadFactV18<'_, '_>,
            &fe2o3_kernel_ir::FormalRuntimeSliceReadDomainV1,
            &fe2o3_kernel_ir::CanonicalGuardedPredicateFactV18<'_, '_>,
        )>()?,
        h::<fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1>()?,
        h::<fe2o3_kernel_ir::CanonicalGuardedGlobalReadReasonV1>()?,
        h::<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18>()?,
        h::<&GlobalSourceAccessPairV18>()?,
        h::<Option<&GlobalSourceAccessPairV18>>()?,
        h::<ProductionSourceOwnedViewErrorV18>()?,
        h::<PendingGlobalReadConditionErrorV18>()?,
        h::<Option<SourceOwnedQueryFailureV18>>()?,
        h::<SourceOwnedQueryFailureV18>()?,
        h::<(
            &ProductionSourceOwnedViewErrorV18,
            SourceOwnedQueryFailureV18,
        )>()?,
        h::<(&&'static str, &'static str)>()?,
        h::<ArgumentResourceV1>()?,
        h::<bool>()?,
        h::<&Option<fe2o3_kernel_ir::CanonicalGuardedPredicateFactV18<'_, '_>>>()?,
        h::<Result<(), fe2o3_pliron::CanonicalRankedPolicyFailureV1>>()?,
        h::<Result<(), PendingGlobalNativeErrorV18>>()?,
        h::<Option<ArgumentResourceV1>>()?,
        h::<&ArgumentResourceV1>()?,
        h::<Option<&GlobalSourceAccessPairV18>>()?,
        h::<&&GlobalSourceAccessPairV18>()?,
        h::<&Result<(), PendingGlobalReadConditionErrorV18>>()?,
        h::<SourceOwnedResultV18<()>>()?,
        h::<Result<(), ArgumentResourceV1>>()?,
        h::<
            Result<
                &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
                fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
            >,
        >()?,
        h::<
            Result<
                fe2o3_kernel_ir::CanonicalGuardedGlobalReadOutcomeV18<'_, '_>,
                fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
            >,
        >()?,
        h::<
            Result<
                Option<fe2o3_kernel_ir::CanonicalGuardedPredicateFactV18<'_, '_>>,
                fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
            >,
        >()?,
        h::<Result<(), PendingGlobalReadConditionErrorV18>>()?,
        h::<std::thread::Result<Result<(), PendingGlobalReadConditionErrorV18>>>()?,
        h::<std::panic::AssertUnwindSafe<Capture<'_>>>()?,
        h::<(
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
            [usize; 3],
        )>()?,
        h::<(
            &PendingGlobalReadConditionsV18<'_, '_>,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<(
            Option<&PendingGlobalReadConditionsV18<'_, '_>>,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<(
            &PendingGlobalSourceAccessesV18<'_>,
            &fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
        )>()?,
        h::<(
            &PendingGlobalSourceAccessesV18<'_>,
            SourceOwnedQueryFailureV18,
        )>()?,
        h::<(&ScopedSourceCleanupV29, &ArgumentResourceV1)>()?,
    ])
}

impl PendingGlobalSourceAccessesV18<'_> {
    // Call inside the existing with_optimized_guarded_reads_v18 scope. The
    // complete formal analysis is borrowed once, never rebuilt by this query.
    fn with_local_read_conditions_v18<'work>(
        &self,
        native: &fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>,
        facts: &GlobalReadFactsV18<'_, '_>,
        operation: SliceOperation,
        budget: &mut ArgumentBudgetV1<'work>,
        mut consume: impl for<'s, 'g> FnMut(
            Option<&PendingGlobalReadConditionsV18<'s, 'g>>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<(), PendingGlobalReadConditionErrorV18>,
    ) -> Result<(), PendingGlobalReadConditionErrorV18> {
        self.roles.original.check(budget)?;
        self.roles.observe_custody(budget)?;
        let inventory = self
            .roles
            .optimized
            .pending_global_output_v18(self.roles.original, budget)?;
        native
            .check_owner(inventory.owner(), budget)
            .map_err(PendingGlobalReadConditionErrorV18::Native)?;
        let storage = self.roles.original.retain_query(
            global_read_condition_headers_v18(
                std::mem::size_of_val(&consume),
                std::mem::align_of_val(&consume),
            )
            .map_err(Into::into),
        )?;
        self.roles
            .original
            .retain_query(budget.reserve_storage(storage).map_err(Into::into))?;
        let ledger = budget.work_ledger_identity_v1();
        let slot = std::ptr::from_ref(&*budget) as usize;
        let required = budget.storage();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let result = (|| {
                let owner = facts
                    .owner(budget)
                    .map_err(|error| self.local_read_formal_error(error))?;
                if !std::ptr::eq(owner, inventory.owner()) {
                    return Err(self
                        .roles
                        .original
                        .source
                        .missing::<()>("pending global read changed formal owner")
                        .unwrap_err()
                        .into());
                }
                let pair = self.access(operation, budget)?;
                let result = match pair.filter(|pair| !pair.output.writing) {
                    None => consume(None, budget),
                    Some(pair) => {
                        self.check_native_pair_v18(native, pair, budget)?;
                        let outcome = facts
                            .read_at(operation, budget)
                            .map_err(|error| self.local_read_formal_error(error))?;
                        let fe2o3_kernel_ir::CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(fact) = outcome
                        else {
                            let fe2o3_kernel_ir::CanonicalGuardedGlobalReadOutcomeV18::NotProved(reason) = outcome else { unreachable!() };
                            let source_refusal = self.roles.original.source.missing::<()>(
                                "pending global read local conditions are unproved").unwrap_err();
                            return Err(PendingGlobalReadConditionErrorV18::Unproved { operation, reason, source_refusal });
                        };
                        self.check_local_read_domain_v18(inventory, facts, pair, &fact, budget)?;
                        consume(
                            Some(&PendingGlobalReadConditionsV18 { pair, native, fact }),
                            budget,
                        )
                    }
                };
                result?;
                native
                    .check_owner(inventory.owner(), budget)
                    .map_err(PendingGlobalReadConditionErrorV18::Native)
            })();
            // Classify the entire query before destroying the owned callback.
            // A destructor panic must not replace an earlier resource refusal.
            let resource = match &result {
                Err(PendingGlobalReadConditionErrorV18::Source(
                    ProductionSourceOwnedViewErrorV18::Resource(error),
                ))
                | Err(PendingGlobalReadConditionErrorV18::Native(
                    fe2o3_pliron::CanonicalRankedPolicyFailureV1::Resource(error),
                )) => Some(*error),
                _ => None,
            };
            if let Some(error) = resource {
                let _ = self.roles.original.retain_query(Err::<(), _>(error.into()));
            }
            let first = self.roles.original.source.guard.first.get();
            // Keep the richer formal diagnostic only when its recorded source
            // refusal is the selected first error. A later callback sentinel
            // cannot replace an earlier swallowed query/resource refusal.
            let same_formal = match (&result, first) {
                (
                    Err(
                        PendingGlobalReadConditionErrorV18::Formal { source_refusal, .. }
                        | PendingGlobalReadConditionErrorV18::Unproved { source_refusal, .. },
                    ),
                    Some(first),
                ) => match (source_refusal, first) {
                    (
                        ProductionSourceOwnedViewErrorV18::Resource(actual),
                        SourceOwnedQueryFailureV18::Resource(expected),
                    ) => *actual == expected,
                    (
                        ProductionSourceOwnedViewErrorV18::Binding(actual),
                        SourceOwnedQueryFailureV18::Binding(expected),
                    ) => *actual == expected,
                    _ => false,
                },
                _ => false,
            };
            let result = match first {
                Some(first) if !same_formal => {
                    Err(PendingGlobalReadConditionErrorV18::Source(first.error()))
                }
                _ => result,
            };
            drop(consume);
            result
        }));
        let custody = ledger == budget.work_ledger_identity_v1()
            && slot == std::ptr::from_ref(&*budget) as usize
            && budget.storage() >= required;
        if !custody {
            self.roles.original.source.cleanup.deny_refund();
            let _ = native.refuse_retained_custody();
        }
        let postflight = if !custody || budget.storage() != required {
            self.roles
                .original
                .retain_query(Err(ArgumentResourceV1::Accounting.into()))
        } else {
            self.roles.observe_custody(budget)
        };
        // Query errors already retain their source refusal and must keep their
        // concrete formal/native diagnostic. Refund only this frame, not a floor.
        let released = if self.roles.original.source.cleanup.is_denied() {
            Err(ArgumentResourceV1::Accounting)
        } else {
            budget
                .release_storage(storage)
                .inspect_err(|_| self.roles.original.source.cleanup.deny_refund())
        };
        match caught {
            Err(payload) => std::panic::resume_unwind(payload),
            Ok(Err(error)) => Err(error),
            Ok(Ok(())) => postflight
                .and(released.map_err(Into::into))
                .map_err(Into::into),
        }
    }

    fn local_read_formal_error(
        &self,
        error: fe2o3_kernel_ir::CanonicalGuardedGlobalReadErrorV1,
    ) -> PendingGlobalReadConditionErrorV18 {
        let source_refusal =
            optimized_source_observed_formal_error_v18(self.roles.original, &error);
        PendingGlobalReadConditionErrorV18::Formal {
            error,
            source_refusal,
        }
    }

    fn check_local_read_domain_v18(
        &self,
        inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
        facts: &GlobalReadFactsV18<'_, '_>,
        pair: &GlobalSourceAccessPairV18,
        fact: &GlobalReadFactV18<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), PendingGlobalReadConditionErrorV18> {
        check_source_read_endpoint_prepaid_v30(
            self.roles.original,
            inventory,
            facts,
            pair,
            fact,
            budget,
        )
    }
}

#[cfg(test)]
include!("production_optimized_source_global_read_conditions_commands_v18_tests.rs");

include!("production_optimized_source_shared_entry_v18.rs");
