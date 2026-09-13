// Opt-in allocation admission for the existing exact wire decoder.

trait DecodeResourceBudgetV12 {
    fn work_budget(&mut self) -> &mut CanonicalKernelIrWorkBudgetV1;
    fn work_limit(&self) -> usize;
    fn reserve(
        &mut self,
        amount: usize,
    ) -> Result<(), crate::CanonicalKernelIrVerificationResourceErrorV1>;
    fn release(
        &mut self,
        amount: usize,
    ) -> Result<(), crate::CanonicalKernelIrVerificationResourceErrorV1>;
}

impl DecodeResourceBudgetV12 for crate::CanonicalKernelIrVerificationResourceBudgetV1<'_> {
    fn work_budget(&mut self) -> &mut CanonicalKernelIrWorkBudgetV1 {
        self.work_budget_v1()
    }
    fn work_limit(&self) -> usize {
        self.work_limit_v1()
    }
    fn reserve(
        &mut self,
        amount: usize,
    ) -> Result<(), crate::CanonicalKernelIrVerificationResourceErrorV1> {
        self.reserve_storage(amount)
    }
    fn release(
        &mut self,
        amount: usize,
    ) -> Result<(), crate::CanonicalKernelIrVerificationResourceErrorV1> {
        self.release_storage(amount)
    }
}

enum DecodeBudgetV12<'a> {
    Work(&'a mut CanonicalKernelIrWorkBudgetV1),
    Resources(&'a mut dyn DecodeResourceBudgetV12),
}
impl DecodeBudgetV12<'_> {
    fn work_budget(&mut self) -> &mut CanonicalKernelIrWorkBudgetV1 {
        match self {
            Self::Work(work) => work,
            Self::Resources(resources) => resources.work_budget(),
        }
    }
    fn charge_work(&mut self, amount: usize) -> Result<(), CanonicalKernelIrWorkLimitV1> {
        self.work_budget().charge_work(amount)
    }
    fn limit(&self) -> usize {
        match self {
            Self::Work(work) => work.limit(),
            Self::Resources(resources) => resources.work_limit(),
        }
    }
    fn reserve(&mut self, amount: usize) -> Result<(), KernelIrDecodeError> {
        match self {
            Self::Work(_) => Ok(()),
            Self::Resources(resources) => resources
                .reserve(amount)
                .map_err(KernelIrDecodeError::Resource),
        }
    }
    fn release(&mut self, amount: usize) -> Result<(), KernelIrDecodeError> {
        match self {
            Self::Work(_) => Ok(()),
            Self::Resources(resources) => resources
                .release(amount)
                .map_err(KernelIrDecodeError::Resource),
        }
    }
}

// The pinned Rust B-tree has at most eleven inline keys and twelve child
// pointers per node. Three nodes per inserted key conservatively covers splits
// and root replacement; four extra payloads cover collect/sort scratch. Allocator
// bookkeeping and RSS are not claimed.
pub(crate) fn decoded_tree_payload_bound_v12<T>(
    count: usize,
) -> Result<usize, KernelIrDecodeError> {
    std::mem::size_of::<T>()
        .checked_mul(11)
        .and_then(|n| n.checked_add(14 * std::mem::size_of::<usize>()))
        .and_then(|n| n.checked_mul(3))
        .and_then(|n| n.checked_add(4 * std::mem::size_of::<T>()))
        .and_then(|n| n.checked_mul(count))
        .ok_or(KernelIrDecodeError::Resource(
            crate::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
        ))
}

impl Reader<'_, '_> {
    fn tracks_allocations(&self) -> bool {
        matches!(self.budget, Some(DecodeBudgetV12::Resources(_)))
    }
    fn reserve_payload(&mut self, amount: usize) -> Result<(), KernelIrDecodeError> {
        self.budget
            .as_mut()
            .map_or(Ok(()), |budget| budget.reserve(amount))
    }
    fn release_payload(&mut self, amount: usize) -> Result<(), KernelIrDecodeError> {
        self.budget
            .as_mut()
            .map_or(Ok(()), |budget| budget.release(amount))
    }
    fn vector<T>(&mut self, count: usize) -> Result<Vec<T>, KernelIrDecodeError> {
        if !self.tracks_allocations() {
            return Ok(Vec::with_capacity(count));
        }
        let bytes =
            count
                .checked_mul(std::mem::size_of::<T>())
                .ok_or(KernelIrDecodeError::Resource(
                    crate::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
                ))?;
        self.reserve_payload(bytes)?;
        let mut values = Vec::new();
        values.try_reserve_exact(count).map_err(|_| {
            KernelIrDecodeError::Resource(
                crate::CanonicalKernelIrVerificationResourceErrorV1::Allocation,
            )
        })?;
        if values.capacity() != count {
            return Err(KernelIrDecodeError::Resource(
                crate::CanonicalKernelIrVerificationResourceErrorV1::Allocation,
            ));
        }
        Ok(values)
    }
    fn tree<T>(&mut self, count: usize) -> Result<(), KernelIrDecodeError> {
        if self.tracks_allocations() {
            self.reserve_payload(decoded_tree_payload_bound_v12::<T>(count)?)?;
        }
        Ok(())
    }

}

