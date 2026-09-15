// Included beneath the existing source/output occurrence view. These temporary
// numeric rows and returned counts are diagnostics, never attachment authority.

/// Diagnostic counts from an exact ordinary ranked/output memory census.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSourceOutputEffectCensusV1 {
    accesses: usize,
    private_allocations: usize,
}

impl ProductionSourceOutputEffectCensusV1 {
    /// Number of ordinary accesses matched in both directions.
    pub const fn accesses(self) -> usize {
        self.accesses
    }

    /// Number of physical allocations covered by checked private-array rows.
    pub const fn private_allocations(self) -> usize {
        self.private_allocations
    }
}

#[derive(Clone, Copy)]
struct SourceOutputCensusAccessV1 {
    physical: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    ranked: [u32; 2],
    kind: dialect_kernel::AccessKindAttr,
    space: AddressSpace,
}

fn source_output_census_ranked_kind_v1(
    operation: &ProductionRankedOperationV1,
) -> Result<Option<dialect_kernel::AccessKindAttr>, ProductionSourceOutputErrorV1> {
    use ProductionRankedOperationV1 as Ranked;
    use ProductionSourceOutputErrorV1 as Error;
    match operation {
        Ranked::Access { kind, .. } | Ranked::ValueAccess { kind, .. }
            if matches!(
                kind,
                dialect_kernel::AccessKindAttr::Read | dialect_kernel::AccessKindAttr::Write
            ) =>
        {
            Ok(Some(*kind))
        }
        Ranked::ExecutionLayout { .. }
        | Ranked::View { .. }
        | Ranked::ViewInSpace { .. }
        | Ranked::IndexConstant { .. }
        | Ranked::IndexUnsignedCast { .. }
        | Ranked::IndexUnknown { .. }
        | Ranked::InvocationIndex { .. }
        | Ranked::IndexBinary { .. }
        | Ranked::DeterministicJoin { .. }
        | Ranked::CheckedTiledIndex2D { .. }
        | Ranked::CheckedRowStripedIndex2D { .. }
        | Ranked::PredicatedCheckedTiledIndex2D { .. }
        | Ranked::PredicatedCheckedRowStripedIndex2D { .. }
        | Ranked::Dimension { .. }
        | Ranked::OwnershipContract { .. }
        | Ranked::SemanticSymbol { .. }
        | Ranked::SemanticConstant { .. }
        | Ranked::SemanticBinary { .. }
        | Ranked::SemanticExpression { .. }
        | Ranked::RequireEquivalent { .. }
        | Ranked::RequireAuthenticatedReferenceEquivalent { .. }
        | Ranked::RequestAuthenticatedReferenceEquivalent { .. }
        | Ranked::RequireEffectRefinement { .. }
        | Ranked::RequestEffectRefinement { .. }
        | Ranked::RequireNumericalRefinement { .. }
        | Ranked::RequestNumericalRefinement { .. } => Ok(None),
        _ => Err(Error::Invalid(
            "output effect census requires a separate ranked effect rule",
        )),
    }
}

fn source_output_census_physical_kind_v1(
    operation: &Operation,
) -> Result<
    Option<(Option<dialect_kernel::AccessKindAttr>, AddressSpace)>,
    ProductionSourceOutputErrorV1,
> {
    use ProductionSourceOutputErrorV1 as Error;
    use dialect_kernel::AccessKindAttr as Access;
    match &operation.kind {
        OperationKind::Load { access, .. }
            if access.address_space == AddressSpace::Global && !access.volatile =>
        {
            Ok(Some((Some(Access::Read), AddressSpace::Global)))
        }
        OperationKind::Store { access, .. }
            if matches!(
                access.address_space,
                AddressSpace::Global | AddressSpace::Private
            ) && !access.volatile =>
        {
            Ok(Some((Some(Access::Write), access.address_space)))
        }
        OperationKind::Alloca {
            address_space: AddressSpace::Private,
            ..
        } => Ok(Some((None, AddressSpace::Private))),
        OperationKind::Constant(_)
        | OperationKind::Intrinsic(_)
        | OperationKind::Unary { .. }
        | OperationKind::Binary { .. }
        | OperationKind::Compare { .. }
        | OperationKind::Cast { .. }
        | OperationKind::Select { .. }
        | OperationKind::SliceLength { .. }
        | OperationKind::SliceData { .. }
        | OperationKind::GetElementPointer { .. } => Ok(None),
        _ => Err(Error::Invalid(
            "output effect census requires a separate physical effect rule",
        )),
    }
}

