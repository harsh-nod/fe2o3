use super::*;
use fe2o3_mir_model::SemanticExpandedTerminatorOriginV1;

/// Ordered index over existing events. It does not authorize the requests or
/// create source values; every selected coordinate is rechecked below.
pub(super) struct CheckedRows<'a> {
    pub(super) owner: &'a ProductionSemanticSsaOwnerV1,
    pub(super) query: ProductionSemanticSsaSourceQueryV1<'a>,
    pub(super) input: &'a PhaseEmissionInputV1,
    pub(super) bindings: &'a [SemanticExpandedDefinedCapabilityV1],
    pub(super) order: Vec<usize>,
}

impl<'a> CheckedRows<'a> {
    pub(super) fn new(
        owner: &'a ProductionSemanticSsaOwnerV1,
        input: &'a PhaseEmissionInputV1,
        work: &mut usize,
    ) -> PhaseResult<Self> {
        let semantic = owner.source_semantic();
        let view = owner
            .execution_view_for_root(input.root)
            .ok_or_else(|| rejected("phase emission root is absent"))?;
        if input.semantic != *semantic.semantic_sha256().as_bytes()
            || input.expansion != *owner.execution_expansion().identity()
            || input.expanded_root != *view.identity()
            || input.source_protocol == [0; 32]
            || semantic.roots() != [input.root]
            || input.phases.is_empty()
        {
            return Err(rejected(
                "phase emission substituted its exact source owner",
            ));
        }
        spend(
            work,
            input
                .retained_bytes()
                .ok_or_else(|| rejected("phase emission request capacity overflow"))?,
        )?;
        let query = owner
            .source_query_for_root(input.root, view.body())
            .map_err(|_| rejected("phase emission lost its exact query attachment"))?;
        let bindings = input.bindings.as_slice();
        let mut next = 0usize;
        for (instance, frame) in view.instances().iter().enumerate() {
            spend(work, 1)?;
            let function = semantic
                .functions()
                .get(frame.function().index() as usize)
                .ok_or_else(|| rejected("phase retained frame function is absent"))?;
            let Some(contract) = function.defined_capability_contract() else {
                continue;
            };
            let binding = bindings
                .get(next)
                .ok_or_else(|| rejected("phase emission omitted an existing defined binding"))?;
            if binding.root() != input.root
                || binding.callee_instance().index() as usize != instance
                || binding.expansion_identity() != &input.expansion
                || binding.root_identity() != &input.expanded_root
                || binding.contract() != *contract
                || frame.function_identity() != function.identity()
                || Some(binding.caller_instance()) != frame.parent()
                || Some(binding.call_block()) != frame.call_block()
            {
                return Err(rejected(
                    "phase emission substituted its complete immutable binding roster",
                ));
            }
            next += 1;
        }
        if next != bindings.len() {
            return Err(rejected(
                "phase emission added an unused or foreign defined binding",
            ));
        }
        let order = reserve(input.rows.len(), work)?;
        let mut checked = Self {
            owner,
            query,
            input,
            bindings,
            order,
        };
        checked.check_roster(work)?;
        for (index, row) in input.rows.iter().enumerate() {
            spend(work, 1)?;
            checked.check_event(row.boundary, work)?;
            checked.check_action(*row, work)?;
            let key = row_key(*row);
            let mut lo = 0usize;
            let mut hi = checked.order.len();
            while lo < hi {
                spend(work, 1)?;
                let middle = lo + (hi - lo) / 2;
                match row_key(input.rows[checked.order[middle]]).cmp(&key) {
                    std::cmp::Ordering::Less => lo = middle + 1,
                    std::cmp::Ordering::Greater => hi = middle,
                    std::cmp::Ordering::Equal => {
                        return Err(rejected("phase emission reused an existing event"));
                    }
                }
            }
            spend(work, checked.order.len() - lo)?;
            checked.order.insert(lo, index);
        }
        Ok(checked)
    }

    pub(super) fn binding(
        &self,
        instance: SemanticCallInstanceIdV1,
        work: &mut usize,
    ) -> PhaseResult<&SemanticExpandedDefinedCapabilityV1> {
        let mut found = None;
        for binding in self.bindings {
            spend(work, 1)?;
            if binding.root() == self.input.root && binding.callee_instance() == instance {
                if found.replace(binding).is_some()
                    || binding.expansion_identity() != &self.input.expansion
                    || binding.root_identity() != &self.input.expanded_root
                {
                    return Err(rejected("phase emission binding is ambiguous or foreign"));
                }
            }
        }
        found.ok_or_else(|| rejected("phase emission binding is absent"))
    }

