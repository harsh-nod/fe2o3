//! Paid inert V3 embedding and independent complete stored-metadata replay.
use super::*;

#[cfg(test)]
impl InertCompilerModuleTextV1 {
    pub(crate) fn allocation_parts_for_test_v3(&self) -> (&String, [&Vec<String>; 5]) {
        (
            &self.llvm_ir,
            [
                &self.kernel_entries,
                &self.device_definitions,
                &self.internal_helpers,
                &self.device_ffi_exports,
                &self.external_declarations,
            ],
        )
    }
}
use fe2o3_compiler_ffi::{
    COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3, COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3,
    CompilerDescriptorSourceErrorV3, CompilerDescriptorSourceIdentityV3,
    CompilerDescriptorSourceV3 as Source,
};
use fe2o3_kernel_descriptor::{DESCRIPTOR_QUERY_STORAGE_V3, DescriptorWireErrorV3};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{
    any::Any,
    cmp::Ordering,
    error::Error,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

const PREFIX: &str =
    "\nmodule asm \".section .fe2o3.kd.v3,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n";
type Payload = Box<dyn Any + Send>;
pub(crate) const SCOPE_STORAGE: usize = 2 * size_of::<usize>()
    + size_of::<CanonicalKernelIrWorkLedgerIdentityV1>()
    + size_of::<[Option<Payload>; 2]>();

#[derive(Debug)]
pub(crate) enum NominalModuleErrorV3 {
    Resource(Resource),
    Construction(CompilerModuleConstructionError),
    Source(CompilerDescriptorSourceErrorV3<Resource>),
    Descriptor(DescriptorWireErrorV3<Resource>),
    Metadata(&'static str),
    Panicked,
}
pub(crate) type E = NominalModuleErrorV3;
pub(crate) type R<T> = Result<T, E>;
impl From<Resource> for E {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "nominal compiler module: {self:?}")
    }
}
impl Error for E {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Construction(e) => Some(e),
            Self::Source(e) => Some(e),
            Self::Descriptor(e) => Some(e),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NominalModuleStorageV3(usize);
impl NominalModuleStorageV3 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

pub(crate) fn scoped<'w, T>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>) -> R<T>,
) -> R<T> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'w> as usize;
    let paid = SCOPE_STORAGE
        .checked_add(size_of::<R<T>>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(paid)?;
    let mut payloads: [Option<Payload>; 2] = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(r) => r,
        Err(p) => {
            payloads[0] = Some(p);
            Err(E::Panicked)
        }
    };
    let same =
        ledger == budget.work_ledger_identity_v1() && slot == budget as *const Budget<'w> as usize;
    let valid = same
        && floor
            .checked_add(paid)
            .is_some_and(|minimum| budget.storage() >= minimum);
    if !valid {
        let rejected = std::mem::replace(&mut result, Err(Resource::Accounting.into()));
        payloads[1] = catch_unwind(AssertUnwindSafe(|| drop(rejected))).err();
    }
    if same && budget.storage() >= floor {
        if let Err(e) = budget.release_storage(budget.storage() - floor) {
            let rejected = std::mem::replace(&mut result, Err(e.into()));
            payloads[1] = catch_unwind(AssertUnwindSafe(|| drop(rejected))).err();
        }
    }
    drop(payloads);
    result
}

