impl BorrowedSubgroupLoweringV1 {
    fn checked_contract(
        &self,
        source: ExecutionCapabilitySourceV1,
    ) -> Result<ExecutionCapabilityOpV1> {
        if self.loans.is_empty() {
            return Err(reject("borrowed subgroup lost its retained shared loans"));
        }
        let Type::ExecutionCapability(result) = &self.result_type else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let ExecutionCapabilityRoleV1::BorrowedSubgroup {
            workgroup_reference,
            workgroup,
            width,
        } = result.role
        else {
            return Err(reject("borrowed subgroup lost its borrowed result role"));
        };
        if source.operation != *self.source_contract.source_identity().as_bytes()
            || source.block != self.source_block.index()
        {
            return Err(reject(
                "borrowed subgroup source carrier changed the original call",
            ));
        }
        if let Some(occurrence) = source.occurrence {
            if occurrence.expansion_identity() != self.expansion_identity
                || occurrence.expanded_root_identity() != self.root_identity
                || occurrence.caller_instance() != self.caller_instance.index()
                || occurrence.expanded_block() != self.execution_block.index()
            {
                return Err(reject(
                    "borrowed subgroup source carrier lost its checked occurrence",
                ));
            }
        }
        let operation = ExecutionCapabilityOperationV1::SubgroupDeriveBorrowed {
            workgroup_reference,
            workgroup,
            subgroup: result.source_type,
            width,
        };
        let contract = ExecutionCapabilityOpV1 {
            operands: vec![self.owned_value],
            signature: ExecutionCapabilitySignatureV1::new(
                &[workgroup_reference],
                result.source_type,
            )
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
            operation,
            provenance: result.provenance.clone(),
            workgroup_brand: result.workgroup_brand,
            epoch_before: result.epoch,
            epoch_after: None,
            source,
            obligations: ExecutionSafetyObligationsV1::from_bits(
                self.source_contract.obligations().bits(),
            ),
        };
        if !contract.is_complete() {
            return Err(reject("borrowed subgroup contract is incomplete"));
        }
        Ok(contract)
    }
}

