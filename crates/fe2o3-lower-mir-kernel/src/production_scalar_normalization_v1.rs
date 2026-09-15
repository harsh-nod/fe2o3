#[derive(Clone, Debug, Eq, PartialEq)]
enum NormalizedScalarNodeV1<Child> {
    Symbol {
        symbol: u32,
        scalar: ProductionSemanticScalarTypeV2,
    },
    Constant {
        scalar: ProductionSemanticScalarTypeV2,
        bits: u64,
    },
    Load {
        site: SemanticAccessSiteV1,
        scalar: ProductionSemanticScalarTypeV2,
    },
    Unary {
        operation: ProductionSemanticUnaryOpV2,
        scalar: ProductionSemanticScalarTypeV2,
        operand: Child,
    },
    Binary {
        operation: ProductionSemanticBinaryOpV2,
        scalar: ProductionSemanticScalarTypeV2,
        overflow: ProductionOverflowContractV2,
        lhs: Child,
        rhs: Child,
    },
    Compare {
        operation: ProductionSemanticComparisonV2,
        operand_scalar: ProductionSemanticScalarTypeV2,
        lhs: Child,
        rhs: Child,
    },
    Select {
        scalar: ProductionSemanticScalarTypeV2,
        condition: Child,
        when_true: Child,
        when_false: Child,
    },
    Cast {
        kind: ProductionSemanticCastV2,
        source: ProductionSemanticScalarTypeV2,
        target: ProductionSemanticScalarTypeV2,
        operand: Child,
    },
}

fn normalize_ranked_expression_core_v1<
    'kir,
    'ranked,
    C: ScalarNormalizationContextV1<'kir, 'ranked>,
>(
    expression: &ProductionSemanticExpressionV2,
    depth: usize,
    context: &mut C,
) -> Option<C::Node> {
    context.charge()?;
    if depth > MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 {
        return None;
    }
    let next = depth.checked_add(1)?;
    let node = match expression {
        ProductionSemanticExpressionV2::Symbol { symbol, scalar } => {
            NormalizedScalarNodeV1::Symbol {
                symbol: *symbol,
                scalar: *scalar,
            }
        }
        ProductionSemanticExpressionV2::Constant { scalar, bits } => {
            NormalizedScalarNodeV1::Constant {
                scalar: *scalar,
                bits: *bits,
            }
        }
        ProductionSemanticExpressionV2::Load(load) => {
            let site = context.ranked_site((load.block, load.operation))?;
            let source = context.ranked_source(site)?;
            if source.access != dialect_kernel::AccessKindAttr::Read {
                return None;
            }
            let IndexedRankedAllocationV1::View(source_view) = source.allocation else {
                return None;
            };
            if source_view != load.view {
                return None;
            }
            let ProductionRankedValueV1::Local(view) = source_view else {
                return None;
            };
            let definition = context.ranked_view(view)?;
            if definition.memory_space != dialect_kernel::MemorySpaceAttr::Global
                || definition.allocation_origin != load.allocation_origin
            {
                return None;
            }
            let operation = context.ranked_operation((load.block, load.operation))?;
            let indices = match operation {
                ProductionRankedOperationV1::Access { indices, .. }
                | ProductionRankedOperationV1::ValueAccess { indices, .. }
                | ProductionRankedOperationV1::AtomicAccess { indices, .. }
                | ProductionRankedOperationV1::AtomicValueAccess { indices, .. } => indices,
                _ => return None,
            };
            if indices.as_slice() != load.indices.as_ref() {
                return None;
            }
            NormalizedScalarNodeV1::Load {
                site,
                scalar: load.scalar,
            }
        }
        ProductionSemanticExpressionV2::Unary {
            operation,
            scalar,
            operand,
        } => NormalizedScalarNodeV1::Unary {
            operation: *operation,
            scalar: *scalar,
            operand: normalize_ranked_expression_core_v1(operand, next, context)?,
        },
        ProductionSemanticExpressionV2::Binary {
            operation,
            scalar,
            overflow,
            lhs,
            rhs,
        } => NormalizedScalarNodeV1::Binary {
            operation: *operation,
            scalar: *scalar,
            overflow: *overflow,
            lhs: normalize_ranked_expression_core_v1(lhs, next, context)?,
            rhs: normalize_ranked_expression_core_v1(rhs, next, context)?,
        },
        ProductionSemanticExpressionV2::Compare {
            operation,
            operand_scalar,
            lhs,
            rhs,
        } => NormalizedScalarNodeV1::Compare {
            operation: *operation,
            operand_scalar: *operand_scalar,
            lhs: normalize_ranked_expression_core_v1(lhs, next, context)?,
            rhs: normalize_ranked_expression_core_v1(rhs, next, context)?,
        },
        ProductionSemanticExpressionV2::Select {
            scalar,
            condition,
            when_true,
            when_false,
        } => NormalizedScalarNodeV1::Select {
            scalar: *scalar,
            condition: normalize_ranked_expression_core_v1(condition, next, context)?,
            when_true: normalize_ranked_expression_core_v1(when_true, next, context)?,
            when_false: normalize_ranked_expression_core_v1(when_false, next, context)?,
        },
        ProductionSemanticExpressionV2::Cast {
            kind,
            source,
            target,
            operand,
        } => {
            let operand = normalize_ranked_expression_core_v1(operand, next, context)?;
            if source == target {
                return Some(operand);
            } else {
                NormalizedScalarNodeV1::Cast {
                    kind: *kind,
                    source: *source,
                    target: *target,
                    operand,
                }
            }
        }
    };
    context.emit(node)
}

