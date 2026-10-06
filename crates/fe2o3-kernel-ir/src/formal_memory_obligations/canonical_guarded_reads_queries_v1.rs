use super::*;

/// A normalized index coordinate, meaningful only on its borrowed read fact.
/// This descriptive value is not a constructor or detached proof token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalGuardedReadIndexOriginV1 {
    /// Exact SSA origin through the analyzer's supported representation links.
    ProvenOrigin(ValueId),
    /// The exact block parameter is guarded, but has no unique incoming origin.
    ExactBlockParameter(ValueId),
}

/// A borrowed local read fact; the runtime allocation remains an obligation.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::CanonicalGuardedGlobalReadFactV18;
/// fn detach<'s, 'g>(fact: &CanonicalGuardedGlobalReadFactV18<'s, 'g>)
///     -> CanonicalGuardedGlobalReadFactV18<'s, 'g> { fact.clone() }
/// ```
pub struct CanonicalGuardedGlobalReadFactV1<'scope, 'g, O = VerifiedCanonicalKernelIrModuleV12> {
    owner: &'g O,
    row: &'scope ReadRow,
    conditions: &'scope RuntimeSliceReadConditionsV1,
}
impl<O> CanonicalGuardedGlobalReadFactV1<'_, '_, O> {
    /// Exact borrowed owner, not an identity-only substitute.
    pub const fn owner(&self) -> &O {
        self.owner
    }
    /// Exact physical operation occurrence.
    pub const fn operation(&self) -> Coordinate {
        self.row.coordinate
    }
    /// Descriptive local domain. A copy alone is not a fresh graph fact.
    pub const fn domain(&self) -> &FormalRuntimeSliceReadDomainV1 {
        &self.conditions.domain
    }
    /// Existing representation analysis equates the actual GEP offset and
    /// comparison index to this coordinate, not to an equal numeric value.
    /// The coordinate belongs to this fact's owner and function occurrence.
    pub const fn normalized_index_origin(&self) -> CanonicalGuardedReadIndexOriginV1 {
        match self.conditions.index_origin {
            runtime_slice_read_v1::ReadIndex::ProvenOrigin(value) => {
                CanonicalGuardedReadIndexOriginV1::ProvenOrigin(value)
            }
            runtime_slice_read_v1::ReadIndex::ExactBlockParameter(value) => {
                CanonicalGuardedReadIndexOriginV1::ExactBlockParameter(value)
            }
        }
    }
    /// Exact SliceLength result reached from the comparison's right operand.
    /// Another SliceLength on the same slice is not this occurrence.
    pub const fn normalized_length_origin(&self) -> ValueId {
        self.conditions.length_origin
    }
    /// Actual comparison operands before representation normalization.
    /// The actual GEP offset remains available as `domain().index()`.
    pub const fn comparison_operands(&self) -> (ValueId, ValueId) {
        (
            self.conditions.domain.guard_index(),
            self.conditions.domain.length(),
        )
    }
    /// The graph proves no live allocation or launch binding.
    pub const fn requires_runtime_allocation_binding(&self) -> bool {
        true
    }
}

/// A local result is never an empty complete safety report.
pub enum CanonicalGuardedGlobalReadOutcomeV1<'scope, 'g, O = VerifiedCanonicalKernelIrModuleV12> {
    /// Exact slice bound/alignment/provenance conditions on this occurrence.
    ProvedLocalConditions(CanonicalGuardedGlobalReadFactV1<'scope, 'g, O>),
    /// Required graph facts are not available.
    NotProved(CanonicalGuardedGlobalReadReasonV1),
}

/// Predicate truth derived from an actual edge dominating an exact read site.
pub struct CanonicalGuardedPredicateFactV1<'scope, 'g, O = VerifiedCanonicalKernelIrModuleV12> {
    owner: &'g O,
    at: Coordinate,
    row: &'scope PredicateRow,
}
impl<O> CanonicalGuardedPredicateFactV1<'_, '_, O> {
    /// Exact graph subject.
    pub const fn owner(&self) -> &O {
        self.owner
    }
    /// Actual query site dominated by the successful edge.
    pub const fn at(&self) -> Coordinate {
        self.at
    }
    /// Exact Boolean SSA definition; no caller-provided truth.
    pub const fn value(&self) -> ValueId {
        self.row.value
    }
    /// Derived polarity, not an input authorization bit.
    pub const fn is_true(&self) -> bool {
        self.row.truth
    }
    /// Actual predecessor, successor ordinal and target.
    pub const fn edge(&self) -> (BlockId, usize, BlockId) {
        (
            self.row.edge.source,
            self.row.edge.ordinal,
            self.row.edge.target,
        )
    }
}

