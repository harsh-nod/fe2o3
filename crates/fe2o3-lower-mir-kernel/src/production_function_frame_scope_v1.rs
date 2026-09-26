// Only this module can separate a live cursor from its accounting receipt.
// Its higher-ranked consumption boundary cannot return the cursor or a value
// containing it. Suspended emission retains the complete owning value instead.
mod execution_availability_owner_v1 {
    use super::*;

    pub(super) struct ExecutionAvailabilityLeaseV1<'source> {
        references: Option<&'source SourceReferenceEmissionV29<'source, 'source>>,
        growth: Option<EmissionServiceGrowthV1<'source>>,
        entry: usize,
        required: usize,
        owned: usize,
        slot: usize,
        ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        denied: std::cell::Cell<bool>,
    }

    pub(super) struct OwnedExecutionAvailabilityV1<'source> {
        cursor: ExecutionAvailabilityV29<'source>,
        lease: ExecutionAvailabilityLeaseV1<'source>,
    }

    pub(super) struct CompletedExecutionAvailabilityV1<'source> {
        lease: ExecutionAvailabilityLeaseV1<'source>,
    }

    pub(super) struct OwnedFunctionFrameV1<'source, 'service> {
        prepared: PreparedFunctionFrameV1<'source, 'service>,
        plan_storage: FunctionPlanStorageV1<'source>,
        lease: ExecutionAvailabilityLeaseV1<'source>,
    }

    pub(super) struct SuspendedFunctionFrameV1<'source> {
        frame: DetachedFunctionFrameV1<'source, 'source>,
        context: FunctionFrameOutputContextV1<'source>,
        plan_storage: FunctionPlanStorageV1<'source>,
        lease: ExecutionAvailabilityLeaseV1<'source>,
    }

    pub(super) struct SuspendedCallFunctionFrameV1<'source> {
        owner: SuspendedFunctionFrameV1<'source>,
        pending: AwaitingDefinedCallV1,
        start: SuspendedCallStartV1,
    }

    pub(super) struct CompletedFunctionFrameV1<'source> {
        output: UnassembledFunctionFrameV1<'source>,
        plan_storage: FunctionPlanStorageV1<'source>,
        availability: CompletedExecutionAvailabilityV1<'source>,
    }

    pub(super) fn execution_availability_consumption_headers_v1<R>()
    -> Result<usize, ArgumentResourceV1> {
        type Outcome<R> = std::thread::Result<Result<R, ProductionSemanticKirErrorV1>>;
        argument_sum_v1(&[
            std::mem::size_of::<Outcome<R>>(),
            std::mem::size_of::<(CompletedExecutionAvailabilityV1<'static>, Outcome<R>)>(),
            std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
        ])
    }

    pub(super) fn execution_availability_lease_headers_v1() -> Result<usize, ArgumentResourceV1> {
        type Payload = Box<dyn std::any::Any + Send>;
        argument_sum_v1(&[
            source_reference_emission_headers_v29::<ExecutionAvailabilityV29<'static>>()?,
            std::mem::size_of::<ExecutionAvailabilityLeaseV1<'static>>(),
            std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<
                Result<
                    Result<ExecutionAvailabilityV29<'static>, ProductionSemanticKirErrorV1>,
                    Payload,
                >,
            >(),
            std::mem::size_of::<Option<Payload>>(),
            std::mem::size_of::<Result<(), Payload>>(),
            std::mem::size_of::<
                Result<OwnedExecutionAvailabilityV1<'static>, ProductionSemanticKirErrorV1>,
            >(),
        ])
    }

    impl<'source> ExecutionAvailabilityLeaseV1<'source> {
        fn begin(
            references: Option<&'source SourceReferenceEmissionV29<'source, 'source>>,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<Self, ProductionSemanticKirErrorV1> {
            let entry = budget.storage();
            let owned = execution_availability_lease_headers_v1()?;
            budget.reserve_storage(owned)?;
            let mut lease = Self {
                references,
                growth: None,
                entry,
                required: argument_sum_v1(&[entry, owned])?,
                owned,
                slot: budget as *const ArgumentBudgetV1<'_> as usize,
                ledger: budget.work_ledger_identity_v1(),
                denied: std::cell::Cell::new(false),
            };
            let captured = (|| {
                // Initial, post-construction, and final receipt comparison are
                // prepaid, so selected error/panic cleanup never spends work.
                budget.charge_work(4 + 4 + 4)?;
                if let Some(root) =
                    references.and_then(|references| references.plan.storage_root.as_ref())
                {
                    let references = references.ok_or(ArgumentResourceV1::Accounting)?;
                    budget.source_reference_owner_v29(references.plan)?;
                    budget.charge_work(argument_sum_v1(&[
                        source_storage_v29::SOURCE_STORAGE_ROOT_GROWTH_WORK_V29,
                        4,
                    ])?)?;
                    lease.growth = Some(
                        root.capture_retained_growth()
                            .ok_or(ArgumentResourceV1::Accounting)?,
                    );
                }
                Ok(())
            })();
            if let Err(error) = captured {
                let _ = CompletedExecutionAvailabilityV1 { lease }.release(budget);
                return Err(error);
            }
            Ok(lease)
        }

        fn capture_cursor_storage(
            &mut self,
            budget: &ArgumentBudgetV1<'_>,
        ) -> Result<(), ProductionSemanticKirErrorV1> {
            if self.slot != budget as *const ArgumentBudgetV1<'_> as usize
                || self.ledger != budget.work_ledger_identity_v1()
                || budget.storage() < self.required
            {
                self.deny_refund();
                return Err(ArgumentResourceV1::Accounting.into());
            }
            self.required = budget.storage();
            self.owned = self
                .required
                .checked_sub(self.entry)
                .ok_or(ArgumentResourceV1::Accounting)?;
            // Cursor construction is a read-only source operation. A future arena
            // allocation must not silently become cursor-owned disposable credit.
            if !self.permits_release(budget) {
                self.deny_refund();
                return Err(ArgumentResourceV1::Accounting.into());
            }
            Ok(())
        }

        fn permits_release(&self, budget: &ArgumentBudgetV1<'_>) -> bool {
            !self.denied.get()
                && budget.permits_prepared_input_refund_v1(
                    self.references.map(|references| references.plan),
                    self.slot,
                    self.ledger,
                    self.required,
                    self.owned,
                )
                && self.growth.as_ref().is_none_or(|growth| {
                    growth.permits_refund(self.entry, self.required, budget.storage(), self.owned)
                })
        }

        fn deny_refund(&self) {
            self.denied.set(true);
            if let Some(root) = self
                .references
                .and_then(|references| references.plan.storage_root.as_ref())
            {
                root.deny_active_root_refund();
            }
        }

        fn reserve_consumption<R>(
            &mut self,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<(), ProductionSemanticKirErrorV1> {
            if !self.permits_release(budget) {
                self.deny_refund();
                return Err(ArgumentResourceV1::Accounting.into());
            }
            let headers = execution_availability_consumption_headers_v1::<R>()?;
            let required = argument_sum_v1(&[self.required, headers])?;
            let owned = argument_sum_v1(&[self.owned, headers])?;
            budget.reserve_storage(headers)?;
            self.required = required;
            self.owned = owned;
            budget.charge_work(4 + 4)?;
            if !self.permits_release(budget) {
                self.deny_refund();
                return Err(ArgumentResourceV1::Accounting.into());
            }
            Ok(())
        }
    }

    impl CompletedExecutionAvailabilityV1<'_> {
        pub(super) fn permits_release(&self, budget: &ArgumentBudgetV1<'_>) -> bool {
            self.lease.permits_release(budget)
        }

        // This receipt can only be produced after the cursor has been discarded or
        // consumed by the higher-ranked callback. Extra outputs and C2 growth stay paid.
        pub(super) fn release(
            self,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<(), ProductionSemanticKirErrorV1> {
            let Self { lease } = self;
            let allowed = lease.permits_release(budget);
            if !allowed {
                lease.deny_refund();
            }
            let owned = lease.owned;
            let references = lease.references;
            drop(lease);
            if !allowed {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            if let Err(error) = budget.release_storage(owned) {
                if let Some(root) =
                    references.and_then(|references| references.plan.storage_root.as_ref())
                {
                    root.deny_active_root_refund();
                }
                return Err(error.into());
            }
            Ok(())
        }
    }

    impl<'source> OwnedExecutionAvailabilityV1<'source> {
        pub(super) fn with_call_parameters<'scope>(
            self,
            parameters: PreparedExecutionParametersV29<'scope>,
        ) -> Result<OwnedExecutionAvailabilityV1<'scope>, ProductionSemanticKirErrorV1>
        where
            'source: 'scope,
        {
            let Self { cursor, lease } = self;
            Ok(OwnedExecutionAvailabilityV1 {
                cursor: cursor.with_call_parameters_v29(parameters)?,
                lease,
            })
        }

        pub(super) fn new(
            instances: &ExecutionInstancesV29<'source>,
            instance: ProductionCallInstanceIdV1,
            references: Option<&'source SourceReferenceEmissionV29<'source, 'source>>,
            identities: Option<&'source ExecutionIdentityPlanV1<'source, 'source>>,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<Self, ProductionSemanticKirErrorV1> {
            if let Some(references) = references {
                references.check(budget)?;
            }
            let mut lease = ExecutionAvailabilityLeaseV1::begin(references, budget)?;
            let constructed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                ExecutionAvailabilityV29::new_with_identity(
                    instances, instance, references, identities, budget,
                )
            }));
            let captured = lease.capture_cursor_storage(budget);
            match constructed {
                Ok(Ok(cursor)) => {
                    if let Err(error) = captured {
                        drop(cursor);
                        drop(lease);
                        return Err(error);
                    }
                    Ok(Self { cursor, lease })
                }
                Ok(Err(error)) => {
                    if captured.is_ok() {
                        let _ = CompletedExecutionAvailabilityV1 { lease }.release(budget);
                    }
                    Err(error)
                }
                Err(payload) => {
                    // Keep the established source-availability panic boundary.
                    // The enclosing source scope owns any rejected panic payload.
                    let _ = source_reference_discard_v29([Some(payload)]);
                    if captured.is_ok() {
                        let _ = CompletedExecutionAvailabilityV1 { lease }.release(budget);
                    }
                    Err(source_reference_error_v29(
                        "source reference availability construction or callback panicked",
                    ))
                }
            }
        }

        pub(super) fn consume<R>(
            self,
            budget: &mut ArgumentBudgetV1<'_>,
            consume: impl for<'cursor> FnOnce(
                ExecutionAvailabilityV29<'cursor>,
                &mut ArgumentBudgetV1<'_>,
            ) -> Result<R, ProductionSemanticKirErrorV1>,
        ) -> (
            CompletedExecutionAvailabilityV1<'source>,
            std::thread::Result<Result<R, ProductionSemanticKirErrorV1>>,
        ) {
            let Self { cursor, mut lease } = self;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                lease.reserve_consumption::<R>(budget)?;
                consume(cursor, budget)
            }));
            (CompletedExecutionAvailabilityV1 { lease }, result)
        }

        pub(super) fn discard(
            self,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<(), ProductionSemanticKirErrorV1> {
            let Self { cursor, lease } = self;
            drop(cursor);
            CompletedExecutionAvailabilityV1 { lease }.release(budget)
        }

        #[allow(clippy::too_many_arguments)]
        pub(super) fn prepare_frame<'service>(
            self,
            semantic: &'source fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
            plan: FunctionEmissionPlanV1<'source>,
            semantic_ssa: &'source ProductionSemanticSsaFunctionPlanV1,
            defined_function_ids: &'source BTreeMap<SemanticFunctionIdV1, FunctionId>,
            defined_function_signatures: impl Into<ExecutionSignatureSourceV29<'source>>,
            required_workgroup: Option<[u32; 3]>,
            infallible_asserts: InfallibleAssertDecisionsV1<'source>,
            launch_rank: u8,
            authenticated_ranked_control: bool,
            max_operations: usize,
            private_array_work: &'service mut PrivateArrayLazyBudgetV1,
            private_array_sources: Option<PrivateArraySourcesV1<'source>>,
            budget: &'service mut ArgumentBudgetV1<'_>,
            placement: SemanticEmissionPlacementV1,
            execution_calls: &'service mut (dyn ExecutionDefinedCallConsumerV29 + 'service),
            lifecycle: &'service mut (dyn ExecutionLifecycleConsumerV29 + 'service),
        ) -> Result<OwnedFunctionFrameV1<'source, 'service>, ProductionSemanticKirErrorV1> {
            let Self { cursor, mut lease } = self;
            // Unlike consume<R>, this boundary has exactly one concrete output
            // owner. No callback can separate a live frame from the cursor lease.
            lease.reserve_consumption::<OwnedFunctionFrameV1<'source, 'service>>(budget)?;
            let plan_storage = FunctionPlanStorageV1::new(Some(&cursor), budget)?;
            let prepared = prepare_function_frame_v1(
                semantic,
                plan,
                semantic_ssa,
                defined_function_ids,
                defined_function_signatures,
                required_workgroup,
                infallible_asserts,
                launch_rank,
                authenticated_ranked_control,
                max_operations,
                false,
                private_array_work,
                private_array_sources,
                budget,
                placement,
                Some(cursor),
                Some(execution_calls),
                Some(lifecycle),
            )?;
            Ok(OwnedFunctionFrameV1 {
                prepared,
                plan_storage,
                lease,
            })
        }
    }

    impl<'source, 'service> OwnedFunctionFrameV1<'source, 'service> {
        pub(super) fn step(&mut self) -> Result<bool, ProductionSemanticKirErrorV1> {
            self.prepared.frame.step(None)
        }

        pub(super) fn step_until_child(&mut self) -> Result<FunctionFrameStepV1, ProductionSemanticKirErrorV1> {
            self.prepared.frame.step_inner_v1(None, true)
        }

        pub(super) fn detach_call(
            self,
            pending: AwaitingDefinedCallV1,
        ) -> Result<(SuspendedCallFunctionFrameV1<'source>, EmissionServicesV1<'service>), ProductionSemanticKirErrorV1> {
            let Self { prepared, plan_storage, lease } = self;
            let (frame, services, start) = prepared.frame.detach_call_v1(&pending)?;
            Ok((SuspendedCallFunctionFrameV1 {
                owner: SuspendedFunctionFrameV1 { frame, context: prepared.context, plan_storage, lease },
                pending,
                start,
            }, services))
        }

        pub(super) fn detach(
            self,
        ) -> Result<
            (
                SuspendedFunctionFrameV1<'source>,
                EmissionServicesV1<'service>,
            ),
            ProductionSemanticKirErrorV1,
        > {
            let Self {
                prepared,
                plan_storage,
                lease,
            } = self;
            let (frame, services) = prepared.frame.detach()?;
            Ok((
                SuspendedFunctionFrameV1 {
                    frame,
                    context: prepared.context,
                    plan_storage,
                    lease,
                },
                services,
            ))
        }

        pub(super) fn finish(
            self,
        ) -> Result<CompletedFunctionFrameV1<'source>, ProductionSemanticKirErrorV1> {
            let Self {
                prepared,
                plan_storage,
                lease,
            } = self;
            let output = prepared.finish_lowering(None)?;
            // finish_lowering consumed and dropped the cursor before returning
            // this artifact. There is no service borrow or live cursor in it.
            Ok(CompletedFunctionFrameV1 {
                output,
                plan_storage,
                availability: CompletedExecutionAvailabilityV1 { lease },
            })
        }
    }

    impl<'source> SuspendedFunctionFrameV1<'source> {
        pub(super) fn attach<'service>(
            self,
            services: EmissionServicesV1<'service>,
        ) -> Result<OwnedFunctionFrameV1<'source, 'service>, ProductionSemanticKirErrorV1> {
            Ok(OwnedFunctionFrameV1 {
                prepared: PreparedFunctionFrameV1 {
                    frame: self.frame.attach(services)?,
                    context: self.context,
                },
                plan_storage: self.plan_storage,
                lease: self.lease,
            })
        }
    }

    impl<'source> SuspendedCallFunctionFrameV1<'source> {
        pub(super) fn request(&self) -> &ExecutionCallRequestV1 { &self.pending.request }

        pub(super) fn start(&self) -> &SuspendedCallStartV1 { &self.start }

        pub(super) fn resume<'service>(
            mut self,
            mut services: EmissionServicesV1<'service>,
            completed: CheckedExecutionReturnV1,
            child: CheckedChildEmissionProgressV1,
            progress: &mut EmissionSubtreeProgressV1,
        ) -> Result<OwnedFunctionFrameV1<'source, 'service>, ProductionSemanticKirErrorV1> {
            self.owner.frame.lowering.admit_child_progress_v1(&self.pending.request, &child, &mut services)?;
            let mut frame = self.owner.attach(services)?;
            frame.prepared.frame.lowering.with_emission_budget_v1(|this, budget| {
                progress.record_child(&self.pending.request, child, &mut this.private_arrays.work, budget)
            })?;
            frame.prepared.frame.resume_defined_call_v1(self.pending, completed, None)?;
            Ok(frame)
        }
    }

    impl CompletedFunctionFrameV1<'_> {
        pub(super) fn assemble(
            self,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<LoweredFunctionResultV1, ProductionSemanticKirErrorV1> {
            let Self {
                output,
                plan_storage,
                availability,
            } = self;
            let outcome =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| output.assemble(budget)));
            match outcome {
                Ok(Ok(output)) => {
                    plan_storage.finish(budget)?;
                    availability.release(budget)?;
                    Ok(output)
                }
                Ok(Err(error)) => {
                    plan_storage.abandon(budget);
                    let _ = availability.release(budget);
                    Err(error)
                }
                Err(payload) => {
                    plan_storage.abandon(budget);
                    let _ = availability.release(budget);
                    std::panic::resume_unwind(payload)
                }
            }
        }
    }

    #[cfg(test)]
    impl<'source> ExecutionAvailabilityLeaseV1<'source> {
        pub(super) fn test_begin(
            references: Option<&'source SourceReferenceEmissionV29<'source, 'source>>,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<Self, ProductionSemanticKirErrorV1> {
            Self::begin(references, budget)
        }

        pub(super) fn test_fields(&self) -> (usize, usize, usize) {
            (self.entry, self.required, self.owned)
        }

        pub(super) fn test_capture(
            &mut self,
            budget: &ArgumentBudgetV1<'_>,
        ) -> Result<(), ProductionSemanticKirErrorV1> {
            self.capture_cursor_storage(budget)
        }

        pub(super) fn test_reserve_consumption<R>(
            &mut self,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<(), ProductionSemanticKirErrorV1> {
            self.reserve_consumption::<R>(budget)
        }

        pub(super) fn test_permits_release(&self, budget: &ArgumentBudgetV1<'_>) -> bool {
            self.permits_release(budget)
        }

        // The independent header tests construct only an empty accounting lease,
        // never a cursor. Production has no counterpart to this test-only method.
        pub(super) fn test_release(
            self,
            budget: &mut ArgumentBudgetV1<'_>,
        ) -> Result<(), ProductionSemanticKirErrorV1> {
            CompletedExecutionAvailabilityV1 { lease: self }.release(budget)
        }
    }
}

