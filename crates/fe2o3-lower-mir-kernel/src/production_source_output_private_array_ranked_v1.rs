/// One immutable, source-qualified private write after checked N/B/O transport.
/// The numeric observations grant no artifact, formal or launch authority.
#[derive(Debug)]
pub struct PrivateArrayOutputFactV1 {
    source_key: [u32; 7],
    original_effect: usize,
    original_slot: usize,
    ranked: (usize, u32, u32),
    offset: u64,
    output: Option<PrivateArrayOutputStoreV1>,
}

impl PrivateArrayOutputFactV1 {
    /// Owner, function, block, statement, role class, role ordinal and component.
    pub const fn source_key(&self) -> [u32; 7] {
        self.source_key
    }
    /// Original effect and slot ordinals in this receipt's private-array relation.
    pub const fn original_effect_and_slot(&self) -> (usize, usize) {
        (self.original_effect, self.original_slot)
    }
    /// Root ordinal, ranked block and ranked operation of the matched access.
    pub const fn ranked_access(&self) -> (usize, u32, u32) {
        self.ranked
    }
    /// Source-proven element offset, not a byte offset.
    pub const fn offset(&self) -> u64 {
        self.offset
    }
    /// None means a checked unreachable omission, not an unretained source local.
    pub const fn output(&self) -> Option<&PrivateArrayOutputStoreV1> {
        self.output.as_ref()
    }
    /// Numeric observations alone never authorize compilation or execution.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Actual O coordinates and stored-value use, not a new scalar evaluator.
#[derive(Debug)]
pub struct PrivateArrayOutputStoreV1 {
    allocation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    gep: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    access: fe2o3_kernel_ir::CanonicalKirAccessCoordinateV1,
    value_definition: fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1,
    value: ValueId,
    executable: bool,
}

impl PrivateArrayOutputStoreV1 {
    /// Actual checked-output allocation containing the addressed element.
    pub const fn allocation(&self) -> fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
        self.allocation
    }
    /// Actual checked-output pointer-index operation used by the Store.
    pub const fn gep(&self) -> fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
        self.gep
    }
    /// Actual checked-output Store and its exact memory-effect ordinal.
    pub const fn access(&self) -> fe2o3_kernel_ir::CanonicalKirAccessCoordinateV1 {
        self.access
    }
    /// Checked-output definition selected by the Store's value operand.
    pub const fn value_definition(&self) -> fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 {
        self.value_definition
    }
    /// Function-local SSA value used by the actual checked-output Store.
    pub const fn value(&self) -> ValueId {
        self.value
    }
    /// Whether checked execution can reach the original source effect.
    pub const fn executable(&self) -> bool {
        self.executable
    }
}

/// Scoped custody of a closed private-array Store correspondence, not execution permission.
/// Unrelated memory effects, calls, assembly and Execution operations are refused.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionMaterializedRankedModuleReceiptV1,
///     PrivateArrayOutputFactV1, ProductionSourceOutputErrorV1};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// use fe2o3_pliron::CheckedNeutralKernelIrOwnerV1;
/// fn escape<'a>(receipt: &'a ProductionMaterializedRankedModuleReceiptV1,
///     bound: &'a VerifiedCanonicalKernelIrModuleV12, output: &'a CheckedNeutralKernelIrOwnerV1,
///     budget: &mut Budget<'_>) -> Result<&'a PrivateArrayOutputFactV1, ProductionSourceOutputErrorV1> {
///     receipt.with_checked_private_array_output_v1(bound, output, budget,
///         |scope, budget| Ok(scope.get(0, budget)?.unwrap()))
/// }
/// ```
pub struct CheckedPrivateArrayOutputRankedV1<'scope> {
    _receipt: &'scope ProductionMaterializedRankedModuleReceiptV1,
    _bound: &'scope fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    _output: SourceOutputCheckedEndpointV1<'scope>,
    rows: &'scope [PrivateArrayOutputFactV1],
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
}

