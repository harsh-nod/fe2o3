//! Call-expanded plans retain a separate identity and share the module's resource envelope.

use super::*;
use fe2o3_mir_model::SsaInputSiteV1;

/// Exclusive event endpoints recorded by the adapter that actually emitted them.
/// One row per nonempty statement/terminator, not one row per SSA event.
#[derive(Default)]
pub(super) struct ExecutionEventOriginsV1(Vec<(u32, usize, Option<u32>)>);

impl ExecutionEventOriginsV1 {
    pub(super) fn record(
        &mut self,
        block: u32,
        statement: Option<u32>,
        events: std::ops::Range<usize>,
    ) {
        if !events.is_empty() {
            self.0.push((block, events.end, statement));
        }
    }

    pub(super) fn statement(&self, block: u32, event: u32) -> Option<Option<u32>> {
        let index = self
            .0
            .partition_point(|(candidate, end, _)| (*candidate, *end) <= (block, event as usize));
        self.0
            .get(index)
            .filter(|(candidate, _, _)| *candidate == block)
            .map(|(_, _, statement)| *statement)
    }

    pub(super) fn resources(
        &self,
    ) -> Result<SemanticSsaAuxiliaryResourcesV1, ProductionSemanticSsaErrorV1> {
        // Include vector growth/minimum-capacity slack and diagnostic lookup work.
        let storage_words = self
            .0
            .len()
            .checked_mul(6)
            .and_then(|words| words.checked_add(if self.0.is_empty() { 0 } else { 12 }))
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        let work_units = self
            .0
            .len()
            .checked_mul(2)
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        Ok(SemanticSsaAuxiliaryResourcesV1 {
            storage_words,
            work_units,
        })
    }
}

pub(super) fn wrap_error(
    view: &SemanticExpandedRootV1,
    events: &ExecutionEventOriginsV1,
    error: ProductionSemanticSsaErrorV1,
) -> ProductionSemanticSsaErrorV1 {
    let mut block = None;
    let mut target = None;
    let mut local = None;
    let mut statement = None;
    match &error {
        ProductionSemanticSsaErrorV1::Planner { function, error }
            if *function == view.source_body() =>
        {
            match error {
                SsaPlannerErrorV1::UndefinedAtUse {
                    block: at,
                    event,
                    variable,
                } => {
                    block = Some(at.get());
                    local = Some(variable.get());
                    statement = events.statement(at.get(), *event);
                }
                SsaPlannerErrorV1::UnknownVariable { site, variable, .. } => {
                    local = Some(variable.get());
                    match site {
                        SsaInputSiteV1::Event { block: at, event } => {
                            block = Some(at.get());
                            statement = events.statement(at.get(), *event);
                        }
                        SsaInputSiteV1::EdgeDefinition { edge, .. } => {
                            block = Some(edge.source().get());
                            statement = Some(None);
                        }
                        SsaInputSiteV1::EntryDefinition(_) => {}
                    }
                }
                SsaPlannerErrorV1::UndefinedAtEdge {
                    edge,
                    target: at,
                    variable,
                } => {
                    block = Some(edge.source().get());
                    target = Some(at.get());
                    local = Some(variable.get());
                    statement = Some(None);
                }
                SsaPlannerErrorV1::UndefinedAtEntry { variable } => local = Some(variable.get()),
                SsaPlannerErrorV1::InvalidEntry { entry, .. } => block = Some(entry.get()),
                SsaPlannerErrorV1::UnknownTarget {
                    edge, target: at, ..
                } => {
                    block = Some(edge.source().get());
                    target = Some(at.get());
                    statement = Some(None);
                }
                SsaPlannerErrorV1::InvalidEdgeRole { edge }
                | SsaPlannerErrorV1::NonCanonicalDefinitions { edge: Some(edge) } => {
                    block = Some(edge.source().get());
                    statement = Some(None);
                }
                _ => {}
            }
        }
        ProductionSemanticSsaErrorV1::PartialMove {
            function,
            block: at,
            statement: at_statement,
            local: variable,
            ..
        } if *function == view.source_body() => {
            block = Some(*at);
            local = Some(*variable);
            statement = Some(*at_statement);
        }
        _ => {}
    }
    let block_origin = block.and_then(|block| view.block_origins().get(block as usize));
    let coordinates = |origin: &fe2o3_mir_model::SemanticExpandedBlockOriginV1| {
        (origin.instance(), origin.function(), origin.block())
    };
    ProductionSemanticSsaErrorV1::ExpandedExecution {
        root: view.root(),
        execution_view_identity: *view.identity(),
        source_block: block_origin.map(coordinates),
        source_target: target
            .and_then(|target| view.block_origins().get(target as usize))
            .map(coordinates),
        source_statement: statement
            .flatten()
            .and_then(|statement| block_origin?.statements().get(statement as usize).copied()),
        source_terminator: match statement {
            Some(None) => block_origin.map(|origin| origin.terminator()),
            _ => None,
        },
        source_local: local
            .and_then(|local| view.local_origins().get(local as usize))
            .copied(),
        error: Box::new(error),
    }
}

pub(super) struct ExecutionSsaPlansV1 {
    expansion: SemanticCallExpansionV1,
    plans: Box<[(SemanticFunctionIdV1, ProductionSemanticSsaFunctionPlanV1)]>,
    summary: ProductionSemanticSsaSummaryV1,
    identity: ProductionSemanticSsaIdentityV1,
}

