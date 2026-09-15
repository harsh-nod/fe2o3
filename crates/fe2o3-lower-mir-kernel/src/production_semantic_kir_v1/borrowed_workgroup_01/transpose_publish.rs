// Included inside borrowed_workgroup_01. The old inclusive check_live is
// unchanged. Success here is only a pre-Publish Workgroup lifetime predicate.
impl WorkgroupSourceResolverV1<'_, '_> {
    pub(super) fn before_transpose_publish(
        &mut self,
        origin: &Origin,
        consumer: u32,
        borrowed_at: u32,
    ) -> Result<()> {
        self.graph.charge(4)?;
        if !std::ptr::eq(self.graph.body, self.view.body())
            || self.view.root() != self.context.selected_root
            || self
                .owner
                .execution_view_for_root(self.view.root())
                .is_none_or(|view| !std::ptr::eq(view, self.view))
            || self
                .owner
                .execution_plan_for_root(self.view.root())
                .is_none_or(|plan| !std::ptr::eq(plan.plan(), self.graph.ssa))
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let (call, publish) =
            checked_execution_source_call_v1(self.owner, self.view, self.context, consumer)?;
        let SemanticExecutionCapabilityOperationV1::Gfx950Transpose(transpose) =
            publish.operation()
        else {
            return Err(reject("owned Workgroup endpoint is not transpose Publish"));
        };
        let fe2o3_mir_model::semantic_mir_v1::SemanticGfx950TransposeOperationV1::Publish {
            input_workgroup,
            ..
        } = transpose.operation()
        else {
            return Err(reject("owned Workgroup endpoint is not transpose Publish"));
        };
        let SemanticExecutionCapabilityOperationV1::WorkgroupDerive { workgroup, .. } =
            origin.contract.operation()
        else {
            return Err(reject(
                "transpose Workgroup origin is not its original derive",
            ));
        };
        if input_workgroup != workgroup
            || publish.provenance() != origin.contract.provenance()
            || publish.workgroup_brand() != origin.contract.workgroup_brand()
            || publish.epoch_before() != origin.contract.epoch_before()
            || publish.epoch_after().is_none()
            || !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable)
        {
            return Err(reject(
                "transpose Publish changed its Workgroup root or epoch",
            ));
        }
        let [_, receiver] = call.arguments() else {
            return Err(reject(
                "transpose Publish lost its exact owned Workgroup input",
            ));
        };
        if !matches!(receiver, SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
            if place.ty() == input_workgroup && place.projections().is_empty())
        {
            return Err(reject(
                "transpose Publish lost its exact owned Workgroup input",
            ));
        }
        let next = call
            .destination()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .edge()
            .target()
            .index();
        let before = transpose_before_publish_point(self.graph, consumer, next)?;

        let issuer = self.graph.definition(origin.issuer)?;
        if issuer.statement.is_some() || self.execution_call(issuer.block)?.1 != origin.contract {
            return Err(reject(
                "transpose Workgroup issuer changed its retained definition",
            ));
        }
        let region = self.graph.path_region(issuer.block, consumer)?;
        for block in 0..self.graph.body.blocks().len() {
            self.graph.charge(1)?;
            if !region[block] || block as u32 == consumer {
                continue;
            }
            let SemanticTerminatorKindV1::Call(call) =
                self.graph.body.blocks()[block].terminator().kind()
            else {
                continue;
            };
            let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation:
                    SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
                        contract: candidate,
                    },
                ..
            }) = self
                .owner
                .source_semantic()
                .callables()
                .get(call.callee().index() as usize)
            else {
                continue;
            };
            if candidate.provenance() == origin.contract.provenance()
                && candidate.workgroup_brand() == origin.contract.workgroup_brand()
                && candidate.epoch_before() == origin.contract.epoch_before()
                && candidate.epoch_after().is_some()
            {
                self.execution_call(block as u32)?;
                return Err(reject(
                    "transpose Workgroup crosses a prior matching epoch transition",
                ));
            }
        }
        // Empty loans do not skip the nonempty-cycle/epoch-transition checks.
        if origin.loans.is_empty() || origin.epoch_projection {
            return Err(reject(
                "transpose Publish lacks its retained Workgroup owner loans",
            ));
        }
        let (shared_call, shared_contract) =
            checked_execution_source_call_v1(self.owner, self.view, self.context, borrowed_at)?;
        if !matches!(shared_contract.operation(),
            SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed { workgroup: owned, .. }
            if owned == workgroup)
            || shared_contract.provenance() != origin.contract.provenance()
            || shared_contract.workgroup_brand() != origin.contract.workgroup_brand()
            || shared_contract.epoch_before() != origin.contract.epoch_before()
            || shared_contract.epoch_after().is_some()
        {
            return Err(reject(
                "transpose Publish lacks its exact earlier borrowed subgroup call",
            ));
        }
        let [shared_receiver] = shared_call.arguments() else {
            return Err(reject("transpose earlier shared source arity changed"));
        };
        let shared = self.resolve_operand(borrowed_at, shared_receiver, &[], shared_contract, 0)?;
        self.check_live(&shared, borrowed_at)?;
        let owned = self.resolve_operand(consumer, receiver, &[], shared_contract, 0)?;
        if owned.issuer != origin.issuer
            || owned.contract != origin.contract
            || shared.issuer != origin.issuer
            || shared.contract != origin.contract
            || shared.loans.is_empty()
            || shared.epoch_projection
            || owned.epoch_projection
            || !self.graph.reaches(borrowed_at, consumer)?
        {
            return Err(reject(
                "transpose Publish and shared source changed the Workgroup SSA issuer",
            ));
        }
        for loan in &shared.loans {
            self.graph.charge(origin.loans.len())?;
            if !origin.loans.contains(loan) {
                return Err(reject(
                    "transpose Publish omitted an original shared Workgroup loan",
                ));
            }
        }
        for &loan in origin.loans.iter().chain(&owned.loans) {
            self.graph.loan_live(loan, before)?;
        }
        Ok(())
    }
}

// The only production caller derives next from the authenticated Publish call.
// No caller-facing arbitrary endpoint or block-exclusion flag is exposed.
fn transpose_before_publish_point(graph: &mut Graph<'_>, consumer: u32, next: u32) -> Result<Site> {
    graph.charge(1)?;
    let end = u32::try_from(
        graph
            .body
            .blocks()
            .get(consumer as usize)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .statements()
            .len(),
    )
    .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    // A reflexive query on consumer would reject every call. Starting at its
    // actual normal successor detects exactly a nonempty return path, including
    // a self-loop, before the current static occurrence can be excluded.
    if graph.reaches(next, consumer)? {
        return Err(reject(
            "transpose Publish crosses a cycle without a proven epoch generation",
        ));
    }
    Ok(Site {
        block: consumer,
        statement: Some(end),
        local: 0,
    })
}
