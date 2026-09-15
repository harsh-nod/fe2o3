use super::capability_ssa_graph_01::{
    CapabilityDefinitionSiteV1, CapabilityLoanV1, CapabilitySsaGraphV1,
};
use super::numerical_policy_math_occurrences_01::{MathOccurrencesV1, MathStatementSiteV1};
use super::*;
use fe2o3_mir_model::SemanticExpandedTerminatorOriginV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Role {
    Context,
    Math,
    Policy,
    Bound,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MathOwnerV1 {
    pub(super) issuer: SsaValueV1,
    pub(super) context: SsaValueV1,
    pub(super) getter: Option<MathStatementSiteV1>,
    pub(super) bind: Option<MathStatementSiteV1>,
    pub(super) math: Option<SsaValueV1>,
    pub(super) policy: Option<SsaValueV1>,
    loans: BTreeSet<CapabilityLoanV1>,
}

impl MathOwnerV1 {
    fn same_owner(&self, other: &Self) -> bool {
        self.issuer == other.issuer
            && self.context == other.context
            && self.getter == other.getter
            && self.bind == other.bind
            && self.math == other.math
            && self.policy == other.policy
    }
}

/// A plan is built from the immutable replayed owner. No method accepts a
/// caller-selected SSA issuer or a type-indexed value as authority.
pub(super) struct MathCustodyPlanV1 {
    pub(super) occurrences: MathOccurrencesV1,
    pub(super) consumers: BTreeMap<SemanticBlockIdV1, MathOwnerV1>,
    pub(super) transports: BTreeMap<u32, MathTransportV1>,
    pub(super) references: BTreeMap<MathStatementSiteV1, MathReferenceFlowV1>,
    pub(super) receivers: BTreeMap<SemanticBlockIdV1, SsaValueV1>,
}

#[derive(Clone, Debug)]
pub(super) struct MathReferenceFlowV1 {
    pub(super) assignment: fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    pub(super) source_local: u32,
    pub(super) source: SsaValueV1,
    pub(super) issuer: SsaValueV1,
    pub(super) role: Role,
    pub(super) contract: SemanticNumericalPolicyMathContractV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MathTransportV1 {
    pub(super) contract: SemanticNumericalPolicyMathContractV1,
    pub(super) bound: bool,
    pub(super) capture: Option<MathCapturePathV1>,
}

impl MathTransportV1 {
    pub(super) fn semantic_type(self) -> SemanticTypeIdV1 {
        if let Some(path) = self.capture {
            return path.carrier;
        }
        if self.bound {
            self.contract.types().bound
        } else {
            self.contract.types().math
        }
    }

    fn same_binding(self, other: Self) -> bool {
        self.bound == other.bound
            && self.capture == other.capture
            && self.contract.types() == other.contract.types()
            && self.contract.policy() == other.contract.policy()
            && self.contract.kernel_brand() == other.contract.kernel_brand()
            && self.contract.provenance() == other.contract.provenance()
    }
}

impl MathCustodyPlanV1 {
    pub(super) fn new(
        owner: &ProductionSemanticSsaOwnerV1,
        context: &RootKernelContextLoweringV1,
        max_work: usize,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let occurrences = MathOccurrencesV1::new(owner, context, max_work / 2)?;
        let view = owner
            .execution_view_for_root(context.selected_root)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let ssa = owner
            .execution_plan_for_root(context.selected_root)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if ssa.function_identity() != view.body().identity() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let mut graph =
            CapabilitySsaGraphV1::new(view.body(), ssa.plan(), max_work - max_work / 2)?;
        let mut consumers = BTreeMap::new();
        let mut transports = BTreeMap::new();
        let mut references = BTreeMap::new();
        let mut receivers = BTreeMap::new();
        for (&block, occurrence) in &occurrences.consumers {
            let mut resolver = Resolver {
                owner,
                context,
                occurrences: &occurrences,
                graph: &mut graph,
                contract: occurrence.contract,
                transports: &mut transports,
                references: &mut references,
            };
            let bound = occurrence
                .call
                .arguments()
                .first()
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if !matches!(bound, SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) if place.projections().is_empty()) {
                return Err(reject("Math consumer requires its exact unprojected SSA receiver"));
            }
            let resolved = resolver.reference_operand(
                block.index(),
                bound,
                occurrence.contract.types().bound_reference,
                Role::Bound,
                0,
            )?;
            if resolved.bind.is_none()
                || resolved.getter.is_none()
                || resolved.math.is_none()
                || resolved.policy.is_none()
                || resolved.loans.is_empty()
            {
                return Err(reject(
                    "policy Math consumer lacks a retained constructor and shared owner loans",
                ));
            }
            let site = CapabilityDefinitionSiteV1 {
                block: block.index(),
                statement: None,
                local: 0,
            };
            for loan in &resolved.loans {
                resolver.graph.loan_live(*loan, site)?;
            }
            let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = bound else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            receivers.insert(block, resolver.graph.use_value(block.index(), place.local().index())?);
            consumers.insert(block, resolved);
        }
        Ok(Self {
            occurrences,
            consumers,
            transports,
            references,
            receivers,
        })
    }
}

include!("capture_custody.rs");
include!("context_reborrow.rs");
include!("policy_source.rs");

#[cfg(test)]
#[path = "capture_path_tests.rs"]
mod capture_path_tests;

struct Resolver<'a, 'g> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    context: &'a RootKernelContextLoweringV1,
    occurrences: &'a MathOccurrencesV1,
    graph: &'g mut CapabilitySsaGraphV1<'a>,
    contract: SemanticNumericalPolicyMathContractV1,
    transports: &'g mut BTreeMap<u32, MathTransportV1>,
    references: &'g mut BTreeMap<MathStatementSiteV1, MathReferenceFlowV1>,
}