/// Private parser entry; only full checked canonical constructors expose output.
pub(crate) fn decode_module_v12_with_allocation_budget_v1(
    bytes: &[u8],
    budget: &mut crate::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<Module, KernelIrDecodeError> {
    decode_module_impl_v1(
        bytes,
        KERNEL_IR_VERSION_V12,
        false,
        Some(DecodeBudgetV12::Resources(budget)),
    )
}

#[cfg(test)]
mod decode_allocation_tests_v12 {
    use super::*;
    use crate::{
        CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
    };

    #[test]
    fn typed_count_charge_precedes_host_allocation_and_keeps_prefix() {
        let bytes = 5 * std::mem::size_of::<Operation>();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, bytes + 6);
        budget.reserve_storage(7).unwrap();
        {
            let mut reader = Reader::new(&[], Some(DecodeBudgetV12::Resources(&mut budget)));
            assert!(matches!(
                reader.vector::<Operation>(5),
                Err(KernelIrDecodeError::Resource(
                    CanonicalKernelIrVerificationResourceErrorV1::Storage(_)
                ))
            ));
        }
        assert_eq!(budget.storage(), 7);
        assert_eq!(budget.failed_storage(), Some(bytes + 7));
        assert_eq!(budget.work(), 0);
    }

    #[test]
    fn identifiers_charge_only_string_buffers_before_construction() {
        let text = "identity";
        let mut bytes = (text.len() as u32).to_le_bytes().to_vec();
        bytes.extend_from_slice(text.as_bytes());
        let payload = text.len();
        for (limit, success) in [(payload, true), (payload - 1, false)] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, limit);
            let result = {
                let mut reader = Reader::new(&bytes, Some(DecodeBudgetV12::Resources(&mut budget)));
                reader.text("test")
            };
            assert_eq!(result.is_ok(), success);
            assert_eq!(budget.storage(), if success { payload } else { 0 });
            assert_eq!(budget.work(), 4 + 3 * text.len());
        }
    }

    #[test]
    fn nested_pointer_and_slice_boxes_have_independent_preallocation_charges() {
        let ty = Type::pointer(
            Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadOnly),
            AddressSpace::Private,
            AccessMode::ReadWrite,
        );
        let mut writer = Writer::new(KERNEL_IR_VERSION_V12, None);
        encode_type(&mut writer, &ty, 0).unwrap();
        let bytes = writer.bytes;
        let payload = 2 * std::mem::size_of::<Type>();
        for (limit, success) in [(payload, true), (payload - 1, false)] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, limit);
            let result = {
                let mut reader = Reader::new(&bytes, Some(DecodeBudgetV12::Resources(&mut budget)));
                reader.version = KERNEL_IR_VERSION_V12;
                decode_type(&mut reader, 0)
            };
            assert_eq!(result.is_ok(), success);
            if success {
                assert_eq!(result.unwrap(), ty);
            }
            assert_eq!(
                budget.storage(),
                if success { payload } else { payload / 2 }
            );
        }
    }

    #[test]
    fn capability_tree_and_temporary_owned_order_key_coexist() {
        let capabilities = [TargetCapability::Extension {
            namespace: "space".to_owned(),
            name: "name".to_owned(),
        }]
        .into_iter()
        .collect();
        let mut writer = Writer::new(KERNEL_IR_VERSION_V12, None);
        encode_capabilities(&mut writer, &capabilities).unwrap();
        let tree = decoded_tree_payload_bound_v12::<TargetCapability>(1).unwrap();
        let strings = 9;
        let peak = tree + 2 * strings;
        for (limit, success) in [(peak, true), (peak - 1, false)] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, limit);
            let result = {
                let mut reader =
                    Reader::new(&writer.bytes, Some(DecodeBudgetV12::Resources(&mut budget)));
                reader.version = KERNEL_IR_VERSION_V12;
                decode_capabilities(&mut reader)
            };
            assert_eq!(result.is_ok(), success);
            assert_eq!(budget.storage(), tree + strings);
            if success {
                assert_eq!(result.unwrap(), capabilities);
                assert_eq!(budget.peak_storage(), peak);
            } else {
                assert_eq!(budget.failed_storage(), Some(peak));
            }
        }
    }
}
