//! Actual checked-add to nonwrapping-add/false relation with complete lineage.
use crate::{
    CanonicalKirGuardedUpdateV1 as Update, CanonicalKirInductionErrorV1 as FactsError,
    CanonicalKirInductionFactsV1 as Facts, CanonicalKirInductionOutcomeV1 as Outcome,
    CanonicalKirInventoryErrorV1 as InventoryError, CanonicalKirInventoryV1 as Inventory,
    CanonicalKirLoopErrorV1 as LoopError, CanonicalKirLoopLimitsV1 as Limits,
    CanonicalKirLoopsV1 as Loops, canonical_kir_private_cell_pair_resources_v1 as resources,
};
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirOperationCoordinateV1 as Site,
    CheckedBinaryOperator, Constant, OperationKind, ScalarType, Type,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{fmt, mem::size_of};

/// One original operation, in exact input inventory order. Rows are inert.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirInductionRefinementOriginV1 {
    /// Complete unchanged operation, possibly shifted by an earlier split.
    Unchanged { input: Site, output: Site },
    /// Exact adjacent numeric Add and Bool false, preserving original result IDs.
    CheckedAddSplit {
        input: Site,
        sum_output: Site,
        false_output: Site,
        /// First independently checked qualifying input-facts row.
        induction_row_ordinal: usize,
    },
}
type Row = CanonicalKirInductionRefinementOriginV1;
impl Row {
    /// Exact original operation for either kind of row.
    pub const fn input(self) -> Site {
        match self {
            Self::Unchanged { input, .. } | Self::CheckedAddSplit { input, .. } => input,
        }
    }
}

/// Typed refusal of this particular independently checked refinement relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalKirInductionRefinementErrorV1 {
    Resource(Resource),
    Inventory(InventoryError),
    Loops(LoopError),
    Facts(FactsError),
    /// Configured output growth is refused before comparison/mutation admission.
    OutputLimit {
        actual: usize,
        limit: usize,
    },
    Mismatch(&'static str),
    Panicked,
}
type Error = CanonicalKirInductionRefinementErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl From<Resource> for Error {
    fn from(v: Resource) -> Self {
        Self::Resource(v)
    }
}
impl From<InventoryError> for Error {
    fn from(v: InventoryError) -> Self {
        Self::Inventory(v)
    }
}
impl From<LoopError> for Error {
    fn from(v: LoopError) -> Self {
        Self::Loops(v)
    }
}
impl From<FactsError> for Error {
    fn from(v: FactsError) -> Self {
        Self::Facts(v)
    }
}
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "canonical induction-add refinement: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Unreserved witness header. Both owners and complete rows remain borrowed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirInductionRefinementStorageV1(usize);
impl CanonicalKirInductionRefinementStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Sealed actual-pair relation, not source/native/artifact or launch authority.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::CheckedCanonicalKirInductionRefinementV1;
/// fn duplicate<'a>(v: &CheckedCanonicalKirInductionRefinementV1<'a>)
///     -> CheckedCanonicalKirInductionRefinementV1<'a> { v.clone() }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_analysis::check_canonical_kir_induction_refinement_v1;
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn detach(input: Owner, output: &Owner, budget: &mut Budget<'_>) {
///     let (pair, _) = check_canonical_kir_induction_refinement_v1(
///         &input, output, &[], Default::default(), budget).unwrap();
///     drop(input); let _ = pair.input();
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_analysis::{check_canonical_kir_induction_refinement_v1,
///     CanonicalKirInductionRefinementOriginV1};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn detach(input: &Owner, output: &Owner, budget: &mut Budget<'_>) {
///     let rows = Vec::<CanonicalKirInductionRefinementOriginV1>::new();
///     let (pair, _) = check_canonical_kir_induction_refinement_v1(
///         input, output, &rows, Default::default(), budget).unwrap();
///     drop(rows); let _ = pair.origins();
/// }
/// ```
pub struct CheckedCanonicalKirInductionRefinementV1<'a> {
    input: &'a Owner,
    output: &'a Owner,
    origins: &'a [Row],
    limits: Limits,
}
impl<'a> CheckedCanonicalKirInductionRefinementV1<'a> {
    pub const fn input(&self) -> &'a Owner {
        self.input
    }
    pub const fn output(&self) -> &'a Owner {
        self.output
    }
    pub const fn origins(&self) -> &'a [Row] {
        self.origins
    }
    pub const fn limits(&self) -> Limits {
        self.limits
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Replays actual input no-wrap facts and independently checks complete ordered
/// one-to-two coverage, both stable result IDs, all payloads and unchanged CFG.
/// No producer is invoked. Only first qualifying Guarded NonWrapping CheckedAdd
/// rows over U8/U16/U32/U64 are selected. NoUpdate never authorizes a rewrite.
/// Additional work is O(F+B+R+O+wire), scratch O(O), beyond existing analysis
/// services. All actual capacities coexist with live endpoints until drop.
pub fn check_canonical_kir_induction_refinement_v1<'a>(
    input: &'a Owner,
    output: &'a Owner,
    origins: &'a [Row],
    limits: Limits,
    budget: &mut Budget<'_>,
) -> Result<(
    CheckedCanonicalKirInductionRefinementV1<'a>,
    CanonicalKirInductionRefinementStorageV1,
)> {
    resources::scoped(budget, |meter| check(input, output, origins, limits, meter))
}

