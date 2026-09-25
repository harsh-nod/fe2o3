//! Additional storage for a retained copy of the original sealed packing plan.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::mem::size_of_val;

impl GeneratedArgumentPackingPlanV1 {
    /// Includes the owner shell, both boxed arrays, each cloned field name and
    /// conservative shared-seal retention. The seal allocation is shared, but
    /// must stay covered even when the original plan drops. Reserves nothing.
    pub(crate) fn conditional_clone_storage_v1(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<usize, Resource> {
        budget.charge_work(self.fields.len())?;
        let mut bytes = size_of::<Self>()
            .checked_add(size_of_val(&*self.fields))
            .and_then(|n| n.checked_add(size_of_val(&*self.components)))
            .and_then(|n| {
                n.checked_add(
                    2 * size_of::<usize>() + size_of::<GeneratedArgumentPackingPlanSealV1>(),
                )
            })
            .ok_or(Resource::Arithmetic)?;
        for field in &self.fields {
            bytes = bytes
                .checked_add(field.name().as_str().len())
                .ok_or(Resource::Arithmetic)?;
        }
        Ok(bytes)
    }

    /// Returns an unreserved addition on the same cumulative account. The
    /// caller must reserve it before keeping the clone, including on error paths.
    pub(crate) fn clone_for_conditional_v1(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, usize), Resource> {
        let bytes = self.conditional_clone_storage_v1(budget)?;
        budget.with_prepaid_scope(budget.storage(), 1, bytes, bytes, |_| {
            Ok((self.clone(), bytes))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_artifacts::Name;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    fn plan() -> GeneratedArgumentPackingPlanV1 {
        let field = AbiField::new(
            Name::new("count_with_retained_name_storage").unwrap(),
            0,
            4,
            4,
            AbiKind::Scalar(ScalarType::U32),
            Mutability::Immutable,
            Access::ByValue,
            AddressSpace::Value,
            u32::scalar_type_identity_v1(PointerWidth::Bits64),
            ArgumentOwnership::ByValue,
            AliasClass::Value,
        )
        .unwrap();
        let layout = AbiLayout::new(8, 8, PointerWidth::Bits64, vec![field]).unwrap();
        packing_plan_from_layout(KernelId::from_bytes([42; 32]), &layout)
    }

    #[test]
    fn clone_quote_covers_deep_fields_components_names_and_shared_seal() {
        let original = plan();
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let bytes = original.conditional_clone_storage_v1(&mut budget).unwrap();
        assert_eq!(
            bytes,
            size_of::<GeneratedArgumentPackingPlanV1>()
                + size_of::<AbiField>()
                + size_of::<GeneratedPackingComponentV1>()
                + "count_with_retained_name_storage".len()
                + 2 * size_of::<usize>()
                + size_of::<GeneratedArgumentPackingPlanSealV1>()
        );
        let (copy, retained) = original.clone_for_conditional_v1(&mut budget).unwrap();
        assert_eq!(retained, bytes);
        assert_eq!(budget.storage(), 0);
        budget.reserve_storage(retained).unwrap();
        assert!(Arc::ptr_eq(&copy.seal, &original.seal));
        assert_ne!(copy.fields.as_ptr(), original.fields.as_ptr());
        assert_ne!(copy.components.as_ptr(), original.components.as_ptr());
        assert_ne!(
            copy.fields[0].name().as_str().as_ptr(),
            original.fields[0].name().as_str().as_ptr()
        );
        let input = original.scalar(0, 7_u32).unwrap();
        drop(original);
        assert!(copy.pack([input]).is_ok());
        assert!(copy.pack([plan().scalar(0, 7_u32).unwrap()]).is_err());
        drop(copy);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn clone_exact_and_one_short_work_and_storage_keep_prefix_and_denial_history() {
        let original = plan();
        let run = |work_limit, storage_limit| {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.charge_work(19).unwrap();
            budget.reserve_storage(73).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = original.clone_for_conditional_v1(&mut budget);
            let ok = result.is_ok();
            drop(result);
            assert_eq!(budget.storage(), 73);
            assert!(ledger == budget.work_ledger_identity_v1());
            (
                ok,
                budget.work(),
                budget.peak_storage(),
                budget.failed_work(),
                budget.failed_storage(),
            )
        };
        let (ok, work, storage, _, _) = run(1_000_000, 1_000_000);
        assert!(ok);
        assert_eq!(run(work, storage), (true, work, storage, None, None));
        let short_work = run(work - 1, storage);
        assert!(!short_work.0 && short_work.3.is_some());
        let short_storage = run(work, storage - 1);
        assert!(!short_storage.0 && short_storage.4.is_some());
    }
}
