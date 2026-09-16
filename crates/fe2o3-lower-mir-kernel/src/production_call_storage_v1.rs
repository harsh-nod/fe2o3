// Requested logical payload, excluding allocator excess and immutable inputs.
// The construction ledger is shared across functions and root merges.
struct CallReturnBufferV1 {
    rows: Vec<SemanticKirCallReturnV1>,
    requested: usize,
}

impl CallReturnBufferV1 {
    fn empty() -> Self {
        Self {
            rows: Vec::new(),
            requested: 0,
        }
    }

    fn for_function(
        function: &SemanticFunctionDeclV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        budget.charge_work(function.blocks().len())?;
        let count = function
            .blocks()
            .iter()
            .filter(|block| {
                matches!(
                    block.terminator().kind(),
                    SemanticTerminatorKindV1::Call(_) | SemanticTerminatorKindV1::Return
                )
            })
            .count();
        let bytes = Self::bytes(count)?;
        budget.reserve_storage(bytes)?;
        match argument_vec_v1(count) {
            Ok(rows) => Ok(Self {
                rows,
                requested: count,
            }),
            Err(error) => {
                budget.release_storage(bytes)?;
                Err(error.into())
            }
        }
    }

    fn from_box(rows: Box<[SemanticKirCallReturnV1]>) -> Self {
        Self {
            requested: rows.len(),
            rows: rows.into_vec(),
        }
    }

    fn bytes(count: usize) -> Result<usize, ArgumentResourceV1> {
        argument_product_v1(count, std::mem::size_of::<SemanticKirCallReturnV1>())
    }

    fn append(
        &mut self,
        mut source: Self,
        limit: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let floor = budget.storage();
        let old = self.requested;
        let released = Self::bytes(source.requested)?;
        let result = (|| {
            let total = argument_sum_v1(&[self.rows.len(), source.rows.len()])?;
            enforce_limit(ProductionSemanticKirResourceV1::Blocks, total, limit)?;
            budget.charge_work(argument_sum_v1(&[source.rows.len(), 1])?)?;
            if total > self.requested {
                let next = argument_product_v1(self.requested.max(1), 2)?
                    .min(limit)
                    .max(total);
                budget.charge_work(self.rows.len())?;
                // Old and new requested backing storage can coexist during growth.
                budget.reserve_storage(Self::bytes(next)?)?;
                self.rows
                    .try_reserve_exact(next - self.rows.len())
                    .map_err(|_| ArgumentResourceV1::Allocation)?;
                budget.release_storage(Self::bytes(self.requested)?)?;
                self.requested = next;
            }
            self.rows.append(&mut source.rows);
            Ok(())
        })();
        drop(source);
        let retained = floor
            .checked_sub(released)
            .and_then(|floor| floor.checked_add(Self::bytes(self.requested - old).ok()?))
            .ok_or(ArgumentResourceV1::Accounting)?;
        budget.release_storage(
            budget
                .storage()
                .checked_sub(retained)
                .ok_or(ArgumentResourceV1::Accounting)?,
        )?;
        result
    }

    fn into_box(
        self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Box<[SemanticKirCallReturnV1]>, ProductionSemanticKirErrorV1> {
        let floor = budget.storage();
        let old = Self::bytes(self.requested)?;
        let result = (|| {
            let retained = Self::bytes(self.rows.len())?;
            budget.charge_work(self.rows.len())?;
            budget.reserve_storage(retained)?;
            // Quota denial and Vec growth are recoverable. As with other owned
            // correspondence slices, allocator failure during Box compaction
            // follows the standard library's process-level allocation policy.
            let rows = self.rows.into_boxed_slice();
            budget.release_storage(old)?;
            Ok(rows)
        })();
        if result.is_err() {
            let retained = floor
                .checked_sub(old)
                .ok_or(ArgumentResourceV1::Accounting)?;
            budget.release_storage(
                budget
                    .storage()
                    .checked_sub(retained)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )?;
        }
        result
    }

    fn order_blocks(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ArgumentResourceV1> {
        budget.charge_work(argument_product_v1(self.rows.len(), 128)?)?;
        sort_correspondence_keys_v1(&mut self.rows, 31, &|row| {
            u64::from(row.semantic_block.index())
        });
        Ok(())
    }

    fn order(
        &mut self,
        functions: &BTreeMap<(SemanticFunctionIdV1, SemanticFunctionIdV1), usize>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if functions.len() > u32::MAX as usize {
            return Err(ArgumentResourceV1::Arithmetic.into());
        }
        // Pinned BTreeMap nodes contain at most 11 keys, compared linearly.
        // Allow two identity comparisons per key and extra traversal work.
        let lookup = argument_product_v1(
            argument_sum_v1(&[functions.len().checked_ilog2().unwrap_or(0) as usize, 2])?,
            24,
        )?;
        budget.charge_work(argument_product_v1(
            self.rows.len(),
            argument_sum_v1(&[lookup, 196])?,
        )?)?;
        let floor = budget.storage();
        let result = (|| {
            budget.reserve_storage(argument_product_v1(
                self.rows.len(),
                std::mem::size_of::<(u64, SemanticKirCallReturnV1)>(),
            )?)?;
            let mut indexed = argument_vec_v1(self.rows.len())?;
            for row in &self.rows {
                let ordinal = functions
                    .get(&(row.correspondence_owner, row.semantic_function))
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                let key = ((*ordinal as u64) << 32) | u64::from(row.semantic_block.index());
                indexed.push((key, *row));
            }
            sort_correspondence_keys_v1(&mut indexed, 63, &|row| row.0);
            for (row, (_, sorted)) in self.rows.iter_mut().zip(indexed) {
                *row = sorted;
            }
            Ok(())
        })();
        budget.release_storage(budget.storage() - floor)?;
        result
    }
}
