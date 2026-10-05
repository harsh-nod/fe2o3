//! Shared original SSA live-in equations, without emitted-code or proof authority.
use crate::canonical_kir_private_cell_pair_resources_v1 as resources;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};

/// An original boundary equation or its inherited resource account refused.
#[derive(Debug)]
pub enum SourceSsaBoundaryErrorV31 {
    /// A metered work, storage, allocation or accounting operation refused.
    Resource(Resource),
    /// The checked source plan and ordered topology do not agree completely.
    Statement(&'static str),
    /// The caller substituted another source plan at a borrowed query.
    ForeignPlan,
    /// A scoped derivation panicked; no checked result was returned.
    Panicked,
}
type Error = SourceSsaBoundaryErrorV31;
type Result<T> = std::result::Result<T, Error>;
type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "source SSA boundary: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}

fn vector<T>(count: usize, meter: &mut Meter<'_, '_>) -> Result<Vec<T>> {
    meter.table(count).map(|(rows, _)| rows)
}

include!("source_ssa_boundary_equations_v31.rs");

/// Storage of the returned boundary owner and its actual retained capacity.
/// This receipt is unreserved: callers must adopt it before retaining queries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceSsaBoundaryStorageV31 {
    retained: usize,
}
impl SourceSsaBoundaryStorageV31 {
    /// Logical retained bytes, excluding temporary derivation frames and events.
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

/// Complete live-in SSA names for one exact borrowed plan and ordered topology.
/// This validates original SSA equations only. A consumer must independently
/// join actual code, ordered edges and selectors; no source-equivalence,
/// executable, artifact or launch authority is granted by this owner.
pub struct SourceSsaBoundariesV31<'source> {
    plan: &'source Plan,
    entry: Block,
    successors: &'source [Vec<Block>],
    checked: Boundaries,
}

struct DeriveCapture<'a> {
    plan: &'a Plan,
    input: ControlInput<'a>,
}
impl<'a> DeriveCapture<'a> {
    fn parts(self) -> (&'a Plan, ControlInput<'a>) {
        (self.plan, self.input)
    }
}
struct QueryCapture<'v, 's> {
    owner: &'v SourceSsaBoundariesV31<'s>,
    plan: &'v Plan,
    block: Block,
    variable: Variable,
}
impl<'v, 's> QueryCapture<'v, 's> {
    fn parts(self) -> (&'v SourceSsaBoundariesV31<'s>, &'v Plan, Block, Variable) {
        (self.owner, self.plan, self.block, self.variable)
    }
}

