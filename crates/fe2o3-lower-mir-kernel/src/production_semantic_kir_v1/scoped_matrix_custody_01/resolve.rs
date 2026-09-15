use super::super::borrowed_workgroup_01::{
    EpochOccurrence, Origin as Workgroup, WorkgroupSourceResolverV1,
    checked_execution_source_call_v1,
};
use super::super::numerical_policy_math_custody_01::{
    PolicySourceRequirementV1, checked_policy_source_v1, context_source_reference_kind_v1,
};
use super::*;

mod borrow_failure_observation {
    include!("borrow_failure_observation.rs");
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Role {
    Narrow,
    Bound,
    Matrix,
    Subgroup,
    Partition,
    Lane,
    Policy,
    Context,
}

// Each route retains its real source contract. A bound BF16 use must never
// acquire a synthetic gfx950 projection or satisfy a Narrow consumer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Requirement {
    Bound(SemanticPolicyMatrixBindV1),
    Narrow(SemanticPolicyGfx950NarrowV1),
    Transpose(SemanticExecutionCapabilityContractV1),
}

include!("scope_requirement.rs");
include!("partition_source.rs");

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Origin {
    pub(super) issuer: SsaValueV1,
    pub(super) context: SsaValueV1,
    pub(super) matrix: Option<SsaValueV1>,
    pub(super) subgroup: Option<SsaValueV1>,
    pub(super) policy: Option<SsaValueV1>,
    pub(super) bind: Option<Site>,
    pub(super) narrow: Option<Site>,
    pub(super) workgroup: Option<Workgroup>,
    pub(super) loans: BTreeSet<Loan>,
}

impl Origin {
    fn same_owner(&self, other: &Self) -> bool {
        self.issuer == other.issuer
            && self.context == other.context
            && self.matrix == other.matrix
            && self.subgroup == other.subgroup
            && self.policy == other.policy
            && self.bind == other.bind
            && self.narrow == other.narrow
            && self
                .workgroup
                .as_ref()
                .map(|w| (w.issuer, w.contract, w.epoch_projection))
                == other
                    .workgroup
                    .as_ref()
                    .map(|w| (w.issuer, w.contract, w.epoch_projection))
    }
}

pub(super) struct Resolver<'a, 'g> {
    pub(super) owner: &'a ProductionSemanticSsaOwnerV1,
    pub(super) view: &'a SemanticExpandedRootV1,
    pub(super) context: &'g RootKernelContextLoweringV1,
    pub(super) graph: &'g mut Graph<'a>,
    pub(super) occurrences: &'g occurrences::Occurrences,
    pub(super) epochs: &'g [EpochOccurrence],
    pub(super) requirement: Requirement,
}

impl<'a, 'g> Resolver<'a, 'g> {
    fn types(&self) -> &[SemanticTypeDeclV1] {
        self.owner.source_semantic().types()
    }

    pub(super) fn operand(
        &mut self,
        block: u32,
        operand: &SemanticOperandV1,
        role: Role,
    ) -> Result<Origin> {
        self.operand_path(block, operand, &[], operand.ty(), role, 0)
    }

    fn operand_path(
        &mut self,
        block: u32,
        operand: &SemanticOperandV1,
        tail: &[SemanticProjectionV1],
        target: SemanticTypeIdV1,
        role: Role,
        depth: usize,
    ) -> Result<Origin> {
        let (SemanticOperandV1::Copy(p) | SemanticOperandV1::Move(p)) = operand else {
            return Err(reject(
                "scoped Matrix custody cannot originate from a constant",
            ));
        };
        let count = p
            .projections()
            .len()
            .checked_add(tail.len())
            .ok_or_else(mismatch)?;
        if count > 16 {
            return Err(reject(
                "scoped Matrix capture path exceeds its closed limit",
            ));
        }
        self.graph.charge(count)?;
        let mut path = p.projections().to_vec();
        path.extend_from_slice(tail);
        let value = self.graph.use_value(block, p.local().index())?;
        self.value(value, &path, target, role, depth + 1)
    }

