impl UnitLocalCallRowV1 {
    fn source_site_key(&self) -> [u32; 3] {
        [
            self.caller.root.index(),
            self.caller.function.index(),
            self.source_block.index(),
        ]
    }
}

fn sort_unit_local_calls_v1(
    calls: &mut [UnitLocalCallRowV1],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert_origin_sort_v1(calls, budget, |left, right, budget| {
        budget.charge_work(3)?;
        Ok(left.source_site_key().cmp(&right.source_site_key()))
    })
    .map_err(call_index_error_v1)?;
    budget.charge_work(argument_product_v1(calls.len(), 3)?)?;
    if calls
        .windows(2)
        .any(|pair| pair[0].source_site_key() == pair[1].source_site_key())
    {
        return Err(unit_local_mismatch_v1());
    }
    Ok(())
}

/// A live root/call-qualified Unit-local relation for source bounds projection.
/// The callee's private effects remain real effects, not a raw-empty summary.
/// This relation proves no ranked/output/target contract beyond this exact call's
/// lack of caller-visible memory, argument, result or continuation transfers.
/// Copied identities are inert; consumers must use the borrowed result in scope.
pub struct ProductionUnitLocalBoundsNeutralCallV1<'a> {
    row: &'a UnitLocalCallRowV1,
    association: &'a UnitLocalAssociationRowV1,
    source: &'a SemanticDirectCallV1,
    operation: &'a Operation,
}

impl ProductionUnitLocalBoundsNeutralCallV1<'_> {
    /// Original root selecting this call association.
    pub fn root(&self) -> SemanticFunctionIdV1 {
        self.row.caller.root
    }
    /// Original caller function, not a physical function ordinal.
    pub fn caller(&self) -> SemanticFunctionIdV1 {
        self.row.caller.function
    }
    /// Original zero-argument Unit helper under this root.
    pub fn callee(&self) -> SemanticFunctionIdV1 {
        self.association.key.function
    }
    /// Source block containing the exact direct-call terminator.
    pub fn source_block(&self) -> SemanticBlockIdV1 {
        self.row.source_block
    }
    /// Actual stored-order operation coordinate in the borrowed inventory.
    pub fn native_call(&self) -> fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
        self.row.call
    }
    /// Exact source call from the retained semantic owner.
    pub fn source_call(&self) -> &SemanticDirectCallV1 {
        self.source
    }
    /// Exact executable Call, not a reconstructed operation.
    pub fn operation(&self) -> &Operation {
        self.operation
    }
}

impl ProductionPreRankedKirOwnerV1 {
    /// Minimum owner payload required by source-helper queries. This adds an
    /// incoming occurrence capture's separate reservation exactly once; capture
    /// created by materialization is already in the cached retained subtotal.
    /// Inventory and query scratch remain additional. A numeric floor is not
    /// evidence that the caller retained or reserved the actual allocations.
    pub fn unit_local_source_storage_floor_v1(
        &self,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        Ok(argument_sum_v1(&[
            self.retained_analysis_storage_v1(),
            self.helper_memory.capture.preexisting_storage(),
        ])?)
    }
}

