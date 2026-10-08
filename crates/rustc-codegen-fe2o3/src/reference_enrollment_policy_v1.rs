//! Explicit reference selection captured in the canonical rustc invocation.
//! Decoding selects no Instance and grants no source, proof or native authority.

use crate::reference_effect_v1::ReferenceBindingErrorV1 as Error;
use crate::rustc_semantic_plan_v1::SourceClosureWorkV1;
use fe2o3_rustc_invocation::{
    CompileEnvironmentV2, MAX_ENVIRONMENT_VALUE_BYTES_V2, RustcInvocationDescriptorV3,
};
use serde::Deserialize;

pub(crate) const ENV: &str = "FE2O3_REFERENCE_ENROLLMENT_V1";
const MAX_BYTES: usize = MAX_ENVIRONMENT_VALUE_BYTES_V2;
const MAX_BINDINGS: usize = 256;
const MAX_SELECTOR_BYTES: usize = 1_024;

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReferenceEnrollmentRequestV1 {
    version: u16,
    bindings: Vec<ReferenceEnrollmentBindingV1>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReferenceEnrollmentBindingV1 {
    kernel: String,
    reference: String,
}

impl ReferenceEnrollmentBindingV1 {
    pub(crate) fn kernel(&self) -> &str {
        &self.kernel
    }

    pub(crate) fn reference(&self) -> &str {
        &self.reference
    }
}

impl ReferenceEnrollmentRequestV1 {
    /// The descriptor is inert. Its original admitted owner must independently
    /// survive collection and replay; equal descriptor bytes cannot replace it.
    pub(crate) fn from_descriptor(
        descriptor: &RustcInvocationDescriptorV3,
        work: &mut SourceClosureWorkV1,
    ) -> Result<Option<Self>, Error> {
        Self::from_environment(descriptor.compile_environment(), work)
    }

    pub(crate) fn bindings(&self) -> &[ReferenceEnrollmentBindingV1] {
        &self.bindings
    }

    fn from_environment(
        environment: &CompileEnvironmentV2,
        work: &mut SourceClosureWorkV1,
    ) -> Result<Option<Self>, Error> {
        for entry in environment.entries() {
            charge(work, entry.key().len().saturating_add(1))?;
            if entry.key() == ENV {
                return Self::decode(entry.value(), work).map(Some);
            }
        }
        Ok(None)
    }

    fn decode(bytes: &str, work: &mut SourceClosureWorkV1) -> Result<Self, Error> {
        charge(work, 1)?;
        if bytes.len() > MAX_BYTES {
            return Err(Error::new(
                "reference enrollment request exceeds byte limit",
            ));
        }
        // Prepay the bounded decoder, decoded string/vector backing and checks.
        // This is cumulative source work, not canonical TARGET storage or RSS.
        let quote = bytes
            .len()
            .checked_mul(64)
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .ok_or_else(|| Error::new("reference enrollment work overflow"))?;
        charge(work, quote)?;
        let request: Self = serde_json::from_str(bytes)
            .map_err(|_| Error::new("invalid reference enrollment request schema"))?;
        // Serde also accepts positional structs. Enrollment uses JSON objects;
        // keep the typed pass above so duplicate fields cannot be overwritten.
        let shape: serde_json::Value = serde_json::from_str(bytes)
            .map_err(|_| Error::new("invalid reference enrollment request schema"))?;
        if !shape.is_object()
            || !shape
                .get("bindings")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|rows| rows.iter().all(serde_json::Value::is_object))
        {
            return Err(Error::new("invalid reference enrollment request schema"));
        }
        drop(shape);
        if request.version != 1 {
            return Err(Error::new(
                "unsupported reference enrollment request version",
            ));
        }
        if request.bindings.is_empty() || request.bindings.len() > MAX_BINDINGS {
            return Err(Error::new(
                "reference enrollment binding count outside limits",
            ));
        }
        for binding in &request.bindings {
            for selector in [&binding.kernel, &binding.reference] {
                if selector.is_empty()
                    || selector.len() > MAX_SELECTOR_BYTES
                    || selector.chars().any(char::is_control)
                {
                    return Err(Error::new("invalid reference enrollment selector"));
                }
            }
        }
        if request
            .bindings
            .windows(2)
            .any(|pair| pair[0].kernel >= pair[1].kernel)
        {
            return Err(Error::new(
                "reference enrollment roots must be unique and sorted",
            ));
        }
        Ok(request)
    }
}

fn charge(work: &mut SourceClosureWorkV1, amount: usize) -> Result<(), Error> {
    work.charge(amount)
        .map_err(|error| Error::new(error.to_string()))
}

#[cfg(test)]
#[path = "reference_enrollment_policy_v1_tests.rs"]
mod tests;
