use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
    Function, FunctionId, Kernel, Module, VerificationFunctionIndexRowV1,
    VerificationFunctionIndexV1, verification_bounded_sort_by_v1, verification_find_last_by_v1,
};

#[derive(Clone, Copy)]
pub(crate) struct VerificationKernelRowV1<'module> {
    pub(crate) kernel: &'module Kernel,
    pub(crate) input_ordinal: usize,
}

pub(crate) struct VerificationModuleStateV1<'module> {
    functions: VerificationFunctionIndexV1<'module>,
    kernel_rows: Vec<VerificationKernelRowV1<'module>>,
    referenced_entries: Vec<&'module FunctionId>,
    maximum_entry_identifier_bytes: usize,
    retained_storage: usize,
}

impl<'module> VerificationModuleStateV1<'module> {
    const KERNEL_ROW_STORAGE: usize = 2;
    const REFERENCED_ENTRY_ROW_STORAGE: usize = 1;

    pub(crate) fn build(
        module: &'module Module,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Self, CanonicalKernelIrVerificationResourceErrorV1> {
        let checkpoint = budget.storage_checkpoint();
        match Self::build_inner(module, budget) {
            Ok(built) => Ok(built),
            Err(error) => {
                let _ = budget.rollback_storage(checkpoint);
                Err(error)
            }
        }
    }