fn compare(a: &str, b: &str, budget: &mut Budget<'_>) -> R<Ordering> {
    budget.charge_work(
        a.len()
            .checked_add(b.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?,
    )?;
    Ok(a.cmp(b))
}
fn sort<T>(rows: &mut [T], name: impl Fn(&T) -> &str, budget: &mut Budget<'_>) -> R<()> {
    fn sift<T>(
        rows: &mut [T],
        mut root: usize,
        end: usize,
        name: &impl Fn(&T) -> &str,
        budget: &mut Budget<'_>,
    ) -> R<()> {
        loop {
            budget.charge_work(1)?;
            let left = root
                .checked_mul(2)
                .and_then(|n| n.checked_add(1))
                .ok_or(Resource::Arithmetic)?;
            if left >= end {
                return Ok(());
            }
            let right = left.checked_add(1).ok_or(Resource::Arithmetic)?;
            let child = if right < end
                && compare(name(&rows[left]), name(&rows[right]), budget)? == Ordering::Less
            {
                right
            } else {
                left
            };
            if compare(name(&rows[root]), name(&rows[child]), budget)? != Ordering::Less {
                return Ok(());
            }
            budget.charge_work(1)?;
            rows.swap(root, child);
            root = child;
        }
    }
    let count = rows.len();
    for root in (0..count / 2).rev() {
        sift(rows, root, count, &name, budget)?;
    }
    for end in (1..rows.len()).rev() {
        budget.charge_work(1)?;
        rows.swap(0, end);
        sift(rows, 0, end, &name, budget)?;
    }
    Ok(())
}

// The enclosing paid module/closure header owns the Vec headers. This helper
// pays actual backing only; each initialized String header lives in a Vec slot.
fn payload_vector<T>(count: usize, budget: &mut Budget<'_>) -> R<Vec<T>> {
    budget.charge_work(2)?;
    let requested = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let actual = rows
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
    Ok(rows)
}
fn push_name(rows: &mut Vec<String>, name: &str, budget: &mut Budget<'_>) -> R<()> {
    budget.charge_work(name.len().checked_add(2).ok_or(Resource::Arithmetic)?)?;
    if rows.len() == rows.capacity() {
        return Err(Resource::Accounting.into());
    }
    budget.reserve_storage(name.len())?;
    let mut value = String::new();
    value
        .try_reserve_exact(name.len())
        .map_err(|_| Resource::Allocation)?;
    budget.reserve_storage(
        value
            .capacity()
            .checked_sub(name.len())
            .ok_or(Resource::Accounting)?,
    )?;
    value.push_str(name);
    rows.push(value);
    Ok(())
}
fn preflight(owner: &Owner, budget: &mut Budget<'_>) -> R<()> {
    // Verified canonical bytes bound every visited record/type/name. The old
    // allocation-free bounds helper traverses only this actual verified graph.
    budget.charge_work(
        owner
            .canonical()
            .canonical_bytes()
            .len()
            .checked_mul(64)
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?,
    )?;
    enforce_compiler_module_bounds(owner.module()).map_err(E::Construction)
}
fn symbols(owner: &Owner, budget: &mut Budget<'_>) -> R<CompilerModuleSymbolClosureV1> {
    let module = owner.module();
    let count = module.functions.len();
    let mut value = CompilerModuleSymbolClosureV1 {
        kernel_entries: payload_vector(module.kernels.len(), budget)?,
        device_definitions: payload_vector(count, budget)?,
        internal_helpers: payload_vector(count, budget)?,
        device_ffi_exports: payload_vector(count, budget)?,
        external_declarations: payload_vector(
            count.checked_mul(2).ok_or(Resource::Arithmetic)?,
            budget,
        )?,
    };
    for kernel in &module.kernels {
        push_name(&mut value.kernel_entries, kernel.id.as_str(), budget)?;
    }
    for function in &module.functions {
        budget.charge_work(
            function
                .id
                .as_str()
                .len()
                .checked_mul(32)
                .and_then(|n| n.checked_add(8))
                .ok_or(Resource::Arithmetic)?,
        )?;
        match function.role {
            FunctionRole::InternalHelper => {
                push_name(&mut value.internal_helpers, function.id.as_str(), budget)?;
                push_name(&mut value.device_definitions, function.id.as_str(), budget)?;
            }
            FunctionRole::DeviceFfiExport => {
                push_name(&mut value.device_ffi_exports, function.id.as_str(), budget)?;
                push_name(&mut value.device_definitions, function.id.as_str(), budget)?;
            }
            FunctionRole::ExternalImport
                if FloatOperation::from_intrinsic_id(&function.id).is_none()
                    && AmdGpuDiagnosticOperation::from_intrinsic_id(&function.id).is_none() =>
            {
                push_name(
                    &mut value.external_declarations,
                    function.id.as_str(),
                    budget,
                )?;
            }
            _ => {}
        }
    }
    // Paid by preflight plus the per-ID fixed comparison allowance above.
    for name in ocml_link_imports(module) {
        push_name(&mut value.external_declarations, name, budget)?;
    }
    for rows in [
        &mut value.kernel_entries,
        &mut value.device_definitions,
        &mut value.internal_helpers,
        &mut value.device_ffi_exports,
        &mut value.external_declarations,
    ] {
        sort(rows, String::as_str, budget)?;
    }
    Ok(value)
}
pub(crate) fn module_storage(
    module: &InertCompilerModuleTextV1,
    budget: &mut Budget<'_>,
) -> R<usize> {
    let mut retained = size_of::<InertCompilerModuleTextV1>()
        .checked_add(module.llvm_ir.capacity())
        .ok_or(Resource::Arithmetic)?;
    for rows in [
        &module.kernel_entries,
        &module.device_definitions,
        &module.internal_helpers,
        &module.device_ffi_exports,
        &module.external_declarations,
    ] {
        budget.charge_work(rows.len().checked_add(2).ok_or(Resource::Arithmetic)?)?;
        retained = retained
            .checked_add(
                rows.capacity()
                    .checked_mul(size_of::<String>())
                    .ok_or(Resource::Arithmetic)?,
            )
            .ok_or(Resource::Arithmetic)?;
        for name in rows {
            retained = retained
                .checked_add(name.capacity())
                .ok_or(Resource::Arithmetic)?;
        }
    }
    Ok(retained)
}
fn suffix_length(bytes: usize) -> R<usize> {
    let chunks = bytes.checked_add(15).ok_or(Resource::Arithmetic)? / 16;
    bytes
        .checked_mul(6)
        .and_then(|n| chunks.checked_mul(18).and_then(|m| n.checked_add(m)))
        .and_then(|n| n.checked_add(PREFIX.len()))
        .ok_or(Resource::Arithmetic.into())
}
fn embedded_text(prefix: &str, wire: &[u8], budget: &mut Budget<'_>) -> R<String> {
    budget.charge_work(prefix.len().checked_add(1).ok_or(Resource::Arithmetic)?)?;
    if prefix.contains(".fe2o3.kd.") {
        return Err(E::Metadata("descriptor section already present"));
    }
    let length = prefix
        .len()
        .checked_add(suffix_length(wire.len())?)
        .ok_or(Resource::Arithmetic)?;
    if prefix.is_empty() || length > dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES {
        return Err(E::Metadata("complete native text bound"));
    }
    budget.charge_work(length)?;
    budget.reserve_storage(length)?;
    let mut text = String::new();
    text.try_reserve_exact(length)
        .map_err(|_| Resource::Allocation)?;
    budget.reserve_storage(
        text.capacity()
            .checked_sub(length)
            .ok_or(Resource::Accounting)?,
    )?;
    text.push_str(prefix);
    text.push_str(PREFIX);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for chunk in wire.chunks(16) {
        text.push_str("module asm \".byte ");
        for (index, byte) in chunk.iter().copied().enumerate() {
            if index != 0 {
                text.push_str(", ");
            }
            text.push_str("0x");
            text.push(HEX[usize::from(byte >> 4)] as char);
            text.push(HEX[usize::from(byte & 15)] as char);
        }
        text.push_str("\"\n");
    }
    if text.len() != length {
        return Err(Resource::Accounting.into());
    }
    Ok(text)
}

pub(crate) fn revalidate_source(source: &Source, budget: &mut Budget<'_>) -> R<()> {
    let scratch = COMPILER_DESCRIPTOR_SOURCE_VALIDATION_STORAGE_V3;
    let extent = source
        .storage()
        .retained_storage()
        .checked_add(scratch)
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(scratch)?;
    source
        .revalidate(extent, &mut |w| budget.charge_work(w))
        .map_err(E::Source)?;
    budget.release_storage(scratch)?;
    Ok(())
}
fn contains(rows: &[String], name: &str, budget: &mut Budget<'_>) -> R<()> {
    let mut start = 0;
    let mut end = rows.len();
    while start < end {
        budget.charge_work(1)?;
        let mid = start + (end - start) / 2;
        match compare(&rows[mid], name, budget)? {
            Ordering::Less => start = mid + 1,
            Ordering::Greater => end = mid,
            Ordering::Equal => return Ok(()),
        }
    }
    Err(E::Metadata("missing actual symbol"))
}
fn expect(rows: &[String], count: &mut usize, name: &str, budget: &mut Budget<'_>) -> R<()> {
    contains(rows, name, budget)?;
    *count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
    Ok(())
}
// This checker walks actual roles and tests membership/counts of the stored
// sorted lists. It neither calls the producer closure nor reconstructs a manifest
// from the same stored metadata. No borrowed/owned symbol list is allocated.
fn check_symbols(
    owner: &Owner,
    module: &InertCompilerModuleTextV1,
    budget: &mut Budget<'_>,
) -> R<()> {
    let lists = [
        &module.kernel_entries,
        &module.device_definitions,
        &module.internal_helpers,
        &module.device_ffi_exports,
        &module.external_declarations,
    ];
    budget.reserve_storage(size_of::<[usize; 5]>() + size_of::<[&Vec<String>; 5]>())?;
    for rows in lists {
        budget.charge_work(rows.len().checked_add(1).ok_or(Resource::Arithmetic)?)?;
        for pair in rows.windows(2) {
            if compare(&pair[0], &pair[1], budget)? != Ordering::Less {
                return Err(E::Metadata("unordered or duplicate symbol"));
            }
        }
    }
    let mut counts = [0usize; 5];
    for root in &owner.module().kernels {
        expect(lists[0], &mut counts[0], root.id.as_str(), budget)?;
    }
    for function in &owner.module().functions {
        budget.charge_work(
            function
                .id
                .as_str()
                .len()
                .checked_mul(32)
                .and_then(|n| n.checked_add(8))
                .ok_or(Resource::Arithmetic)?,
        )?;
        let name = function.id.as_str();
        if function.role == FunctionRole::InternalHelper {
            expect(lists[1], &mut counts[1], name, budget)?;
            expect(lists[2], &mut counts[2], name, budget)?;
        }
        if function.role == FunctionRole::DeviceFfiExport {
            expect(lists[1], &mut counts[1], name, budget)?;
            expect(lists[3], &mut counts[3], name, budget)?;
        }
        let float = FloatOperation::from_intrinsic_id(&function.id);
        let diagnostic = AmdGpuDiagnosticOperation::from_intrinsic_id(&function.id);
        if function.role == FunctionRole::ExternalImport && float.is_none() && diagnostic.is_none()
        {
            expect(lists[4], &mut counts[4], name, budget)?;
        }
        if let Some(FloatOperation::F32Math {
            function,
            implementation: F32MathImplementation::OcmlAbiV1,
            ..
        }) = float
        {
            let external = match function {
                F32MathFunction::Sin => "__ocml_sin_f32",
                F32MathFunction::Cos => "__ocml_cos_f32",
                F32MathFunction::Exp => "__ocml_exp_f32",
                F32MathFunction::Exp2 => "__ocml_exp2_f32",
                F32MathFunction::Ln => "__ocml_log_f32",
                F32MathFunction::Log2 => "__ocml_log2_f32",
                F32MathFunction::Log10 => "__ocml_log10_f32",
                _ => return Err(E::Metadata("invalid OCML intrinsic role")),
            };
            expect(lists[4], &mut counts[4], external, budget)?;
        }
    }
    budget.charge_work(5)?;
    if lists
        .iter()
        .zip(counts)
        .any(|(actual, expected)| actual.len() != expected)
    {
        return Err(E::Metadata("complete actual symbol closure"));
    }
    budget.release_storage(size_of::<[usize; 5]>() + size_of::<[&Vec<String>; 5]>())?;
    Ok(())
}
struct DescriptorName<'a> {
    entry: &'a str,
    symbol: &'a str,
}
fn check_descriptor_symbols(
    module: &InertCompilerModuleTextV1,
    source: &Source,
    budget: &mut Budget<'_>,
) -> R<()> {
    let scratch_floor = budget.storage();
    {
        budget.reserve_storage(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3)?;
        let extent = source
            .storage()
            .retained_storage()
            .checked_add(COMPILER_DESCRIPTOR_SOURCE_TABLE_STORAGE_V3)
            .ok_or(Resource::Arithmetic)?;
        let table = source
            .table(extent, &mut |w| budget.charge_work(w))
            .map_err(E::Source)?;
        budget
            .reserve_storage(DESCRIPTOR_QUERY_STORAGE_V3 + size_of::<Vec<DescriptorName<'_>>>())?;
        let mut names = payload_vector(table.kernel_count(), budget)?;
        for index in 0..table.kernel_count() {
            let row = table
                .kernel(index, &mut |w| budget.charge_work(w))
                .map_err(E::Descriptor)?;
            budget.charge_work(1)?;
            if names.len() == names.capacity() {
                return Err(Resource::Accounting.into());
            }
            names.push(DescriptorName {
                entry: row.entry_name(),
                symbol: row.descriptor_symbol(),
            });
        }
        sort(&mut names, |row| row.entry, budget)?;
        budget.charge_work(1)?;
        if names.is_empty() || names.len() != module.kernel_entries.len() {
            return Err(E::Metadata("descriptor/kernel closure"));
        }
        for (row, name) in names.iter().zip(&module.kernel_entries) {
            if compare(row.entry, name, budget)? != Ordering::Equal {
                return Err(E::Metadata("descriptor entry pair"));
            }
            budget.charge_work(
                row.symbol
                    .len()
                    .checked_add(1)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let symbol = row
                .symbol
                .strip_suffix(".kd")
                .ok_or(E::Metadata("descriptor symbol suffix"))?;
            if compare(symbol, name, budget)? != Ordering::Equal {
                return Err(E::Metadata("descriptor symbol pair"));
            }
        }
    }
    budget.release_storage(
        budget
            .storage()
            .checked_sub(scratch_floor)
            .ok_or(Resource::Accounting)?,
    )?;
    Ok(())
}

/// Independent stored-tag and all-five-vector replay. Correct LLVM alone cannot
/// authenticate this metadata. Actual native/table replay remains a separate join.
pub(crate) fn check_nominal_compiler_module_metadata_v3(
    owner: &Owner,
    module: &InertCompilerModuleTextV1,
    source: &Source,
    budget: &mut Budget<'_>,
) -> R<()> {
    scoped(budget, |budget| {
        preflight(owner, budget)?;
        revalidate_source(source, budget)?;
        budget.charge_work(size_of::<CompilerDescriptorSourceIdentityV3>() + 2)?;
        if module.descriptor_source_identity
            != Some(DescriptorSourceIdentity::V3(source.identity()))
        {
            return Err(E::Metadata("V3 binding tag/identity"));
        }
        check_symbols(owner, module, budget)?;
        check_descriptor_symbols(module, source, budget)
    })
}

/// No KIR lowering or source-custody constructor. The caller keeps the actual
/// final graph, original LLVM prefix and A.1 source backing prepaid throughout.
/// Returns one full unreserved module receipt, including its embedded headers.
pub(crate) fn retain_nominal_compiler_module_text_v3(
    owner: &Owner,
    llvm22_prefix: &str,
    source: &Source,
    budget: &mut Budget<'_>,
) -> R<(InertCompilerModuleTextV1, NominalModuleStorageV3)> {
    scoped(budget, |budget| {
        preflight(owner, budget)?;
        revalidate_source(source, budget)?;
        budget.reserve_storage(size_of::<InertCompilerModuleTextV1>())?;
        let closure = symbols(owner, budget)?;
        let llvm_ir = embedded_text(llvm22_prefix, source.canonical_bytes(), budget)?;
        let module = InertCompilerModuleTextV1 {
            llvm_ir,
            kernel_entries: closure.kernel_entries,
            device_definitions: closure.device_definitions,
            internal_helpers: closure.internal_helpers,
            device_ffi_exports: closure.device_ffi_exports,
            external_declarations: closure.external_declarations,
            descriptor_source_identity: Some(DescriptorSourceIdentity::V3(source.identity())),
        };
        check_nominal_compiler_module_metadata_v3(owner, &module, source, budget)?;
        let receipt = NominalModuleStorageV3(module_storage(&module, budget)?);
        Ok((module, receipt))
    })
}

#[cfg(test)]
impl InertCompilerModuleTextV1 {
    pub(crate) fn replace_first_ascii_byte_for_test_v3(&mut self, byte: u8) -> u8 {
        let old = self.llvm_ir.as_bytes()[0];
        assert!(old.is_ascii() && byte.is_ascii());
        self.llvm_ir
            .replace_range(..1, std::str::from_utf8(&[byte]).unwrap());
        old
    }
    pub(crate) const fn descriptor_binding_version_for_test_v3(&self) -> Option<u16> {
        match self.descriptor_source_identity {
            None => None,
            Some(DescriptorSourceIdentity::V1(_)) => Some(1),
            Some(DescriptorSourceIdentity::V3(_)) => Some(3),
        }
    }
}

#[cfg(test)]
#[path = "kernel_ir_codegen_nominal_descriptor_v3_tests.rs"]
mod tests;
