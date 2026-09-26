//! Independent actual-final-subject V3 descriptor/native relation, never authority.
use crate::{
    LoweringErrors, MAX_COMPILER_MODULE_TEXT_BYTES,
    MAX_PRODUCTION_SEMANTIC_ANCHOR_LLVM_TEXT_BYTES_V1, ProductionLlvmLayoutBindingErrorV1,
    bind_production_llvm22_worker_layout_v1,
    lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1 as lower_942,
    lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1 as lower_950,
};
use fe2o3_amd_target::{AmdTargetId, ProductionAmdTargetProfileV1 as Profile};
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1 as Inventory,
    KernelIrContractCatalogBindingErrorV1, check_kernel_ir_contract_catalog_v1,
};
use fe2o3_kernel_descriptor::{
    CodeObjectVersion, DescriptorWireErrorV3, DeviceDescriptorTableV3 as Table,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1, InertCanonicalKernelIrContractCatalogV1 as Catalog,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{
    any::Any,
    cmp::Ordering,
    error::Error,
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

const PREFIX: &[u8] =
    b"\nmodule asm \".section .fe2o3.kd.v3,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n";
const HEX: &[u8; 16] = b"0123456789abcdef";
#[path = "native_v12_text_descriptor_queries.rs"]
pub(super) mod queries;
use queries::{KernelQuery, TableQuery};
type Payload = Box<dyn Any + Send>;
pub(super) const SCOPE_STORAGE: usize = 2 * size_of::<usize>()
    + size_of::<CanonicalKernelIrWorkLedgerIdentityV1>()
    + size_of::<[Option<Payload>; 2]>();

/// Typed refusal from independent actual-owner V3 physical/native replay.
#[derive(Debug)]
pub enum NativeV12TextDescriptorReplayErrorV3 {
    /// Published bytes are not the supplied actual final graph.
    OutputBytes,
    /// A complete structural, target, or native-text invariant failed.
    Invalid(&'static str),
    /// A descriptor argument or explicit ABI extent does not match KIR.
    Physical {
        root: usize,
        argument: Option<usize>,
        field: &'static str,
    },
    /// A declared or operation-derived requirement disagrees with the profile.
    Requirement {
        function: Option<usize>,
        field: &'static str,
    },
    /// Actual connected-owner inventory reconstruction refused.
    Inventory(CanonicalKirInventoryErrorV1),
    /// The supplied catalog does not bind to the actual final inventory.
    Catalog(KernelIrContractCatalogBindingErrorV1),
    /// A paid structured V3 query refused.
    Descriptor(DescriptorWireErrorV3<Resource>),
    /// Actual final-owner native emission refused.
    Lowering(LoweringErrors),
    /// The LLVM22 worker text-layout binding refused.
    Layout(ProductionLlvmLayoutBindingErrorV1),
    /// The original cumulative resource ledger refused.
    Resource(Resource),
    /// A private replay scope caught an unwind before returning authority.
    Panicked,
}
pub(super) type E = NativeV12TextDescriptorReplayErrorV3;
pub(super) type R<T> = Result<T, E>;
impl From<Resource> for E {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native V12/V3 descriptor replay: {self:?}")
    }
}
impl Error for E {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Inventory(e) => Some(e),
            Self::Catalog(e) => Some(e),
            Self::Descriptor(e) => Some(e),
            Self::Lowering(e) => Some(e),
            Self::Layout(e) => Some(e),
            Self::Resource(e) => Some(e),
            _ => None,
        }
    }
}

