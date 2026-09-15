use fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct BorrowedSubgroupTransportV1 {
    contract: SemanticExecutionCapabilityContractV1,
    source: ExecutionTypeIdentityV1,
    reference: ExecutionTypeIdentityV1,
    workgroup: ExecutionTypeIdentityV1,
    width: u32,
}

impl BorrowedSubgroupTransportV1 {
    pub(super) fn register(
        types: &[SemanticTypeDeclV1],
        callables: &[SemanticCallableDeclV1],
        bindings: &mut BTreeMap<SemanticTypeIdV1, SemanticPromotedBindingV1>,
    ) -> Result<()> {
        for callable in callables {
            let SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
                ..
            } = callable
            else {
                continue;
            };
            let SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed {
                workgroup_reference,
                workgroup,
                subgroup,
                width,
            } = contract.operation()
            else {
                continue;
            };
            if binding.identity() != contract.source_identity()
                || !shared_type(types, workgroup_reference, workgroup)
                || width != 64
            {
                return Err(reject(
                    "borrowed subgroup SSA transport source reference changed",
                ));
            }
            let transport = Self {
                contract: *contract,
                source: execution_type_identity_v1(types, subgroup)?,
                reference: execution_type_identity_v1(types, workgroup_reference)?,
                workgroup: execution_type_identity_v1(types, workgroup)?,
                width,
            };
            insert_compiler_issued_ssa_binding_v1(
                bindings,
                subgroup,
                SemanticPromotedBindingV1::BorrowedSubgroup(transport),
            )?;
        }
        Ok(())
    }

    fn role(self) -> ExecutionCapabilityRoleV1 {
        ExecutionCapabilityRoleV1::BorrowedSubgroup {
            workgroup_reference: self.reference,
            workgroup: self.workgroup,
            width: self.width,
        }
    }

    pub(super) fn types(
        self,
        types: &[SemanticTypeDeclV1],
        semantic: SemanticTypeIdV1,
        context: Option<&KernelContextTypeV1>,
    ) -> Result<Vec<Type>> {
        let context = context
            .ok_or_else(|| reject("borrowed subgroup transport lacks the authenticated root"))?;
        let provenance = self.contract.provenance();
        if semantic != self.contract.signature().output()
            || execution_type_identity_v1(types, semantic)? != self.source
            || context.kernel_marker() != provenance.kernel_marker().as_bytes()
            || context.target() != provenance.target_brand().as_bytes()
            || context.launch() != provenance.launch_brand().as_bytes()
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(vec![Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
            source_type: self.source,
            provenance: ExecutionCapabilityProvenanceV1 {
                root: context.root().clone(),
                kernel_binding: *provenance.kernel_binding().as_bytes(),
                frontend_unit: *provenance.frontend_unit().as_bytes(),
                kernel_marker: *provenance.kernel_marker().as_bytes(),
                target_brand: *provenance.target_brand().as_bytes(),
                launch_brand: *provenance.launch_brand().as_bytes(),
                issuance: *provenance.issuance().as_bytes(),
            },
            workgroup_brand: self.contract.workgroup_brand().map(|id| *id.as_bytes()),
            epoch: self.contract.epoch_before().map(|id| *id.as_bytes()),
            role: self.role(),
        })])
    }

    pub(super) fn values(
        self,
        binding: &SemanticValueBindingV1,
    ) -> std::result::Result<Vec<(ValueId, Type)>, &'static str> {
        let values = binding.values()?;
        let [(value, Type::ExecutionCapability(capability))] = values.as_slice() else {
            return Err("borrowed subgroup transport lost its logical capability");
        };
        let p = self.contract.provenance();
        if !capability.is_complete()
            || capability.source_type != self.source
            || capability.role != self.role()
            || capability.provenance.kernel_binding != *p.kernel_binding().as_bytes()
            || capability.provenance.frontend_unit != *p.frontend_unit().as_bytes()
            || capability.provenance.kernel_marker != *p.kernel_marker().as_bytes()
            || capability.provenance.target_brand != *p.target_brand().as_bytes()
            || capability.provenance.launch_brand != *p.launch_brand().as_bytes()
            || capability.provenance.issuance != *p.issuance().as_bytes()
            || capability.workgroup_brand
                != self.contract.workgroup_brand().map(|id| *id.as_bytes())
            || capability.epoch != self.contract.epoch_before().map(|id| *id.as_bytes())
        {
            return Err("borrowed subgroup transport changed its retained type or provenance");
        }
        Ok(vec![(
            *value,
            Type::ExecutionCapability(capability.clone()),
        )])
    }

    pub(super) fn restore(
        self,
        types: &[SemanticTypeDeclV1],
        semantic: SemanticTypeIdV1,
        values: &[ValueDef],
    ) -> Result<SemanticValueBindingV1> {
        let [value] = values else {
            return Err(reject("borrowed subgroup SSA transport changed arity"));
        };
        let Type::ExecutionCapability(capability) = &value.ty else {
            return Err(reject("borrowed subgroup SSA transport lost its role"));
        };
        let p = &capability.provenance;
        let context = KernelContextTypeV1::new(
            p.root.clone(),
            p.kernel_marker,
            p.target_brand,
            p.launch_brand,
        );
        if self.types(types, semantic, Some(&context))? != [value.ty.clone()] {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let binding = SemanticValueBindingV1::Value {
            id: value.id,
            ty: value.ty.clone(),
        };
        self.values(&binding).map_err(reject)?;
        Ok(binding)
    }
}

