/// Inert census after checking every source initializer component against O.
/// This is not a receipt, functional proof, or executable admission capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceOutputPrivateArrayInitializerV1 {
    /// The existing source query proved the whole local was promoted.
    ProvenUnretained,
    /// Every retained component has its exact checked O address and value use.
    Checked {
        /// Complete original source component count.
        components: u64,
        /// Actual O Stores whose source effect is checked reachable.
        retained_executable: u64,
        /// Actual O Stores whose source effect is checked unreachable.
        retained_nonexecutable: u64,
        /// Checked unreachable effects with no retained O Store.
        omitted_unreachable: u64,
    },
}

impl ProductionSourceOutputPrivateArrayInitializerV1 {
    /// Numeric observations alone never grant authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl ProductionSourceOutputOccurrencesV1<'_, '_> {
    /// Checks the complete literal initializer through the actual N/B/O owners.
    /// The constructor's checked typed transition transports each Store value;
    /// this query also checks the actual O Store operand, address and source row.
    /// Keep the same caller ledger and all live owner/view receipts reserved,
    /// including the separately accounted B and coordinate-preservation owners.
    /// The returned census is inert and is not an attachment or launch receipt.
    pub fn private_array_initializer(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<ProductionSourceOutputPrivateArrayInitializerV1, ProductionSourceOutputErrorV1>
    {
        use ProductionSourceOutputErrorV1 as Error;
        use ProductionSourceOutputPrivateArrayAccessV1 as Access;
        use ProductionSourceOutputPrivateArrayInitializerV1 as Outcome;
        // Preserve the fixed floor precharge. The source subtotal already
        // includes its sealed graph, assertion-origin and helper receipts.
        budget.charge_work(4).map_err(Error::Resource)?;
        let minimum = self
            .source
            .retained_analysis_storage_v1()
            .checked_add(self.checked_output.storage().retained_storage())
            .and_then(|n| n.checked_add(self.storage.retained_storage()))
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        if budget.storage() < minimum {
            return Err(Error::Resource(AssertOriginResourceV1::Accounting));
        }
        // Validate the source/N grammar, values and whole component census once.
        let Some(components) = self
            .source
            .materialized_private_array_initializer_count(owner, function, site, budget)
            .map_err(Error::PrivateArray)?
        else {
            return Ok(Outcome::ProvenUnretained);
        };
        budget.charge_work(1).map_err(Error::Resource)?;
        let fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement { block, statement } =
            site
        else {
            return Err(Error::Invalid("initializer source statement is absent"));
        };
        let mut key = source_output_array_key_v1(
            owner,
            function,
            block.get(),
            statement,
            fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::Destination,
            0,
            budget,
        )?;
        // Count conversion, six prefix fields, and the original-row borrow.
        budget.charge_work(8).map_err(Error::Resource)?;
        let count = usize::try_from(components)
            .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        let prefix = [
            key[0] as usize,
            key[1] as usize,
            key[2] as usize,
            key[3] as usize,
            key[4] as usize,
            key[5] as usize,
        ];
        let original = &self.source.correspondence.private_arrays;
        let row_prefix = |row: &SourceOutputArrayRowV1| {
            [
                row.key[0] as usize,
                row.key[1] as usize,
                row.key[2] as usize,
                row.key[3] as usize,
                row.key[4] as usize,
                row.key[5] as usize,
            ]
        };
        let mut work = PrivateArrayQueryWorkV1 { budget };
        let start =
            private_array_partition_v1(&self.private_arrays, row_prefix, prefix, false, &mut work)
                .map_err(Error::PrivateArray)?;
        let end =
            private_array_partition_v1(&self.private_arrays, row_prefix, prefix, true, &mut work)
                .map_err(Error::PrivateArray)?;
        let budget = &mut *work.budget;
        budget.charge_work(3).map_err(Error::Resource)?;
        let rows = self
            .private_arrays
            .get(start..end)
            .ok_or(Error::Invalid("initializer output component range changed"))?;
        if rows.len() != count {
            return Err(Error::Invalid(
                "initializer output component census changed",
            ));
        }
        let mut selected_slot = None;
        let mut retained_executable = 0u64;
        let mut retained_nonexecutable = 0u64;
        let mut omitted_unreachable = 0u64;
        for (component, row) in rows.iter().enumerate() {
            // Two original indexes, ordinal conversion, and initializer tag.
            budget.charge_work(4).map_err(Error::Resource)?;
            let effect = original
                .effects
                .get(row.original_effect)
                .ok_or(Error::Invalid("initializer original effect is absent"))?;
            let slot = original
                .slots
                .get(row.original_slot)
                .ok_or(Error::Invalid("initializer original slot is absent"))?;
            let component = u32::try_from(component)
                .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            let PrivateArrayIndexV1::InitializerElement {
                component: actual_component,
                ..
            } = effect.original_index
            else {
                return Err(Error::Invalid("initializer original component tag changed"));
            };
            budget.charge_work(1).map_err(Error::Resource)?;
            key[6] = component;
            let actual_key = source_output_array_key_v1(
                effect.owner,
                effect.function,
                effect.semantic_block,
                effect.semantic_statement,
                effect.role,
                actual_component,
                budget,
            )?;
            // Two seven-field keys and seven component/slot/access predicates.
            budget.charge_work(21).map_err(Error::Resource)?;
            if row.key != key
                || actual_key != key
                || actual_component != component
                || effect.access != PrivateArrayAccessV1::Write
                || slot.owner != owner
                || slot.function != function
                || slot.local != effect.local
                || slot.length != components
                || selected_slot.is_some_and(|prior| prior != row.original_slot)
            {
                return Err(Error::Invalid(
                    "initializer original occurrence identity changed",
                ));
            }
            selected_slot = Some(row.original_slot);
            budget.charge_work(1).map_err(Error::Resource)?;
            let access = match row.placement {
                SourceOutputArrayPlacementV1::Unsupported => {
                    return Err(Error::Invalid(
                        "initializer output placement is unsupported",
                    ));
                }
                SourceOutputArrayPlacementV1::OmittedUnreachable => Access::OmittedUnreachable,
                SourceOutputArrayPlacementV1::Retained(anchors) => self
                    .private_array_retained_write_v1(slot, anchors, u64::from(component), budget)?,
            };
            // Outcome tag/reachability, checked increment, and counter update.
            budget.charge_work(4).map_err(Error::Resource)?;
            let counter = match access {
                Access::Retained {
                    executable: true, ..
                } => &mut retained_executable,
                Access::Retained {
                    executable: false, ..
                } => &mut retained_nonexecutable,
                Access::OmittedUnreachable => &mut omitted_unreachable,
                Access::ProvenUnretained => {
                    return Err(Error::Invalid(
                        "initializer retained component became unretained",
                    ));
                }
            };
            *counter = counter
                .checked_add(1)
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        }
        // Two checked sums, full census equality, and the inert result.
        budget.charge_work(4).map_err(Error::Resource)?;
        if retained_executable
            .checked_add(retained_nonexecutable)
            .and_then(|n| n.checked_add(omitted_unreachable))
            != Some(components)
        {
            return Err(Error::Invalid("initializer output outcome census changed"));
        }
        Ok(Outcome::Checked {
            components,
            retained_executable,
            retained_nonexecutable,
            omitted_unreachable,
        })
    }
}
