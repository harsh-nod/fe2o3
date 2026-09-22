//! One inert expanded history: the unchanged U prefix and exactly one scalar node.
use crate::{
    CheckedLoopUnrollHistoryV1 as UChecked, DecodedLoopUnrollHistoryV1 as UDecoded,
    InertLoopUnrollHistoryRefV1 as UFrame, LoopUnrollHistoryErrorV1, LoopUnrollHistoryWireErrorV1,
    MAX_LOOP_UNROLL_HISTORY_BYTES_V1, MAX_LOOP_UNROLL_HISTORY_STORAGE_V1,
    materialize_loop_unroll_history_v1, private_cell_promotion_resources_v1 as resources,
    read_loop_unroll_history_v1,
    scalar_fixed_point_history_decode_v1::{
        DecodedScalarFixedPointHistoryV1 as ScalarDecoded,
        ReplayedScalarFixedPointHistoryV1 as ScalarChecked,
        materialize_scalar_fixed_point_history_v1,
    },
    scalar_fixed_point_history_v1::{
        InertScalarFixedPointHistoryRefV1 as ScalarFrame, MAX_SCALAR_FIXED_POINT_HISTORY_BYTES_V1,
        ScalarFixedPointHistoryErrorV1, read_scalar_fixed_point_history_v1,
    },
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{error::Error as StdError, fmt, mem::size_of};

pub const EXPANDED_HISTORY_MAGIC_V1: [u8; 8] = *b"F2EPH1\0\0";
pub const EXPANDED_HISTORY_DOMAIN_V1: &[u8] = b"FE2O3/EXPANDED-HISTORY/V1\0";
pub const EXPANDED_HISTORY_POLICY_ID_V1: &[u8] = b"FE2O3/EXPANDED-PRODUCTION-POLICY/V1\0";
const HEADER: usize = 48;
const SCRATCH: usize = size_of::<[&[u8]; 2]>() + size_of::<[usize; 3]>();
/// Layer-local child-bound sum; publication independently intersects its own
/// unchanged capsule/slot limits. This does not promise every maximum can fit.
pub const MAX_EXPANDED_HISTORY_BYTES_V1: usize = HEADER
    .checked_add(MAX_LOOP_UNROLL_HISTORY_BYTES_V1)
    .expect("fixed U bound")
    .checked_add(MAX_SCALAR_FIXED_POINT_HISTORY_BYTES_V1)
    .expect("fixed scalar bound");

#[derive(Debug)]
pub enum ExpandedHistoryErrorV1 {
    Resource(Resource),
    Header,
    Length,
    Reserved,
    Limit,
    UnrollWire(LoopUnrollHistoryWireErrorV1),
    Unroll(LoopUnrollHistoryErrorV1),
    Scalar(ScalarFixedPointHistoryErrorV1),
    Panicked,
}
type Error = ExpandedHistoryErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(v: Resource) -> Self {
        Self::Resource(v)
    }
}
impl resources::ScopeError for Error {
    fn panicked() -> Self {
        Self::Panicked
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expanded history: {self:?}")
    }
}
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::UnrollWire(e) => Some(e),
            Self::Unroll(e) => Some(e),
            Self::Scalar(e) => Some(e),
            _ => None,
        }
    }
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or_else(|| Resource::Arithmetic.into())
}
fn configured(b: &Budget<'_>) -> Result<()> {
    if b.storage_limit() > MAX_LOOP_UNROLL_HISTORY_STORAGE_V1 {
        Err(Error::Limit)
    } else {
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpandedHistoryStorageV1(usize);
impl ExpandedHistoryStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Complete framing only. Both embedded frames borrow this exact backing.
pub struct InertExpandedHistoryRefV1<'w> {
    wire: &'w [u8],
    prefix: UFrame<'w>,
    scalar: ScalarFrame<'w>,
}
impl<'w> InertExpandedHistoryRefV1<'w> {
    pub const fn canonical_bytes(&self) -> &'w [u8] {
        self.wire
    }
    pub const fn prefix(&self) -> &UFrame<'w> {
        &self.prefix
    }
    pub const fn scalar(&self) -> &ScalarFrame<'w> {
        &self.scalar
    }
    pub fn output_bytes(&self) -> &'w [u8] {
        self.scalar.output_bytes()
    }
    /// Absolute terminal-graph directory range in this expanded wire. Complete
    /// semantic replay, not this framing-only locator, establishes the final role.
    pub fn final_graph_range(&self) -> std::ops::Range<usize> {
        let offset = HEADER + self.prefix.canonical_bytes().len();
        let range = self.scalar.final_graph_range();
        offset + range.start..offset + range.end
    }
    pub const fn policy_identity(&self) -> &'static [u8] {
        EXPANDED_HISTORY_POLICY_ID_V1
    }
    pub const fn storage(&self) -> ExpandedHistoryStorageV1 {
        ExpandedHistoryStorageV1(size_of::<Self>())
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
pub struct InertExpandedHistoryBytesV1 {
    bytes: Vec<u8>,
    storage: ExpandedHistoryStorageV1,
}
impl InertExpandedHistoryBytesV1 {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub const fn storage(&self) -> ExpandedHistoryStorageV1 {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
fn fields(wire: &[u8]) -> Result<[&[u8]; 2]> {
    if wire[..8] != EXPANDED_HISTORY_MAGIC_V1
        || wire[8..12] != [1, 0, 1, 0]
        || wire[12..16] != (HEADER as u32).to_le_bytes()
        || wire[24..26] != [1, 0]
    {
        return Err(Error::Header);
    }
    if wire[26..32] != [0; 6] {
        return Err(Error::Reserved);
    }
    let n = |at| -> Result<usize> {
        usize::try_from(u64::from_le_bytes(
            wire[at..at + 8].try_into().map_err(|_| Error::Length)?,
        ))
        .map_err(|_| Resource::Arithmetic.into())
    };
    if n(16)? != wire.len() {
        return Err(Error::Length);
    }
    let (u, scalar) = (n(32)?, n(40)?);
    if u == 0
        || u > MAX_LOOP_UNROLL_HISTORY_BYTES_V1
        || scalar == 0
        || scalar > MAX_SCALAR_FIXED_POINT_HISTORY_BYTES_V1
    {
        return Err(Error::Limit);
    }
    let end = add(HEADER, u)?;
    if add(end, scalar)? != wire.len() {
        return Err(Error::Length);
    }
    Ok([
        wire.get(HEADER..end).ok_or(Error::Length)?,
        wire.get(end..).ok_or(Error::Length)?,
    ])
}
/// Preserves both child domains/readers and their exact typed refusals. Caller
/// prepays wire; success transfers just the enclosing fixed header unreserved.
pub fn read_expanded_history_v1<'w>(
    wire: &'w [u8],
    budget: &mut Budget<'_>,
) -> Result<InertExpandedHistoryRefV1<'w>> {
    configured(budget)?;
    if wire.len() > MAX_EXPANDED_HISTORY_BYTES_V1 {
        return Err(Error::Limit);
    }
    if wire.len() < HEADER {
        return Err(Error::Length);
    }
    if budget.storage() < wire.len() {
        return Err(Resource::Accounting.into());
    }
    resources::scoped(budget, |meter| {
        meter.reserve(add(size_of::<InertExpandedHistoryRefV1<'_>>(), SCRATCH)?)?;
        meter.work(HEADER)?;
        let [u, scalar] = fields(wire)?;
        let prefix =
            meter.derive(|b| read_loop_unroll_history_v1(u, b).map_err(Error::UnrollWire))?;
        meter.reserve(prefix.storage().retained_storage())?;
        let scalar = meter
            .derive(|b| read_scalar_fixed_point_history_v1(scalar, b).map_err(Error::Scalar))?;
        meter.reserve(scalar.storage().retained_storage())?;
        meter.work(1)?;
        Ok(InertExpandedHistoryRefV1 {
            wire,
            prefix,
            scalar,
        })
    })
}
/// Inert canonical composition, not a live execution/source join. Separate
/// child byte backing and frame headers stay prepaid. No child is rewritten.
pub fn encode_expanded_history_v1(
    prefix: &UFrame<'_>,
    scalar: &ScalarFrame<'_>,
    budget: &mut Budget<'_>,
) -> Result<InertExpandedHistoryBytesV1> {
    configured(budget)?;
    let minimum = add(
        add(
            prefix.canonical_bytes().len(),
            prefix.storage().retained_storage(),
        )?,
        add(
            scalar.canonical_bytes().len(),
            scalar.storage().retained_storage(),
        )?,
    )?;
    if budget.storage() < minimum {
        return Err(Resource::Accounting.into());
    }
    resources::scoped(budget, |meter| {
        meter.reserve(add(size_of::<InertExpandedHistoryBytesV1>(), SCRATCH)?)?;
        meter.work(3)?;
        let lengths = [
            prefix.canonical_bytes().len(),
            scalar.canonical_bytes().len(),
        ];
        let total = add(HEADER, add(lengths[0], lengths[1])?)?;
        if total > MAX_EXPANDED_HISTORY_BYTES_V1 {
            return Err(Error::Limit);
        }
        let (mut bytes, capacity) = meter.table::<u8>(total)?;
        meter.work(total)?;
        bytes.resize(total, 0);
        meter.work(total)?;
        bytes[..8].copy_from_slice(&EXPANDED_HISTORY_MAGIC_V1);
        bytes[8..12].copy_from_slice(&[1, 0, 1, 0]);
        bytes[12..16].copy_from_slice(&(HEADER as u32).to_le_bytes());
        bytes[16..24].copy_from_slice(
            &u64::try_from(total)
                .map_err(|_| Resource::Arithmetic)?
                .to_le_bytes(),
        );
        bytes[24..26].copy_from_slice(&1u16.to_le_bytes());
        for (i, n) in lengths.into_iter().enumerate() {
            bytes[32 + i * 8..40 + i * 8].copy_from_slice(
                &u64::try_from(n)
                    .map_err(|_| Resource::Arithmetic)?
                    .to_le_bytes(),
            );
        }
        bytes[HEADER..HEADER + lengths[0]].copy_from_slice(prefix.canonical_bytes());
        bytes[HEADER + lengths[0]..].copy_from_slice(scalar.canonical_bytes());
        let frame = meter.derive(|b| read_expanded_history_v1(&bytes, b))?;
        let paid = frame.storage().retained_storage();
        meter.reserve(paid)?;
        meter.work(1)?;
        drop(frame);
        meter.release(paid)?;
        Ok(InertExpandedHistoryBytesV1 {
            bytes,
            storage: ExpandedHistoryStorageV1(add(
                size_of::<InertExpandedHistoryBytesV1>(),
                capacity,
            )?),
        })
    })
}
/// Owning U and all scalar graphs, with short independent receipts kept separate.
/// ```compile_fail
/// use fe2o3_kernel_opt::DecodedExpandedHistoryV1;
/// fn forge<'a>() -> DecodedExpandedHistoryV1<'a, 'a> { Default::default() }
/// ```
pub struct DecodedExpandedHistoryV1<'f, 'w> {
    frame: &'f InertExpandedHistoryRefV1<'w>,
    prefix: UDecoded<'f, 'w>,
    scalar: ScalarDecoded<'f, 'w>,
    storage: ExpandedHistoryStorageV1,
}
impl<'f, 'w> DecodedExpandedHistoryV1<'f, 'w> {
    pub const fn frame(&self) -> &'f InertExpandedHistoryRefV1<'w> {
        self.frame
    }
    pub const fn prefix(&self) -> &UDecoded<'f, 'w> {
        &self.prefix
    }
    pub const fn scalar(&self) -> &ScalarDecoded<'f, 'w> {
        &self.scalar
    }
    pub fn output(&self) -> &Owner {
        self.scalar.output()
    }
    pub const fn storage(&self) -> ExpandedHistoryStorageV1 {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    /// Complete original B..U relation followed by the exact U/scalar join.
    /// This deliberately does not authenticate source or fixed production U settings.
    pub fn check_semantics<'a>(
        &'a self,
        budget: &mut Budget<'_>,
    ) -> Result<ReplayedExpandedHistoryV1<'a, 'f, 'w>> {
        configured(budget)?;
        let required = add(
            self.storage.0,
            add(self.frame.storage().0, self.frame.canonical_bytes().len())?,
        )?;
        if budget.storage() < required {
            return Err(Resource::Accounting.into());
        }
        let inherited = budget.storage();
        resources::scoped(budget, |meter| {
            let mut retained = size_of::<ReplayedExpandedHistoryV1<'_, '_, '_>>();
            meter.reserve(retained)?;
            let prefix = meter.derive(|b| self.prefix.check_semantics(b).map_err(Error::Unroll))?;
            let paid = prefix.storage().retained_storage();
            meter.reserve(paid)?;
            retained = add(retained, paid)?;
            let scalar = meter.derive(|b| {
                self.scalar
                    .check_semantics(prefix.output(), b)
                    .map_err(Error::Scalar)
            })?;
            let paid = scalar.storage().retained_storage();
            meter.reserve(paid)?;
            retained = add(retained, paid)?;
            meter.work(1)?;
            Ok(ReplayedExpandedHistoryV1 {
                prefix,
                scalar,
                inherited,
                storage: ExpandedHistoryStorageV1(retained),
            })
        })
    }
}
/// No source anchor, ABI, nominal ownership, native or publication grant.
/// ```compile_fail
/// use fe2o3_kernel_opt::{DecodedExpandedHistoryV1 as D, ReplayedExpandedHistoryV1 as R};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
/// fn escape<'f, 'w>(v: D<'f, 'w>, b: &mut B<'_>) -> R<'static, 'f, 'w> { v.check_semantics(b).unwrap() }
/// ```
pub struct ReplayedExpandedHistoryV1<'a, 'f, 'w> {
    prefix: UChecked<'a>,
    scalar: ScalarChecked<'a, 'f, 'w>,
    inherited: usize,
    storage: ExpandedHistoryStorageV1,
}
impl<'a, 'f, 'w> ReplayedExpandedHistoryV1<'a, 'f, 'w> {
    pub const fn prefix(&self) -> &UChecked<'a> {
        &self.prefix
    }
    pub const fn scalar(&self) -> &ScalarChecked<'a, 'f, 'w> {
        &self.scalar
    }
    pub fn output(&self) -> &'a Owner {
        self.scalar.output()
    }
    pub const fn storage(&self) -> ExpandedHistoryStorageV1 {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    pub fn replay(&self, budget: &mut Budget<'_>) -> Result<()> {
        configured(budget)?;
        if budget.storage() < add(self.inherited, self.storage.0)? {
            return Err(Resource::Accounting.into());
        }
        resources::scoped(budget, |meter| {
            meter.work(3)?;
            let expected = add(
                size_of::<Self>(),
                add(
                    self.prefix.storage().retained_storage(),
                    self.scalar.storage().retained_storage(),
                )?,
            )?;
            if expected != self.storage.0 {
                return Err(Resource::Accounting.into());
            }
            meter.derive(|b| self.prefix.replay(b).map_err(Error::Unroll))?;
            meter.derive(|b| self.scalar.replay(b).map_err(Error::Scalar))?;
            Ok(())
        })
    }
}
/// Child receipts and actual capacities coexist conservatively; no embedded
/// header overlap or equal graph role is converted into a storage credit.
pub fn materialize_expanded_history_v1<'f, 'w>(
    frame: &'f InertExpandedHistoryRefV1<'w>,
    budget: &mut Budget<'_>,
) -> Result<DecodedExpandedHistoryV1<'f, 'w>> {
    configured(budget)?;
    if budget.storage() < add(frame.storage().0, frame.canonical_bytes().len())? {
        return Err(Resource::Accounting.into());
    }
    resources::scoped(budget, |meter| {
        let mut retained = size_of::<DecodedExpandedHistoryV1<'_, '_>>();
        meter.reserve(retained)?;
        let prefix = meter.derive(|b| {
            materialize_loop_unroll_history_v1(frame.prefix(), b).map_err(Error::UnrollWire)
        })?;
        let paid = prefix.storage().retained_storage();
        meter.reserve(paid)?;
        retained = add(retained, paid)?;
        let scalar = meter.derive(|b| {
            materialize_scalar_fixed_point_history_v1(frame.scalar(), b).map_err(Error::Scalar)
        })?;
        let paid = scalar.storage().retained_storage();
        meter.reserve(paid)?;
        retained = add(retained, paid)?;
        meter.work(1)?;
        Ok(DecodedExpandedHistoryV1 {
            frame,
            prefix,
            scalar,
            storage: ExpandedHistoryStorageV1(retained),
        })
    })
}

#[cfg(test)]
#[path = "expanded_history_v1_tests.rs"]
mod tests;
