//! Exact native-V12 LLVM text and descriptor embedding, never final admission.

use std::{
    cmp::Ordering,
    error::Error,
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

use fe2o3_amd_target::{AmdTargetId, ProductionAmdTargetProfileV1};
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1, KernelIrContractCatalogBindingErrorV1,
    check_kernel_ir_contract_catalog_v1,
};
use fe2o3_kernel_descriptor::{
    CodeObjectVersion, DeviceDescriptorTableV1, MAX_DESCRIPTOR_TABLE_BYTES, ValidationError,
    encode_device_descriptor_table_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    InertCanonicalKernelIrContractCatalogV1 as Catalog,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};

use crate::{
    LoweringErrors, MAX_COMPILER_MODULE_TEXT_BYTES,
    MAX_PRODUCTION_SEMANTIC_ANCHOR_LLVM_TEXT_BYTES_V1, ProductionLlvmLayoutBindingErrorV1,
    bind_production_llvm22_worker_layout_v1,
    lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1 as lower_942,
    lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1 as lower_950,
};

const DESCRIPTOR_PREFIX: &[u8] =
    b"\nmodule asm \".section .fe2o3.kd.v1,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n";
const BYTE_PREFIX: &[u8] = b"module asm \".byte ";
const HEX: &[u8; 16] = b"0123456789abcdef";

#[derive(Debug)]
pub enum NativeV12TextDescriptorReplayErrorV1 {
    OutputBytes,
    Invalid(&'static str),
    Inventory(CanonicalKirInventoryErrorV1),
    Catalog(KernelIrContractCatalogBindingErrorV1),
    Descriptor(ValidationError),
    Lowering(LoweringErrors),
    Layout(ProductionLlvmLayoutBindingErrorV1),
    Resource(Resource),
    Panicked,
}
impl fmt::Display for NativeV12TextDescriptorReplayErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "native V12 text/descriptor replay: {self:?}")
    }
}
impl Error for NativeV12TextDescriptorReplayErrorV1 {}
impl From<Resource> for NativeV12TextDescriptorReplayErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
type E = NativeV12TextDescriptorReplayErrorV1;

