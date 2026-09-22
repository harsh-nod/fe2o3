use super::*;
use crate::{
    MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2, MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2,
    ProductionSemanticLoadV2,
};
use primitives::{add, limit, need, product};

// Each closed table drives both directions; the frozen wire table pins the numbers.
macro_rules! enum_codec {
    ($method:ident, $ty:ident, {$($variant:ident = $tag:literal),+ $(,)?}) => {
        impl Writer<'_, '_> {
            pub fn $method(&mut self, value: $ty) -> R<()> {
                self.u16(match value { $($ty::$variant => $tag),+ })
            }
        }
        impl Reader<'_, '_, '_> {
            pub fn $method(&mut self) -> R<$ty> {
                match self.u16()? {
                    $($tag => Ok($ty::$variant),)+
                    tag => Err(E::Tag { field: stringify!($method), tag }),
                }
            }
        }
    };
}
enum_codec!(access, AccessKindAttr, {Read = 1, Write = 2, AtomicRead = 3, AtomicWrite = 4, AtomicReadModifyWrite = 5});
enum_codec!(coverage, OwnershipCoverageAttr, {ExactView = 1, ExactEffectDomain = 2, TotalView = 3, CollectiveContributions = 4});
enum_codec!(partition, OwnershipPartitionAttr, {ExactSets = 1, DenseRectangles = 2});
enum_codec!(ordering, AtomicOrderingAttr, {Relaxed = 1, Acquire = 2, Release = 3, AcquireRelease = 4, SequentiallyConsistent = 5});
enum_codec!(atomic_scope, AtomicScopeAttr, {SingleThread = 1, Workgroup = 2, Agent = 3, Device = 4, System = 5});
enum_codec!(memory_space, MemorySpaceAttr, {Private = 1, Workgroup = 2, Global = 3});
enum_codec!(index_kind, IndexBinaryKindAttr, {Add = 1, Multiply = 2, Remainder = 3, Divide = 4});
enum_codec!(pipeline_event, PipelineEventKindAttr, {Stage = 1, Commit = 2, Wait = 3, Consume = 4, Discard = 5, Release = 6});
enum_codec!(convergence, TensorConvergenceAttr, {UniformSubgroup = 1, Divergent = 2, UniformWorkgroup = 3, Opaque = 4});
enum_codec!(evaluation_order, SemanticEvaluationOrderAttr, {Ascending = 1, Descending = 2, Lexicographic = 3, Explicit = 4});
enum_codec!(semantic_coverage, SemanticCoverageBindingAttr, {TotalView = 1, CollectiveContributions = 2});
enum_codec!(semantic_binary, SemanticBinaryKindAttr, {Add = 1, Multiply = 2});
enum_codec!(hierarchy, HierarchyAttr, {Grid = 1, Workgroup = 2, Subgroup = 3, Lane = 4});
enum_codec!(address_space, AddressSpaceAttr, {Private = 1, Workgroup = 2, Global = 3, Constant = 4, Generic = 5});
enum_codec!(memory_scope, MemoryScopeAttr, {Subgroup = 1, Workgroup = 2, Device = 3, System = 4});
enum_codec!(memory_order, MemoryOrderAttr, {Acquire = 1, Release = 2, AcquireRelease = 3, SequentiallyConsistent = 4});
enum_codec!(rounding, ProductionIeeeRoundingModeV2, {NearestTiesToEven = 1, TowardZero = 2, TowardPositive = 3, TowardNegative = 4});
enum_codec!(exceptional, ProductionIeeeExceptionalValuePolicyV2, {PreserveExactBits = 1, CanonicalNan = 2});
enum_codec!(overflow, ProductionOverflowContractV2, {Wrapping = 1, Checked = 2});
enum_codec!(unary, ProductionSemanticUnaryOpV2, {Not = 1, Negate = 2});
enum_codec!(binary, ProductionSemanticBinaryOpV2, {Add = 1, Subtract = 2, Multiply = 3, Divide = 4, Remainder = 5, BitXor = 6, BitAnd = 7, BitOr = 8, ShiftLeft = 9, ShiftRight = 10});
enum_codec!(comparison, ProductionSemanticComparisonV2, {Equal = 1, LessThan = 2, LessOrEqual = 3, NotEqual = 4, GreaterOrEqual = 5, GreaterThan = 6});
enum_codec!(cast, ProductionSemanticCastV2, {Integer = 1, IntegerToFloat = 2, FloatToFloat = 3, FloatToIntegerSaturating = 4});
enum_codec!(collective_kind, ProductionCollectiveSemanticKindV1, {FiniteFold = 1, FiniteRecurrence = 2, PermutationGather = 3});

