//! Closed borrowed access to common live V5/V6 facts for functional joins.
//! Wire identity remains version-specific; no V6 owner becomes a V5 record.

use super::{
    ProductionMiddleEndCoverageSummaryV5, ProductionMiddleEndEvidenceIdentityV5,
    ProductionMiddleEndEvidenceIdentityV6, ProductionMiddleEndEvidenceV5,
    ProductionMiddleEndEvidenceV6, ProductionMiddleEndSemanticSummaryV5,
    ProductionMiddleEndTypedSemanticReconciliationV5, ProductionTypedSemanticObligationSummaryV2,
};

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::ProductionMiddleEndEvidenceV5 {}
    impl Sealed for super::ProductionMiddleEndEvidenceV6 {}
}

/// The actual versioned identity of a live evidence owner, not new authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionMiddleEndLiveIdentityV1 {
    V5(ProductionMiddleEndEvidenceIdentityV5),
    V6(ProductionMiddleEndEvidenceIdentityV6),
}

impl ProductionMiddleEndLiveIdentityV1 {
    pub const fn sha256(&self) -> &[u8; 32] {
        match self { Self::V5(value) => value.sha256(), Self::V6(value) => value.sha256() }
    }
    pub const fn byte_len(self) -> u64 {
        match self { Self::V5(value) => value.byte_len(), Self::V6(value) => value.byte_len() }
    }
    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        match self { Self::V5(value) => value.matches_canonical_bytes(bytes), Self::V6(value) => value.matches_canonical_bytes(bytes) }
    }
}

/// Only live, move-only V5 and V6 owners implement this sealed interface.
/// Existing functional checks still compare every retained fact and exact
/// canonical identity. Inert records and caller-defined fact providers cannot
/// enter the live reconciliation boundary through this interface.
///
/// ```compile_fail
/// use fe2o3_pliron::*;
/// #[derive(Debug)] struct Forged;
/// impl ProductionMiddleEndEvidenceViewV1 for Forged {
///     fn identity(&self) -> ProductionMiddleEndLiveIdentityV1 { unimplemented!() }
///     fn ranked_kernel_identity(&self) -> &[u8; 32] { unimplemented!() }
///     fn canonical_bytes(&self) -> &[u8] { unimplemented!() }
///     fn coverage_summary(&self) -> ProductionMiddleEndCoverageSummaryV5 { unimplemented!() }
///     fn semantic_summary(&self) -> ProductionMiddleEndSemanticSummaryV5 { unimplemented!() }
///     fn typed_semantic_summary(&self) -> ProductionTypedSemanticObligationSummaryV2 { unimplemented!() }
///     fn typed_semantic_reconciliation(&self) -> ProductionMiddleEndTypedSemanticReconciliationV5 { unimplemented!() }
/// }
/// ```
pub trait ProductionMiddleEndEvidenceViewV1: sealed::Sealed + std::fmt::Debug {
    fn identity(&self) -> ProductionMiddleEndLiveIdentityV1;
    fn ranked_kernel_identity(&self) -> &[u8; 32];
    fn canonical_bytes(&self) -> &[u8];
    fn coverage_summary(&self) -> ProductionMiddleEndCoverageSummaryV5;
    fn semantic_summary(&self) -> ProductionMiddleEndSemanticSummaryV5;
    fn typed_semantic_summary(&self) -> ProductionTypedSemanticObligationSummaryV2;
    fn typed_semantic_reconciliation(&self) -> ProductionMiddleEndTypedSemanticReconciliationV5;
}

macro_rules! implement_live_view {
    ($owner:ty, $variant:ident) => {
        impl ProductionMiddleEndEvidenceViewV1 for $owner {
            fn identity(&self) -> ProductionMiddleEndLiveIdentityV1 {
                ProductionMiddleEndLiveIdentityV1::$variant(<$owner>::identity(self))
            }
            fn ranked_kernel_identity(&self) -> &[u8; 32] { <$owner>::ranked_kernel_identity(self) }
            fn canonical_bytes(&self) -> &[u8] { <$owner>::canonical_bytes(self) }
            fn coverage_summary(&self) -> ProductionMiddleEndCoverageSummaryV5 { <$owner>::coverage_summary(self) }
            fn semantic_summary(&self) -> ProductionMiddleEndSemanticSummaryV5 { <$owner>::semantic_summary(self) }
            fn typed_semantic_summary(&self) -> ProductionTypedSemanticObligationSummaryV2 { <$owner>::typed_semantic_summary(self) }
            fn typed_semantic_reconciliation(&self) -> ProductionMiddleEndTypedSemanticReconciliationV5 { <$owner>::typed_semantic_reconciliation(self) }
        }
    };
}

implement_live_view!(ProductionMiddleEndEvidenceV5, V5);
implement_live_view!(ProductionMiddleEndEvidenceV6, V6);
