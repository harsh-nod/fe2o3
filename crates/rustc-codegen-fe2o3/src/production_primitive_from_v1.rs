//! Checked normalization of a concrete pinned-core primitive From body.
//! Rustc CTFE is trusted for exact scalar values, not recursively source-audited.
use fe2o3_mir_model::semantic_mir_v1::{SemanticMirLimitsV1, SemanticMirResourceV1};
use rustc_abi::CanonAbi;
use rustc_middle::{
    mir::Body,
    ty::{self, Instance, Ty, TyCtxt, TypingEnv},
};
use rustc_target::callconv::{FnAbi, PassMode};
use std::fmt;

mod checker;
mod raw;
mod scalar;
mod scratch;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrimitiveFromStageV1 {
    Collector,
    ClosureReplay,
    Preflight,
    BodyReplay,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrimitiveFromSlabV1 {
    Header,
    Locals,
    Aliases,
    Blocks,
    Structure,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrimitiveFromBackingV1 {
    Requested,
    Actual,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PrimitiveFromAllocationPhaseV1 {
    slab: PrimitiveFromSlabV1,
    backing: PrimitiveFromBackingV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrimitiveFromResourceV1 {
    Structural {
        resource: SemanticMirResourceV1,
        actual: u64,
        maximum: u64,
    },
    SizeOverflow {
        phase: PrimitiveFromAllocationPhaseV1,
    },
    ScratchLimit {
        phase: PrimitiveFromAllocationPhaseV1,
        actual: u64,
        maximum: u64,
    },
    Allocation {
        phase: PrimitiveFromAllocationPhaseV1,
        requested_elements: usize,
        element_bytes: usize,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RawSiteV1 {
    Signature,
    Local(u32),
    RequiredConstant(u32),
    Statement { block: u32, statement: u32 },
    Terminator(u32),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrimitiveFromRawRefusalV1 {
    Body,
    Type,
    Abi,
    ConstantIdentity,
    ConstantEvaluation,
    ConstantValue,
    LiteralAllocation,
    LiteralBytes,
    Place,
    Statement,
    Rvalue,
    Terminator,
    Edge,
    Unwind,
    Call(crate::production_raw_call_audit_v1::RawCallRefusalV1),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PrimitiveFromSemanticRefusalV1 {
    Unsupported,
    UnknownBranch,
    InvalidScalar,
    WrongReturn,
    Cycle,
    Coordinate,
}
#[derive(Debug)]
pub(crate) enum PrimitiveFromErrorV1<E> {
    Work {
        stage: PrimitiveFromStageV1,
        source: E,
    },
    Resource {
        stage: PrimitiveFromStageV1,
        source: PrimitiveFromResourceV1,
    },
    RawPolicy {
        stage: PrimitiveFromStageV1,
        site: RawSiteV1,
        reason: PrimitiveFromRawRefusalV1,
    },
    Semantics {
        stage: PrimitiveFromStageV1,
        site: RawSiteV1,
        reason: PrimitiveFromSemanticRefusalV1,
    },
}
impl<E: fmt::Debug> fmt::Display for PrimitiveFromErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "checked primitive From: ")?;
        match self {
            Self::Work { stage, source } => write!(f, "{stage:?} Work {source:?}"),
            Self::Resource { stage, source } => write!(f, "{stage:?} Resource {source}"),
            Self::RawPolicy {
                stage,
                site,
                reason,
            } => write!(f, "{stage:?} RawPolicy {site}: {reason}"),
            Self::Semantics {
                stage,
                site,
                reason,
            } => write!(f, "{stage:?} Semantics {site}: {reason:?}"),
        }
    }
}
impl fmt::Display for PrimitiveFromRawRefusalV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Call(reason) => write!(f, "Call({reason:?})"),
            other => write!(f, "{other:?}"),
        }
    }
}
impl fmt::Display for RawSiteV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Signature => write!(f, "Signature"),
            Self::Local(local) => write!(f, "Local({local})"),
            Self::RequiredConstant(index) => write!(f, "RequiredConstant({index})"),
            Self::Statement { block, statement } => write!(f, "Statement({block}, {statement})"),
            Self::Terminator(block) => write!(f, "Terminator({block})"),
        }
    }
}
impl fmt::Display for PrimitiveFromResourceV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Structural {
                resource,
                actual,
                maximum,
            } => write!(
                f,
                "Structural {resource:?}: actual={actual}, maximum={maximum}"
            ),
            Self::SizeOverflow { phase } => {
                write!(f, "SizeOverflow {:?}/{:?}", phase.slab, phase.backing)
            }
            Self::ScratchLimit {
                phase,
                actual,
                maximum,
            } => write!(
                f,
                "ScratchLimit {:?}/{:?}: actual={actual}, maximum={maximum}",
                phase.slab, phase.backing
            ),
            Self::Allocation {
                phase,
                requested_elements,
                element_bytes,
            } => write!(
                f,
                "Allocation {:?}/{:?}: requested_elements={requested_elements}, element_bytes={element_bytes}",
                phase.slab, phase.backing
            ),
        }
    }
}
type Result<T, E> = std::result::Result<T, PrimitiveFromErrorV1<E>>;
pub(crate) fn primitive_widening_types_v1(input: Ty<'_>, output: Ty<'_>) -> bool {
    scalar::ty(input)
        .zip(scalar::ty(output))
        .is_some_and(|(a, b)| scalar::lossless(a, b))
}
use PrimitiveFromAllocationPhaseV1 as Phase;
use PrimitiveFromBackingV1 as Backing;
use PrimitiveFromRawRefusalV1 as Raw;
use PrimitiveFromResourceV1 as Resource;
use PrimitiveFromSemanticRefusalV1 as Semantic;
use PrimitiveFromSlabV1 as Slab;
use PrimitiveFromStageV1 as Stage;