impl Writer<'_, '_> {
    pub fn scalar(&mut self, scalar: Scalar) -> R<()> {
        match scalar {
            Scalar::Bool => self.u16(1),
            Scalar::Integer { signed, bits } => {
                self.u16(2)?;
                self.boolean(signed)?;
                self.u16(bits)
            }
            Scalar::Float { bits } => {
                self.u16(3)?;
                self.u16(bits)
            }
        }
    }
    pub fn numerical(&mut self, numerical: Numerical) -> R<()> {
        match numerical {
            Numerical::ExactBitVectorOperatorCongruence => self.u16(1),
            Numerical::ExactIeee754OperatorCongruence {
                rounding,
                exceptional_values,
            } => {
                self.u16(2)?;
                self.rounding(rounding)?;
                self.exceptional(exceptional_values)
            }
            Numerical::Relaxed => self.u16(3),
            Numerical::ErrorBounded {
                absolute_error_f64_bits,
                relative_error_f64_bits,
            } => {
                self.u16(4)?;
                self.u64(absolute_error_f64_bits)?;
                self.u64(relative_error_f64_bits)
            }
        }
    }
    pub fn expression(&mut self, expression: &Expr) -> R<()> {
        let mut count = 0;
        self.expression_at(expression, 1, &mut count)
    }
    fn expression_at(&mut self, expression: &Expr, depth: usize, count: &mut usize) -> R<()> {
        *count = add(*count, 1)?;
        limit(
            "expression nodes",
            *count,
            MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2,
        )?;
        limit(
            "expression depth",
            depth,
            MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2,
        )?;
        self.shape.expression()?;
        self.budget
            .reserve_storage(size_of::<(&Expr, usize, usize)>())?;
        self.budget.charge_work(1)?;
        let result = (|| {
            match expression {
                Expr::Symbol { symbol, scalar } => {
                    self.u16(1)?;
                    self.u32(*symbol)?;
                    self.scalar(*scalar)?;
                }
                Expr::Constant { scalar, bits } => {
                    self.u16(2)?;
                    self.scalar(*scalar)?;
                    self.u64(*bits)?;
                }
                Expr::Load(load) => {
                    self.u16(3)?;
                    self.u32(load.block)?;
                    self.u32(load.operation)?;
                    self.scalar(load.scalar)?;
                    self.u64(load.allocation_origin)?;
                    self.value(load.view)?;
                    self.values(&load.indices, MAX_RANKED_MEMORY_RANK)?;
                }
                Expr::Unary {
                    operation,
                    scalar,
                    operand,
                } => {
                    self.u16(4)?;
                    self.unary(*operation)?;
                    self.scalar(*scalar)?;
                    self.expression_at(operand, depth + 1, count)?;
                }
                Expr::Binary {
                    operation,
                    scalar,
                    overflow,
                    lhs,
                    rhs,
                } => {
                    self.u16(5)?;
                    self.binary(*operation)?;
                    self.scalar(*scalar)?;
                    self.overflow(*overflow)?;
                    self.expression_at(lhs, depth + 1, count)?;
                    self.expression_at(rhs, depth + 1, count)?;
                }
                Expr::Compare {
                    operation,
                    operand_scalar,
                    lhs,
                    rhs,
                } => {
                    self.u16(6)?;
                    self.comparison(*operation)?;
                    self.scalar(*operand_scalar)?;
                    self.expression_at(lhs, depth + 1, count)?;
                    self.expression_at(rhs, depth + 1, count)?;
                }
                Expr::Select {
                    scalar,
                    condition,
                    when_true,
                    when_false,
                } => {
                    self.u16(7)?;
                    self.scalar(*scalar)?;
                    self.expression_at(condition, depth + 1, count)?;
                    self.expression_at(when_true, depth + 1, count)?;
                    self.expression_at(when_false, depth + 1, count)?;
                }
                Expr::Cast {
                    kind,
                    source,
                    target,
                    operand,
                } => {
                    self.u16(8)?;
                    self.cast(*kind)?;
                    self.scalar(*source)?;
                    self.scalar(*target)?;
                    self.expression_at(operand, depth + 1, count)?;
                }
            }
            Ok(())
        })();
        self.budget
            .release_storage(size_of::<(&Expr, usize, usize)>())?;
        result
    }
}
impl Reader<'_, '_, '_> {
    pub fn scalar(&mut self) -> R<Scalar> {
        match self.u16()? {
            1 => Ok(Scalar::Bool),
            2 => Ok(Scalar::Integer {
                signed: self.boolean()?,
                bits: self.u16()?,
            }),
            3 => Ok(Scalar::Float { bits: self.u16()? }),
            tag => Err(E::Tag {
                field: "scalar",
                tag,
            }),
        }
    }
    pub fn numerical(&mut self) -> R<Numerical> {
        match self.u16()? {
            1 => Ok(Numerical::ExactBitVectorOperatorCongruence),
            2 => Ok(Numerical::ExactIeee754OperatorCongruence {
                rounding: self.rounding()?,
                exceptional_values: self.exceptional()?,
            }),
            3 => Ok(Numerical::Relaxed),
            4 => Ok(Numerical::ErrorBounded {
                absolute_error_f64_bits: self.u64()?,
                relative_error_f64_bits: self.u64()?,
            }),
            tag => Err(E::Tag {
                field: "numerical",
                tag,
            }),
        }
    }
    pub fn expression(&mut self) -> R<Option<Expr>> {
        let mut count = 0;
        self.expression_at(1, &mut count)
    }
    fn boxed(&mut self, expression: Option<Expr>) -> R<Option<Box<Expr>>> {
        if !self.materialize {
            return Ok(None);
        }
        self.budget.reserve_storage(size_of::<Expr>())?;
        self.heap = add(self.heap, size_of::<Expr>())?;
        Ok(Some(Box::new(need(expression)?)))
    }
    fn expression_at(&mut self, depth: usize, count: &mut usize) -> R<Option<Expr>> {
        *count = add(*count, 1)?;
        limit(
            "expression nodes",
            *count,
            MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2,
        )?;
        limit(
            "expression depth",
            depth,
            MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2,
        )?;
        self.shape.expression()?;
        // At most three child results and one returned node coexist in this recursive frame.
        const FRAME: usize = size_of::<([Option<Expr>; 3], Expr, usize, usize)>();
        self.budget.reserve_storage(FRAME)?;
        self.budget.charge_work(1)?;
        let result = (|| match self.u16()? {
            1 => {
                let symbol = self.u32()?;
                let scalar = self.scalar()?;
                self.build(|| Ok(Expr::Symbol { symbol, scalar }))
            }
            2 => {
                let scalar = self.scalar()?;
                let bits = self.u64()?;
                self.build(|| Ok(Expr::Constant { scalar, bits }))
            }
            3 => {
                let block = self.u32()?;
                let operation = self.u32()?;
                let scalar = self.scalar()?;
                let allocation_origin = self.u64()?;
                let view = self.value()?;
                let indices = self.values(MAX_RANKED_MEMORY_RANK)?;
                if !self.materialize {
                    return Ok(None);
                }
                let before = product(indices.capacity(), size_of::<Value>())?;
                let indices = indices.into_boxed_slice();
                let after = product(indices.len(), size_of::<Value>())?;
                let released = before.checked_sub(after).ok_or(Resource::Accounting)?;
                self.heap = self
                    .heap
                    .checked_sub(released)
                    .ok_or(Resource::Accounting)?;
                self.budget.release_storage(released)?;
                Ok(Some(Expr::Load(ProductionSemanticLoadV2 {
                    block,
                    operation,
                    scalar,
                    allocation_origin,
                    view,
                    indices,
                })))
            }
            4 => {
                let operation = self.unary()?;
                let scalar = self.scalar()?;
                let child = self.expression_at(depth + 1, count)?;
                let operand = self.boxed(child)?;
                self.build(|| {
                    Ok(Expr::Unary {
                        operation,
                        scalar,
                        operand: need(operand)?,
                    })
                })
            }
            5 => {
                let operation = self.binary()?;
                let scalar = self.scalar()?;
                let overflow = self.overflow()?;
                let child = self.expression_at(depth + 1, count)?;
                let lhs = self.boxed(child)?;
                let child = self.expression_at(depth + 1, count)?;
                let rhs = self.boxed(child)?;
                self.build(|| {
                    Ok(Expr::Binary {
                        operation,
                        scalar,
                        overflow,
                        lhs: need(lhs)?,
                        rhs: need(rhs)?,
                    })
                })
            }
            6 => {
                let operation = self.comparison()?;
                let operand_scalar = self.scalar()?;
                let child = self.expression_at(depth + 1, count)?;
                let lhs = self.boxed(child)?;
                let child = self.expression_at(depth + 1, count)?;
                let rhs = self.boxed(child)?;
                self.build(|| {
                    Ok(Expr::Compare {
                        operation,
                        operand_scalar,
                        lhs: need(lhs)?,
                        rhs: need(rhs)?,
                    })
                })
            }
            7 => {
                let scalar = self.scalar()?;
                let child = self.expression_at(depth + 1, count)?;
                let condition = self.boxed(child)?;
                let child = self.expression_at(depth + 1, count)?;
                let when_true = self.boxed(child)?;
                let child = self.expression_at(depth + 1, count)?;
                let when_false = self.boxed(child)?;
                self.build(|| {
                    Ok(Expr::Select {
                        scalar,
                        condition: need(condition)?,
                        when_true: need(when_true)?,
                        when_false: need(when_false)?,
                    })
                })
            }
            8 => {
                let kind = self.cast()?;
                let source = self.scalar()?;
                let target = self.scalar()?;
                let child = self.expression_at(depth + 1, count)?;
                let operand = self.boxed(child)?;
                self.build(|| {
                    Ok(Expr::Cast {
                        kind,
                        source,
                        target,
                        operand: need(operand)?,
                    })
                })
            }
            tag => Err(E::Tag {
                field: "expression",
                tag,
            }),
        })();
        self.budget.release_storage(FRAME)?;
        result
    }
}

// Capacity only: callers precharge one visit per node before entering legacy construction.
pub(super) fn expression_heap(expression: &Expr, visits: &mut usize) -> R<usize> {
    *visits = visits.checked_sub(1).ok_or(Resource::Accounting)?;
    let child = |value: &Expr, visits: &mut usize| -> R<usize> {
        add(size_of::<Expr>(), expression_heap(value, visits)?)
    };
    match expression {
        Expr::Symbol { .. } | Expr::Constant { .. } => Ok(0),
        Expr::Load(load) => product(load.indices.len(), size_of::<Value>()),
        Expr::Unary { operand, .. } | Expr::Cast { operand, .. } => child(operand, visits),
        Expr::Binary { lhs, rhs, .. } | Expr::Compare { lhs, rhs, .. } => {
            add(child(lhs, visits)?, child(rhs, visits)?)
        }
        Expr::Select {
            condition,
            when_true,
            when_false,
            ..
        } => add(
            add(child(condition, visits)?, child(when_true, visits)?)?,
            child(when_false, visits)?,
        ),
    }
}