fn normalize_kir_expression_core_v1<
    'kir,
    'ranked,
    C: ScalarNormalizationContextV1<'kir, 'ranked>,
>(
    value: ValueId,
    depth: usize,
    context: &mut C,
) -> Option<C::Node> {
    context.charge()?;
    if depth > MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 || !context.enter(value)? {
        return None;
    }
    let result = normalize_kir_expression_inner_core_v1(value, depth, context);
    context.leave(value);
    result
}

fn normalize_kir_expression_inner_core_v1<
    'kir,
    'ranked,
    C: ScalarNormalizationContextV1<'kir, 'ranked>,
>(
    value: ValueId,
    depth: usize,
    context: &mut C,
) -> Option<C::Node> {
    let value = context.unique_origin(value)?;
    if let Some((argument, scalar)) = context.parameter(value)? {
        let symbol = PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2.checked_add(argument)?;
        return context.emit(NormalizedScalarNodeV1::Symbol { symbol, scalar });
    }
    let operation = context.operation(value)?;
    let scalar = operation
        .results
        .iter()
        .find(|result| result.id == value)
        .and_then(|result| kir_semantic_scalar_v1(&result.ty))?;
    let next = depth.checked_add(1)?;
    let recurse =
        |operand, context: &mut C| normalize_kir_expression_core_v1(operand, next, context);
    let node = match &operation.kind {
        OperationKind::Constant(constant) => {
            let (constant_scalar, bits) = normalize_kir_constant_v1(constant)?;
            if constant_scalar != scalar {
                return None;
            }
            NormalizedScalarNodeV1::Constant { scalar, bits }
        }
        OperationKind::Unary { op, operand } => NormalizedScalarNodeV1::Unary {
            operation: match op {
                UnaryOp::Not => ProductionSemanticUnaryOpV2::Not,
                UnaryOp::Negate => ProductionSemanticUnaryOpV2::Negate,
            },
            scalar,
            operand: recurse(*operand, context)?,
        },
        OperationKind::Binary { op, lhs, rhs } => {
            let (operation, overflow) = normalize_kir_binary_v1(*op, operation, value)?;
            NormalizedScalarNodeV1::Binary {
                operation,
                scalar,
                overflow,
                lhs: recurse(*lhs, context)?,
                rhs: recurse(*rhs, context)?,
            }
        }
        OperationKind::Compare {
            predicate,
            lhs,
            rhs,
        } => {
            let lhs_scalar = context.scalar(*lhs)?;
            NormalizedScalarNodeV1::Compare {
                operation: normalize_kir_comparison_v1(*predicate),
                operand_scalar: lhs_scalar,
                lhs: recurse(*lhs, context)?,
                rhs: recurse(*rhs, context)?,
            }
        }
        OperationKind::Select {
            condition,
            true_value,
            false_value,
        } => NormalizedScalarNodeV1::Select {
            scalar,
            condition: recurse(*condition, context)?,
            when_true: recurse(*true_value, context)?,
            when_false: recurse(*false_value, context)?,
        },
        OperationKind::Cast { kind, value, to } => {
            let source = context.scalar(*value)?;
            let target = kir_semantic_scalar_v1(to)?;
            let operand = recurse(*value, context)?;
            if source == target {
                return Some(operand);
            } else {
                NormalizedScalarNodeV1::Cast {
                    kind: normalize_kir_cast_v1(*kind, source, target)?,
                    source,
                    target,
                    operand,
                }
            }
        }
        OperationKind::Load { .. } => {
            let site = context.load_site(value)?;
            NormalizedScalarNodeV1::Load { site, scalar }
        }
        _ => return None,
    };
    context.emit(node)
}

