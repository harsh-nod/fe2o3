//! Owning same-frame inputs for the unchanged borrowed Policy8 semantic checker.
use crate::{
    AdmittedPolicy8HistoryGraphPoolV1 as Pool, CanonicalPolicy5SemanticInputsV1,
    CanonicalPolicy6ContinuationClaimsV1, CanonicalPolicy6SemanticInputsV1,
    CanonicalPolicy7SemanticErrorV1, CanonicalPolicy7SemanticInputsV1,
    CanonicalPolicy8CompositionErrorV1 as SemanticError, CanonicalPolicy8ContinuationClaimsV1,
    CanonicalPolicy8GraphPoolErrorV1, CanonicalPolicy8HistoryRoleV1 as Role,
    CanonicalPolicy8SemanticInputsV1, DecodedCanonicalPolicy7RowsV1 as Policy7Rows,
    InertPolicy8HistoryRefV1 as Frame, POLICY8_COMMUTATIVE_PASS_NAME_V1,
    ReplayedPolicy8SemanticRelationV1, admit_policy8_history_graph_pool_v1,
    check_published_policy8_semantic_relation_v1, decode_canonical_policy7_rows_v1,
};
use fe2o3_kernel_analysis::CanonicalKirLoadForwardingRowV1 as LoadRow;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKirBlockCoordinateV1,
    CanonicalKirFunctionCoordinateV1, CanonicalKirOperationCoordinateV1 as Coordinate,
    CanonicalKirTransitionReceiptErrorV1, InertCanonicalKirTransitionGraphIdentityV1 as Identity,
    InertOwnedCanonicalKirOccurrenceRowsV1 as TailRows,
    VerifiedCanonicalKernelIrModuleV12 as Owner, materialize_canonical_kir_occurrence_rows_v1,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

// Conservative fixed metadata work: seven graph lookups, record/row views,
// eighty J/K identity bytes and bounded nested input construction.
const SEMANTIC_INPUT_WORK: usize = 128;

