use std::fmt::{self, Write as _};
use std::mem::size_of;

use crate::{
    BlockId, CanonicalKernelIrVerificationResourceBudgetV1,
    CanonicalKernelIrVerificationResourceErrorV1, Diagnostic, DiagnosticCode, DiagnosticLocation,
    Function, FunctionId, Kernel, KernelId, Module, ModuleId, verification_bounded_sort_by_v1,
};

// Fixed rows use the verifier's machine-word cell convention. String payloads
// are charged separately in bytes. This excludes allocator metadata and RSS.
const DIAGNOSTIC_ROW_STORAGE_V1: usize = size_of::<Diagnostic>().div_ceil(size_of::<usize>());

/// Internal locations borrow the input module; only collected errors own IDs.
#[derive(Clone, Copy, Debug)]
pub(crate) struct VerificationDiagnosticLocationV1<'m> {
    pub(crate) module: &'m ModuleId,
    pub(crate) function: Option<&'m FunctionId>,
    pub(crate) kernel: Option<&'m KernelId>,
    pub(crate) block: Option<BlockId>,
    pub(crate) operation: Option<usize>,
}

impl<'m> VerificationDiagnosticLocationV1<'m> {
    pub(crate) const fn module(module: &'m Module) -> Self {
        Self {
            module: &module.id,
            function: None,
            kernel: None,
            block: None,
            operation: None,
        }
    }

    pub(crate) const fn function(module: &'m Module, function: &'m Function) -> Self {
        Self {
            function: Some(&function.id),
            ..Self::module(module)
        }
    }

    pub(crate) const fn kernel(module: &'m Module, kernel: &'m Kernel) -> Self {
        Self {
            kernel: Some(&kernel.id),
            ..Self::module(module)
        }
    }

    pub(crate) const fn at_block(mut self, block: BlockId) -> Self {
        self.block = Some(block);
        self
    }

    pub(crate) const fn at_operation(mut self, operation: usize) -> Self {
        self.operation = Some(operation);
        self
    }

    fn copy_plan(self) -> Result<(usize, usize), CanonicalKernelIrVerificationResourceErrorV1> {
        let mut storage = 0_usize;
        // Five location fields, plus reserve/construction and byte copying for
        // every present identifier, including present empty identifiers.
        let mut work = 5_usize;
        for value in [
            Some(self.module.as_str()),
            self.function.map(FunctionId::as_str),
            self.kernel.map(KernelId::as_str),
        ]
        .into_iter()
        .flatten()
        {
            storage = storage
                .checked_add(value.len())
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
            work = value
                .len()
                .checked_add(2)
                .and_then(|copy| work.checked_add(copy))
                .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        }
        Ok((storage, work))
    }
}

struct DiagnosticLengthCounterV1 {
    bytes: usize,
}

impl fmt::Write for DiagnosticLengthCounterV1 {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.bytes = self.bytes.checked_add(value.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}

/// Formatting may diverge from its counting pass. Never append an unadmitted
/// chunk, even when the Display implementation ignores an earlier fmt::Error.
struct DiagnosticBoundedWriterV1<'a> {
    message: &'a mut String,
    maximum: usize,
    failed: bool,
}

impl fmt::Write for DiagnosticBoundedWriterV1<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let next = self.message.len().checked_add(value.len());
        if self.failed
            || next.is_none_or(|next| next > self.maximum || next > self.message.capacity())
        {
            self.failed = true;
            return Err(fmt::Error);
        }
        self.message.push_str(value);
        Ok(())
    }
}

enum VerificationDiagnosticCollectorModeV1 {
    Count {
        count: usize,
    },
    Materialize {
        expected: usize,
        diagnostics: Vec<Diagnostic>,
        buffer_storage: usize,
    },
}

/// Two-pass diagnostic collector used only by metered module verification.
///
/// Count formats into a nonallocating counter and never copies identifiers.
/// Materialize owns one exact-capacity roster plus exact-capacity identifier
/// and message buffers. The caller-owned input module is outside this ledger.
/// Exact capacities use the pinned Rust allocator contract, not physical RSS.
pub(crate) struct VerificationDiagnosticCollectorV1 {
    mode: VerificationDiagnosticCollectorModeV1,
    #[cfg(test)]
    failure: Option<DiagnosticFailurePointV1>,
}

impl VerificationDiagnosticCollectorV1 {
    pub(crate) const fn count() -> Self {
        Self {
            mode: VerificationDiagnosticCollectorModeV1::Count { count: 0 },
            #[cfg(test)]
            failure: None,
        }
    }