impl Resolver<'_, '_> {
    fn retain_transport(
        &mut self,
        value: SsaValueV1,
        role: Role,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if !matches!(role, Role::Math | Role::Bound) {
            return Ok(());
        }
        let local = match value {
            SsaValueV1::BlockArgument { variable, .. } => variable.get(),
            _ => self.graph.definition(value)?.local,
        };
        let transport = MathTransportV1 {
            contract: self.contract,
            bound: role == Role::Bound,
            capture: None,
        };
        if let Some(old) = self.transports.get(&local) {
            if !old.same_binding(transport) {
                return Err(reject(
                    "one policy Math SSA local has conflicting nominal bindings",
                ));
            }
        } else {
            self.graph.charge(1)?;
            self.transports.insert(local, transport);
        }
        Ok(())
    }

    fn ty(&self, role: Role) -> SemanticTypeIdV1 {
        let types = self.contract.types();
        match role {
            Role::Context => self.context.semantic_type,
            Role::Math => types.math,
            Role::Policy => types.capability,
            Role::Bound => types.bound,
        }
    }

    fn step(&mut self, depth: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.graph.charge(1)?;
        if depth > 128 {
            return Err(reject("policy Math reference path is cyclic or too deep"));
        }
        Ok(())
    }

    fn merge(
        &mut self,
        value: SsaValueV1,
        role: Role,
        reference: Option<SemanticTypeIdV1>,
        depth: usize,
    ) -> Result<Option<MathOwnerV1>, ProductionSemanticKirErrorV1> {
        let SsaValueV1::BlockArgument { block, variable } = value else {
            return Ok(None);
        };
        let mut merged: Option<MathOwnerV1> = None;
        for incoming in self.graph.incoming(block.get(), variable.get())? {
            let candidate = match reference {
                Some(reference) => self.reference(incoming, reference, role, depth + 1)?,
                None => self.owned(incoming, role, depth + 1)?,
            };
            if let Some(previous) = &mut merged {
                if !previous.same_owner(&candidate) {
                    return Err(reject(
                        "policy Math SSA join has distinct issuer definitions",
                    ));
                }
                self.graph.charge(candidate.loans.len())?;
                previous.loans.extend(candidate.loans);
            } else {
                merged = Some(candidate);
            }
        }
        merged
            .map(Some)
            .ok_or_else(|| reject("policy Math SSA argument lacks an original owner"))
    }

