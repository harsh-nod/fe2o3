//! Shared fixed values of the existing production canonical-phase policy.
//! These finite refusal thresholds are not algorithmic worst-case estimates,
//! acceptance guarantees, a whole-compiler budget, or semantic authority.

/// Existing production canonical materialization/assertion-phase logical work
/// ceiling. Convert explicitly on hosts whose usize may not represent it.
pub const CANONICAL_PHASE_WORK_LIMIT_V1: u64 = 18_014_398_509_481_984;
/// Existing production canonical-phase logical storage ceiling, not RSS or a
/// permission to widen a narrower component's own resource domain.
pub const CANONICAL_PHASE_STORAGE_LIMIT_V1: usize = 2_147_483_648;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_phase_policy_preserves_existing_typed_values() {
        let work: u64 = CANONICAL_PHASE_WORK_LIMIT_V1;
        let storage: usize = CANONICAL_PHASE_STORAGE_LIMIT_V1;
        assert_eq!(work, 1u64 << 54);
        assert_eq!(storage, 1usize << 31);
    }
}