// One expression grammar, with legacy boxed trees and paid canonical nodes.
trait ScalarNormalizationContextV1<'kir, 'ranked> {
    type Node;

    fn charge(&mut self) -> Option<()>;
    fn enter(&mut self, value: ValueId) -> Option<bool>;
    fn leave(&mut self, value: ValueId);
    fn emit(&mut self, node: NormalizedScalarNodeV1<Self::Node>) -> Option<Self::Node>;
    fn unique_origin(&mut self, value: ValueId) -> Option<ValueId>;
    fn parameter(
        &mut self,
        value: ValueId,
    ) -> Option<Option<(u32, ProductionSemanticScalarTypeV2)>>;
    fn operation(&mut self, value: ValueId) -> Option<&'kir Operation>;
    fn scalar(&mut self, value: ValueId) -> Option<ProductionSemanticScalarTypeV2>;
    fn load_site(&mut self, value: ValueId) -> Option<SemanticAccessSiteV1>;
    fn ranked_site(&mut self, location: (u32, u32)) -> Option<SemanticAccessSiteV1>;
    fn ranked_source(&mut self, site: SemanticAccessSiteV1) -> Option<IndexedRankedAccessSourceV1>;
    fn ranked_view(&mut self, value: ProductionRankedValueIdV1) -> Option<RankedViewDefinitionV1>;
    fn ranked_operation(
        &mut self,
        location: (u32, u32),
    ) -> Option<&'ranked ProductionRankedOperationV1>;
}

struct LegacyScalarNormalizationV1<'a, 'kir, 'ranked> {
    function: Option<&'kir Function>,
    kir: Option<&'a KirCorrelationIndexV1<'kir>>,
    sites: Option<&'a BTreeMap<(FunctionOperationLocation, u32), SemanticAccessSiteV1>>,
    lowering: Option<&'ranked ProductionRankedKernelLoweringInputV1>,
    ranked: Option<&'a RankedCorrelationIndexV1>,
    visiting: Option<&'a mut BTreeSet<ValueId>>,
    budget: &'a mut UnsupportedIndexCorrelationBudgetV1,
}

