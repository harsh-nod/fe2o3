struct CpcTransportV1<'a, 's, 'm> {
    origins: &'a CpcOriginsV1<'s, 'm>,
    assertions: CsaTransportV1,
    operations: Vec<ProductionCanonicalPrivateOperationV1>,
    storage: usize,
}
impl<'a, 's, 'm> CpcTransportV1<'a, 's, 'm> {
    fn allocate(
        origins: &'a CpcOriginsV1<'s, 'm>,
        assertions: CsaTransportV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<Self> {
        // CsaTransport's header is already paid; it is embedded, not duplicated.
        let header = std::mem::size_of::<Self>()
            .checked_sub(std::mem::size_of::<CsaTransportV1>())
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        budget.reserve_storage(header)?;
        let operations = cs_vec_v1(origins.operations.len(), budget)?;
        let storage = argument_sum_v1(&[header, assertions.storage, cs_extent_v1(&operations)?])?;
        Ok(Self {
            origins,
            assertions,
            operations,
            storage,
        })
    }
    fn original(
        origins: &'a CpcOriginsV1<'s, 'm>,
        coverage: &Coverage<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<Self> {
        let assertions = CsaTransportV1::original(origins.source, coverage, budget)?;
        let mut result = Self::allocate(origins, assertions, budget)?;
        for row in &origins.operations {
            cs_push_v1(
                &mut result.operations,
                ProductionCanonicalPrivateOperationV1 {
                    original: row.coordinate,
                    kind: row.kind,
                    current: Some(row.coordinate),
                    removed: None,
                },
                budget,
            )?;
        }
        Ok(result)
    }
    fn check_current(
        &self,
        inventory: &CanonicalKirInventoryV1<'_>,
        lineage: &CsLineageV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<()> {
        self.origins.check(self.origins.source, budget)?;
        budget.charge_work(2)?;
        if self.operations.len() != self.origins.operations.len()
            || lineage.operations.len() != inventory.operations().len()
        {
            return Err(cs_invalid_v1("complete private operation state"));
        }
        let mut seen = cpc_index_v1(inventory.operations().len(), budget)?;
        for (ordinal, (row, original)) in self
            .operations
            .iter()
            .zip(&self.origins.operations)
            .enumerate()
        {
            budget.charge_work(4)?;
            if row.original != original.coordinate
                || row.kind != original.kind
                || row.current.is_some() == row.removed.is_some()
            {
                return Err(cs_invalid_v1("private original/disposition invariant"));
            }
            let original_block =
                cs_block_v1(self.origins.source.inventory, row.original.block, budget)?;
            if let Some(current) = row.current {
                let index = cs_operation_v1(inventory, current, budget)?;
                if cpc_kind_v1(&inventory.operations()[index].operation.kind) != Some(row.kind)
                    || lineage.operations[index]
                        != ProductionCanonicalScalarOperationOriginV1::Original(row.original)
                    || seen[index].replace(ordinal).is_some()
                {
                    return Err(cs_invalid_v1(
                        "unique current private operation/source join",
                    ));
                }
            } else if lineage.block_controls[original_block].reachable {
                return Err(cs_invalid_v1(
                    "removed private operation has live source control",
                ));
            }
        }
        for (ordinal, operation) in inventory.operations().iter().enumerate() {
            budget.charge_work(2)?;
            if cpc_kind_v1(&operation.operation.kind).is_some() != seen[ordinal].is_some() {
                return Err(cs_invalid_v1(
                    "complete inverse private/call operation coverage",
                ));
            }
        }
        // This temporary index is no longer borrowed before its actual-capacity refund.
        let storage = argument_sum_v1(&[std::mem::size_of_val(&seen), cs_extent_v1(&seen)?])?;
        drop(seen);
        budget.release_storage(storage)?;
        Ok(())
    }
}

fn cpc_pair_slots_v1(
    input: &CanonicalKirInventoryV1<'_>,
    output: &CanonicalKirInventoryV1<'_>,
    rows: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
    before: CsOperationV1,
    after: CsOperationV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    let before = &input.operations()[cs_operation_v1(input, before, budget)?];
    let after = &output.operations()[cs_operation_v1(output, after, budget)?];
    budget.charge_work(3)?;
    if cpc_kind_v1(&before.operation.kind) != cpc_kind_v1(&after.operation.kind)
        || before.operands.len() != after.operands.len()
        || before.results.len() != after.results.len()
    {
        return Err(cs_invalid_v1("private/call typed slot extents"));
    }
    // The actual transition checker already proved exact attributes/effects and
    // value substitutions. Here keep every ordered source use and result anchor.
    for (left, right) in before.operands.clone().zip(after.operands.clone()) {
        budget.charge_work(3)?;
        let row = rows
            .uses
            .get(right)
            .ok_or_else(|| cs_invalid_v1("call operand transport roster"))?;
        if row.input != input.uses()[left].coordinate
            || row.output != output.uses()[right].coordinate
        {
            return Err(cs_invalid_v1("ordered private/call operand occurrence"));
        }
    }
    for (left, right) in before.results.clone().zip(after.results.clone()) {
        budget.charge_work(3)?;
        let row = rows
            .definitions
            .get(left)
            .ok_or_else(|| cs_invalid_v1("call result transport roster"))?;
        if row.input != input.definitions()[left].coordinate {
            return Err(cs_invalid_v1("private/call original result ordinal"));
        }
        let outputs = cs_range_v1(row.outputs, rows.definition_outputs.len(), budget)?;
        budget.charge_work(outputs.len())?;
        if rows.definition_outputs[outputs]
            .iter()
            .filter(|row| {
                row.output == output.definitions()[right].coordinate
                    && row.kind == fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1::Retained
            })
            .count()
            != 1
        {
            return Err(cs_invalid_v1("private/call exact retained result anchor"));
        }
    }
    Ok(())
}

impl<'a, 's, 'm> CsPairObserverV1 for CpcTransportV1<'a, 's, 'm> {
    type Next = Self;
    #[allow(clippy::too_many_arguments)]
    fn pair(
        &self,
        input: &CanonicalKirInventoryV1<'_>,
        output: &CanonicalKirInventoryV1<'_>,
        control: &fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1<'_, '_, '_>,
        rows: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
        previous: &CsLineageV1,
        next: &CsLineageV1,
        round: u16,
        integer: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<Self> {
        self.check_current(input, previous, budget)?;
        let assertions = self.assertions.pair(
            input, output, control, rows, previous, next, round, integer, budget,
        )?;
        let mut result = Self::allocate(self.origins, assertions, budget)?;
        let mut inverse = cpc_index_v1(input.operations().len(), budget)?;
        for row in rows.operations {
            budget.charge_work(2)?;
            let fe2o3_kernel_ir::CanonicalKirOperationOriginV1::Retained(original) = row.origin
            else {
                continue;
            };
            let input_index = cs_operation_v1(input, original, budget)?;
            if cpc_kind_v1(&input.operations()[input_index].operation.kind).is_none() {
                continue;
            }
            let output_index = cs_operation_v1(output, row.output, budget)?;
            if inverse[input_index].replace(output_index).is_some() {
                return Err(cpc_error_v1(
                    original,
                    ProductionCanonicalScalarAssertionStepV1 { round, integer },
                    "private operation duplicated by adjacent relation",
                ));
            }
        }
        let step = ProductionCanonicalScalarAssertionStepV1 { round, integer };
        for old in &self.operations {
            budget.charge_work(3)?;
            let mut updated = *old;
            if let Some(current) = old.current {
                let input_index = cs_operation_v1(input, current, budget)?;
                if let Some(index) = inverse[input_index] {
                    let after = output.operations()[index].coordinate;
                    cpc_pair_slots_v1(input, output, rows, current, after, budget)?;
                    updated.current = Some(after);
                } else {
                    let block = control.block(current.block, budget)?;
                    if block.reachable {
                        return Err(cpc_error_v1(
                            old.original,
                            step,
                            "live private/call operation omitted",
                        ));
                    }
                    updated.current = None;
                    updated.removed = Some(step);
                }
            } else {
                let event = old
                    .removed
                    .ok_or_else(|| cpc_error_v1(old.original, step, "omission without event"))?;
                if event.round > round || (event.round == round && !event.integer && integer) {
                    return Err(cpc_error_v1(
                        old.original,
                        step,
                        "future private removal event",
                    ));
                }
            }
            cs_push_v1(&mut result.operations, updated, budget)?;
        }
        result.check_current(output, next, budget)?;
        let storage = argument_sum_v1(&[std::mem::size_of_val(&inverse), cs_extent_v1(&inverse)?])?;
        drop(inverse);
        budget.release_storage(storage)?;
        Ok(result)
    }
    fn retained(next: &Self) -> usize {
        next.storage
    }
    fn replace(&mut self, next: Self) -> usize {
        let previous = std::mem::replace(self, next);
        let retired = previous.storage;
        drop(previous);
        retired
    }
}

#[cfg(test)]
pub(super) fn read_test_private_call_state_v1(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    budget: &mut ArgumentBudgetV1<'_>,
    fault: u8,
) -> CsResultV1<()> {
    // Component mutations of unauthenticated private state, never an admission API.
    cpc_scope_v1(budget, |budget| {
        owner
            .original
            .with_checked_canonical_ranked_source_v1(budget, |view, budget| {
                Ok(cpc_with_source_v1(
                    view,
                    budget,
                    |source, coverage, _, budget| {
                        let mut origins = CpcOriginsV1::derive(source, budget)?;
                        match fault {
                            0 => {
                                origins.aliases.pop().expect("nonempty aliases");
                            }
                            1 => origins.aliases[0].span = usize::MAX,
                            2 => origins.calls[0].callee = usize::MAX,
                            3 => origins.calls[0].root = SemanticFunctionIdV1::from_index(u32::MAX),
                            _ => {}
                        }
                        origins.check(source, budget)?;
                        let mut transport = CpcTransportV1::original(&origins, coverage, budget)?;
                        let row = &mut transport.operations[0];
                        match fault {
                            4 => {
                                row.current = None;
                                row.removed = Some(ProductionCanonicalScalarAssertionStepV1 {
                                    round: 0,
                                    integer: true,
                                });
                            }
                            5 => {
                                row.removed = Some(ProductionCanonicalScalarAssertionStepV1 {
                                    round: 0,
                                    integer: true,
                                })
                            }
                            6 => row.current = None,
                            7 => {
                                let first = transport.operations[0].current;
                                transport.operations[1].current = first;
                            }
                            8 => row.kind = ProductionCanonicalPrivateOperationKindV1::Address,
                            _ => panic!("expected an earlier source-alias refusal"),
                        }
                        let lineage = cs_lineage_with_observer_v1(
                            owner,
                            source.inventory,
                            &mut transport,
                            budget,
                        )?;
                        drop(lineage);
                        Ok(())
                    },
                ))
            })?
    })
}

#[cfg(test)]
pub(super) fn read_test_private_call_foreign_source_v1(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    donor: &ProductionCanonicalScalarFixedPointOwnerV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    cpc_scope_v1(budget, |budget| {
        owner
            .original
            .with_checked_canonical_ranked_source_v1(budget, |view, budget| {
                Ok(cpc_with_source_v1(view, budget, |source, _, _, budget| {
                    let origins = CpcOriginsV1::derive(source, budget)?;
                    donor
                        .original
                        .with_checked_canonical_ranked_source_v1(budget, |other, budget| {
                            Ok(origins.check(other.source, budget))
                        })?
                }))
            })?
    })
}

#[cfg(test)]
pub(super) fn read_test_private_call_slot_v1(
    owner: &ProductionCanonicalScalarFixedPointOwnerV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    cpc_scope_v1(budget, |budget| {
        let (input, receipt) =
            CanonicalKirInventoryV1::derive(owner.original.executable(), budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let step = owner.history.rounds()[0].integer();
        let (output, receipt) = CanonicalKirInventoryV1::derive(step.owner(), budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let rows = step.occurrences().candidate();
        budget.charge_work(input.operations().len())?;
        let before = input
            .operations()
            .iter()
            .find(|r| matches!(r.operation.kind, OperationKind::Store { .. }))
            .expect("real private Store fixture");
        budget.charge_work(rows.operations.len())?;
        let after = rows
            .operations
            .iter()
            .find(|r| {
                r.origin
                    == fe2o3_kernel_ir::CanonicalKirOperationOriginV1::Retained(before.coordinate)
            })
            .expect("real retained Store relation")
            .output;
        cpc_pair_slots_v1(&input, &output, rows, before.coordinate, after, budget)?;
        let mut uses = cpc_vector_v1(rows.uses.len(), budget)?;
        for row in rows.uses {
            cs_push_v1(&mut uses, *row, budget)?;
        }
        let after = &output.operations()[cs_operation_v1(&output, after, budget)?];
        assert_eq!(before.operands.len(), 2);
        uses[after.operands.start].input = input.uses()[before.operands.start + 1].coordinate;
        let changed = fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1 {
            uses: &uses,
            ..rows
        };
        cpc_pair_slots_v1(
            &input,
            &output,
            changed,
            before.coordinate,
            after.coordinate,
            budget,
        )
    })
}
