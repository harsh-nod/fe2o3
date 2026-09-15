// Shared Policy/Context source resolution. No Math contract is synthesized.
#[derive(Clone, Copy)]
pub(super) struct PolicySourceRequirementV1 {
    capability: SemanticTypeIdV1,
    policy: SemanticTypeIdentityV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
}

impl PolicySourceRequirementV1 {
    fn math(contract: SemanticNumericalPolicyMathContractV1) -> Self {
        Self {
            capability: contract.types().capability,
            policy: contract.policy(),
            provenance: contract.provenance(),
        }
    }
    pub(super) fn matrix(
        record: fe2o3_mir_model::semantic_mir_v1::SemanticPolicyMatrixBindV1,
    ) -> Self {
        Self {
            capability: record.types().capability,
            policy: record.identity().policy(),
            provenance: record.identity().provenance(),
        }
    }
    pub(super) fn matrix_narrow(
        record: fe2o3_mir_model::semantic_mir_v1::SemanticPolicyGfx950NarrowV1,
    ) -> Self {
        Self {
            capability: record.types().bind.capability,
            policy: record.identity().policy(),
            provenance: record.identity().provenance(),
        }
    }
}

pub(super) struct PolicySourceResultV1 {
    pub(super) issuer: SsaValueV1,
    pub(super) context: SsaValueV1,
    pub(super) loans: BTreeSet<CapabilityLoanV1>,
}

// Classifies only the exact Context reference edge. Source resolution below
// still checks the actual producer, reborrow direction and retained loans.
pub(super) fn context_source_reference_kind_v1(
    types: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
    context: SemanticTypeIdV1,
) -> Option<SemanticBorrowKindV1> {
    math_reference_kind_v1(types, reference, context, Role::Context)
}

/// The caller resolves any exact capture path to its current reference SSA
/// value first. This helper retains the old rejection of projected captures.
pub(super) fn checked_policy_source_v1(
    owner: &ProductionSemanticSsaOwnerV1,
    context: &RootKernelContextLoweringV1,
    graph: &mut CapabilitySsaGraphV1<'_>,
    requirement: PolicySourceRequirementV1,
    value: SsaValueV1,
    reference: Option<SemanticTypeIdV1>,
    context_only: bool,
) -> Result<PolicySourceResultV1, ProductionSemanticKirErrorV1> {
    let role = if context_only {
        Role::Context
    } else {
        Role::Policy
    };
    let result = source_query(
        owner,
        context,
        graph,
        requirement,
        value,
        reference,
        role,
        0,
        None,
    )?;
    Ok(PolicySourceResultV1 {
        issuer: result.issuer,
        context: result.context,
        loans: result.loans,
    })
}

include!("context_source.rs");

fn source_query(
    owner: &ProductionSemanticSsaOwnerV1,
    context: &RootKernelContextLoweringV1,
    graph: &mut CapabilitySsaGraphV1<'_>,
    requirement: PolicySourceRequirementV1,
    value: SsaValueV1,
    reference: Option<SemanticTypeIdV1>,
    role: Role,
    depth: usize,
    references: Option<(
        &mut BTreeMap<MathStatementSiteV1, MathReferenceFlowV1>,
        SemanticNumericalPolicyMathContractV1,
    )>,
) -> Result<MathOwnerV1, ProductionSemanticKirErrorV1> {
    source_query_requirement(owner, context, graph, SourceRequirementV1::Policy(requirement),
        value, reference, role, depth, references)
}

fn source_query_requirement(
    owner: &ProductionSemanticSsaOwnerV1,
    context: &RootKernelContextLoweringV1,
    graph: &mut CapabilitySsaGraphV1<'_>,
    requirement: SourceRequirementV1,
    value: SsaValueV1,
    reference: Option<SemanticTypeIdV1>,
    role: Role,
    depth: usize,
    references: Option<(
        &mut BTreeMap<MathStatementSiteV1, MathReferenceFlowV1>,
        SemanticNumericalPolicyMathContractV1,
    )>,
) -> Result<MathOwnerV1, ProductionSemanticKirErrorV1> {
    if role == Role::Policy { requirement.policy()?; }
    let view = owner
        .execution_view_for_root(context.selected_root)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if !std::ptr::eq(graph.body, view.body())
        || !global_capability_provenance_matches_v1(context, requirement.provenance())
        || !matches!(role, Role::Policy | Role::Context)
    {
        return Err(reject(
            "Policy source query changed its owner, root or role",
        ));
    }
    let mut resolver = PolicySourceResolverV1 {
        owner,
        context,
        graph,
        requirement,
        references,
    };
    match reference {
        Some(reference) => resolver.reference(value, reference, role, depth),
        None => resolver.owned(value, role, depth),
    }
}

struct PolicySourceResolverV1<'a, 'g, 'body, 'r> {
    owner: &'a ProductionSemanticSsaOwnerV1,
    context: &'a RootKernelContextLoweringV1,
    graph: &'g mut CapabilitySsaGraphV1<'body>,
    requirement: SourceRequirementV1,
    references: Option<(
        &'r mut BTreeMap<MathStatementSiteV1, MathReferenceFlowV1>,
        SemanticNumericalPolicyMathContractV1,
    )>,
}

