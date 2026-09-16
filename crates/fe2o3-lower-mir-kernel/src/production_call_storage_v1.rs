// Requested logical payload, excluding allocator excess and immutable inputs.
// Site headers and typed components share one construction ledger.
struct CallPayloadBufferV1<T> {
    rows: Vec<T>,
    requested: usize,
}

impl<T> CallPayloadBufferV1<T> {
    fn empty() -> Self {
        Self {
            rows: Vec::new(),
            requested: 0,
        }
    }

    fn new(
        count: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
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

    fn from_box(rows: Box<[T]>) -> Self {
        Self {
            requested: rows.len(),
            rows: rows.into_vec(),
        }
    }

    fn bytes(count: usize) -> Result<usize, ArgumentResourceV1> {
        argument_product_v1(count, std::mem::size_of::<T>())
    }

    fn push(&mut self, row: T) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.rows.len() >= self.requested {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        self.rows.push(row);
        Ok(())
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
    ) -> Result<Box<[T]>, ProductionSemanticKirErrorV1> {
        let floor = budget.storage();
        let old = Self::bytes(self.requested)?;
        let result = (|| {
            let retained = Self::bytes(self.rows.len())?;
            budget.charge_work(self.rows.len())?;
            budget.reserve_storage(retained)?;
            // Box compaction follows the standard library's allocation-failure policy.
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
}

type CallReturnPayloadV1 = (Box<[SemanticKirCallReturnV1]>, Box<[CallResultComponentV1]>);

struct CallReturnBufferV1 {
    sites: CallPayloadBufferV1<SemanticKirCallReturnV1>,
    components: CallPayloadBufferV1<CallResultComponentV1>,
}

impl CallReturnBufferV1 {
    fn empty() -> Self {
        Self {
            sites: CallPayloadBufferV1::empty(),
            components: CallPayloadBufferV1::empty(),
        }
    }

    fn for_function(
        function: &SemanticFunctionDeclV1,
        callables: &[SemanticCallableDeclV1],
        signatures: &BTreeMap<SemanticFunctionIdV1, LoweredFunctionSignatureV1>,
        return_width: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        budget.charge_work(function.blocks().len())?;
        let lookup = argument_product_v1(
            signatures.len().checked_ilog2().unwrap_or(0) as usize + 2,
            24,
        )?;
        let (mut count, mut components) = (0_usize, 0_usize);
        for block in function.blocks() {
            let width = match block.terminator().kind() {
                SemanticTerminatorKindV1::Return => return_width,
                SemanticTerminatorKindV1::Call(call) => {
                    if let Some(SemanticCallableDeclV1::Defined { function }) =
                        callables.get(call.callee().index() as usize)
                    {
                        budget.charge_work(lookup)?;
                        // Unreachable targets need not belong to the retained helper closure.
                        signatures
                            .get(function)
                            .map_or(0, |signature| signature.result_types.len())
                    } else {
                        0
                    }
                }
                _ => continue,
            };
            count = argument_sum_v1(&[count, 1])?;
            components = argument_sum_v1(&[components, width])?;
        }
        if components > u32::MAX as usize {
            return Err(ArgumentResourceV1::Arithmetic.into());
        }
        budget.charge_work(argument_product_v1(components, 32)?)?;
        let floor = budget.storage();
        let result = (|| {
            Ok(Self {
                sites: CallPayloadBufferV1::new(count, budget)?,
                components: CallPayloadBufferV1::new(components, budget)?,
            })
        })();
        if result.is_err() {
            budget.release_storage(budget.storage() - floor)?;
        }
        result
    }

    fn from_box(
        sites: Box<[SemanticKirCallReturnV1]>,
        components: Box<[CallResultComponentV1]>,
    ) -> Self {
        Self {
            sites: CallPayloadBufferV1::from_box(sites),
            components: CallPayloadBufferV1::from_box(components),
        }
    }

    fn bytes(sites: usize, components: usize) -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            CallPayloadBufferV1::<SemanticKirCallReturnV1>::bytes(sites)?,
            CallPayloadBufferV1::<CallResultComponentV1>::bytes(components)?,
        ])
    }

    fn requested_bytes(&self) -> Result<usize, ArgumentResourceV1> {
        Self::bytes(self.sites.requested, self.components.requested)
    }

    fn component_span(
        &self,
        first: usize,
    ) -> Result<CallComponentSpanV1, ProductionSemanticKirErrorV1> {
        let count = self
            .components
            .rows
            .len()
            .checked_sub(first)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if count == 0 {
            return Ok(CallComponentSpanV1::EMPTY);
        }
        Ok(CallComponentSpanV1 {
            first: u32::try_from(first).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            count: u32::try_from(count).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        })
    }

    fn append(
        &mut self,
        mut source: Self,
        limit: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let floor = budget.storage();
        let old = self.requested_bytes()?;
        let consumed = source.requested_bytes()?;
        let result = (|| {
            enforce_limit(
                ProductionSemanticKirResourceV1::Blocks,
                argument_sum_v1(&[self.sites.rows.len(), source.sites.rows.len()])?,
                limit,
            )?;
            budget.charge_work(source.sites.rows.len())?;
            let offset = u32::try_from(self.components.rows.len())
                .map_err(|_| ArgumentResourceV1::Arithmetic)?;
            u32::try_from(argument_sum_v1(&[
                self.components.rows.len(),
                source.components.rows.len(),
            ])?)
            .map_err(|_| ArgumentResourceV1::Arithmetic)?;
            for row in &mut source.sites.rows {
                row.rebase_components(offset)?;
            }
            self.components
                .append(source.components, u32::MAX as usize, budget)?;
            let result = self.sites.append(source.sites, limit, budget);
            if result.is_err() {
                self.components.rows.truncate(offset as usize);
            }
            result
        })();
        let retained = floor
            .checked_sub(old)
            .and_then(|n| n.checked_sub(consumed))
            .and_then(|n| n.checked_add(self.requested_bytes().ok()?))
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
    ) -> Result<CallReturnPayloadV1, ProductionSemanticKirErrorV1> {
        let floor = budget.storage();
        let old = self.requested_bytes()?;
        let result = (|| {
            let components = self.components.into_box(budget)?;
            Ok((self.sites.into_box(budget)?, components))
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
        budget.charge_work(argument_product_v1(self.sites.rows.len(), 128)?)?;
        sort_correspondence_keys_v1(&mut self.sites.rows, 31, &|row| {
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
        let lookup = argument_product_v1(
            functions.len().checked_ilog2().unwrap_or(0) as usize + 2,
            24,
        )?;
        budget.charge_work(argument_product_v1(
            self.sites.rows.len(),
            argument_sum_v1(&[lookup, 196])?,
        )?)?;
        let floor = budget.storage();
        let result = (|| {
            budget.reserve_storage(argument_product_v1(
                self.sites.rows.len(),
                std::mem::size_of::<(u64, SemanticKirCallReturnV1)>(),
            )?)?;
            let mut indexed = argument_vec_v1(self.sites.rows.len())?;
            for row in &self.sites.rows {
                let ordinal = functions
                    .get(&(row.correspondence_owner, row.semantic_function))
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                indexed.push((
                    ((*ordinal as u64) << 32) | u64::from(row.semantic_block.index()),
                    *row,
                ));
            }
            sort_correspondence_keys_v1(&mut indexed, 63, &|row| row.0);
            for (row, (_, sorted)) in self.sites.rows.iter_mut().zip(indexed) {
                *row = sorted;
            }
            Ok(())
        })();
        budget.release_storage(budget.storage() - floor)?;
        result
    }
}
