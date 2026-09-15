//! Compiler-private body selection. A failed proof never changes source MIR.

use super::{
    core_option_compare_v1::{
        ReviewedPromotedOptionCompareV1, prove_promoted_option_comparisons_v1,
    },
    core_primitive_value_v1::{ReviewedCorePrimitiveCastV1, prove_core_primitive_cast_v1},
    core_wrapping_v1::{
        ReviewedCoreWrappingShiftV1, ReviewedU32WrappingShrV1, prove_core_u32_wrapping_shr_v1,
        prove_core_wrapping_shift_v1,
    },
};
use rustc_middle::{
    mir::Body,
    ty::{Instance, TyCtxt},
};
use std::borrow::Cow;

#[derive(Debug)]
enum SourceExpansionV1<'tcx> {
    PromotedOptionCompare(ReviewedPromotedOptionCompareV1<'tcx>),
    WrappingShift(ReviewedU32WrappingShrV1<'tcx>),
    GeneralWrappingShift(ReviewedCoreWrappingShiftV1<'tcx>),
    PrimitiveCast(ReviewedCorePrimitiveCastV1<'tcx>),
}

impl<'tcx> SourceExpansionV1<'tcx> {
    fn expand_mir(&self, tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Body<'tcx> {
        match self {
            Self::PromotedOptionCompare(proof) => {
                debug_assert_eq!(proof.instance(), instance);
                proof.expand_mir(tcx)
            }
            Self::WrappingShift(proof) => {
                debug_assert_eq!(proof.instance(), instance);
                proof.expand_mir(tcx)
            }
            Self::GeneralWrappingShift(proof) => {
                debug_assert_eq!(proof.instance(), instance);
                proof.expand_mir(tcx)
            }
            Self::PrimitiveCast(proof) => {
                debug_assert_eq!(proof.instance(), instance);
                proof.expand_mir(tcx)
            }
        }
    }

    fn fingerprint(&self, tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> [u8; 16] {
        match self {
            Self::PromotedOptionCompare(proof) => proof.expansion_fingerprint(tcx, body),
            Self::WrappingShift(proof) => proof.expansion_fingerprint(tcx, body),
            Self::GeneralWrappingShift(proof) => proof.expansion_fingerprint(tcx, body),
            Self::PrimitiveCast(proof) => proof.expansion_fingerprint(tcx, body),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ProductionMirV1<'tcx> {
    instance: Instance<'tcx>,
    body: Cow<'tcx, Body<'tcx>>,
    proof: Option<SourceExpansionV1<'tcx>>,
}

impl<'tcx> ProductionMirV1<'tcx> {
    pub(crate) fn instance(&self) -> Instance<'tcx> {
        self.instance
    }

    pub(crate) fn body(&self) -> &Body<'tcx> {
        &self.body
    }

    pub(crate) fn is_source_expansion(&self) -> bool {
        self.proof.is_some()
    }

    pub(crate) fn expansion_fingerprint(&self, tcx: TyCtxt<'tcx>) -> Option<[u8; 16]> {
        self.proof
            .as_ref()
            .map(|proof| proof.fingerprint(tcx, &self.body))
    }
}

/// Select before recursive call discovery, then retain the selection through
/// preflight and ordinary semantic construction. No helper is a panic terminal.
pub(crate) fn production_mir_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> ProductionMirV1<'tcx> {
    let proof = prove_core_u32_wrapping_shr_v1(tcx, instance)
        .map(SourceExpansionV1::WrappingShift)
        .or_else(|| {
            prove_core_wrapping_shift_v1(tcx, instance).map(SourceExpansionV1::GeneralWrappingShift)
        })
        .or_else(|| {
            prove_core_primitive_cast_v1(tcx, instance).map(SourceExpansionV1::PrimitiveCast)
        })
        .or_else(|| {
            prove_promoted_option_comparisons_v1(tcx, instance)
                .map(SourceExpansionV1::PromotedOptionCompare)
        });
    let body = match &proof {
        Some(proof) => Cow::Owned(proof.expand_mir(tcx, instance)),
        None => Cow::Borrowed(tcx.instance_mir(instance.def)),
    };
    ProductionMirV1 {
        instance,
        body,
        proof,
    }
}