/// Fixed-header transfer only. All four borrowed inputs remain caller-reserved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeV12TextDescriptorReplayStorageV1(usize);
impl NativeV12TextDescriptorReplayStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Move-only exact text/descriptor relation on these immutable borrowed inputs.
/// This does not prove source-derived ABI, functional/formal safety, execution
/// history, protected publication, LLVM-to-machine refinement or host authority.
///
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV1;
/// fn duplicate(value: ReplayedNativeV12TextDescriptorRelationV1<'_, '_, '_, '_>) {
///     let _ = value.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV1 as R;
/// use fe2o3_amd_target::ProductionAmdTargetProfileV1 as P;
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as O,
///     InertCanonicalKernelIrContractCatalogV1 as C};
/// use fe2o3_kernel_descriptor::DeviceDescriptorTableV1 as D;
/// fn forge<'a>(o: &'a O, c: &'a C, d: &'a D, l: &'a str) -> R<'a, 'a, 'a, 'a> {
///     R { output: o, catalog: c, descriptors: d, final_llvm: l,
///         profile: P::Gfx942, prefix_bytes: 0, storage: todo!() }
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_amdgcn_model::{ReplayedNativeV12TextDescriptorRelationV1 as R,
///     check_native_v12_text_descriptor_relation_v1 as check};
/// use fe2o3_amd_target::ProductionAmdTargetProfileV1 as P;
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as O,
///     InertCanonicalKernelIrContractCatalogV1 as C,
///     CanonicalKernelIrVerificationResourceBudgetV1 as B};
/// use fe2o3_kernel_descriptor::DeviceDescriptorTableV1 as D;
/// fn escape<'a>(o: O, c: &'a C, d: &'a D, l: &'a str, b: &mut B<'_>)
///     -> R<'static, 'a, 'a, 'a> {
///     check(&o, c, o.canonical().canonical_bytes(), P::Gfx942, d, l, b).unwrap()
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_amdgcn_model::{ReplayedNativeV12TextDescriptorRelationV1 as R,
///     check_native_v12_text_descriptor_relation_v1 as check};
/// use fe2o3_amd_target::ProductionAmdTargetProfileV1 as P;
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as O,
///     InertCanonicalKernelIrContractCatalogV1 as C,
///     CanonicalKernelIrVerificationResourceBudgetV1 as B};
/// use fe2o3_kernel_descriptor::DeviceDescriptorTableV1 as D;
/// fn escape<'a>(o: &'a O, c: C, d: &'a D, l: &'a str, b: &mut B<'_>)
///     -> R<'a, 'static, 'a, 'a> {
///     check(o, &c, o.canonical().canonical_bytes(), P::Gfx942, d, l, b).unwrap()
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_amdgcn_model::{ReplayedNativeV12TextDescriptorRelationV1 as R,
///     check_native_v12_text_descriptor_relation_v1 as check};
/// use fe2o3_amd_target::ProductionAmdTargetProfileV1 as P;
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as O,
///     InertCanonicalKernelIrContractCatalogV1 as C,
///     CanonicalKernelIrVerificationResourceBudgetV1 as B};
/// use fe2o3_kernel_descriptor::DeviceDescriptorTableV1 as D;
/// fn escape<'a>(o: &'a O, c: &'a C, d: D, l: &'a str, b: &mut B<'_>)
///     -> R<'a, 'a, 'static, 'a> {
///     check(o, c, o.canonical().canonical_bytes(), P::Gfx942, &d, l, b).unwrap()
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_amdgcn_model::{ReplayedNativeV12TextDescriptorRelationV1 as R,
///     check_native_v12_text_descriptor_relation_v1 as check};
/// use fe2o3_amd_target::ProductionAmdTargetProfileV1 as P;
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12 as O,
///     InertCanonicalKernelIrContractCatalogV1 as C,
///     CanonicalKernelIrVerificationResourceBudgetV1 as B};
/// use fe2o3_kernel_descriptor::DeviceDescriptorTableV1 as D;
/// fn escape<'a>(o: &'a O, c: &'a C, d: &'a D, l: String, b: &mut B<'_>)
///     -> R<'a, 'a, 'a, 'static> {
///     check(o, c, o.canonical().canonical_bytes(), P::Gfx942, d, &l, b).unwrap()
/// }
/// ```
pub struct ReplayedNativeV12TextDescriptorRelationV1<'o, 'c, 'd, 'l> {
    output: &'o Owner,
    catalog: &'c Catalog,
    descriptors: &'d DeviceDescriptorTableV1,
    final_llvm: &'l str,
    profile: ProductionAmdTargetProfileV1,
    prefix_bytes: usize,
    storage: NativeV12TextDescriptorReplayStorageV1,
}
impl<'o, 'c, 'd, 'l> ReplayedNativeV12TextDescriptorRelationV1<'o, 'c, 'd, 'l> {
    pub const fn output(&self) -> &'o Owner {
        self.output
    }
    pub const fn catalog(&self) -> &'c Catalog {
        self.catalog
    }
    pub const fn descriptors(&self) -> &'d DeviceDescriptorTableV1 {
        self.descriptors
    }
    pub const fn final_llvm(&self) -> &'l str {
        self.final_llvm
    }
    pub fn pre_descriptor_llvm(&self) -> &'l str {
        &self.final_llvm[..self.prefix_bytes]
    }
    pub const fn profile(&self) -> ProductionAmdTargetProfileV1 {
        self.profile
    }
    pub const fn storage(&self) -> NativeV12TextDescriptorReplayStorageV1 {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn scoped<T>(
    budget: &mut Budget<'_>,
    run: impl FnOnce(&mut Budget<'_>) -> Result<T, E>,
) -> Result<T, E> {
    let floor = budget.storage();
    let result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(E::Panicked)
        }
    };
    let restored = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)
        .and_then(|bytes| budget.release_storage(bytes));
    if let Err(error) = restored {
        drop(result);
        return Err(error.into());
    }
    result
}

