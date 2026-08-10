//! Compilation-session semantic identities recognized from rustc.
//!
//! This module is the extension boundary between rustc definition recognition
//! and MIR import. Recognizers receive a resolved [`DefId`]; source paths and
//! other textual spellings never establish an identity. A recognized item only
//! records exact `DefId` equality in the current compilation session. It carries
//! no persistent provider provenance and grants no proof, executable, or
//! artifact authority. New semantic feature families should add a variant here
//! and recognize it from rustc identity before lowering consumes it.

use crate::trusted_device_items::{self, TrustedDeviceItem};
use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SessionRecognizedSemanticItem {
    TrustedDevice(TrustedDeviceItem),
}

impl SessionRecognizedSemanticItem {
    pub(crate) fn canonical_path(self) -> &'static str {
        match self {
            Self::TrustedDevice(item) => item.canonical_path(),
        }
    }

    pub(crate) fn trusted_device_item(self) -> TrustedDeviceItem {
        match self {
            Self::TrustedDevice(item) => item,
        }
    }

    #[cfg(test)]
    pub(crate) const fn trusted_device_for_test(item: TrustedDeviceItem) -> Self {
        Self::TrustedDevice(item)
    }
}

pub(crate) fn classify(tcx: TyCtxt<'_>, def_id: DefId) -> Option<SessionRecognizedSemanticItem> {
    trusted_device_items::classify(tcx, def_id).map(SessionRecognizedSemanticItem::TrustedDevice)
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
        ] {
            let recognized = SessionRecognizedSemanticItem::trusted_device_for_test(item);
            assert_eq!(recognized.trusted_device_item(), item);
            assert_eq!(recognized.canonical_path(), item.canonical_path());
        }
    }
}