// Meter owns its own header; the public wrapper also owns the callback and
// catch/result/disposal envelopes that coexist in resources::scoped.
fn boundary_scope_headers_v31<T, C>() -> Result<usize> {
    type Capture<'a, 'w, C> = (C, &'a mut Meter<'a, 'w>);
    type Frame<'a, 'w, T, C> = (
        C,
        Capture<'a, 'w, C>,
        std::panic::AssertUnwindSafe<Capture<'a, 'w, C>>,
        std::thread::Result<Result<T>>,
        [Result<T>; 2],
        std::panic::AssertUnwindSafe<Result<T>>,
        std::thread::Result<()>,
        [Option<Box<dyn std::any::Any + Send>>; 2],
        Result<()>,
    );
    size_of::<Frame<'_, '_, T, C>>()
        .checked_add(std::mem::align_of::<Frame<'_, '_, T, C>>())
        .ok_or_else(|| Resource::Arithmetic.into())
}

fn boundary_owner_headers_v31() -> Result<usize> {
    type Frame<'a, 'w> = (
        &'a Plan,
        ControlInput<'a>,
        &'a mut Meter<'a, 'w>,
        SourceSsaBoundariesV31<'a>,
        SourceSsaBoundaryStorageV31,
        Result<(SourceSsaBoundariesV31<'a>, SourceSsaBoundaryStorageV31)>,
        Result<Boundaries>,
        usize,
    );
    size_of::<Frame<'_, '_>>()
        .checked_add(std::mem::align_of::<Frame<'_, '_>>())
        .ok_or(Error::Resource(Resource::Arithmetic))?
        .checked_add(boundary_scope_headers_v31::<
            (SourceSsaBoundariesV31<'_>, SourceSsaBoundaryStorageV31),
            DeriveCapture<'_>,
        >()?)
        .ok_or_else(|| Resource::Arithmetic.into())
}

fn boundary_query_headers_v31() -> Result<usize> {
    type Frame<'a> = (
        &'a SourceSsaBoundariesV31<'a>,
        &'a Plan,
        Block,
        Variable,
        Result<Value>,
        Option<usize>,
        Option<Value>,
    );
    size_of::<Frame<'_>>()
        .checked_add(boundary_scope_headers_v31::<Value, QueryCapture<'_, '_>>()?)
        .ok_or_else(|| Resource::Arithmetic.into())
}

impl<'source> SourceSsaBoundariesV31<'source> {
    /// Checks all original events, invocation entry, and ordered edge equations.
    /// The topology must be rederived by the caller from the same source as the
    /// plan, not selected from emitted control flow. Edge-local definitions are
    /// applied only on their exact edge. All scratch is settled before return.
    pub fn derive(
        plan: &'source Plan,
        entry: Block,
        successors: &'source [Vec<Block>],
        budget: &mut Budget<'_>,
    ) -> Result<(Self, SourceSsaBoundaryStorageV31)> {
        let capture = DeriveCapture {
            plan,
            input: ControlInput { entry, successors },
        };
        let run = move |meter: &mut Meter<'_, '_>| {
            let (plan, input) = capture.parts();
            meter.reserve(boundary_owner_headers_v31()?)?;
            let entry = input.entry;
            let successors = input.successors;
            let checked = Boundaries::derive(plan, input, meter)?;
            let retained = checked
                .rows
                .capacity()
                .checked_mul(size_of::<Row>())
                .and_then(|bytes| bytes.checked_add(size_of::<Self>()))
                .ok_or(Resource::Arithmetic)?;
            Ok((
                Self {
                    plan,
                    entry,
                    successors,
                    checked,
                },
                SourceSsaBoundaryStorageV31 { retained },
            ))
        };
        assert_eq!(std::mem::size_of_val(&run), size_of::<DeriveCapture<'_>>());
        assert_eq!(
            std::mem::align_of_val(&run),
            std::mem::align_of::<DeriveCapture<'_>>()
        );
        resources::scoped(budget, run)
    }

    /// Returns one checked live-in SSA name under the same original plan.
    /// The caller keeps this owner's returned retained storage paid.
    pub fn value(
        &self,
        plan: &Plan,
        block: Block,
        variable: Variable,
        budget: &mut Budget<'_>,
    ) -> Result<Value> {
        let capture = QueryCapture {
            owner: self,
            plan,
            block,
            variable,
        };
        let run = move |meter: &mut Meter<'_, '_>| {
            let (owner, plan, block, variable) = capture.parts();
            meter.reserve(boundary_query_headers_v31()?)?;
            meter.work(1)?;
            if !std::ptr::eq(owner.plan, plan) {
                return Err(Error::ForeignPlan);
            }
            owner.checked.value(block, variable, meter)
        };
        assert_eq!(
            std::mem::size_of_val(&run),
            size_of::<QueryCapture<'_, '_>>()
        );
        assert_eq!(
            std::mem::align_of_val(&run),
            std::mem::align_of::<QueryCapture<'_, '_>>()
        );
        resources::scoped(budget, run)
    }

    /// Original invocation entry, distinct from recurrent predecessors.
    pub const fn entry(&self) -> Block {
        self.entry
    }
    /// Exact complete ordered source topology borrowed for the equations.
    pub const fn successors(&self) -> &'source [Vec<Block>] {
        self.successors
    }
}

#[cfg(test)]
#[path = "source_ssa_boundaries_v31_tests.rs"]
mod tests;