/// Exact checked operation whose own overflow result is false at this site.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::CanonicalGuardedNoWrapFactV1;
/// fn detach<'s, 'g>(fact: &CanonicalGuardedNoWrapFactV1<'s, 'g>)
///     -> CanonicalGuardedNoWrapFactV1<'s, 'g> {
///     fact.clone()
/// }
/// ```
pub struct CanonicalGuardedNoWrapFactV1<'scope, 'g, O = VerifiedCanonicalKernelIrModuleV12> {
    predicate: CanonicalGuardedPredicateFactV1<'scope, 'g, O>,
    coordinate: Coordinate,
    operation: &'g Operation,
    value: ValueId,
    overflow: ValueId,
    lhs: ValueId,
    rhs: ValueId,
    scalar: ScalarType,
}
impl<O> CanonicalGuardedNoWrapFactV1<'_, '_, O> {
    /// Actual checked operation coordinate.
    pub const fn operation_coordinate(&self) -> Coordinate {
        self.coordinate
    }
    /// Exact borrowed operation, including its checked operator.
    pub const fn operation(&self) -> &Operation {
        self.operation
    }
    /// Exact result ordinal zero.
    pub const fn value(&self) -> ValueId {
        self.value
    }
    /// Exact Bool result ordinal one.
    pub const fn overflow(&self) -> ValueId {
        self.overflow
    }
    /// Exact ordered inputs, not an interval summary.
    pub const fn operands(&self) -> (ValueId, ValueId) {
        (self.lhs, self.rhs)
    }
    /// Actual unsigned/Index result type.
    pub const fn scalar(&self) -> ScalarType {
        self.scalar
    }
    /// Borrowed false-overflow edge fact for the actual query site.
    pub const fn predicate(&self) -> &CanonicalGuardedPredicateFactV1<'_, '_, O> {
        &self.predicate
    }
}

/// A local read condition tied to the exact V18 canonical owner.
pub type CanonicalGuardedGlobalReadFactV18<'scope, 'g> =
    CanonicalGuardedGlobalReadFactV1<'scope, 'g, VerifiedCanonicalKernelIrModuleV18>;

/// Supported or unproved local conditions from an actual V18 graph.
pub type CanonicalGuardedGlobalReadOutcomeV18<'scope, 'g> =
    CanonicalGuardedGlobalReadOutcomeV1<'scope, 'g, VerifiedCanonicalKernelIrModuleV18>;

/// An exact dominating edge predicate on the borrowed V18 graph.
pub type CanonicalGuardedPredicateFactV18<'scope, 'g> =
    CanonicalGuardedPredicateFactV1<'scope, 'g, VerifiedCanonicalKernelIrModuleV18>;

/// A false-overflow fact from the exact checked operation on a V18 graph.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::CanonicalGuardedNoWrapFactV18;
/// fn duplicate<'s, 'g>(fact: &CanonicalGuardedNoWrapFactV18<'s, 'g>)
///     -> CanonicalGuardedNoWrapFactV18<'s, 'g> { fact.clone() }
/// ```
pub type CanonicalGuardedNoWrapFactV18<'scope, 'g> =
    CanonicalGuardedNoWrapFactV1<'scope, 'g, VerifiedCanonicalKernelIrModuleV18>;

