//! A separate narrow relation. Historical positional State and its scalar,
//! literal, phi and control solvers are deliberately not invoked here.

use super::{Candidate, DescendantKind, Inventory, Origin, index, payload};
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirControlFlowScopeErrorV1 as CfgError, CanonicalKirControlFlowViewV1 as Cfg,
    CanonicalKirDefinitionCoordinateV1 as Definition, OperationKind, ScalarType,
    with_canonical_kir_control_flow_v1,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[path = "commutative_bitwise_shape.rs"]
mod shape;

/// Exact resource, inherited coordinate/payload, CFG, or closed-rule refusal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalKirCommutativeBitwiseCseErrorV1 {
    Resource(Resource),
    Transition(super::CanonicalKirTransitionErrorV1),
    ControlFlow(CfgError),
    Rule(&'static str),
    Panicked,
}
type Error = CanonicalKirCommutativeBitwiseCseErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<super::CanonicalKirTransitionErrorV1> for Error {
    fn from(error: super::CanonicalKirTransitionErrorV1) -> Self {
        Self::Transition(error)
    }
}
impl From<CfgError> for Error {
    fn from(error: CfgError) -> Self {
        Self::ControlFlow(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "commutative bitwise CSE: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Borrow-bound evidence for this relation only, not a historical transition,
/// executed pass, full equivalence proof, source admission, or runtime authority.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::{CheckedCanonicalKirCommutativeBitwiseCseV1 as New, CheckedCanonicalKirTransitionV1 as Old};
/// fn cannot_relabel<'a, 'i, 'o, 'r>(new: New<'a, 'i, 'o, 'r>) -> Old<'a, 'i, 'o, 'r> { new }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_analysis::{CanonicalKirInventoryV1 as Inventory, CheckedCanonicalKirCommutativeBitwiseCseV1 as Checked, check_canonical_kir_commutative_bitwise_cse_v1 as check};
/// use fe2o3_kernel_ir::{CanonicalKirTransitionCandidateV1 as Candidate, CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn cannot_detach<'a, 'i, 'o, 'r>(input: &'a Inventory<'i>, output: Inventory<'o>, rows: Candidate<'r>, budget: &mut Budget<'_>) -> Checked<'a, 'i, 'o, 'r> {
///     check(input, &output, rows, budget).unwrap().0
/// }
/// ```
#[derive(Debug)]
pub struct CheckedCanonicalKirCommutativeBitwiseCseV1<'a, 'input, 'output, 'rows> {
    input: &'a Inventory<'input>,
    output: &'a Inventory<'output>,
    rows: Candidate<'rows>,
    proved_pairs: usize,
}
impl<'a, 'input, 'output, 'rows>
    CheckedCanonicalKirCommutativeBitwiseCseV1<'a, 'input, 'output, 'rows>
{
    pub const fn input(&self) -> &'a Inventory<'input> {
        self.input
    }
    pub const fn output(&self) -> &'a Inventory<'output> {
        self.output
    }
    pub const fn rows(&self) -> Candidate<'rows> {
        self.rows
    }
    /// Each omitted single-result definition has exactly one proved pair.
    pub const fn proved_pairs(&self) -> usize {
        self.proved_pairs
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Transfer of the small borrowed result header only. Subjects, inventories and
/// candidate rows are caller-owned/prepaid and are not copied into this receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirCommutativeBitwiseCseStorageV1(usize);
impl CanonicalKirCommutativeBitwiseCseStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Checks unchanged declaration/CFG shape, exact retained operation/use/effect
/// order, and only fixed I/U8/16/32/64 AND/OR/XOR deletions. Candidate descendant
/// pairs are obligations, not equivalence premises. Function and block arguments
/// remain distinct atoms. A dense memoized DFS grounds at most one pair per
/// omitted definition, with four child visits per pair and no fixed-point scan.
///
/// New vector/header storage is actual-capacity bytes; the separately reviewed
/// CFG scope retains its documented logical row-cell contract, not bytes/RSS.
/// All arrays are prepaid before that scope. Work, peak and first denial remain
/// cumulative. Local owned scratch drops before the same-ledger floor is restored.
/// The returned small checked header must be reserved before a later allocation.
pub fn check_canonical_kir_commutative_bitwise_cse_v1<'a, 'input, 'output, 'rows>(
    input: &'a Inventory<'input>,
    output: &'a Inventory<'output>,
    rows: Candidate<'rows>,
    budget: &mut Budget<'_>,
) -> Result<(
    CheckedCanonicalKirCommutativeBitwiseCseV1<'a, 'input, 'output, 'rows>,
    CanonicalKirCommutativeBitwiseCseStorageV1,
)> {
    scoped(budget, |budget| {
        let retained = size_of::<CheckedCanonicalKirCommutativeBitwiseCseV1<'_, '_, '_, '_>>();
        budget.charge_work(1)?;
        budget.reserve_storage(retained)?;
        let proved_pairs = {
            let mut state = State::new(input, output, rows, budget)?;
            state.check_shape(budget)?;
            state.prove(budget)?;
            state.check_uses(budget)?;
            state.proved_pairs
        };
        Ok((
            CheckedCanonicalKirCommutativeBitwiseCseV1 {
                input,
                output,
                rows,
                proved_pairs,
            },
            CanonicalKirCommutativeBitwiseCseStorageV1(retained),
        ))
    })
}

const NONE: usize = usize::MAX;
const UNSEEN: u8 = 0;
const ACTIVE: u8 = 1;
const PROVED: u8 = 2;

#[derive(Clone, Copy)]
struct Frame {
    definition: usize,
    child: usize,
}

struct State<'a, 'input, 'output, 'rows> {
    input: &'a Inventory<'input>,
    output: &'a Inventory<'output>,
    rows: Candidate<'rows>,
    retained: Vec<usize>,    // Input operation to output operation, or NONE.
    origins: Vec<usize>,     // Output operation to exact input operation.
    anchors: Vec<usize>,     // Output definition to exact retained input definition.
    descendants: Vec<usize>, // Each input definition to its sole output definition.
    seen: Vec<u8>,
    status: Vec<u8>,
    stack: Vec<Frame>,
    proved_pairs: usize,
}

fn reserve<T>(count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>> {
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(1)?;
    budget.reserve_storage(bytes)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let excess = values
        .capacity()
        .checked_sub(count)
        .and_then(|n| n.checked_mul(size_of::<T>()))
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(excess)?;
    Ok(values)
}
fn filled<T: Copy>(count: usize, value: T, budget: &mut Budget<'_>) -> Result<Vec<T>> {
    let mut values = reserve(count, budget)?;
    budget.charge_work(count)?;
    values.resize(count, value);
    Ok(values)
}

impl<'a, 'input, 'output, 'rows> State<'a, 'input, 'output, 'rows> {
    fn new(
        input: &'a Inventory<'input>,
        output: &'a Inventory<'output>,
        rows: Candidate<'rows>,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.charge_work(1)?;
        budget.reserve_storage(size_of::<Self>())?;
        Ok(Self {
            input,
            output,
            rows,
            retained: filled(input.operations().len(), NONE, budget)?,
            origins: filled(output.operations().len(), NONE, budget)?,
            anchors: filled(output.definitions().len(), NONE, budget)?,
            descendants: filled(input.definitions().len(), NONE, budget)?,
            seen: filled(output.definitions().len(), 0, budget)?,
            status: filled(input.definitions().len(), UNSEEN, budget)?,
            stack: reserve(input.definitions().len(), budget)?,
            proved_pairs: 0,
        })
    }

    fn target(&self, definition: usize, budget: &mut Budget<'_>) -> Result<usize> {
        budget.charge_work(2)?;
        let output = *self
            .descendants
            .get(definition)
            .ok_or(Error::Rule("definition lookup"))?;
        self.anchors
            .get(output)
            .copied()
            .filter(|i| *i != NONE)
            .ok_or(Error::Rule("retained target anchor"))
    }

    fn eligible(&self, definition: usize, budget: &mut Budget<'_>) -> Result<usize> {
        budget.charge_work(2)?;
        let row = self
            .input
            .definitions()
            .get(definition)
            .ok_or(Error::Rule("definition lookup"))?;
        let Definition::Result {
            operation,
            result: 0,
        } = row.coordinate
        else {
            return Err(Error::Rule("only single-result bitwise definitions"));
        };
        let operation = index::operation(self.input, operation, budget)?;
        let op = &self.input.operations()[operation];
        if !matches!(
            op.operation.kind,
            OperationKind::Binary {
                op: BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor,
                ..
            }
        ) || op.results.len() != 1
            || op.operands.len() != 2
            || !matches!(
                row.ty.as_scalar(),
                Some(
                    ScalarType::I8
                        | ScalarType::I16
                        | ScalarType::I32
                        | ScalarType::I64
                        | ScalarType::U8
                        | ScalarType::U16
                        | ScalarType::U32
                        | ScalarType::U64
                )
            )
        {
            return Err(Error::Rule("closed fixed integer bitwise eligibility"));
        }
        for use_index in op.operands.clone() {
            budget.charge_work(1)?;
            let operand = self.input.uses()[use_index].definition;
            if !payload::ty(row.ty, self.input.definitions()[operand].ty, budget)? {
                return Err(Error::Rule("bitwise operand type"));
            }
        }
        Ok(operation)
    }

    fn operands(&self, definition: usize, budget: &mut Budget<'_>) -> Result<[usize; 4]> {
        let target = self.target(definition, budget)?;
        let a = self.eligible(definition, budget)?;
        let b = self.eligible(target, budget)?;
        if self.retained[a] != NONE
            || self.retained[b] == NONE
            || !payload::operation(
                &self.input.operations()[a].operation.kind,
                &self.input.operations()[b].operation.kind,
                budget,
            )?
            || !payload::ty(
                self.input.definitions()[definition].ty,
                self.input.definitions()[target].ty,
                budget,
            )?
        {
            return Err(Error::Rule("removed/retained pair payload"));
        }
        let a = self.input.operations()[a].operands.start;
        let b = self.input.operations()[b].operands.start;
        budget.charge_work(4)?;
        Ok([
            self.input.uses()[a].definition,
            self.input.uses()[a + 1].definition,
            self.input.uses()[b].definition,
            self.input.uses()[b + 1].definition,
        ])
    }

    fn prove(&mut self, budget: &mut Budget<'_>) -> Result<()> {
        let input = self.input;
        for function in input.functions() {
            budget.charge_work(1)?;
            if function.function.body.is_none() {
                continue;
            }
            with_canonical_kir_control_flow_v1(
                input.owner(),
                function.coordinate,
                Default::default(),
                budget,
                |cfg, budget| {
                    for definition in function.definitions.clone() {
                        budget.charge_work(1)?;
                        if self.status[definition] != PROVED {
                            self.pair(definition, cfg, budget)?;
                        }
                    }
                    Ok::<_, Error>(())
                },
            )?;
        }
        Ok(())
    }

    fn pair(
        &mut self,
        definition: usize,
        cfg: &mut Cfg<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        self.enter(definition, cfg, budget)?;
        while let Some(frame) = self.stack.last().copied() {
            budget.charge_work(1)?;
            let operands = self.operands(frame.definition, budget)?;
            if frame.child < operands.len() {
                self.stack.last_mut().expect("live frame").child += 1;
                let child = operands[frame.child];
                budget.charge_work(1)?;
                match self.status[child] {
                    PROVED => {}
                    ACTIVE => return Err(Error::Rule("cyclic ungrounded definition pairs")),
                    UNSEEN => self.enter(child, cfg, budget)?,
                    _ => return Err(Error::Rule("pair state")),
                }
            } else {
                let a = self.target(operands[0], budget)?;
                let b = self.target(operands[1], budget)?;
                let c = self.target(operands[2], budget)?;
                let d = self.target(operands[3], budget)?;
                budget.charge_work(4)?;
                if !((a == c && b == d) || (a == d && b == c)) {
                    return Err(Error::Rule("unproved exact-or-swapped operand pair"));
                }
                self.status[frame.definition] = PROVED;
                self.proved_pairs = self
                    .proved_pairs
                    .checked_add(1)
                    .ok_or(Resource::Arithmetic)?;
                self.stack.pop();
            }
        }
        Ok(())
    }

    fn enter(
        &mut self,
        definition: usize,
        cfg: &mut Cfg<'_, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        budget.charge_work(1)?;
        if self.status[definition] != UNSEEN {
            return Err(Error::Rule("pair entered twice"));
        }
        let target = self.target(definition, budget)?;
        let a = self.eligible(definition, budget)?;
        let b = self.eligible(target, budget)?;
        let removed = self.input.operations()[a].coordinate;
        let retained = self.input.operations()[b].coordinate;
        if retained.block.function != removed.block.function
            || retained.block.function != cfg.function()
            || !cfg.dominates(retained.block, removed.block, budget)?
            || (retained.block == removed.block && retained.operation >= removed.operation)
        {
            return Err(Error::Rule(
                "retained expression must precede and dominate deletion",
            ));
        }
        if self.stack.len() >= self.input.definitions().len() {
            return Err(Error::Rule("pair stack bound"));
        }
        budget.charge_work(1)?;
        self.status[definition] = ACTIVE;
        self.stack.push(Frame {
            definition,
            child: 0,
        });
        Ok(())
    }
}

fn scoped<'work, T>(
    budget: &mut Budget<'work>,
    run: impl FnOnce(&mut Budget<'work>) -> Result<T>,
) -> Result<T> {
    let floor = budget.storage();
    let slot = budget as *const Budget<'_> as usize;
    let ledger = budget.work_ledger_identity_v1();
    let protected = catch_unwind(AssertUnwindSafe(|| run(budget)));
    let mut panics = [None, None];
    let mut result = match protected {
        Ok(result) => result,
        Err(payload) => {
            panics[0] = Some(payload);
            Err(Error::Panicked)
        }
    };
    if slot != budget as *const Budget<'_> as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage() < floor
    {
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(result))) {
            panics[1] = Some(payload);
        }
        result = Err(Resource::Accounting.into());
    } else {
        let delta = budget.storage() - floor;
        // A successful result is only a borrowed view; all owned scratch is
        // already dropped inside run. Failed owned test stages drop in unwind.
        if let Err(error) = budget.release_storage(delta) {
            result = Err(error.into());
        }
    }
    drop(panics);
    result
}

#[cfg(test)]
#[path = "commutative_bitwise_resources_tests.rs"]
mod resource_tests;