impl ProductionUnitLocalSourceV1<'_> {
    /// Joins one actual source call to its sealed root-qualified Unit-local body
    /// and exact executable Call. Missing source sites return None; a substituted
    /// source object or inconsistent native association is an error. No callable
    /// is reclassified as raw-empty and no module or source roster is rescanned.
    /// The result must be consumed inside this source scope, which retains the
    /// original inventory, Work ledger and complete occurrence-capture floor.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1;
    /// use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// use fe2o3_mir_model::semantic_mir_v1::{SemanticFunctionIdV1, SemanticBlockIdV1, SemanticDirectCallV1};
    /// fn escape(owner: &ProductionPreRankedKirOwnerV1,
    ///     inventory: &CanonicalKirInventoryV1<'_>, budget: &mut Budget<'_>,
    ///     root: SemanticFunctionIdV1, caller: SemanticFunctionIdV1,
    ///     block: SemanticBlockIdV1, call: &SemanticDirectCallV1) {
    ///     let mut saved = None;
    ///     owner.with_checked_unit_local_source_v1(inventory, budget, |view, budget| {
    ///         saved = view.bounds_neutral_call_v1(root, caller, block, call, budget)?;
    ///         Ok(())
    ///     }).unwrap();
    ///     drop(saved);
    /// }
    /// ```
    pub fn bounds_neutral_call_v1(
        &self,
        root: SemanticFunctionIdV1,
        caller: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<ProductionUnitLocalBoundsNeutralCallV1<'_>>, ProductionSemanticKirErrorV1>
    {
        budget.charge_work(5)?;
        if self.ledger != budget as *const ArgumentBudgetV1<'_> as usize
            || self.work_ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let key = [root.index(), caller.index(), block.index()];
        let found = assert_origin_find_v1(&self.rows.calls, budget, |row, budget| {
            budget.charge_work(3)?;
            Ok(row.source_site_key().cmp(&key))
        })
        .map_err(call_index_error_v1)?;
        let Some(index) = found else { return Ok(None) };

        budget.charge_work(48)?;
        let mismatch = unit_local_mismatch_v1;
        let row = &self.rows.calls[index];
        let semantic = self.semantic_ssa.source_semantic();
        let source = semantic
            .functions()
            .get(caller.index() as usize)
            .ok_or_else(mismatch)?;
        let source_block = source
            .blocks()
            .get(block.index() as usize)
            .ok_or_else(mismatch)?;
        let SemanticTerminatorKindV1::Call(actual_source) = source_block.terminator().kind() else {
            return Err(mismatch());
        };
        if !std::ptr::eq(actual_source, call) {
            return Err(mismatch());
        }
        let association = self
            .rows
            .associations
            .get(row.callee_association)
            .ok_or_else(mismatch)?;
        let body = self
            .rows
            .bodies
            .get(association.body)
            .ok_or_else(mismatch)?;
        let returned = self
            .rows
            .control
            .get(row.return_control)
            .ok_or_else(mismatch)?;
        let Some(SemanticCallableDeclV1::Defined { function }) =
            semantic.callables().get(call.callee().index() as usize)
        else {
            return Err(mismatch());
        };
        let destination = call.destination().ok_or_else(mismatch)?;
        if association.key.root != root
            || association.key.function != *function
            || body.physical != association.key.physical
            || row.call.block.function.0 as usize != row.caller.physical
            || returned.association != row.callee_association
            || association.return_control != row.return_control
            || !matches!(returned.kind, UnitLocalControlKindV1::Return { local, unit_type, .. }
                if local == association.unit_return_local && unit_type == row.unit_type)
            || row.unit_type != association.unit_type
            || destination.place().local() != row.destination_local
            || destination.place().ty() != row.unit_type
            || destination.edge().target() != row.continuation_source
            || !destination.place().projections().is_empty()
            || !call.arguments().is_empty()
        {
            return Err(mismatch());
        }
        let module = self.inventory.owner().module();
        let native_function = module
            .functions
            .get(row.caller.physical)
            .ok_or_else(mismatch)?;
        let native_body = native_function.body.as_ref().ok_or_else(mismatch)?;
        let native_block = native_body
            .blocks
            .get(row.call.block.block as usize)
            .ok_or_else(mismatch)?;
        let operation = native_block
            .operations
            .get(row.call.operation as usize)
            .ok_or_else(mismatch)?;
        let target = module
            .functions
            .get(association.key.physical)
            .ok_or_else(mismatch)?;
        let OperationKind::Call { callee, arguments } = &operation.kind else {
            return Err(mismatch());
        };
        budget.charge_work(argument_sum_v1(&[
            callee.as_str().len(),
            target.id.as_str().len(),
        ])?)?;
        if callee != &target.id
            || !arguments.is_empty()
            || !operation.results.is_empty()
            || !matches!(&native_block.terminator, Some(Terminator::Branch { target, arguments })
                if *target == row.continuation_physical && arguments.is_empty())
        {
            return Err(mismatch());
        }
        Ok(Some(ProductionUnitLocalBoundsNeutralCallV1 {
            row,
            association,
            source: actual_source,
            operation,
        }))
    }
}
