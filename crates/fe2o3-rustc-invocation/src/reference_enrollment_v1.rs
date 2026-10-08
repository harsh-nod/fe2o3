//! Bounded, inert interpretation of explicit reference enrollment requests.
//!
//! Decoding does not authenticate an invocation, resolve compiler instances,
//! establish policy origins, or grant source, proof, or native authority. A host
//! must independently retain and authenticate the original admitted invocation;
//! equal descriptor bytes cannot replace that owner.

use std::fmt;

use crate::{CompileEnvironmentV2, MAX_ENVIRONMENT_VALUE_BYTES_V2, RustcInvocationDescriptorV3};

mod parse;

/// Captured compile-environment key containing a V1 enrollment request.
pub const REFERENCE_ENROLLMENT_ENV_V1: &str = "FE2O3_REFERENCE_ENROLLMENT_V1";
/// Maximum encoded JSON request size, in bytes.
pub const MAX_REFERENCE_ENROLLMENT_BYTES_V1: usize = MAX_ENVIRONMENT_VALUE_BYTES_V2;
/// Maximum number of bindings, also subject to the complete request byte bound.
pub const MAX_REFERENCE_ENROLLMENT_BINDINGS_V1: usize = 256;
/// Maximum decoded UTF-8 byte length of either selector.
pub const MAX_REFERENCE_ENROLLMENT_SELECTOR_BYTES_V1: usize = 1_024;

/// A bounded descriptive request, not an authenticated enrollment or capability.
///
/// Fields are private and access is read-only. Construction is only through the
/// metered decoder; this public type deliberately does not implement Deserialize.
#[derive(Debug, Eq, PartialEq)]
pub struct ReferenceEnrollmentRequestV1 {
    version: u16,
    bindings: Vec<ReferenceEnrollmentBindingV1>,
}

/// An inert pair of selector strings, without compiler resolution or authority.
#[derive(Debug, Eq, PartialEq)]
pub struct ReferenceEnrollmentBindingV1 {
    kernel: String,
    reference: String,
}

/// A schema/limit refusal or the caller's unchanged cumulative-meter error.
#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ReferenceEnrollmentDecodeErrorV1<E> {
    /// The request violated the fixed V1 schema or one of its bounds.
    InvalidRequest(&'static str),
    /// The caller refused a work charge before the corresponding decoding work.
    Work(E),
}

impl<E: fmt::Display> fmt::Display for ReferenceEnrollmentDecodeErrorV1<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest(message) => formatter.write_str(message),
            Self::Work(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ReferenceEnrollmentDecodeErrorV1<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidRequest(_) => None,
            Self::Work(error) => Some(error),
        }
    }
}

impl ReferenceEnrollmentBindingV1 {
    /// The requested kernel selector, not a resolved or authenticated root.
    pub fn kernel(&self) -> &str {
        &self.kernel
    }

    /// The requested reference selector, not a proof or authority handle.
    pub fn reference(&self) -> &str {
        &self.reference
    }
}

impl ReferenceEnrollmentRequestV1 {
    /// The supported request schema version.
    pub fn version(&self) -> u16 {
        self.version
    }

    /// Read-only bindings in strictly increasing, unique kernel-selector order.
    pub fn bindings(&self) -> &[ReferenceEnrollmentBindingV1] {
        &self.bindings
    }

    /// Measures this inert owner, including unused vector and string capacity.
    ///
    /// Counts one request header, the bindings vector's actual capacity times
    /// the binding header size, and both strings' actual capacities for every
    /// initialized binding. String headers are already in the vector backing;
    /// unused vector slots have no string backing to visit.
    ///
    /// This allocation-free traversal visits at most
    /// [`MAX_REFERENCE_ENROLLMENT_BINDINGS_V1`] bindings. Callers must fund the
    /// traversal before calling it and reserve the returned logical backing
    /// charge while retaining this owner. `None` means arithmetic overflow,
    /// not a zero charge. This observation neither reserves storage nor grants
    /// authority.
    ///
    /// The quote excludes the input owner, parser temporaries, allocator
    /// metadata/rounding, and stack. It is not a peak-allocation or RSS bound,
    /// and cannot retroactively prepay decoding.
    pub fn retained_storage_bytes(&self) -> Option<usize> {
        checked_retained_storage_bytes(
            self.bindings.capacity(),
            self.bindings
                .iter()
                .map(|binding| (binding.kernel.capacity(), binding.reference.capacity())),
        )
    }