/// Same-frame owned-input construction failure, not a semantic verdict.
#[derive(Debug)]
pub enum CanonicalPolicy8HistoryInputsErrorV1 {
    /// Cumulative work/storage, allocation or cleanup refusal.
    Resource(Resource),
    /// Actual fresh graph-pool admission failed.
    Graphs(CanonicalPolicy8GraphPoolErrorV1),
    /// Existing P7 row decoder refused its actual same-frame record.
    /// Boxed diagnostic backing is outside the returned input-owner ledger.
    Policy7Rows(Box<CanonicalPolicy7SemanticErrorV1>),
    /// Existing neutral-row materializer refused the actual tail view.
    TailRows(CanonicalKirTransitionReceiptErrorV1),
    /// A supposedly framed P5 row extent was inconsistent.
    LoadRowExtent,
    /// Local owned construction unwound before cleanup.
    Panicked,
}
type Error = CanonicalPolicy8HistoryInputsErrorV1;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Policy8 decoded history inputs: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Complete new header, P5 capacity and full opaque pool/P7/P8 receipts.
/// Returned unreserved with explicit conservative embedded-header overlap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalPolicy8HistoryInputsStorageV1(usize);
impl CanonicalPolicy8HistoryInputsStorageV1 {
    /// Reserve while inputs live before any subsequent controlled allocation.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Move-only materialized input backing from one immutable inert frame.
/// Fresh graph admission and row syntax are established, but no transformation
/// preservation, executed policy, source/native proof or publication authority.
/// A full semantic receipt is returned only by borrowing `check_semantics`;
/// it is never stored inside this owner or converted into an execution owner.
///
/// ```compile_fail
/// use fe2o3_kernel_opt::DecodedPolicy8HistoryInputsV1;
/// fn duplicate(value: DecodedPolicy8HistoryInputsV1<'_, '_, '_>) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::{DecodedPolicy8HistoryInputsV1 as Inputs, InertPolicy8HistoryRefV1 as Frame,
///     materialize_policy8_history_inputs_v1 as materialize};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn escape<'w, 'k>(frame: Frame<'w, 'k>, b: &mut Budget<'_>) -> Inputs<'static, 'w, 'k> {
///     materialize(&frame, b).unwrap()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::DecodedPolicy8HistoryInputsV1 as Inputs;
/// fn wire<'f, 'w, 'k>(value: Inputs<'f, 'w, 'k>) -> Inputs<'f, 'static, 'k> { value }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::DecodedPolicy8HistoryInputsV1 as Inputs;
/// fn output<'f, 'w, 'k>(value: Inputs<'f, 'w, 'k>) -> Inputs<'f, 'w, 'static> { value }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::{DecodedPolicy8HistoryInputsV1 as Inputs, ReplayedPolicy8SemanticRelationV1 as Receipt};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn escape<'a>(value: Inputs<'_, '_, '_>, budget: &mut Budget<'_>) -> Receipt<'a> {
///     value.check_semantics(budget).unwrap()
/// }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_opt::DecodedPolicy8HistoryInputsV1 as Inputs;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn moved(value: Inputs<'_, '_, '_>, budget: &mut Budget<'_>) {
///     let receipt = value.check_semantics(budget).unwrap();
///     drop(value);
///     let _ = receipt.output();
/// }
/// ```
pub struct DecodedPolicy8HistoryInputsV1<'frame, 'wire, 'k> {
    pool: Pool<'frame, 'wire, 'k>,
    load_rows: Vec<LoadRow>,
    policy7_rows: Policy7Rows<'wire>,
    tail_rows: TailRows,
    storage: CanonicalPolicy8HistoryInputsStorageV1,
}
impl<'frame, 'wire, 'k> DecodedPolicy8HistoryInputsV1<'frame, 'wire, 'k> {
    /// The one original inert frame; nested record contents are still claims.
    pub const fn frame(&self) -> &'frame Frame<'wire, 'k> {
        self.pool.frame()
    }
    /// Actual admitted role subject, including exact external K aliases.
    pub fn graph(&self, role: Role) -> &Owner {
        self.pool.graph(role)
    }
    /// Exact external typed K, not a new owner reconstructed from a digest.
    pub const fn external_output(&self) -> &'k Owner {
        self.pool.external_output()
    }
    /// Complete conservative logical transfer returned unreserved.
    pub const fn storage(&self) -> CanonicalPolicy8HistoryInputsStorageV1 {
        self.storage
    }
    /// Materialized backing alone does not prove a transformation relation.
    pub const fn proves_semantic_preservation(&self) -> bool {
        false
    }
    /// The owner does not authenticate an executed schedule or producer.
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    /// Source, native, proof, publication and launch authority remain absent.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    fn semantic_inputs(&self) -> CanonicalPolicy8SemanticInputsV1<'_> {
        let frame = self.pool.frame();
        let j = self.pool.graph(Role::J);
        let k = self.pool.external_output();
        CanonicalPolicy8SemanticInputsV1 {
            prefix: CanonicalPolicy7SemanticInputsV1 {
                prefix: CanonicalPolicy6SemanticInputsV1 {
                    prefix: CanonicalPolicy5SemanticInputsV1 {
                        input: self.pool.graph(Role::B),
                        intermediate: self.pool.graph(Role::C),
                        stored: self.pool.graph(Role::S),
                        output: self.pool.graph(Role::O),
                        policy4_wire: frame.policy4_wire(),
                        policy5_record: frame.policy5_record(),
                        load_rows: &self.load_rows,
                    },
                    output: self.pool.graph(Role::I),
                    continuation: CanonicalPolicy6ContinuationClaimsV1 {
                        composition_record: frame.unvalidated_policy6_record(),
                        integer_record: frame.integer_record(),
                        transition_wire: frame.transition_wire(),
                    },
                },
                output: j,
                continuation: self.policy7_rows.claims(),
            },
            output: k,
            continuation: CanonicalPolicy8ContinuationClaimsV1 {
                pass_name: POLICY8_COMMUTATIVE_PASS_NAME_V1,
                input: Identity::from_verified(j.canonical().identity()),
                output: Identity::from_verified(k.canonical().identity()),
                occurrences: self.tail_rows.candidate(),
            },
        }
    }

    #[cfg(test)]
    pub(crate) fn test_components(&self) -> (CanonicalPolicy8SemanticInputsV1<'_>, [usize; 5]) {
        (
            self.semantic_inputs(),
            [
                size_of::<Self>(),
                self.load_rows.capacity() * size_of::<LoadRow>(),
                self.pool.storage().retained_storage(),
                self.policy7_rows.storage().retained_storage(),
                self.tail_rows.storage().retained_storage(),
            ],
        )
    }

    /// Calls the existing complete P8 composition exactly once, on this caller's
    /// cumulative Budget, with no optimizer, duplicate prefix decoder or graph
    /// admission. P4/O-I and all nested record/row semantics retain their existing
    /// error and charge order after one fixed 128-unit typed-input assembly debit.
    ///
    /// The input owner's complete transfer must already remain reserved. The
    /// numeric lower-bound check is necessary accounting, NOT proof of reservation
    /// provenance or the construction ledger. Borrowed external backing remains
    /// separately reserved once or external. The unchanged checker binds this
    /// call's full actual entry floor/original Work and preserves denial history.
    /// Its returned receipt borrows self and is UNRESERVED under its existing
    /// contract; reserve before later controlled allocation and drop before refund.
    /// This returns semantic evidence only, never execution or source authority.
    pub fn check_semantics<'a>(
        &'a self,
        budget: &mut Budget<'_>,
    ) -> Result<ReplayedPolicy8SemanticRelationV1<'a>, SemanticError> {
        if budget.storage() < self.storage.retained_storage() {
            return Err(Resource::Accounting.into());
        }
        budget.charge_work(SEMANTIC_INPUT_WORK)?;
        check_published_policy8_semantic_relation_v1(self.semantic_inputs(), budget)
    }
}