struct Context<'a, E, F: FnMut(usize) -> std::result::Result<(), E>> {
    stage: Stage,
    limits: SemanticMirLimitsV1,
    charge: &'a mut F,
    counts: [u64; 8],
}
impl<E, F: FnMut(usize) -> std::result::Result<(), E>> Context<'_, E, F> {
    fn work(&mut self, amount: usize) -> Result<(), E> {
        (self.charge)(amount).map_err(|source| PrimitiveFromErrorV1::Work {
            stage: self.stage,
            source,
        })
    }
    fn raw(&self, site: RawSiteV1, reason: Raw) -> PrimitiveFromErrorV1<E> {
        PrimitiveFromErrorV1::RawPolicy {
            stage: self.stage,
            site,
            reason,
        }
    }
    fn semantic(&self, site: RawSiteV1, reason: Semantic) -> PrimitiveFromErrorV1<E> {
        PrimitiveFromErrorV1::Semantics {
            stage: self.stage,
            site,
            reason,
        }
    }
    fn resource(&self, source: Resource) -> PrimitiveFromErrorV1<E> {
        PrimitiveFromErrorV1::Resource {
            stage: self.stage,
            source,
        }
    }
    fn structural(&mut self, resource: SemanticMirResourceV1, amount: usize) -> Result<(), E> {
        use SemanticMirResourceV1 as R;
        let slot = match resource {
            R::Locals => 0,
            R::Blocks => 1,
            R::Statements => 2,
            R::Projections => 3,
            R::Operands => 4,
            R::CallArguments => 5,
            R::SwitchTargets => 6,
            R::ConstantBytes => 7,
            _ => return Err(self.raw(RawSiteV1::Signature, Raw::Body)),
        };
        let phase = Phase {
            slab: Slab::Structure,
            backing: Backing::Requested,
        };
        let amount =
            u64::try_from(amount).map_err(|_| self.resource(Resource::SizeOverflow { phase }))?;
        let actual = self.counts[slot]
            .checked_add(amount)
            .ok_or_else(|| self.resource(Resource::SizeOverflow { phase }))?;
        let maximum = self.limits.limit(resource);
        if actual > maximum {
            return Err(self.resource(Resource::Structural {
                resource,
                actual,
                maximum,
            }));
        }
        self.counts[slot] = actual;
        Ok(())
    }
}

/// Private fixed-size actual-query custody. No invocation scratch is retained.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CheckedPrimitiveFromV1<'tcx> {
    instance: Instance<'tcx>,
    body: &'tcx Body<'tcx>,
    abi: &'tcx FnAbi<'tcx, Ty<'tcx>>,
}
impl<'tcx> CheckedPrimitiveFromV1<'tcx> {
    pub(crate) fn input_type(self) -> Ty<'tcx> {
        self.abi.args[0].layout.ty
    }
    pub(crate) fn output_type(self) -> Ty<'tcx> {
        self.abi.ret.layout.ty
    }
    pub(crate) fn body(self) -> &'tcx Body<'tcx> {
        self.body
    }
    pub(crate) fn abi(self) -> &'tcx FnAbi<'tcx, Ty<'tcx>> {
        self.abi
    }
    pub(crate) fn same_producers(self, other: Self) -> bool {
        self.instance == other.instance
            && std::ptr::eq(self.body, other.body)
            && std::ptr::eq(self.abi, other.abi)
    }
}

pub(crate) fn check_primitive_from_v1<'tcx, E>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    stage: Stage,
    limits: SemanticMirLimitsV1,
    charge: &mut impl FnMut(usize) -> std::result::Result<(), E>,
) -> Result<Option<CheckedPrimitiveFromV1<'tcx>>, E> {
    let mut cx = Context {
        stage,
        limits,
        charge,
        counts: [0; 8],
    };
    let Some((input, output)) =
        crate::trusted_device_items::primitive_from_candidate_v1(tcx, instance, &mut |n| {
            cx.work(n)
        })?
    else {
        return Ok(None);
    };
    cx.work(1)?;
    let abi = tcx
        .fn_abi_of_instance(
            TypingEnv::fully_monomorphized().as_query_input((instance, ty::List::empty())),
        )
        .map_err(|_| cx.raw(RawSiteV1::Signature, Raw::Abi))?;
    if abi.conv != CanonAbi::Rust
        || abi.c_variadic
        || abi.fixed_count != 1
        || abi.args.len() != 1
        || abi.args[0].layout.ty != input
        || abi.ret.layout.ty != output
        || !matches!(abi.args[0].mode, PassMode::Direct(_))
        || !matches!(abi.ret.mode, PassMode::Direct(_))
        || abi.args[0].layout.size.bits()
            != u64::from(
                scalar::ty(input)
                    .ok_or_else(|| cx.raw(RawSiteV1::Signature, Raw::Type))?
                    .bit_width(),
            )
        || abi.ret.layout.size.bits()
            != u64::from(
                scalar::ty(output)
                    .ok_or_else(|| cx.raw(RawSiteV1::Signature, Raw::Type))?
                    .bit_width(),
            )
    {
        return Err(cx.raw(RawSiteV1::Signature, Raw::Abi));
    }
    cx.work(1)?;
    let body = tcx.instance_mir(instance.def);
    // The checker receives actual query products, never a producer success bit.
    checker::check(tcx, instance, body, input, output, &mut cx)?;
    Ok(Some(CheckedPrimitiveFromV1 {
        instance,
        body,
        abi,
    }))
}

#[cfg(test)]
mod tests;