    fn check_roster(&self, work: &mut usize) -> PhaseResult<()> {
        let mut expected = 0usize;
        for (index, phase) in self.input.phases.iter().enumerate() {
            spend(work, 1)?;
            if phase.leases.len() >= fe2o3_kernel_ir::MAX_EXECUTION_CAPABILITY_RESULTS_V1 {
                return Err(rejected(
                    "phase emission allocation roster exceeds result arity",
                ));
            }
            for (instance, role) in [
                (phase.owner, 0),
                (phase.wrapper, 1),
                (phase.issue, 2),
                (phase.finish, 3),
            ] {
                let binding = self.binding(instance, work)?;
                let Defined::ReusablePhase(record) = binding.contract() else {
                    return Err(rejected("phase emission changed its defined source family"));
                };
                let actual = match record.recipe() {
                    Recipe::OwnerConvert { .. } => 0,
                    Recipe::WithPhase { .. } => 1,
                    Recipe::Issue { .. } => 2,
                    Recipe::Finish { .. } => 3,
                    Recipe::Bind { .. } => 4,
                };
                if role != actual || record.provenance().root() != self.input.root {
                    return Err(rejected("phase emission changed an exact recipe role"));
                }
            }
            let wrapper = self.binding(phase.wrapper, work)?;
            let issue = self.binding(phase.issue, work)?;
            let finish = self.binding(phase.finish, work)?;
            if issue.caller_instance() != wrapper.callee_instance()
                || finish.caller_instance() != phase.closure
            {
                return Err(rejected("phase emission substituted a caller instance"));
            }
            let mut first_owner = true;
            for prior in &self.input.phases[..index] {
                spend(work, 1)?;
                if prior.owner == phase.owner {
                    first_owner = false;
                }
                if prior.wrapper == phase.wrapper
                    || prior.issue == phase.issue
                    || prior.finish == phase.finish
                    || prior.closure == phase.closure
                {
                    return Err(rejected("phase emission reused a phase occurrence"));
                }
            }
            expected = expected
                .checked_add(5 + usize::from(first_owner))
                .and_then(|n| {
                    phase
                        .leases
                        .len()
                        .checked_mul(2)
                        .and_then(|m| n.checked_add(m))
                })
                .ok_or_else(|| rejected("phase emission row count overflow"))?;
            for action in [
                PhaseEmissionActionV1::Begin,
                PhaseEmissionActionV1::Seal,
                PhaseEmissionActionV1::RelayClosure,
                PhaseEmissionActionV1::RelayDrop,
                PhaseEmissionActionV1::End,
            ] {
                self.require_action(index, action, 1, work)?;
            }
            self.require_action(
                index,
                PhaseEmissionActionV1::OwnerConvert,
                usize::from(first_owner),
                work,
            )?;
            for (lease_index, lease) in phase.leases.iter().enumerate() {
                self.require_action(
                    index,
                    PhaseEmissionActionV1::Bind { lease: lease_index },
                    1,
                    work,
                )?;
                self.require_action(
                    index,
                    PhaseEmissionActionV1::CloseStorage { lease: lease_index },
                    1,
                    work,
                )?;
                let bind = self.binding(lease.bind, work)?;
                let storage = self.binding(lease.storage_conversion, work)?;
                if bind.caller_instance() != phase.closure
                    || !matches!(bind.contract(), Defined::ReusablePhase(record)
                        if matches!(record.recipe(), Recipe::Bind { .. }))
                    || !matches!(storage.contract(), Defined::ReusableLdsConversion(_))
                {
                    return Err(rejected(
                        "phase emission changed a Bind or allocation conversion",
                    ));
                }
                for prior in &phase.leases[..lease_index] {
                    spend(work, 1)?;
                    if prior.bind == lease.bind || prior.allocation == lease.allocation {
                        return Err(rejected("phase emission reused a Bind allocation"));
                    }
                }
            }
        }
        if expected != self.input.rows.len() {
            return Err(rejected("phase emission omitted or added lifecycle rows"));
        }
        for binding in self.bindings {
            spend(work, 1)?;
            if binding.root() != self.input.root {
                continue;
            }
            let Defined::ReusablePhase(record) = binding.contract() else {
                continue;
            };
            let mut uses = 0usize;
            for phase in &self.input.phases {
                spend(work, 1)?;
                uses += usize::from(
                    [phase.owner, phase.wrapper, phase.issue, phase.finish]
                        .contains(&binding.callee_instance()),
                );
                for lease in &phase.leases {
                    spend(work, 1)?;
                    uses += usize::from(lease.bind == binding.callee_instance());
                }
            }
            if uses == 0 || (uses != 1 && !matches!(record.recipe(), Recipe::OwnerConvert { .. })) {
                return Err(rejected(
                    "phase emission did not consume the defined occurrence roster",
                ));
            }
        }
        Ok(())
    }