fn exact_output_bytes(owner: &[u8], published: &[u8], budget: &mut Budget<'_>) -> Result<(), E> {
    budget.charge_work(2)?;
    budget.charge_work(
        owner
            .len()
            .checked_add(published.len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    if owner != published {
        return Err(E::OutputBytes);
    }
    Ok(())
}

fn indices(count: usize, budget: &mut Budget<'_>) -> Result<Vec<usize>, E> {
    budget.charge_work(count.checked_add(2).ok_or(Resource::Arithmetic)?)?;
    let requested = count
        .checked_mul(size_of::<usize>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(
        size_of::<Vec<usize>>()
            .checked_add(requested)
            .ok_or(Resource::Arithmetic)?,
    )?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let actual = rows
        .capacity()
        .checked_mul(size_of::<usize>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
    rows.extend(0..count);
    Ok(rows)
}

fn compare_names(a: &str, b: &str, budget: &mut Budget<'_>) -> Result<Ordering, E> {
    let work = a
        .len()
        .checked_add(b.len())
        .and_then(|n| n.checked_add(1))
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(work)?;
    Ok(a.cmp(b))
}

fn sort_indices<'a>(
    rows: &mut [usize],
    name: impl Fn(usize) -> &'a str,
    budget: &mut Budget<'_>,
) -> Result<(), E> {
    fn sift<'a>(
        rows: &mut [usize],
        mut root: usize,
        end: usize,
        name: &impl Fn(usize) -> &'a str,
        budget: &mut Budget<'_>,
    ) -> Result<(), E> {
        loop {
            budget.charge_work(1)?;
            let left = root
                .checked_mul(2)
                .and_then(|n| n.checked_add(1))
                .ok_or(Resource::Arithmetic)?;
            if left >= end {
                return Ok(());
            }
            let right = left + 1;
            let child = if right < end
                && compare_names(name(rows[left]), name(rows[right]), budget)? == Ordering::Less
            {
                right
            } else {
                left
            };
            if compare_names(name(rows[root]), name(rows[child]), budget)? != Ordering::Less {
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

fn check_roster(
    output: &Owner,
    table: &DeviceDescriptorTableV1,
    budget: &mut Budget<'_>,
) -> Result<(), E> {
    budget.charge_work(1)?;
    let kernels = &output.module().kernels;
    let descriptors = table.kernels();
    if kernels.is_empty() || kernels.len() != descriptors.len() {
        return Err(E::Invalid("complete descriptor/kernel roster"));
    }
    let floor = budget.storage();
    {
        let mut kernel_rows = indices(kernels.len(), budget)?;
        let mut descriptor_rows = indices(descriptors.len(), budget)?;
        sort_indices(&mut kernel_rows, |i| kernels[i].id.as_str(), budget)?;
        sort_indices(
            &mut descriptor_rows,
            |i| descriptors[i].entry_name().as_str(),
            budget,
        )?;
        let mut previous = None;
        for (&kernel, &descriptor) in kernel_rows.iter().zip(&descriptor_rows) {
            let export = kernels[kernel].id.as_str();
            let row = &descriptors[descriptor];
            if compare_names(export, row.entry_name().as_str(), budget)? != Ordering::Equal {
                return Err(E::Invalid("descriptor entry bijection"));
            }
            if let Some(old) = previous {
                if compare_names(old, export, budget)? != Ordering::Less {
                    return Err(E::Invalid("duplicate descriptor entry"));
                }
            }
            // Pairwise binding: swapping two otherwise valid .kd names is not a bijection.
            budget.charge_work(
                row.descriptor_symbol()
                    .as_str()
                    .len()
                    .checked_add(1)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let symbol = row.descriptor_symbol().as_str().strip_suffix(".kd");
            let Some(symbol) = symbol else {
                return Err(E::Invalid("descriptor symbol suffix"));
            };
            if compare_names(export, symbol, budget)? != Ordering::Equal {
                return Err(E::Invalid("descriptor entry/symbol pair"));
            }
            previous = Some(export);
        }
    }
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    )?;
    Ok(())
}

fn check_catalog(output: &Owner, catalog: &Catalog, budget: &mut Budget<'_>) -> Result<(), E> {
    let floor = budget.storage();
    {
        let (inventory, storage) =
            CanonicalKirInventoryV1::derive(output, budget).map_err(E::Inventory)?;
        budget.reserve_storage(storage.retained_storage())?;
        let (_checked, storage) =
            check_kernel_ir_contract_catalog_v1(&inventory, catalog, budget).map_err(E::Catalog)?;
        budget.reserve_storage(storage.retained_storage())?;
    }
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    )?;
    Ok(())
}

fn reconcile_output(requested: usize, actual: usize, budget: &mut Budget<'_>) -> Result<(), E> {
    if actual >= requested {
        budget.reserve_storage(actual - requested)?;
    } else {
        budget.release_storage(requested - actual)?;
    }
    Ok(())
}

// Existing engines own their internal allocation/work policy. This envelope
// accounts their visible returned buffer, not realloc transients or engine cost.
fn engine_text(
    maximum: usize,
    budget: &mut Budget<'_>,
    run: impl FnOnce() -> Result<String, E>,
) -> Result<(String, usize), E> {
    budget.charge_work(1)?;
    let requested = maximum
        .checked_add(size_of::<String>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let text = run()?;
    let actual = text
        .capacity()
        .checked_add(size_of::<String>())
        .ok_or(Resource::Arithmetic)?;
    reconcile_output(requested, actual, budget)?;
    if text.len() > maximum {
        return Err(E::Invalid("native text bound"));
    }
    Ok((text, actual))
}

fn descriptor_bytes(
    table: &DeviceDescriptorTableV1,
    budget: &mut Budget<'_>,
) -> Result<(Vec<u8>, usize), E> {
    budget.charge_work(1)?;
    let requested = MAX_DESCRIPTOR_TABLE_BYTES
        .checked_add(size_of::<Vec<u8>>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let bytes = encode_device_descriptor_table_v1(table).map_err(E::Descriptor)?;
    let actual = bytes
        .capacity()
        .checked_add(size_of::<Vec<u8>>())
        .ok_or(Resource::Arithmetic)?;
    reconcile_output(requested, actual, budget)?;
    Ok((bytes, actual))
}

fn suffix_length(bytes: usize) -> Result<usize, Resource> {
    let chunks = bytes.checked_add(15).ok_or(Resource::Arithmetic)? / 16;
    bytes
        .checked_mul(6)
        .and_then(|n| chunks.checked_mul(18).and_then(|c| n.checked_add(c)))
        .and_then(|n| n.checked_add(DESCRIPTOR_PREFIX.len()))
        .ok_or(Resource::Arithmetic)
}

struct ExactText<'a> {
    bytes: &'a [u8],
    cursor: usize,
}
impl ExactText<'_> {
    fn take(&mut self, expected: &[u8]) -> Result<(), E> {
        let end = self
            .cursor
            .checked_add(expected.len())
            .ok_or(Resource::Arithmetic)?;
        if self.bytes.get(self.cursor..end) != Some(expected) {
            return Err(E::Invalid("exact native LLVM/descriptor text"));
        }
        self.cursor = end;
        Ok(())
    }
}

fn compare_text(
    prefix: &[u8],
    descriptor: &[u8],
    final_text: &[u8],
    budget: &mut Budget<'_>,
) -> Result<(), E> {
    budget.charge_work(2)?;
    let expected = prefix
        .len()
        .checked_add(suffix_length(descriptor.len())?)
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(
        expected
            .checked_add(final_text.len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    if expected != final_text.len() {
        return Err(E::Invalid("complete native LLVM length"));
    }
    let mut exact = ExactText {
        bytes: final_text,
        cursor: 0,
    };
    exact.take(prefix)?;
    exact.take(DESCRIPTOR_PREFIX)?;
    for chunk in descriptor.chunks(16) {
        exact.take(BYTE_PREFIX)?;
        for (ordinal, byte) in chunk.iter().copied().enumerate() {
            if ordinal != 0 {
                exact.take(b", ")?;
            }
            exact.take(&[
                b'0',
                b'x',
                HEX[usize::from(byte >> 4)],
                HEX[usize::from(byte & 15)],
            ])?;
        }
        exact.take(b"\"\n")?;
    }
    if exact.cursor != final_text.len() {
        return Err(E::Invalid("trailing native LLVM"));
    }
    Ok(())
}

fn reserve_wrapper(budget: &mut Budget<'_>) -> Result<NativeV12TextDescriptorReplayStorageV1, E> {
    budget.charge_work(1)?;
    let bytes = size_of::<ReplayedNativeV12TextDescriptorRelationV1<'_, '_, '_, '_>>();
    budget.reserve_storage(bytes)?;
    Ok(NativeV12TextDescriptorReplayStorageV1(bytes))
}

/// Checks complete published O bytes, the existing actual-O catalog binding,
/// native L plus LLVM22 text layout, and exact descriptor bytes/entry pairs.
/// No target rebinding, optimizer invocation, graph clone or legacy replay runs.
/// Descriptor bytes use the existing canonical zero-digest table encoding.
/// Source-derived ABI/ownership, functional/formal proof and protected execution
/// provenance remain separate obligations, never inferred from this result.
///
/// All borrowed inputs remain caller-reserved. New comparison/index work and
/// scratch use this ledger; existing lowering/layout/descriptor-codec internal
/// work and allocation remain their separate bounded domains. Visible engine
/// outputs are reserved before calls and reconciled before subsequent work.
/// On success reserve the returned fixed-header receipt before another allocation;
/// drop the result before releasing it. Every exit restores the entry floor,
/// preserving prior work, peak and first denial; cleanup failure takes precedence.
pub fn check_native_v12_text_descriptor_relation_v1<'o, 'c, 'd, 'l>(
    output: &'o Owner,
    catalog: &'c Catalog,
    published_output_bytes: &[u8],
    profile: ProductionAmdTargetProfileV1,
    descriptors: &'d DeviceDescriptorTableV1,
    final_llvm: &'l str,
    budget: &mut Budget<'_>,
) -> Result<ReplayedNativeV12TextDescriptorRelationV1<'o, 'c, 'd, 'l>, E> {
    scoped(budget, |budget| {
        exact_output_bytes(
            output.canonical().canonical_bytes(),
            published_output_bytes,
            budget,
        )?;
        budget.charge_work(5 + 32 + profile.device_target().len())?;
        if final_llvm.is_empty() || final_llvm.len() > MAX_COMPILER_MODULE_TEXT_BYTES {
            return Err(E::Invalid("final LLVM input bound"));
        }
        let target = AmdTargetId::parse(profile.device_target())
            .map_err(|_| E::Invalid("closed profile"))?;
        if descriptors.device_target().as_amd_target_id() != target
            || descriptors.code_object_version() != CodeObjectVersion::V6
            || descriptors.canonical_code_object_digest().as_bytes() != &[0; 32]
        {
            return Err(E::Invalid("descriptor profile/COV6/zero digest"));
        }
        check_roster(output, descriptors, budget)?;
        check_catalog(output, catalog, budget)?;
        let (dialect, dialect_storage) =
            engine_text(MAX_COMPILER_MODULE_TEXT_BYTES, budget, || {
                match profile {
                    ProductionAmdTargetProfileV1::Gfx942 => lower_942(output),
                    ProductionAmdTargetProfileV1::Gfx950 => lower_950(output),
                }
                .map_err(E::Lowering)
            })?;
        let (prefix, prefix_storage) = engine_text(
            MAX_PRODUCTION_SEMANTIC_ANCHOR_LLVM_TEXT_BYTES_V1,
            budget,
            || bind_production_llvm22_worker_layout_v1(&dialect).map_err(E::Layout),
        )?;
        drop(dialect);
        budget.release_storage(dialect_storage)?;
        let (descriptor, descriptor_storage) = descriptor_bytes(descriptors, budget)?;
        compare_text(
            prefix.as_bytes(),
            &descriptor,
            final_llvm.as_bytes(),
            budget,
        )?;
        let prefix_bytes = prefix.len();
        drop(descriptor);
        budget.release_storage(descriptor_storage)?;
        drop(prefix);
        budget.release_storage(prefix_storage)?;
        let storage = reserve_wrapper(budget)?;
        Ok(ReplayedNativeV12TextDescriptorRelationV1 {
            output,
            catalog,
            descriptors,
            final_llvm,
            profile,
            prefix_bytes,
            storage,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use std::{cell::Cell, fmt::Write as _};

    const PRIOR: usize = 7;
    const FLOOR: usize = 29;

    fn expected_suffix(bytes: &[u8]) -> String {
        let mut text = String::from(
            "\nmodule asm \".section .fe2o3.kd.v1,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n",
        );
        for chunk in bytes.chunks(16) {
            text.push_str("module asm \".byte ");
            for (i, byte) in chunk.iter().enumerate() {
                if i != 0 {
                    text.push_str(", ");
                }
                write!(text, "0x{byte:02x}").unwrap();
            }
            text.push_str("\"\n");
        }
        text
    }

    #[test]
    fn full_output_comparison_has_exact_prepaid_prefix() {
        for short in [false, true] {
            let mut work = Work::new(PRIOR + 8 - usize::from(short));
            let mut budget = Budget::new(&mut work, FLOOR);
            budget.charge_work(PRIOR).unwrap();
            budget.reserve_storage(FLOOR).unwrap();
            let result = exact_output_bytes(b"abc", b"abc", &mut budget);
            assert_eq!(result.is_ok(), !short);
            assert_eq!(budget.work(), PRIOR + if short { 2 } else { 8 });
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(work.failed_work(), short.then_some(PRIOR + 8));
        }
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, FLOOR);
        assert!(matches!(
            exact_output_bytes(b"abc", b"abd", &mut budget),
            Err(E::OutputBytes)
        ));
        assert!(matches!(
            exact_output_bytes(b"abc", b"ab", &mut budget),
            Err(E::OutputBytes)
        ));
    }

    #[test]
    fn suffix_boundaries_compare_every_byte_with_independent_work_limits() {
        for count in [1usize, 15, 16, 17] {
            let bytes = (0..count).map(|i| (i as u8) ^ 0xab).collect::<Vec<_>>();
            let prefix = b"native prefix\n";
            let suffix = expected_suffix(&bytes);
            assert_eq!(
                suffix.len(),
                DESCRIPTOR_PREFIX.len() + 6 * count + 18 * count.div_ceil(16)
            );
            assert_eq!(suffix_length(count).unwrap(), suffix.len());
            let text = [prefix.as_slice(), suffix.as_bytes()].concat();
            let expected_work = 2 + 2 * text.len();
            for short in [false, true] {
                let mut work = Work::new(PRIOR + expected_work - usize::from(short));
                let mut budget = Budget::new(&mut work, FLOOR);
                budget.charge_work(PRIOR).unwrap();
                budget.reserve_storage(FLOOR).unwrap();
                let result = compare_text(prefix, &bytes, &text, &mut budget);
                assert_eq!(result.is_ok(), !short);
                assert_eq!(budget.work(), PRIOR + if short { 2 } else { expected_work });
                assert_eq!(budget.storage(), FLOOR);
                assert_eq!(work.failed_work(), short.then_some(PRIOR + expected_work));
            }
            for offset in 0..text.len() {
                let mut bad = text.clone();
                bad[offset] ^= 1;
                let mut work = Work::new(100_000);
                let mut budget = Budget::new(&mut work, FLOOR);
                assert!(
                    compare_text(prefix, &bytes, &bad, &mut budget).is_err(),
                    "byte {offset}"
                );
            }
            let mut work = Work::new(100_000);
            let mut budget = Budget::new(&mut work, FLOOR);
            let mut extra = text.clone();
            extra.push(b'\n');
            assert!(compare_text(prefix, &bytes, &extra, &mut budget).is_err());
            assert!(compare_text(prefix, &bytes, &text[..text.len() - 1], &mut budget).is_err());
        }
        assert!(suffix_length(usize::MAX).is_err());
    }

    #[test]
    fn byte_paid_heapsort_handles_nonlexical_and_equal_names() {
        let names = ["zeta", "beta", "alpha", "beta"];
        let mut rows = [0, 1, 2, 3];
        let mut work = Work::new(100_000);
        let mut budget = Budget::new(&mut work, FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        sort_indices(&mut rows, |i| names[i], &mut budget).unwrap();
        assert_eq!(rows.map(|i| names[i]), ["alpha", "beta", "beta", "zeta"]);
        assert_eq!(budget.storage(), FLOOR);
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, FLOOR);
        assert!(matches!(
            sort_indices(&mut [0, 1], |i| names[i], &mut budget),
            Err(E::Resource(Resource::Work(_)))
        ));
    }

    #[test]
    fn current_wrapper_size_is_charged_once_with_exact_failure_prefix() {
        let header = size_of::<ReplayedNativeV12TextDescriptorRelationV1<'_, '_, '_, '_>>();
        for short_work in [false, true] {
            for short_storage in [false, true] {
                let mut work = Work::new(PRIOR + 1 - usize::from(short_work));
                let mut budget =
                    Budget::new(&mut work, FLOOR + header - usize::from(short_storage));
                budget.charge_work(PRIOR).unwrap();
                budget.reserve_storage(FLOOR).unwrap();
                let result = scoped(&mut budget, reserve_wrapper);
                assert_eq!(result.is_ok(), !short_work && !short_storage);
                if let Ok(receipt) = result {
                    assert_eq!(receipt.retained_storage(), header);
                }
                assert_eq!(budget.storage(), FLOOR);
                assert_eq!(budget.work(), PRIOR + usize::from(!short_work));
                assert_eq!(
                    budget.failed_storage(),
                    (!short_work && short_storage).then_some(FLOOR + header)
                );
                assert_eq!(work.failed_work(), short_work.then_some(PRIOR + 1));
            }
        }
    }

    #[test]
    fn opaque_output_denial_precedes_call_and_actual_excess_is_not_hidden() {
        let maximum = 8;
        let requested = maximum + size_of::<String>();
        let called = Cell::new(false);
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, FLOOR + requested - 1);
        budget.reserve_storage(FLOOR).unwrap();
        let result = scoped(&mut budget, |budget| {
            engine_text(maximum, budget, || {
                called.set(true);
                Ok(String::new())
            })
        });
        assert!(result.is_err());
        assert!(!called.get());
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_storage(), Some(FLOOR + requested));

        let capacity = Cell::new(0);
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, FLOOR + requested);
        budget.reserve_storage(FLOOR).unwrap();
        let result = scoped(&mut budget, |budget| {
            engine_text(maximum, budget, || {
                let text = String::with_capacity(maximum + 1);
                capacity.set(text.capacity());
                Ok(text)
            })
        });
        assert!(result.is_err());
        assert!(capacity.get() > maximum);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(
            budget.failed_storage(),
            Some(FLOOR + size_of::<String>() + capacity.get())
        );
    }

    #[test]
    fn scope_preserves_inner_errors_and_panic_unless_cleanup_is_corrupt() {
        for corrupt in [false, true] {
            for panic in [false, true] {
                let mut work = Work::new(100);
                let mut budget = Budget::new(&mut work, 100);
                budget.charge_work(PRIOR).unwrap();
                budget.reserve_storage(FLOOR).unwrap();
                let result: Result<(), E> = scoped(&mut budget, |budget| {
                    budget.charge_work(1)?;
                    if corrupt {
                        budget.release_storage(1)?;
                    }
                    if panic {
                        panic!("native text replay scope");
                    }
                    Err(E::Invalid("original error"))
                });
                if corrupt {
                    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
                } else if panic {
                    assert!(matches!(result, Err(E::Panicked)));
                } else {
                    assert!(matches!(result, Err(E::Invalid("original error"))));
                }
                assert_eq!(budget.storage(), FLOOR - usize::from(corrupt));
                assert_eq!(budget.work(), PRIOR + 1);
            }
        }
    }
}
