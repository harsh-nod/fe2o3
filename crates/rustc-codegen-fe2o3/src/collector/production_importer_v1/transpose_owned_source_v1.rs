//! Private source custody for optimized-away transpose inputs. This is not an
//! issuer, an SSA definition factory, or permission to accept an inert MIR row.
use super::*;
use crate::collector::production_importer_v1::source_body_v1::{Correspondence, Replay};
use crate::production_semantic_terminal_v1::{
    ProductionExecutionTerminalV1 as Terminal, ProductionTerminalExpansionV1 as Expansion,
};
use crate::rustc_semantic_adapter_v1::{
    SemanticIdentityDigestV1, canonical_function_identities_v1,
};
use crate::rustc_semantic_plan_v1::{ProductionSemanticPreflightPlanV1, TerminalExpansionRecipeV1};
use fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAssignmentV1, SemanticBlockIdV1, SemanticConstantValueV1, SemanticDirectCallV1,
    SemanticFunctionDeclV1, SemanticLocalIdV1, SemanticOperandV1, SemanticRvalueKindV1,
    SemanticStatementKindV1, SemanticTerminatorKindV1,
};
use rustc_hir::{HirId, def_id::LocalDefId};
use rustc_middle::{
    mir,
    ty::{self, Instance, TyCtxt},
};

#[path = "transpose_owned_source_v1/auth.rs"]
mod auth;
#[path = "transpose_owned_source_v1/bounded.rs"]
mod bounded;
#[path = "transpose_owned_source_v1/consumer.rs"]
mod consumer;
#[path = "transpose_owned_source_v1/canonical.rs"]
mod canonical;
#[path = "transpose_owned_source_v1/production.rs"]
mod production;
#[path = "transpose_owned_source_v1/ssa.rs"]
mod ssa;
#[path = "transpose_owned_source_v1/error.rs"]
mod error;
#[path = "transpose_owned_source_v1/typed.rs"]
pub(super) mod typed;
pub(super) use production::{SourceSeal, attach};
#[path = "transpose_owned_source_v1/hir.rs"]
mod hir;
#[path = "transpose_owned_source_v1/mapping.rs"]
mod mapping;
#[path = "transpose_owned_source_v1/plan.rs"]
mod plan;
#[path = "transpose_owned_source_v1/protocol.rs"]
mod protocol;
#[path = "transpose_owned_source_v1/raw.rs"]
mod raw;
#[path = "transpose_owned_source_v1/rules.rs"]
mod rules;

#[cfg(test)]
#[path = "transpose_owned_source_v1/tests.rs"]
pub(super) mod tests;

pub(super) use consumer::{BoundFlow, BoundPlan};
pub(super) use plan::{MappedPlan, SourcePlan};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    Work,
    Depth,
    Allocation,
    Source(&'static str),
}
type Result<T> = std::result::Result<T, Error>;

/// Preserve replay/construction errors, including their original resource
/// payloads. A source-shape rejection does not replace a shared-budget error.
#[derive(Debug)]
pub(crate) enum PlanError {
    Source(Error),
    Replay(ProductionSemanticImportErrorV1),
    Ssa(fe2o3_pliron::ProductionSemanticSsaErrorV1),
    Query(fe2o3_pliron::ProductionSemanticSsaSourceQueryErrorV1),
    Typed(fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1),
}

impl From<Error> for PlanError {
    fn from(error: Error) -> Self {
        Self::Source(error)
    }
}

impl From<ProductionSemanticImportErrorV1> for PlanError {
    fn from(error: ProductionSemanticImportErrorV1) -> Self {
        Self::Replay(error)
    }
}

impl From<fe2o3_pliron::ProductionSemanticSsaSourceQueryErrorV1> for PlanError {
    fn from(error: fe2o3_pliron::ProductionSemanticSsaSourceQueryErrorV1) -> Self {
        Self::Query(error)
    }
}

type PlanResult<T> = std::result::Result<T, PlanError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Site {
    function: SemanticFunctionIdV1, // retained producer coordinate, not final MIR
    block: mir::BasicBlock,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BorrowUse<'tcx> {
    receiver: HirId,
    method: HirId,
    instance: Instance<'tcx>,
    site: Site,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Nodes<'tcx> {
    outer: LocalDefId,
    closure: LocalDefId,
    issued_binding: HirId,
    staged_binding: HirId,
    workgroup_binding: HirId,
    issue: HirId,
    matrix_call: HirId,
    capture: HirId,
    stage: HirId,
    stage_receiver: HirId,
    publish: HirId,
    publish_receiver: HirId,
    workgroup_receiver: HirId,
    capture_field: u32,
    workgroup_parameter: usize,
    borrows: Vec<BorrowUse<'tcx>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Coordinates {
    issue: Site,
    capture: (Site, usize),
    matrix_call: Site,
    closure_call: Site,
    stage: Site,
    publish: Site,
    workgroup_local: mir::Local,
}

/// Construction/replay require the live retained plan plus the existing closed
/// source validators. The root is never reconstructed from a ZST or a type.
pub(super) struct Authentication<'a, 'tcx> {
    pub(super) semantic: &'a AdmittedInertSemanticMirV1,
    pub(super) root: &'a AuthenticatedProductionKernelContextRootV1,
    pub(super) contexts: &'a AuthenticatedProductionKernelContextsV1,
    pub(super) retained: &'a ProductionSemanticPreflightPlanV1<'tcx>,
}

