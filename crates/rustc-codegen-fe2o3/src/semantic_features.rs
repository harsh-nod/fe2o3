//! Compilation-session semantic identities recognized from rustc.
//!
//! This module is the extension boundary between rustc definition recognition
//! and MIR import. Recognizers receive a resolved [`DefId`]; source paths and
//! other textual spellings never establish an identity. A recognized item only
//! records exact `DefId` equality in the current compilation session. It carries
//! no persistent provider provenance by itself and grants no proof, executable,
//! or artifact authority. Exact profiles that stop MIR traversal at one of these
//! items must separately bind its role, path, DefPathHash, and provider identity.
//! New semantic feature families should add a variant here and recognize it from
//! rustc identity before lowering consumes it.

use crate::trusted_device_items::{self, TrustedDeviceItem};
use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SessionRecognizedSemanticItem {
    TrustedDevice(TrustedDeviceItem),
    #[cfg(feature = "qualification-oracles-test-only")]
    FlashAttentionCompilerIntrinsic(
        crate::collected_flash_attention_v1::FlashAttentionCompilerIntrinsicV1,
    ),
    #[cfg(feature = "qualification-oracles-test-only")]
    MoeTop2CompilerIntrinsic(crate::collected_moe_top2_v1::MoeTop2CompilerIntrinsicV1),
    #[cfg(feature = "qualification-oracles-test-only")]
    WorkgroupSyncCompilerIntrinsic(WorkgroupSyncCompilerIntrinsicV1),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[cfg(feature = "qualification-oracles-test-only")]
pub(crate) enum WorkgroupSyncCompilerIntrinsicV1 {
    ThreadIdxX,
    ThreadIdxY,
    ThreadIdxZ,
    BlockIdxX,
    BlockIdxY,
    BlockIdxZ,
    BlockDimX,
    BlockDimY,
    BlockDimZ,
    LaunchExtent1d,
    ScratchFromDynamicLds,
    ColdPath,
    AtomicXadd,
}

#[cfg(feature = "qualification-oracles-test-only")]
impl WorkgroupSyncCompilerIntrinsicV1 {
    pub(crate) const fn canonical_path(self) -> &'static str {
        match self {
            Self::ThreadIdxX => "fe2o3_device::thread::thread_idx_x",
            Self::ThreadIdxY => "fe2o3_device::thread::thread_idx_y",
            Self::ThreadIdxZ => "fe2o3_device::thread::thread_idx_z",
            Self::BlockIdxX => "fe2o3_device::thread::block_idx_x",
            Self::BlockIdxY => "fe2o3_device::thread::block_idx_y",
            Self::BlockIdxZ => "fe2o3_device::thread::block_idx_z",
            Self::BlockDimX => "fe2o3_device::thread::block_dim_x",
            Self::BlockDimY => "fe2o3_device::thread::block_dim_y",
            Self::BlockDimZ => "fe2o3_device::thread::block_dim_z",
            Self::LaunchExtent1d => "fe2o3_device::thread::launch_extent_1d",
            Self::ScratchFromDynamicLds => {
                "fe2o3_device::WorkgroupCollectiveScratch::from_dynamic_lds"
            }
            Self::ColdPath => "core::intrinsics::cold_path",
            Self::AtomicXadd => "core::intrinsics::atomic_xadd",
        }
    }

    pub(crate) const fn is_rustc_intrinsic(self) -> bool {
        matches!(self, Self::ColdPath | Self::AtomicXadd)
    }
}

impl SessionRecognizedSemanticItem {
    pub(crate) fn canonical_path(self) -> &'static str {
        match self {
            Self::TrustedDevice(item) => item.canonical_path(),
            #[cfg(feature = "qualification-oracles-test-only")]
            Self::FlashAttentionCompilerIntrinsic(item) => item.canonical_path(),
            #[cfg(feature = "qualification-oracles-test-only")]
            Self::MoeTop2CompilerIntrinsic(item) => item.canonical_path(),
            #[cfg(feature = "qualification-oracles-test-only")]
            Self::WorkgroupSyncCompilerIntrinsic(item) => item.canonical_path(),
        }
    }

    pub(crate) const fn trusted_device_item(self) -> Option<TrustedDeviceItem> {
        match self {
            Self::TrustedDevice(item) => Some(item),
            #[cfg(feature = "qualification-oracles-test-only")]
            Self::FlashAttentionCompilerIntrinsic(_)
            | Self::MoeTop2CompilerIntrinsic(_)
            | Self::WorkgroupSyncCompilerIntrinsic(_) => None,
        }
    }

    #[cfg(test)]
    pub(crate) const fn trusted_device_for_test(item: TrustedDeviceItem) -> Self {
        Self::TrustedDevice(item)
    }
}

