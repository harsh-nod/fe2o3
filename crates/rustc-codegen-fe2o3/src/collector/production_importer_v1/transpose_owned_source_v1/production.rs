//! Live provider attachment. This runs while original rustc source owners are
//! available; later structural consumers must join the private final-owner seal.
use super::*;

#[cfg(test)]
#[path = "production_tests.rs"]
pub(super) mod tests;

/// Move-only, compiler-private source evidence. Neither inert footer rows nor a
/// public digest constructor can manufacture this object.
pub(in super::super) struct SourceSeal {
    semantic: [u8; 32],
    root: SemanticFunctionIdV1,
    rows: usize,
}

impl SourceSeal {
    pub(in super::super) fn matches(
        &self,
        owner: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        root: SemanticFunctionIdV1,
    ) -> bool {
        self.root == root
            && self.rows != 0
            && owner.source_semantic().roots() == [root]
            && owner.source_semantic().transpose_owned_flows().len() == self.rows
            && owner.source_semantic().semantic_sha256().as_bytes() == &self.semantic
    }

    /// The live provider already authenticated this exact final byte owner.
    /// Rebuild borrowed coordinates after the move into SSA; all SSA uses are
    /// then obtained afresh from this one replayed owner, never transported IDs.
    pub(in super::super) fn rebind<'a>(
        &self,
        owner: &'a fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        root: SemanticFunctionIdV1,
        work: &mut usize,
    ) -> PlanResult<BoundPlan<'a>> {
        bounded::charge(work, 1)?;
        if !self.matches(owner, root) {
            return Err(Error::Source("transpose final owner lost its live source seal").into());
        }
        let source = owner.source_semantic();
        let mut flows = Vec::new();
        for row in source.transpose_owned_flows() {
            bounded::charge(work, 26)?;
            let sites = row.sites();
            let mut borrows = Vec::new();
            for site in row.workgroup_borrows() {
                bounded::push(&mut borrows, (site.function, site.block), work)?;
            }
            let [outer, helper, closure] = row.bodies();
            let body = |id: SemanticFunctionIdV1| {
                source.functions().get(id.index() as usize).ok_or_else(|| {
                    PlanError::Source(Error::Source("transpose sealed body is absent"))
                })
            };
            let pair = |site: fe2o3_mir_model::semantic_mir_v1::SemanticOwnedSourceCallSiteV1| {
                (site.function, site.block)
            };
            bounded::push(
                &mut flows,
                MappedFlow {
                    issue: pair(sites.issue),
                    capture: (
                        sites.capture.function,
                        sites.capture.block,
                        sites.capture.statement,
                    ),
                    capture_field: sites.capture_field,
                    matrix_call: pair(sites.matrix_call),
                    closure_call: pair(sites.closure_call),
                    stage: pair(sites.stage),
                    publish: pair(sites.publish),
                    workgroup_local: sites.workgroup_local,
                    workgroup_borrows: borrows,
                    source_binding: *row.source_binding(),
                    bodies: [
                        (outer.function(), body(outer.function())?),
                        (helper.function(), body(helper.function())?),
                        (closure.function(), body(closure.function())?),
                    ],
                },
                work,
            )?;
        }
        MappedPlan {
            source,
            root,
            flows,
        }
        .bind(owner, work)
    }
}

pub(in super::super) fn attach<'tcx>(
    tcx: TyCtxt<'tcx>,
    retained: &ProductionSemanticPreflightPlanV1<'tcx>,
    contexts: &mut AuthenticatedProductionKernelContextsV1,
    semantic: AdmittedInertSemanticMirV1,
) -> PlanResult<AdmittedInertSemanticMirV1> {
    let limits = SemanticMirLimitsV1::default();
    let mut work = usize::try_from(limits.limit(SemanticMirResourceV1::ValidationWork))
        .map_err(|_| Error::Work)?;
    let mut fragment_bytes = limits.limit(SemanticMirResourceV1::CanonicalBytes);
    if contexts.transpose_source.is_some() || !semantic.transpose_owned_flows().is_empty() {
        return Err(Error::Source("transpose source custody was already attached").into());
    }
    let mut occurrences = 0usize;
    for recipe in retained.terminal_expansion_producers() {
        bounded::charge(&mut work, 1)?;
        if matches!(
            recipe.expansion,
            Expansion::Execution(
                Terminal::Gfx950TransposeIssue
                    | Terminal::Gfx950TransposeStageB4
                    | Terminal::Gfx950TransposeStageB8
                    | Terminal::Gfx950TransposePublish
            )
        ) {
            occurrences = occurrences.checked_add(1).ok_or(Error::Work)?;
        }
    }
    if occurrences == 0 {
        return Ok(semantic);
    }
    // Explicit bounded first scope. No flow belonging to another root is
    // filtered out or silently represented by an empty canonical footer.
    let [root] = contexts.roots.as_ref() else {
        return Err(Error::Source(
            "transpose source requires a complete single authenticated root partition",
        )
        .into());
    };
    let selected_root = root.selected_root;
    if semantic.roots() != [selected_root] {
        return Err(
            Error::Source("transpose canonical and authenticated root rosters differ").into(),
        );
    }
    let rows = {
        let auth = Authentication {
            semantic: &semantic,
            root,
            contexts,
            retained,
        };
        SourcePlan::observe(tcx, &auth, &mut work)?
            .replay(tcx, &auth, &mut work)?
            .into_canonical_rows(&mut fragment_bytes, &mut work)?
    };
    if rows.is_empty() {
        return Err(
            Error::Source("transpose live occurrence roster produced no source flow").into(),
        );
    }
    let row_count = rows.len();
    // The common model owns this consuming overload. No full-table clone or
    // stale preliminary semantic/expansion identity is carried forward.
    let wire_version = semantic.wire_version().max(fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V25);
    let semantic = semantic
        .with_transpose_owned_flows_for_wire_version(rows, wire_version, limits)
        .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    {
        let auth = Authentication {
            semantic: &semantic,
            root,
            contexts,
            retained,
        };
        SourcePlan::observe(tcx, &auth, &mut work)?
            .replay(tcx, &auth, &mut work)?
            .check_canonical_footer(&mut fragment_bytes, &mut work)?;
    }
    // No fallible work follows publication of the seal. A later Context entry
    // failure still discards the entire ConstructedProductionSemanticMir owner.
    contexts.transpose_source = Some(SourceSeal {
        semantic: *semantic.semantic_sha256().as_bytes(),
        root: selected_root,
        rows: row_count,
    });
    Ok(semantic)
}
