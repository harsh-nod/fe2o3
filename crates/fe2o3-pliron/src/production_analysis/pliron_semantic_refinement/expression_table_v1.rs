#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum SemanticNodeV1 {
    Symbol(u32),
    Constant(u64),
    Binary(SemanticBinaryKindAttr, usize, usize),
    /// Opaque equality is sound only for byte-identical retained commitments.
    Commitment([u64; 4]),
    TypedExpression(SemanticTypedExpressionV1),
    TypedRoot(SemanticTypedExpressionV1, SemanticNumericalContractV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SemanticExpressionBuildErrorV1 {
    ResourceLimit,
    InvalidTypedExpression(&'static str),
}

/// Shared canonical expression table used by scalar and effect refinement.
pub(crate) struct SemanticExpressionTableV1 {
    nodes: Vec<SemanticNodeV1>,
    facts: HashMap<pliron::value::Value, usize>,
    typed_root_commitments: Vec<[u64; 4]>,
}

impl SemanticExpressionTableV1 {
    pub(crate) fn from_inventory(
        context: &Context,
        inventory: &BoundedPlironFunctionInventoryV1,
    ) -> Result<Self, SemanticExpressionBuildErrorV1> {
        let definitions = inventory
            .operations()
            .iter()
            .filter_map(|site| {
                let pointer = site.pointer();
                let operation = Operation::get_op_dyn(pointer, context);
                is_semantic_refinement_definition_v1(&*operation).then_some(pointer)
            })
            .collect::<Vec<_>>();
        Self::build(context, &definitions)
    }

    fn build(
        context: &Context,
        definitions: &[pliron::context::Ptr<Operation>],
    ) -> Result<Self, SemanticExpressionBuildErrorV1> {
        if definitions.len() > MAX_PLIRON_SEMANTIC_NODES_V1 {
            return Err(SemanticExpressionBuildErrorV1::ResourceLimit);
        }
        let mut nodes = Vec::new();
        let mut interned = HashMap::new();
        let mut facts = HashMap::new();
        for _ in 0..=definitions.len() {
            let mut changed = false;
            for definition in definitions {
                let operation = Operation::get_op_dyn(*definition, context);
                let node = if let Some(symbol) = operation.downcast_ref::<SemanticSymbolOp>() {
                    symbol.symbol(context).map(SemanticNodeV1::Symbol)
                } else if let Some(constant) = operation.downcast_ref::<SemanticConstantOp>() {
                    constant.value(context).map(SemanticNodeV1::Constant)
                } else if let Some(binary) = operation.downcast_ref::<SemanticBinaryOp>() {
                    let lhs = facts.get(&binary.lhs(context)).copied();
                    let rhs = facts.get(&binary.rhs(context)).copied();
                    match (binary.kind(context), lhs, rhs) {
                        (Some(kind), Some(lhs), Some(rhs)) => {
                            let (lhs, rhs) = if lhs <= rhs { (lhs, rhs) } else { (rhs, lhs) };
                            Some(SemanticNodeV1::Binary(kind, lhs, rhs))
                        }
                        _ => None,
                    }
                } else if let Some(commitment) =
                    operation.downcast_ref::<SemanticExpressionCommitmentOp>()
                {
                    commitment.identity(context).map(SemanticNodeV1::Commitment)
                } else if let Some(symbol) = operation.downcast_ref::<SemanticTypedSymbolOp>() {
                    match (symbol.symbol(context), symbol.scalar(context)) {
                        (Some(symbol), Some(scalar)) => Some(SemanticNodeV1::TypedExpression(
                            SemanticTypedExpressionV1::Symbol { symbol, scalar },
                        )),
                        _ => None,
                    }
                } else if let Some(constant) = operation.downcast_ref::<SemanticTypedConstantOp>() {
                    match (constant.bits(context), constant.scalar(context)) {
                        (Some(bits), Some(scalar)) => Some(SemanticNodeV1::TypedExpression(
                            SemanticTypedExpressionV1::Constant { scalar, bits },
                        )),
                        _ => None,
                    }
                } else if let Some(unary) = operation.downcast_ref::<SemanticTypedUnaryOp>() {
                    let operand =
                        typed_expression(&nodes, facts.get(&unary.operand(context)).copied());
                    match (unary.kind(context), unary.scalar(context), operand) {
                        (Some(operation), Some(scalar), Some(operand)) => Some(
                            SemanticNodeV1::TypedExpression(SemanticTypedExpressionV1::Unary {
                                operation,
                                scalar,
                                operand: Box::new(operand),
                            }),
                        ),
                        _ => None,
                    }
                } else if let Some(binary) = operation.downcast_ref::<SemanticTypedBinaryOp>() {
                    let lhs = typed_expression(&nodes, facts.get(&binary.lhs(context)).copied());
                    let rhs = typed_expression(&nodes, facts.get(&binary.rhs(context)).copied());
                    match (
                        binary.kind(context),
                        binary.scalar(context),
                        binary.overflow(context),
                        lhs,
                        rhs,
                    ) {
                        (Some(operation), Some(scalar), Some(overflow), Some(lhs), Some(rhs)) => {
                            Some(SemanticNodeV1::TypedExpression(
                                SemanticTypedExpressionV1::Binary {
                                    operation,
                                    scalar,
                                    overflow,
                                    lhs: Box::new(lhs),
                                    rhs: Box::new(rhs),
                                },
                            ))
                        }
                        _ => None,
                    }
                } else if let Some(compare) = operation.downcast_ref::<SemanticTypedCompareOp>() {
                    let lhs = typed_expression(&nodes, facts.get(&compare.lhs(context)).copied());
                    let rhs = typed_expression(&nodes, facts.get(&compare.rhs(context)).copied());
                    match (
                        compare.kind(context),
                        compare.operand_scalar(context),
                        lhs,
                        rhs,
                    ) {
                        (Some(operation), Some(operand_scalar), Some(lhs), Some(rhs)) => Some(
                            SemanticNodeV1::TypedExpression(SemanticTypedExpressionV1::Compare {
                                operation,
                                operand_scalar,
                                lhs: Box::new(lhs),
                                rhs: Box::new(rhs),
                            }),
                        ),
                        _ => None,
                    }
                } else if let Some(select) = operation.downcast_ref::<SemanticTypedSelectOp>() {
                    let condition =
                        typed_expression(&nodes, facts.get(&select.condition(context)).copied());
                    let when_true =
                        typed_expression(&nodes, facts.get(&select.when_true(context)).copied());
                    let when_false =
                        typed_expression(&nodes, facts.get(&select.when_false(context)).copied());
                    match (select.scalar(context), condition, when_true, when_false) {
                        (Some(scalar), Some(condition), Some(when_true), Some(when_false)) => Some(
                            SemanticNodeV1::TypedExpression(SemanticTypedExpressionV1::Select {
                                scalar,
                                condition: Box::new(condition),
                                when_true: Box::new(when_true),
                                when_false: Box::new(when_false),
                            }),
                        ),
                        _ => None,
                    }
                } else if let Some(cast) = operation.downcast_ref::<SemanticTypedCastOp>() {
                    let operand =
                        typed_expression(&nodes, facts.get(&cast.operand(context)).copied());
                    match (
                        cast.kind(context),
                        cast.source(context),
                        cast.target(context),
                        operand,
                    ) {
                        (Some(kind), Some(source), Some(target), Some(operand)) => Some(
                            SemanticNodeV1::TypedExpression(SemanticTypedExpressionV1::Cast {
                                kind,
                                source,
                                target,
                                operand: Box::new(operand),
                            }),
                        ),
                        _ => None,
                    }
                } else if let Some(root) = operation.downcast_ref::<SemanticTypedExpressionRootOp>()
                {
                    let expression =
                        typed_expression(&nodes, facts.get(&root.expression(context)).copied());
                    match (
                        expression,
                        root.policy(context),
                        root.rounding(context),
                        root.exceptional_values(context),
                        root.commitment(context),
                    ) {
                        (
                            Some(expression),
                            Some(policy),
                            Some(rounding),
                            Some(exceptional_values),
                            Some(commitment),
                        ) => {
                            let contract = SemanticNumericalContractV1 {
                                policy,
                                rounding,
                                exceptional_values,
                            };
                            validate_typed_root(&expression, contract, commitment)?;
                            Some(SemanticNodeV1::TypedRoot(expression, contract))
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                let Some(node) = node else { continue };
                if let SemanticNodeV1::TypedExpression(expression) = &node {
                    expression.validate().map_err(|error| {
                        SemanticExpressionBuildErrorV1::InvalidTypedExpression(match error {
                            dialect_kernel::SemanticTypedExpressionErrorV1::ResourceLimit => {
                                "typed semantic expression exceeds its node or depth bound"
                            }
                            dialect_kernel::SemanticTypedExpressionErrorV1::TypeMismatch => {
                                "typed semantic expression has an invalid type or operator"
                            }
                            dialect_kernel::SemanticTypedExpressionErrorV1::ConstantOutOfRange => {
                                "typed semantic constant exceeds its scalar width"
                            }
                            dialect_kernel::SemanticTypedExpressionErrorV1::UnsupportedNumericalPolicy => {
                                "typed semantic numerical policy is unsupported"
                            }
                            dialect_kernel::SemanticTypedExpressionErrorV1::IncompleteDomain => {
                                "typed semantic operation definedness is incomplete"
                            }
                        })
                    })?;
                }
                let identity = if let Some(identity) = interned.get(&node).copied() {
                    identity
                } else {
                    if nodes.len() == MAX_PLIRON_SEMANTIC_NODES_V1 {
                        return Err(SemanticExpressionBuildErrorV1::ResourceLimit);
                    }
                    let identity = nodes.len();
                    nodes.push(node.clone());
                    interned.insert(node, identity);
                    identity
                };
                let result = definition.deref(context).get_result(0);
                if facts.insert(result, identity) != Some(identity) {
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let mut typed_root_commitments = Vec::new();
        for definition in definitions {
            let operation = Operation::get_op_dyn(*definition, context);
            if is_typed_semantic_definition(&*operation)
                && !facts.contains_key(&definition.deref(context).get_result(0))
            {
                return Err(SemanticExpressionBuildErrorV1::InvalidTypedExpression(
                    "typed semantic SSA node cannot be reconstructed",
                ));
            }
            if let Some(root) = operation.downcast_ref::<SemanticTypedExpressionRootOp>() {
                if !facts.contains_key(&root.result(context)) {
                    return Err(SemanticExpressionBuildErrorV1::InvalidTypedExpression(
                        "typed semantic SSA root cannot be reconstructed",
                    ));
                }
                typed_root_commitments.push(root.commitment(context).ok_or(
                    SemanticExpressionBuildErrorV1::InvalidTypedExpression(
                        "typed semantic root lacks its commitment",
                    ),
                )?);
            }
        }
        Ok(Self {
            nodes,
            facts,
            typed_root_commitments,
        })
    }

    pub(crate) fn identity(&self, value: pliron::value::Value) -> Option<usize> {
        self.facts.get(&value).copied()
    }

    pub(crate) fn equivalent(
        &self,
        actual: pliron::value::Value,
        expected: pliron::value::Value,
    ) -> Option<bool> {
        Some(self.identity(actual)? == self.identity(expected)?)
    }

    pub(crate) fn typed_root_commitments(&self) -> &[[u64; 4]] {
        &self.typed_root_commitments
    }

    fn typed_root_fact(&self, value: pliron::value::Value) -> Option<TypedRootFactV1> {
        match self.nodes.get(self.identity(value)?)? {
            SemanticNodeV1::TypedRoot(expression, contract) => Some(TypedRootFactV1 {
                scalar: expression.scalar(),
                contract: *contract,
            }),
            _ => None,
        }
    }

    pub(crate) fn describe_value(&self, value: pliron::value::Value) -> Option<String> {
        self.identity(value).map(|identity| self.describe(identity))
    }

    fn describe(&self, identity: usize) -> String {
        enum Task {
            Node(usize),
            Static(&'static str),
        }

        let payload_limit = MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1 - 3;
        let mut writer = BoundedSemanticDescriptionWriterV1::new();
        let mut tasks = vec![Task::Node(identity)];
        while let Some(task) = tasks.pop() {
            let rendered = match task {
                Task::Static(text) => writer.write_str(text),
                Task::Node(identity) => match &self.nodes[identity] {
                    SemanticNodeV1::Symbol(symbol) => write!(&mut writer, "s{symbol}"),
                    SemanticNodeV1::Constant(value) => {
                        write!(&mut writer, "c0x{value:016x}")
                    }
                    SemanticNodeV1::Binary(kind, lhs, rhs) => {
                        tasks.push(Task::Static(")"));
                        tasks.push(Task::Node(*rhs));
                        tasks.push(Task::Static(match kind {
                            SemanticBinaryKindAttr::Add => " + ",
                            SemanticBinaryKindAttr::Multiply => " * ",
                        }));
                        tasks.push(Task::Node(*lhs));
                        tasks.push(Task::Static("("));
                        continue;
                    }
                    SemanticNodeV1::Commitment(identity) => write!(
                        &mut writer,
                        "typed-commitment:{:016x}{:016x}{:016x}{:016x}",
                        identity[0], identity[1], identity[2], identity[3]
                    ),
                    SemanticNodeV1::TypedExpression(expression) => {
                        write!(&mut writer, "typed-node:{expression:?}")
                    }
                    SemanticNodeV1::TypedRoot(expression, contract) => {
                        write!(&mut writer, "typed-root:{contract:?}:{expression:?}")
                    }
                },
            };
            if rendered.is_err() {
                break;
            }
            if writer.output.len() == payload_limit && !tasks.is_empty() {
                writer.truncated = true;
                break;
            }
        }
        writer.finish()
    }
}

fn is_typed_semantic_definition(operation: &dyn pliron::op::Op) -> bool {
    operation.downcast_ref::<SemanticTypedSymbolOp>().is_some()
        || operation
            .downcast_ref::<SemanticTypedConstantOp>()
            .is_some()
        || operation.downcast_ref::<SemanticTypedUnaryOp>().is_some()
        || operation.downcast_ref::<SemanticTypedBinaryOp>().is_some()
        || operation.downcast_ref::<SemanticTypedCompareOp>().is_some()
        || operation.downcast_ref::<SemanticTypedSelectOp>().is_some()
        || operation.downcast_ref::<SemanticTypedCastOp>().is_some()
        || operation
            .downcast_ref::<SemanticTypedExpressionRootOp>()
            .is_some()
}

pub(crate) fn is_semantic_refinement_definition_v1(operation: &dyn pliron::op::Op) -> bool {
    operation.downcast_ref::<SemanticSymbolOp>().is_some()
        || operation.downcast_ref::<SemanticConstantOp>().is_some()
        || operation.downcast_ref::<SemanticBinaryOp>().is_some()
        || operation
            .downcast_ref::<SemanticExpressionCommitmentOp>()
            .is_some()
        || is_typed_semantic_definition(operation)
}

pub(crate) fn is_semantic_refinement_contract_v1(operation: &dyn pliron::op::Op) -> bool {
    operation
        .downcast_ref::<TensorResultComponentOp>()
        .is_some()
        || operation.downcast_ref::<RequireEquivalentOp>().is_some()
        || operation.downcast_ref::<RequireRefinementOp>().is_some()
        || operation
            .downcast_ref::<RequireTensorRefinementOp>()
            .is_some()
        || operation
            .downcast_ref::<RequireEffectRefinementOp>()
            .is_some()
        || operation
            .downcast_ref::<RequireNumericalRefinementOp>()
            .is_some()
        || operation.downcast_ref::<ObligationOp>().is_some()
        || operation.downcast_ref::<EvidenceRefOp>().is_some()
        || operation.downcast_ref::<OwnershipContractOp>().is_some()
        || operation.downcast_ref::<RequireFiniteFoldOp>().is_some()
        || operation
            .downcast_ref::<RequireFiniteRecurrenceOp>()
            .is_some()
        || operation
            .downcast_ref::<RequirePermutationGatherOp>()
            .is_some()
}

fn typed_expression(
    nodes: &[SemanticNodeV1],
    identity: Option<usize>,
) -> Option<SemanticTypedExpressionV1> {
    match nodes.get(identity?)? {
        SemanticNodeV1::TypedExpression(expression) => Some(expression.clone()),
        _ => None,
    }
}

fn validate_typed_root(
    expression: &SemanticTypedExpressionV1,
    contract: SemanticNumericalContractV1,
    commitment: [u64; 4],
) -> Result<(), SemanticExpressionBuildErrorV1> {
    expression.validate().map_err(|error| {
        SemanticExpressionBuildErrorV1::InvalidTypedExpression(match error {
            dialect_kernel::SemanticTypedExpressionErrorV1::ResourceLimit => {
                "typed semantic expression exceeds its node or depth bound"
            }
            dialect_kernel::SemanticTypedExpressionErrorV1::TypeMismatch => {
                "typed semantic expression has an invalid type or operator"
            }
            dialect_kernel::SemanticTypedExpressionErrorV1::ConstantOutOfRange => {
                "typed semantic constant exceeds its scalar width"
            }
            dialect_kernel::SemanticTypedExpressionErrorV1::UnsupportedNumericalPolicy => {
                "typed semantic numerical policy is unsupported"
            }
            dialect_kernel::SemanticTypedExpressionErrorV1::IncompleteDomain => {
                "typed semantic operation definedness is incomplete"
            }
        })
    })?;
    expression.validate_static_domains().map_err(|_| {
        SemanticExpressionBuildErrorV1::InvalidTypedExpression(
            "typed semantic operation definedness is incomplete",
        )
    })?;
    contract.validate(expression).map_err(|_| {
        SemanticExpressionBuildErrorV1::InvalidTypedExpression(
            "typed semantic numerical policy does not match the expression",
        )
    })?;
    if digest_words(expression.canonical_transcript_sha256(contract)) != commitment {
        return Err(SemanticExpressionBuildErrorV1::InvalidTypedExpression(
            "typed semantic commitment does not match the reconstructed expression and policy",
        ));
    }
    Ok(())
}

fn digest_words(digest: [u8; 32]) -> [u64; 4] {
    std::array::from_fn(|index| {
        u64::from_le_bytes(
            digest[index * 8..(index + 1) * 8]
                .try_into()
                .expect("digest word has fixed width"),
        )
    })
}
