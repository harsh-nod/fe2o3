type AggregateSourceRequirementV30 = fe2o3_pliron::CanonicalRankedSourceRequirementV18;

// The first full source/native conjunction supplies every role, not an opcode
// census chosen by this transport. Only checked private replacement retires one.
struct AggregateSourceRolesV30 {
    owner: usize,
    next_stage: usize,
    rows: Vec<Option<AggregateSourceRequirementV30>>,
    initial: usize,
    retired: usize,
}

fn aggregate_role_headers_v30() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        AggregateSourceRolesV30,
        Vec<Option<AggregateSourceRequirementV30>>,
        Vec<AggregateUniqueCoordinateV30<AggregateOperationV30>>,
        Result<AggregateSourceRolesV30, ProductionAggregateSourceErrorV30>,
        SourceOwnedResultV18<AggregateSourceRolesV30>,
        Result<(), ProductionSourceNativeLifecycleErrorV18>,
        Result<(), ProductionAggregateSourceErrorV30>,
        &'a AggregateSourceStageV30<'a>,
        &'a fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a fe2o3_pliron::PendingCanonicalMixedMemoryPoliciesV26<'a, 'a>,
        &'a mut [bool],
        &'a mut Vec<Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>>,
        &'a mut ArgumentBudgetV1<'a>,
        Option<fe2o3_kernel_analysis::CanonicalKirAggregateOperationTransportV30>,
        Result<
            Option<fe2o3_kernel_analysis::CanonicalKirAggregateOperationTransportV30>,
            fe2o3_kernel_analysis::CanonicalKirAggregateSsaErrorV18,
        >,
        [usize; 12],
        [Option<AggregateSourceRequirementV30>; 3],
        Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>,
        Result<
            Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>,
            fe2o3_pliron::CanonicalRankedPolicyFailureV1,
        >,
        Result<
            Option<&'a fe2o3_pliron::CanonicalMixedPipelineReportV26>,
            fe2o3_pliron::CanonicalRankedPolicyFailureV1,
        >,
        Result<
            &'a [fe2o3_pliron::CanonicalRankedSourceObligationV18],
            fe2o3_pliron::CanonicalRankedPolicyFailureV1,
        >,
        Result<usize, fe2o3_pliron::CanonicalRankedPolicyFailureV1>,
        Result<
            &'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
            fe2o3_pliron::CanonicalRankedPolicyFailureV1,
        >,
    );
    argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        std::mem::align_of::<Frame<'_>>(),
        size_of::<
            Result<
                &[fe2o3_pliron::CanonicalRankedSourceObligationV18],
                ProductionSourceNativeLifecycleErrorV18,
            >,
        >(),
    ])
}

impl AggregateStageStateV30 for AggregateSourceRolesV30 {
    fn retained_storage(&self) -> Result<usize, ArgumentResourceV1> {
        argument_product_v1(
            self.rows.capacity(),
            size_of::<Option<AggregateSourceRequirementV30>>(),
        )
    }
}