impl ProductionSourceOutputOccurrencesV1<'_, '_> {
    fn census_private_source_v1(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        source: ProductionRankedAccessSourceV1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1, ProductionSourceOutputErrorV1>
    {
        use ProductionSourceOutputErrorV1 as Error;
        budget.charge_work(3).map_err(Error::Resource)?;
        let statement = source.semantic_statement().ok_or(Error::Invalid(
            "output private census requires an ordinary source statement",
        ))?;
        let prefix = [
            owner.index(),
            function.index(),
            source.semantic_block(),
            statement,
        ];
        let ordinal = assert_origin_find_v1(&self.private_arrays, budget, |row, budget| {
            budget.charge_work(4)?;
            Ok(row.key[..4].cmp(&prefix))
        })
        .map_err(Error::SourceOrigin)?
        .ok_or(Error::Invalid("output private census source row absent"))?;
        // A dense ordinal never supplies the operand role. Require one sealed
        // retained source row at this site, then read its original role below.
        budget.charge_work(11).map_err(Error::Resource)?;
        if ordinal
            .checked_sub(1)
            .and_then(|index| self.private_arrays.get(index))
            .is_some_and(|row| row.key[..4] == prefix)
            || self
                .private_arrays
                .get(ordinal + 1)
                .is_some_and(|row| row.key[..4] == prefix)
            || source.semantic_access_ordinal() != 0
        {
            return Err(Error::Invalid(
                "output private census requires one source-qualified write",
            ));
        }
        let row = &self.private_arrays[ordinal];
        let effect = self
            .source
            .correspondence
            .private_arrays
            .effects
            .get(row.original_effect)
            .ok_or(Error::Invalid("output private census original row absent"))?;
        budget.charge_work(3).map_err(Error::Resource)?;
        if effect.access != PrivateArrayAccessV1::Write
            || effect.role != fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::Destination
        {
            return Err(Error::Invalid(
                "output private census requires the original destination role",
            ));
        }
        match self.private_array_write(
            owner,
            function,
            fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                block: fe2o3_mir_model::SsaBlockIdV1::new(source.semantic_block()),
                statement,
            },
            effect.role,
            budget,
        )? {
            ProductionSourceOutputPrivateArrayAccessV1::Retained {
                coordinate,
                executable: true,
                ..
            } if coordinate.effect == 0 => Ok(coordinate.operation),
            _ => Err(Error::Invalid(
                "output private census requires an executable retained write",
            )),
        }
    }

