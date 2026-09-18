use crate::{
    ProductionExecutionSourceInputV29, ProductionScopeCallKindV29,
    ProductionScopeCallableCandidateV29, ProductionScopeEventKindV29,
};

// Source agreement is not producer authentication. The backend retains its
// originating receipt; this private view never grants launch authority.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped root orchestration is pending")
)]
struct ExecutionLifecycleSourceV29<'a> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    input: ProductionExecutionSourceInputV29<'a>,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped root orchestration is pending")
)]
impl<'a> ExecutionLifecycleSourceV29<'a> {
    fn new(
        owner: &'a ProductionSemanticSsaOwnerV1,
        launch: &crate::ProductionSourceLaunchRosterV1,
        input: ProductionExecutionSourceInputV29<'a>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        crate::with_checked_execution_source_v29(owner, launch, input, budget, |_, _| Ok(()))
            .map_err(|error| match error {
                crate::ProductionContextRootErrorV29::Resource(error) => error.into(),
                _ => execution_lifecycle_error_v29(),
            })?;
        Ok(Self {
            owner,
            input,
            ledger: budget.work_ledger_identity_v1(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeferredLifecycleSourceV29 {
    Issuance { root: usize },
    Derive { event: usize },
    Return { event: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DeferredLifecycleKindV29 {
    Issue {
        result: SemanticExecutionIdentityV29,
    },
    Derive {
        context: SemanticExecutionIdentityV29,
        result: SemanticExecutionIdentityV29,
    },
    End {
        workgroup: SemanticExecutionIdentityV29,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DeferredLifecycleEventV29 {
    source: DeferredLifecycleSourceV29,
    block: SemanticBlockIdV1,
    original_block: BlockId,
    original_gap: u32,
    kind: DeferredLifecycleKindV29,
}

// Gaps precede helper expansion. They must be joined to the retained terminator
// spans and relocated before these events become canonical operations.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Lifecycle insertion remains pending")
)]
struct PendingLifecycleEventsV29 {
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    source: ExecutionCallSourceV29,
    instance: ProductionCallInstanceIdV1,
    function: SemanticFunctionIdV1,
    placement: SemanticEmissionPlacementV1,
    rows: Vec<DeferredLifecycleEventV29>,
    retained_storage: usize,
}

impl PendingLifecycleEventsV29 {
    fn check_identity(
        &self,
        instances: &ExecutionInstancesV29<'_>,
        instance: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.ledger != budget.work_ledger_identity_v1() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        if self.instance != instance
            || self.source != ExecutionCallSourceV29::from_instances(instances, budget)?
            || !instances
                .instance(instance)
                .is_some_and(|row| row.function() == self.function)
        {
            return Err(execution_lifecycle_error_v29());
        }
        Ok(())
    }
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped root orchestration is pending")
)]
struct ExecutionLifecycleProducerV29<'a> {
    source: &'a ExecutionLifecycleSourceV29<'a>,
    function: &'a SemanticFunctionDeclV1,
    ssa: &'a ProductionSemanticSsaFunctionPlanV1,
    pending: PendingLifecycleEventsV29,
    provider: bool,
    workgroup: Option<SemanticExecutionBindingV29>,
    expected_rows: usize,
    finished: bool,
}

fn execution_lifecycle_error_v29() -> ProductionSemanticKirErrorV1 {
    unsupported(
        0,
        None,
        None,
        "execution lifecycle differs from its retained source instance",
    )
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped root orchestration is pending")
)]
impl<'a> ExecutionLifecycleProducerV29<'a> {
    fn new(
        source: &'a ExecutionLifecycleSourceV29<'a>,
        instances: &ExecutionInstancesV29<'a>,
        instance: ProductionCallInstanceIdV1,
        placement: SemanticEmissionPlacementV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let floor = budget.storage();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::build(source, instances, instance, placement, budget)
        }));
        match result {
            Ok(Ok(producer)) => Ok(producer),
            Ok(Err(error)) => {
                budget.release_storage(budget.storage() - floor)?;
                Err(error)
            }
            Err(payload) => {
                let _ = budget.release_storage(budget.storage() - floor);
                std::panic::resume_unwind(payload)
            }
        }
    }

    fn build(
        source: &'a ExecutionLifecycleSourceV29<'a>,
        instances: &ExecutionInstancesV29<'a>,
        instance: ProductionCallInstanceIdV1,
        placement: SemanticEmissionPlacementV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        if source.ledger != budget.work_ledger_identity_v1()
            || !std::ptr::eq(source.owner, instances.owner())
        {
            return Err(execution_lifecycle_error_v29());
        }
        let row = instances
            .instance(instance)
            .ok_or_else(execution_lifecycle_error_v29)?;
        let provider = matches!(
            source.input.classes.get(row.function().index() as usize),
            Some(ProductionScopeCallableCandidateV29::Provider { .. })
        );
        let mut producer = Self {
            source,
            function: row.declaration(),
            ssa: row.ssa(),
            pending: PendingLifecycleEventsV29 {
                ledger: source.ledger,
                source: ExecutionCallSourceV29::from_instances(instances, budget)?,
                instance,
                function: row.function(),
                placement,
                rows: Vec::new(),
                retained_storage: 0,
            },
            provider,
            workgroup: None,
            expected_rows: 0,
            finished: false,
        };
        let mut derives = 0;
        for index in 0..producer.function.blocks().len() {
            budget.charge_work(1)?;
            let block = SemanticBlockIdV1::from_index(
                u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            if producer
                .ssa
                .plan()
                .is_reachable(SsaBlockIdV1::new(block.index()))
                && let Some(event) = producer.expected_event(block, budget)?
            {
                producer.expected_rows = argument_sum_v1(&[producer.expected_rows, 1])?;
                derives += usize::from(matches!(event, DeferredLifecycleSourceV29::Derive { .. }));
            }
        }
        // The authenticated with_workgroup provider derives one scope, with
        // potentially several mutually exclusive normal return sites.
        if derives != usize::from(provider) {
            return Err(execution_lifecycle_error_v29());
        }
        let floor = budget.storage();
        producer.pending.rows = emission_vec_v1(producer.expected_rows, budget)?;
        producer.pending.retained_storage = budget.storage() - floor;
        Ok(producer)
    }

    fn expected_event(
        &self,
        block: SemanticBlockIdV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<Option<DeferredLifecycleSourceV29>, ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        let body = self
            .function
            .blocks()
            .get(block.index() as usize)
            .ok_or_else(execution_lifecycle_error_v29)?;
        let expected = match body.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => {
                let declaration = self
                    .source
                    .owner
                    .source_semantic()
                    .callables()
                    .get(call.callee().index() as usize)
                    .ok_or_else(execution_lifecycle_error_v29)?;
                match declaration {
                    SemanticCallableDeclV1::CompilerIntrinsic {
                        operation:
                            SemanticCompilerIntrinsicOperationV1::Execution(
                                SemanticExecutionOperationV29::ContextIssue { .. },
                            ),
                        ..
                    } => {
                        budget.charge_work(self.source.input.roots.len())?;
                        let root = self
                            .source
                            .input
                            .roots
                            .iter()
                            .position(|root| {
                                root.root == self.pending.function && root.issuance.block == block
                            })
                            .ok_or_else(execution_lifecycle_error_v29)?;
                        if self.pending.function != self.pending.source.root {
                            return Err(execution_lifecycle_error_v29());
                        }
                        return Ok(Some(DeferredLifecycleSourceV29::Issuance { root }));
                    }
                    SemanticCallableDeclV1::CompilerIntrinsic {
                        operation:
                            SemanticCompilerIntrinsicOperationV1::Execution(
                                SemanticExecutionOperationV29::WorkgroupDerive { .. },
                            ),
                        ..
                    } if self.provider => ProductionScopeEventKindV29::Call {
                        callee: call.callee(),
                        kind: ProductionScopeCallKindV29::Derive,
                    },
                    _ => return Ok(None),
                }
            }
            SemanticTerminatorKindV1::Return if self.provider => {
                ProductionScopeEventKindV29::Return
            }
            _ => return Ok(None),
        };
        budget.charge_work(self.source.input.events.len())?;
        let event = self
            .source
            .input
            .events
            .iter()
            .position(|event| {
                event.function == self.pending.function
                    && event.block == block
                    && event.statement_count == body.statements().len()
                    && event.kind == expected
            })
            .ok_or_else(execution_lifecycle_error_v29)?;
        Ok(Some(
            if matches!(expected, ProductionScopeEventKindV29::Return) {
                DeferredLifecycleSourceV29::Return { event }
            } else {
                DeferredLifecycleSourceV29::Derive { event }
            },
        ))
    }

    fn check_lowering(
        &self,
        lowering: &mut SemanticFunctionLoweringV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let cursor = lowering
            .execution
            .as_ref()
            .ok_or_else(execution_lifecycle_error_v29)?;
        let budget = lowering
            .emission_work
            .as_deref_mut()
            .ok_or(ArgumentResourceV1::Accounting)?;
        self.check_instance(cursor, budget)?;
        if lowering.emission_placement != self.pending.placement
            || lowering.semantic_function != self.pending.function
            || !std::ptr::eq(lowering.function, self.function)
            || !std::ptr::eq(
                lowering.callables,
                self.source.owner.source_semantic().callables(),
            )
        {
            return Err(execution_lifecycle_error_v29());
        }
        Ok(())
    }

    fn prepare_event(
        &self,
        lowering: &mut SemanticFunctionLoweringV1<'_>,
        block: SemanticBlockIdV1,
    ) -> Result<DeferredLifecycleSourceV29, ProductionSemanticKirErrorV1> {
        self.check_lowering(lowering)?;
        let cursor = lowering
            .execution
            .as_ref()
            .ok_or_else(execution_lifecycle_error_v29)?;
        let budget = lowering
            .emission_work
            .as_deref_mut()
            .ok_or(ArgumentResourceV1::Accounting)?;
        budget.charge_work(self.pending.rows.len())?;
        if cursor.block != Some(SsaBlockIdV1::new(block.index()))
            || self.pending.rows.len() >= self.expected_rows
            || self.pending.rows.iter().any(|row| row.block == block)
        {
            return Err(execution_lifecycle_error_v29());
        }
        self.expected_event(block, budget)?
            .ok_or_else(execution_lifecycle_error_v29)
    }

    fn record(
        &mut self,
        source: DeferredLifecycleSourceV29,
        block: SemanticBlockIdV1,
        gap: usize,
        kind: DeferredLifecycleKindV29,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.pending.rows.len() >= self.pending.rows.capacity() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let row = DeferredLifecycleEventV29 {
            source,
            block,
            original_block: self.pending.placement.block(block.index())?,
            original_gap: u32::try_from(gap).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            kind,
        };
        self.pending.rows.push(row);
        Ok(())
    }
}

impl ExecutionLifecycleConsumerV29 for ExecutionLifecycleProducerV29<'_> {
    fn check_instance(
        &self,
        execution: &ExecutionAvailabilityV29<'_>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        execution.check_ledger(budget)?;
        if self.pending.ledger != budget.work_ledger_identity_v1() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.charge_work(8)?;
        execution.check_source(self.function, self.ssa)?;
        if self.finished
            || execution.source != self.pending.source
            || execution.instance != self.pending.instance
            || execution.function_id != self.pending.function
        {
            return Err(execution_lifecycle_error_v29());
        }
        Ok(())
    }