/// Non-Clone and private fields: only successful live checks create a receipt.
pub(super) struct CheckedFlow<'tcx> {
    nodes: Nodes<'tcx>,
    coordinates: Coordinates,
    source_binding: [u8; 32],
}

/// Private inert coordinates for the proposed canonical builder. No public
/// constructor and no claim that decoding these fields recreates the receipt.
pub(super) struct MappedFlow<'a> {
    pub(super) issue: (SemanticFunctionIdV1, SemanticBlockIdV1),
    pub(super) capture: (SemanticFunctionIdV1, SemanticBlockIdV1, u32),
    pub(super) capture_field: u32,
    pub(super) matrix_call: (SemanticFunctionIdV1, SemanticBlockIdV1),
    pub(super) closure_call: (SemanticFunctionIdV1, SemanticBlockIdV1),
    pub(super) stage: (SemanticFunctionIdV1, SemanticBlockIdV1),
    pub(super) publish: (SemanticFunctionIdV1, SemanticBlockIdV1),
    pub(super) workgroup_local: SemanticLocalIdV1,
    // Every shared use must be discharged by the existing Workgroup endpoint.
    pub(super) workgroup_borrows: Vec<(SemanticFunctionIdV1, SemanticBlockIdV1)>,
    pub(super) source_binding: [u8; 32],
    // These full replay-checked bodies feed the future canonical builder. It
    // derives body commitments internally; the caller supplies no body hash.
    pub(super) bodies: [(SemanticFunctionIdV1, &'a SemanticFunctionDeclV1); 3],
}

impl<'tcx> CheckedFlow<'tcx> {
    pub(super) fn check(
        tcx: TyCtxt<'tcx>,
        auth: &Authentication<'_, 'tcx>,
        publish: &TerminalExpansionRecipeV1<'tcx>,
        work: &mut usize,
    ) -> Result<Self> {
        if publish.expansion != Expansion::Execution(Terminal::Gfx950TransposePublish) {
            return Err(Error::Source(
                "owned source flow is not a closed Publish occurrence",
            ));
        }
        let observed = hir::observe(tcx, auth, publish, work)?;
        let coordinates = raw::check(tcx, auth, publish, &observed, work)?;
        let nodes = observed.nodes;
        let mut digest = SemanticIdentityDigestV1::new(b"fe2o3/production/transpose-owned-flow/v1");
        bounded::charge(work, auth.retained.canonical_transcript().len().div_ceil(8))?;
        digest.field(auth.retained.canonical_transcript());
        digest.field(&auth.root.root_function_identity);
        digest.field(&auth.root.issuance_identity);
        for node in [
            nodes.issued_binding,
            nodes.staged_binding,
            nodes.workgroup_binding,
            nodes.issue,
            nodes.matrix_call,
            nodes.capture,
            nodes.stage,
            nodes.stage_receiver,
            nodes.publish,
            nodes.publish_receiver,
            nodes.workgroup_receiver,
        ] {
            bounded::charge(work, 2)?;
            digest.field(
                &tcx.def_path_hash(node.owner.def_id.to_def_id())
                    .0
                    .to_le_bytes(),
            );
            digest.field(&node.local_id.as_u32().to_le_bytes());
        }
        digest.field(&nodes.capture_field.to_le_bytes());
        for borrow in &nodes.borrows {
            bounded::charge(work, 4)?;
            digest.field(
                canonical_function_identities_v1(tcx, borrow.instance)
                    .function()
                    .as_bytes(),
            );
            digest.field(&borrow.receiver.local_id.as_u32().to_le_bytes());
            digest.field(&borrow.method.local_id.as_u32().to_le_bytes());
            digest.field(&(borrow.site.block.index() as u64).to_le_bytes());
        }
        Ok(Self {
            nodes,
            coordinates,
            source_binding: digest.finish(),
        })
    }
}

fn function_for_instance<'tcx>(
    retained: &ProductionSemanticPreflightPlanV1<'tcx>,
    instance: Instance<'tcx>,
    work: &mut usize,
) -> Result<SemanticFunctionIdV1> {
    let mut found = None;
    for (index, producer) in retained.function_producers().iter().enumerate() {
        bounded::charge(work, 1)?;
        if producer.instance == instance
            && found
                .replace(SemanticFunctionIdV1::from_index(
                    u32::try_from(index).map_err(|_| Error::Work)?,
                ))
                .is_some()
        {
            return Err(Error::Source(
                "duplicate retained monomorphic function instance",
            ));
        }
    }
    found.ok_or(Error::Source(
        "missing retained monomorphic function instance",
    ))
}

fn normalize<'tcx, T: ty::TypeFoldable<TyCtxt<'tcx>>>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    value: T,
) -> Result<T> {
    instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            ty::TypingEnv::fully_monomorphized(),
            ty::EarlyBinder::bind(value),
        )
        .map_err(|_| Error::Source("owned source type/instance failed normalization"))
}