pub(crate) fn classify(tcx: TyCtxt<'_>, def_id: DefId) -> Option<SessionRecognizedSemanticItem> {
    let trusted = trusted_device_items::classify(tcx, def_id)
        .map(SessionRecognizedSemanticItem::TrustedDevice);
    #[cfg(feature = "qualification-oracles-test-only")]
    {
        return trusted.or_else(|| {
            crate::collected_flash_attention_v1::classify_exact_flash_attention_compiler_intrinsic(
                tcx, def_id,
            )
            .map(SessionRecognizedSemanticItem::FlashAttentionCompilerIntrinsic)
        })
        .or_else(|| {
            crate::collected_moe_top2_v1::classify_exact_moe_top2_compiler_intrinsic(tcx, def_id)
                .map(SessionRecognizedSemanticItem::MoeTop2CompilerIntrinsic)
        })
        .or_else(|| {
            crate::collected_workgroup_sync_v1::classify_exact_workgroup_sync_compiler_intrinsic(
                tcx, def_id,
            )
            .map(SessionRecognizedSemanticItem::WorkgroupSyncCompilerIntrinsic)
        });
    }
    #[cfg(not(feature = "qualification-oracles-test-only"))]
    trusted
}

#[cfg(test)]
mod tests {
    use super::SessionRecognizedSemanticItem;
    use crate::trusted_device_items::TrustedDeviceItem;

    #[test]
    fn session_recognition_preserves_identity_and_canonical_diagnostics() {
        for item in [
            TrustedDeviceItem::ThreadIndex1d,
            TrustedDeviceItem::ThreadIndexGet,
            TrustedDeviceItem::DisjointSliceGetMut,
            TrustedDeviceItem::MemoryOffsetFrom,
            TrustedDeviceItem::MemoryVolatileLoad,
            TrustedDeviceItem::MemoryVolatileStore,
            TrustedDeviceItem::MemoryCopyNonOverlapping,
            TrustedDeviceItem::MemoryCopyOneNonOverlapping,
        ] {
            let recognized = SessionRecognizedSemanticItem::trusted_device_for_test(item);
            assert_eq!(recognized.trusted_device_item(), Some(item));
            assert_eq!(recognized.canonical_path(), item.canonical_path());
        }
    }

    #[test]
    #[cfg(feature = "qualification-oracles-test-only")]
    fn compiler_only_terminals_do_not_claim_device_lowering_authority() {
        let compiler_only = [
            (
                SessionRecognizedSemanticItem::FlashAttentionCompilerIntrinsic(
                    crate::collected_flash_attention_v1::FlashAttentionCompilerIntrinsicV1::FabsF32,
                ),
                "core::intrinsics::fabs::<f32>",
            ),
            (
                SessionRecognizedSemanticItem::MoeTop2CompilerIntrinsic(
                    crate::collected_moe_top2_v1::MoeTop2CompilerIntrinsicV1::FabsF32,
                ),
                "core::intrinsics::fabs::<f32>",
            ),
            (
                SessionRecognizedSemanticItem::WorkgroupSyncCompilerIntrinsic(
                    super::WorkgroupSyncCompilerIntrinsicV1::ThreadIdxX,
                ),
                "fe2o3_device::thread::thread_idx_x",
            ),
            (
                SessionRecognizedSemanticItem::WorkgroupSyncCompilerIntrinsic(
                    super::WorkgroupSyncCompilerIntrinsicV1::ColdPath,
                ),
                "core::intrinsics::cold_path",
            ),
            (
                SessionRecognizedSemanticItem::WorkgroupSyncCompilerIntrinsic(
                    super::WorkgroupSyncCompilerIntrinsicV1::AtomicXadd,
                ),
                "core::intrinsics::atomic_xadd",
            ),
        ];
        for (recognized, expected_path) in compiler_only {
            assert_eq!(recognized.trusted_device_item(), None);
            assert_eq!(recognized.canonical_path(), expected_path);
        }
    }
}