    fn require_catalog_entry(
        &self,
        callable: fe2o3_mir_model::semantic_mir_v1::SemanticCallableIdV1,
        declaration: &SemanticCallableDeclV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.pending.ledger != budget.work_ledger_identity_v1() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.charge_work(8)?;
        if self.finished
            || !self
                .source
                .owner
                .source_semantic()
                .callables()
                .get(callable.index() as usize)
                .is_some_and(|actual| std::ptr::eq(actual, declaration))
        {
            return Err(execution_lifecycle_error_v29());
        }
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation_identity,
            operation,
        } = declaration
        else {
            return Err(execution_lifecycle_error_v29());
        };
        let valid = match operation {
            SemanticCompilerIntrinsicOperationV1::Execution(
                SemanticExecutionOperationV29::ContextIssue { context },
            ) => {
                budget.charge_work(self.source.input.roots.len())?;
                self.source.input.roots.iter().any(|root| {
                    root.issuer == callable
                        && root.issuer_identity == binding.identity()
                        && root.context_type == *context
                })
            }
            SemanticCompilerIntrinsicOperationV1::Execution(
                SemanticExecutionOperationV29::WorkgroupDerive { context, workgroup },
            ) => matches!(self.source.input.classes.get(callable.index() as usize),
                Some(ProductionScopeCallableCandidateV29::Derive {
                    binding: found, operation, context: input, workgroup: output
                }) if *found == binding.identity() && operation == operation_identity
                    && input == context && output == workgroup),
            _ => false,
        };
        if !valid {
            return Err(execution_lifecycle_error_v29());
        }
        Ok(())
    }

    fn produce(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_>,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operation: SemanticExecutionOperationV29,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let source = self.prepare_event(lowering, block)?;
        let actual = self
            .function
            .blocks()
            .get(block.index() as usize)
            .ok_or_else(execution_lifecycle_error_v29)?;
        if !matches!(actual.terminator().kind(), SemanticTerminatorKindV1::Call(found)
            if std::ptr::eq(found, call))
        {
            return Err(execution_lifecycle_error_v29());
        }
        let declaration = lowering
            .callables
            .get(call.callee().index() as usize)
            .ok_or_else(execution_lifecycle_error_v29)?;
        self.require_catalog_entry(
            call.callee(),
            declaration,
            lowering
                .emission_work
                .as_deref_mut()
                .ok_or(ArgumentResourceV1::Accounting)?,
        )?;
        if !matches!(declaration, SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::Execution(found), ..
        } if *found == operation)
        {
            return Err(execution_lifecycle_error_v29());
        }
        let destination = call
            .destination()
            .ok_or_else(execution_lifecycle_error_v29)?;
        if !destination.place().projections().is_empty()
            || matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_))
        {
            return Err(execution_lifecycle_error_v29());
        }
        let occurrence = ProductionCallOccurrenceV1 {
            caller: self.pending.instance,
            block,
        };
        let (binding, kind) = match (source, operation) {
            (
                DeferredLifecycleSourceV29::Issuance { root },
                SemanticExecutionOperationV29::ContextIssue { context },
            ) => {
                let retained = self.source.input.roots[root];
                if !call.arguments().is_empty()
                    || destination.place().ty() != context
                    || retained.issuer != call.callee()
                    || retained.context_type != context
                    || retained.issuance.destination != destination.place().local()
                    || retained.issuance.target != destination.edge().target()
                    || retained.issuance.unwind != call.unwind()
                {
                    return Err(execution_lifecycle_error_v29());
                }
                let binding = SemanticExecutionBindingV29::context(
                    lowering.types,
                    context,
                    occurrence,
                    ValueId(lowering.next_value),
                )
                .map_err(|_| execution_lifecycle_error_v29())?;
                let kind = DeferredLifecycleKindV29::Issue {
                    result: binding.identity,
                };
                (binding, kind)
            }
            (
                DeferredLifecycleSourceV29::Derive { .. },
                SemanticExecutionOperationV29::WorkgroupDerive { context, workgroup },
            ) => {
                if call.arguments().len() != 1
                    || destination.place().ty() != workgroup
                    || self.workgroup.is_some()
                {
                    return Err(execution_lifecycle_error_v29());
                }
                let borrowed = lowering.lower_source_operand_v29(
                    block,
                    None,
                    Some(ExecutionOperandV29::CallArgument(0)),
                    &call.arguments()[0],
                    operations,
                )?;
                let SemanticValueBindingV1::ExecutionBorrow(borrowed) = borrowed else {
                    return Err(execution_lifecycle_error_v29());
                };
                if borrowed.kind != SemanticBorrowKindV1::Mutable
                    || borrowed.borrowed.semantic_type() != context
                {
                    return Err(execution_lifecycle_error_v29());
                }
                borrowed
                    .check_type(lowering.types, call.arguments()[0].ty())
                    .map_err(|_| execution_lifecycle_error_v29())?;
                let binding = SemanticExecutionBindingV29::workgroup(
                    lowering.types,
                    workgroup,
                    occurrence,
                    ValueId(lowering.next_value),
                    &borrowed.borrowed,
                )
                .map_err(|_| execution_lifecycle_error_v29())?;
                let kind = DeferredLifecycleKindV29::Derive {
                    context: binding.context,
                    result: binding.identity,
                };
                (binding, kind)
            }
            _ => return Err(execution_lifecycle_error_v29()),
        };
        let next_value = lowering
            .next_value
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        self.record(source, block, operations.len(), kind)?;
        lowering.next_value = next_value;
        if matches!(kind, DeferredLifecycleKindV29::Derive { .. }) {
            self.workgroup = Some(binding.clone());
        }
        Ok(SemanticValueBindingV1::Execution(binding))
    }

    fn normal_return(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_>,
        block: SemanticBlockIdV1,
        operations: &[Operation],
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_lowering(lowering)?;
        if !self.provider {
            return Ok(());
        }
        let source = self.prepare_event(lowering, block)?;
        if !matches!(source, DeferredLifecycleSourceV29::Return { .. }) {
            return Err(execution_lifecycle_error_v29());
        }
        let workgroup = self
            .workgroup
            .as_ref()
            .ok_or_else(execution_lifecycle_error_v29)?
            .identity;
        self.record(
            source,
            block,
            operations.len(),
            DeferredLifecycleKindV29::End { workgroup },
        )
    }

    fn finish(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_>,
    ) -> Result<PendingLifecycleEventsV29, ProductionSemanticKirErrorV1> {
        self.check_lowering(lowering)?;
        let budget = lowering
            .emission_work
            .as_deref_mut()
            .ok_or(ArgumentResourceV1::Accounting)?;
        budget.charge_work(self.pending.rows.len())?;
        if self.pending.rows.len() != self.expected_rows
            || self.provider != self.workgroup.is_some()
        {
            return Err(execution_lifecycle_error_v29());
        }
        enforce_limit(
            ProductionSemanticKirResourceV1::Operations,
            argument_sum_v1(&[lowering.emitted_operations, self.pending.rows.len()])?,
            lowering.max_operations,
        )?;
        for row in &self.pending.rows {
            let value = match row.kind {
                DeferredLifecycleKindV29::Issue { result }
                | DeferredLifecycleKindV29::Derive { result, .. } => result,
                DeferredLifecycleKindV29::End { workgroup } => workgroup,
            };
            if value.producer.caller != self.pending.instance
                || value.value.0 >= lowering.next_value
            {
                return Err(execution_lifecycle_error_v29());
            }
        }
        self.finished = true;
        Ok(PendingLifecycleEventsV29 {
            ledger: self.pending.ledger,
            source: self.pending.source,
            instance: self.pending.instance,
            function: self.pending.function,
            placement: self.pending.placement,
            rows: std::mem::take(&mut self.pending.rows),
            retained_storage: std::mem::take(&mut self.pending.retained_storage),
        })
    }
}
