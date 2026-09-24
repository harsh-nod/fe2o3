use super::*;
use crate::production::{
    MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 as MAX_DEPTH,
    MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2 as MAX_NODES, ProductionSemanticLoadV2,
};

impl Wire for Box<ProductionSemanticExpressionV2> {
    fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
        out.retain(size_of::<ProductionSemanticExpressionV2>())?;
        (**self).emit(out)
    }
    fn read<R: Resolver>(
        input: &mut Decoder<'_, '_, '_, R>,
    ) -> Result<Self, DecodeError<R::Error>> {
        input.reserve(size_of::<ProductionSemanticExpressionV2>())?;
        Ok(Box::new(ProductionSemanticExpressionV2::read(input)?))
    }
}

impl Wire for ProductionSemanticLoadV2 {
    fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
        self.block.emit(out)?;
        self.operation.emit(out)?;
        self.scalar.emit(out)?;
        self.allocation_origin.emit(out)?;
        self.view.emit(out)?;
        out.count(self.indices.len(), MAX_RANKED_MEMORY_RANK)?;
        out.retain_array::<ProductionRankedValueV1>(self.indices.len())?;
        for value in &self.indices {
            value.emit(out)?;
        }
        Ok(())
    }
    fn read<R: Resolver>(
        input: &mut Decoder<'_, '_, '_, R>,
    ) -> Result<Self, DecodeError<R::Error>> {
        let block = u32::read(input)?;
        let operation = u32::read(input)?;
        let scalar = ProductionSemanticScalarTypeV2::read(input)?;
        let allocation_origin = u64::read(input)?;
        let view = ProductionRankedValueV1::read(input)?;
        let count = input.count(MAX_RANKED_MEMORY_RANK)?;
        let mut indices = input.vector(count)?;
        for _ in 0..count {
            indices.push(ProductionRankedValueV1::read(input)?);
        }
        Ok(Self {
            block,
            operation,
            scalar,
            allocation_origin,
            view,
            indices: indices.into_boxed_slice(),
        })
    }
}

impl Wire for ProductionSemanticExpressionV2 {
    fn emit(&self, out: &mut Encoder<'_, '_, '_>) -> Result<(), WireError> {
        if out.depth == 0 {
            out.nodes = 0;
        }
        if out.depth >= MAX_DEPTH || out.nodes >= MAX_NODES {
            return Err(WireError::Invalid("expression limit"));
        }
        out.depth += 1;
        out.nodes += 1;
        macro_rules! fields { ($tag:literal; $($value:expr),*) => {{
            ($tag as u8).emit(out)?;
            $($value.emit(out)?;)*
        }}; }
        match self {
            Self::Symbol { symbol, scalar } => fields!(1; symbol, scalar),
            Self::Constant { scalar, bits } => fields!(2; scalar, bits),
            Self::Load(load) => fields!(3; load),
            Self::Unary {
                operation,
                scalar,
                operand,
            } => fields!(4; operation, scalar, operand),
            Self::Binary {
                operation,
                scalar,
                overflow,
                lhs,
                rhs,
            } => fields!(5; operation, scalar, overflow, lhs, rhs),
            Self::Compare {
                operation,
                operand_scalar,
                lhs,
                rhs,
            } => fields!(6; operation, operand_scalar, lhs, rhs),
            Self::Select {
                scalar,
                condition,
                when_true,
                when_false,
            } => fields!(7; scalar, condition, when_true, when_false),
            Self::Cast {
                kind,
                source,
                target,
                operand,
            } => fields!(8; kind, source, target, operand),
        }
        out.depth -= 1;
        Ok(())
    }
    fn read<R: Resolver>(
        input: &mut Decoder<'_, '_, '_, R>,
    ) -> Result<Self, DecodeError<R::Error>> {
        if input.depth == 0 {
            input.nodes = 0;
        }
        if input.depth >= MAX_DEPTH || input.nodes >= MAX_NODES {
            return Err(WireError::Invalid("expression limit").into());
        }
        input.depth += 1;
        input.nodes += 1;
        let value = match u8::read(input)? {
            1 => Self::Symbol {
                symbol: u32::read(input)?,
                scalar: ProductionSemanticScalarTypeV2::read(input)?,
            },
            2 => Self::Constant {
                scalar: ProductionSemanticScalarTypeV2::read(input)?,
                bits: u64::read(input)?,
            },
            3 => Self::Load(ProductionSemanticLoadV2::read(input)?),
            4 => Self::Unary {
                operation: ProductionSemanticUnaryOpV2::read(input)?,
                scalar: ProductionSemanticScalarTypeV2::read(input)?,
                operand: Box::read(input)?,
            },
            5 => Self::Binary {
                operation: ProductionSemanticBinaryOpV2::read(input)?,
                scalar: ProductionSemanticScalarTypeV2::read(input)?,
                overflow: ProductionOverflowContractV2::read(input)?,
                lhs: Box::read(input)?,
                rhs: Box::read(input)?,
            },
            6 => Self::Compare {
                operation: ProductionSemanticComparisonV2::read(input)?,
                operand_scalar: ProductionSemanticScalarTypeV2::read(input)?,
                lhs: Box::read(input)?,
                rhs: Box::read(input)?,
            },
            7 => Self::Select {
                scalar: ProductionSemanticScalarTypeV2::read(input)?,
                condition: Box::read(input)?,
                when_true: Box::read(input)?,
                when_false: Box::read(input)?,
            },
            8 => Self::Cast {
                kind: ProductionSemanticCastV2::read(input)?,
                source: ProductionSemanticScalarTypeV2::read(input)?,
                target: ProductionSemanticScalarTypeV2::read(input)?,
                operand: Box::read(input)?,
            },
            tag => {
                return Err(WireError::UnknownTag {
                    field: "semantic expression",
                    tag,
                }
                .into());
            }
        };
        input.depth -= 1;
        Ok(value)
    }
}
