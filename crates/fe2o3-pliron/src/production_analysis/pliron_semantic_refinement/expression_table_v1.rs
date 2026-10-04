use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::InvocationObserverV1;

type ExpressionObservationV1<'o, 'p, 'r> = Option<(
    ProductionAnalysisResourcePhaseV1,
    &'o InvocationObserverV1<'p, 'r>,
)>;

fn expression_quota_v1(observation: ExpressionObservationV1<'_, '_, '_>, resource: &'static str) {
    if let Some((phase, observer)) = observation {
        observer.deny(ProductionAnalysisResourceLimitV1 { phase, resource });
    }
}

fn map_typed_expression_error_v1(
    error: dialect_kernel::SemanticTypedExpressionErrorV1,
    observation: ExpressionObservationV1<'_, '_, '_>,
) -> SemanticExpressionBuildErrorV1 {
    use dialect_kernel::SemanticTypedExpressionErrorV1 as Error;
    SemanticExpressionBuildErrorV1::InvalidTypedExpression(match error {
        Error::ResourceLimit => {
            expression_quota_v1(observation, "typed semantic expression node/depth limit");
            "typed semantic expression exceeds its node or depth bound"
        }
        Error::TypeMismatch => "typed semantic expression has an invalid type or operator",
        Error::ConstantOutOfRange => "typed semantic constant exceeds its scalar width",
        Error::UnsupportedNumericalPolicy => "typed semantic numerical policy is unsupported",
        Error::IncompleteDomain => "typed semantic operation definedness is incomplete",
    })
}

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
    #[cfg(test)]
    pub(crate) fn from_inventory(
        context: &Context,
        inventory: &BoundedPlironFunctionInventoryV1,
    ) -> Result<Self, SemanticExpressionBuildErrorV1> {
        Self::from_inventory_with_observation_v1(context, inventory, None)
    }

    pub(crate) fn from_inventory_with_observation_v1(
        context: &Context,
        inventory: &BoundedPlironFunctionInventoryV1,
        observation: ExpressionObservationV1<'_, '_, '_>,
    ) -> Result<Self, SemanticExpressionBuildErrorV1> {
        let run = || {
            let definitions = inventory
                .operations()
                .iter()
                .filter_map(|site| {
                    let pointer = site.pointer();
                    let operation = Operation::get_op_dyn(pointer, context);
                    is_semantic_refinement_definition_v1(&*operation).then_some(pointer)
                })
                .collect::<Vec<_>>();
            Self::build_with_observation_v1(context, &definitions, observation)
        };
        match observation {
            None => run(),
            Some((_, observer)) => observer.with_projection(&Ok, |_| run()),
        }
    }

    fn build_with_observation_v1(
        context: &Context,
        definitions: &[pliron::context::Ptr<Operation>],
        observation: ExpressionObservationV1<'_, '_, '_>,
    ) -> Result<Self, SemanticExpressionBuildErrorV1> {
        let run = || {
            if definitions.len() > MAX_PLIRON_SEMANTIC_NODES_V1 {
                expression_quota_v1(observation, "semantic expression definition limit");
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
                    } else if let Some(constant) =
                        operation.downcast_ref::<SemanticTypedConstantOp>()
                    {
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
                        let lhs =
                            typed_expression(&nodes, facts.get(&binary.lhs(context)).copied());
                        let rhs =
                            typed_expression(&nodes, facts.get(&binary.rhs(context)).copied());
                        match (
                            binary.kind(context),
                            binary.scalar(context),
                            binary.overflow(context),
                            lhs,
                            rhs,
                        ) {
                            (
                                Some(operation),
                                Some(scalar),
                                Some(overflow),
                                Some(lhs),
                                Some(rhs),
                            ) => Some(SemanticNodeV1::TypedExpression(
                                SemanticTypedExpressionV1::Binary {
                                    operation,
                                    scalar,
                                    overflow,
                                    lhs: Box::new(lhs),
                                    rhs: Box::new(rhs),
                                },
                            )),
                            _ => None,
                        }
                    } else if let Some(compare) = operation.downcast_ref::<SemanticTypedCompareOp>()
                    {
                        let lhs =
                            typed_expression(&nodes, facts.get(&compare.lhs(context)).copied());
                        let rhs =
                            typed_expression(&nodes, facts.get(&compare.rhs(context)).copied());
                        match (
                            compare.kind(context),
                            compare.operand_scalar(context),
                            lhs,
                            rhs,
                        ) {
                            (Some(operation), Some(operand_scalar), Some(lhs), Some(rhs)) => {
                                Some(SemanticNodeV1::TypedExpression(
                                    SemanticTypedExpressionV1::Compare {
                                        operation,
                                        operand_scalar,
                                        lhs: Box::new(lhs),
                                        rhs: Box::new(rhs),
                                    },
                                ))
                            }
                            _ => None,
                        }
                    } else if let Some(select) = operation.downcast_ref::<SemanticTypedSelectOp>() {
                        let condition = typed_expression(
                            &nodes,
                            facts.get(&select.condition(context)).copied(),
                        );
                        let when_true = typed_expression(
                            &nodes,
                            facts.get(&select.when_true(context)).copied(),
                        );
                        let when_false = typed_expression(
                            &nodes,
                            facts.get(&select.when_false(context)).copied(),
                        );
                        match (select.scalar(context), condition, when_true, when_false) {
                            (Some(scalar), Some(condition), Some(when_true), Some(when_false)) => {
                                Some(SemanticNodeV1::TypedExpression(
                                    SemanticTypedExpressionV1::Select {
                                        scalar,
                                        condition: Box::new(condition),
                                        when_true: Box::new(when_true),
                                        when_false: Box::new(when_false),
                                    },
                                ))
                            }
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
                    } else if let Some(root) =
                        operation.downcast_ref::<SemanticTypedExpressionRootOp>()
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
                                validate_typed_root(
                                    &expression,
                                    contract,
                                    commitment,
                                    observation,
                                )?;
                                Some(SemanticNodeV1::TypedRoot(expression, contract))
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };
                    let Some(node) = node else { continue };
                    if let SemanticNodeV1::TypedExpression(expression) = &node {
                        expression
                            .validate()
                            .map_err(|error| map_typed_expression_error_v1(error, observation))?;
                    }
                    let identity = if let Some(identity) = interned.get(&node).copied() {
                        identity
                    } else {
                        if nodes.len() == MAX_PLIRON_SEMANTIC_NODES_V1 {
                            expression_quota_v1(
                                observation,
                                "semantic expression table node limit",
                            );
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
        };
        match observation {
            None => run(),
            Some((_, observer)) => observer.with_projection(&Ok, |_| run()),
        }
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
    observation: ExpressionObservationV1<'_, '_, '_>,
) -> Result<(), SemanticExpressionBuildErrorV1> {
    expression
        .validate()
        .map_err(|error| map_typed_expression_error_v1(error, observation))?;
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

#[cfg(test)]
mod observed_expression_table_tests {
    use super::*;
    use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
        InvocationReceiptFailureV1 as Failure, InvocationReceiptV1 as Receipt,
    };
    use dialect_kernel::{
        ReturnOp, SemanticOverflowAttr as Overflow, SemanticScalarKindAttr as ScalarKind,
        SemanticTypedBinaryKindAttr as Binary, SemanticTypedUnaryKindAttr as Unary,
    };
    use pliron::{builtin::types::FunctionType, dialect::DialectName, op::Op, value::Value};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    type Phase = ProductionAnalysisResourcePhaseV1;
    type Error = SemanticExpressionBuildErrorV1;
    const QUOTA: &str = "typed semantic expression node/depth limit";
    const BOUNDED: &str = "typed semantic expression exceeds its node or depth bound";

    #[derive(Clone, Copy, Debug)]
    enum Shape {
        Depth(usize),
        Nodes(usize),
        Mismatch,
    }

    fn fixture(shape: Shape) -> (Context, FuncOp, Value) {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let signature = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "observed_expression_table".try_into().unwrap(),
            signature,
        );
        let entry = function.get_entry_block(&context);
        let scalar = SemanticTypedScalarV1::new(ScalarKind::UnsignedInteger, 32).unwrap();
        let symbol = SemanticTypedSymbolOp::new(&mut context, 7, scalar);
        symbol.get_operation().insert_at_back(entry, &context);
        let mut last = symbol.result(&context);
        let (doublings, unaries) = match shape {
            Shape::Depth(depth) => (0, depth - 1),
            Shape::Nodes(nodes) => (12, nodes - 8191),
            Shape::Mismatch => (0, 0),
        };
        for _ in 0..doublings {
            let op = SemanticTypedBinaryOp::new(
                &mut context,
                Binary::Add,
                Overflow::Wrapping,
                scalar,
                last,
                last,
            );
            op.get_operation().insert_at_back(entry, &context);
            last = op.result(&context);
        }
        for _ in 0..unaries {
            let op = SemanticTypedUnaryOp::new(&mut context, Unary::Not, scalar, last);
            op.get_operation().insert_at_back(entry, &context);
            last = op.result(&context);
        }
        if matches!(shape, Shape::Mismatch) {
            let wrong = SemanticTypedScalarV1::new(ScalarKind::UnsignedInteger, 64).unwrap();
            let rhs = SemanticTypedConstantOp::new(&mut context, 4, wrong);
            rhs.get_operation().insert_at_back(entry, &context);
            let rhs = rhs.result(&context);
            let op = SemanticTypedBinaryOp::new(
                &mut context,
                Binary::Add,
                Overflow::Wrapping,
                scalar,
                last,
                rhs,
            );
            op.get_operation().insert_at_back(entry, &context);
            last = op.result(&context);
        }
        ReturnOp::new(&mut context)
            .get_operation()
            .insert_at_back(entry, &context);
        (context, function, last)
    }

    #[test]
    fn actual_boundaries_and_type_error_preserve_none() {
        for shape in [
            Shape::Depth(128),
            Shape::Depth(129),
            Shape::Nodes(8192),
            Shape::Nodes(8193),
            Shape::Mismatch,
        ] {
            let (context, function, last) = fixture(shape);
            let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
            let ordinary = SemanticExpressionTableV1::from_inventory(&context, &inventory);
            let quota = matches!(shape, Shape::Depth(129) | Shape::Nodes(8193));
            let expected_error = if quota {
                Some(Error::InvalidTypedExpression(BOUNDED))
            } else if matches!(shape, Shape::Mismatch) {
                Some(Error::InvalidTypedExpression(
                    "typed semantic expression has an invalid type or operator",
                ))
            } else {
                None
            };
            assert_eq!(
                ordinary.as_ref().err().copied(),
                expected_error,
                "{shape:?}"
            );
            for caller in [Phase::SemanticRefinement, Phase::EffectRefinement] {
                let mut receipt = Receipt::new(
                    Default::default(),
                    ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
                )
                .unwrap();
                let phase = receipt.phase(caller, 0).unwrap();
                let observed = SemanticExpressionTableV1::from_inventory_with_observation_v1(
                    &context,
                    &inventory,
                    Some((caller, &phase.observer(&Ok))),
                );
                assert_eq!(
                    observed.as_ref().err().copied(),
                    expected_error,
                    "{shape:?}"
                );
                if let (Ok(a), Ok(b)) = (&ordinary, &observed) {
                    assert_eq!(
                        (&a.nodes, &a.facts, &a.typed_root_commitments),
                        (&b.nodes, &b.facts, &b.typed_root_commitments)
                    );
                    let SemanticNodeV1::TypedExpression(expression) =
                        &b.nodes[b.identity(last).unwrap()]
                    else {
                        panic!("typed result missing")
                    };
                    let stats = expression.validate().unwrap();
                    match shape {
                        Shape::Depth(depth) => {
                            assert_eq!((stats.nodes, stats.depth), (depth, depth))
                        }
                        Shape::Nodes(nodes) => assert_eq!((stats.nodes, stats.depth), (nodes, 14)),
                        Shape::Mismatch => unreachable!(),
                    }
                }
                drop(observed);
                drop(phase);
                let state = receipt.snapshot();
                let denial = quota.then_some(ProductionAnalysisResourceLimitV1 {
                    phase: caller,
                    resource: QUOTA,
                });
                assert_eq!(state.first_denial, denial);
                assert!(!state.caught_panic);
                // Table events only: no preflight admission or owner transfer.
                assert_eq!(state.committed, Default::default());
                assert_eq!(
                    receipt.complete(),
                    denial.map_or(Ok(state.committed), |error| Err(Failure::Denied(error)))
                );
            }
        }
    }

    #[test]
    fn real_borrow_panic_preserves_first_denial() {
        let (context, function, _) = fixture(Shape::Depth(129));
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        let first = inventory.operations()[0].pointer();
        for caller in [Phase::SemanticRefinement, Phase::EffectRefinement] {
            for through_inventory in [false, true] {
                for seed_quota in [false, true] {
                    let mut receipt = Receipt::new(
                        Default::default(),
                        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
                    )
                    .unwrap();
                    let phase = receipt.phase(caller, 0).unwrap();
                    let observer = phase.observer(&Ok);
                    let observation = Some((caller, &observer));
                    if seed_quota {
                        assert_eq!(
                            SemanticExpressionTableV1::from_inventory_with_observation_v1(
                                &context,
                                &inventory,
                                observation,
                            )
                            .err(),
                            Some(Error::InvalidTypedExpression(BOUNDED))
                        );
                    }
                    assert!(
                        SemanticExpressionTableV1::build_with_observation_v1(
                            &context,
                            &[first],
                            observation,
                        )
                        .is_ok()
                    );
                    let held = first.deref_mut(&context);
                    let outcome = catch_unwind(AssertUnwindSafe(|| {
                        if through_inventory {
                            SemanticExpressionTableV1::from_inventory_with_observation_v1(
                                &context,
                                &inventory,
                                observation,
                            )
                        } else {
                            SemanticExpressionTableV1::build_with_observation_v1(
                                &context,
                                &[first],
                                observation,
                            )
                        }
                    }));
                    assert!(outcome.is_err());
                    drop(held);
                    drop(phase);
                    let state = receipt.snapshot();
                    let denial = seed_quota.then_some(ProductionAnalysisResourceLimitV1 {
                        phase: caller,
                        resource: QUOTA,
                    });
                    assert_eq!(state.first_denial, denial);
                    assert!(state.caught_panic);
                    assert_eq!(state.committed, Default::default());
                    assert_eq!(
                        receipt.complete(),
                        Err(denial.map_or(Failure::CaughtPanic, Failure::Denied))
                    );
                }
            }
        }
    }
}