impl<'scope, 'g, O> CheckedCanonicalGuardedGlobalReadsV1<'scope, 'g, O> {
    /// Paid exact-owner query. Caller scratch may be above the captured floor.
    pub fn owner(&self, budget: &mut Budget<'_>) -> Result<&'g O> {
        self.accounting.enter(budget)?;
        Ok(self.facts.owner)
    }

    /// Includes declarations in original module order; no fabricated body report.
    pub fn function_count(&self, budget: &mut Budget<'_>) -> Result<usize> {
        self.accounting.enter(budget)?;
        Ok(self.facts.functions.len())
    }

    /// Complete local Global-read, other-Global-effect and unresolved-call counts.
    /// These counters are descriptive pending subjects, never effect clearance.
    pub fn function_effects(
        &self,
        function: FunctionCoordinate,
        budget: &mut Budget<'_>,
    ) -> Result<(usize, usize, usize)> {
        self.accounting.enter(budget)?;
        self.accounting.save((|| {
            budget.charge_work(2)?;
            let row = self
                .facts
                .functions
                .get(usize::try_from(function.0).map_err(|_| ResourceError::Arithmetic)?)
                .ok_or(ResourceError::Accounting)?;
            Ok((
                row.global_read_occurrences,
                row.other_global_effects,
                row.unresolved_calls,
            ))
        })())
    }

    /// Local read proof at the exact actual operation; runtime validity is pending.
    pub fn read_at(
        &self,
        coordinate: Coordinate,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalGuardedGlobalReadOutcomeV1<'_, 'g, O>> {
        self.accounting.enter(budget)?;
        self.accounting.save((|| {
            let (function, _) = self.operation_at(coordinate, budget)?;
            let selected = verification_find_last_by_v1(&function.reads, 3, budget, |row| {
                row.coordinate.cmp(&coordinate)
            })?;
            let Some(row) = selected.map(|index| &function.reads[index]) else {
                return Ok(CanonicalGuardedGlobalReadOutcomeV1::NotProved(
                    CanonicalGuardedGlobalReadReasonV1::NotOrdinaryGlobalRead,
                ));
            };
            Ok(match row.conditions.as_ref() {
                Some(conditions) => CanonicalGuardedGlobalReadOutcomeV1::ProvedLocalConditions(
                    CanonicalGuardedGlobalReadFactV1 {
                        owner: self.facts.owner,
                        row,
                        conditions,
                    },
                ),
                None => CanonicalGuardedGlobalReadOutcomeV1::NotProved(row.reason),
            })
        })())
    }

    /// Asks whether an actual Boolean definition is true here; it never supplies
    /// truth to the engine or weakens an unsupported graph into success.
    pub fn true_at(
        &self,
        at: Coordinate,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> Result<Option<CanonicalGuardedPredicateFactV1<'_, 'g, O>>> {
        self.predicate_at(at, value, true, budget)
    }

    /// Asks for the corresponding independently derived false-polarity fact.
    pub fn false_at(
        &self,
        at: Coordinate,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> Result<Option<CanonicalGuardedPredicateFactV1<'_, 'g, O>>> {
        self.predicate_at(at, value, false, budget)
    }

    /// Requires this checked operation's own exact overflow definition false.
    /// Does not imply that this operation supplies a selected source address;
    /// the consumer must join the returned operand/result subject to that use.
    pub fn no_wrap_at(
        &self,
        at: Coordinate,
        checked: Coordinate,
        budget: &mut Budget<'_>,
    ) -> Result<Option<CanonicalGuardedNoWrapFactV1<'_, 'g, O>>> {
        self.accounting.enter(budget)?;
        self.accounting.save((|| {
            self.operation_at(at, budget)?;
            let (_, operation) = self.operation_at(checked, budget)?;
            budget.charge_work(12)?;
            if at.block.function != checked.block.function {
                return Ok(None);
            }
            let OperationKind::Binary {
                op: BinaryOp::Checked(_),
                lhs,
                rhs,
            } = operation.kind
            else {
                return Ok(None);
            };
            let [value, overflow] = operation.results.as_slice() else {
                return Ok(None);
            };
            let Type::Scalar(
                scalar @ (ScalarType::U8
                | ScalarType::U16
                | ScalarType::U32
                | ScalarType::U64
                | ScalarType::Index),
            ) = value.ty
            else {
                return Ok(None);
            };
            if overflow.ty != Type::BOOL {
                return Ok(None);
            }
            // Verified owner construction already checked exact same operand
            // types and the value/Bool result pair. Here identity stays exact.
            let Some(predicate) = self.predicate_at(at, overflow.id, false, budget)? else {
                return Ok(None);
            };
            Ok(Some(CanonicalGuardedNoWrapFactV1 {
                predicate,
                coordinate: checked,
                operation,
                value: value.id,
                overflow: overflow.id,
                lhs,
                rhs,
                scalar,
            }))
        })())
    }

    fn operation_at(
        &self,
        coordinate: Coordinate,
        budget: &mut Budget<'_>,
    ) -> Result<(&FunctionFacts<'g>, &'g Operation)> {
        budget.charge_work(8)?;
        let invalid = || Failure::Coordinate(coordinate);
        let function = self
            .facts
            .functions
            .get(usize::try_from(coordinate.block.function.0).map_err(|_| invalid())?)
            .ok_or_else(invalid)?;
        let block = function
            .function
            .body
            .as_ref()
            .ok_or_else(invalid)?
            .blocks
            .get(usize::try_from(coordinate.block.block).map_err(|_| invalid())?)
            .ok_or_else(invalid)?;
        let operation = block
            .operations
            .get(usize::try_from(coordinate.operation).map_err(|_| invalid())?)
            .ok_or_else(invalid)?;
        Ok((function, operation))
    }

    fn predicate_at(
        &self,
        at: Coordinate,
        value: ValueId,
        truth: bool,
        budget: &mut Budget<'_>,
    ) -> Result<Option<CanonicalGuardedPredicateFactV1<'_, 'g, O>>> {
        self.accounting.enter(budget)?;
        self.accounting.save((|| {
            let (function, _) = self.operation_at(at, budget)?;
            budget.charge_work(2)?;
            let block = &function
                .function
                .body
                .as_ref()
                .ok_or(ResourceError::Accounting)?
                .blocks[usize::try_from(at.block.block).map_err(|_| ResourceError::Arithmetic)?];
            let control = verification_find_last_by_v1(&function.controls, 1, budget, |row| {
                row.block.cmp(&block.id)
            })?;
            let Some((start, end)) = control.and_then(|index| function.controls[index].interval)
            else {
                return Ok(None);
            };
            let selected = verification_find_last_by_v1(&function.predicates, 4, budget, |row| {
                match (row.value, row.truth).cmp(&(value, truth)) {
                    std::cmp::Ordering::Equal if row.interval.0 <= start => {
                        std::cmp::Ordering::Equal
                    }
                    std::cmp::Ordering::Equal => std::cmp::Ordering::Greater,
                    order => order,
                }
            })?;
            let Some(selected) = selected else {
                return Ok(None);
            };
            budget.charge_work(5)?;
            let row = &function.predicates[function.predicates[selected].covering];
            if row.interval.0 > start || end > row.interval.1 {
                return Ok(None);
            }
            Ok(Some(CanonicalGuardedPredicateFactV1 {
                owner: self.facts.owner,
                at,
                row,
            }))
        })())
    }
}
