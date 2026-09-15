use fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1;
use numerical_policy_math_custody_01::{MathCustodyPlanV1, MathTransportV1};
use numerical_policy_math_occurrences_01::MathStatementSiteV1;

#[derive(Clone, Debug)]
struct MathProducerV1 {
    adapter: NumericalPolicyMathLoweringV1,
    operation: NumericalPolicyMathOperationV1,
    source: ExecutionCapabilitySourceV1,
    dependencies: Vec<SsaValueV1>,
    assignment: SemanticAssignmentV1,
}

struct MathSsaLoweringPlanV1 {
    custody: MathCustodyPlanV1,
    producers: BTreeMap<MathStatementSiteV1, MathProducerV1>,
    bridges: BTreeMap<MathStatementSiteV1, MathBridgeV1>,
}

impl MathSsaLoweringPlanV1 {
    fn new(
        owner: &ProductionSemanticSsaOwnerV1,
        function: &SemanticFunctionDeclV1,
        context: &RootKernelContextLoweringV1,
        max_work: usize,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let custody = MathCustodyPlanV1::new(owner, context, max_work)?;
        let view = owner
            .execution_view_for_root(context.selected_root)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if view.body() != function {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let mut producers = BTreeMap::new();
        for (&block, resolved) in &custody.consumers {
            let consumer = &custody.occurrences.consumers[&block];
            let adapter = NumericalPolicyMathLoweringV1::new(
                owner.source_semantic().types(),
                consumer.contract,
                &context.context_type,
            )?;
            let getter_site = resolved
                .getter
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let bind_site = resolved
                .bind
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let getter = &custody.occurrences.getters[&getter_site];
            let bind = &custody.occurrences.binds[&bind_site];
            for (site, binding, operation, dependencies) in [
                (
                    getter_site,
                    &getter.binding,
                    NumericalPolicyMathOperationV1::MathDerive {
                        context: execution_type_identity_v1(
                            owner.source_semantic().types(),
                            getter.record.types().context_reference,
                        )?,
                        binding: adapter.binding,
                    },
                    vec![resolved.context],
                ),
                (
                    bind_site,
                    &bind.binding,
                    NumericalPolicyMathOperationV1::Bind {
                        binding: adapter.binding,
                    },
                    vec![
                        resolved
                            .math
                            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
                        resolved
                            .policy
                            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
                    ],
                ),
            ] {
                let source = CheckedExecutionSourceCarrierV1::source_for_defined_statement(
                    owner,
                    context.selected_root,
                    function,
                    *owner.execution_expansion().identity(),
                    *view.identity(),
                    binding,
                    site.block,
                    site.statement,
                )?;
                let SemanticStatementKindV1::Assign(assignment) = function.blocks()
                    [site.block.index() as usize]
                    .statements()[site.statement as usize]
                    .kind()
                else {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                };
                if let Some(previous) = producers.get(&site) {
                    let previous: &MathProducerV1 = previous;
                    if previous.operation != operation
                        || previous.dependencies != dependencies
                        || previous.source != source
                        || previous.adapter.provenance != adapter.provenance
                        || previous.assignment != *assignment
                    {
                        return Err(unsupported(
                            0,
                            Some(site.block.index()),
                            Some(site.statement),
                            "one defined Math producer has conflicting checked consumer bindings",
                        ));
                    }
                } else {
                    producers.insert(
                        site,
                        MathProducerV1 {
                            adapter: adapter.clone(),
                            operation,
                            source,
                            dependencies,
                            assignment: assignment.clone(),
                        },
                    );
                }
            }
        }
        let bridges = math_bridge_plans_v1(owner, context, &custody, &producers, max_work)?;
        Ok(Self { custody, producers, bridges })
    }
}

include!("capture_transport.rs");

impl MathTransportV1 {
    fn kernel_type(
        self,
        types: &[SemanticTypeDeclV1],
        semantic_type: SemanticTypeIdV1,
        context: Option<&KernelContextTypeV1>,
    ) -> Result<Type, ProductionSemanticKirErrorV1> {
        if self.capture.is_some() || semantic_type != self.semantic_type() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let adapter = NumericalPolicyMathLoweringV1::new(
            types,
            self.contract,
            context.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
        )?;
        Ok(numerical_policy_math_capability_type_v1(
            if self.bound {
                adapter.binding.bound
            } else {
                adapter.binding.math
            },
            if self.bound {
                ExecutionCapabilityRoleV1::NumericalPolicyMathBound(adapter.binding)
            } else {
                ExecutionCapabilityRoleV1::NumericalPolicyMathSource(adapter.binding)
            },
            &adapter.provenance,
        ))
    }

    fn values(
        self,
        binding: &SemanticValueBindingV1,
        expected: &[Type],
    ) -> Result<Vec<(ValueId, Type)>, &'static str> {
        if self.capture.is_some() {
            return self.capture_values(binding, expected);
        }
        let (
            SemanticValueBindingV1::Value {
                id,
                ty: Type::ExecutionCapability(capability),
            },
            [Type::ExecutionCapability(expected)],
        ) = (binding, expected)
        else {
            return Err("Math SSA transport needs one existing capability value");
        };
        let role_matches = matches!(
            (&capability.role, self.bound),
            (ExecutionCapabilityRoleV1::NumericalPolicyMathBound(_), true)
                | (
                    ExecutionCapabilityRoleV1::NumericalPolicyMathSource(_),
                    false
                )
        );
        if capability != expected || !role_matches || !capability.is_complete() {
            return Err("Math SSA transport changed root, policy or nominal authority type");
        }
        Ok(vec![(*id, Type::ExecutionCapability(capability.clone()))])
    }