impl ExecutionSsaPlansV1 {
    pub(super) fn try_new(
        semantic: &AdmittedInertSemanticMirV1,
        source_plans: &[ProductionSemanticSsaFunctionPlanV1],
        limits: ProductionSemanticSsaLimitsV1,
        source_summary: ProductionSemanticSsaSummaryV1,
        source_identity: ProductionSemanticSsaIdentityV1,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        let expansion =
            SemanticCallExpansionV1::try_new(semantic, SemanticCallExpansionLimitsV1::default())
                .map_err(ProductionSemanticSsaErrorV1::CallExpansion)?;
        let (plans, summary, identity) = construct_plans(
            semantic,
            &expansion,
            source_plans,
            limits,
            source_summary,
            source_identity,
        )?;
        Ok(Self {
            expansion,
            plans,
            summary,
            identity,
        })
    }

    pub(super) fn verify_replay(
        &self,
        semantic: &AdmittedInertSemanticMirV1,
        source_plans: &[ProductionSemanticSsaFunctionPlanV1],
        limits: ProductionSemanticSsaLimitsV1,
        source_summary: ProductionSemanticSsaSummaryV1,
        source_identity: ProductionSemanticSsaIdentityV1,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        self.expansion
            .verify_replay(semantic)
            .map_err(ProductionSemanticSsaErrorV1::CallExpansion)?;
        let (plans, summary, identity) = construct_plans(
            semantic,
            &self.expansion,
            source_plans,
            limits,
            source_summary,
            source_identity,
        )?;
        if plans != self.plans || summary != self.summary || identity != self.identity {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        Ok(())
    }

    pub(super) const fn expansion(&self) -> &SemanticCallExpansionV1 {
        &self.expansion
    }
    pub(super) const fn summary(&self) -> ProductionSemanticSsaSummaryV1 {
        self.summary
    }
    pub(super) const fn identity(&self) -> ProductionSemanticSsaIdentityV1 {
        self.identity
    }

    pub(super) fn plan(
        &self,
        root: SemanticFunctionIdV1,
    ) -> Option<&ProductionSemanticSsaFunctionPlanV1> {
        self.plans
            .binary_search_by_key(&root, |(root, _)| *root)
            .ok()
            .map(|index| &self.plans[index].1)
    }
}

fn construct_plans(
    semantic: &AdmittedInertSemanticMirV1,
    expansion: &SemanticCallExpansionV1,
    source_plans: &[ProductionSemanticSsaFunctionPlanV1],
    limits: ProductionSemanticSsaLimitsV1,
    mut summary: ProductionSemanticSsaSummaryV1,
    source_identity: ProductionSemanticSsaIdentityV1,
) -> Result<
    (
        Box<[(SemanticFunctionIdV1, ProductionSemanticSsaFunctionPlanV1)]>,
        ProductionSemanticSsaSummaryV1,
        ProductionSemanticSsaIdentityV1,
    ),
    ProductionSemanticSsaErrorV1,
> {
    let mut plans = Vec::new();
    let mut digest = Sha256::new();
    digest.update(b"fe2o3.production-semantic-execution-ssa.v1\0");
    digest.update(source_identity.as_bytes());
    digest.update(expansion.identity());
    for view in expansion
        .roots()
        .iter()
        .filter(|view| view.has_expanded_calls())
    {
        let without_events = ExecutionEventOriginsV1::default();
        let initializations = frame_initialization::FrameInitializationsV1::derive(
            semantic,
            view,
            source_plans,
            limits,
        )
        .map_err(|error| wrap_error(view, &without_events, error))?;
        let transparent_borrows = transparent_borrow_sites_v1(view.body(), semantic.callables());
        let plan = plan_semantic_function_ssa_with_borrow_sites_v1(
            view.source_body(),
            view.body(),
            Some(semantic.types()),
            semantic.callables(),
            limits,
            &transparent_borrows,
            Some((view, initializations)),
        )?;
        accumulate_summary_v1(&mut summary, &plan, view.body().locals().len(), limits)
            .map_err(|error| wrap_error(view, &without_events, error))?;
        summary.function_count = summary.function_count.checked_add(1).ok_or_else(|| {
            wrap_error(
                view,
                &without_events,
                ProductionSemanticSsaErrorV1::ResourceOverflow,
            )
        })?;
        digest.update(view.root().index().to_le_bytes());
        digest.update(view.identity());
        plan.frame_initializations.hash_into(&mut digest);
        digest.update(
            derive_semantic_ssa_identity_v1(
                semantic.semantic_sha256().as_bytes(),
                std::slice::from_ref(&plan),
                summary,
            )
            .as_bytes(),
        );
        plans.push((view.root(), plan));
    }
    let identity = if plans.is_empty() {
        source_identity
    } else {
        ProductionSemanticSsaIdentityV1(digest.finalize().into())
    };
    Ok((plans.into_boxed_slice(), summary, identity))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_replay_rejects_omitted_frame_initializations() {
        let mut owner = frame_initialization::tests::owner(false);
        assert!(
            !owner.execution.plans[0]
                .1
                .frame_initializations()
                .is_empty()
        );
        owner.execution.plans[0].1.frame_initializations =
            frame_initialization::FrameInitializationsV1::default();
        assert_eq!(
            owner.verify_replay(),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        );
    }
}