    /// Interpret only this inert descriptor's captured environment.
    ///
    /// This does not authenticate the descriptor or its invocation owner.
    /// Charging and absence behavior are those of [`Self::from_environment`].
    pub fn from_descriptor<E>(
        descriptor: &RustcInvocationDescriptorV3,
        charge: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Option<Self>, ReferenceEnrollmentDecodeErrorV1<E>> {
        Self::from_environment(descriptor.compile_environment(), charge)
    }

    /// Find the explicit request in a captured environment, never the live one.
    ///
    /// Charges each visited key's byte length plus one, stopping at the request.
    /// Absence returns None without decoding. The callback must apply charges
    /// cumulatively; its failure is returned unchanged in the Work variant.
    pub fn from_environment<E>(
        environment: &CompileEnvironmentV2,
        mut charge: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Option<Self>, ReferenceEnrollmentDecodeErrorV1<E>> {
        for entry in environment.entries() {
            charge(entry.key().len().saturating_add(1))
                .map_err(ReferenceEnrollmentDecodeErrorV1::Work)?;
            if entry.key() == REFERENCE_ENROLLMENT_ENV_V1 {
                return Self::decode(entry.value(), charge).map(Some);
            }
        }
        Ok(None)
    }

    /// Decode the fixed V1 object schema under a caller-owned cumulative meter.
    ///
    /// Charges one before the encoded-byte check, then prepays exactly
    /// `64 * bytes.len() + size_of::<Self>()` before serde or decoded validation.
    /// The quote covers bounded decoding, string/vector backing, and checks;
    /// it is source work, not canonical TARGET storage, RSS, or authority.
    ///
    /// # Storage and failure
    ///
    /// The work callback does not reserve storage. One fixed-schema parse uses
    /// internal serde allocations, including on schema-error paths. Output
    /// allocations are fallible, but allocation failure inside serde is not
    /// guaranteed to return a decode error. Callers requiring
    /// prepaid parsing storage need a separately justified trusted allocation
    /// boundary before entering this method. Neither the work quote nor a
    /// later [`Self::retained_storage_bytes`] observation supplies that boundary.
    pub fn decode<E>(
        bytes: &str,
        mut charge: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, ReferenceEnrollmentDecodeErrorV1<E>> {
        use ReferenceEnrollmentDecodeErrorV1::{InvalidRequest, Work};

        charge(1).map_err(Work)?;
        if bytes.len() > MAX_REFERENCE_ENROLLMENT_BYTES_V1 {
            return Err(InvalidRequest(
                "reference enrollment request exceeds byte limit",
            ));
        }
        let quote = bytes
            .len()
            .checked_mul(64)
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .ok_or(InvalidRequest("reference enrollment work overflow"))?;
        charge(quote).map_err(Work)?;
        let parsed = parse::request(bytes)
            .map_err(|_| InvalidRequest("invalid reference enrollment request schema"))?;
        let request = parsed.request;
        if request.version != 1 {
            return Err(InvalidRequest(
                "unsupported reference enrollment request version",
            ));
        }
        if parsed.binding_count == 0 || parsed.binding_count > MAX_REFERENCE_ENROLLMENT_BINDINGS_V1
        {
            return Err(InvalidRequest(
                "reference enrollment binding count outside limits",
            ));
        }
        if parsed.invalid_selector {
            return Err(InvalidRequest("invalid reference enrollment selector"));
        }
        if request
            .bindings
            .windows(2)
            .any(|pair| pair[0].kernel >= pair[1].kernel)
        {
            return Err(InvalidRequest(
                "reference enrollment roots must be unique and sorted",
            ));
        }
        Ok(request)
    }
}

// Keep capacity arithmetic separately testable without enormous allocations.
fn checked_retained_storage_bytes(
    bindings_capacity: usize,
    selector_capacities: impl IntoIterator<Item = (usize, usize)>,
) -> Option<usize> {
    let mut bytes = std::mem::size_of::<ReferenceEnrollmentRequestV1>().checked_add(
        bindings_capacity.checked_mul(std::mem::size_of::<ReferenceEnrollmentBindingV1>())?,
    )?;
    for (kernel_capacity, reference_capacity) in selector_capacities {
        bytes = bytes
            .checked_add(kernel_capacity)?
            .checked_add(reference_capacity)?;
    }
    Some(bytes)
}

#[cfg(test)]
#[path = "reference_enrollment_v1_tests.rs"]
mod tests;
