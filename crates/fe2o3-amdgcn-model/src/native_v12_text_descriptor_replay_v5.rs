//! Conditional V5 physical/native content replay, never CPU or machine authority.
use crate::native_v12_text_descriptor_replay_v3 as shared;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_descriptor::DeviceDescriptorTableV5 as Table;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    InertCanonicalKernelIrContractCatalogV1 as Catalog,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{error::Error, fmt, mem::size_of};

/// A typed V5 refusal carrying the existing physical/query/text diagnostic.
/// This error wrapper cannot convert a V5 table or relation to an older version.
#[derive(Debug)]
pub struct NativeV12TextDescriptorReplayErrorV5(shared::NativeV12TextDescriptorReplayErrorV3);
impl fmt::Display for NativeV12TextDescriptorReplayErrorV5 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native V12/V5 descriptor replay: {:?}", self.0)
    }
}
impl Error for NativeV12TextDescriptorReplayErrorV5 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

/// Unreserved fixed header; all five borrowed backings remain separately prepaid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeV12TextDescriptorReplayStorageV5(usize);
impl NativeV12TextDescriptorReplayStorageV5 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Exact physical/requirements/catalog/native text relation for a V5 table.
/// This does NOT check that its V2 contracts match CPU/source/formula custody,
/// establish Rust nominal/ownership provenance, or prove LLVM/machine semantics.
/// It grants no artifact, publication or launch authority.
///
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV5 as R;
/// fn duplicate(v: R<'_, '_, '_, '_, '_>) { let _ = v.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV5 as R;
/// fn forge<'a>() -> R<'a, 'a, 'a, 'a, 'a> { R::default() }
/// ```
/// ```compile_fail,E0308
/// use fe2o3_amdgcn_model::{ReplayedNativeV12TextDescriptorRelationV5 as R5, ReplayedNativeV12TextDescriptorRelationV3 as R3};
/// fn downgrade<'a>(v: R5<'a, 'a, 'a, 'a, 'a>) -> R3<'a, 'a, 'a, 'a, 'a> { v }
/// ```
/// ```compile_fail,E0308
/// use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV5 as R;
/// use fe2o3_kernel_descriptor::DeviceDescriptorTableV3;
/// fn old_table<'a>(v: &R<'a, 'a, 'a, 'a, 'a>) -> &'a DeviceDescriptorTableV3<'a> { v.descriptors() }
/// ```
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV5 as R;
/// fn escape_output<'a>(v: R<'a, 'static, 'static, 'static, 'static>) -> R<'static, 'static, 'static, 'static, 'static> { v }
/// ```
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV5 as R;
/// fn escape_catalog<'a>(v: R<'static, 'a, 'static, 'static, 'static>) -> R<'static, 'static, 'static, 'static, 'static> { v }
/// ```
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV5 as R;
/// fn escape_table<'a>(v: R<'static, 'static, 'a, 'static, 'static>) -> R<'static, 'static, 'static, 'static, 'static> { v }
/// ```
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV5 as R;
/// fn escape_wire<'a>(v: R<'static, 'static, 'a, 'a, 'static>) -> R<'static, 'static, 'a, 'static, 'static> { v }
/// ```
/// ```compile_fail
/// use fe2o3_amdgcn_model::ReplayedNativeV12TextDescriptorRelationV5 as R;
/// fn escape_native<'a>(v: R<'static, 'static, 'static, 'static, 'a>) -> R<'static, 'static, 'static, 'static, 'static> { v }
/// ```
pub struct ReplayedNativeV12TextDescriptorRelationV5<'o, 'c, 'd, 'w, 'l> {
    output: &'o Owner,
    catalog: &'c Catalog,
    descriptors: &'d Table<'w>,
    final_llvm: &'l str,
    profile: Profile,
    prefix_bytes: usize,
    storage: NativeV12TextDescriptorReplayStorageV5,
}
impl<'o, 'c, 'd, 'w, 'l> ReplayedNativeV12TextDescriptorRelationV5<'o, 'c, 'd, 'w, 'l> {
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
    pub const fn storage(&self) -> NativeV12TextDescriptorReplayStorageV5 {
        self.storage
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Runs the existing inventory/ABI/capability/native engines on actual F and a
/// typed V5 table, requiring exactly one complete `.fe2o3.kd.v5` suffix. No older
/// table/relation is constructed. Contract semantics require a separate genuine
/// source/proof join; this function checks the complete contract bytes only.
/// Inputs stay prepaid; reserve the returned unreserved header while it lives.
pub fn check_native_v12_text_descriptor_relation_v5<'o, 'c, 'd, 'w, 'l>(
    output: &'o Owner,
    catalog: &'c Catalog,
    published_output_bytes: &[u8],
    selected: Profile,
    descriptors: &'d Table<'w>,
    final_llvm: &'l str,
    budget: &mut Budget<'_>,
) -> Result<
    ReplayedNativeV12TextDescriptorRelationV5<'o, 'c, 'd, 'w, 'l>,
    NativeV12TextDescriptorReplayErrorV5,
> {
    shared::scoped(budget, |budget| {
        let prefix_bytes = shared::check_relation(
            output,
            catalog,
            published_output_bytes,
            selected,
            descriptors,
            final_llvm,
            budget,
        )?;
        let bytes = size_of::<ReplayedNativeV12TextDescriptorRelationV5<'_, '_, '_, '_, '_>>();
        budget.charge_work(1)?;
        budget.reserve_storage(bytes)?;
        Ok(ReplayedNativeV12TextDescriptorRelationV5 {
            output,
            catalog,
            descriptors,
            final_llvm,
            profile: selected,
            prefix_bytes,
            storage: NativeV12TextDescriptorReplayStorageV5(bytes),
        })
    })
    .map_err(NativeV12TextDescriptorReplayErrorV5)
}

#[cfg(test)]
#[path = "native_v12_text_descriptor_replay_v5_tests.rs"]
mod tests;
