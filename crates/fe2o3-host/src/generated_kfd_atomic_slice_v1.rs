fn reject_unjoined_atomic_runtime_v1(
    descriptor: &fe2o3_kernel_descriptor::KernelDescriptorV1,
) -> Result<(), GeneratedKfdPrepareError> {
    // Physical packing is not a runtime memory-coherence proof. Keep the V1
    // protected path closed until its invocation joins those obligations.
    if descriptor
        .arguments()
        .iter()
        .any(|argument| argument.alias() == fe2o3_kernel_descriptor::AliasSemantics::SharedAtomic)
    {
        return Err(GeneratedKfdPrepareError::AtomicRuntimeContractUnavailable);
    }
    Ok(())
}

/// Exclusively leased host atomic storage for a nominal shared device atomic slice.
///
/// This address-free binding copies values, not atomic object representations. It
/// grants no coherence or dispatch authority and retains the mutable host lease
/// until checked completion or cancellation drops the binding.
#[doc(hidden)]
pub struct GeneratedKfdAtomicSliceU32<'allocation> {
    values: &'allocation mut [std::sync::atomic::AtomicU32],
}

impl<'allocation> GeneratedKfdAtomicSliceU32<'allocation> {
    pub fn new(values: &'allocation mut [std::sync::atomic::AtomicU32]) -> Self {
        Self { values }
    }

    pub const fn len(&self) -> usize {
        self.values.len()
    }
    pub const fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn type_identity_v1(
        pointer_width: fe2o3_artifacts::PointerWidth,
    ) -> fe2o3_artifacts::TypeIdentity {
        crate::generated_argument_plan::atomic_slice_u32_type_identity_v1(pointer_width)
    }

    pub fn bind_argument(
        self,
        plan: &GeneratedArgumentPackingPlanV1,
        argument_index: usize,
    ) -> Result<GeneratedKfdSliceBinding<'allocation>, GeneratedKfdArgumentError> {
        let input = plan
            .bind_generated_address_free_atomic_slice_u32_v1(
                argument_index,
                self.values.len(),
                GeneratedArgumentBorrowV1::new(),
            )
            .map_err(GeneratedKfdArgumentError::Pack)?;
        if self.values.is_empty() {
            return Ok(GeneratedKfdSliceBinding {
                argument_index,
                input,
                buffer: None,
                required_alignment: 4,
                writeback: None,
            });
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(self.values.len() * 4)
            .map_err(|_| GeneratedKfdArgumentError::AllocationFailure)?;
        for value in self.values.iter() {
            bytes.extend_from_slice(
                &value
                    .load(std::sync::atomic::Ordering::Relaxed)
                    .to_le_bytes(),
            );
        }
        let buffer =
            Gfx942RuntimeDispatchBufferV1::new(bytes, Gfx942RuntimeBufferAccessV1::ReadWrite)
                .map_err(GeneratedKfdArgumentError::Buffer)?;
        let writeback = GeneratedKfdWriteback {
            destination: NonNull::new(self.values.as_mut_ptr().cast())
                .expect("nonempty atomic slice has a non-null pointer"),
            byte_len: self.values.len() * 4,
            apply_values: apply_atomic_u32_values_v1,
            _borrow: PhantomData,
        };
        Ok(GeneratedKfdSliceBinding {
            argument_index,
            input,
            buffer: Some(buffer),
            required_alignment: 4,
            writeback: Some(writeback),
        })
    }
}

unsafe fn apply_atomic_u32_values_v1(destination: NonNull<u8>, bytes: &[u8]) {
    assert!(bytes.len().is_multiple_of(4));
    // SAFETY: the writeback constructor retains the exact AtomicU32 destination
    // and exclusive lease; completion validates the entire byte length first.
    let values = unsafe {
        std::slice::from_raw_parts_mut(
            destination.cast::<std::sync::atomic::AtomicU32>().as_ptr(),
            bytes.len() / 4,
        )
    };
    for (value, encoded) in values.iter().zip(bytes.chunks_exact(4)) {
        value.store(
            u32::from_le_bytes(encoded.try_into().expect("exact u32 width")),
            std::sync::atomic::Ordering::Relaxed,
        );
    }
}

#[cfg(test)]
mod atomic_slice_tests {
    include!("generated_kfd_atomic_slice_v1_tests.rs");
}
