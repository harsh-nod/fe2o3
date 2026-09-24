//! Owning, opt-in, bounded literal-loop cloning. No source/native/default authority.
use crate::private_cell_promotion_resources_v1 as resources;
use fe2o3_kernel_analysis::{
    CanonicalKirGuardDistanceV1 as Distance, CanonicalKirGuardedUpdateV1 as Update,
    CanonicalKirInductionErrorV1 as FactsError, CanonicalKirInductionFactsV1 as Facts,
    CanonicalKirInductionOutcomeV1 as Outcome, CanonicalKirInventoryErrorV1 as InventoryError,
    CanonicalKirInventoryV1 as Inventory, CanonicalKirIterationScopeV1 as Completion,
    CanonicalKirLoopErrorV1 as LoopError, CanonicalKirLoopUnrollCopyV1 as CopyRole,
    CanonicalKirLoopUnrollErrorV1 as PairError, CanonicalKirLoopUnrollLimitsV1 as Limits,
    CanonicalKirLoopUnrollOriginV1 as Row, CanonicalKirLoopUnrollOriginsV1 as Origins,
    CanonicalKirLoopUnrollSelectionV1 as Selection, CanonicalKirLoopUnrollStorageV1 as PairStorage,
    CanonicalKirLoopsV1 as Loops, CheckedCanonicalKirLoopUnrollPairV1 as Pair,
    check_canonical_kir_loop_unroll_pair_v1 as check_pair,
};
use fe2o3_kernel_ir::{
    AddressSpace, BasicBlock, BlockId, CanonicalKernelIrReplayStorageV12 as OwnerStorage,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirEdgeArgumentCoordinateV1 as Argument, CanonicalKirEdgeCoordinateV1 as Edge,
    CanonicalKirOperationCoordinateV1 as Site, CastKind, Module, Operation, OperationKind,
    ScalarType, Terminator, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrIdentityV12 as Identity, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{fmt, mem::size_of};

#[derive(Debug)]
pub enum OwnedLoopUnrollErrorV1 {
    Resource(Resource),
    Inventory(InventoryError),
    Loops(LoopError),
    Facts(FactsError),
    Admission(fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12),
    Pair(PairError),
    InvalidIterationLimit(u8),
    OutputLimit {
        kind: &'static str,
        actual: usize,
        limit: usize,
    },
    ForeignInput,
    LimitsMismatch,
    Recipe(&'static str),
    Panicked,
}
type Error = OwnedLoopUnrollErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Meter<'a, 'w> = resources::Meter<'a, 'w, Error>;
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<InventoryError> for Error {
    fn from(e: InventoryError) -> Self {
        Self::Inventory(e)
    }
}
impl From<LoopError> for Error {
    fn from(e: LoopError) -> Self {
        Self::Loops(e)
    }
}
impl From<FactsError> for Error {
    fn from(e: FactsError) -> Self {
        Self::Facts(e)
    }
}
impl From<PairError> for Error {
    fn from(e: PairError) -> Self {
        Self::Pair(e)
    }
}
impl From<fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12> for Error {
    fn from(e: fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12) -> Self {
        Self::Admission(e)
    }
}
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "owning loop unroll: {self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnedLoopUnrollStorageV1(usize);
impl OwnedLoopUnrollStorageV1 {
    pub fn retained_storage(self) -> usize {
        self.0
    }
}

#[derive(Debug)]
struct Rows {
    selection: Option<Selection>,
    blocks: Vec<Row<Block>>,
    definitions: Vec<Row<Definition>>,
    operations: Vec<Row<Site>>,
    terminators: Vec<Row<Block>>,
    edges: Vec<Row<Edge>>,
    arguments: Vec<Row<Argument>>,
}
impl Rows {
    fn view(&self) -> Origins<'_> {
        Origins {
            selection: self.selection,
            blocks: &self.blocks,
            definitions: &self.definitions,
            operations: &self.operations,
            terminators: &self.terminators,
            edges: &self.edges,
            arguments: &self.arguments,
        }
    }
    fn backing(&self) -> Result<usize> {
        [
            (self.blocks.capacity(), size_of::<Row<Block>>()),
            (self.definitions.capacity(), size_of::<Row<Definition>>()),
            (self.operations.capacity(), size_of::<Row<Site>>()),
            (self.terminators.capacity(), size_of::<Row<Block>>()),
            (self.edges.capacity(), size_of::<Row<Edge>>()),
            (self.arguments.capacity(), size_of::<Row<Argument>>()),
        ]
        .into_iter()
        .try_fold(0usize, |n, (c, s)| {
            n.checked_add(c.checked_mul(s).ok_or(Resource::Arithmetic)?)
                .ok_or(Resource::Arithmetic.into())
        })
    }
}

/// A move-only actual admitted result plus complete inert lineage. Reserve its
/// UNRESERVED addition before further controlled work; drop before releasing.
/// The domain covers this header, origin Vec actual capacities, fresh admitted
/// owner storage, all temporary headers/backing and cloned scalar Type boxes.
/// It excludes borrowed input ownership, allocator metadata and process RSS.
/// Existing copy/admission services retain their documented accounting domain.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::OwnedLoopUnrollV1;
/// fn cannot_clone(owner: OwnedLoopUnrollV1) { let _ = owner.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::OwnedLoopUnrollV1;
/// fn rows_private(owner: OwnedLoopUnrollV1) { let _ = owner.rows; }
/// ```
#[derive(Debug)]
pub struct OwnedLoopUnrollV1 {
    output: Owner,
    output_storage: OwnerStorage,
    input: Identity,
    rows: Rows,
    limits: Limits,
    retained: usize,
}
impl OwnedLoopUnrollV1 {
    pub fn output(&self) -> &Owner {
        &self.output
    }
    pub fn origins(&self) -> Origins<'_> {
        self.rows.view()
    }
    pub fn limits(&self) -> Limits {
        self.limits
    }
    pub fn grants_authority(&self) -> bool {
        false
    }
    pub fn replay<'i, 'o>(
        &'o self,
        input: &'i Owner,
        limits: Limits,
        budget: &mut Budget<'_>,
    ) -> Result<(Pair<'i, 'o, 'o>, PairStorage)> {
        if budget.storage() < self.retained {
            return Err(Resource::Accounting.into());
        }
        resources::scoped(budget, |m| {
            m.work(11)?;
            if limits != self.limits {
                return Err(Error::LimitsMismatch);
            }
            m.work(size_of::<Identity>() + 3)?;
            if input.canonical().identity() != &self.input {
                return Err(Error::ForeignInput);
            }
            if retained(self.output_storage, &self.rows)? != self.retained {
                return Err(Resource::Accounting.into());
            }
            m.derive(|b| {
                Ok(check_pair(
                    input,
                    &self.output,
                    self.rows.view(),
                    limits,
                    b,
                )?)
            })
        })
    }
}
fn header() -> Result<usize> {
    size_of::<OwnedLoopUnrollV1>()
        .checked_sub(size_of::<Owner>())
        .ok_or(Resource::Arithmetic.into())
}
fn retained(output: OwnerStorage, rows: &Rows) -> Result<usize> {
    let backing = rows.backing()?;
    header()?
        .checked_add(output.retained_storage())
        .and_then(|n| n.checked_add(backing))
        .ok_or(Resource::Arithmetic.into())
}