    fn reference_operand(
        &mut self,
        block: u32,
        operand: &SemanticOperandV1,
        reference: SemanticTypeIdV1,
        role: Role,
        depth: usize,
    ) -> Result<MathOwnerV1, ProductionSemanticKirErrorV1> {
        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
            return Err(reject(
                "policy Math reference cannot originate from a constant",
            ));
        };
        if place.ty() != reference
            || !numerical_policy_math_shared_type_v1(
                self.owner.source_semantic().types(),
                reference,
                self.ty(role),
            )
        {
            return Err(reject(
                "policy Math reference lost its exact shared pointee edge",
            ));
        }
        if !place.projections().is_empty() {
            return self.captured_reference_operand(block, operand, &[], reference, role, depth + 1);
        }
        let value = self.graph.use_value(block, place.local().index())?;
        self.reference(value, reference, role, depth + 1)
    }

    fn reference(
        &mut self,
        value: SsaValueV1,
        reference: SemanticTypeIdV1,
        role: Role,
        depth: usize,
    ) -> Result<MathOwnerV1, ProductionSemanticKirErrorV1> {
        if matches!(role, Role::Policy | Role::Context) {
            return self.shared_source(value, Some(reference), role, depth);
        }
        self.step(depth)?;
        let reference_kind = math_reference_kind_v1(
            self.owner.source_semantic().types(), reference, self.ty(role), role,
        ).ok_or_else(|| reject("policy Math reference has no exact permitted pointee edge"))?;
        self.retain_transport(value, role)?;
        if let Some(merged) = self.merge(value, role, Some(reference), depth)? {
            return Ok(merged);
        }
        let site = self.graph.definition(value)?;
        let Some(statement) = site.statement else {
            return Err(reject(
                "policy Math shared reference was returned by an unmodeled terminal",
            ));
        };
        let SemanticStatementKindV1::Assign(assignment) =
            self.graph.body.blocks()[site.block as usize].statements()[statement as usize].kind()
        else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let assignment = assignment.clone();
        if assignment.destination().ty() != reference
            || assignment.value().result_type() != reference
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let resolved = match assignment.value().kind() {
            SemanticRvalueKindV1::Use(operand) => {
                if reference_kind == SemanticBorrowKindV1::Mutable {
                    // Only internal Context transport can reach an exclusive edge.
                    // Consumer operands still enter through the shared-only check.
                    let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
                        return Err(reject("Context reference transfer cannot originate from a constant"));
                    };
                    if role != Role::Context || place.ty() != reference || !place.projections().is_empty() {
                        return Err(reject("Context reference transfer changed its exact reference"));
                    }
                    let source = self.graph.use_value(site.block, place.local().index())?;
                    self.reference(source, reference, role, depth + 1)
                } else {
                    self.reference_operand(site.block, operand, reference, role, depth + 1)
                }
            }
            SemanticRvalueKindV1::Borrow {
                kind,
                place,
            } if *kind == reference_kind && place.ty() == self.ty(role) => {
                let source = self.graph.use_value(site.block, place.local().index())?;
                match place.projections() {
                    [] => {
                        let mut owner = self.owned(source, role, depth + 1)?;
                        owner.loans.insert(CapabilityLoanV1 {
                            borrow: site,
                            owner_local: place.local().index(),
                            owner_value: source,
                        });
                        Ok(owner)
                    }
                    [projection] if projection.kind() == SemanticProjectionKindV1::Dereference => {
                        let source_reference =
                            self.graph.body.locals()[place.local().index() as usize].ty();
                        let source_kind = math_reference_kind_v1(
                            self.owner.source_semantic().types(),
                            source_reference, self.ty(role), role,
                        ).ok_or_else(|| reject("policy Math reborrow is not an exact permitted reference"))?;
                        if !math_reborrow_kind_matches_v1(source_kind, reference_kind) {
                            return Err(reject("Context reborrow cannot strengthen a shared reference"));
                        }
                        self.reference(source, source_reference, role, depth + 1)
                    }
                    _ => Err(reject(
                        "policy Math shared borrow has an unsupported projection",
                    )),
                }
            }
            _ => Err(reject(
                "policy Math reference lacks a shared borrow, copy or checked transfer",
            )),
        }?;
        let source_local = match assignment.value().kind() {
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place))
            | SemanticRvalueKindV1::Borrow { place, .. } => place.local().index(),
            _ => return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch),
        };
        let flow = MathReferenceFlowV1 {
            source: self.graph.use_value(site.block, source_local)?, source_local,
            issuer: resolved.issuer, assignment, role, contract: self.contract,
        };
        let key = MathStatementSiteV1 { block: SemanticBlockIdV1::from_index(site.block), statement };
        if let Some(previous) = self.references.get(&key) {
            if previous.assignment != flow.assignment || previous.source != flow.source
                || previous.issuer != flow.issuer || previous.role != flow.role
                || !(MathTransportV1 { contract: previous.contract, bound: false, capture: None }).same_binding(
                    MathTransportV1 { contract: flow.contract, bound: false, capture: None }) {
                return Err(reject("Math shared reference has conflicting checked SSA custody"));
            }
        } else {
            self.graph.charge(1)?;
            self.references.insert(key, flow);
        }
        Ok(resolved)
    }

    fn owned_operand(
        &mut self,
        block: u32,
        operand: &SemanticOperandV1,
        role: Role,
        depth: usize,
    ) -> Result<MathOwnerV1, ProductionSemanticKirErrorV1> {
        let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
            return Err(reject(
                "policy Math authority cannot originate from a ZST or scalar constant",
            ));
        };
        if place.ty() != self.ty(role) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let source = self.graph.use_value(block, place.local().index())?;
        match place.projections() {
            [] => self.owned(source, role, depth + 1),
            [projection] if projection.kind() == SemanticProjectionKindV1::Dereference => {
                let reference = self.graph.body.locals()[place.local().index() as usize].ty();
                if !numerical_policy_math_shared_type_v1(
                    self.owner.source_semantic().types(),
                    reference,
                    self.ty(role),
                ) {
                    return Err(reject("policy Math dereference is not shared"));
                }
                self.reference(source, reference, role, depth + 1)
            }
            _ => Err(reject(
                "policy Math authority was reconstructed through an unsupported projection",
            )),
        }
    }

    fn owned(
        &mut self,
        value: SsaValueV1,
        role: Role,
        depth: usize,
    ) -> Result<MathOwnerV1, ProductionSemanticKirErrorV1> {
        if matches!(role, Role::Policy | Role::Context) {
            return self.shared_source(value, None, role, depth);
        }
        self.step(depth)?;
        self.retain_transport(value, role)?;
        if let Some(merged) = self.merge(value, role, None, depth)? {
            return Ok(merged);
        }
        let site = self.graph.definition(value)?;
        if let Some(statement) = site.statement {
            let key = MathStatementSiteV1 {
                block: SemanticBlockIdV1::from_index(site.block),
                statement,
            };
            if role == Role::Math
                && let Some(getter) = self.occurrences.getters.get(&key).cloned()
            {
                let record = getter.record;
                if record.types().math != self.ty(Role::Math)
                    || record.types().context != self.ty(Role::Context)
                    || record.kernel_brand() != self.contract.kernel_brand()
                    || record.provenance() != self.contract.provenance()
                    || getter.binding.destination().local().index() != site.local
                {
                    return Err(reject(
                        "policy Math getter does not match its consumer root and nominal brand",
                    ));
                }
                let context = self.reference_operand(
                    getter.binding.expanded_call_block().index(),
                    &getter.binding.arguments()[0],
                    record.types().context_reference,
                    Role::Context,
                    depth + 1,
                )?;
                let plan = self.owner.execution_plan_for_root(self.context.selected_root)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                let result = plan.defined_math_results().iter().find(|result|
                    result.getter() == getter.binding.callee_instance()
                    && result.bridge() == getter.bridge_instance
                    && result.block() == getter.bridge_return_site.block
                    && result.statement() == getter.bridge_return_site.statement
                    && result.getter_block() == key.block
                    && result.getter_statement() == key.statement)
                    .ok_or_else(|| reject("Math getter lacks its replayed bridge result"))?;
                let receiver = self.graph.use_value(result.block().index(), result.receiver().index())?;
                let formal = self.reference(receiver, record.types().context_reference, Role::Context, depth + 1)?;
                if !context.same_owner(&formal) {
                    return Err(reject("Math getter formal receiver changed its context issuer"));
                }
                for loan in &formal.loans {
                    self.graph.loan_live(*loan, CapabilityDefinitionSiteV1 {
                        block: result.block().index(), statement: Some(result.statement()),
                        local: result.receiver().index(),
                    })?;
                    self.graph.loan_live(*loan, site)?;
                }
                for loan in &context.loans {
                    self.graph.loan_live(*loan, site)?;
                }
                return Ok(MathOwnerV1 {
                    issuer: value,
                    context: context.issuer,
                    getter: Some(key),
                    bind: None,
                    math: None,
                    policy: None,
                    // The getter result retains the kernel brand, not &self.
                    loans: BTreeSet::new(),
                });
            }
            if role == Role::Bound
                && let Some(bind) = self.occurrences.binds.get(&key).cloned()
            {
                let record = bind.record;
                let expected = self.contract.types();
                if record.types().all()
                    != [
                        expected.math_reference,
                        expected.math,
                        expected.policy_reference,
                        expected.capability,
                        expected.bound,
                    ]
                    || record.policy() != self.contract.policy()
                    || record.kernel_brand() != self.contract.kernel_brand()
                    || record.provenance() != self.contract.provenance()
                    || bind.binding.callee_return().index() != site.local
                {
                    return Err(reject(
                        "policy Math constructor substituted its consumer contract",
                    ));
                }
                let SemanticStatementKindV1::Assign(assignment) =
                    self.graph.body.blocks()[site.block as usize].statements()[statement as usize]
                        .kind()
                else {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                };
                let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                };
                let arguments = aggregate.operands()[..2].to_vec();
                let math = self.reference_operand(
                    site.block,
                    &arguments[0],
                    expected.math_reference,
                    Role::Math,
                    depth + 1,
                )?;
                let policy = self.reference_operand(
                    site.block,
                    &arguments[1],
                    expected.policy_reference,
                    Role::Policy,
                    depth + 1,
                )?;
                if math.context != policy.context {
                    return Err(reject(
                        "policy Math constructor arguments originate from different context definitions",
                    ));
                }
                let mut loans = math.loans.clone();
                loans.extend(policy.loans);
                for loan in &loans {
                    self.graph.loan_live(*loan, site)?;
                }
                return Ok(MathOwnerV1 {
                    issuer: value,
                    context: math.context,
                    getter: math.getter,
                    bind: Some(key),
                    math: Some(math.issuer),
                    policy: Some(policy.issuer),
                    loans,
                });
            }
            let SemanticStatementKindV1::Assign(assignment) =
                self.graph.body.blocks()[site.block as usize].statements()[statement as usize]
                    .kind()
            else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            if assignment.destination().ty() != self.ty(role) {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() else {
                return Err(reject(
                    "policy Math authority has no authenticated original producer",
                ));
            };
            return self.owned_operand(site.block, &operand.clone(), role, depth + 1);
        }
        Err(reject("policy Math cannot derive branded authority from an unmodeled terminal"))
    }
}

fn reject(detail: &'static str) -> ProductionSemanticKirErrorV1 {
    unsupported(0, None, None, detail)
}