    /// Checks both directions between ordinary ranked accesses and actual O
    /// accesses, and accounts for every physical private allocation via C.
    /// Retained unreachable memory, calls, generated effects, atomics, guards,
    /// synchronization, tensor and compiler-owned scalar storage require their
    /// own rules and reject. Pure physically retained unreachable code is inert.
    ///
    /// The caller must also check Global allocation/write values, the private
    /// allocation/index hook, mandatory ranked reports and checked source CFG.
    /// This census checks neither control/effect ordering nor write values and
    /// cannot construct an attachment or formal owner. Scratch uses the same
    /// caller canonical ledger and is dropped before restoring the entry floor.
    #[allow(
        clippy::too_many_arguments,
        reason = "Exact root/body, ranked graph, both source rosters and live ledger remain distinct inputs"
    )]
    pub fn check_ranked_output_effect_census(
        &self,
        owner: SemanticFunctionIdV1,
        function: SemanticFunctionIdV1,
        lowering: &ProductionRankedKernelLoweringInputV1,
        sources: &[ProductionRankedAccessSourceV1],
        generated: &[ProductionRankedExecutableEffectSourceV1],
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<ProductionSourceOutputEffectCensusV1, ProductionSourceOutputErrorV1> {
        use ProductionSourceOutputErrorV1 as Error;
        use fe2o3_kernel_ir::{
            CanonicalKirBlockCoordinateV1 as BlockCoordinate,
            CanonicalKirOperationCoordinateV1 as Coordinate,
        };
        let floor = budget.storage();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            budget.charge_work(5).map_err(Error::Resource)?;
            let minimum = self
                .source
                .executable_storage()
                .retained_storage()
                .checked_add(self.source.assert_origin_storage().payload_storage())
                .and_then(|n| n.checked_add(self.checked_output.storage().retained_storage()))
                .and_then(|n| n.checked_add(self.storage.retained_storage()))
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            if floor < minimum {
                return Err(Error::Resource(AssertOriginResourceV1::Accounting));
            }
            if !generated.is_empty() {
                return Err(Error::Invalid(
                    "output effect census requires a separate generated effect rule",
                ));
            }
            let origins = &self.source.assert_origins().origins.functions;
            let alias = assert_origin_find_v1(origins, budget, |entry, budget| {
                budget.charge_work(2)?;
                Ok((entry.owner, entry.function).cmp(&(owner, function)))
            })
            .map_err(Error::SourceOrigin)?
            .ok_or(Error::Invalid("output census source alias absent"))?;
            budget.charge_work(5).map_err(Error::Resource)?;
            let canonical = origins[alias].canonical;
            let input = self
                .bound
                .module()
                .functions
                .get(canonical.0 as usize)
                .ok_or(Error::Invalid("output census bound function absent"))?;
            let output = self
                .output()
                .module()
                .functions
                .get(canonical.0 as usize)
                .ok_or(Error::Invalid("output census output function absent"))?;
            if !private_array_equal_bytes_v1(
                input.id.as_str().as_bytes(),
                output.id.as_str().as_bytes(),
                &mut PrivateArrayQueryWorkV1 { budget },
            )
            .map_err(Error::PrivateArray)?
            {
                return Err(Error::Invalid("output census exact function changed"));
            }
            if output.role != fe2o3_kernel_ir::FunctionRole::KernelEntry {
                return Err(Error::Invalid("output census requires an entry function"));
            }
            let body = output
                .body
                .as_ref()
                .ok_or(Error::Invalid("output census body absent"))?;
            budget.charge_work(7).map_err(Error::Resource)?;
            let bytes = sources
                .len()
                .checked_mul(std::mem::size_of::<SourceOutputCensusAccessV1>())
                .and_then(|n| {
                    self.private_arrays
                        .len()
                        .checked_mul(std::mem::size_of::<Coordinate>())
                        .and_then(|m| n.checked_add(m))
                })
                .and_then(|n| n.checked_add(std::mem::size_of::<Vec<SourceOutputCensusAccessV1>>()))
                .and_then(|n| n.checked_add(std::mem::size_of::<Vec<Coordinate>>()))
                .ok_or(Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            std::alloc::Layout::array::<SourceOutputCensusAccessV1>(sources.len())
                .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            std::alloc::Layout::array::<Coordinate>(self.private_arrays.len())
                .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?;
            budget.reserve_storage(bytes).map_err(Error::Resource)?;
            let mut accesses = Vec::<SourceOutputCensusAccessV1>::new();
            let mut allocations = Vec::<Coordinate>::new();
            budget.charge_work(2).map_err(Error::Resource)?;
            accesses
                .try_reserve_exact(sources.len())
                .map_err(|_| Error::Resource(AssertOriginResourceV1::Allocation))?;
            let mut accesses = source_output_census_exact_capacity_v1(accesses, sources.len())?;
            budget.charge_work(2).map_err(Error::Resource)?;
            allocations
                .try_reserve_exact(self.private_arrays.len())
                .map_err(|_| Error::Resource(AssertOriginResourceV1::Allocation))?;
            let mut allocations =
                source_output_census_exact_capacity_v1(allocations, self.private_arrays.len())?;
            for source in sources {
                budget.charge_work(5).map_err(Error::Resource)?;
                let operation = lowering
                    .kernel()
                    .blocks()
                    .get(source.ranked_block() as usize)
                    .and_then(|block| block.operations().get(source.ranked_operation() as usize))
                    .ok_or(Error::Invalid("output census ranked access absent"))?;
                let kind = source_output_census_ranked_kind_v1(operation)?.ok_or(
                    Error::Invalid("output census source does not name a ranked access"),
                )?;
                let (physical, space) = match self.global_access(
                    owner,
                    function,
                    source.semantic_block(),
                    source.semantic_statement(),
                    source.semantic_access_ordinal(),
                    budget,
                )? {
                    ProductionSourceOutputGlobalAccessV1::Retained {
                        operation,
                        executable: true,
                        ..
                    } => (operation, AddressSpace::Global),
                    ProductionSourceOutputGlobalAccessV1::Unsupported(
                        ProductionSourceOutputGlobalUnsupportedV1::PrivateMemory,
                    ) => {
                        budget.charge_work(1).map_err(Error::Resource)?;
                        if kind != dialect_kernel::AccessKindAttr::Write {
                            return Err(Error::Invalid(
                                "output census refuses a private read source",
                            ));
                        }
                        (
                            self.census_private_source_v1(owner, function, *source, budget)?,
                            AddressSpace::Private,
                        )
                    }
                    _ => {
                        return Err(Error::Invalid(
                            "output census source access is unsupported or nonexecutable",
                        ));
                    }
                };
                budget.charge_work(2).map_err(Error::Resource)?;
                if physical.block.function != canonical {
                    return Err(Error::Invalid(
                        "output census access escaped its exact function",
                    ));
                }
                accesses.push(SourceOutputCensusAccessV1 {
                    physical,
                    ranked: [source.ranked_block(), source.ranked_operation()],
                    kind,
                    space,
                });
            }
            for row in &self.private_arrays {
                budget.charge_work(3).map_err(Error::Resource)?;
                if row.key[0] != owner.index() || row.key[1] != function.index() {
                    continue;
                }
                let SourceOutputArrayPlacementV1::Retained(anchors) = row.placement else {
                    if matches!(row.placement, SourceOutputArrayPlacementV1::Unsupported)
                        && matches!(
                            self.block(
                                owner,
                                function,
                                SemanticBlockIdV1::from_index(row.key[2]),
                                budget
                            )?,
                            ProductionSourceOutputBlockV1::Materialized {
                                executable: true,
                                ..
                            }
                        )
                    {
                        return Err(Error::Invalid(
                            "output census has an unsupported executable private source",
                        ));
                    }
                    continue;
                };
                budget.charge_work(2).map_err(Error::Resource)?;
                if anchors.executable {
                    if anchors.allocation.block.function != canonical {
                        return Err(Error::Invalid(
                            "output census allocation escaped its exact function",
                        ));
                    }
                    allocations.push(anchors.allocation);
                }
            }
            assert_origin_sort_v1(&mut accesses, budget, |left, right, budget| {
                budget.charge_work(2)?;
                Ok(left.ranked.cmp(&right.ranked))
            })
            .map_err(Error::SourceOrigin)?;
            let mut ranked_count = 0usize;
            for (block_index, block) in lowering.kernel().blocks().iter().enumerate() {
                budget.charge_work(1).map_err(Error::Resource)?;
                for (operation_index, operation) in block.operations().iter().enumerate() {
                    budget.charge_work(2).map_err(Error::Resource)?;
                    if let Some(kind) = source_output_census_ranked_kind_v1(operation)? {
                        budget.charge_work(6).map_err(Error::Resource)?;
                        let coordinate = [
                            u32::try_from(block_index)
                                .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                            u32::try_from(operation_index)
                                .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                        ];
                        let claim = accesses
                            .get(ranked_count)
                            .ok_or(Error::Invalid("output census ranked access lacks a source"))?;
                        if claim.ranked != coordinate || claim.kind != kind {
                            return Err(Error::Invalid(
                                "output census ranked source order or kind differs",
                            ));
                        }
                        ranked_count += 1;
                    }
                }
            }
            budget.charge_work(1).map_err(Error::Resource)?;
            if ranked_count != accesses.len() {
                return Err(Error::Invalid(
                    "output census has duplicate or unused ranked sources",
                ));
            }
            assert_origin_sort_v1(&mut accesses, budget, |left, right, budget| {
                budget.charge_work(3)?;
                Ok(left.physical.cmp(&right.physical))
            })
            .map_err(Error::SourceOrigin)?;
            assert_origin_sort_v1(&mut allocations, budget, |left, right, budget| {
                budget.charge_work(3)?;
                Ok(left.cmp(right))
            })
            .map_err(Error::SourceOrigin)?;
            let (mut access_count, mut allocation_cursor, mut allocation_count) =
                (0usize, 0usize, 0usize);
            let mut runtime_failure_seen = false;
            for (block_index, block) in body.blocks.iter().enumerate() {
                budget.charge_work(1).map_err(Error::Resource)?;
                for (operation_index, operation) in block.operations.iter().enumerate() {
                    budget.charge_work(2).map_err(Error::Resource)?;
                    if matches!(operation.kind, OperationKind::Call { .. }) {
                        budget.charge_work(5).map_err(Error::Resource)?;
                        // The replayed lowerer emits at most one shared runtime
                        // failure block per function, outside all source blocks.
                        // A second Call cannot use that rule, even with its name.
                        if runtime_failure_seen {
                            return Err(Error::Invalid("duplicate runtime failure trap Call"));
                        }
                        let coordinate = Coordinate {
                            block: BlockCoordinate {
                                function: canonical,
                                block: u32::try_from(block_index).map_err(|_| {
                                    Error::Resource(AssertOriginResourceV1::Arithmetic)
                                })?,
                            },
                            operation: u32::try_from(operation_index)
                                .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                        };
                        self.census_runtime_failure_trap_v1(owner, function, coordinate, budget)?;
                        runtime_failure_seen = true;
                        continue;
                    }
                    let Some((kind, space)) = source_output_census_physical_kind_v1(operation)?
                    else {
                        continue;
                    };
                    budget.charge_work(4).map_err(Error::Resource)?;
                    let coordinate = Coordinate {
                        block: BlockCoordinate {
                            function: canonical,
                            block: u32::try_from(block_index)
                                .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                        },
                        operation: u32::try_from(operation_index)
                            .map_err(|_| Error::Resource(AssertOriginResourceV1::Arithmetic))?,
                    };
                    if let Some(kind) = kind {
                        budget.charge_work(6).map_err(Error::Resource)?;
                        let claim = accesses.get(access_count).ok_or(Error::Invalid(
                            "output census physical access lacks a ranked source",
                        ))?;
                        if claim.physical != coordinate
                            || claim.kind != kind
                            || claim.space != space
                        {
                            return Err(Error::Invalid(
                                "output census physical access order, kind or space differs",
                            ));
                        }
                        access_count += 1;
                    } else {
                        budget.charge_work(4).map_err(Error::Resource)?;
                        if allocations.get(allocation_cursor) != Some(&coordinate) {
                            return Err(Error::Invalid(
                                "output census private allocation lacks a checked array source",
                            ));
                        }
                        loop {
                            budget.charge_work(4).map_err(Error::Resource)?;
                            if allocations.get(allocation_cursor) != Some(&coordinate) {
                                break;
                            }
                            budget.charge_work(1).map_err(Error::Resource)?;
                            allocation_cursor += 1;
                        }
                        allocation_count += 1;
                    }
                }
            }
            budget.charge_work(2).map_err(Error::Resource)?;
            if access_count != accesses.len() || allocation_cursor != allocations.len() {
                return Err(Error::Invalid(
                    "output census has duplicate or unused physical claims",
                ));
            }
            Ok(ProductionSourceOutputEffectCensusV1 {
                accesses: access_count,
                private_allocations: allocation_count,
            })
        }));
        let release = budget
            .storage()
            .checked_sub(floor)
            .ok_or(Error::Resource(AssertOriginResourceV1::Accounting))?;
        budget.release_storage(release).map_err(Error::Resource)?;
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

