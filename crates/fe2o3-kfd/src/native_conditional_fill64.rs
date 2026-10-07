//! Inert numeric premises for the native V5 closed-fill profile.
//!
//! These caller-supplied facts are not source, proof, currentness, device or
//! execution authority. They are deliberately not convertible to nominal V4
//! premises. Native publication still requires independently retained owners.

use sha2::{Digest, Sha256};

/// Complete fixed-profile numbers, with no device address or authority token.
///
/// ```compile_fail
/// use fe2o3_kfd::{ConditionalDispatchPremisesV1, NativeConditionalFill64PremisesV1};
/// fn cannot_rebrand(p: NativeConditionalFill64PremisesV1) -> ConditionalDispatchPremisesV1 {
///     p.into()
/// }
/// ```
#[derive(Debug)]
pub struct NativeConditionalFill64PremisesV1 {
    contract_identity: [u8; 32],
    output_elements: u64,
    max_grid_x: u32,
    identity: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeConditionalFill64ErrorV1 {
    ContractIdentity,
    OutputExtent,
    GridBound,
    InvocationShape,
}

impl NativeConditionalFill64PremisesV1 {
    pub fn new(
        contract_identity: [u8; 32],
        output_elements: u64,
        max_grid_x: u32,
    ) -> Result<Self, NativeConditionalFill64ErrorV1> {
        use NativeConditionalFill64ErrorV1 as Error;
        if contract_identity == [0; 32] {
            return Err(Error::ContractIdentity);
        }
        if output_elements == 0 || output_elements.checked_mul(4).is_none() {
            return Err(Error::OutputExtent);
        }
        if max_grid_x < 64 || output_elements > u64::from(max_grid_x / 64 * 64) {
            return Err(Error::GridBound);
        }
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/KFD/NATIVE-V5-CONDITIONAL-FILL64-PREMISES/V1\0");
        hash.update(contract_identity);
        hash.update(output_elements.to_le_bytes());
        hash.update(max_grid_x.to_le_bytes());
        // Fixed ABI and target selector: gfx942:xnack-, CoV6, wave64.
        hash.update([1, 6, 64, 0]);
        for value in [0_u32, 8, 4, 16, 272, 8, 64, 1, 1] {
            hash.update(value.to_le_bytes());
        }
        Ok(Self {
            contract_identity,
            output_elements,
            max_grid_x,
            identity: hash.finalize().into(),
        })
    }

    pub const fn contract_identity(&self) -> &[u8; 32] {
        &self.contract_identity
    }
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn output_elements(&self) -> u64 {
        self.output_elements
    }
    pub const fn max_grid_x(&self) -> u32 {
        self.max_grid_x
    }

    /// Checks numbers only. This never inspects or admits a source artifact.
    pub fn validate_shape_v1(
        &self,
        grid: [u32; 3],
        workgroup: [u16; 3],
        output_bytes: u64,
    ) -> Result<(), NativeConditionalFill64ErrorV1> {
        if self.output_elements.checked_mul(4) != Some(output_bytes)
            || grid[0] == 0
            || !grid[0].is_multiple_of(64)
            || grid[0] > self.max_grid_x
            || u64::from(grid[0]) < self.output_elements
            || grid[1..] != [1, 1]
            || workgroup != [64, 1, 1]
        {
            return Err(NativeConditionalFill64ErrorV1::InvocationShape);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_fill64_bounds_and_identity_are_exact() {
        let p = NativeConditionalFill64PremisesV1::new([1; 32], 65, 129).unwrap();
        assert_eq!(p.validate_shape_v1([128, 1, 1], [64, 1, 1], 260), Ok(()));
        for (grid, group, bytes) in [
            ([64, 1, 1], [64, 1, 1], 260),
            ([129, 1, 1], [64, 1, 1], 260),
            ([192, 1, 1], [64, 1, 1], 260),
            ([128, 2, 1], [64, 1, 1], 260),
            ([128, 1, 1], [32, 1, 1], 260),
            ([128, 1, 1], [64, 1, 1], 256),
        ] {
            assert!(p.validate_shape_v1(grid, group, bytes).is_err());
        }
        for (contract, elements, max) in [
            ([0; 32], 1, 64),
            ([1; 32], 0, 64),
            ([1; 32], u64::MAX, u32::MAX),
            ([1; 32], 65, 127),
            ([1; 32], 1, 63),
        ] {
            assert!(NativeConditionalFill64PremisesV1::new(contract, elements, max).is_err());
        }
        for other in [
            NativeConditionalFill64PremisesV1::new([2; 32], 65, 129).unwrap(),
            NativeConditionalFill64PremisesV1::new([1; 32], 64, 129).unwrap(),
            NativeConditionalFill64PremisesV1::new([1; 32], 65, 128).unwrap(),
        ] {
            assert_ne!(p.identity(), other.identity());
        }
        let edge = u32::MAX / 64 * 64;
        let p = NativeConditionalFill64PremisesV1::new([1; 32], u64::from(edge), u32::MAX).unwrap();
        assert!(
            p.validate_shape_v1([edge, 1, 1], [64, 1, 1], u64::from(edge) * 4)
                .is_ok()
        );
    }
}
