//! Inert reader of the existing fixed integer/DCE execution record. No map,
//! report or sealed execution owner is reconstructed from numeric claims.

use crate::{
    INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1,
    fixed_integer_continuation_v1::INTEGER_CONTINUATION_PASSES,
    fixed_policy_v3::{
        POLICY3_CANONICAL_CAP, POLICY3_GRAPH_CAP, POLICY3_MAX_PASSES, POLICY3_SESSION_WORK_CAP,
        pass_tag,
    },
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};

/// Closed syntax/endpoint failures, not an execution authentication result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegerContinuationClaimErrorV1 {
    Framing,
    Endpoint,
    Profile,
    Pass,
    Resource(Resource),
}
type Error = IntegerContinuationClaimErrorV1;
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unauthenticated integer continuation claim: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Borrowed syntax only. Dynamic work, epochs, map digest and graph/report
/// fields are not authenticated observations or resource allowances.
///
/// ```compile_fail
/// use fe2o3_pliron::{UnauthenticatedIntegerContinuationClaimV1 as Claim,
///     IntegerContinuationExecutionWitnessV1 as Witness};
/// fn not_execution(claim: Claim<'_>) -> Witness { claim }
/// ```
pub struct UnauthenticatedIntegerContinuationClaimV1<'wire> {
    bytes: &'wire [u8; INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1],
}
impl UnauthenticatedIntegerContinuationClaimV1<'_> {
    pub const fn canonical_bytes(&self) -> &[u8; INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1] {
        self.bytes
    }
    /// The declared observed-map digest is not an F2NTR receipt identity.
    pub fn declared_map_digest(&self) -> &[u8; 32] {
        self.bytes[264..296].try_into().expect("fixed record field")
    }
    pub fn declared_profile_work(&self) -> u64 {
        u64::from_le_bytes(self.bytes[96..104].try_into().expect("fixed record field"))
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Read exactly the existing 416 bytes without running optimization. Two
/// framing units precede length conversion; 416 byte-inspection units precede
/// all field reads. There is no heap allocation or owned payload receipt.
/// Caller-owned record/graph backing remains borrowed/external; an enclosing
/// retained result accounts for this small claim header in its wrapper fee.
pub fn read_unauthenticated_integer_continuation_claim_v1<'wire>(
    input: &Owner,
    output: &Owner,
    bytes: &'wire [u8],
    budget: &mut Budget<'_>,
) -> Result<UnauthenticatedIntegerContinuationClaimV1<'wire>, Error> {
    budget.charge_work(2).map_err(Error::Resource)?;
    let bytes: &[u8; INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1] =
        bytes.try_into().map_err(|_| Error::Framing)?;
    budget
        .charge_work(INTEGER_CONTINUATION_EXECUTION_RECORD_BYTES_V1)
        .map_err(Error::Resource)?;
    let mut reader = Reader { bytes, cursor: 0 };
    if reader.u16()? != 6
        || reader.u16()? != 1
        || reader.u16()? != INTEGER_CONTINUATION_PASSES.len() as u16
        || reader.u16()? != 0
    {
        return Err(Error::Framing);
    }
    for owner in [input, output] {
        let digest = reader.raw::<32>()?;
        let length = reader.u64()?;
        if digest != owner.canonical().identity().digest()
            || length != owner.canonical().identity().canonical_length()
        {
            return Err(Error::Endpoint);
        }
        if length > POLICY3_CANONICAL_CAP as u64 {
            return Err(Error::Profile);
        }
    }
    for ordinal in 0..6 {
        let value = reader.u64()?;
        if ordinal == 4 && value != INTEGER_CONTINUATION_PASSES.len() as u64 {
            return Err(Error::Profile);
        }
    }
    for cap in [
        POLICY3_CANONICAL_CAP,
        POLICY3_CANONICAL_CAP,
        POLICY3_MAX_PASSES,
        POLICY3_GRAPH_CAP,
        POLICY3_SESSION_WORK_CAP,
    ] {
        if reader.u64()? != cap as u64 {
            return Err(Error::Profile);
        }
    }
    for _ in 0..4 {
        let _ = reader.u64()?;
    }
    let _final_graph_digest = reader.raw::<32>()?;
    for _ in 0..3 {
        let _ = reader.u64()?;
    }
    let _map_digest = reader.raw::<32>()?;
    for pass in INTEGER_CONTINUATION_PASSES {
        if reader.raw::<1>()?[0] != pass_tag(pass)
            || reader.raw::<1>()?[0] > 1
            || reader.u16()? != 0
        {
            return Err(Error::Pass);
        }
        for _ in 0..7 {
            let _ = reader.u64()?;
        }
    }
    if reader.cursor != bytes.len() {
        return Err(Error::Framing);
    }
    Ok(UnauthenticatedIntegerContinuationClaimV1 { bytes })
}

struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}
impl<'a> Reader<'a> {
    fn raw<const N: usize>(&mut self) -> Result<&'a [u8; N], Error> {
        let end = self.cursor.checked_add(N).ok_or(Error::Framing)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(Error::Framing)?;
        self.cursor = end;
        Ok(value)
    }
    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(*self.raw()?))
    }
    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(*self.raw()?))
    }
}

#[cfg(test)]
#[path = "fixed_integer_continuation_claim_v1_tests.rs"]
mod tests;
