//! Bounded, inert interpretation of explicit reference enrollment requests.
//!
//! Decoding does not authenticate an invocation, resolve compiler instances,
//! establish policy origins, or grant source, proof, or native authority. A host
//! must independently retain and authenticate the original admitted invocation;
//! equal descriptor bytes cannot replace that owner.

use std::fmt;

use serde::Deserialize;

use crate::{CompileEnvironmentV2, MAX_ENVIRONMENT_VALUE_BYTES_V2, RustcInvocationDescriptorV3};

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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRequest {
    version: u16,
    bindings: Vec<WireBinding>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireBinding {
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
        let request: WireRequest = serde_json::from_str(bytes)
            .map_err(|_| InvalidRequest("invalid reference enrollment request schema"))?;
        // The typed pass rejects duplicates; the shape pass rejects serde's
        // positional-struct representation without overwriting duplicate keys.
        let shape: serde_json::Value = serde_json::from_str(bytes)
            .map_err(|_| InvalidRequest("invalid reference enrollment request schema"))?;
        if !shape.is_object()
            || !shape
                .get("bindings")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|rows| rows.iter().all(serde_json::Value::is_object))
        {
            return Err(InvalidRequest(
                "invalid reference enrollment request schema",
            ));
        }
        drop(shape);
        if request.version != 1 {
            return Err(InvalidRequest(
                "unsupported reference enrollment request version",
            ));
        }
        if request.bindings.is_empty()
            || request.bindings.len() > MAX_REFERENCE_ENROLLMENT_BINDINGS_V1
        {
            return Err(InvalidRequest(
                "reference enrollment binding count outside limits",
            ));
        }
        for binding in &request.bindings {
            for selector in [&binding.kernel, &binding.reference] {
                if selector.is_empty()
                    || selector.len() > MAX_REFERENCE_ENROLLMENT_SELECTOR_BYTES_V1
                    || selector.chars().any(char::is_control)
                {
                    return Err(InvalidRequest("invalid reference enrollment selector"));
                }
            }
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
        Ok(Self {
            version: request.version,
            bindings: request
                .bindings
                .into_iter()
                .map(|binding| ReferenceEnrollmentBindingV1 {
                    kernel: binding.kernel,
                    reference: binding.reference,
                })
                .collect(),
        })
    }
}

#[cfg(test)]
#[path = "reference_enrollment_v1_tests.rs"]
mod tests;
