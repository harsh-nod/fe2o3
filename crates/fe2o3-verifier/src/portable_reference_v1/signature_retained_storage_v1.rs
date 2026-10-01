//! Complete actual Box payloads of the inert signature; its header is excluded.
use super::ReferenceLogicalSignaturePreimageV1;
use super::ReferenceSignatureInputV1;
use fe2o3_kernel_ir::{LogicalStorageCounterV1, LogicalStorageErrorV1};

fn fixed<T: Copy>(_: &T) {}
fn copy_row<T: Copy>() {}

impl ReferenceLogicalSignaturePreimageV1 {
    /// Charge a zero-byte owner visit and both owned Box payloads, including
    /// empty boxes. The caller counts this inline header once. All debits use
    /// its original counter; discard prefix charges on any enclosing failure.
    /// This does not derive relations, validate a signature or grant authority.
    pub fn charge_retained_heap_storage_v1(
        &self,
        counter: &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> {
        counter.charge(0, 1)?;
        let Self {
            kernel_inputs,
            reference_inputs,
            reference_return,
            reference_abi,
            reference_safety,
            reference_c_variadic,
        } = self;
        fixed(reference_return);
        fixed(reference_abi);
        fixed(reference_safety);
        fixed(reference_c_variadic);
        copy_row::<ReferenceSignatureInputV1>();
        let _: &Box<[ReferenceSignatureInputV1]> = kernel_inputs;
        let _: &Box<[ReferenceSignatureInputV1]> = reference_inputs;
        counter.array::<ReferenceSignatureInputV1>(kernel_inputs.len())?;
        counter.array::<ReferenceSignatureInputV1>(reference_inputs.len())?;
        Ok(())
    }
}