fn source_output_census_exact_capacity_v1<T>(
    rows: Vec<T>,
    prepaid: usize,
) -> Result<Vec<T>, ProductionSourceOutputErrorV1> {
    if rows.capacity() != prepaid {
        // Consume and drop any unexpected capacity before the caller may
        // allocate or charge again. No row has been written into this buffer.
        return Err(ProductionSourceOutputErrorV1::Resource(
            AssertOriginResourceV1::Accounting,
        ));
    }
    Ok(rows)
}

#[cfg(test)]
mod source_output_census_capacity_tests {
    use super::*;

    #[test]
    fn first_excess_capacity_is_rejected_before_later_work_denial() {
        // Deliberately preallocate excess capacity to exercise the guard's
        // allocator-result branch deterministically. This is a guard-component
        // test, not a claim that the real root's allocator returned excess.
        const HISTORY: usize = 7;
        const FLOOR: usize = 11;
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(HISTORY + 2);
        let mut budget = AssertOriginBudgetV1::new(&mut work, FLOOR + 8);
        budget.charge_work(HISTORY).unwrap();
        budget.reserve_storage(FLOOR + 8).unwrap();
        let second_allocation = std::cell::Cell::new(false);
        let result = (|| -> Result<(), ProductionSourceOutputErrorV1> {
            budget
                .charge_work(2)
                .map_err(ProductionSourceOutputErrorV1::Resource)?;
            let excess = Vec::<u32>::with_capacity(2);
            let _first = source_output_census_exact_capacity_v1(excess, 1)?;
            budget
                .charge_work(2)
                .map_err(ProductionSourceOutputErrorV1::Resource)?;
            second_allocation.set(true);
            Ok(())
        })();
        assert!(matches!(
            result,
            Err(ProductionSourceOutputErrorV1::Resource(
                AssertOriginResourceV1::Accounting
            ))
        ));
        assert!(!second_allocation.get());
        assert_eq!(budget.work(), HISTORY + 2);
        budget.release_storage(8).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR + 8);
        // The immediate capacity error was not replaced by the next denial.
        // That later attempt independently records its first attempted prefix.
        assert!(
            matches!(budget.charge_work(2), Err(AssertOriginResourceV1::Work(error)) if error.actual() == HISTORY + 4)
        );
        assert_eq!(budget.work(), HISTORY + 2);
        assert_eq!(work.failed_work(), Some(HISTORY + 4));
    }

    #[test]
    fn exact_first_capacity_drops_before_second_prepaid_charge_failure() {
        const HISTORY: usize = 7;
        const FLOOR: usize = 11;
        for available in [1, 3] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(HISTORY + available);
            let mut budget = AssertOriginBudgetV1::new(&mut work, FLOOR + 8);
            budget.charge_work(HISTORY).unwrap();
            budget.reserve_storage(FLOOR + 8).unwrap();
            let allocations = std::cell::Cell::new(0);
            let result = (|| -> Result<(), ProductionSourceOutputErrorV1> {
                budget
                    .charge_work(2)
                    .map_err(ProductionSourceOutputErrorV1::Resource)?;
                allocations.set(1);
                let first = Vec::<u32>::with_capacity(1);
                let _first = source_output_census_exact_capacity_v1(first, 1)?;
                budget
                    .charge_work(2)
                    .map_err(ProductionSourceOutputErrorV1::Resource)?;
                allocations.set(2);
                Ok(())
            })();
            let accepted = HISTORY + if available == 1 { 0 } else { 2 };
            assert!(
                matches!(result, Err(ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Work(error))) if error.actual() == accepted + 2)
            );
            assert_eq!(allocations.get(), usize::from(available == 3));
            assert_eq!(budget.work(), accepted);
            budget.release_storage(8).unwrap();
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), FLOOR + 8);
            assert_eq!(work.failed_work(), Some(accepted + 2));
        }
    }
}

include!("production_source_output_runtime_trap_v1.rs");