    fn from_values(
        self,
        types: &[SemanticTypeDeclV1],
        semantic_type: SemanticTypeIdV1,
        values: &[ValueDef],
        expected: &[Type],
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        if self.capture.is_some() {
            return self.capture_from_values(types, semantic_type, values, expected);
        }
        let [
            ValueDef {
                id,
                ty: Type::ExecutionCapability(capability),
            },
        ] = values
        else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        let context = KernelContextTypeV1::new(
            capability.provenance.root.clone(),
            capability.provenance.kernel_marker,
            capability.provenance.target_brand,
            capability.provenance.launch_brand,
        );
        let ty = self.kernel_type(types, semantic_type, Some(&context))?;
        if expected != [ty.clone()] || ty != values[0].ty {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let binding = SemanticValueBindingV1::Value { id: *id, ty };
        self.values(&binding, expected)
            .map_err(|detail| unsupported(0, None, None, detail))?;
        Ok(binding)
    }
}

impl SemanticFunctionLoweringV1<'_> {
    fn math_issuer_value(
        &self,
        value: SsaValueV1,
    ) -> Result<(ValueId, Type), ProductionSemanticKirErrorV1> {
        let binding = self.semantic_ssa_bindings.get(&value).ok_or_else(|| {
            unsupported(
                0,
                None,
                None,
                "Math original SSA issuer has not been lowered",
            )
        })?;
        let values = binding
            .values()
            .map_err(|detail| unsupported(0, None, None, detail))?;
        let [value] = values.as_slice() else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        Ok(value.clone())
    }

    fn lower_math_producer(
        &mut self,
        block: SemanticBlockIdV1,
        statement: Option<u32>,
        kind: &SemanticStatementKindV1,
        operations: &mut Vec<Operation>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        let Some(statement) = statement else {
            return Ok(false);
        };
        let site = MathStatementSiteV1 { block, statement };
        if self.lower_math_bridge(site, kind)? || self.lower_math_reference(site, kind)? {
            return Ok(true);
        }
        let Some(producer) = self
            .math_ssa
            .as_ref()
            .and_then(|plan| plan.producers.get(&site))
            .cloned()
        else {
            return Ok(false);
        };
        if !matches!(kind, SemanticStatementKindV1::Assign(assignment) if *assignment == producer.assignment)
            || !self.is_kernel_entry
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let operands = producer
            .dependencies
            .iter()
            .map(|value| self.math_issuer_value(*value))
            .collect::<Result<Vec<_>, _>>()?;
        if matches!(producer.operation, NumericalPolicyMathOperationV1::MathDerive { .. }) {
            self.consume_math_bridge(site, &producer.assignment)?;
        }
        if matches!(
            producer.operation,
            NumericalPolicyMathOperationV1::Bind { .. }
        ) {
            let SemanticRvalueKindV1::Aggregate(aggregate) = producer.assignment.value().kind()
            else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            for (argument, original) in aggregate.operands()[..2].iter().zip(&operands) {
                let actual = self.lower_operand(block, Some(statement), argument, operations)?;
                if actual.values().map_err(|detail| {
                    unsupported(0, Some(block.index()), Some(statement), detail)
                })? != [original.clone()]
                {
                    return Err(unsupported(
                        0,
                        Some(block.index()),
                        Some(statement),
                        "Math Bind live reference differs from its original SSA issuer",
                    ));
                }
            }
        }
        let (operation, ty) =
            producer
                .adapter
                .operation(producer.operation, producer.source, &operands)?;
        let value = self.emit(
            operations,
            ty,
            OperationKind::ExecutionCapability(operation),
        )?;
        self.bind_destination(
            block,
            Some(statement),
            producer.assignment.destination(),
            value,
        )?;
        Ok(true)
    }

    fn lower_policy_math_f32(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        contract: SemanticNumericalPolicyMathContractV1,
        callable: SemanticFunctionIdentityV1,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1> {
        let plan = self.math_ssa.as_ref().ok_or_else(|| {
            unsupported(
                0,
                Some(block.index()),
                None,
                "policy Math consumer lacks a replayed source SSA custody plan",
            )
        })?;
        let occurrence = plan
            .custody
            .occurrences
            .consumers
            .get(&block)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let resolved = plan
            .custody
            .consumers
            .get(&block)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if occurrence.contract != contract
            || occurrence.call != *call
            || callable != contract.source_identity()
            || !self.is_kernel_entry
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let bound = self.math_issuer_value(resolved.issuer)?;
        let receiver = *plan.custody.receivers.get(&block)
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let Some(SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = call.arguments().first() else {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        };
        if self.math_live_binding(place.local().index(), receiver)?.values()
            .map_err(|d| unsupported(0, Some(block.index()), None, d))? != [bound.clone()] {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let context = self
            .kernel_context
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let adapter =
            NumericalPolicyMathLoweringV1::new(self.types, contract, &context.context_type)?;
        let source = self
            .execution_source_carrier
            .as_ref()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
            .source_for_call(
                self.correspondence_owner,
                self.function,
                self.execution_source_expansion_identity,
                self.execution_expansion_identity,
                block,
                call,
                callable,
            )?;
        let mut operands = Vec::with_capacity(call.arguments().len());
        for argument in call.arguments() {
            let value = self.lower_operand(block, None, argument, operations)?;
            let values = value
                .values()
                .map_err(|detail| unsupported(0, Some(block.index()), None, detail))?;
            let [value] = values.as_slice() else {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            };
            operands.push(value.clone());
        }
        if operands.first() != Some(&bound) {
            return Err(unsupported(
                0,
                Some(block.index()),
                None,
                "policy Math consumer reference differs from its original Bind SSA issuer",
            ));
        }
        let (operation, ty) = adapter.operation(adapter.consumer(), source, &operands)?;
        self.emit(
            operations,
            ty,
            OperationKind::ExecutionCapability(operation),
        )
    }
}
