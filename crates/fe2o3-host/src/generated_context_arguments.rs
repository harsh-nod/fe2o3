//! Address-free slice metadata for generated ordinary Context arguments.

use crate::GeneratedDeviceScalarV1;
use fe2o3_runtime::{
    RuntimeAccessV1, RuntimeAllocationIdV1, RuntimeMemoryRegionV1, RuntimeValidationErrorV1,
};
use std::marker::PhantomData;

fn checked_slice_region_v1<T: GeneratedDeviceScalarV1>(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    byte_offset: u64,
    elements: u64,
) -> Result<RuntimeMemoryRegionV1, RuntimeValidationErrorV1> {
    // The sealed primitive ABI has positive size and equal size/alignment,
    // independent of the host's layout (including a 32-bit host's u64 alignment).
    let element_bytes = T::RUST_SCALAR_TYPE.size_bytes();
    if elements == 0 || !byte_offset.is_multiple_of(element_bytes) {
        return Err(RuntimeValidationErrorV1::InvalidRange);
    }
    let byte_len = elements
        .checked_mul(element_bytes)
        .ok_or(RuntimeValidationErrorV1::InvalidRange)?;
    byte_offset
        .checked_add(byte_len)
        .ok_or(RuntimeValidationErrorV1::InvalidRange)?;
    Ok(RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset,
        byte_len,
    })
}

macro_rules! context_slice {
    ($name:ident, $access:ident, $description:literal) => {
        #[doc = $description]
        ///
        /// This value retains only an allocation identity and checked metadata, not
        /// an allocation borrow, native pointer, storage owner, or launch authority.
        /// Context validates the live allocation, device, and bounds at submission.
        #[derive(Clone, Copy, Debug)]
        pub struct $name<T: GeneratedDeviceScalarV1> {
            region: RuntimeMemoryRegionV1,
            elements: u64,
            scalar: PhantomData<T>,
        }

        impl<T: GeneratedDeviceScalarV1> $name<T> {
            /// Checks a nonempty, scalar-aligned allocation-relative slice.
            ///
            /// Rejects count multiplication or end-offset overflow. This does not
            /// establish that the allocation is live, large enough, initialized,
            /// or owned by the Context/device used for a later launch.
            pub fn new(
                allocation: RuntimeAllocationIdV1,
                byte_offset: u64,
                elements: u64,
            ) -> Result<Self, RuntimeValidationErrorV1> {
                Ok(Self {
                    region: checked_slice_region_v1::<T>(
                        allocation,
                        RuntimeAccessV1::$access,
                        byte_offset,
                        elements,
                    )?,
                    elements,
                    scalar: PhantomData,
                })
            }

            /// Returns the address-free region with this descriptor's fixed access.
            pub const fn region_v1(&self) -> RuntimeMemoryRegionV1 {
                self.region
            }

            /// Returns the element count encoded in the generated slice ABI.
            pub const fn elements_v1(&self) -> u64 {
                self.elements
            }
        }
    };
}

context_slice!(
    GeneratedContextReadSlice,
    Read,
    "Typed read-only slice metadata for an ordinary Context launch."
);
context_slice!(
    GeneratedContextWriteSlice,
    Write,
    "Typed write-only slice metadata for an ordinary Context launch."
);
context_slice!(
    GeneratedContextReadWriteSlice,
    ReadWrite,
    "Typed read/write slice metadata for an ordinary Context launch."
);

#[cfg(test)]
mod tests;
