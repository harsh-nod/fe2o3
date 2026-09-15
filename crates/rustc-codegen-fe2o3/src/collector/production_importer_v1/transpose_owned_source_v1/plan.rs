use super::*;

/// The complete retained source roster, not a set of optional per-type matches.
/// No public constructor or Clone permits a caller to insert its own receipt.
pub(in super::super) struct SourcePlan<'tcx> {
    flows: Vec<CheckedFlow<'tcx>>,
}

pub(in super::super) struct MappedPlan<'a> {
    pub(super) source: &'a AdmittedInertSemanticMirV1,
    pub(super) root: SemanticFunctionIdV1,
    pub(super) flows: Vec<MappedFlow<'a>>,
}

fn role(expansion: Expansion) -> Option<protocol::Role> {
    match expansion {
        Expansion::Execution(Terminal::Gfx950TransposeIssue) => Some(protocol::Role::Issue),
        Expansion::Execution(
            Terminal::Gfx950TransposeStageB4 | Terminal::Gfx950TransposeStageB8,
        ) => Some(protocol::Role::Stage),
        Expansion::Execution(Terminal::Gfx950TransposePublish) => Some(protocol::Role::Publish),
        _ => None,
    }
}

impl<'tcx> SourcePlan<'tcx> {
    pub(in super::super) fn observe(
        tcx: TyCtxt<'tcx>,
        auth: &Authentication<'_, 'tcx>,
        work: &mut usize,
    ) -> PlanResult<Self> {
        auth::roster(tcx, auth, work)?;
        let mut expected = Vec::new();
        for recipe in auth.retained.terminal_expansion_producers() {
            bounded::charge(work, 1)?;
            if let Some(role) = role(recipe.expansion) {
                bounded::push(
                    &mut expected,
                    (
                        role,
                        Site {
                            function: recipe.caller,
                            block: mir::BasicBlock::from_u32(recipe.block),
                        },
                    ),
                    work,
                )?;
            }
        }
        let mut roster = protocol::Roster::new(expected, work)?;
        let mut flows = Vec::new();
        for publish in auth.retained.terminal_expansion_producers() {
            bounded::charge(work, 1)?;
            if role(publish.expansion) != Some(protocol::Role::Publish) {
                continue;
            }
            let flow = CheckedFlow::check(tcx, auth, publish, work)?;
            let c = &flow.coordinates;
            roster.consume_flow(c.issue, c.stage, c.publish, work)?;
            bounded::push(&mut flows, flow, work)?;
        }
        roster.finish(work)?;
        Ok(Self { flows })
    }

    pub(in super::super) fn replay<'a>(
        self,
        tcx: TyCtxt<'tcx>,
        auth: &Authentication<'a, 'tcx>,
        work: &mut usize,
    ) -> PlanResult<MappedPlan<'a>> {
        // Rebuild complete coverage, not just the remembered Publish subset.
        let observed = Self::observe(tcx, auth, work)?;
        if observed.flows.len() != self.flows.len() {
            return Err(Error::Source("transpose source roster changed on replay").into());
        }
        for (before, now) in self.flows.iter().zip(&observed.flows) {
            bounded::charge(work, 1 + before.nodes.borrows.len())?;
            if before.nodes != now.nodes
                || before.coordinates != now.coordinates
                || before.source_binding != now.source_binding
            {
                return Err(Error::Source("transpose source row changed on replay").into());
            }
        }
        drop(self);
        // One owner for the whole replay, never one fresh budget per row/body.
        let mut replay = Replay::new(tcx, auth.retained, work)?;
        let mut flows = Vec::new();
        for flow in observed.flows {
            let mapped = mapping::flow(auth, &mut replay, flow, work)?;
            bounded::insert_unique_by(
                &mut flows,
                mapped,
                |before, after| before.issue.cmp(&after.issue),
                work,
            )?;
        }
        let root = auth.root.selected_root;
        bounded::charge(work, auth.semantic.roots().len() + 1)?;
        if !auth.semantic.roots().contains(&root)
            || auth
                .semantic
                .functions()
                .get(root.index() as usize)
                .is_none_or(|f| f.identity().as_bytes() != &auth.root.root_function_identity)
        {
            return Err(
                Error::Source("transpose selected root lost canonical source identity").into(),
            );
        }
        Ok(MappedPlan {
            source: auth.semantic,
            root,
            flows,
        })
    }
}

impl<'a> MappedPlan<'a> {
    /// Consume all rows together into the future canonical builder. This is
    /// source evidence only; SSA/loan/epoch consumers must still bind each row.
    pub(in super::super) fn into_flows(self) -> Vec<MappedFlow<'a>> {
        self.flows
    }
}

#[cfg(test)]
#[path = "plan/tests.rs"]
pub(super) mod tests;
