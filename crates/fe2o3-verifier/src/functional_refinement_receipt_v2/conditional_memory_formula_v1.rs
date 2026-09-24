//! Borrowed conditional memory view of the checked scalar effect DAG.

use super::*;

/// A borrowed view of the same checked DAG used by the effect proof. It cannot
/// be manufactured by the memory consumer or retained beyond generation.
pub(crate) struct ConditionalMemoryFormulaV1<'a> {
    program: &'a SemanticFormulaProgramV2,
    pairs: &'a [(ProductionRankedValueV1, ProductionRankedValueV1)],
}

pub(crate) enum ConditionalMemoryLeafV1<'a> {
    Node,
    Symbol(u32),
    Load(&'a fe2o3_pliron::ProductionSemanticLoadV2),
}

pub(crate) fn with_conditional_memory_formula_v1<R, E>(
    kernel: &ProductionRankedKernelV1,
    block: usize,
    operation: usize,
    consume: impl FnOnce(&ConditionalMemoryFormulaV1<'_>) -> Result<R, E>,
) -> Result<R, E>
where
    E: From<FunctionalRefinementVerusExecutionErrorV2>,
{
    let pairs = checked_effect_formula_pairs(kernel, block, operation)?;
    let program = SemanticFormulaProgramV2::build(kernel, &pairs)?;
    let view = ConditionalMemoryFormulaV1 {
        program: &program,
        pairs: &pairs,
    };
    view.require_d1_roles()?;
    consume(&view)
}

impl ConditionalMemoryFormulaV1<'_> {
    fn require_d1_roles(&self) -> Result<(), FunctionalRefinementVerusExecutionErrorV2> {
        use fe2o3_pliron::{
            ProductionSemanticExpressionV2 as X, ProductionSemanticScalarTypeV2 as T,
        };
        if self.pairs.len() != 4 {
            return Err(invalid_ranked_recipe());
        }
        for (role, (gpu, reference)) in self.pairs.iter().enumerate().take(3) {
            for root in [gpu, reference] {
                let ProductionRankedValueV1::Local(root) = root else {
                    return Err(invalid_ranked_recipe());
                };
                let Some(SemanticDefinitionV2::TypedExpression(
                    expression,
                    fe2o3_pliron::ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                )) = self.program.definitions.get(root)
                else {
                    return Err(invalid_ranked_recipe());
                };
                let expected = if role == 0 {
                    X::Symbol {
                        symbol: 0,
                        scalar: T::Integer {
                            signed: false,
                            bits: 64,
                        },
                    }
                } else {
                    X::Constant {
                        scalar: T::Bool,
                        bits: 1,
                    }
                };
                if expression != &expected {
                    return Err(invalid_ranked_recipe());
                }
            }
        }
        Ok(())
    }

    pub(crate) fn require_value_width(
        &self,
        bytes: u64,
    ) -> Result<(), FunctionalRefinementVerusExecutionErrorV2> {
        let Some((gpu, reference)) = self.pairs.get(3) else {
            return Err(invalid_ranked_recipe());
        };
        for root in [gpu, reference] {
            let ProductionRankedValueV1::Local(root) = root else {
                return Err(invalid_ranked_recipe());
            };
            let Some(SemanticDefinitionV2::TypedExpression(x, _)) =
                self.program.definitions.get(root)
            else {
                return Err(invalid_ranked_recipe());
            };
            if u64::from(x.scalar().bit_width())
                != bytes.checked_mul(8).ok_or_else(invalid_ranked_recipe)?
            {
                return Err(invalid_ranked_recipe());
            }
        }
        Ok(())
    }

    pub(crate) fn symbols(&self) -> impl Iterator<Item = u32> + '_ {
        self.program.symbols.iter().copied()
    }

    /// Visit every reachable node, including repeated load occurrences. The
    /// caller charges each visit and joins loads to checked executable reads.
    pub(crate) fn visit_leaves<E>(
        &self,
        mut visit: impl FnMut(ConditionalMemoryLeafV1<'_>) -> Result<(), E>,
    ) -> Result<(), E> {
        fn expression<E>(
            x: &fe2o3_pliron::ProductionSemanticExpressionV2,
            visit: &mut impl FnMut(ConditionalMemoryLeafV1<'_>) -> Result<(), E>,
        ) -> Result<(), E> {
            use fe2o3_pliron::ProductionSemanticExpressionV2 as X;
            visit(ConditionalMemoryLeafV1::Node)?;
            match x {
                X::Symbol { symbol, .. } => visit(ConditionalMemoryLeafV1::Symbol(*symbol))?,
                X::Load(load) => visit(ConditionalMemoryLeafV1::Load(load))?,
                X::Constant { .. } => {}
                X::Unary { operand, .. } | X::Cast { operand, .. } => expression(operand, visit)?,
                X::Binary { lhs, rhs, .. } | X::Compare { lhs, rhs, .. } => {
                    expression(lhs, visit)?;
                    expression(rhs, visit)?;
                }
                X::Select {
                    condition,
                    when_true,
                    when_false,
                    ..
                } => {
                    expression(condition, visit)?;
                    expression(when_true, visit)?;
                    expression(when_false, visit)?;
                }
            }
            Ok(())
        }
        for identity in &self.program.order {
            visit(ConditionalMemoryLeafV1::Node)?;
            match &self.program.definitions[identity] {
                SemanticDefinitionV2::Symbol(symbol) => {
                    visit(ConditionalMemoryLeafV1::Symbol(*symbol))?
                }
                SemanticDefinitionV2::TypedExpression(x, _) => expression(x, &mut visit)?,
                _ => {}
            }
        }
        Ok(())
    }

    /// Callable role expressions and the equality lemma share one renderer,
    /// roots, symbol order and numerical interpretation. Ordinary replay bytes
    /// remain unchanged.
    pub(crate) fn render(&self) -> Result<Box<str>, FunctionalRefinementVerusExecutionErrorV2> {
        let mut source = BoundedVerusSourceV2::default();
        self.program
            .write_lemma(&mut source, self.pairs, "fe2o3_conditional_effect_v1", true)?;
        for (role, pair) in self.pairs.iter().enumerate() {
            for (side, root) in [("gpu", pair.0), ("reference", pair.1)] {
                write!(source, "pub open spec fn fe2o3_memory_{side}_role{role}_v1({IEEE_CONGRUENCE_PARAMETER_V2}")
                    .map_err(|_| generated_source_limit())?;
                for symbol in &self.program.symbols {
                    write!(source, ", s{symbol}: int").map_err(|_| generated_source_limit())?;
                }
                source
                    .write_str(") -> int {\n")
                    .map_err(|_| generated_source_limit())?;
                self.program.write_definitions(&mut source)?;
                let ProductionRankedValueV1::Local(root) = root else {
                    return Err(invalid_ranked_recipe());
                };
                writeln!(source, "v{}\n}}", root.get()).map_err(|_| generated_source_limit())?;
            }
        }
        Ok(source.into_string().into_boxed_str())
    }
}

#[cfg(test)]
pub(crate) fn with_conditional_memory_development_v1<R>(
    reads: bool,
    wrong_value: bool,
    consume: impl FnOnce(&ConditionalMemoryFormulaV1<'_>) -> R,
) -> R {
    use fe2o3_pliron::{
        ProductionNumericalContractV2 as N, ProductionOverflowContractV2 as O,
        ProductionSemanticBinaryOpV2 as B, ProductionSemanticExpressionV2 as X,
        ProductionSemanticLoadV2 as L, ProductionSemanticScalarTypeV2 as T,
    };
    let scalar = T::Integer {
        signed: false,
        bits: 32,
    };
    let id = ProductionRankedValueIdV1::new;
    let local = |n| ProductionRankedValueV1::Local(id(n));
    let value = |wrong| {
        let rhs = X::Constant {
            scalar,
            bits: if wrong { 2 } else { 1 },
        };
        if reads {
            X::Binary {
                operation: B::Add,
                scalar,
                overflow: O::Wrapping,
                lhs: Box::new(X::Load(L {
                    block: 0,
                    operation: 5,
                    scalar,
                    allocation_origin: 1,
                    view: local(10),
                    indices: vec![local(11)].into_boxed_slice(),
                })),
                rhs: Box::new(rhs),
            }
        } else {
            rhs
        }
    };
    let definitions: BTreeMap<_, _> = [
        (
            id(0),
            SemanticDefinitionV2::TypedExpression(
                X::Symbol {
                    symbol: 0,
                    scalar: T::Integer {
                        signed: false,
                        bits: 64,
                    },
                },
                N::ExactBitVectorOperatorCongruence,
            ),
        ),
        (
            id(1),
            SemanticDefinitionV2::TypedExpression(
                X::Constant {
                    scalar: T::Bool,
                    bits: 1,
                },
                N::ExactBitVectorOperatorCongruence,
            ),
        ),
        (
            id(2),
            SemanticDefinitionV2::TypedExpression(value(false), N::exact_for(scalar)),
        ),
        (
            id(3),
            SemanticDefinitionV2::TypedExpression(value(wrong_value), N::exact_for(scalar)),
        ),
    ]
    .into_iter()
    .collect();
    let mut symbols = BTreeSet::new();
    for definition in definitions.values() {
        if let SemanticDefinitionV2::TypedExpression(x, _) = definition {
            x.symbols(&mut symbols);
        }
    }
    let program = SemanticFormulaProgramV2 {
        definitions,
        order: vec![id(0), id(1), id(2), id(3)],
        symbols,
    };
    let pairs = [
        (local(0), local(0)),
        (local(1), local(1)),
        (local(1), local(1)),
        (local(2), local(3)),
    ];
    let view = ConditionalMemoryFormulaV1 {
        program: &program,
        pairs: &pairs,
    };
    view.require_d1_roles().unwrap();
    consume(&view)
}