impl BorrowedWorkgroupPlanV1<'_> {
    pub(super) fn needed(owner: &ProductionSemanticSsaOwnerV1, root: SemanticFunctionIdV1) -> bool {
        owner.execution_view_for_root(root).is_some_and(|view| view.body().blocks().iter().any(|block| {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else { return false; };
            matches!(owner.source_semantic().callables().get(call.callee().index() as usize), Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract }, ..
            }) if matches!(contract.operation(), SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed { .. }
                | SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexV2 { .. })
                || borrowed_allocation_pair(owner.source_semantic().types(), contract.operation()).is_ok_and(|pair| pair.is_some()))
        }))
    }

    pub(super) fn epoch_transport(
        &self,
        local: u32,
    ) -> Option<(SemanticTypeIdV1, SemanticPromotedTransportV1)> {
        self.epoch_transports
            .get(&local)
            .map(|(ty, binding)| (*ty, SemanticPromotedTransportV1::Semantic(*binding)))
    }

    fn contract_for_epoch(
        &mut self,
        ty: SemanticTypeIdV1,
    ) -> Result<Option<SemanticExecutionCapabilityContractV1>> {
        let work = self
            .epochs
            .len()
            .checked_mul(
                self.owner
                    .source_semantic()
                    .callables()
                    .len()
                    .saturating_add(1),
            )
            .ok_or_else(|| reject("epoch contract lookup work limit"))?;
        self.graph.charge(work)?;
        Ok(self.epochs.iter().find_map(|epoch| {
            if epoch.projection.types().epoch_reference != ty { return None; }
            self.owner.source_semantic().callables().iter().find_map(|callable| {
                let SemanticCallableDeclV1::CompilerIntrinsic { operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract }, .. } = callable else { return None; };
                matches!(contract.operation(), SemanticExecutionCapabilityOperationV1::SubgroupDeriveBorrowed { workgroup_reference, workgroup, .. }
                    if workgroup_reference == epoch.projection.types().reference && workgroup == epoch.projection.types().workgroup
                    && contract.provenance() == epoch.projection.provenance() && contract.workgroup_brand() == Some(epoch.projection.brand())
                    && contract.epoch_before() == Some(epoch.projection.epoch())).then_some(*contract)
            })
        }))
    }

    fn prepare_epoch_transports(&mut self) -> Result<()> {
        let plan = self
            .owner
            .execution_plan_for_root(self.view.root())
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if !std::ptr::eq(plan.plan(), self.graph.ssa) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let mut definitions = Vec::new();
        for block in self.graph.ssa.reverse_postorder() {
            let events = self
                .graph
                .ssa
                .resolved_events(*block)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            self.graph.charge(events.len())?;
            for (_, event) in events {
                if let SsaResolvedEventV1::Define { variable, value } = event {
                    let Some(local) = epoch_definition_source_local_v1(
                        self.graph.body.locals(),
                        *variable,
                        *block,
                        |index| {
                            plan.defined_reusable_phase_relays()
                                .get(index)
                                .map(|relay| (relay.variable(), relay.pack_block()))
                        },
                    )? else {
                        continue;
                    };
                    let ty = local.ty();
                    if let Some(contract) = self.contract_for_epoch(ty)? {
                        definitions.push((variable.get(), *value, contract));
                    }
                }
            }
        }
        let mut issuers = BTreeMap::new();
        for (local, value, contract) in definitions {
            let origin = self.resolve(value, &[], contract, 0)?;
            if !origin.epoch_projection || origin.loans.is_empty() {
                return Err(reject(
                    "epoch SSA transport lacks its replayed projection and shared loan",
                ));
            }
            if issuers
                .insert(local, origin.issuer)
                .is_some_and(|old| old != origin.issuer)
            {
                return Err(reject(
                    "epoch local transports conflicting Workgroup SSA issuers",
                ));
            }
            let SemanticExecutionCapabilityOperationV1::WorkgroupDerive { workgroup, .. } =
                origin.contract.operation()
            else {
                unreachable!()
            };
            let binding = SemanticPromotedBindingV1::SubgroupPartitionAuthority {
                contract: origin.contract,
                source_type: execution_type_identity_v1(
                    self.owner.source_semantic().types(),
                    workgroup,
                )?,
            };
            if self
                .epoch_transports
                .insert(local, (workgroup, binding))
                .is_some_and(|old| old != (workgroup, binding))
            {
                return Err(reject(
                    "epoch SSA transport changed its exact Workgroup source type",
                ));
            }
        }
        Ok(())
    }
}

// Each caller visit is covered by the enclosing resolved-event debit. This
// allocation-free classifier gives relays no Rust type or epoch authority.
fn epoch_definition_source_local_v1(
    locals: &[SemanticLocalDeclV1],
    variable: fe2o3_mir_model::SsaVariableIdV1,
    block: SsaBlockIdV1,
    relay_at: impl FnOnce(usize) -> Option<(fe2o3_mir_model::SsaVariableIdV1, SemanticBlockIdV1)>,
) -> Result<Option<&SemanticLocalDeclV1>> {
    if let Some(local) = locals.get(variable.get() as usize) {
        return Ok(Some(local));
    }
    // The same replayed plan numbers its relay rows immediately after locals.
    let (retained_variable, pack_block) = (variable.get() as usize)
        .checked_sub(locals.len())
        .and_then(relay_at)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if retained_variable != variable || pack_block.index() != block.get() {
        return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
    }
    Ok(None)
}

#[cfg(test)]
mod epoch_namespace_tests {
    use super::*;
    include!("epoch_namespace_tests.rs");
}