    pub(crate) fn materialize(
        expected: usize,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Self, CanonicalKernelIrVerificationResourceErrorV1> {
        let row_storage = expected
            .checked_mul(DIAGNOSTIC_ROW_STORAGE_V1)
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        budget.reserve_storage(row_storage)?;
        let mut diagnostics = Vec::new();
        let allocation = diagnostics.try_reserve_exact(expected);
        if allocation.is_err() || diagnostics.capacity() != expected {
            let error = if allocation.is_err() {
                CanonicalKernelIrVerificationResourceErrorV1::Allocation
            } else {
                CanonicalKernelIrVerificationResourceErrorV1::Accounting
            };
            drop(diagnostics);
            budget.release_storage(row_storage)?;
            return Err(error);
        }
        Ok(Self {
            mode: VerificationDiagnosticCollectorModeV1::Materialize {
                expected,
                diagnostics,
                buffer_storage: 0,
            },
            #[cfg(test)]
            failure: None,
        })
    }

    pub(crate) fn emit(
        &mut self,
        location: VerificationDiagnosticLocationV1<'_>,
        code: DiagnosticCode,
        message_work_upper: usize,
        arguments: fmt::Arguments<'_>,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        budget.charge_work(1)?;
        budget.charge_work(message_work_upper)?;
        let mut counter = DiagnosticLengthCounterV1 { bytes: 0 };
        counter
            .write_fmt(arguments)
            .map_err(|_| CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
        if counter.bytes > message_work_upper {
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
        }

        match &mut self.mode {
            VerificationDiagnosticCollectorModeV1::Count { count } => {
                *count = count
                    .checked_add(1)
                    .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
            }
            VerificationDiagnosticCollectorModeV1::Materialize {
                expected,
                diagnostics,
                buffer_storage,
            } => {
                if diagnostics.len() >= *expected || diagnostics.capacity() != *expected {
                    return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
                }
                let (identifier_storage, copy_work) = location.copy_plan()?;
                let pending_storage = identifier_storage
                    .checked_add(counter.bytes)
                    .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
                let next_buffer_storage = buffer_storage
                    .checked_add(pending_storage)
                    .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
                let materialization_work = message_work_upper
                    .checked_add(counter.bytes)
                    .and_then(|work| work.checked_add(1))
                    .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
                budget.charge_work(copy_work)?;
                budget.charge_work(materialization_work)?;
                budget.reserve_storage(pending_storage)?;

                let construction = DiagnosticConstructionV1 {
                    #[cfg(test)]
                    failure: self.failure,
                };
                // A failed construction returns only after all partial owners
                // have dropped. The roster and older buffers remain charged.
                let pending = construction.build(location, code, counter.bytes, arguments);
                match pending {
                    Ok(diagnostic) => {
                        diagnostics.push(diagnostic);
                        *buffer_storage = next_buffer_storage;
                    }
                    Err(error) => {
                        budget.release_storage(pending_storage)?;
                        return Err(error);
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn counted(&self) -> Option<usize> {
        match self.mode {
            VerificationDiagnosticCollectorModeV1::Count { count } => Some(count),
            VerificationDiagnosticCollectorModeV1::Materialize { .. } => None,
        }
    }

    pub(crate) fn finish_materialized(
        self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<Vec<Diagnostic>, CanonicalKernelIrVerificationResourceErrorV1> {
        let VerificationDiagnosticCollectorModeV1::Materialize {
            expected,
            mut diagnostics,
            buffer_storage,
        } = self.mode
        else {
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
        };
        if diagnostics.len() != expected {
            drop(diagnostics);
            release_diagnostic_storage_v1(expected, buffer_storage, budget)?;
            return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
        }

        let finish = (|| {
            budget.charge_work(diagnostics.len())?;
            let comparison_width = diagnostics
                .iter()
                .map(diagnostic_comparison_width_v1)
                .try_fold(1_usize, |maximum, width| {
                    width.map(|width| maximum.max(width))
                })?;
            verification_bounded_sort_by_v1(
                &mut diagnostics,
                comparison_width,
                budget,
                Diagnostic::cmp,
            )
        })();
        if let Err(error) = finish {
            drop(diagnostics);
            release_diagnostic_storage_v1(expected, buffer_storage, budget)?;
            return Err(error);
        }

        // The completed roster transfers to the caller; every owner remained
        // charged throughout comparison-width inspection and in-place sorting.
        if let Err(error) = release_diagnostic_storage_v1(expected, buffer_storage, budget) {
            drop(diagnostics);
            return Err(error);
        }
        Ok(diagnostics)
    }

    pub(crate) fn abandon(
        self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        match self.mode {
            VerificationDiagnosticCollectorModeV1::Count { .. } => Ok(()),
            VerificationDiagnosticCollectorModeV1::Materialize {
                expected,
                diagnostics,
                buffer_storage,
            } => {
                drop(diagnostics);
                release_diagnostic_storage_v1(expected, buffer_storage, budget)
            }
        }
    }
}

// Failure injection is local to one test collector, never an allocator hook or
// process-global switch. The production construction carries no extra state.
struct DiagnosticConstructionV1 {
    #[cfg(test)]
    failure: Option<DiagnosticFailurePointV1>,
}

impl DiagnosticConstructionV1 {
    fn build(
        &self,
        location: VerificationDiagnosticLocationV1<'_>,
        code: DiagnosticCode,
        message_bytes: usize,
        arguments: fmt::Arguments<'_>,
    ) -> Result<Diagnostic, CanonicalKernelIrVerificationResourceErrorV1> {
        #[cfg(test)]
        self.check(DiagnosticFailurePointV1::ModuleReserve)?;
        let module = ModuleId::new(copy_diagnostic_string_v1(location.module.as_str())?);

        let function = if let Some(function) = location.function {
            #[cfg(test)]
            self.check(DiagnosticFailurePointV1::FunctionReserve)?;
            Some(FunctionId::new(copy_diagnostic_string_v1(
                function.as_str(),
            )?))
        } else {
            None
        };
        let kernel = if let Some(kernel) = location.kernel {
            #[cfg(test)]
            self.check(DiagnosticFailurePointV1::KernelReserve)?;
            Some(KernelId::new(copy_diagnostic_string_v1(kernel.as_str())?))
        } else {
            None
        };
        #[cfg(test)]
        self.check(DiagnosticFailurePointV1::MessageReserve)?;
        let mut message = reserve_diagnostic_string_v1(message_bytes)?;
        {
            let mut writer = DiagnosticBoundedWriterV1 {
                message: &mut message,
                maximum: message_bytes,
                failed: false,
            };
            if writer.write_fmt(arguments).is_err()
                || writer.failed
                || writer.message.len() != message_bytes
            {
                return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
            }
        }
        let diagnostic = Diagnostic {
            location: DiagnosticLocation {
                module,
                function,
                kernel,
                block: location.block,
                operation: location.operation,
            },
            code,
            message,
        };
        #[cfg(test)]
        if let Err(error) = self.check(DiagnosticFailurePointV1::Publication) {
            drop(diagnostic);
            return Err(error);
        }
        Ok(diagnostic)
    }

    #[cfg(test)]
    fn check(
        &self,
        point: DiagnosticFailurePointV1,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        if self.failure == Some(point) {
            Err(if point == DiagnosticFailurePointV1::Publication {
                CanonicalKernelIrVerificationResourceErrorV1::Accounting
            } else {
                CanonicalKernelIrVerificationResourceErrorV1::Allocation
            })
        } else {
            Ok(())
        }
    }
}

fn reserve_diagnostic_string_v1(
    bytes: usize,
) -> Result<String, CanonicalKernelIrVerificationResourceErrorV1> {
    let mut value = String::new();
    value
        .try_reserve_exact(bytes)
        .map_err(|_| CanonicalKernelIrVerificationResourceErrorV1::Allocation)?;
    if value.capacity() != bytes {
        return Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting);
    }
    Ok(value)
}

fn copy_diagnostic_string_v1(
    source: &str,
) -> Result<String, CanonicalKernelIrVerificationResourceErrorV1> {
    let mut value = reserve_diagnostic_string_v1(source.len())?;
    value.push_str(source);
    Ok(value)
}

fn diagnostic_comparison_width_v1(
    diagnostic: &Diagnostic,
) -> Result<usize, CanonicalKernelIrVerificationResourceErrorV1> {
    diagnostic
        .location
        .module
        .as_str()
        .len()
        .checked_add(
            diagnostic
                .location
                .function
                .as_ref()
                .map_or(0, |function| function.as_str().len()),
        )
        .and_then(|width| {
            width.checked_add(
                diagnostic
                    .location
                    .kernel
                    .as_ref()
                    .map_or(0, |kernel| kernel.as_str().len()),
            )
        })
        .and_then(|width| width.checked_add(diagnostic.message.len()))
        // Three string terminal decisions, four optional-field tags, two
        // numeric coordinates, the code, and message termination fit here.
        .and_then(|width| width.checked_add(16))
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)
}

fn release_diagnostic_storage_v1(
    count: usize,
    buffer_storage: usize,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
    let storage = count
        .checked_mul(DIAGNOSTIC_ROW_STORAGE_V1)
        .and_then(|storage| storage.checked_add(buffer_storage))
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    budget.release_storage(storage)
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiagnosticFailurePointV1 {
    ModuleReserve,
    FunctionReserve,
    KernelReserve,
    MessageReserve,
    Publication,
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::CanonicalKernelIrWorkBudgetV1;

    fn location<'m>(
        module: &'m ModuleId,
        function: Option<&'m FunctionId>,
        kernel: Option<&'m KernelId>,
    ) -> VerificationDiagnosticLocationV1<'m> {
        VerificationDiagnosticLocationV1 {
            module,
            function,
            kernel,
            block: None,
            operation: None,
        }
    }

    fn emit_text(
        collector: &mut VerificationDiagnosticCollectorV1,
        location: VerificationDiagnosticLocationV1<'_>,
        message: &str,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), CanonicalKernelIrVerificationResourceErrorV1> {
        collector.emit(
            location,
            DiagnosticCode::InvalidIdentity,
            message.len(),
            format_args!("{message}"),
            budget,
        )
    }

    fn retained_count(collector: &VerificationDiagnosticCollectorV1) -> usize {
        match &collector.mode {
            VerificationDiagnosticCollectorModeV1::Materialize { diagnostics, .. } => {
                diagnostics.len()
            }
            VerificationDiagnosticCollectorModeV1::Count { .. } => panic!("not materialized"),
        }
    }

    fn source_string(value: &str, spare: usize) -> String {
        let mut source = String::with_capacity(value.len() + spare);
        source.push_str(value);
        source
    }

    #[test]
    fn fixed_rows_cover_the_string_owned_layout_and_borrowed_locations_are_copy() {
        fn require_copy<T: Copy>() {}
        require_copy::<VerificationDiagnosticLocationV1<'static>>();
        assert!(DIAGNOSTIC_ROW_STORAGE_V1 * size_of::<usize>() >= size_of::<Diagnostic>());
        assert!((DIAGNOSTIC_ROW_STORAGE_V1 - 1) * size_of::<usize>() < size_of::<Diagnostic>());
        let module = Module::new("module");
        let loc = VerificationDiagnosticLocationV1::module(&module)
            .at_block(BlockId(7))
            .at_operation(11);
        assert!(std::ptr::eq(loc.module, &module.id));
        assert_eq!(loc.block, Some(BlockId(7)));
        assert_eq!(loc.operation, Some(11));
    }

    #[test]
    fn long_identifiers_are_not_copied_or_charged_in_count_mode() {
        let module = ModuleId::new(source_string(&"m".repeat(4096), 8192));
        let function = FunctionId::new(source_string(&"f".repeat(8192), 16384));
        let kernel = KernelId::new(source_string(&"k".repeat(16384), 32768));
        let loc = location(&module, Some(&function), Some(&kernel));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(2);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        let mut collector = VerificationDiagnosticCollectorV1::count();
        // Any attempt to enter materialization would fail this collector.
        collector.failure = Some(DiagnosticFailurePointV1::ModuleReserve);
        emit_text(&mut collector, loc, "x", &mut budget).unwrap();
        assert_eq!(collector.counted(), Some(1));
        assert_eq!(budget.work(), 2);
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.peak_storage(), 0);
        collector.abandon(&mut budget).unwrap();
    }

    #[test]
    fn exact_two_pass_accounting_covers_all_location_shapes() {
        let module = ModuleId::new("module");
        let function = FunctionId::new("function");
        let kernel = KernelId::new("kernel");
        let message = "diagnostic";
        for (function, kernel, length, present) in [
            (None, None, 6, 1),
            (Some(&function), None, 14, 2),
            (None, Some(&kernel), 12, 2),
            (Some(&function), Some(&kernel), 20, 3),
        ] {
            let loc = location(&module, function, kernel)
                .at_block(BlockId(3))
                .at_operation(9);
            let copy_work = 5 + length + 2 * present;
            let count_work = 1 + message.len();
            let materialize_work = count_work + copy_work + 2 * message.len() + 1;
            let exact_work = count_work + materialize_work + 1;
            let exact_storage = DIAGNOSTIC_ROW_STORAGE_V1 + length + message.len();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_work);
            let mut budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, exact_storage);
            let mut count = VerificationDiagnosticCollectorV1::count();
            emit_text(&mut count, loc, message, &mut budget).unwrap();
            assert_eq!(count.counted(), Some(1));
            let mut collector =
                VerificationDiagnosticCollectorV1::materialize(1, &mut budget).unwrap();
            emit_text(&mut collector, loc, message, &mut budget).unwrap();
            let diagnostics = collector.finish_materialized(&mut budget).unwrap();
            assert_eq!(budget.work(), exact_work);
            assert_eq!(budget.storage(), 0);
            assert_eq!(budget.peak_storage(), exact_storage);
            assert_eq!(diagnostics[0].location.module.as_str(), "module");
            assert_eq!(
                diagnostics[0]
                    .location
                    .function
                    .as_ref()
                    .map(FunctionId::as_str),
                function.map(FunctionId::as_str)
            );
            assert_eq!(
                diagnostics[0]
                    .location
                    .kernel
                    .as_ref()
                    .map(KernelId::as_str),
                kernel.map(KernelId::as_str)
            );
            assert_eq!(diagnostics[0].location.block, Some(BlockId(3)));
            assert_eq!(diagnostics[0].location.operation, Some(9));
            assert_eq!(diagnostics[0].code, DiagnosticCode::InvalidIdentity);
            assert_eq!(diagnostics[0].message, message);
        }
    }

    #[test]
    fn materialized_ids_own_only_visible_bytes_and_outlive_sources() {
        let module_text = "module".repeat(1024);
        let function_text = "function".repeat(1024);
        let kernel_text = "kernel".repeat(1024);
        let module = ModuleId::new(source_string(&module_text, 8192));
        let function = FunctionId::new(source_string(&function_text, 16384));
        let kernel = KernelId::new(source_string(&kernel_text, 32768));
        assert!(module.retained_capacity_bytes() > module.as_str().len());
        assert!(function.retained_capacity_bytes() > function.as_str().len());
        assert!(kernel.retained_capacity_bytes() > kernel.as_str().len());
        let payload = module_text.len() + function_text.len() + kernel_text.len() + 1;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            DIAGNOSTIC_ROW_STORAGE_V1 + payload,
        );
        let mut collector = VerificationDiagnosticCollectorV1::materialize(1, &mut budget).unwrap();
        emit_text(
            &mut collector,
            location(&module, Some(&function), Some(&kernel)),
            "x",
            &mut budget,
        )
        .unwrap();
        let diagnostics = collector.finish_materialized(&mut budget).unwrap();
        let owned = &diagnostics[0].location;
        assert_ne!(owned.module.as_str().as_ptr(), module.as_str().as_ptr());
        assert_ne!(
            owned.function.as_ref().unwrap().as_str().as_ptr(),
            function.as_str().as_ptr()
        );
        assert_ne!(
            owned.kernel.as_ref().unwrap().as_str().as_ptr(),
            kernel.as_str().as_ptr()
        );
        assert_eq!(owned.module.retained_capacity_bytes(), module_text.len());
        assert_eq!(
            owned.function.as_ref().unwrap().retained_capacity_bytes(),
            function_text.len()
        );
        assert_eq!(
            owned.kernel.as_ref().unwrap().retained_capacity_bytes(),
            kernel_text.len()
        );
        assert_eq!(diagnostics[0].message.capacity(), 1);
        drop((module, function, kernel));
        assert_eq!(owned.module.as_str(), module_text);
        assert_eq!(owned.function.as_ref().unwrap().as_str(), function_text);
        assert_eq!(owned.kernel.as_ref().unwrap().as_str(), kernel_text);
        assert_eq!(budget.storage(), 0);
        assert_eq!(budget.peak_storage(), DIAGNOSTIC_ROW_STORAGE_V1 + payload);
    }

    #[test]
    fn empty_identifiers_drop_source_spare_capacity_and_still_charge_copy_work() {
        let module = ModuleId::new(String::with_capacity(4096));
        let function = FunctionId::new(String::with_capacity(8192));
        let kernel = KernelId::new(String::with_capacity(16384));
        // Empty-message emit: roster action 1, location work 5+2*3,
        // message construction 1; finish adds one row.
        let mut work = CanonicalKernelIrWorkBudgetV1::new(14);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            DIAGNOSTIC_ROW_STORAGE_V1,
        );
        let mut collector = VerificationDiagnosticCollectorV1::materialize(1, &mut budget).unwrap();
        emit_text(
            &mut collector,
            location(&module, Some(&function), Some(&kernel)),
            "",
            &mut budget,
        )
        .unwrap();
        let diagnostics = collector.finish_materialized(&mut budget).unwrap();
        assert_eq!(diagnostics[0].location.module.retained_capacity_bytes(), 0);
        assert_eq!(
            diagnostics[0]
                .location
                .function
                .as_ref()
                .unwrap()
                .retained_capacity_bytes(),
            0
        );
        assert_eq!(
            diagnostics[0]
                .location
                .kernel
                .as_ref()
                .unwrap()
                .retained_capacity_bytes(),
            0
        );
        assert_eq!(diagnostics[0].message.capacity(), 0);
        assert_eq!(budget.work(), 14);
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn exact_storage_and_one_under_fail_before_any_pending_allocation() {
        let module = ModuleId::new("mod");
        let function = FunctionId::new("func");
        let kernel = KernelId::new("kern");
        let loc = location(&module, Some(&function), Some(&kernel));
        let payload = 3 + 4 + 4 + 4;
        let exact = DIAGNOSTIC_ROW_STORAGE_V1 + payload;
        for limit in [DIAGNOSTIC_ROW_STORAGE_V1 - 1, exact - 1, exact] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, limit);
            let initialized = VerificationDiagnosticCollectorV1::materialize(1, &mut budget);
            if limit < DIAGNOSTIC_ROW_STORAGE_V1 {
                assert!(matches!(
                    initialized,
                    Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(error))
                        if error.actual() == DIAGNOSTIC_ROW_STORAGE_V1
                ));
                assert_eq!(budget.storage(), 0);
                continue;
            }
            let mut collector = initialized.unwrap();
            if limit < exact {
                collector.failure = Some(DiagnosticFailurePointV1::ModuleReserve);
                assert!(matches!(
                    emit_text(&mut collector, loc, "text", &mut budget),
                    Err(CanonicalKernelIrVerificationResourceErrorV1::Storage(error))
                        if error.actual() == exact && error.limit() == exact - 1
                ));
                assert_eq!(retained_count(&collector), 0);
                assert_eq!(budget.storage(), DIAGNOSTIC_ROW_STORAGE_V1);
                assert_eq!(budget.peak_storage(), DIAGNOSTIC_ROW_STORAGE_V1);
                collector.abandon(&mut budget).unwrap();
            } else {
                emit_text(&mut collector, loc, "text", &mut budget).unwrap();
                collector.finish_materialized(&mut budget).unwrap();
                assert_eq!(budget.peak_storage(), exact);
            }
            assert_eq!(budget.storage(), 0);
        }
    }

    #[test]
    fn copy_work_and_message_work_are_precharged_at_exact_boundaries() {
        let module = ModuleId::new("mod");
        let loc = location(&module, None, None);
        let initial_work = 1 + 4;
        let copy_work = 5 + 3 + 2;
        let extra_message_work = 4 + 4 + 1;
        let emit_work = initial_work + copy_work + extra_message_work;
        for (limit, accepted, attempted) in [
            (
                initial_work + copy_work - 1,
                initial_work,
                initial_work + copy_work,
            ),
            (emit_work - 1, initial_work + copy_work, emit_work),
            (emit_work, emit_work, emit_work + 1),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
                &mut work,
                DIAGNOSTIC_ROW_STORAGE_V1 + 3 + 4,
            );
            let mut collector =
                VerificationDiagnosticCollectorV1::materialize(1, &mut budget).unwrap();
            let emitted = emit_text(&mut collector, loc, "text", &mut budget);
            if limit < emit_work {
                assert!(matches!(
                    emitted,
                    Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                        if error.actual() == attempted && error.limit() == limit
                ));
                assert_eq!(budget.storage(), DIAGNOSTIC_ROW_STORAGE_V1);
                assert_eq!(budget.peak_storage(), DIAGNOSTIC_ROW_STORAGE_V1);
                collector.abandon(&mut budget).unwrap();
            } else {
                emitted.unwrap();
                assert!(matches!(
                    collector.finish_materialized(&mut budget),
                    Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                        if error.actual() == attempted && error.limit() == limit
                ));
            }
            assert_eq!(budget.work(), accepted);
            assert_eq!(budget.storage(), 0);
        }
    }

    #[test]
    fn every_partial_construction_failure_preserves_prior_owners_and_rolls_back() {
        let module = ModuleId::new("module");
        let function = FunctionId::new("function");
        let kernel = KernelId::new("kernel");
        let loc = location(&module, Some(&function), Some(&kernel));
        let payload = 6 + 8 + 6 + 1;
        let rows = 2 * DIAGNOSTIC_ROW_STORAGE_V1;
        for failure in [
            DiagnosticFailurePointV1::ModuleReserve,
            DiagnosticFailurePointV1::FunctionReserve,
            DiagnosticFailurePointV1::KernelReserve,
            DiagnosticFailurePointV1::MessageReserve,
            DiagnosticFailurePointV1::Publication,
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, rows + 2 * payload);
            let mut collector =
                VerificationDiagnosticCollectorV1::materialize(2, &mut budget).unwrap();
            emit_text(&mut collector, loc, "z", &mut budget).unwrap();
            collector.failure = Some(failure);
            assert_eq!(
                emit_text(&mut collector, loc, "a", &mut budget),
                Err(if failure == DiagnosticFailurePointV1::Publication {
                    CanonicalKernelIrVerificationResourceErrorV1::Accounting
                } else {
                    CanonicalKernelIrVerificationResourceErrorV1::Allocation
                })
            );
            assert_eq!(retained_count(&collector), 1);
            assert_eq!(budget.storage(), rows + payload);
            assert_eq!(budget.peak_storage(), rows + 2 * payload);
            // Retry proves the failed pending owner did not consume a slot or
            // leave a logical payload reservation behind.
            collector.failure = None;
            emit_text(&mut collector, loc, "a", &mut budget).unwrap();
            let diagnostics = collector.finish_materialized(&mut budget).unwrap();
            assert_eq!(diagnostics[0].message, "a");
            assert_eq!(diagnostics[1].message, "z");
            assert_eq!(budget.storage(), 0);
        }
    }

    #[test]
    fn bounded_writer_rejects_chunks_before_push_and_latches_failure() {
        let mut message = reserve_diagnostic_string_v1(2).unwrap();
        {
            let mut writer = DiagnosticBoundedWriterV1 {
                message: &mut message,
                maximum: 2,
                failed: false,
            };
            writer.write_str("a").unwrap();
            assert!(writer.write_str("bc").is_err());
            assert_eq!(writer.message.as_str(), "a");
            assert_eq!(writer.message.capacity(), 2);
            // Ignoring a previous error must not permit a later publication.
            assert!(writer.write_str("b").is_err());
            assert_eq!(writer.message.as_str(), "a");
        }
        assert_eq!(message.capacity(), 2);
        let mut unicode = reserve_diagnostic_string_v1(2).unwrap();
        let mut writer = DiagnosticBoundedWriterV1 {
            message: &mut unicode,
            maximum: 2,
            failed: false,
        };
        writer.write_str("\u{e9}").unwrap();
        assert!(writer.write_str("x").is_err());
        assert_eq!(writer.message.len(), 2);
        assert_eq!(writer.message.capacity(), 2);
    }

    struct DivergentDisplay {
        calls: Cell<usize>,
        second: &'static str,
        explicit_error: bool,
        swallow_oversize: bool,
    }

    impl fmt::Display for DivergentDisplay {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            let call = self.calls.get();
            self.calls.set(call + 1);
            if call == 0 {
                return formatter.write_str("aa");
            }
            if self.explicit_error {
                formatter.write_str("a")?;
                return Err(fmt::Error);
            }
            if self.swallow_oversize {
                let _ = formatter.write_str("oversize");
                let _ = formatter.write_str("aa");
                return Ok(());
            }
            formatter.write_str(self.second)
        }
    }

    #[test]
    fn divergent_formatting_drops_pending_ids_and_message_before_releasing_payload() {
        let module = ModuleId::new("module");
        let function = FunctionId::new("function");
        let kernel = KernelId::new("kernel");
        let loc = location(&module, Some(&function), Some(&kernel));
        let payload = 6 + 8 + 6 + 2;
        let rows = 2 * DIAGNOSTIC_ROW_STORAGE_V1;
        for (second, explicit_error, swallow_oversize) in [
            ("aaa", false, false),
            ("a", false, false),
            ("", true, false),
            ("", false, true),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, rows + 2 * payload);
            let mut collector =
                VerificationDiagnosticCollectorV1::materialize(2, &mut budget).unwrap();
            emit_text(&mut collector, loc, "ok", &mut budget).unwrap();
            let message = DivergentDisplay {
                calls: Cell::new(0),
                second,
                explicit_error,
                swallow_oversize,
            };
            assert_eq!(
                collector.emit(
                    loc,
                    DiagnosticCode::TypeMismatch,
                    2,
                    format_args!("{message}"),
                    &mut budget,
                ),
                Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting)
            );
            assert_eq!(message.calls.get(), 2);
            assert_eq!(retained_count(&collector), 1);
            assert_eq!(budget.storage(), rows + payload);
            assert_eq!(budget.peak_storage(), rows + 2 * payload);
            collector.abandon(&mut budget).unwrap();
            assert_eq!(budget.storage(), 0);
        }
    }

    #[test]
    fn sorted_errors_retain_caller_prefix_and_all_coexisting_buffers() {
        let module = ModuleId::new("module");
        let function = FunctionId::new("function");
        let kernel = KernelId::new("kernel");
        let loc = location(&module, Some(&function), Some(&kernel));
        let payload = 6 + 8 + 6 + 1;
        let copy_work = 5 + 20 + 2 * 3;
        let emit_work = 1 + 1 + copy_work + 1 + 1 + 1;
        let sort_work = 4 * 3 * 2 * (20 + 1 + 16);
        let exact_work = 7 + 3 * emit_work + 3 + sort_work;
        let exact_storage = 11 + 3 * DIAGNOSTIC_ROW_STORAGE_V1 + 3 * payload;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_work);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, exact_storage);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(11).unwrap();
        let mut collector = VerificationDiagnosticCollectorV1::materialize(3, &mut budget).unwrap();
        for (index, message) in ["c", "a", "b"].into_iter().enumerate() {
            emit_text(&mut collector, loc, message, &mut budget).unwrap();
            assert_eq!(
                budget.storage(),
                11 + 3 * DIAGNOSTIC_ROW_STORAGE_V1 + (index + 1) * payload
            );
        }
        let diagnostics = collector.finish_materialized(&mut budget).unwrap();
        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        assert_eq!(budget.work(), exact_work);
        assert_eq!(budget.storage(), 11);
        assert_eq!(budget.peak_storage(), exact_storage);
        budget.release_storage(11).unwrap();
    }

    #[test]
    fn sort_work_one_under_drops_all_roster_and_buffer_owners() {
        let module = ModuleId::new("m");
        let loc = location(&module, None, None);
        let emit_work = 1 + 1 + (5 + 1 + 2) + 1 + 1 + 1;
        let exact_work = 2 * emit_work + 2 + 4 * 2 * (1 + 1 + 16);
        let exact_storage = 3 + 2 * DIAGNOSTIC_ROW_STORAGE_V1 + 4;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_work - 1);
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, exact_storage);
        budget.reserve_storage(3).unwrap();
        let mut collector = VerificationDiagnosticCollectorV1::materialize(2, &mut budget).unwrap();
        emit_text(&mut collector, loc, "b", &mut budget).unwrap();
        emit_text(&mut collector, loc, "a", &mut budget).unwrap();
        assert!(matches!(
            collector.finish_materialized(&mut budget),
            Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                if error.actual() == exact_work && error.limit() == exact_work - 1
        ));
        assert_eq!(budget.storage(), 3);
        assert_eq!(budget.peak_storage(), exact_storage);
        budget.release_storage(3).unwrap();
    }

    #[test]
    fn incomplete_finish_and_abandon_drop_all_owned_payloads() {
        let module = ModuleId::new("module");
        let loc = location(&module, None, None);
        for finish in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
                &mut work,
                5 + 2 * DIAGNOSTIC_ROW_STORAGE_V1 + 7,
            );
            budget.reserve_storage(5).unwrap();
            let mut collector =
                VerificationDiagnosticCollectorV1::materialize(2, &mut budget).unwrap();
            emit_text(&mut collector, loc, "x", &mut budget).unwrap();
            if finish {
                assert!(matches!(
                    collector.finish_materialized(&mut budget),
                    Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting)
                ));
            } else {
                collector.abandon(&mut budget).unwrap();
            }
            assert_eq!(budget.storage(), 5);
            budget.release_storage(5).unwrap();
        }
    }

    #[test]
    fn excess_diagnostic_rejects_without_reserving_another_buffer() {
        let module = ModuleId::new("m");
        let loc = location(&module, None, None);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            DIAGNOSTIC_ROW_STORAGE_V1 + 2,
        );
        let mut collector = VerificationDiagnosticCollectorV1::materialize(1, &mut budget).unwrap();
        emit_text(&mut collector, loc, "x", &mut budget).unwrap();
        let accepted_work = budget.work();
        collector.failure = Some(DiagnosticFailurePointV1::ModuleReserve);
        assert_eq!(
            emit_text(&mut collector, loc, "y", &mut budget),
            Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting)
        );
        assert_eq!(budget.work(), accepted_work + 2);
        assert_eq!(budget.storage(), DIAGNOSTIC_ROW_STORAGE_V1 + 2);
        assert_eq!(retained_count(&collector), 1);
        collector.finish_materialized(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn invalid_message_upper_does_not_copy_ids_or_publish() {
        let module = ModuleId::new("module");
        let loc = location(&module, None, None);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            DIAGNOSTIC_ROW_STORAGE_V1,
        );
        let mut collector = VerificationDiagnosticCollectorV1::materialize(1, &mut budget).unwrap();
        collector.failure = Some(DiagnosticFailurePointV1::ModuleReserve);
        assert_eq!(
            collector.emit(
                loc,
                DiagnosticCode::InvalidIdentity,
                1,
                format_args!("long"),
                &mut budget,
            ),
            Err(CanonicalKernelIrVerificationResourceErrorV1::Accounting)
        );
        assert_eq!(retained_count(&collector), 0);
        assert_eq!(budget.work(), 2);
        assert_eq!(budget.storage(), DIAGNOSTIC_ROW_STORAGE_V1);
        collector.abandon(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn empty_message_still_requires_the_count_roster_action() {
        let module = ModuleId::new("");
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        let mut collector = VerificationDiagnosticCollectorV1::count();
        assert!(matches!(
            emit_text(&mut collector, location(&module, None, None), "", &mut budget),
            Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                if error.actual() == 1 && error.limit() == 0
        ));
        assert_eq!(collector.counted(), Some(0));
        assert_eq!(budget.storage(), 0);
    }
}