impl PolicySourceResolverV1<'_, '_, '_, '_> {
    fn ty(&self, role: Role) -> Result<SemanticTypeIdV1, ProductionSemanticKirErrorV1> {
        match role {
            Role::Context => Ok(self.context.semantic_type),
            Role::Policy => Ok(self.requirement.policy()?.capability),
            _ => Err(reject("source query requires the closed Policy/Context role")),
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
        reference: Option<SemanticTypeIdV1>,
        role: Role,
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
    fn reference(
        &mut self,
        value: SsaValueV1,
        reference: SemanticTypeIdV1,
        role: Role,
        depth: usize,
    ) -> Result<MathOwnerV1, ProductionSemanticKirErrorV1> {
        self.step(depth)?;
        let kind = math_reference_kind_v1(
            self.owner.source_semantic().types(),
            reference,
            self.ty(role)?,
            role,
        )
        .ok_or_else(|| reject("policy Math reference has no exact permitted pointee edge"))?;
        if let Some(result) = self.merge(value, Some(reference), role, depth)? {
            return Ok(result);
        }
        let site = self.graph.definition(value)?;
        let statement = site.statement.ok_or_else(|| {
            reject("policy Math shared reference was returned by an unmodeled terminal")
        })?;
        let SemanticStatementKindV1::Assign(a) =
            self.graph.body.blocks()[site.block as usize].statements()[statement as usize].kind()
        else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let a = a.clone();
        if a.destination().ty() != reference || a.value().result_type() != reference {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let (source_local, resolved) = match a.value().kind() {
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p))
                if p.ty() == reference && p.projections().is_empty() =>
            {
                let value = self.graph.use_value(site.block, p.local().index())?;
                (
                    p.local().index(),
                    self.reference(
                        value,
                        reference,
                        role,
                        depth
                            + if kind == SemanticBorrowKindV1::Mutable {
                                1
                            } else {
                                2
                            },
                    )?,
                )
            }
            SemanticRvalueKindV1::Borrow {
                kind: actual,
                place,
            } if *actual == kind && place.ty() == self.ty(role)? => {
                let source = self.graph.use_value(site.block, place.local().index())?;
                let resolved = match place.projections() {
                    [] => {
                        let mut result = self.owned(source, role, depth + 1)?;
                        result.loans.insert(CapabilityLoanV1 {
                            borrow: site,
                            owner_local: place.local().index(),
                            owner_value: source,
                        });
                        result
                    }
                    [p] if p.kind() == SemanticProjectionKindV1::Dereference => {
                        let source_reference =
                            self.graph.body.locals()[place.local().index() as usize].ty();
                        let source_kind = math_reference_kind_v1(
                            self.owner.source_semantic().types(),
                            source_reference,
                            self.ty(role)?,
                            role,
                        )
                        .ok_or_else(|| {
                            reject("policy Math reborrow is not an exact permitted reference")
                        })?;
                        if !math_reborrow_kind_matches_v1(source_kind, kind) {
                            return Err(reject(
                                "Context reborrow cannot strengthen a shared reference",
                            ));
                        }
                        self.reference(source, source_reference, role, depth + 1)?
                    }
                    _ => {
                        return Err(reject(
                            "policy Math shared borrow has an unsupported projection",
                        ));
                    }
                };
                (place.local().index(), resolved)
            }
            _ => {
                return Err(reject(
                    "policy Math reference lacks a shared borrow, copy or checked transfer",
                ));
            }
        };
        if let Some((references, contract)) = &mut self.references {
            let flow = MathReferenceFlowV1 {
                source: self.graph.use_value(site.block, source_local)?,
                source_local,
                issuer: resolved.issuer,
                role,
                assignment: a,
                contract: *contract,
            };
            let key = MathStatementSiteV1 {
                block: SemanticBlockIdV1::from_index(site.block),
                statement,
            };
            retain_source_reference_v1(self.graph, references, key, flow)?;
        }
        Ok(resolved)
    }
    fn owned(
        &mut self,
        value: SsaValueV1,
        role: Role,
        depth: usize,
    ) -> Result<MathOwnerV1, ProductionSemanticKirErrorV1> {
        self.step(depth)?;
        if let Some(result) = self.merge(value, None, role, depth)? {
            return Ok(result);
        }
        let site = self.graph.definition(value)?;
        if let Some(statement) = site.statement {
            if role == Role::Context
                && let Some(entry) = &self.context.entry_transfer
                && value == entry.parameter
            {
                if entry.context != self.ty(role)?
                    || entry.block.index() != site.block
                    || entry.statement != statement
                    || entry.destination.index() != site.local
                    || entry.issuer == value
                {
                    return Err(reject(
                        "policy Math Context entry lost its checked SSA relation",
                    ));
                }
                return self.owned(entry.issuer, role, depth + 1);
            }
            let SemanticStatementKindV1::Assign(a) = self.graph.body.blocks()[site.block as usize]
                .statements()[statement as usize]
                .kind()
            else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            if a.destination().ty() != self.ty(role)? {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p)) =
                a.value().kind()
            else {
                return Err(reject(
                    "policy Math authority has no authenticated original producer",
                ));
            };
            if p.ty() != self.ty(role)? {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            let source = self.graph.use_value(site.block, p.local().index())?;
            let source_reference = self.graph.body.locals()[p.local().index() as usize].ty();
            return match p.projections() {
                [] => self.owned(source, role, depth + 2),
                [p] if p.kind() == SemanticProjectionKindV1::Dereference => {
                    let reference = source_reference;
                    if !numerical_policy_math_shared_type_v1(
                        self.owner.source_semantic().types(),
                        reference,
                        self.ty(role)?,
                    ) {
                        return Err(reject("policy Math dereference is not shared"));
                    }
                    self.reference(source, reference, role, depth + 2)
                }
                _ => Err(reject(
                    "policy Math authority was reconstructed through an unsupported projection",
                )),
            };
        }
        let view = self
            .owner
            .execution_view_for_root(self.context.selected_root)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let origin = &view.block_origins()[site.block as usize];
        if origin.terminator() != SemanticExpandedTerminatorOriginV1::Source {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let SemanticTerminatorKindV1::Call(call) = self.graph.body.blocks()[site.block as usize]
            .terminator()
            .kind()
        else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let call = call.clone();
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            binding, operation, ..
        }) = self
            .owner
            .source_semantic()
            .callables()
            .get(call.callee().index() as usize)
        else {
            return Err(reject(
                "policy Math issuer is not a checked source intrinsic",
            ));
        };
        let context = match (role, *operation) {
            (
                Role::Context,
                SemanticCompilerIntrinsicOperationV1::KernelContextIssue { context },
            ) if context == self.context.semantic_type
                && call.arguments().is_empty()
                && call.destination().map(|d| d.place().ty()) == Some(context)
                && *self.owner.source_semantic().functions()
                    [origin.function().index() as usize]
                    .identity()
                    .as_bytes()
                    == self.context.source.function() =>
            {
                value
            }
            (
                Role::Policy,
                SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            ) => {
                let SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue {
                    context,
                    capability,
                    policy,
                } = contract.operation()
                else {
                    return Err(reject(
                        "policy Math policy referent is not numerical issuance",
                    ));
                };
                let requirement = self.requirement.policy()?;
                if capability != requirement.capability
                    || policy != requirement.policy
                    || contract.provenance() != requirement.provenance
                    || contract.source_identity() != binding.identity()
                    || call.arguments().len() != 1
                {
                    return Err(reject(
                        "policy Math policy issuer changed its source, policy or root",
                    ));
                }
                let (SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p)) =
                    &call.arguments()[0]
                else {
                    return Err(reject(
                        "policy Math reference cannot originate from a constant",
                    ));
                };
                if !p.projections().is_empty()
                    || p.ty() != context
                    || !numerical_policy_math_shared_type_v1(
                        self.owner.source_semantic().types(),
                        context,
                        self.context.semantic_type,
                    )
                {
                    return Err(reject(
                        "policy Math reference lost its exact shared pointee edge",
                    ));
                }
                let source = self.graph.use_value(site.block, p.local().index())?;
                let context = self.reference(source, context, Role::Context, depth + 2)?;
                for loan in &context.loans {
                    self.graph.loan_live(*loan, site)?;
                }
                context.issuer
            }
            _ => {
                return Err(reject(
                    "policy Math cannot derive branded authority from an unmodeled terminal",
                ));
            }
        };
        Ok(MathOwnerV1 {
            issuer: value,
            context,
            getter: None,
            bind: None,
            math: None,
            policy: None,
            loans: BTreeSet::new(),
        })
    }
}