    fn require_action(
        &self,
        phase: usize,
        action: PhaseEmissionActionV1,
        expected: usize,
        work: &mut usize,
    ) -> PhaseResult<()> {
        let mut count = 0usize;
        for row in &self.input.rows {
            spend(work, 1)?;
            count += usize::from(row.phase == phase && row.action == action);
        }
        if count != expected {
            return Err(rejected(
                "phase emission changed the exact lifecycle action roster",
            ));
        }
        Ok(())
    }

    pub(super) fn check_event(
        &self,
        boundary: PhaseEmissionBoundaryV1,
        work: &mut usize,
    ) -> PhaseResult<()> {
        let block = SsaBlockIdV1::new(boundary.site.block().index());
        let site = self
            .query
            .event_site(block, boundary.event, &mut || spend(work, 1).is_ok())
            .map_err(|_| rejected("phase emission event has no original source site"))?;
        if site != boundary.site {
            return Err(rejected(
                "phase emission substituted an original event site",
            ));
        }
        let events = self
            .query
            .plan()
            .plan()
            .resolved_events(block)
            .ok_or_else(|| rejected("phase emission selected an unreachable event"))?;
        let mut lo = 0usize;
        let mut hi = events.len();
        while lo < hi {
            spend(work, 1)?;
            let middle = lo + (hi - lo) / 2;
            match events[middle].0.cmp(&boundary.event) {
                std::cmp::Ordering::Less => lo = middle + 1,
                std::cmp::Ordering::Greater => hi = middle,
                std::cmp::Ordering::Equal => {
                    let same = match (boundary.kind, events[middle].1) {
                        (
                            PhaseBoundaryKindV1::Define,
                            SsaResolvedEventV1::Define { variable, value },
                        )
                        | (PhaseBoundaryKindV1::Use, SsaResolvedEventV1::Use { variable, value })
                        | (
                            PhaseBoundaryKindV1::Kill,
                            SsaResolvedEventV1::Kill {
                                variable,
                                previous: Some(value),
                            },
                        ) => variable == boundary.variable && value == boundary.value,
                        _ => false,
                    };
                    return if same {
                        Ok(())
                    } else {
                        Err(rejected("phase emission changed the exact SSA event"))
                    };
                }
            }
        }
        Err(rejected("phase emission event is absent"))
    }

