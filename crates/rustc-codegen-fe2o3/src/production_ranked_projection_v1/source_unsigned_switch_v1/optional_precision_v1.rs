//! A local precision miss publishes nothing; mandatory failures remain errors.
use super::*;

#[cfg(test)]
#[path = "optional_precision_observation_tests.rs"]
mod observation_tests;

#[derive(Clone, Copy, Debug)]
pub(in super::super) enum OperandSideV1 {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug)]
struct SiteV1 {
    definition: ScalarAssignmentSiteV1,
    use_block: usize,
    operand: OperandSideV1,
}

pub(in super::super) struct OptionalSourceComparisonV1 {
    exhausted: bool,
    extent_writes: Vec<usize>,
    site: Option<SiteV1>,
}

impl TotalUnsignedIndexProjectorV1<'_, '_, '_> {
    pub(in super::super) fn optional_read_index_v1(
        mut self,
        operand: &SemanticOperandV1,
        use_block: usize,
        use_statement: usize,
    ) -> Result<Option<ProductionRankedValueV1>, ProductionRankedProjectionErrorV1> {
        if self.optional_source.is_some()
            || !matches!(self.roots, TotalUnsignedIndexRootsV1::Invocation { .. })
        {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "optional read index lacks its unique invocation transaction",
            ));
        }
        self.assertion_proofs.charge(1)?;
        if self.unsigned_bits(operand.ty()) != Some(64)
            || !self.source_comparison_operand_type_v1(operand)
        {
            return Ok(None);
        }
        // Reuse the existing transaction's node/emission charges. This scope
        // cannot bind runtime arguments or extents and consumes its local cache.
        self.assertion_proofs.charge(
            std::mem::size_of::<OptionalSourceComparisonV1>()
                .div_ceil(std::mem::size_of::<usize>())
                + self.states.capacity()
                + 3,
        )?;
        let checkpoint = (self.operations.len(), *self.next_value, *self.next_argument);
        self.optional_source = Some(OptionalSourceComparisonV1 {
            exhausted: false,
            extent_writes: Vec::new(),
            site: None,
        });
        let result = self.resolve_operand(operand, use_block, use_statement);
        let transaction = self.optional_source.take().expect("owned read transaction");
        let accepted = matches!(&result, Ok(Some(value)) if value.invocation_dependent)
            && !transaction.exhausted;
        if !accepted {
            // Rollback was prepaid by emission. Shared proof work and sound
            // source-only caches remain charged, including on a fatal error.
            self.operations.truncate(checkpoint.0);
            *self.next_value = checkpoint.1;
            *self.next_argument = checkpoint.2;
        }
        Ok(result?.filter(|_| accepted).map(|value| value.ranked))
    }

    pub(in super::super) fn optional_source_unsigned_switches_v1(
        mut self,
        uses: &ProjectedGlobalSemanticUsesV1,
        allocations: &[Option<AllocationContractV1>],
        extent_arguments: &mut [Option<u32>],
    ) -> Result<Vec<Option<ProjectedDeterministicSwitchV1>>, ProductionRankedProjectionErrorV1>
    {
        if self.optional_source.is_some()
            || !matches!(self.roots, TotalUnsignedIndexRootsV1::Invocation { .. })
        {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "optional source comparison lacks its unique invocation transaction",
            ));
        }
        if uses.source_unsigned_comparisons.is_empty() {
            return self.source_unsigned_switches_v1(uses, allocations, extent_arguments);
        }
        // Charge checkpoint storage and the final disposal of the local cache.
        // Source-only dominance/range caches remain with the shared proof owner.
        self.assertion_proofs.charge(
            std::mem::size_of::<OptionalSourceComparisonV1>()
                .div_ceil(std::mem::size_of::<usize>())
                + self.states.capacity()
                + 3,
        )?;
        let checkpoint = (self.operations.len(), *self.next_value, *self.next_argument);
        self.optional_source = Some(OptionalSourceComparisonV1 {
            exhausted: false,
            extent_writes: Vec::new(),
            site: None,
        });
        let result = self.source_unsigned_switches_v1(uses, allocations, extent_arguments);
        let transaction = self
            .optional_source
            .take()
            .expect("owned source transaction");
        if result.is_err() || transaction.exhausted {
            // Emission and journal insertion prepay rollback, even when the
            // shared work limit caused the error. No proof work is refunded.
            self.operations.truncate(checkpoint.0);
            *self.next_value = checkpoint.1;
            *self.next_argument = checkpoint.2;
            for &origin in transaction.extent_writes.iter().rev() {
                extent_arguments[origin] = None;
            }
        }
        let switches = result?;
        if transaction.exhausted {
            if let Some(site) = transaction.site {
                let source = self.function.blocks()[site.definition.block].statements()
                    [site.definition.statement]
                    .source();
                eprintln!(
                    "fe2o3 optional source-comparison precision miss: function={:?} definition_bb={} statement={} use_bb={} operand={:?} node_work={} limit={} source={:?}",
                    self.function.identity(),
                    site.definition.block,
                    site.definition.statement,
                    site.use_block,
                    site.operand,
                    self.node_work,
                    MAX_PURE_UNIFORM_INDEX_NODES_V1,
                    source,
                );
            }
            return Ok(Vec::new());
        }
        // Consuming self prevents cached provisional ranked IDs from escaping
        // either outcome. Previously published direct predicates are untouched.
        Ok(switches)
    }

    pub(in super::super) fn charge_optional_source_work_v1(
        &mut self,
        amount: usize,
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        if self.optional_source.is_some() {
            self.assertion_proofs.charge(amount)?;
        }
        Ok(())
    }

    pub(in super::super) fn optional_source_exhausted_v1(&self) -> bool {
        self.optional_source
            .as_ref()
            .is_some_and(|state| state.exhausted)
    }

    pub(in super::super) fn mark_optional_source_exhausted_v1(&mut self) -> bool {
        if let Some(state) = &mut self.optional_source {
            state.exhausted = true;
            true
        } else {
            false
        }
    }

    pub(in super::super) fn optional_source_site_v1(
        &mut self,
        definition: ScalarAssignmentSiteV1,
        use_block: usize,
        operand: OperandSideV1,
    ) {
        if let Some(state) = &mut self.optional_source {
            state.site = Some(SiteV1 {
                definition,
                use_block,
                operand,
            });
        }
    }

    pub(in super::super) fn journal_optional_extent_v1(
        &mut self,
        origin: usize,
        arguments: &[Option<u32>],
    ) -> Result<(), ProductionRankedProjectionErrorV1> {
        let Some(state) = &mut self.optional_source else {
            return Ok(());
        };
        let slot = arguments
            .get(origin)
            .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                "a volatile load slice extent origin outside the semantic local table",
            ))?;
        if slot.is_some() {
            return Ok(());
        }
        // Each journal entry owns one None -> Some transition, including its
        // rollback. A repeated extent is already Some and needs no new entry.
        self.assertion_proofs.charge(2)?;
        let journal = &mut state.extent_writes;
        if journal.len() == journal.capacity() {
            let old = journal.capacity();
            let requested = journal.len().checked_add(1).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "optional comparison journal size overflow",
                ),
            )?;
            let peak = old
                .checked_add(requested)
                .filter(|peak| *peak <= MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1)
                .ok_or(ProductionRankedProjectionErrorV1::Unsupported(
                    "optional comparison journal exceeds the state limit",
                ))?;
            self.assertion_proofs.charge(peak)?;
            journal.try_reserve_exact(1).map_err(|_| {
                ProductionRankedProjectionErrorV1::Unsupported(
                    "optional comparison journal storage cannot be reserved",
                )
            })?;
            if old
                .checked_add(journal.capacity())
                .is_none_or(|actual| actual > MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1)
            {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "optional comparison journal exceeds the state limit",
                ));
            }
            self.assertion_proofs
                .charge(journal.capacity().saturating_sub(requested))?;
        }
        journal.push(origin);
        Ok(())
    }
}
