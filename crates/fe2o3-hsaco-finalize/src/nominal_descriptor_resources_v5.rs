//! Logical callback debits, separate from the existing bounded ELF domain.
use fe2o3_hsaco::MAX_HSACO_BYTES;
use fe2o3_kernel_descriptor::{
    CANONICAL_CODE_OBJECT_DOMAIN_V1, DescriptorWorkBoundsV5, MAX_ARGUMENTS_PER_KERNEL,
    MAX_DESCRIPTOR_TABLE_BYTES, MAX_KERNELS, MAX_NAME_BYTES, MAX_PHYSICAL_COMPONENTS_PER_KERNEL,
};

/// Finite bounds for the actual V5 nominal finalizer's callback charges.
/// They do not bound ELF parser allocations, CPU instructions or process RSS,
/// and neither authenticate bytes nor change any operation's input limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NominalDescriptorWorkBoundsV5 {
    raw_inspection: usize,
    finalized_inspection: usize,
    launch_derivation: usize,
    finalization: usize,
    reconstruction: usize,
}
impl NominalDescriptorWorkBoundsV5 {
    /// Existing independent descriptor/count/HSACO limits, with checked sums.
    pub fn admitted_limits() -> Option<Self> {
        let descriptor = DescriptorWorkBoundsV5::admitted_limits()?;
        // cross_check first requires equal kernel counts. Inspected bindings
        // contain exactly one entry per kernel. These searches therefore use
        // the descriptor limit, not the less restrictive ELF parser limit.
        let physical_kernel = sum(&[
            descriptor.kernel(),
            MAX_NAME_BYTES + 128,
            mul(MAX_KERNELS, MAX_NAME_BYTES + 1)?,
            MAX_KERNELS,
            descriptor.requirement(),
            mul(
                MAX_ARGUMENTS_PER_KERNEL,
                sum(&[descriptor.argument_next(), descriptor.source_type()])?,
            )?,
            1, // terminal argument-cursor step
            mul(
                MAX_PHYSICAL_COMPONENTS_PER_KERNEL,
                sum(&[descriptor.component(), 32])?,
            )?,
        ])?;
        let physical = sum(&[64, mul(MAX_KERNELS, physical_kernel)?])?;
        let normalized_digest = sum(&[
            MAX_HSACO_BYTES,
            CANONICAL_CODE_OBJECT_DOMAIN_V1.len(),
            8,
            128,
        ])?;
        let raw_inspection = sum(&[1, descriptor.decode(), physical, 32])?;
        let finalized_inspection = sum(&[raw_inspection, normalized_digest])?;
        let launch_derivation = sum(&[raw_inspection, mul(MAX_KERNELS, descriptor.kernel())?])?;
        let finalization = sum(&[
            raw_inspection,
            MAX_DESCRIPTOR_TABLE_BYTES, // exact source-descriptor comparison
            normalized_digest,
            MAX_HSACO_BYTES, // owned output copy
            MAX_HSACO_BYTES, // unchanged bytes outside the digest
            finalized_inspection,
        ])?;
        let reconstruction = sum(&[
            finalized_inspection,
            MAX_HSACO_BYTES,
            raw_inspection,
            normalized_digest,
        ])?;
        Some(Self {
            raw_inspection,
            finalized_inspection,
            launch_derivation,
            finalization,
            reconstruction,
        })
    }
    /// One complete zero-digest inspection and physical descriptor check.
    pub const fn raw_inspection(self) -> usize {
        self.raw_inspection
    }
    /// One complete finalized inspection, including normalized byte hashing.
    pub const fn finalized_inspection(self) -> usize {
        self.finalized_inspection
    }
    /// Raw inspection followed by all per-kernel common-launch queries.
    pub const fn launch_derivation(self) -> usize {
        self.launch_derivation
    }
    /// Raw inspection, exact source join, copy/hash, and final reinspection.
    pub const fn finalization(self) -> usize {
        self.finalization
    }
    /// Final inspection, copied zeroed preimage, and both integrity rechecks.
    pub const fn reconstruction(self) -> usize {
        self.reconstruction
    }
}
fn sum(values: &[usize]) -> Option<usize> {
    values.iter().try_fold(0usize, |n, v| n.checked_add(*v))
}
fn mul(a: usize, b: usize) -> Option<usize> {
    a.checked_mul(b)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_nominal_schedule_covers_each_nested_inspection_and_copy() {
        let quote = NominalDescriptorWorkBoundsV5::admitted_limits().unwrap();
        assert!(quote.launch_derivation() > quote.raw_inspection());
        assert!(quote.finalization() > quote.raw_inspection() + quote.finalized_inspection());
        assert!(quote.reconstruction() > quote.raw_inspection() + quote.finalized_inspection());
        assert_eq!(sum(&[usize::MAX, 1]), None);
        assert_eq!(mul(usize::MAX, 2), None);
    }
}