fn check<'a>(
    input: &'a Owner,
    output: &'a Owner,
    origins: &'a [Row],
    limits: Limits,
    meter: &mut Meter<'_, '_>,
) -> Result<(
    CheckedCanonicalKirInductionRefinementV1<'a>,
    CanonicalKirInductionRefinementStorageV1,
)> {
    let header = size_of::<CheckedCanonicalKirInductionRefinementV1<'_>>();
    meter.reserve(header)?;
    let (a, a_size) = meter.derive(|b| Ok(Inventory::derive(input, b)?))?;
    meter.reserve(a_size.retained_storage())?;
    let (loops, loops_size) = meter.derive(|b| Ok(Loops::derive(&a, limits, b)?))?;
    meter.reserve(loops_size.retained_storage())?;
    let (facts, facts_size) = meter.derive(|b| Ok(Facts::derive(&loops, limits, b)?))?;
    meter.reserve(facts_size.retained_storage())?;
    meter.derive(|b| Ok(facts.replay(&loops, limits, b)?))?;
    meter.reserve(size_of::<Vec<Option<usize>>>())?;
    let (mut expected, expected_size) = meter.table::<Option<usize>>(a.operations().len())?;
    for _ in a.operations() {
        meter.push(&mut expected, None)?;
    }
    let mut splits = 0usize;
    for (index, row) in facts.rows().iter().enumerate() {
        meter.work(20)?;
        if !matches!(row.outcome(), Outcome::Guarded(f) if f.guarded_update() == Update::NonWrapping)
        {
            continue;
        }
        let recurrence = row.recurrence();
        if !matches!(
            recurrence.scalar(),
            ScalarType::U8 | ScalarType::U16 | ScalarType::U32 | ScalarType::U64
        ) {
            continue;
        }
        let Definition::Result {
            operation: site,
            result: 0,
        } = recurrence.update()
        else {
            continue;
        };
        let at = operation_index(&a, site)?;
        let op = a.operations()[at].operation;
        if !matches!(
            op.kind,
            OperationKind::Binary {
                op: BinaryOp::Checked(CheckedBinaryOperator::Add),
                ..
            }
        ) || !matches!(op.results.as_slice(), [sum, flag] if sum.ty == Type::Scalar(recurrence.scalar()) && flag.ty == Type::BOOL)
            || recurrence.overflow()
                != Some(Definition::Result {
                    operation: site,
                    result: 1,
                })
        {
            continue;
        }
        if expected[at].is_none() {
            expected[at] = Some(index);
            splits = splits.checked_add(1).ok_or(Resource::Arithmetic)?;
        }
    }
    meter.work(4)?;
    let count = a
        .operations()
        .len()
        .checked_add(splits)
        .ok_or(Resource::Arithmetic)?;
    if count > limits.operations {
        return Err(Error::OutputLimit {
            actual: count,
            limit: limits.operations,
        });
    }
    if origins.len() != a.operations().len() {
        return Err(Error::Mismatch("complete original operation roster"));
    }
    let (b, b_size) = meter.derive(|budget| Ok(Inventory::derive(output, budget)?))?;
    meter.reserve(b_size.retained_storage())?;
    meter.work(
        input
            .canonical()
            .canonical_bytes()
            .len()
            .checked_add(output.canonical().canonical_bytes().len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    crate::canonical_kir_same_cfg_payload_v1::check(input.module(), output.module())
        .map_err(Error::Mismatch)?;
    if b.operations().len() != count {
        return Err(Error::Mismatch("exact output operation growth"));
    }
    for (old_block, new_block) in a.blocks().iter().zip(b.blocks()) {
        meter.work(3)?;
        let mut next = new_block.operations.start;
        for at in old_block.operations.clone() {
            meter.work(24)?;
            let original = &a.operations()[at];
            let actual = b
                .operations()
                .get(next)
                .filter(|_| next < new_block.operations.end)
                .ok_or(Error::Mismatch("missing output operation"))?;
            if original.coordinate.block != actual.coordinate.block
                || origins[at].input() != original.coordinate
            {
                return Err(Error::Mismatch("exact original order and block"));
            }
            if let Some(fact) = expected[at] {
                let false_at = next.checked_add(1).ok_or(Resource::Arithmetic)?;
                let flag = b
                    .operations()
                    .get(false_at)
                    .filter(|_| false_at < new_block.operations.end)
                    .ok_or(Error::Mismatch("missing adjacent false definition"))?;
                if origins[at]
                    != (Row::CheckedAddSplit {
                        input: original.coordinate,
                        sum_output: actual.coordinate,
                        false_output: flag.coordinate,
                        induction_row_ordinal: fact,
                    })
                {
                    return Err(Error::Mismatch(
                        "exact split coordinates and qualifying fact",
                    ));
                }
                let OperationKind::Binary { lhs, rhs, .. } = original.operation.kind else {
                    return Err(Error::Mismatch("checked input operation"));
                };
                if actual.operation.kind
                    != (OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs,
                        rhs,
                    })
                    || actual.operation.results.as_slice() != &original.operation.results[..1]
                    || flag.operation.kind != OperationKind::Constant(Constant::Bool(false))
                    || flag.operation.results.as_slice() != &original.operation.results[1..]
                {
                    return Err(Error::Mismatch(
                        "exact sum and false payloads and result IDs",
                    ));
                }
                next = false_at.checked_add(1).ok_or(Resource::Arithmetic)?;
            } else {
                if origins[at]
                    != (Row::Unchanged {
                        input: original.coordinate,
                        output: actual.coordinate,
                    })
                    || original.operation != actual.operation
                {
                    return Err(Error::Mismatch("unchanged complete operation"));
                }
                next = next.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
        }
        if next != new_block.operations.end {
            return Err(Error::Mismatch("complete ordered block output"));
        }
    }
    let (output_loops, output_loops_size) =
        meter.derive(|budget| Ok(Loops::derive(&b, limits, budget)?))?;
    meter.reserve(output_loops_size.retained_storage())?;
    meter.derive(|budget| Ok(output_loops.replay(&b, limits, budget)?))?;
    drop(output_loops);
    meter.release(output_loops_size.retained_storage())?;
    drop(expected);
    meter.release(
        expected_size
            .checked_add(size_of::<Vec<Option<usize>>>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    drop(facts);
    meter.release(facts_size.retained_storage())?;
    drop(loops);
    meter.release(loops_size.retained_storage())?;
    drop(b);
    meter.release(b_size.retained_storage())?;
    drop(a);
    meter.release(a_size.retained_storage())?;
    meter.work(1)?;
    Ok((
        CheckedCanonicalKirInductionRefinementV1 {
            input,
            output,
            origins,
            limits,
        },
        CanonicalKirInductionRefinementStorageV1(header),
    ))
}

// Coordinates are dense vector ordinals, never user BlockId/ValueId magnitudes.
fn operation_index(inventory: &Inventory<'_>, site: Site) -> Result<usize> {
    let function = inventory
        .functions()
        .get(site.block.function.0 as usize)
        .filter(|f| f.coordinate == site.block.function)
        .ok_or(Error::Mismatch("function coordinate"))?;
    let at = function
        .blocks
        .start
        .checked_add(site.block.block as usize)
        .filter(|at| *at < function.blocks.end)
        .ok_or(Error::Mismatch("block coordinate"))?;
    let block = inventory
        .blocks()
        .get(at)
        .filter(|b| b.coordinate == site.block)
        .ok_or(Error::Mismatch("actual block coordinate"))?;
    block
        .operations
        .start
        .checked_add(site.operation as usize)
        .filter(|at| *at < block.operations.end && inventory.operations()[*at].coordinate == site)
        .ok_or(Error::Mismatch("operation coordinate"))
}

#[cfg(test)]
#[path = "canonical_kir_induction_refinement_v1_tests.rs"]
mod tests;