impl<'kir, 'ranked> ScalarNormalizationContextV1<'kir, 'ranked>
    for LegacyScalarNormalizationV1<'_, 'kir, 'ranked>
{
    type Node = NormalizedScalarExpressionV1;

    fn charge(&mut self) -> Option<()> {
        self.budget.charge()
    }
    fn enter(&mut self, value: ValueId) -> Option<bool> {
        Some(self.visiting.as_mut()?.insert(value))
    }
    fn leave(&mut self, value: ValueId) {
        if let Some(visiting) = self.visiting.as_mut() {
            visiting.remove(&value);
        }
    }
    fn emit(&mut self, node: NormalizedScalarNodeV1<Self::Node>) -> Option<Self::Node> {
        use NormalizedScalarExpressionV1 as Tree;
        use NormalizedScalarNodeV1 as Node;
        Some(match node {
            Node::Symbol { symbol, scalar } => Tree::Symbol { symbol, scalar },
            Node::Constant { scalar, bits } => Tree::Constant { scalar, bits },
            Node::Load { site, scalar } => Tree::Load { site, scalar },
            Node::Unary {
                operation,
                scalar,
                operand,
            } => Tree::Unary {
                operation,
                scalar,
                operand: Box::new(operand),
            },
            Node::Binary {
                operation,
                scalar,
                overflow,
                lhs,
                rhs,
            } => Tree::Binary {
                operation,
                scalar,
                overflow,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            Node::Compare {
                operation,
                operand_scalar,
                lhs,
                rhs,
            } => Tree::Compare {
                operation,
                operand_scalar,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            Node::Select {
                scalar,
                condition,
                when_true,
                when_false,
            } => Tree::Select {
                scalar,
                condition: Box::new(condition),
                when_true: Box::new(when_true),
                when_false: Box::new(when_false),
            },
            Node::Cast {
                kind,
                source,
                target,
                operand,
            } => Tree::Cast {
                kind,
                source,
                target,
                operand: Box::new(operand),
            },
        })
    }
    fn unique_origin(&mut self, value: ValueId) -> Option<ValueId> {
        unique_kir_ssa_origin_v1(self.kir?, value, self.budget)
    }
    fn parameter(
        &mut self,
        value: ValueId,
    ) -> Option<Option<(u32, ProductionSemanticScalarTypeV2)>> {
        let function = self.function?;
        let body = function.body.as_ref()?;
        let Some(parameter) = body
            .parameters
            .iter()
            .position(|candidate| *candidate == value)
        else {
            return Some(None);
        };
        let scalar = kir_semantic_scalar_v1(function.signature.parameters.get(parameter)?)?;
        Some(Some((u32::try_from(parameter).ok()?, scalar)))
    }
    fn operation(&mut self, value: ValueId) -> Option<&'kir Operation> {
        self.kir?.definitions.get(&value).copied()
    }
    fn scalar(&mut self, value: ValueId) -> Option<ProductionSemanticScalarTypeV2> {
        kir_value_scalar_v1(self.function?, self.kir?, value)
    }
    fn load_site(&mut self, value: ValueId) -> Option<SemanticAccessSiteV1> {
        let location = *self.kir?.definition_locations.get(&value)?;
        self.sites?.get(&(location, 0)).copied()
    }
    fn ranked_site(&mut self, location: (u32, u32)) -> Option<SemanticAccessSiteV1> {
        self.ranked?
            .sites_by_ranked_location
            .get(&location)
            .copied()
    }
    fn ranked_source(&mut self, site: SemanticAccessSiteV1) -> Option<IndexedRankedAccessSourceV1> {
        self.ranked?.sources_by_site.get(&site).copied()
    }
    fn ranked_view(&mut self, value: ProductionRankedValueIdV1) -> Option<RankedViewDefinitionV1> {
        self.ranked?.view_definitions.get(&value).copied()
    }
    fn ranked_operation(
        &mut self,
        location: (u32, u32),
    ) -> Option<&'ranked ProductionRankedOperationV1> {
        self.lowering?
            .kernel()
            .blocks()
            .get(location.0 as usize)?
            .operations()
            .get(location.1 as usize)
    }
}