/// Full fixed-header transfer; all borrowed backing remains separately prepaid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeV12TextDescriptorReplayStorageV3(usize);
impl NativeV12TextDescriptorReplayStorageV3 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Exact physical/requirements/catalog/native agreement, not Rust nominal kind,
/// ownership, source history, race/formal proof, LLVM refinement or launch authority.
/// The five input lifetimes cannot escape their owning objects.
///
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV3;
/// fn clone(v: ReplayedNativeV12TextDescriptorRelationV3<'_, '_, '_, '_, '_>) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV3 as R;
/// fn forge<'a>() -> R<'a, 'a, 'a, 'a, 'a> { R::default() }
/// ```
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV3 as R;
/// fn escape_output<'a>(v: R<'a, 'static, 'static, 'static, 'static>) -> R<'static, 'static, 'static, 'static, 'static> { v }
/// ```
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV3 as R;
/// fn escape_catalog<'a>(v: R<'static, 'a, 'static, 'static, 'static>) -> R<'static, 'static, 'static, 'static, 'static> { v }
/// ```
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV3 as R;
/// fn escape_table<'a>(v: R<'static, 'static, 'a, 'static, 'static>) -> R<'static, 'static, 'static, 'static, 'static> { v }
/// ```
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV3 as R;
/// fn escape_wire<'a>(v: R<'static, 'static, 'a, 'a, 'static>) -> R<'static, 'static, 'a, 'static, 'static> { v }
/// ```
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV3 as R;
/// fn escape_native<'a>(v: R<'static, 'static, 'static, 'static, 'a>) -> R<'static, 'static, 'static, 'static, 'static> { v }
/// ```
pub struct ReplayedNativeV12TextDescriptorRelationV3<'o, 'c, 'd, 'w, 'l> {
    output: &'o Owner,
    catalog: &'c Catalog,
    descriptors: &'d Table<'w>,
    final_llvm: &'l str,
    profile: Profile,
    prefix_bytes: usize,
    storage: NativeV12TextDescriptorReplayStorageV3,
}
impl<'o, 'c, 'd, 'w, 'l> ReplayedNativeV12TextDescriptorRelationV3<'o, 'c, 'd, 'w, 'l> {
    pub const fn output(&self) -> &'o Owner {
        self.output
    }
    pub const fn catalog(&self) -> &'c Catalog {
        self.catalog
    }
    pub const fn descriptors(&self) -> &'d Table<'w> {
        self.descriptors
    }
    pub const fn final_llvm(&self) -> &'l str {
        self.final_llvm
    }
    pub fn pre_descriptor_llvm(&self) -> &'l str {
        &self.final_llvm[..self.prefix_bytes]
    }
    pub const fn profile(&self) -> Profile {
        self.profile
    }
    pub const fn storage(&self) -> NativeV12TextDescriptorReplayStorageV3 {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

// Independent of V1 cleanup. Panic payload destructors run only after accepted
// original-ledger cleanup; a replacement ledger is never credited or repaired.
pub(super) fn scoped<'w, T>(
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

pub(super) fn vector<T>(count: usize, budget: &mut Budget<'_>) -> R<Vec<T>> {
    budget.charge_work(2)?;
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(
        bytes
            .checked_add(size_of::<Vec<T>>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let actual = rows
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(actual.checked_sub(bytes).ok_or(Resource::Accounting)?)?;
    Ok(rows)
}
pub(super) fn push<T>(rows: &mut Vec<T>, row: T, budget: &mut Budget<'_>) -> R<()> {
    budget.charge_work(1)?;
    if rows.len() == rows.capacity() {
        return Err(Resource::Accounting.into());
    }
    rows.push(row);
    Ok(())
}
pub(super) fn compare(a: &str, b: &str, budget: &mut Budget<'_>) -> R<Ordering> {
    budget.charge_work(
        a.len()
            .checked_add(b.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?,
    )?;
    Ok(a.cmp(b))
}
pub(super) fn sort<T>(
    rows: &mut [T],
    compare: &mut impl FnMut(&T, &T, &mut Budget<'_>) -> R<Ordering>,
    budget: &mut Budget<'_>,
) -> R<()> {
    fn sift<T>(
        rows: &mut [T],
        mut root: usize,
        end: usize,
        compare: &mut impl FnMut(&T, &T, &mut Budget<'_>) -> R<Ordering>,
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
            let child =
                if right < end && compare(&rows[left], &rows[right], budget)? == Ordering::Less {
                    right
                } else {
                    left
                };
            if compare(&rows[root], &rows[child], budget)? != Ordering::Less {
                return Ok(());
            }
            budget.charge_work(1)?;
            rows.swap(root, child);
            root = child;
        }
    }
    let count = rows.len();
    for root in (0..count / 2).rev() {
        sift(rows, root, count, compare, budget)?;
    }
    for end in (1..rows.len()).rev() {
        budget.charge_work(1)?;
        rows.swap(0, end);
        sift(rows, 0, end, compare, budget)?;
    }
    Ok(())
}

pub(super) struct Root {
    pub kernel: usize,
    pub descriptor: usize,
}
struct DescriptorName<'a> {
    ordinal: usize,
    entry: &'a str,
    symbol: &'a str,
}
pub(super) fn roots<'wire, D: TableQuery<'wire>>(
    inventory: &Inventory<'_>,
    table: &D,
    budget: &mut Budget<'_>,
) -> R<Vec<Root>> {
    budget.charge_work(2)?;
    let kernels = inventory.kernels();
    if kernels.is_empty() || kernels.len() != table.kernel_count() {
        return Err(E::Invalid("complete kernel/descriptor roster"));
    }
    let mut rows = vector::<Root>(kernels.len(), budget)?;
    let scratch_floor = budget.storage();
    {
        budget.reserve_storage(D::QUERY_STORAGE)?;
        let mut names = vector::<DescriptorName<'_>>(kernels.len(), budget)?;
        let mut indices = vector::<usize>(kernels.len(), budget)?;
        for index in 0..kernels.len() {
            let row = table.kernel(index, budget)?;
            push(
                &mut names,
                DescriptorName {
                    ordinal: index,
                    entry: row.entry_name(),
                    symbol: row.descriptor_symbol(),
                },
                budget,
            )?;
            push(&mut indices, index, budget)?;
        }
        sort(
            &mut names,
            &mut |a, b, budget| compare(a.entry, b.entry, budget),
            budget,
        )?;
        sort(
            &mut indices,
            &mut |a, b, budget| {
                compare(
                    kernels[*a].kernel.id.as_str(),
                    kernels[*b].kernel.id.as_str(),
                    budget,
                )
            },
            budget,
        )?;
        let mut previous = None;
        for (&kernel, descriptor) in indices.iter().zip(&names) {
            let name = kernels[kernel].kernel.id.as_str();
            if compare(name, descriptor.entry, budget)? != Ordering::Equal {
                return Err(E::Invalid("descriptor entry bijection"));
            }
            if let Some(old) = previous
                && compare(old, name, budget)? != Ordering::Less
            {
                return Err(E::Invalid("duplicate kernel export"));
            }
            budget.charge_work(
                descriptor
                    .symbol
                    .len()
                    .checked_add(1)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let symbol = descriptor
                .symbol
                .strip_suffix(".kd")
                .ok_or(E::Invalid("descriptor suffix"))?;
            if compare(name, symbol, budget)? != Ordering::Equal {
                return Err(E::Invalid("descriptor symbol pair"));
            }
            push(
                &mut rows,
                Root {
                    kernel,
                    descriptor: descriptor.ordinal,
                },
                budget,
            )?;
            previous = Some(name);
        }
    }
    budget.release_storage(
        budget
            .storage()
            .checked_sub(scratch_floor)
            .ok_or(Resource::Accounting)?,
    )?;
    Ok(rows)
}

pub(super) fn profile<'wire, D: TableQuery<'wire>>(
    table: &D,
    profile: Profile,
    budget: &mut Budget<'_>,
) -> R<()> {
    budget.charge_work(36 + profile.device_target().len())?;
    let target =
        AmdTargetId::parse(profile.device_target()).map_err(|_| E::Invalid("closed profile"))?;
    if table.device_target().as_amd_target_id() != target
        || table.code_object_version() != CodeObjectVersion::V6
        || table.canonical_code_object_digest().as_bytes() != &[0; 32]
    {
        return Err(E::Invalid("descriptor target/COV6/zero digest"));
    }
    Ok(())
}
fn exact_output_bytes(owner: &[u8], published: &[u8], budget: &mut Budget<'_>) -> R<()> {
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
fn engine_text(
    maximum: usize,
    budget: &mut Budget<'_>,
    run: impl FnOnce() -> R<String>,
) -> R<(String, usize)> {
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
    if actual >= requested {
        budget.reserve_storage(actual - requested)?;
    } else {
        budget.release_storage(requested - actual)?;
    }
    if text.len() > maximum {
        return Err(E::Invalid("native text bound"));
    }
    Ok((text, actual))
}
#[cfg(test)]
fn suffix_length(bytes: usize) -> Result<usize, Resource> {
    suffix_length_for(bytes, PREFIX)
}
fn suffix_length_for(bytes: usize, section: &[u8]) -> Result<usize, Resource> {
    let chunks = bytes.checked_add(15).ok_or(Resource::Arithmetic)? / 16;
    bytes
        .checked_mul(6)
        .and_then(|n| chunks.checked_mul(18).and_then(|m| n.checked_add(m)))
        .and_then(|n| n.checked_add(section.len()))
        .ok_or(Resource::Arithmetic)
}
struct Exact<'a> {
    bytes: &'a [u8],
    cursor: usize,
}
impl Exact<'_> {
    fn take(&mut self, expected: &[u8]) -> R<()> {
        let end = self
            .cursor
            .checked_add(expected.len())
            .ok_or(Resource::Arithmetic)?;
        if self.bytes.get(self.cursor..end) != Some(expected) {
            return Err(E::Invalid("exact native/descriptor text"));
        }
        self.cursor = end;
        Ok(())
    }
}
#[cfg(test)]
fn compare_text(prefix: &[u8], descriptor: &[u8], text: &[u8], budget: &mut Budget<'_>) -> R<()> {
    compare_text_for(prefix, descriptor, text, PREFIX, budget)
}
pub(super) fn compare_text_for(
    prefix: &[u8],
    descriptor: &[u8],
    text: &[u8],
    section: &[u8],
    budget: &mut Budget<'_>,
) -> R<()> {
    budget.charge_work(2)?;
    let expected = prefix
        .len()
        .checked_add(suffix_length_for(descriptor.len(), section)?)
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(
        expected
            .checked_add(text.len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    if text.len() != expected {
        return Err(E::Invalid("complete native text length"));
    }
    budget.reserve_storage(size_of::<Exact<'_>>() + 4)?;
    let mut exact = Exact {
        bytes: text,
        cursor: 0,
    };
    exact.take(prefix)?;
    exact.take(section)?;
    for chunk in descriptor.chunks(16) {
        exact.take(b"module asm \".byte ")?;
        for (index, byte) in chunk.iter().copied().enumerate() {
            if index != 0 {
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
    if exact.cursor != text.len() {
        return Err(E::Invalid("trailing native text"));
    }
    drop(exact);
    budget.release_storage(size_of::<Exact<'_>>() + 4)?;
    Ok(())
}

/// Checks the actual final graph, catalog, physical descriptor, full requirements,
/// LLVM22-bound native prefix and exactly one V3 suffix. No producer formatter,
/// V1 table, optimizer, source reconstruction, or stage selector is invoked.
/// All input backing stays prepaid. Success transfers an unreserved fixed header;
/// reserve storage() while its borrows live, then drop before release. Existing
/// native/layout engines keep their own internal bounds; visible returned buffers
/// and all new query/sort/comparison scratch use this single cumulative ledger.
pub fn check_native_v12_text_descriptor_relation_v3<'o, 'c, 'd, 'w, 'l>(
    output: &'o Owner,
    catalog: &'c Catalog,
    published_output_bytes: &[u8],
    selected: Profile,
    descriptors: &'d Table<'w>,
    final_llvm: &'l str,
    budget: &mut Budget<'_>,
) -> R<ReplayedNativeV12TextDescriptorRelationV3<'o, 'c, 'd, 'w, 'l>> {
    scoped(budget, |budget| {
        let prefix_bytes = check_relation(
            output,
            catalog,
            published_output_bytes,
            selected,
            descriptors,
            final_llvm,
            budget,
        )?;
        let bytes = size_of::<ReplayedNativeV12TextDescriptorRelationV3<'_, '_, '_, '_, '_>>();
        budget.charge_work(1)?;
        budget.reserve_storage(bytes)?;
        Ok(ReplayedNativeV12TextDescriptorRelationV3 {
            output,
            catalog,
            descriptors,
            final_llvm,
            profile: selected,
            prefix_bytes,
            storage: NativeV12TextDescriptorReplayStorageV3(bytes),
        })
    })
}

// The same engine and operation order serve both typed public boundaries.
// Only the private query types/scratch and exact section prefix vary.
pub(super) fn check_relation<'wire, D: TableQuery<'wire>>(
    output: &Owner,
    catalog: &Catalog,
    published_output_bytes: &[u8],
    selected: Profile,
    descriptors: &D,
    final_llvm: &str,
    budget: &mut Budget<'_>,
) -> R<usize> {
    exact_output_bytes(
        output.canonical().canonical_bytes(),
        published_output_bytes,
        budget,
    )?;
    budget.charge_work(2)?;
    if final_llvm.is_empty() || final_llvm.len() > MAX_COMPILER_MODULE_TEXT_BYTES {
        return Err(E::Invalid("final native bound"));
    }
    profile(descriptors, selected, budget)?;
    let (inventory, receipt) = Inventory::derive(output, budget).map_err(E::Inventory)?;
    budget.reserve_storage(receipt.retained_storage())?;
    {
        let (checked, receipt) =
            check_kernel_ir_contract_catalog_v1(&inventory, catalog, budget).map_err(E::Catalog)?;
        budget.reserve_storage(receipt.retained_storage())?;
        drop(checked);
        budget.release_storage(receipt.retained_storage())?;
    }
    let roster = roots(&inventory, descriptors, budget)?;
    crate::descriptor_physical_abi_v3::check(&inventory, descriptors, &roster, budget)?;
    crate::descriptor_capability_projection_v3::check(
        &inventory,
        descriptors,
        selected,
        &roster,
        budget,
    )?;
    let (dialect, dialect_storage) = engine_text(MAX_COMPILER_MODULE_TEXT_BYTES, budget, || {
        match selected {
            Profile::Gfx942 => lower_942(output),
            Profile::Gfx950 => lower_950(output),
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
    compare_text_for(
        prefix.as_bytes(),
        descriptors.canonical_bytes(),
        final_llvm.as_bytes(),
        D::SUFFIX,
        budget,
    )?;
    let prefix_bytes = prefix.len();
    drop(prefix);
    budget.release_storage(prefix_storage)?;
    Ok(prefix_bytes)
}

#[cfg(test)]
#[path = "native_v12_text_descriptor_resources_v3_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "native_v12_text_descriptor_replay_v3_tests.rs"]
pub(super) mod tests;