use execution_availability_owner_v1::{
    CompletedExecutionAvailabilityV1, CompletedFunctionFrameV1, OwnedExecutionAvailabilityV1,
    OwnedFunctionFrameV1, SuspendedFunctionFrameV1, SuspendedCallFunctionFrameV1, execution_availability_lease_headers_v1,
};
#[cfg(test)]
use execution_availability_owner_v1::{
    ExecutionAvailabilityLeaseV1, execution_availability_consumption_headers_v1,
};

// Final-package compile-fail probes use the actual private API. The cfgs are
// supplied only to this crate by cargo rustc, never to its dependencies.
// UI-BEGIN direct
#[allow(unexpected_cfgs)]
#[cfg(fe2o3_cursor_escape_direct_v1)]
fn scoped_cursor_cannot_escape_directly<'a>(
    instances: &ExecutionInstancesV29<'a>,
    instance: ProductionCallInstanceIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ExecutionAvailabilityV29<'a>, ProductionSemanticKirErrorV1> {
    with_source_reference_availability_and_identity_v1(
        instances,
        instance,
        None,
        None,
        budget,
        |cursor, _| Ok(cursor),
    )
}
// UI-END direct

// UI-BEGIN nested
#[allow(unexpected_cfgs)]
#[cfg(fe2o3_cursor_escape_nested_v1)]
fn scoped_cursor_cannot_escape_inside_an_output<'a>(
    instances: &ExecutionInstancesV29<'a>,
    instance: ProductionCallInstanceIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(u8, ExecutionAvailabilityV29<'a>), ProductionSemanticKirErrorV1> {
    with_source_reference_availability_and_identity_v1(
        instances,
        instance,
        None,
        None,
        budget,
        |cursor, _| Ok((7, cursor)),
    )
}
// UI-END nested

// UI-BEGIN buffer_direct
#[allow(unexpected_cfgs)]
#[cfg(fe2o3_cursor_buffer_escape_direct_v1)]
fn scoped_cursor_cannot_export_its_index<'a>(
    instances: &ExecutionInstancesV29<'a>,
    instance: ProductionCallInstanceIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<UnitLocalSourceIndexV1>, ProductionSemanticKirErrorV1> {
    with_source_reference_availability_and_identity_v1(
        instances,
        instance,
        None,
        None,
        budget,
        |cursor, _| Ok(cursor.index),
    )
}
// UI-END buffer_direct

// UI-BEGIN buffer_nested
#[allow(unexpected_cfgs)]
#[cfg(fe2o3_cursor_buffer_escape_nested_v1)]
fn scoped_cursor_cannot_export_a_nested_cfg_buffer<'a>(
    instances: &ExecutionInstancesV29<'a>,
    instance: ProductionCallInstanceIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(u8, Vec<Option<ExecutionCfgLeafV29>>), ProductionSemanticKirErrorV1> {
    with_source_reference_availability_and_identity_v1(
        instances,
        instance,
        None,
        None,
        budget,
        |cursor, _| Ok((7, cursor.cfg.leaves)),
    )
}
// UI-END buffer_nested
