//! Scoped accounting for reference extraction, not another resource ledger.
//!
//! Allocation charges count requested payload bytes and initialization/copy
//! work in the caller's monotone work domain. They are not live-storage or
//! allocator-internal guarantees. Rustc query internals are a separate domain.

use std::cell::RefCell;

use super::*;
use crate::rustc_semantic_plan_v1::SourceClosureWorkV1;

pub(super) enum ReferenceExtractionWorkV1<'a> {
    Shared(RefCell<&'a mut SourceClosureWorkV1>),
    /// Replay-only charging into the caller's original canonical account.
    /// This does not satisfy the live-rustc `is_shared` authentication gate.
    Canonical(RefCell<&'a mut dyn FnMut(usize) -> Result<(), ReferenceBindingErrorV1>>),
    /// Existing descriptive queries outside authenticated extraction.
    Inspection,
}

impl<'a> ReferenceExtractionWorkV1<'a> {
    pub(super) fn canonical(
        charge: &'a mut dyn FnMut(usize) -> Result<(), ReferenceBindingErrorV1>,
    ) -> Self {
        Self::Canonical(RefCell::new(charge))
    }

    pub(super) fn borrowed(source: &'a mut SourceClosureWorkV1) -> Self {
        Self::Shared(RefCell::new(source))
    }

    pub(super) fn is_shared(&self) -> bool {
        matches!(self, Self::Shared(_))
    }
}

impl ReferenceWorkV1 for ReferenceExtractionWorkV1<'_> {
    fn charge(&self, amount: usize) -> Result<(), ReferenceBindingErrorV1> {
        match self {
            Self::Shared(source) => source
                .borrow_mut()
                .charge(amount)
                .map_err(|error| ReferenceBindingErrorV1::new(error.to_string())),
            Self::Inspection => Ok(()),
            Self::Canonical(charge) => (*charge.borrow_mut())(amount),
        }
    }
}

pub(super) use fe2o3_verifier::portable_reference_v1::extraction::add;

#[cfg(test)]
#[path = "reference_extraction_work_v1_tests.rs"]
mod tests;
