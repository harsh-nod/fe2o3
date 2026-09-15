impl SemanticFunctionLoweringV1<'_> {
    pub(super) fn matrix_current_leaf(
        &self,
        block: SemanticBlockIdV1,
        context: SemanticTypeIdV1,
    ) -> Result<Option<SemanticValueBindingV1>, ProductionSemanticKirErrorV1> {
        let Some(plan) = &self.matrix_derives else {
            return Ok(None);
        };
        let mut matches = plan
            .bridges
            .values()
            .filter(|bridge| bridge.current_block == block.index());
        let Some(bridge) = matches.next() else {
            return Ok(None);
        };
        if matches.next().is_some()
            || bridge.state != State::Pending
            || bridge.record.types().unbranded_matrix != context
        {
            return Err(mismatch());
        }
        Ok(Some(SemanticValueBindingV1::MatrixBridgeLeaf {
            result: bridge.result,
        }))
    }

    fn live_matrix_context(
        &self,
        local: u32,
        value: SsaValueV1,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let actual = self
            .locals
            .get(local as usize)
            .and_then(Option::as_ref)
            .ok_or_else(mismatch)?;
        let recorded = self
            .semantic_ssa_bindings
            .get(&value)
            .ok_or_else(mismatch)?;
        match (actual, recorded) {
            (
                SemanticValueBindingV1::KernelContext {
                    value: a,
                    context: at,
                },
                SemanticValueBindingV1::KernelContext {
                    value: b,
                    context: bt,
                },
            ) if a == b
                && at == bt
                && self
                    .kernel_context
                    .is_some_and(|root| &root.context_type == at) =>
            {
                Ok(actual.clone())
            }
            _ => Err(rejected(
                "Matrix receiver is not its checked live Context SSA value",
            )),
        }
    }

    pub(super) fn lower_matrix_derive(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        kind: &SemanticStatementKindV1,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let Some(statement) = statement else {
            return Ok(false);
        };
        let site = (block, statement);
        let Some(plan) = &self.matrix_derives else {
            return Ok(false);
        };
        if let Some(flow) = plan.references.get(&site).cloned() {
            if !matches!(kind, SemanticStatementKindV1::Assign(a) if *a == flow.assignment) {
                return Err(mismatch());
            }
            let actual = self.live_matrix_context(flow.local, flow.value)?;
            let Some(SemanticValueBindingV1::KernelContext {
                value: issuer,
                context,
            }) = self.semantic_ssa_bindings.get(&flow.issuer)
            else {
                return Err(mismatch());
            };
            if !matches!(&actual, SemanticValueBindingV1::KernelContext { value, context: ty }
                if value == issuer && ty == context)
            {
                return Err(rejected(
                    "Matrix reference differs from its exact Context issuer",
                ));
            }
            if matches!(
                flow.assignment.value().kind(),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(_))
            ) {
                self.locals[flow.local as usize] = None;
                self.retained_local_initialized.remove(&flow.local);
            }
            self.bind_destination(
                block,
                Some(statement),
                flow.assignment.destination(),
                actual,
            )?;
            return Ok(true);
        }
        if let Some(bridge) = plan.bridges.get(&site).cloned() {
            if bridge.state != State::Pending
                || !self.is_kernel_entry
                || !matches!(kind, SemanticStatementKindV1::Assign(a) if *a == bridge.assignment)
            {
                return Err(mismatch());
            }
            let actual =
                self.live_matrix_context(bridge.result.receiver().index(), bridge.receiver)?;
            let Some(SemanticValueBindingV1::KernelContext {
                value: issuer,
                context,
            }) = self.semantic_ssa_bindings.get(&bridge.context)
            else {
                return Err(mismatch());
            };
            if !matches!(&actual, SemanticValueBindingV1::KernelContext { value, context: ty }
                if value == issuer && ty == context)
                || !matches!(self.locals.get(bridge.result.current().index() as usize).and_then(Option::as_ref),
                    Some(SemanticValueBindingV1::MatrixBridgeLeaf { result }) if *result == bridge.result)
                || !matches!(self.semantic_ssa_bindings.get(&bridge.current),
                    Some(SemanticValueBindingV1::MatrixBridgeLeaf { result }) if *result == bridge.result)
            {
                return Err(rejected(
                    "Matrix bridge substituted the actual receiver or Current occurrence",
                ));
            }
            for (local, value) in [
                (bridge.result.return_local(), bridge.returned),
                (bridge.result.destination(), bridge.destination),
            ] {
                if self
                    .pending_semantic_ssa_definitions
                    .get_mut(&(block.index(), local.index()))
                    .and_then(VecDeque::pop_front)
                    != Some(value)
                    || self.semantic_ssa_bindings.contains_key(&value)
                {
                    return Err(mismatch());
                }
                self.locals[local.index() as usize] = None;
                self.retained_local_initialized.remove(&local.index());
            }
            self.matrix_derives
                .as_mut()
                .unwrap()
                .bridges
                .get_mut(&site)
                .unwrap()
                .state = State::Transferred;
            return Ok(true);
        }
        let getter = plan
            .bridges
            .iter()
            .find(|(_, bridge)| {
                bridge.result.getter_block() == block
                    && bridge.result.getter_statement() == statement
            })
            .map(|(site, bridge)| (*site, bridge.clone()));
        let Some((bridge_site, bridge)) = getter else {
            return Ok(false);
        };
        if bridge.state != State::Transferred
            || !self.is_kernel_entry
            || !matches!(kind, SemanticStatementKindV1::Assign(a) if *a == bridge.getter_assignment)
            || self.locals[bridge.result.destination().index() as usize].is_some()
            || self.semantic_ssa_bindings.contains_key(&bridge.destination)
        {
            return Err(mismatch());
        }
        let Some(SemanticValueBindingV1::KernelContext { value, context }) =
            self.semantic_ssa_bindings.get(&bridge.context)
        else {
            return Err(mismatch());
        };
        let value = SemanticValueBindingV1::KernelMatrix(Box::new(MatrixValueV1 {
            context: *value,
            context_type: context.clone(),
            record: bridge.record,
            source: bridge.source,
            result: bridge.result,
        }));
        self.bind_destination(
            block,
            Some(statement),
            bridge.getter_assignment.destination(),
            value,
        )?;
        self.matrix_derives
            .as_mut()
            .unwrap()
            .bridges
            .get_mut(&bridge_site)
            .unwrap()
            .state = State::Consumed;
        Ok(true)
    }

    pub(super) fn matrix_bridge_entry(&self, block: u32, local: u32, value: SsaValueV1) -> bool {
        self.matrix_derives.as_ref().is_some_and(|plan| {
            plan.bridges.values().any(|bridge| {
                bridge.state == State::Transferred
                    && bridge.result.getter_block().index() == block
                    && bridge.result.destination().index() == local
                    && bridge.destination == value
            })
        })
    }

    pub(super) fn require_matrix_derives_consumed(
        &self,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.matrix_derives.as_ref().is_some_and(|plan| {
            plan.bridges
                .values()
                .any(|bridge| bridge.state != State::Consumed)
        }) {
            return Err(rejected(
                "Matrix bridge receipt was not consumed at the exact getter return",
            ));
        }
        Ok(())
    }
}