    fn build_inner(
        module: &'module Module,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Self, CanonicalKernelIrVerificationResourceErrorV1> {
        let functions = VerificationFunctionIndexV1::build(module, budget)?;
        let kernel_count = module.kernels.len();
        budget.charge_work(kernel_count)?;
        let mut maximum_kernel_identifier_bytes = 0_usize;
        let mut maximum_entry_identifier_bytes = 0_usize;
        for kernel in &module.kernels {
            maximum_kernel_identifier_bytes =
                maximum_kernel_identifier_bytes.max(kernel.id.as_str().len());
            maximum_entry_identifier_bytes =
                maximum_entry_identifier_bytes.max(kernel.entry.as_str().len());
        }
        let retained_storage = kernel_count
            .checked_mul(
                Self::KERNEL_ROW_STORAGE
                    .checked_add(Self::REFERENCED_ENTRY_ROW_STORAGE)
                    .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?,
            )
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        budget.charge_work(kernel_count)?;
        budget.reserve_storage(retained_storage)?;
        let mut kernel_rows = Vec::new();
        if kernel_rows.try_reserve_exact(kernel_count).is_err() {
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Allocation);
        }
        let mut referenced_entries = Vec::new();
        if referenced_entries.try_reserve_exact(kernel_count).is_err() {
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Allocation);
        }
        for (input_ordinal, kernel) in module.kernels.iter().enumerate() {
            kernel_rows.push(VerificationKernelRowV1 {
                kernel,
                input_ordinal,
            });
            referenced_entries.push(&kernel.entry);
        }

        let kernel_comparison_width = maximum_kernel_identifier_bytes
            .checked_add(2)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        verification_bounded_sort_by_v1(
            &mut kernel_rows,
            kernel_comparison_width,
            budget,
            |left, right| {
                left.kernel
                    .id
                    .cmp(&right.kernel.id)
                    .then_with(|| left.input_ordinal.cmp(&right.input_ordinal))
            },
        )?;
        let entry_comparison_width = maximum_entry_identifier_bytes
            .checked_add(1)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        verification_bounded_sort_by_v1(
            &mut referenced_entries,
            entry_comparison_width,
            budget,
            |left, right| left.cmp(right),
        )?;

        Ok(Self {
            functions,
            kernel_rows,
            referenced_entries,
            maximum_entry_identifier_bytes,
            retained_storage,
        })
    }

    pub(crate) fn function_rows(&self) -> &[VerificationFunctionIndexRowV1<'module>] {
        self.functions.rows()
    }

    pub(crate) fn find_function(
        &self,
        needle: &FunctionId,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Option<&'module Function>, CanonicalKernelIrVerificationResourceErrorV1> {
        self.functions.find(needle, budget)
    }

    pub(crate) fn find_function_row(
        &self,
        needle: &FunctionId,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        Option<VerificationFunctionIndexRowV1<'module>>,
        CanonicalKernelIrVerificationResourceErrorV1,
    > {
        self.functions.find_row(needle, budget)
    }

    pub(crate) fn kernel_rows(&self) -> &[VerificationKernelRowV1<'module>] {
        &self.kernel_rows
    }

    pub(crate) fn charge_function_duplicate_comparison(
        &self,
        left: &FunctionId,
        right: &FunctionId,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let width = left
            .as_str()
            .len()
            .max(right.as_str().len())
            .checked_add(1)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        budget.charge_work(width)
    }

    pub(crate) fn charge_kernel_duplicate_comparison(
        &self,
        left: &Kernel,
        right: &Kernel,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        let width = left
            .id
            .as_str()
            .len()
            .max(right.id.as_str().len())
            .checked_add(1)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        budget.charge_work(width)
    }

    pub(crate) fn entry_is_referenced(
        &self,
        entry: &FunctionId,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<bool, CanonicalKernelIrVerificationResourceErrorV1> {
        let width = self
            .maximum_entry_identifier_bytes
            .max(entry.as_str().len())
            .checked_add(1)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        verification_find_last_by_v1(&self.referenced_entries, width, budget, |candidate| {
            (*candidate).cmp(entry)
        })
        .map(|ordinal| ordinal.is_some())
    }

    pub(crate) fn release(
        self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        drop(self.kernel_rows);
        drop(self.referenced_entries);
        let retained_result = budget.release_storage(self.retained_storage);
        let function_result = self.functions.release(budget);
        retained_result.and(function_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CanonicalKernelIrWorkBudgetV1, FunctionRole, Kernel, LaunchDomain, Signature, Type,
    };

    fn declaration(id: &str) -> Function {
        Function {
            id: id.into(),
            signature: Signature::new(vec![Type::INDEX], vec![]),
            role: FunctionRole::ExternalImport,
            body: None,
            required_capabilities: Default::default(),
        }
    }

    fn module() -> Module {
        let common = "x".repeat(256);
        let first = FunctionId::new(format!("{common}a"));
        let second = FunctionId::new(format!("{common}b"));
        Module {
            id: "module".into(),
            functions: vec![declaration(first.as_str()), declaration(second.as_str())],
            kernels: vec![
                Kernel {
                    id: "kernel-b".into(),
                    entry: second,
                    domain: LaunchDomain::D1 {
                        x: crate::LaunchExtent::Dynamic,
                    },
                    workgroup_size: None,
                    required_capabilities: Default::default(),
                },
                Kernel {
                    id: "kernel-a".into(),
                    entry: first,
                    domain: LaunchDomain::D1 {
                        x: crate::LaunchExtent::Dynamic,
                    },
                    workgroup_size: None,
                    required_capabilities: Default::default(),
                },
            ],
            required_capabilities: Default::default(),
        }
    }

    #[test]
    fn long_prefix_module_rows_are_bounded_and_referenced() {
        // Function census/fill + width-259 two-row sort; kernel census/fill +
        // width-10 sort; width-258 referenced-entry sort. Each of the two
        // entry and function queries takes respectively three and two
        // comparisons, plus its fixed query action.
        const EXACT_BUILD_WORK: usize = 2 + 2 + 4 * 2 * 259 + 2 + 2 + 4 * 2 * 10 + 4 * 2 * 258;
        const EXACT_QUERY_WORK: usize = 2 * (1 + 3 * 258) + 2 * (1 + 2 * 258);
        const EXACT_WORK: usize = EXACT_BUILD_WORK + EXACT_QUERY_WORK;
        const EXACT_STORAGE: usize = 2 * 2 + 2 * (2 + 1);
        let module = module();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, EXACT_STORAGE - 1);
        assert!(matches!(
            VerificationModuleStateV1::build(&module, &mut budget),
            Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(error))
                if error.actual() == EXACT_STORAGE && error.limit() == EXACT_STORAGE - 1
        ));
        assert_eq!(budget.storage(), 0);

        let mut work = CanonicalKernelIrWorkBudgetV1::new(EXACT_WORK - 1);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, EXACT_STORAGE);
        let state = VerificationModuleStateV1::build(&module, &mut budget).unwrap();
        assert_eq!(state.function_rows().len(), 2);
        assert_eq!(state.kernel_rows()[0].kernel.id.as_str(), "kernel-a");
        for function in &module.functions {
            assert!(
                state
                    .entry_is_referenced(&function.id, &mut budget)
                    .unwrap()
            );
        }
        assert_eq!(
            state
                .find_function(&module.functions[0].id, &mut budget)
                .unwrap()
                .unwrap()
                .id,
            module.functions[0].id
        );
        assert!(matches!(
            state.find_function(&module.functions[1].id, &mut budget),
            Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                if error.actual() == EXACT_WORK && error.limit() == EXACT_WORK - 1
        ));
        state.release(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);

        let mut work = CanonicalKernelIrWorkBudgetV1::new(EXACT_WORK);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, EXACT_STORAGE);
        let state = VerificationModuleStateV1::build(&module, &mut budget).unwrap();
        for function in &module.functions {
            assert!(
                state
                    .entry_is_referenced(&function.id, &mut budget)
                    .unwrap()
            );
            assert_eq!(
                state
                    .find_function(&function.id, &mut budget)
                    .unwrap()
                    .unwrap()
                    .id,
                function.id
            );
        }
        state.release(&mut budget).unwrap();
        assert_eq!(budget.work(), EXACT_WORK);
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.peak_storage(), EXACT_STORAGE);
    }
}
