//! Workload-neutral disposition of reviewed device semantic terminals.

use rustc_hir::def_id::DefId;
use rustc_middle::ty::TyCtxt;

use crate::trusted_device_items::{self, TrustedDeviceItem};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionTerminalExpansionV1 {
    ThreadIndex1d,
    ThreadIndexGet,
    DisjointSliceGetMut,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionSemanticTerminalRuleV1 {
    Expand(ProductionTerminalExpansionV1),
    Reject(TrustedDeviceItem),
}

impl ProductionSemanticTerminalRuleV1 {
    pub(crate) const fn from_trusted_device_item(item: TrustedDeviceItem) -> Self {
        match item {
            TrustedDeviceItem::ThreadIndex1d => {
                Self::Expand(ProductionTerminalExpansionV1::ThreadIndex1d)
            }
            TrustedDeviceItem::ThreadIndexGet => {
                Self::Expand(ProductionTerminalExpansionV1::ThreadIndexGet)
            }
            TrustedDeviceItem::DisjointSliceGetMut => {
                Self::Expand(ProductionTerminalExpansionV1::DisjointSliceGetMut)
            }
            unsupported => Self::Reject(unsupported),
        }
    }

    #[cfg(test)]
    const fn trusted_device_item(self) -> TrustedDeviceItem {
        match self {
            Self::Expand(ProductionTerminalExpansionV1::ThreadIndex1d) => {
                TrustedDeviceItem::ThreadIndex1d
            }
            Self::Expand(ProductionTerminalExpansionV1::ThreadIndexGet) => {
                TrustedDeviceItem::ThreadIndexGet
            }
            Self::Expand(ProductionTerminalExpansionV1::DisjointSliceGetMut) => {
                TrustedDeviceItem::DisjointSliceGetMut
            }
            Self::Reject(item) => item,
        }
    }
}

pub(crate) fn classify(tcx: TyCtxt<'_>, def_id: DefId) -> Option<ProductionSemanticTerminalRuleV1> {
    trusted_device_items::classify(tcx, def_id)
        .map(ProductionSemanticTerminalRuleV1::from_trusted_device_item)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_terminals_have_explicit_workload_neutral_expansions() {
        let cases = [
            (
                TrustedDeviceItem::ThreadIndex1d,
                ProductionTerminalExpansionV1::ThreadIndex1d,
            ),
            (
                TrustedDeviceItem::ThreadIndexGet,
                ProductionTerminalExpansionV1::ThreadIndexGet,
            ),
            (
                TrustedDeviceItem::DisjointSliceGetMut,
                ProductionTerminalExpansionV1::DisjointSliceGetMut,
            ),
        ];
        for (item, expansion) in cases {
            let rule = ProductionSemanticTerminalRuleV1::from_trusted_device_item(item);
            assert_eq!(rule, ProductionSemanticTerminalRuleV1::Expand(expansion));
            assert_eq!(rule.trusted_device_item(), item);
        }
    }

    #[test]
    fn every_unimplemented_terminal_is_retained_as_an_explicit_rejection() {
        for item in [
            TrustedDeviceItem::MemoryVolatileLoad,
            TrustedDeviceItem::MemoryVolatileStore,
            TrustedDeviceItem::MemoryCopyNonOverlapping,
            TrustedDeviceItem::WorkgroupSyncthreads,
            TrustedDeviceItem::DeviceMatrixMultiplyAccumulate,
        ] {
            let rule = ProductionSemanticTerminalRuleV1::from_trusted_device_item(item);
            assert_eq!(rule, ProductionSemanticTerminalRuleV1::Reject(item));
            assert_eq!(rule.trusted_device_item(), item);
        }
    }
}
