//! Authenticated semantic identities imported from rustc.
//!
//! This module is the extension boundary between rustc definition recognition
//! and MIR import. Recognizers receive a resolved [`DefId`]; source paths and
//! other textual spellings never grant semantic authority. New semantic
//! feature families should add a variant here and authenticate it from rustc
//! identity before lowering consumes it.

use crate::trusted_device_items::{self, TrustedDeviceItem};
use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AuthenticatedSemanticItem {
    TrustedDevice(TrustedDeviceItem),
}

impl AuthenticatedSemanticItem {
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

pub(crate) fn classify(tcx: TyCtxt<'_>, def_id: DefId) -> Option<AuthenticatedSemanticItem> {
    trusted_device_items::classify(tcx, def_id).map(AuthenticatedSemanticItem::TrustedDevice)
}

#[cfg(test)]
mod tests {
    use super::AuthenticatedSemanticItem;
    use crate::trusted_device_items::TrustedDeviceItem;

    #[test]
    fn trusted_device_adapter_preserves_identity_and_canonical_diagnostics() {
        for item in [
            TrustedDeviceItem::ThreadIndex1d,
            TrustedDeviceItem::ThreadIndexGet,
            TrustedDeviceItem::DisjointSliceGetMut,
        ] {
            let authenticated = AuthenticatedSemanticItem::trusted_device_for_test(item);
            assert_eq!(authenticated.trusted_device_item(), item);
            assert_eq!(authenticated.canonical_path(), item.canonical_path());
        }
    }
}
