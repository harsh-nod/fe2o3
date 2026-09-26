//! Fixed, leaf-first all-ancestor batch planning. Arena/path extraction, locking,
//! slot validation and commit remain the accounting adapter's responsibility.

use crate::{R67ResourceVectorV1, r67_resource_release_v1, r67_resource_reserve_v1};

macro_rules! resource_domain_declarations_v1 {
    ($($items:tt)*) => { $($items)* };
}
include!("r75_resource_domain_declarations.rs");
include!("r75_resource_domain_bodies.rs");

const _: () = assert!(crate::R70_MAX_RESOURCE_BATCH_MEMBERS_V1 == 65_536);

impl R75ResourceDomainFactsV1 {
    pub const EMPTY: Self = Self {
        used: R67ResourceVectorV1::ZERO,
        capacity: R67ResourceVectorV1::ZERO,
        counts: [0; 3],
        record_limit: 0,
    };
}

macro_rules! resource_domain_rust_expr {
    ($body:expr) => {
        $body
    };
}

#[inline]
#[allow(clippy::question_mark)] // Explicit matches are shared with Verus.
fn resource_domain_records_v1(
    facts: R75ResourceDomainFactsV1,
    count: usize,
    owner: u64,
) -> Result<(usize, u64), R75ResourceDomainErrorV1> {
    resource_domain_records_body_v1!(facts, count, owner)
}

#[inline]
fn resource_domain_leaf_v1(
    used: R67ResourceVectorV1,
    charges: &[R67ResourceVectorV1],
    capacity: R67ResourceVectorV1,
) -> Result<R67ResourceVectorV1, R75ResourceDomainErrorV1> {
    resource_domain_leaf_body_v1!(
        resource_domain_rust_expr,
        used,
        charges,
        capacity,
        next,
        member,
        [],
        [],
        []
    )
}

/// Stages every active ancestor before the caller changes its ledger. Facts are
/// ordered leaf to root; inactive array entries are ignored and remain zero in
/// the plan. The accepted leaf supplies the batch total for every parent, so
/// charges are scanned once, without copying the roster or allocating storage.
#[allow(clippy::question_mark)] // Explicit matches are shared with Verus.
pub fn r75_resource_domain_reserve_v1(
    facts: &[R75ResourceDomainFactsV1; R75_RESOURCE_DOMAIN_LEVELS_V1],
    depth: usize,
    profile: usize,
    charges: &[R67ResourceVectorV1],
    owner: u64,
) -> Result<R75ResourceDomainPlanV1, R75ResourceDomainErrorV1> {
    resource_domain_path_body_v1!(
        resource_domain_rust_expr,
        facts,
        depth,
        profile,
        charges,
        owner,
        plan,
        total,
        level,
        next,
        [],
        [],
        [],
        [],
        [],
        [],
        [],
        []
    )
}

#[cfg(test)]
mod tests;