impl BorrowedWorkgroupPlanV1<'_> {
    fn validate_receipt_source(
        &self,
        receipt: &BorrowedSubgroupLoweringV1,
        source: ExecutionCapabilitySourceV1,
    ) -> Result<()> {
        if source.function
            != *self.owner.source_semantic().functions()[receipt.source_function.index() as usize]
                .identity()
                .as_bytes()
            || self.view.has_expanded_calls() != source.occurrence.is_some()
            || source.occurrence.is_some_and(|o| {
                o.root_source_identity()
                    != *self.owner.source_semantic().functions()[self.view.root().index() as usize]
                        .identity()
                        .as_bytes()
            })
        {
            return Err(reject(
                "borrowed subgroup source owner or occurrence is missing",
            ));
        }
        Ok(())
    }

    fn epoch_projection_binding(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        result_type: SemanticTypeIdV1,
        value: &SemanticRvalueKindV1,
        lowering: &SemanticFunctionLoweringV1<'_>,
    ) -> Result<Option<SemanticValueBindingV1>> {
        if statement != Some(0) || !matches!(value, SemanticRvalueKindV1::Borrow { .. }) {
            return Ok(None);
        }
        let Some(epoch) = self
            .epochs
            .iter()
            .find(|e| e.binding.expanded_entry_block() == block)
            .cloned()
        else {
            return Ok(None);
        };
        let Some(contract) = self.contract_for_epoch(result_type)? else {
            return Err(reject(
                "epoch projection has no retained borrowed Workgroup contract",
            ));
        };
        let local = epoch.binding.callee_return().index();
        let SemanticStatementKindV1::Assign(assignment) =
            self.view.body().blocks()[block.index() as usize].statements()[0].kind()
        else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        if assignment.value().kind() != value
            || assignment.value().result_type() != result_type
            || assignment.destination().local().index() != local
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let definitions = self
            .graph
            .ssa
            .resolved_events(SsaBlockIdV1::new(block.index()))
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .iter()
            .filter_map(|(_, event)| match event {
                SsaResolvedEventV1::Define { variable, value } if variable.get() == local => {
                    Some(*value)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let [definition] = definitions.as_slice() else {
            return Err(reject("epoch projection SSA definition is ambiguous"));
        };
        let origin = self.resolve(*definition, &[], contract, 0)?;
        if !origin.epoch_projection || origin.loans.is_empty() {
            return Err(reject("epoch projection lost its retained shared loan"));
        }
        self.check_live(&origin, block.index())?;
        let binding = lowering
            .semantic_ssa_bindings
            .get(&origin.issuer)
            .ok_or_else(|| reject("epoch projection Workgroup issuer has not been lowered"))?;
        let values = binding.values().map_err(reject)?;
        let [(_, Type::ExecutionCapability(capability))] = values.as_slice() else {
            return Err(reject(
                "epoch projection owner is not one logical SSA value",
            ));
        };
        let SemanticExecutionCapabilityOperationV1::WorkgroupDerive { workgroup, .. } =
            origin.contract.operation()
        else {
            unreachable!()
        };
        if capability.role != ExecutionCapabilityRoleV1::Workgroup
            || capability.source_type != execution_type_identity_v1(lowering.types, workgroup)?
            || !same_provenance(capability, origin.contract, self.context)
        {
            return Err(reject(
                "epoch projection changed its actual Workgroup SSA owner",
            ));
        }
        Ok(Some(binding.clone()))
    }

    fn partition_receipt(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        contract: SemanticExecutionCapabilityContractV1,
        operands: &[ValueId],
        types: &[Type],
    ) -> Result<Option<BorrowedSubgroupLoweringV1>> {
        use fe2o3_mir_model::semantic_mir_v1::SemanticSubgroupPartitionOperationV1 as P;
        let SemanticExecutionCapabilityOperationV1::SubgroupPartition(partition) =
            contract.operation()
        else {
            if types.iter().any(|ty| matches!(ty, Type::ExecutionCapability(c) if matches!(c.role, ExecutionCapabilityRoleV1::BorrowedSubgroup { .. }))) {
                return Err(reject("borrowed subgroup consumer has no checked lifetime adapter"));
            }
            return Ok(None);
        };
        match partition {
            P::Derive { .. } => {
                let [subgroup, workgroup] = operands else {
                    return Err(reject("partition derive lost its two logical SSA operands"));
                };
                if !matches!(types.first(), Some(Type::ExecutionCapability(c)) if matches!(c.role, ExecutionCapabilityRoleV1::BorrowedSubgroup { .. }))
                {
                    return Ok(None);
                }
                let receipt = self.subgroups.get(subgroup).cloned().ok_or_else(|| {
                    reject("partition derive lost the borrowed subgroup issuer and loans")
                })?;
                let epoch = call
                    .arguments()
                    .get(1)
                    .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                self.epoch_matches_subgroup(block, epoch, &receipt)?;
                if *workgroup != receipt.owned_value {
                    return Err(reject(
                        "partition epoch changed its exact lowered Workgroup SSA issuer",
                    ));
                }
                Ok(Some(receipt))
            }
            P::ReduceSumF32 { .. } | P::ReduceMaxF32 { .. } | P::BroadcastF32 { .. } => {
                let Some(receiver) = operands.first() else {
                    return Err(reject("partition consumer lost its receiver"));
                };
                let Some(receipt) = self.partitions.get(receiver).cloned() else {
                    return Err(reject(
                        "partition consumer lost its exact borrowed issuer loan",
                    ));
                };
                // A recorded legacy derive is distinct from a missing loan receipt.
                let Some(receipt) = receipt else {
                    return Ok(None);
                };
                if receipt.loans.is_empty()
                    || receipt.source_contract.provenance() != contract.provenance()
                    || receipt.source_contract.workgroup_brand() != contract.workgroup_brand()
                    || receipt.source_contract.epoch_before() != contract.epoch_before()
                {
                    return Err(reject(
                        "partition consumer changed its borrowed owner epoch",
                    ));
                }
                let issuer = self.graph.definition(receipt.owned_ssa)?;
                let issuer_contract = self.execution_call(issuer.block)?.1;
                let origin = Origin {
                    issuer: receipt.owned_ssa,
                    contract: issuer_contract,
                    loans: receipt.loans.clone(),
                    epoch_projection: false,
                };
                self.check_live(&origin, block.index())?;
                Ok(Some(receipt))
            }
        }
    }
}

impl SemanticFunctionLoweringV1<'_> {
    pub(super) fn lower_borrowed_subgroup_v1(
        &mut self,
        block: SemanticBlockIdV1,
        source: ExecutionCapabilitySourceV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1> {
        let mut plan = self
            .borrowed_workgroup
            .take()
            .ok_or_else(|| reject("borrowed subgroup lacks its replay-checked source SSA plan"))?;
        let result = (|| {
            let receipt = plan.subgroup(block, self)?;
            plan.validate_receipt_source(&receipt, source)?;
            let contract = receipt.checked_contract(source)?;
            let ty = receipt.result_type.clone();
            let value = self.emit_id(
                operations,
                ty.clone(),
                OperationKind::ExecutionCapability(contract),
            )?;
            if plan.subgroups.insert(value, receipt).is_some() {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            Ok(SemanticValueBindingV1::Value { id: value, ty })
        })();
        self.borrowed_workgroup = Some(plan);
        result
    }

    pub(super) fn lower_epoch_projection_v1(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        result_type: SemanticTypeIdV1,
        value: &SemanticRvalueKindV1,
    ) -> Result<Option<SemanticValueBindingV1>> {
        let Some(mut plan) = self.borrowed_workgroup.take() else {
            return Ok(None);
        };
        let result = plan.epoch_projection_binding(block, statement, result_type, value, self);
        self.borrowed_workgroup = Some(plan);
        result
    }

    pub(super) fn check_borrowed_partition_v1(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        contract: SemanticExecutionCapabilityContractV1,
        operands: &[ValueId],
        types: &[Type],
    ) -> Result<Option<BorrowedSubgroupLoweringV1>> {
        let Some(plan) = self.borrowed_workgroup.as_mut() else {
            if types.iter().any(|ty| matches!(ty, Type::ExecutionCapability(c) if matches!(c.role, ExecutionCapabilityRoleV1::BorrowedSubgroup { .. }))) {
                return Err(reject("borrowed subgroup consumer lost the source SSA plan"));
            }
            return Ok(None);
        };
        plan.partition_receipt(block, call, contract, operands, types)
    }

    pub(super) fn retain_borrowed_partition_v1(
        &mut self,
        operation: SemanticExecutionCapabilityOperationV1,
        results: &[ValueDef],
        receipt: Option<BorrowedSubgroupLoweringV1>,
    ) -> Result<()> {
        if !matches!(
            operation,
            SemanticExecutionCapabilityOperationV1::SubgroupPartition(
                fe2o3_mir_model::semantic_mir_v1::SemanticSubgroupPartitionOperationV1::Derive { .. }
            )
        ) {
            return Ok(());
        }
        if let Some(plan) = self.borrowed_workgroup.as_mut() {
            let [result] = results else {
                return Err(reject("partition derive changed its result arity"));
            };
            if plan.partitions.insert(result.id, receipt).is_some() {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
        } else if receipt.is_some() {
            return Err(reject("partition derive lost the source SSA plan"));
        }
        Ok(())
    }
}