    pub(super) fn value(
        &mut self,
        value: SsaValueV1,
        path: &[SemanticProjectionV1],
        target: SemanticTypeIdV1,
        role: Role,
        depth: usize,
    ) -> Result<Origin> {
        self.graph.charge(1 + path.len())?;
        if depth > 128 || path.len() > 16 {
            return Err(reject("scoped Matrix custody path is cyclic or too deep"));
        }
        if let SsaValueV1::BlockArgument { block, variable } = value {
            let mut result: Option<Origin> = None;
            for incoming in self.graph.incoming(block.get(), variable.get())? {
                let candidate = self.value(incoming, path, target, role, depth + 1)?;
                if let Some(previous) = &mut result {
                    if !previous.same_owner(&candidate) {
                        return Err(reject(
                            "scoped Matrix join has distinct current SSA issuers",
                        ));
                    }
                    self.graph.charge(candidate.loans.len())?;
                    previous.loans.extend(candidate.loans);
                    if let (Some(a), Some(b)) = (&mut previous.workgroup, candidate.workgroup) {
                        self.graph.charge(b.loans.len())?;
                        a.loans.extend(b.loans);
                    }
                } else {
                    result = Some(candidate);
                }
            }
            return result
                .ok_or_else(|| reject("scoped Matrix join has no live incoming definition"));
        }
        let site = self.graph.definition(value)?;
        let local_type = self
            .graph
            .body
            .locals()
            .get(site.local as usize)
            .ok_or_else(mismatch)?
            .ty();
        check_path(self.types(), local_type, path, target)?;
        if path.is_empty() && matches!(role, Role::Policy | Role::Context) {
            let owned = if role == Role::Policy {
                self.requirement.bind_types()?.capability
            } else {
                self.context.semantic_type
            };
            if local_type == owned
                || context_reference(self.types(), local_type, owned, role).is_some()
            {
                let result = if role == Role::Context {
                    super::super::numerical_policy_math_custody_01::checked_context_source_v1(
                        self.owner, self.context, self.graph, self.requirement.source_scope()?.0,
                        value, (local_type != owned).then_some(local_type),
                    )?
                } else {
                    checked_policy_source_v1(
                        self.owner, self.context, self.graph, self.requirement.policy()?,
                        value, (local_type != owned).then_some(local_type), false,
                    )?
                };
                return Ok(Origin {
                    issuer: result.issuer,
                    context: result.context,
                    matrix: None,
                    subgroup: None,
                    policy: (role == Role::Policy).then_some(result.issuer),
                    bind: None,
                    narrow: None,
                    workgroup: None,
                    loans: result.loans,
                });
            }
        }
        if let Some(statement) = site.statement {
            let view = self.view;
            let a = occurrences::assignment(view.body(), site)?;
            if !a.destination().projections().is_empty() {
                return Err(reject("scoped Matrix definition is a projected write"));
            }
            match a.value().kind() {
                SemanticRvalueKindV1::Use(operand) => {
                    self.operand_path(site.block, operand, path, target, role, depth + 1)
                }
                SemanticRvalueKindV1::Aggregate(aggregate) => {
                    if path.is_empty() {
                        return self.constructor(value, site, role, depth + 1);
                    }
                    if !matches!(
                        aggregate.kind(),
                        SemanticAggregateKindV1::Aggregate | SemanticAggregateKindV1::Tuple
                    ) || aggregate.operands().len() > 16
                    {
                        return Err(reject(
                            "scoped Matrix capture requires an initialized bounded field aggregate",
                        ));
                    }
                    let Some((field, rest)) = path.split_first() else {
                        return Err(mismatch());
                    };
                    let SemanticProjectionKindV1::Field(index) = field.kind() else {
                        return Err(reject(
                            "scoped Matrix aggregate path is not a retained field",
                        ));
                    };
                    let input = aggregate
                        .operands()
                        .get(index as usize)
                        .ok_or_else(mismatch)?;
                    if input.ty() != field.result_type() {
                        return Err(mismatch());
                    }
                    self.operand_path(site.block, input, rest, target, role, depth + 1)
                }
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place,
                } => {
                    let pointee = shared_pointee(self.types(), a.value().result_type())
                        .ok_or_else(|| {
                            reject("scoped Matrix transport lost its exact shared reference")
                        })?;
                    if pointee != place.ty() {
                        return Err(mismatch());
                    }
                    let (rest, next_type) = if path.is_empty() {
                        (&[][..], pointee)
                    } else {
                        let (first, rest) = path.split_first().ok_or_else(mismatch)?;
                        if first.kind() != SemanticProjectionKindV1::Dereference
                            || first.result_type() != pointee
                        {
                            return Err(mismatch());
                        }
                        (rest, target)
                    };
                    self.graph.charge(1 + place.projections().len())?;
                    let count = place.projections().len().checked_add(rest.len()).ok_or_else(mismatch)?;
                    if count > 16 {
                        return Err(reject("scoped Matrix capture path exceeds its closed limit"));
                    }
                    self.graph.charge(count)?;
                    let mut path = place.projections().to_vec();
                    path.extend_from_slice(rest);
                    let source = self.borrow_place_value(site, place, role)?;
                    let mut result = self.value(source, &path, next_type, role, depth + 2)?;
                    let source_type = self.graph.body.locals()[place.local().index() as usize].ty();
                    // Reference forwarding does not renew or replace its referent loan.
                    if shared_pointee(self.types(), source_type).is_none() {
                        self.graph.charge(1)?;
                        result.loans.insert(Loan {
                            borrow: Site {
                                statement: Some(statement),
                                ..site
                            },
                            owner_local: place.local().index(),
                            owner_value: source,
                        });
                    }
                    Ok(result)
                }
                _ => Err(reject(
                    "scoped Matrix transport has no checked initialized source value",
                )),
            }
        } else {
            self.terminal(value, site, path, target, role, depth + 1)
        }
    }

    fn borrow_place_value(&mut self, site: Site, place: &'a SemanticPlaceV1, role: Role) -> Result<SsaValueV1> {
        use fe2o3_pliron::{ProductionSemanticSsaSourceQueryErrorV1 as Error, ProductionSemanticSsaSourceSiteV1};
        let query = self.owner.source_query_for_root(self.view.root(), self.view.body()).map_err(|_| mismatch())?;
        let mut work_error = None;
        let result = query.borrow_place_use(
            ProductionSemanticSsaSourceSiteV1::new(SemanticBlockIdV1::from_index(site.block), site.statement),
            place,
            &mut || match self.graph.charge(1) {
                Ok(()) => true,
                Err(error) => { work_error = Some(error); false }
            },
        );
        if let Some(error) = work_error { return Err(error); }
        result.map_err(|error| match error {
            Error::NoPromotedUse => {
                borrow_failure_observation::emit(borrow_failure_observation::Observation {
                    body: *self.view.body().identity().as_bytes(),
                    root: self.view.root().index(),
                    block: site.block,
                    statement: site.statement,
                    local: place.local().index(),
                    projections: place.projections().len(),
                    role,
                    owner_type: self.view.body().locals().get(place.local().index() as usize)
                        .map(|local| local.ty().index()),
                    // Planner roster is built by ascending local enumeration.
                    // Failure-only metadata: no second query or budget change.
                    promoted: query.plan().plan().promoted_variables()
                        .binary_search(&fe2o3_mir_model::SsaVariableIdV1::new(place.local().index())).is_ok(),
                    source_block: self.view.block_origins().get(site.block as usize)
                        .map(|origin| (origin.instance().index(), origin.function().index(), origin.block().index())),
                    source_local: self.view.local_origins().get(place.local().index() as usize)
                        .map(|origin| (origin.instance().index(), origin.function().index(), origin.local().index())),
                });
                reject("capability reference has no exact SSA use")
            }
            Error::DisagreeingUses => reject("scoped Matrix Borrow has disagreeing source SSA uses"),
            _ => mismatch(),
        })
    }

    fn constructor(
        &mut self,
        value: SsaValueV1,
        site: Site,
        role: Role,
        depth: usize,
    ) -> Result<Origin> {
        match role {
            Role::Bound => {
                let occurrences = self.occurrences;
                let binding = occurrences.binds.get(&site).ok_or_else(|| {
                    reject("scoped Matrix wrapper has no checked bind occurrence")
                })?;
                if binding.record.types() != self.requirement.bind_types()?
                    || binding.record.identity() != self.requirement.identity()?
                {
                    return Err(mismatch());
                }
                let view = self.view;
                let a = occurrences::assignment(view.body(), site)?;
                let SemanticRvalueKindV1::Aggregate(aggregate) = a.value().kind() else {
                    return Err(mismatch());
                };
                let inputs = &aggregate.operands()[..2];
                let mut matrix = self.operand_path(
                    site.block,
                    &inputs[0],
                    &[],
                    inputs[0].ty(),
                    Role::Matrix,
                    depth + 1,
                )?;
                let policy = self.operand_path(
                    site.block,
                    &inputs[1],
                    &[],
                    inputs[1].ty(),
                    Role::Policy,
                    depth + 1,
                )?;
                if matrix.context != policy.context {
                    return Err(reject(
                        "scoped Matrix and Policy have different Context SSA issuers",
                    ));
                }
                // Check both formal and original actual arguments, not just the constructor recipe.
                for (input, role, expected) in [
                    (&binding.binding.arguments()[0], Role::Matrix, &matrix),
                    (&binding.binding.arguments()[1], Role::Policy, &policy),
                ] {
                    let actual = self.operand_path(
                        binding.binding.expanded_call_block().index(),
                        input,
                        &[],
                        input.ty(),
                        role,
                        depth + 1,
                    )?;
                    if !actual.same_owner(expected) {
                        return Err(reject("scoped Matrix bind substituted an actual parameter"));
                    }
                }
                self.graph.charge(policy.loans.len())?;
                matrix.loans.extend(policy.loans);
                self.live(&matrix, site)?;
                matrix.issuer = value;
                matrix.policy = Some(policy.issuer);
                matrix.bind = Some(site);
                Ok(matrix)
            }
            Role::Narrow => {
                let occurrences = self.occurrences;
                let binding = occurrences.narrows.get(&site).ok_or_else(|| {
                    reject("scoped Matrix wrapper has no checked narrowing occurrence")
                })?;
                if self.requirement != Requirement::Narrow(binding.record) {
                    return Err(mismatch());
                }
                let view = self.view;
                let a = occurrences::assignment(view.body(), site)?;
                let SemanticRvalueKindV1::Aggregate(aggregate) = a.value().kind() else {
                    return Err(mismatch());
                };
                let input = &aggregate.operands()[0];
                let mut bound =
                    self.operand_path(site.block, input, &[], input.ty(), Role::Bound, depth + 1)?;
                let actual = &binding.binding.arguments()[0];
                let original = self.operand_path(
                    binding.binding.expanded_call_block().index(),
                    actual,
                    &[],
                    actual.ty(),
                    Role::Bound,
                    depth + 1,
                )?;
                if !bound.same_owner(&original) {
                    return Err(reject(
                        "scoped Matrix narrowing substituted its actual parameter",
                    ));
                }
                self.live(&bound, site)?;
                bound.issuer = value;
                bound.narrow = Some(site);
                Ok(bound)
            }
            _ => Err(reject(
                "an aggregate or ZST cannot issue scoped Matrix authority",
            )),
        }
    }

    fn execution(
        &self,
        block: u32,
    ) -> Result<(
        &'a SemanticDirectCallV1,
        SemanticExecutionCapabilityContractV1,
    )> {
        let (call, contract) =
            checked_execution_source_call_v1(self.owner, self.view, self.context, block)?;
        let (provenance, brand, epoch) = self.requirement.source_scope()?;
        if contract.provenance() != provenance
            || contract.workgroup_brand() != Some(brand)
            || contract.epoch_before() != Some(epoch)
            || contract.epoch_after().is_some()
            || !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable)
        {
            return Err(reject(
                "scoped Matrix source changed its exact root, execution brand or epoch",
            ));
        }
        Ok((call, contract))
    }

    fn terminal(
        &mut self,
        value: SsaValueV1,
        site: Site,
        path: &[SemanticProjectionV1],
        target: SemanticTypeIdV1,
        role: Role,
        depth: usize,
    ) -> Result<Origin> {
        if role == Role::Partition {
            return self.partition_source(value, site, path, target, depth);
        }
        let (call, contract) = self.execution(site.block)?;
        match (role, contract.operation()) {
            (
                Role::Matrix,
                SemanticExecutionCapabilityOperationV1::MatrixAccess {
                    subgroup,
                    epoch,
                    matrix,
                    subgroup_brand,
                    width,
                },
            ) if path.is_empty()
                && target == matrix
                && matrix == self.requirement.bind_types()?.matrix
                && width == 64
                && subgroup_brand == self.requirement.identity()?.matrix_brand() =>
            {
                let [subgroup_arg, epoch_arg] = call.arguments() else {
                    return Err(mismatch());
                };
                if subgroup_arg.ty() != subgroup || epoch_arg.ty() != epoch {
                    return Err(mismatch());
                }
                let mut result = self.operand_path(
                    site.block,
                    subgroup_arg,
                    &[],
                    subgroup,
                    Role::Subgroup,
                    depth + 1,
                )?;
                let workgroup_issuer = result.workgroup.as_ref().ok_or_else(mismatch)?.issuer;
                let subgroup_site = self
                    .graph
                    .definition(result.subgroup.ok_or_else(mismatch)?)?;
                let (_, subgroup_contract) = self.execution(subgroup_site.block)?;
                let epoch_origin = self.workgroups().resolve_operand(
                    site.block,
                    epoch_arg,
                    &[],
                    subgroup_contract,
                    depth + 1,
                )?;
                self.workgroups().check_live(&epoch_origin, site.block)?;
                if epoch_origin.issuer != workgroup_issuer || !epoch_origin.epoch_projection {
                    return Err(reject(
                        "scoped Matrix epoch and subgroup name different Workgroup SSA issuers",
                    ));
                }
                self.graph.charge(epoch_origin.loans.len())?;
                result.loans.extend(epoch_origin.loans);
                self.live(&result, site)?;
                result.issuer = value;
                result.matrix = Some(value);
                Ok(result)
            }
            (
                Role::Subgroup | Role::Lane,
                SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed {
                    workgroup_reference,
                    workgroup,
                    subgroup,
                    width,
                },
            ) if width == 64 => {
                if role == Role::Subgroup {
                    if !path.is_empty() || target != subgroup {
                        return Err(mismatch());
                    }
                } else {
                    // The lane is a field of this actual initialized subgroup issuer,
                    // not a type-compatible local or synthesized WaveLaneCurrent.
                    if !matches!(path, [field] if field.kind() == SemanticProjectionKindV1::Field(0)
                        && field.result_type() == target)
                        || aggregate_fields(self.types(), subgroup)
                            .and_then(|f| f.first())
                            .copied()
                            != Some(target)
                    {
                        return Err(reject(
                            "scoped lane is not the exact retained subgroup lane field",
                        ));
                    }
                }
                if shared_pointee(self.types(), workgroup_reference) != Some(workgroup) {
                    return Err(mismatch());
                }
                let [receiver] = call.arguments() else {
                    return Err(mismatch());
                };
                let owned = self.workgroups().resolve_operand(
                    site.block,
                    receiver,
                    &[],
                    contract,
                    depth + 1,
                )?;
                if owned.epoch_projection || owned.loans.is_empty() {
                    return Err(reject("scoped subgroup lacks its actual Workgroup loan"));
                }
                self.workgroups().check_live(&owned, site.block)?;
                let issuer_site = self.graph.definition(owned.issuer)?;
                let (issuer, workgroup_contract) = self.execution(issuer_site.block)?;
                let SemanticExecutionCapabilityOperationV1::WorkgroupDerive {
                    context,
                    workgroup: output,
                } = workgroup_contract.operation()
                else {
                    return Err(mismatch());
                };
                if output != workgroup
                    || issuer.arguments().len() != 1
                    || issuer.arguments()[0].ty() != context
                {
                    return Err(mismatch());
                }
                let context_origin = self.operand_path(
                    issuer_site.block,
                    &issuer.arguments()[0],
                    &[],
                    context,
                    Role::Context,
                    depth + 1,
                )?;
                self.live(&context_origin, issuer_site)?;
                Ok(Origin {
                    issuer: value,
                    context: context_origin.context,
                    matrix: None,
                    subgroup: Some(value),
                    policy: None,
                    bind: None,
                    narrow: None,
                    loans: subgroup_loans(self.graph, context_origin.loans, &owned.loans)?,
                    workgroup: Some(owned),
                })
            }
            _ => Err(reject(
                "scoped Matrix authority has no matching checked source issuer",
            )),
        }
    }

    fn workgroups<'s>(&'s mut self) -> WorkgroupSourceResolverV1<'a, 's> {
        WorkgroupSourceResolverV1 {
            owner: self.owner,
            view: self.view,
            context: self.context,
            graph: self.graph,
            epochs: self.epochs,
        }
    }

    pub(super) fn live(&mut self, origin: &Origin, consumer: Site) -> Result<()> {
        for loan in &origin.loans {
            self.graph.loan_live(*loan, consumer)?;
        }
        if let Some(workgroup) = &origin.workgroup {
            self.workgroups().check_live(workgroup, consumer.block)?;
        }
        Ok(())
    }
}