impl AggregateSourceRolesV30 {
    fn seed_storage(
        output: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Self> {
        let mut rows = source_reference_emission_vec_v29(output.operations().len(), budget)
            .map_err(source_argument_error_v18)?;
        budget.charge_work(output.operations().len())?;
        rows.resize(output.operations().len(), None);
        Ok(Self {
            owner: std::ptr::from_ref(output.owner()) as usize,
            next_stage: 0,
            rows,
            initial: 0,
            retired: 0,
        })
    }

    fn fill_initial(
        &mut self,
        native: &ProductionMixedMemoryCheckedNativePoliciesV26<'_, '_>,
        original: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSourceNativeLifecycleErrorV18> {
        native.check_source_subject_v26(original, optimized, budget)?;
        let output = optimized.output_inventory(budget)?;
        if self.next_stage != 0
            || self.owner != std::ptr::from_ref(output.owner()) as usize
            || self.rows.len() != output.operations().len()
            || self.initial != 0
            || self.retired != 0
        {
            return original
                .source
                .missing("aggregate source role seed owner or reuse")
                .map_err(Into::into);
        }
        for role in native.source_obligations_v30(original, optimized, budget)? {
            budget
                .charge_work(3)
                .map_err(ProductionSourceOwnedViewErrorV18::from)?;
            let index = aggregate_operation_index_v30(output, role.coordinate(), budget)?;
            if self.rows[index].replace(role.requirement()).is_some() {
                return original
                    .source
                    .missing("aggregate source role seed repeats")
                    .map_err(Into::into);
            }
            self.initial = self
                .initial
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)
                .map_err(ProductionSourceOwnedViewErrorV18::from)?;
        }
        self.next_stage = 1;
        Ok(())
    }

    fn advance(
        mut self,
        stage: &AggregateSourceStageV30<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionAggregateSourceErrorV30> {
        use ProductionSourceOwnedViewErrorV18::Binding;
        stage.check(budget)?;
        if self.owner != std::ptr::from_ref(stage.input.owner()) as usize
            || self.next_stage != stage.ordinal
            || self.rows.len() != stage.input.operations().len()
        {
            return Err(Binding("aggregate source role stage order or owner").into());
        }
        let mut next = source_reference_emission_vec_v29(stage.output.operations().len(), budget)
            .map_err(source_argument_error_v18)?;
        budget.charge_work(stage.output.operations().len())?;
        next.resize(stage.output.operations().len(), None);
        let mut transferred = 0usize;
        match stage.relation {
            AggregateSourceStageRelationV30::Scalar(pair) => {
                let mut mapped =
                    aggregate_coordinate_table_v30(stage.input.operations().len(), budget)?;
                for row in pair.rows().operations {
                    budget.charge_work(3)?;
                    let fe2o3_kernel_ir::CanonicalKirOperationOriginV1::Retained(input) =
                        row.origin
                    else {
                        continue;
                    };
                    let input = aggregate_operation_index_v30(stage.input, input, budget)?;
                    aggregate_record_coordinate_v30(&mut mapped[input], row.output);
                }
                for (input, role) in self.rows.iter().enumerate() {
                    budget.charge_work(3)?;
                    let Some(requirement) = *role else { continue };
                    let AggregateUniqueCoordinateV30::One(output) = mapped[input] else {
                        return Err(Binding(
                            "aggregate scalar source role is missing or duplicated",
                        )
                        .into());
                    };
                    let output = aggregate_operation_index_v30(stage.output, output, budget)?;
                    if next[output].replace(requirement).is_some() {
                        return Err(
                            Binding("aggregate scalar source role has repeated output").into()
                        );
                    }
                    transferred = transferred
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                }
                let credit = argument_product_v1(
                    mapped.capacity(),
                    size_of::<AggregateUniqueCoordinateV30<AggregateOperationV30>>(),
                )?;
                drop(mapped);
                budget.release_storage(credit)?;
            }
            AggregateSourceStageRelationV30::Aggregate(_) => {
                let index = stage
                    .aggregate_index
                    .ok_or(Binding("aggregate source role index absent"))?;
                for (input, role) in self.rows.iter().enumerate() {
                    budget.charge_work(4)?;
                    let Some(requirement) = *role else { continue };
                    let transport = index.operation(input, budget).map_err(|error| {
                        ProductionAggregateSourceErrorV30::Optimization(
                            fe2o3_kernel_opt::OwnedAggregateFixedpointErrorV18::Aggregate(
                                fe2o3_kernel_opt::OwnedAggregateSsaErrorV18::Check(error),
                            ),
                        )
                    })?;
                    if let Some(retained) = transport.filter(|row| row.is_retained()) {
                        let output =
                            aggregate_operation_index_v30(stage.output, retained.output(), budget)?;
                        if next[output].replace(requirement).is_some() {
                            return Err(Binding(
                                "aggregate retained source role has repeated output",
                            )
                            .into());
                        }
                        transferred = transferred
                            .checked_add(1)
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                    } else {
                        // The independently checked witness, not this opcode
                        // guard, proves that the private allocation is closed.
                        let kind = &stage.input.operations()[input].operation.kind;
                        let expected = if transport.is_some() {
                            matches!(
                                kind,
                                OperationKind::Storage(
                                    fe2o3_kernel_ir::StorageOperationV1::ReadValue { .. }
                                )
                            )
                        } else {
                            matches!(
                                kind,
                                OperationKind::Alloca { .. }
                                    | OperationKind::Storage(
                                        fe2o3_kernel_ir::StorageOperationV1::Project { .. }
                                            | fe2o3_kernel_ir::StorageOperationV1::WriteValue { .. }
                                    )
                            )
                        };
                        if requirement != AggregateSourceRequirementV30::Memory || !expected {
                            return Err(Binding(
                                "aggregate replacement retired a non-private-memory source role",
                            )
                            .into());
                        }
                        self.retired = self
                            .retired
                            .checked_add(1)
                            .ok_or(ArgumentResourceV1::Arithmetic)?;
                    }
                }
            }
        }
        if transferred.checked_add(self.retired) != Some(self.initial) {
            return Err(Binding("aggregate source role complete conservation").into());
        }
        let old = self.retained_storage()?;
        drop(self.rows);
        budget.release_storage(old)?;
        self.rows = next;
        self.owner = std::ptr::from_ref(stage.output.owner()) as usize;
        self.next_stage = self
            .next_stage
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        Ok(self)
    }

    fn join_final(
        &self,
        chain: &fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        output: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
        native: &fe2o3_pliron::PendingCanonicalMixedMemoryPoliciesV26<'_, '_>,
        seen: &mut [bool],
        histories: &mut Vec<Option<fe2o3_pliron::CanonicalRankedPolicyHistoryV1>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionAggregateSourceErrorV30> {
        use ProductionAggregateSourceErrorV30 as E;
        use ProductionSourceOwnedViewErrorV18::Binding;
        let pending = |error| E::Native(ProductionSourceNativeLifecycleErrorV18::Pending(error));
        budget.charge_work(10)?;
        if !std::ptr::eq(output.owner(), chain.owner())
            || self.owner != std::ptr::from_ref(chain.owner()) as usize
            || self.next_stage != argument_product_v1(chain.rounds().len(), 2)?
            || self.rows.len() != output.operations().len()
            || seen.len() != self.rows.len()
            || !histories.is_empty()
            || histories.capacity() < output.functions().len()
            || !std::ptr::eq(native.owner(budget).map_err(pending)?, output.owner())
            || native.function_count(budget).map_err(pending)? != output.functions().len()
        {
            return Err(Binding("aggregate final source role owner or census").into());
        }
        let obligations = native.obligations(budget).map_err(pending)?;
        if obligations.len().checked_add(self.retired) != Some(self.initial) {
            return Err(Binding("aggregate final source role count").into());
        }
        budget.charge_work(seen.len())?;
        seen.fill(false);
        for role in obligations {
            budget.charge_work(4)?;
            let index = aggregate_operation_index_v30(output, role.coordinate(), budget)?;
            if seen[index] || self.rows[index] != Some(role.requirement()) {
                return Err(Binding("aggregate final source role changed or repeated").into());
            }
            seen[index] = true;
        }
        for (role, seen) in self.rows.iter().zip(seen) {
            budget.charge_work(2)?;
            if role.is_some() != *seen {
                return Err(Binding("aggregate final source role omitted").into());
            }
        }
        for (ordinal, function) in output.functions().iter().enumerate() {
            budget.charge_work(4)?;
            let report = native.report(ordinal, budget).map_err(pending)?;
            let history = native.history(ordinal, budget).map_err(pending)?;
            match (function.function.body.is_some(), report, history) {
                (true, Some(report), Some(_))
                    if report.reports().is_clean() && report.paired_stage_count() == 9 =>
                {
                    ()
                }
                (false, None, None) => (),
                _ => {
                    return Err(Binding(
                        "aggregate final function lacks all nine clean native stages",
                    )
                    .into());
                }
            }
            histories.push(history);
        }
        Ok(())
    }
}