impl CheckedPrivateArrayOutputRankedV1<'_> {
    fn require_live(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<(), ProductionSourceOutputErrorV1> {
        // Preserve validation-work-before-accounting precedence.
        budget
            .charge_work(5)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        if self.slot != budget as *const _ as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            return Err(ProductionSourceOutputErrorV1::Resource(
                AssertOriginResourceV1::Accounting,
            ));
        }
        Ok(())
    }

    /// Number of matched source writes, including checked unreachable omissions.
    pub fn len(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<usize, ProductionSourceOutputErrorV1> {
        self.require_live(budget)?;
        budget
            .charge_work(1)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        Ok(self.rows.len())
    }

    /// Borrows one fact after checking the original ledger and live storage floor.
    pub fn get(
        &self,
        index: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Option<&PrivateArrayOutputFactV1>, ProductionSourceOutputErrorV1> {
        self.require_live(budget)?;
        budget
            .charge_work(1)
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        Ok(self.rows.get(index))
    }

    /// This correspondence scope never authorizes artifact publication or launch.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

impl ProductionMaterializedRankedModuleReceiptV1 {
    /// Checks a closed private-array Store subset against this exact immutable
    /// source-ranked receipt and actual checked optimizer output. The original
    /// source queries run once per occurrence, including each whole initializer.
    /// This does not attach checks, admit helpers or activate private-address R2.
    ///
    /// Keep the source's cached analysis subtotal and checked O/history reserved.
    /// B's separate replay receipt remains caller-reserved, as in the occurrence
    /// query API. B has no owner-derived retained-size getter: numeric floors do
    /// not prove its reservation or custody. Exact N/B and B/history equality
    /// establish endpoint custody separately. Existing source/SSA/ranked-roster
    /// allocations retain their existing accounting exclusions.
    ///
    /// New workspace and borrowed views are prepaid on this ledger. All exits
    /// drop workspace before restoring the incoming floor. A changed Work meter
    /// causes Accounting without releasing foreign storage; valid-meter callback
    /// errors/panics are preserved unless a live floor was lost.
    /// Caller-owned output retained in R or captured state must be reserved before
    /// entry; callback scratch must be dropped before returning. Scope rollback
    /// does not account arbitrary surviving caller allocations or transfer them.
    pub fn with_checked_private_array_output_v1<'w, R>(
        &self,
        bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        output: &fe2o3_pliron::CheckedNeutralKernelIrOwnerV1,
        budget: &mut AssertOriginBudgetV1<'w>,
        next: impl for<'scope> FnOnce(
            CheckedPrivateArrayOutputRankedV1<'scope>,
            &mut AssertOriginBudgetV1<'w>,
        ) -> Result<R, ProductionSourceOutputErrorV1>,
    ) -> Result<R, ProductionSourceOutputErrorV1> {
        self.with_checked_private_array_output_endpoint_v1(
            bound,
            SourceOutputCheckedEndpointV1::Optimizer(output),
            budget,
            next,
        )
    }

    /// Checks the same closed Store correspondence using the actual Policy3
    /// output owner. Its execution witness stays borrowed, never converted to
    /// the historical owner. All source, ledger, scope and admission restrictions
    /// of `with_checked_private_array_output_v1` still apply.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::{ProductionMaterializedRankedModuleReceiptV1,
    ///     PrivateArrayOutputFactV1, ProductionSourceOutputErrorV1};
    /// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12,
    ///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
    /// use fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1;
    /// fn escape<'a>(receipt: &'a ProductionMaterializedRankedModuleReceiptV1,
    ///     bound: &'a VerifiedCanonicalKernelIrModuleV12,
    ///     output: &'a CheckedNeutralKernelIrOwnerPolicy3V1,
    ///     budget: &mut Budget<'_>) -> Result<&'a PrivateArrayOutputFactV1, ProductionSourceOutputErrorV1> {
    ///     receipt.with_checked_private_array_output_policy3_v1(bound, output, budget,
    ///         |scope, budget| Ok(scope.get(0, budget)?.unwrap()))
    /// }
    /// ```
    pub fn with_checked_private_array_output_policy3_v1<'w, R>(
        &self,
        bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        output: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
        budget: &mut AssertOriginBudgetV1<'w>,
        next: impl for<'scope> FnOnce(
            CheckedPrivateArrayOutputRankedV1<'scope>,
            &mut AssertOriginBudgetV1<'w>,
        ) -> Result<R, ProductionSourceOutputErrorV1>,
    ) -> Result<R, ProductionSourceOutputErrorV1> {
        self.with_checked_private_array_output_endpoint_v1(
            bound,
            SourceOutputCheckedEndpointV1::OptimizerPolicy3(output),
            budget,
            next,
        )
    }

    fn with_checked_private_array_output_endpoint_v1<'w, R>(
        &self,
        bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        output: SourceOutputCheckedEndpointV1<'_>,
        budget: &mut AssertOriginBudgetV1<'w>,
        next: impl for<'scope> FnOnce(
            CheckedPrivateArrayOutputRankedV1<'scope>,
            &mut AssertOriginBudgetV1<'w>,
        ) -> Result<R, ProductionSourceOutputErrorV1>,
    ) -> Result<R, ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        budget.charge_work(8).map_err(Error::Resource)?;
        let incoming = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let slot = budget as *const _ as usize;
        let minimum = self
            .materialized
            .retained_analysis_storage_v1()
            .checked_add(output.storage().retained_storage())
            .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
        if incoming < minimum {
            return Err(Error::Resource(AssertOriginResourceV1::Accounting));
        }
        let callback_floor = std::cell::Cell::new(None);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            budget
                .reserve_storage(std::mem::size_of::<CheckedPrivateArrayOutputRankedV1<'_>>())
                .map_err(Error::Resource)?;
            let (coordinates, coordinate_storage) =
                fe2o3_kernel_analysis::check_canonical_kir_coordinate_preservation_v1(
                    self.materialized.executable(),
                    bound,
                    budget,
                )
                .map_err(|error| match error {
                    fe2o3_kernel_analysis::CanonicalKirCoordinatePreservationErrorV1::Resource(
                        e,
                    ) => Error::Resource(e),
                    fe2o3_kernel_analysis::CanonicalKirCoordinatePreservationErrorV1::Mismatch(
                        _,
                    ) => Error::InputCustody,
                })?;
            budget
                .reserve_storage(coordinate_storage.retained_storage())
                .map_err(Error::Resource)?;
            let (view, view_storage) = derive_source_output_occurrences_with_endpoint_v1(
                &self.materialized,
                &coordinates,
                output,
                budget,
            )?;
            budget
                .reserve_storage(view_storage.retained_storage())
                .map_err(Error::Resource)?;
            let (inventory, inventory_storage) =
                fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(output.owner(), budget)
                    .map_err(Error::Inventory)?;
            budget
                .reserve_storage(inventory_storage.retained_storage())
                .map_err(Error::Resource)?;
            let workspace = private_array_output_workspace_v1(self, &view, &inventory, budget)?;
            let floor = budget.storage();
            callback_floor.set(Some(floor));
            next(
                CheckedPrivateArrayOutputRankedV1 {
                    _receipt: self,
                    _bound: bound,
                    _output: output,
                    rows: &workspace.facts,
                    slot,
                    ledger,
                    floor,
                },
                budget,
            )
        }));
        if ledger != budget.work_ledger_identity_v1() {
            return Err(Error::Resource(AssertOriginResourceV1::Accounting));
        }
        let lost = callback_floor
            .get()
            .is_some_and(|floor| budget.storage() < floor);
        let release = budget
            .storage()
            .checked_sub(incoming)
            .ok_or(Error::Resource(AssertOriginResourceV1::Accounting))?;
        budget.release_storage(release).map_err(Error::Resource)?;
        if lost {
            return Err(Error::Resource(AssertOriginResourceV1::Accounting));
        }
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

