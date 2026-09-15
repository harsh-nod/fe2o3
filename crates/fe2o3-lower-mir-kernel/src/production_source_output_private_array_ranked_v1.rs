impl ProductionSourceOutputOccurrencesV1<'_, '_> {
    /// Checks only allocation/index agreement for one executable retained write.
    ///
    /// Source role and slot facts remain authenticated at N; C checks the actual
    /// O Store and its operand ancestry. The supplied ranked name/blocks are
    /// inert candidate data. Coordinates select operations from those exact
    /// blocks, and the Access operands must name the supplied View/Index results.
    /// This does not validate ranked SSA, mandatory reports, Store value,
    /// control flow, total effects, formal discharge, or final owner authority.
    ///
    /// No additional index or payload is allocated. All added work uses the live
    /// caller canonical ledger; borrowed ranked storage keeps its existing
    /// ranked-phase accounting. Legacy N query limits are not changed.
    #[allow(
        clippy::too_many_arguments,
        reason = "Keep source role and three exact ranked operation coordinates explicit without a new proof wrapper"
    )]
    pub fn check_ranked_private_array_write_allocation_index(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        site: fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1,
        role: fe2o3_pliron::ProductionSemanticSsaOperandRoleV1,
        function_name: &str,
        blocks: &[fe2o3_pliron::ProductionRankedBlockV1],
        access_coordinate: (u32, u32),
        view_coordinate: (u32, u32),
        index_coordinate: (u32, u32),
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<(), ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        budget.charge_work(4).map_err(Error::Resource)?;
        let declaration = self
            .source
            .semantic_ssa
            .source_semantic()
            .functions()
            .get(owner.index() as usize)
            .and_then(SemanticFunctionDeclV1::kernel_entry)
            .ok_or(Error::Invalid("array allocation/index source entry absent"))?;
        if !private_array_equal_bytes_v1(
            declaration.export_symbol().as_bytes(),
            function_name.as_bytes(),
            &mut PrivateArrayQueryWorkV1 { budget },
        )
        .map_err(Error::PrivateArray)?
        {
            return Err(Error::Invalid(
                "array allocation/index ranked function changed",
            ));
        }
        // The entry owns the exported name. C independently authenticates its
        // selected body, including an admitted transparent wrapper's body.
        let placement = self.private_array_write(owner, function, site, role, budget)?;
        budget.charge_work(2).map_err(Error::Resource)?;
        let ProductionSourceOutputPrivateArrayAccessV1::Retained {
            index: offset,
            executable: true,
            ..
        } = placement
        else {
            return Err(Error::Invalid(
                "array allocation/index leaf requires an executable retained write",
            ));
        };
        let fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement { block, statement } =
            site
        else {
            return Err(Error::Invalid(
                "array allocation/index source statement absent",
            ));
        };
        let key =
            source_output_array_key_v1(owner, function, block.get(), statement, role, budget)?;
        let ordinal = assert_origin_find_v1(&self.private_arrays, budget, |row, budget| {
            budget.charge_work(6)?;
            Ok(row.key.cmp(&key))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("array allocation/index source row absent"))?;
        budget.charge_work(2).map_err(Error::Resource)?;
        let row = &self.private_arrays[ordinal];
        let slot = self
            .source
            .correspondence
            .private_arrays
            .slots
            .get(row.original_slot)
            .ok_or(Error::Invalid("array allocation/index source slot absent"))?;
        budget.charge_work(3).map_err(Error::Resource)?;
        let access = blocks
            .get(access_coordinate.0 as usize)
            .and_then(|block| block.operations().get(access_coordinate.1 as usize))
            .ok_or(Error::Invalid(
                "array allocation/index ranked operation absent",
            ))?;
        let (view_value, index_value) = private_array_ranked_write_values_v1(
            access,
            PrivateArrayAccessV1::Write,
            dialect_kernel::AccessKindAttr::Write,
            None,
            &mut PrivateArrayQueryWorkV1 { budget },
        )
        .map_err(Error::PrivateArray)?
        .ok_or(Error::Invalid(
            "array allocation/index ranked access changed",
        ))?;
        budget.charge_work(3).map_err(Error::Resource)?;
        let view = blocks
            .get(view_coordinate.0 as usize)
            .and_then(|block| block.operations().get(view_coordinate.1 as usize))
            .ok_or(Error::Invalid(
                "array allocation/index ranked operation absent",
            ))?;
        budget.charge_work(1).map_err(Error::Resource)?;
        if !matches!(view, ProductionRankedOperationV1::ViewInSpace { result, .. }
            if view_value == ProductionRankedValueV1::Local(*result))
            || !private_array_ranked_view_matches_v1(
                slot,
                view,
                &mut PrivateArrayQueryWorkV1 { budget },
            )
            .map_err(Error::PrivateArray)?
        {
            return Err(Error::Invalid("array allocation/index ranked view changed"));
        }
        budget.charge_work(3).map_err(Error::Resource)?;
        let index = blocks
            .get(index_coordinate.0 as usize)
            .and_then(|block| block.operations().get(index_coordinate.1 as usize))
            .ok_or(Error::Invalid(
                "array allocation/index ranked operation absent",
            ))?;
        budget.charge_work(1).map_err(Error::Resource)?;
        if !matches!(index, ProductionRankedOperationV1::IndexConstant { result, .. }
            if index_value == ProductionRankedValueV1::Local(*result))
            || !private_array_ranked_index_matches_v1(
                offset,
                index,
                &mut PrivateArrayQueryWorkV1 { budget },
            )
            .map_err(Error::PrivateArray)?
        {
            return Err(Error::Invalid(
                "array allocation/index ranked index changed",
            ));
        }
        Ok(())
    }
}
