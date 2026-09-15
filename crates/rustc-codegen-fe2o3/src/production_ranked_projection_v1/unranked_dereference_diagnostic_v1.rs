//! Bounded observations of an already rejected place, never projection authority.

use super::*;
mod retained_types;
use fe2o3_mir_model::{
    SemanticExpandedLocalOriginV1, SemanticExpandedRootV1, SemanticExpandedStatementOriginV1,
    SemanticExpandedTerminatorOriginV1,
};

pub(super) const REASON: &str = "a dereferenced memory access without a ranked index projection";
pub(super) const MAX_PROJECTIONS: usize = 8;
pub(super) const MAX_FRAMES: usize = 8;

#[derive(Clone, Copy, Debug)]
struct TypedProjection {
    kind: SemanticProjectionKindV1,
    input: u32,
    output: u32,
}

#[derive(Clone, Copy, Debug)]
struct Frame {
    instance: u32,
    function: u32,
    identity: SemanticFunctionIdentityV1,
    parent: Option<u32>,
    call_block: Option<u32>,
}

#[derive(Debug)]
struct Expansion {
    retained_types: String,
    root: u32,
    identity: [u8; 32],
    local: Option<SemanticExpandedLocalOriginV1>,
    original_block: Option<(u32, u32, u32)>,
    statement: Option<SemanticExpandedStatementOriginV1>,
    terminator: Option<SemanticExpandedTerminatorOriginV1>,
    source: Option<SemanticSourceProvenanceV1>,
    frames: [Option<Frame>; MAX_FRAMES],
    remaining_frame: Option<u32>,
}