struct PrivateArrayOutputWorkV1<'a, 'w> {
    budget: &'a mut AssertOriginBudgetV1<'w>,
}
impl PrivateArrayChargeV1 for PrivateArrayOutputWorkV1<'_, '_> {
    type Error = ProductionSourceOutputErrorV1;
    fn charge_private_array_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.budget
            .charge_work(amount)
            .map_err(ProductionSourceOutputErrorV1::Resource)
    }
}

#[derive(Clone, Copy)]
struct PrivateArrayOutputOriginalV1 {
    key: [u32; 7],
    effect: usize,
}
struct PrivateArrayOutputSourceV1 {
    key: [usize; 4],
    root: usize,
    row: usize,
    consumed: bool,
}
struct PrivateArrayOutputDefinitionV1<'a> {
    key: [usize; 2],
    operation: &'a ProductionRankedOperationV1,
}
struct PrivateArrayOutputClaimV1 {
    key: [usize; 3],
    fact: usize,
}
struct PrivateArrayOutputAllocationV1 {
    key: [usize; 3],
    slot: usize,
}
struct PrivateArrayOutputWorkspaceV1<'a> {
    originals: Vec<PrivateArrayOutputOriginalV1>,
    sources: Vec<PrivateArrayOutputSourceV1>,
    definitions: Vec<PrivateArrayOutputDefinitionV1<'a>>,
    facts: Vec<PrivateArrayOutputFactV1>,
    stores: Vec<PrivateArrayOutputClaimV1>,
    allocations: Vec<PrivateArrayOutputAllocationV1>,
    ranked: Vec<[usize; 3]>,
}