fn scoped<'work, T>(
    budget: &mut Budget<'work>,
    run: impl FnOnce(&mut Budget<'work>) -> Result<T, Error>,
) -> Result<T, Error> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(Error::Panicked)
        }
    };
    let cleanup = if ledger != budget.work_ledger_identity_v1() {
        Err(Resource::Accounting)
    } else {
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)
            .and_then(|bytes| budget.release_storage(bytes))
    };
    if let Err(error) = cleanup {
        let rejected = std::mem::replace(&mut result, Err(error.into()));
        payloads[1] = catch_unwind(AssertUnwindSafe(|| drop(rejected))).err();
    }
    drop(payloads);
    result
}

fn coordinate(bytes: &[u8]) -> Coordinate {
    let word = |at| u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    Coordinate {
        block: CanonicalKirBlockCoordinateV1 {
            function: CanonicalKirFunctionCoordinateV1(word(0)),
            block: word(4),
        },
        operation: word(8),
    }
}

// The immutable frame has already checked count, complete extent and order.
// This materializes its six-word rows; it accepts no independent raw input.
fn load_rows(
    frame: &Frame<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<(Vec<LoadRow>, usize), Error> {
    budget.charge_work(1)?;
    let bytes = frame.load_row_bytes();
    let body = bytes.get(4..).ok_or(Error::LoadRowExtent)?;
    if body.len() % 24 != 0 {
        return Err(Error::LoadRowExtent);
    }
    let count = body.len() / 24;
    let requested = count
        .checked_mul(size_of::<LoadRow>())
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(
        body.len()
            .checked_add(requested)
            .and_then(|n| n.checked_add(count))
            .ok_or(Resource::Arithmetic)?,
    )?;
    budget.reserve_storage(requested)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let capacity = rows
        .capacity()
        .checked_mul(size_of::<LoadRow>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(
        capacity
            .checked_sub(requested)
            .ok_or(Resource::Accounting)?,
    )?;
    for bytes in body.chunks_exact(24) {
        rows.push(LoadRow {
            first: coordinate(&bytes[..12]),
            load: coordinate(&bytes[12..]),
        });
    }
    Ok((rows, capacity))
}

/// Materializes all graph/typed-row backing from one and only one inert frame.
/// Order is fresh pool admission once, P5 materialization, existing P7 decoder,
/// then existing owned neutral P8 materializer. Prefix semantics remain unchecked.
/// The subsequent borrowing method reuses the unchanged full semantic checker.
///
/// Prepay the complete wrapper and each observed new P5 capacity. Immediately
/// re-reserve every complete opaque pool/P7/P8 receipt before further controlled
/// allocation; no embedded header credit is inferred. The conservative return is
/// wrapper + P5 backing + full pool/P7/P8 receipts, UNRESERVED. All borrowed frame,
/// wire and K backing remains caller-owned/external or prepaid once. Existing
/// logical decoder/B-tree and transferred diagnostic exclusions are unchanged;
/// this is not RSS or diagnostic-retention accounting and increases no cap.
///
/// Partial owners and scratch drop before cleanup to the intact original Work's
/// full entry floor. Work/peak/first denial remain cumulative. Invalid floor or
/// foreign Work is refused without an unauthorized refund; deferred panic-payload
/// destruction follows the cleanup decision. No public callback is introduced.
pub fn materialize_policy8_history_inputs_v1<'frame, 'wire, 'k>(
    frame: &'frame Frame<'wire, 'k>,
    budget: &mut Budget<'_>,
) -> Result<DecodedPolicy8HistoryInputsV1<'frame, 'wire, 'k>, Error> {
    scoped(budget, |budget| {
        let floor = budget.storage();
        let header = size_of::<DecodedPolicy8HistoryInputsV1<'_, '_, '_>>();
        budget.reserve_storage(header)?;
        let pool = admit_policy8_history_graph_pool_v1(frame, budget).map_err(Error::Graphs)?;
        let pool_storage = pool.storage().retained_storage();
        budget.reserve_storage(pool_storage)?;
        let (load_rows, load_storage) = load_rows(frame, budget)?;
        let policy7_rows = decode_canonical_policy7_rows_v1(frame.policy7_record(), budget)
            .map_err(|error| Error::Policy7Rows(Box::new(error)))?;
        let policy7_storage = policy7_rows.storage().retained_storage();
        budget.reserve_storage(policy7_storage)?;
        let (tail_rows, tail_storage) =
            materialize_canonical_kir_occurrence_rows_v1(frame.tail_rows(), budget)
                .map_err(Error::TailRows)?;
        budget.reserve_storage(tail_storage.retained_storage())?;
        let retained = header
            .checked_add(pool_storage)
            .and_then(|n| n.checked_add(load_storage))
            .and_then(|n| n.checked_add(policy7_storage))
            .and_then(|n| n.checked_add(tail_storage.retained_storage()))
            .ok_or(Resource::Arithmetic)?;
        if budget.storage().checked_sub(floor) != Some(retained) {
            return Err(Resource::Accounting.into());
        }
        Ok(DecodedPolicy8HistoryInputsV1 {
            pool,
            load_rows,
            policy7_rows,
            tail_rows,
            storage: CanonicalPolicy8HistoryInputsStorageV1(retained),
        })
    })
}

#[cfg(test)]
#[path = "checked_optimization_policy8_history_inputs_resource_v1_tests.rs"]
mod resource_tests;