/// Selects the first qualifying actual literal fact, never source spelling.
/// Unsupported graphs are exact no-ops; invalid limits, exhaustion and output
/// growth remain typed refusals. Clones at most eight body iterations. The
/// independent checker verifies the final actual owner before it is returned.
pub fn unroll_canonical_kir_loops_v1(
    input: &Owner,
    limits: Limits,
    budget: &mut Budget<'_>,
) -> Result<(OwnedLoopUnrollV1, OwnedLoopUnrollStorageV1)> {
    resources::scoped(budget, |m| {
        m.work(11)?;
        if limits.max_iterations > 8 {
            return Err(Error::InvalidIterationLimit(limits.max_iterations));
        }
        m.reserve(header()?)?;
        let (i, is) = m.derive(|b| Ok(Inventory::derive(input, b)?))?;
        m.reserve(is.retained_storage())?;
        let (l, ls) = m.derive(|b| Ok(Loops::derive(&i, limits.loops, b)?))?;
        m.reserve(ls.retained_storage())?;
        let (f, fs) = m.derive(|b| Ok(Facts::derive(&l, limits.loops, b)?))?;
        m.reserve(fs.retained_storage())?;
        m.derive(|b| Ok(f.replay(&l, limits.loops, b)?))?;
        let (plan, plan_size) = selection::plan(&i, &f, limits, m)?;
        let (mut candidate, cs) = m.derive(|b| Ok(input.copy_module_for_transformation_v12(b)?))?;
        m.reserve(cs.retained_storage())?;
        let (rows, extra) = build::materialize(&i, &f, &plan, &mut candidate, m)?;
        let (output, os) = m.derive(|b| {
            Ok(Owner::from_module_ref_with_verification_budget_v12(
                &candidate, b,
            )?)
        })?;
        m.reserve(os.retained_storage())?;
        let ps = {
            let (_pair, ps) =
                m.derive(|b| Ok(check_pair(input, &output, rows.view(), limits, b)?))?;
            m.reserve(ps.retained_storage())?;
            ps
        };
        m.release(ps.retained_storage())?;
        let bytes = retained(os, &rows)?;
        // Prepaid destruction covers every bounded candidate descendant/type.
        m.work(
            extra
                .checked_add(input.canonical().canonical_bytes().len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        drop(candidate);
        m.release(
            cs.retained_storage()
                .checked_add(extra)
                .ok_or(Resource::Arithmetic)?,
        )?;
        drop(plan);
        m.release(plan_size)?;
        drop(f);
        m.release(fs.retained_storage())?;
        drop(l);
        m.release(ls.retained_storage())?;
        drop(i);
        m.release(is.retained_storage())?;
        m.work(1)?;
        Ok((
            OwnedLoopUnrollV1 {
                output,
                output_storage: os,
                input: *input.canonical().identity(),
                rows,
                limits,
                retained: bytes,
            },
            OwnedLoopUnrollStorageV1(bytes),
        ))
    })
}

#[path = "loop_unroll_build_v1.rs"]
mod build;
#[cfg(test)]
#[path = "loop_unroll_fixture_v1_tests.rs"]
mod fixture;
#[cfg(test)]
#[path = "loop_unroll_resources_v1_tests.rs"]
mod resource_tests;
#[path = "loop_unroll_selection_v1.rs"]
mod selection;
#[cfg(test)]
#[path = "loop_unroll_sim_v1_tests.rs"]
mod sim_tests;
#[cfg(test)]
#[path = "owned_loop_unroll_v1_tests.rs"]
mod tests;