fn retain_source_reference_v1(
    graph: &mut CapabilitySsaGraphV1<'_>,
    references: &mut BTreeMap<MathStatementSiteV1, MathReferenceFlowV1>,
    key: MathStatementSiteV1,
    flow: MathReferenceFlowV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if let Some(previous) = references.get(&key) {
        if previous.assignment != flow.assignment
            || previous.source != flow.source
            || previous.issuer != flow.issuer
            || previous.role != flow.role
            || !(MathTransportV1 {
                contract: previous.contract,
                bound: false,
                capture: None,
            })
            .same_binding(MathTransportV1 {
                contract: flow.contract,
                bound: false,
                capture: None,
            })
        {
            return Err(reject(
                "Math shared reference has conflicting checked SSA custody",
            ));
        }
    } else {
        graph.charge(1)?;
        references.insert(key, flow);
    }
    Ok(())
}

impl Resolver<'_, '_> {
    fn shared_source(
        &mut self,
        value: SsaValueV1,
        reference: Option<SemanticTypeIdV1>,
        role: Role,
        depth: usize,
    ) -> Result<MathOwnerV1, ProductionSemanticKirErrorV1> {
        source_query(
            self.owner,
            self.context,
            self.graph,
            PolicySourceRequirementV1::math(self.contract),
            value,
            reference,
            role,
            depth,
            Some((self.references, self.contract)),
        )
    }
}