fn private_array_output_components_v1(
    receipt: &ProductionMaterializedRankedModuleReceiptV1,
    view: &ProductionSourceOutputOccurrencesV1<'_, '_>,
    workspace: &mut PrivateArrayOutputWorkspaceV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1 as Site;
    let original = &receipt.materialized.correspondence.private_arrays;
    let mut start = 0usize;
    while start < workspace.originals.len() {
        budget.charge_work(3).map_err(Error::Resource)?;
        let first = workspace.originals[start];
        let effect = &original.effects[first.effect];
        let mut end = start + 1;
        while end < workspace.originals.len() {
            budget.charge_work(7).map_err(Error::Resource)?;
            if workspace.originals[end].key[..6] != first.key[..6] {
                break;
            }
            end += 1;
        }
        budget.charge_work(3).map_err(Error::Resource)?;
        if effect.owner != effect.function {
            return Err(Error::Invalid(
                "private output helper instance is outside the closed subset",
            ));
        }
        let site = Site::Statement {
            block: fe2o3_mir_model::SsaBlockIdV1::new(effect.semantic_block),
            statement: effect.semantic_statement,
        };
        let initializer = matches!(
            effect.original_index,
            PrivateArrayIndexV1::InitializerElement { .. }
        );
        let ordinary_offset = if initializer {
            let outcome =
                view.private_array_initializer(effect.owner, effect.function, site, budget)?;
            budget.charge_work(3).map_err(Error::Resource)?;
            if !matches!(outcome, ProductionSourceOutputPrivateArrayInitializerV1::Checked { components, .. }
                if usize::try_from(components).ok() == Some(end - start))
            {
                return Err(Error::Invalid(
                    "private output initializer original census changed",
                ));
            }
            None
        } else {
            let (_, offset) = view.private_array_write_with_offset_v1(
                effect.owner,
                effect.function,
                site,
                effect.role,
                budget,
            )?;
            budget.charge_work(2).map_err(Error::Resource)?;
            if end - start != 1 || offset.is_none() {
                return Err(Error::Invalid("private output ordinary occurrence changed"));
            }
            offset
        };
        for position in start..end {
            budget.charge_work(3).map_err(Error::Resource)?;
            let selected = workspace.originals[position];
            let effect = &original.effects[selected.effect];
            let component = selected.key[6];
            let offset = ordinary_offset.unwrap_or(u64::from(component));
            if initializer
                != matches!(
                    effect.original_index,
                    PrivateArrayIndexV1::InitializerElement { .. }
                )
            {
                return Err(Error::Invalid("private output component kind changed"));
            }
            let found = private_array_binary_search_v1(
                &view.private_arrays,
                |row| row.key.map(|x| x as usize),
                selected.key.map(|x| x as usize),
                &mut PrivateArrayOutputWorkV1 { budget },
            )?
            .map_err(|_| Error::Invalid("private output component row is absent"))?;
            budget.charge_work(4).map_err(Error::Resource)?;
            let row = &view.private_arrays[found];
            if row.original_effect != selected.effect {
                return Err(Error::Invalid(
                    "private output original effect identity changed",
                ));
            }
            let slot = original
                .slots
                .get(row.original_slot)
                .ok_or(Error::Invalid("private output original slot is absent"))?;
            let source_key = [
                effect.owner.index() as usize,
                effect.semantic_block as usize,
                effect.semantic_statement as usize,
                if initializer { component as usize } else { 0 },
            ];
            let source_index = private_array_binary_search_v1(
                &workspace.sources,
                |row| row.key,
                source_key,
                &mut PrivateArrayOutputWorkV1 { budget },
            )?
            .map_err(|_| Error::Invalid("private output ranked source row is absent"))?;
            budget.charge_work(6).map_err(Error::Resource)?;
            let source = &mut workspace.sources[source_index];
            if source.consumed {
                return Err(Error::Invalid("private output ranked source reused"));
            }
            source.consumed = true;
            let root = &receipt.roots[source.root];
            let access = &root.access_sources[source.row];
            let operation = root
                .lowering
                .kernel()
                .blocks()
                .get(access.ranked_block() as usize)
                .and_then(|b| b.operations().get(access.ranked_operation() as usize))
                .ok_or(Error::Invalid("private output ranked operation is absent"))?;
            let ProductionRankedOperationV1::Access {
                view: ranked_view,
                indices,
                kind,
            } = operation
            else {
                return Err(Error::Invalid(
                    "private output ranked operation is not an Access",
                ));
            };
            budget.charge_work(2).map_err(Error::Resource)?;
            if effect.access != PrivateArrayAccessV1::Write
                || *kind != dialect_kernel::AccessKindAttr::Write
            {
                return Err(Error::Invalid(
                    "private output ranked access is not an ordinary Write",
                ));
            }
            private_array_ranked_address_v1(
                slot,
                offset,
                *ranked_view,
                indices,
                &mut PrivateArrayOutputWorkV1 { budget },
                |value, work| {
                    work.charge_private_array_work(1)?;
                    let ProductionRankedValueV1::Local(value) = value else {
                        return Ok(None);
                    };
                    let found = private_array_binary_search_v1(
                        &workspace.definitions,
                        |row| row.key,
                        [source.root, value.get() as usize],
                        work,
                    )?;
                    match found {
                        Ok(index) => {
                            work.charge_private_array_work(1)?;
                            Ok(Some(workspace.definitions[index].operation))
                        }
                        Err(_) => Ok(None),
                    }
                },
                || Error::Invalid("private output ranked address changed"),
            )?;
            budget.charge_work(24).map_err(Error::Resource)?;
            let ranked = (
                source.root,
                access.ranked_block(),
                access.ranked_operation(),
            );
            let output = match row.placement {
                SourceOutputArrayPlacementV1::Unsupported => {
                    return Err(Error::Invalid("private output placement is unsupported"));
                }
                SourceOutputArrayPlacementV1::OmittedUnreachable => None,
                SourceOutputArrayPlacementV1::Retained(anchors) => {
                    let (value, _) = source_output_definition_v1(
                        view.output(),
                        anchors.uses[4].definition,
                        budget,
                    )?;
                    Some(PrivateArrayOutputStoreV1 {
                        allocation: anchors.allocation,
                        gep: anchors.gep,
                        access: fe2o3_kernel_ir::CanonicalKirAccessCoordinateV1 {
                            operation: anchors.memory,
                            effect: 0,
                        },
                        value_definition: anchors.uses[4].definition,
                        value,
                        executable: anchors.executable,
                    })
                }
            };
            budget.charge_work(24).map_err(Error::Resource)?;
            private_array_output_push_v1(
                &mut workspace.ranked,
                [ranked.0, ranked.1 as usize, ranked.2 as usize],
            )?;
            if let Some(output) = &output {
                private_array_output_push_v1(
                    &mut workspace.allocations,
                    PrivateArrayOutputAllocationV1 {
                        key: private_array_output_coordinate_key_v1(output.allocation),
                        slot: row.original_slot,
                    },
                )?;
                private_array_output_push_v1(
                    &mut workspace.stores,
                    PrivateArrayOutputClaimV1 {
                        key: private_array_output_coordinate_key_v1(output.access.operation),
                        fact: workspace.facts.len(),
                    },
                )?;
            }
            private_array_output_push_v1(
                &mut workspace.facts,
                PrivateArrayOutputFactV1 {
                    source_key: selected.key,
                    original_effect: selected.effect,
                    original_slot: row.original_slot,
                    ranked,
                    offset,
                    output,
                },
            )?;
        }
        start = end;
    }
    Ok(())
}

include!("production_source_output_private_array_census_v1.rs");