    fn check_action(&self, row: PhaseEmissionRowV1, work: &mut usize) -> PhaseResult<()> {
        let phase = self
            .input
            .phases
            .get(row.phase)
            .ok_or_else(|| rejected("phase emission row is outside its phase roster"))?;
        let (kind, value, instance) = match row.action {
            PhaseEmissionActionV1::OwnerConvert => (
                PhaseBoundaryKindV1::Define,
                phase.converted_owner,
                Some(phase.owner),
            ),
            PhaseEmissionActionV1::Begin => (
                PhaseBoundaryKindV1::Define,
                phase.issued_phase,
                Some(phase.issue),
            ),
            PhaseEmissionActionV1::Seal => (
                PhaseBoundaryKindV1::Define,
                phase.completion,
                Some(phase.finish),
            ),
            PhaseEmissionActionV1::RelayClosure => (PhaseBoundaryKindV1::Define, phase.relay, None),
            PhaseEmissionActionV1::RelayDrop => (PhaseBoundaryKindV1::Use, phase.relay, None),
            PhaseEmissionActionV1::End => (PhaseBoundaryKindV1::Kill, phase.relay, None),
            PhaseEmissionActionV1::Bind { lease }
            | PhaseEmissionActionV1::CloseStorage { lease } => {
                let input = phase.leases.get(lease).ok_or_else(|| {
                    rejected("phase emission row is outside its allocation roster")
                })?;
                let defining = matches!(row.action, PhaseEmissionActionV1::Bind { .. });
                (
                    if defining {
                        PhaseBoundaryKindV1::Define
                    } else {
                        PhaseBoundaryKindV1::Kill
                    },
                    input.result,
                    defining.then_some(input.bind),
                )
            }
        };
        if row.boundary.kind != kind || row.boundary.value != value {
            return Err(rejected(
                "phase emission action changed its checked source value",
            ));
        }
        let mut same = 0usize;
        for candidate in &self.input.rows {
            spend(work, 1)?;
            same += usize::from(candidate.phase == row.phase && candidate.action == row.action);
        }
        if same != 1 {
            return Err(rejected("phase emission duplicated a lifecycle action"));
        }
        if let Some(instance) = instance {
            let binding = self.binding(instance, work)?;
            let view = self.owner.execution_view_for_root(self.input.root).unwrap();
            let origin = &view.block_origins()[row.boundary.site.block().index() as usize];
            let statement = row
                .boundary
                .site
                .statement()
                .ok_or_else(|| rejected("phase result is not an original return transfer"))?;
            if row.boundary.variable.get() != binding.destination().local().index()
                || origin.instance() != binding.callee_instance()
                || origin.function() != binding.contract().function()
                || origin.terminator()
                    != (SemanticExpandedTerminatorOriginV1::CallReturn {
                        callee: binding.callee_instance(),
                    })
                || origin.statements().get(statement as usize)
                    != Some(
                        &fe2o3_mir_model::SemanticExpandedStatementOriginV1::ReturnTransfer {
                            callee: binding.callee_instance(),
                        },
                    )
                || !matches!(view.body().blocks()[row.boundary.site.block().index() as usize]
                    .statements().get(statement as usize).map(|statement| statement.kind()),
                    Some(SemanticStatementKindV1::Assign(assignment))
                        if assignment.destination() == binding.destination()
                            && assignment.value().result_type() == binding.destination().ty()
                            && matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place))
                                if place.local() == binding.callee_return()
                                    && place.projections().is_empty()
                                    && place.ty() == binding.destination().ty()))
            {
                return Err(rejected(
                    "phase emission substituted its original normal return",
                ));
            }
        }
        match row.action {
            PhaseEmissionActionV1::RelayClosure
            | PhaseEmissionActionV1::RelayDrop
            | PhaseEmissionActionV1::End => {
                let mut found = None;
                for relay in self.query.plan().defined_reusable_phase_relays() {
                    spend(work, 1)?;
                    if relay.wrapper() == phase.wrapper {
                        if found.replace(relay).is_some() {
                            return Err(rejected("phase emission relay is ambiguous"));
                        }
                    }
                }
                let relay =
                    found.ok_or_else(|| rejected("phase emission lost its original relay"))?;
                let expected_site = match row.action {
                    PhaseEmissionActionV1::RelayClosure => {
                        fe2o3_pliron::ProductionSemanticSsaSourceSiteV1::new(
                            relay.pack_block(),
                            Some(relay.pack_statement()),
                        )
                    }
                    _ => fe2o3_pliron::ProductionSemanticSsaSourceSiteV1::new(
                        relay.drop_block(),
                        None,
                    ),
                };
                if relay.closure() != phase.closure
                    || relay.finish() != phase.finish
                    || row.boundary.variable != relay.variable()
                    || row.boundary.site != expected_site
                    || self.binding(phase.wrapper, work)?.contract()
                        != Defined::ReusablePhase(relay.record())
                {
                    return Err(rejected(
                        "phase emission substituted a completion relay occurrence",
                    ));
                }
            }
            PhaseEmissionActionV1::CloseStorage { lease } => {
                let bind = self.binding(phase.leases[lease].bind, work)?;
                if row.boundary.variable.get() != bind.destination().local().index() {
                    return Err(rejected("phase emission closed a different source lease"));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn row_key(row: PhaseEmissionRowV1) -> (u32, u32) {
    (row.boundary.site.block().index(), row.boundary.event)
}