#[derive(Debug)]
pub(crate) struct UnrankedDereferenceDiagnosticV1 {
    function: SemanticFunctionIdentityV1,
    block: usize,
    site: Option<ProjectedSemanticAccessSiteV1>,
    local: u32,
    role: SemanticLocalRoleV1,
    base_type: u32,
    result_type: u32,
    projections: [Option<TypedProjection>; MAX_PROJECTIONS],
    projection_count: usize,
    access: AccessKindAttr,
    requirement: PlaceAccessRequirementV1,
    atomic: Option<SemanticAtomicAccessV1>,
    memory_space: Option<MemorySpaceAttr>,
    allocation: Option<AllocationContractV1>,
    provenance: Option<LocalAllocationProvenanceV1>,
    checked: Option<CheckedReferenceOriginV1>,
    checked_place_matches: bool,
    source: SemanticSourceProvenanceV1,
    expansion: Option<Expansion>,
    private_flow: Option<private_scalar_capture_v1::flow_observation::Observation>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn reject(
    function: &SemanticFunctionDeclV1,
    block: usize,
    place: &SemanticPlaceV1,
    access: AccessKindAttr,
    requirement: PlaceAccessRequirementV1,
    atomic: Option<SemanticAtomicAccessV1>,
    memory_space: Option<MemorySpaceAttr>,
    contracts: &ProjectionLocalContractsV1,
    source: SemanticSourceProvenanceV1,
) -> ProductionRankedProjectionErrorV1 {
    let local = &function.locals()[place.local().index() as usize];
    let mut projections = [None; MAX_PROJECTIONS];
    let mut input = local.ty().index();
    for (slot, projection) in projections.iter_mut().zip(place.projections()) {
        *slot = Some(TypedProjection {
            kind: projection.kind(),
            input,
            output: projection.result_type().index(),
        });
        input = projection.result_type().index();
    }
    let index = place.local().index() as usize;
    ProductionRankedProjectionErrorV1::UnrankedDereference(Box::new(
        UnrankedDereferenceDiagnosticV1 {
            function: function.identity(),
            block,
            site: None,
            local: place.local().index(),
            role: local.role(),
            base_type: local.ty().index(),
            result_type: place.ty().index(),
            projections,
            projection_count: place.projections().len(),
            access,
            requirement,
            atomic,
            memory_space,
            allocation: contracts.allocations.get(index).copied().flatten(),
            provenance: contracts
                .allocation_provenance
                .get(index)
                .copied()
                .flatten(),
            checked: contracts
                .checked_references
                .origins
                .get(index)
                .copied()
                .flatten(),
            checked_place_matches: checked_reference_origin_for_place(
                place,
                &contracts.checked_references.origins,
            )
            .is_some(),
            source,
            expansion: None,
            private_flow: None,
        },
    ))
}

pub(super) fn attach_private_flow(
    mut error: ProductionRankedProjectionErrorV1,
    function: &SemanticFunctionDeclV1,
    reads: Option<&private_scalar_capture_v1::PrivateScalarReads<'_>>,
) -> ProductionRankedProjectionErrorV1 {
    if let ProductionRankedProjectionErrorV1::UnrankedDereference(diagnostic) = &mut error
        && diagnostic.function == function.identity()
    {
        diagnostic.private_flow = reads.and_then(|reads| reads.observation_for(function));
    }
    error
}

pub(super) fn attach_site(
    mut error: ProductionRankedProjectionErrorV1,
    semantic: &AdmittedInertSemanticMirV1,
    view: &SemanticExpandedRootV1,
    site: ProjectedSemanticAccessSiteV1,
) -> ProductionRankedProjectionErrorV1 {
    let ProductionRankedProjectionErrorV1::UnrankedDereference(diagnostic) = &mut error else {
        return error;
    };
    // The caller supplies the SSA owner's checked view. Never guess a source
    // occurrence from a matching type, source span, or function alone.
    if diagnostic.function != view.body().identity() || diagnostic.block != site.block {
        return error;
    }
    diagnostic.site = Some(site);
    let origin = view.block_origins().get(site.block);
    let statement = site
        .statement
        .and_then(|index| origin?.statements().get(index).copied());
    let terminator = site
        .statement
        .is_none()
        .then(|| origin.map(|origin| origin.terminator()))
        .flatten();
    let original_block = origin.and_then(|origin| {
        semantic
            .functions()
            .get(origin.function().index() as usize)?
            .blocks()
            .get(origin.block().index() as usize)
    });
    let source = original_block.and_then(|block| match (site.statement, statement) {
        (_, Some(SemanticExpandedStatementOriginV1::Source { statement })) => block
            .statements()
            .get(statement as usize)
            .map(|statement| statement.source()),
        (Some(_), None) => None,
        _ => Some(block.terminator().source()),
    });
    let mut next = origin.map(|origin| origin.instance().index());
    let mut frames = [None; MAX_FRAMES];
    for slot in &mut frames {
        let Some(instance) = next else { break };
        let Some(frame) = view.instances().get(instance as usize) else {
            break;
        };
        *slot = Some(Frame {
            instance,
            function: frame.function().index(),
            identity: frame.function_identity(),
            parent: frame.parent().map(|parent| parent.index()),
            call_block: frame.call_block().map(|block| block.index()),
        });
        next = frame.parent().map(|parent| parent.index());
    }
    diagnostic.expansion = Some(Expansion {
        retained_types: retained_types::capture(semantic, view, site, diagnostic),
        root: view.root().index(),
        identity: *view.identity(),
        local: view.local_origins().get(diagnostic.local as usize).copied(),
        original_block: origin.map(|origin| {
            (
                origin.instance().index(),
                origin.function().index(),
                origin.block().index(),
            )
        }),
        statement,
        terminator,
        source,
        frames,
        remaining_frame: next,
    });
    error
}

impl fmt::Display for UnrankedDereferenceDiagnosticV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "semantic-to-ranked projection incomplete: {REASON}; execution function={} bb{} ",
            crate::encode_hex(self.function.as_bytes()),
            self.block
        )?;
        match self.site {
            Some(ProjectedSemanticAccessSiteV1 {
                statement: Some(statement),
                ..
            }) => write!(f, "statement={statement}")?,
            Some(_) => write!(f, "terminator")?,
            None => write!(f, "site=unavailable")?,
        }
        write!(
            f,
            " local=_{} role={:?} base_type={} result_type={} access={:?} requirement={:?} atomic={:?} memory_space={:?}; projections=[",
            self.local,
            self.role,
            self.base_type,
            self.result_type,
            self.access,
            self.requirement,
            self.atomic,
            self.memory_space
        )?;
        for (index, projection) in self.projections.iter().flatten().enumerate() {
            if index != 0 {
                write!(f, ", ")?;
            }
            write!(
                f,
                "{}:{:?}->{}",
                projection.input, projection.kind, projection.output
            )?;
        }
        write!(
            f,
            "] projection_count={} omitted={}; allocation={:?} private_or_argument_origin={:?} checked_origin={:?} checked_place_matches={}; {}",
            self.projection_count,
            self.projection_count.saturating_sub(MAX_PROJECTIONS),
            self.allocation,
            self.provenance,
            self.checked,
            self.checked_place_matches,
            source_label(self.source)
        )?;
        if let Some(observation) = self.private_flow {
            write!(f, "; private_capture={observation:?}")?;
        }
        let Some(expansion) = &self.expansion else {
            return write!(f, "; checked_expansion=unavailable");
        };
        write!(
            f,
            "; checked_expansion={} root={} original_local={:?} original_instance_function_block={:?} original_statement={:?} original_terminator={:?}",
            crate::encode_hex(&expansion.identity),
            expansion.root,
            expansion.local,
            expansion.original_block,
            expansion.statement,
            expansion.terminator
        )?;
        if let Some(source) = expansion.source {
            write!(f, " original_location=({})", source_label(source))?;
        }
        for frame in expansion.frames.iter().flatten() {
            write!(
                f,
                "; frame instance={} function={} identity={} parent={:?} original_call_block={:?}",
                frame.instance,
                frame.function,
                crate::encode_hex(frame.identity.as_bytes()),
                frame.parent,
                frame.call_block
            )?;
        }
        if let Some(next) = expansion.remaining_frame {
            write!(f, "; frames_truncated next_instance={next}")?;
        }
        f.write_str(&expansion.retained_types)?;
        Ok(())
    }
}