fn normalize_ranked_expression_v1(
    expression: &ProductionSemanticExpressionV2,
    lowering: &ProductionRankedKernelLoweringInputV1,
    ranked: &RankedCorrelationIndexV1,
    depth: usize,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Option<NormalizedScalarExpressionV1> {
    normalize_ranked_expression_core_v1(
        expression,
        depth,
        &mut LegacyScalarNormalizationV1 {
            function: None,
            kir: None,
            sites: None,
            lowering: Some(lowering),
            ranked: Some(ranked),
            visiting: None,
            budget,
        },
    )
}

fn normalize_kir_expression_v1(
    function: &Function,
    kir: &KirCorrelationIndexV1<'_>,
    semantic_sites: &BTreeMap<(FunctionOperationLocation, u32), SemanticAccessSiteV1>,
    value: ValueId,
    depth: usize,
    visiting: &mut BTreeSet<ValueId>,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Option<NormalizedScalarExpressionV1> {
    normalize_kir_expression_core_v1(
        value,
        depth,
        &mut LegacyScalarNormalizationV1 {
            function: Some(function),
            kir: Some(kir),
            sites: Some(semantic_sites),
            lowering: None,
            ranked: None,
            visiting: Some(visiting),
            budget,
        },
    )
}

trait ScalarSsaOriginWorklistV1 {
    type Error;
    fn pop(&mut self) -> Result<Option<ValueId>, Self::Error>;
    fn first_visit(&mut self, value: ValueId) -> Result<bool, Self::Error>;
    fn expand(&mut self, value: ValueId) -> Result<ScalarSsaOriginStepV1, Self::Error>;
}

enum ScalarSsaOriginStepV1 {
    Origin(ValueId),
    Expanded,
    Unsupported,
}

fn scalar_ssa_origin_worklist_v1<W: ScalarSsaOriginWorklistV1>(
    worklist: &mut W,
) -> Result<Option<ValueId>, W::Error> {
    let mut origin = None;
    while let Some(value) = worklist.pop()? {
        if !worklist.first_visit(value)? {
            continue;
        }
        match worklist.expand(value)? {
            ScalarSsaOriginStepV1::Origin(value) => match origin {
                None => origin = Some(value),
                Some(previous) if previous == value => {}
                Some(_) => return Ok(None),
            },
            ScalarSsaOriginStepV1::Expanded => {}
            ScalarSsaOriginStepV1::Unsupported => return Ok(None),
        }
    }
    Ok(origin)
}

struct LegacyScalarSsaOriginV1<'a, 'kir> {
    kir: &'a KirCorrelationIndexV1<'kir>,
    pending: Vec<ValueId>,
    visited: BTreeSet<ValueId>,
    budget: &'a mut UnsupportedIndexCorrelationBudgetV1,
}
impl ScalarSsaOriginWorklistV1 for LegacyScalarSsaOriginV1<'_, '_> {
    type Error = ();
    fn pop(&mut self) -> Result<Option<ValueId>, ()> {
        let value = self.pending.pop();
        if value.is_some() {
            self.budget.charge().ok_or(())?;
        }
        Ok(value)
    }
    fn first_visit(&mut self, value: ValueId) -> Result<bool, ()> {
        Ok(self.visited.insert(value))
    }
    fn expand(&mut self, value: ValueId) -> Result<ScalarSsaOriginStepV1, ()> {
        if let Some(inputs) = self.kir.block_parameter_inputs.get(&value) {
            if inputs.is_empty() {
                return Ok(ScalarSsaOriginStepV1::Unsupported);
            }
            self.pending.extend(inputs.iter().copied());
            Ok(ScalarSsaOriginStepV1::Expanded)
        } else {
            Ok(ScalarSsaOriginStepV1::Origin(value))
        }
    }
}

fn unique_kir_ssa_origin_v1(
    kir: &KirCorrelationIndexV1<'_>,
    value: ValueId,
    budget: &mut UnsupportedIndexCorrelationBudgetV1,
) -> Option<ValueId> {
    scalar_ssa_origin_worklist_v1(&mut LegacyScalarSsaOriginV1 {
        kir,
        pending: vec![value],
        visited: BTreeSet::new(),
        budget,
    })
    .ok()
    .flatten()
}

struct SourceOutputScalarSsaOriginV1<'a, 'graph, 'ledger, 'limit>(
    SourceOutputAllocationWorklistV1<'a, 'graph, 'ledger, 'limit>,
);
impl ScalarSsaOriginWorklistV1 for SourceOutputScalarSsaOriginV1<'_, '_, '_, '_> {
    type Error = ProductionSourceOutputErrorV1;
    fn pop(&mut self) -> Result<Option<ValueId>, Self::Error> {
        self.0.pop()
    }
    fn first_visit(&mut self, value: ValueId) -> Result<bool, Self::Error> {
        self.0.first_visit(value)
    }
    fn expand(&mut self, value: ValueId) -> Result<ScalarSsaOriginStepV1, Self::Error> {
        self.0
            .budget
            .charge_work(2)
            .map_err(Self::Error::Resource)?;
        let (_, definition) = self
            .0
            .current
            .ok_or(Self::Error::Invalid("scalar current definition absent"))?;
        if definition.value != Some(value) {
            return Err(Self::Error::Invalid("scalar current definition changed"));
        }
        if matches!(
            definition.coordinate,
            fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1::BlockArgument { .. }
        ) {
            // Reuse the existing paid predecessor order, link checks and bounded queue.
            return Ok(match self.0.expand(value)? {
                ExternalAllocationStepV1::Expanded => ScalarSsaOriginStepV1::Expanded,
                _ => ScalarSsaOriginStepV1::Unsupported,
            });
        }
        Ok(ScalarSsaOriginStepV1::Origin(value))
    }
}

fn ranked_view_definition_v1(
    operation: &ProductionRankedOperationV1,
) -> Option<(ProductionRankedValueIdV1, RankedViewDefinitionV1)> {
    match operation {
        ProductionRankedOperationV1::View {
            result,
            allocation_origin,
            ..
        } => Some((
            *result,
            RankedViewDefinitionV1 {
                allocation_origin: *allocation_origin,
                memory_space: dialect_kernel::MemorySpaceAttr::Global,
                noalias_class: 0,
            },
        )),
        ProductionRankedOperationV1::ViewInSpace {
            result,
            memory_space,
            allocation_origin,
            ..
        } => Some((
            *result,
            RankedViewDefinitionV1 {
                allocation_origin: *allocation_origin,
                memory_space: *memory_space,
                noalias_class: 0,
            },
        )),
        _ => None,
    }
}

#[allow(
    clippy::type_complexity,
    reason = "Preserve the existing ranked correlation descriptor without another owned representation"
)]
fn ranked_access_descriptor_v1(
    operation: &ProductionRankedOperationV1,
) -> Option<(
    dialect_kernel::AccessKindAttr,
    IndexedRankedAllocationV1,
    Option<ProductionRankedValueV1>,
    Option<NormalizedAtomicContractV1>,
)> {
    Some(match operation {
        ProductionRankedOperationV1::Access { kind, view, .. } => {
            (*kind, IndexedRankedAllocationV1::View(*view), None, None)
        }
        ProductionRankedOperationV1::PredicatedAccess { kind, view, .. } => {
            (*kind, IndexedRankedAllocationV1::View(*view), None, None)
        }
        ProductionRankedOperationV1::ValueAccess {
            kind, view, value, ..
        } => (
            *kind,
            IndexedRankedAllocationV1::View(*view),
            Some(*value),
            None,
        ),
        ProductionRankedOperationV1::AtomicAccess {
            kind,
            ordering,
            scope,
            view,
            ..
        } => (
            *kind,
            IndexedRankedAllocationV1::View(*view),
            None,
            Some(normalize_ranked_atomic_contract_v1(*ordering, *scope)),
        ),
        ProductionRankedOperationV1::AtomicValueAccess {
            kind,
            ordering,
            scope,
            view,
            value,
            ..
        } => (
            *kind,
            IndexedRankedAllocationV1::View(*view),
            Some(*value),
            Some(normalize_ranked_atomic_contract_v1(*ordering, *scope)),
        ),
        ProductionRankedOperationV1::AllocationEffect {
            kind,
            memory_space,
            allocation_origin,
            noalias_class,
        } => (
            *kind,
            IndexedRankedAllocationV1::Direct(RankedViewDefinitionV1 {
                allocation_origin: *allocation_origin,
                memory_space: *memory_space,
                noalias_class: *noalias_class,
            }),
            None,
            None,
        ),
        _ => return None,
    })
}