// WorkgroupDerive borrows Context for the whole workgroup lifetime. Keeping
// only the Workgroup loan would lose invalidations of that original Context.
pub(super) fn subgroup_loans(
    graph: &mut Graph<'_>,
    mut context: BTreeSet<Loan>,
    workgroup: &[Loan],
) -> Result<BTreeSet<Loan>> {
    // Existing child convention: one logical work unit per visited loan.
    // This is not a BTreeSet allocator-byte or physical CPU-time bound.
    graph.charge(workgroup.len())?;
    context.extend(workgroup.iter().copied());
    Ok(context)
}

fn context_reference(
    types: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
    context: SemanticTypeIdV1,
    role: Role,
) -> Option<SemanticBorrowKindV1> {
    if role != Role::Context {
        return None;
    }
    context_source_reference_kind_v1(types, reference, context)
}

pub(super) use super::super::capability_ssa_graph_01::shared_source_path::{
    aggregate_fields, shared_pointee,
};

fn check_path(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    path: &[SemanticProjectionV1],
    target: SemanticTypeIdV1,
) -> Result<()> {
    use super::super::capability_ssa_graph_01::shared_source_path::{self, PathError};
    shared_source_path::check_path(types, ty, path, target).map_err(|error| match error {
        PathError::Type => mismatch(),
        PathError::Projection => reject(
            "scoped Matrix path is not an exact shared reference and field path",
        ),
    })
}

#[cfg(test)]
mod transport_regressions {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::*;
    include!("transport_regression_tests.rs");
}
