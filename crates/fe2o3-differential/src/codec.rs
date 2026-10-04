use core::fmt;

use crate::{
    BinaryOp, Expr, KernelCase, MAX_EXPR_DEPTH, MAX_EXPR_NODES, MAX_INPUTS, MAX_WORK_ITEMS,
    ModelError, Program, UnaryOp,
};

const MAGIC: &[u8; 4] = b"F2DF";
const VERSION: u8 = 1;
/// Maximum canonical byte length of one case.
pub const MAX_CANONICAL_BYTES: usize = 16 * 1024;

/// Encodes a valid case into the unique V1 byte representation.
pub fn encode_case_v1(case: &KernelCase) -> Result<Vec<u8>, CodecError> {
    case.validate().map_err(CodecError::InvalidCase)?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    bytes.push(VERSION);
    bytes.extend_from_slice(&case.seed().to_le_bytes());
    bytes.extend_from_slice(&case.program().work_items().to_le_bytes());
    bytes.push(case.program().input_count());
    for input in case.inputs() {
        let length = u16::try_from(input.len()).map_err(|_| CodecError::TooLarge)?;
        bytes.extend_from_slice(&length.to_le_bytes());
        for value in input {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    encode_expr(case.program().expression(), &mut bytes);
    if bytes.len() > MAX_CANONICAL_BYTES {
        return Err(CodecError::TooLarge);
    }
    Ok(bytes)
}

fn encode_expr(expression: &Expr, bytes: &mut Vec<u8>) {
    match expression {
        Expr::Const(value) => {
            bytes.push(0);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        Expr::GlobalId => bytes.push(1),
        Expr::Load { input } => {
            bytes.push(2);
            bytes.push(*input);
        }
        Expr::Unary { op, value } => {
            bytes.push(3);
            bytes.push(match op {
                UnaryOp::Neg => 0,
                UnaryOp::Not => 1,
            });
            encode_expr(value, bytes);
        }
        Expr::Binary { op, left, right } => {
            bytes.push(4);
            bytes.push(match op {
                BinaryOp::Add => 0,
                BinaryOp::Sub => 1,
                BinaryOp::Mul => 2,
                BinaryOp::BitAnd => 3,
                BinaryOp::BitOr => 4,
                BinaryOp::BitXor => 5,
                BinaryOp::Eq => 6,
                BinaryOp::Lt => 7,
            });
            encode_expr(left, bytes);
            encode_expr(right, bytes);
        }
        Expr::Select {
            condition,
            then_value,
            else_value,
        } => {
            bytes.push(5);
            encode_expr(condition, bytes);
            encode_expr(then_value, bytes);
            encode_expr(else_value, bytes);
        }
    }
}

/// Decodes V1 bytes, rejecting oversized, malformed, trailing, and noncanonical input.
pub fn decode_case_v1(bytes: &[u8]) -> Result<KernelCase, CodecError> {
    if bytes.len() > MAX_CANONICAL_BYTES {
        return Err(CodecError::TooLarge);
    }
    let mut reader = Reader::new(bytes);
    if reader.take(4)? != MAGIC {
        return Err(CodecError::BadMagic);
    }
    let version = reader.u8()?;
    if version != VERSION {
        return Err(CodecError::UnsupportedVersion { actual: version });
    }
    let seed = reader.u64()?;
    let work_items = reader.u16()?;
    let input_count = reader.u8()?;
    if usize::from(input_count) > MAX_INPUTS {
        return Err(CodecError::InvalidCase(ModelError::TooManyInputs {
            actual: usize::from(input_count),
        }));
    }
    if work_items == 0 || usize::from(work_items) > MAX_WORK_ITEMS {
        return Err(CodecError::InvalidCase(ModelError::InvalidWorkItemCount {
            actual: usize::from(work_items),
        }));
    }

    let mut inputs = Vec::with_capacity(usize::from(input_count));
    for input in 0..usize::from(input_count) {
        let length = usize::from(reader.u16()?);
        if length != usize::from(work_items) {
            return Err(CodecError::InvalidCase(ModelError::InputLengthMismatch {
                input,
                expected: usize::from(work_items),
                actual: length,
            }));
        }
        let mut values = Vec::with_capacity(length);
        for _ in 0..length {
            values.push(reader.i32()?);
        }
        inputs.push(values);
    }

    let mut nodes = 0;
    let expression = decode_expr(&mut reader, 1, &mut nodes)?;
    if !reader.is_empty() {
        return Err(CodecError::TrailingBytes);
    }
    let program =
        Program::new(input_count, work_items, expression).map_err(CodecError::InvalidCase)?;
    let case = KernelCase::new(seed, program, inputs).map_err(CodecError::InvalidCase)?;
    if encode_case_v1(&case)? != bytes {
        return Err(CodecError::NonCanonical);
    }
    Ok(case)
}

fn decode_expr(
    reader: &mut Reader<'_>,
    depth: usize,
    nodes: &mut usize,
) -> Result<Expr, CodecError> {
    if depth > MAX_EXPR_DEPTH {
        return Err(CodecError::ExpressionTooDeep);
    }
    *nodes += 1;
    if *nodes > MAX_EXPR_NODES {
        return Err(CodecError::ExpressionTooLarge);
    }
    match reader.u8()? {
        0 => Ok(Expr::Const(reader.i32()?)),
        1 => Ok(Expr::GlobalId),
        2 => Ok(Expr::Load {
            input: reader.u8()?,
        }),
        3 => {
            let op = match reader.u8()? {
                0 => UnaryOp::Neg,
                1 => UnaryOp::Not,
                actual => return Err(CodecError::InvalidUnaryTag { actual }),
            };
            Ok(Expr::Unary {
                op,
                value: Box::new(decode_expr(reader, depth + 1, nodes)?),
            })
        }
        4 => {
            let op = match reader.u8()? {
                0 => BinaryOp::Add,
                1 => BinaryOp::Sub,
                2 => BinaryOp::Mul,
                3 => BinaryOp::BitAnd,
                4 => BinaryOp::BitOr,
                5 => BinaryOp::BitXor,
                6 => BinaryOp::Eq,
                7 => BinaryOp::Lt,
                actual => return Err(CodecError::InvalidBinaryTag { actual }),
            };
            Ok(Expr::Binary {
                op,
                left: Box::new(decode_expr(reader, depth + 1, nodes)?),
                right: Box::new(decode_expr(reader, depth + 1, nodes)?),
            })
        }
        5 => Ok(Expr::Select {
            condition: Box::new(decode_expr(reader, depth + 1, nodes)?),
            then_value: Box::new(decode_expr(reader, depth + 1, nodes)?),
            else_value: Box::new(decode_expr(reader, depth + 1, nodes)?),
        }),
        actual => Err(CodecError::InvalidExpressionTag { actual }),
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], CodecError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(CodecError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(CodecError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, CodecError> {
        let mut bytes = [0; 2];
        bytes.copy_from_slice(self.take(2)?);
        Ok(u16::from_le_bytes(bytes))
    }

    fn u64(&mut self) -> Result<u64, CodecError> {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(bytes))
    }

    fn i32(&mut self) -> Result<i32, CodecError> {
        let mut bytes = [0; 4];
        bytes.copy_from_slice(self.take(4)?);
        Ok(i32::from_le_bytes(bytes))
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodecError {
    TooLarge,
    Truncated,
    BadMagic,
    UnsupportedVersion { actual: u8 },
    InvalidExpressionTag { actual: u8 },
    InvalidUnaryTag { actual: u8 },
    InvalidBinaryTag { actual: u8 },
    ExpressionTooLarge,
    ExpressionTooDeep,
    TrailingBytes,
    NonCanonical,
    InvalidCase(ModelError),
}

impl fmt::Display for CodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("case exceeds the canonical byte bound"),
            Self::Truncated => formatter.write_str("case is truncated"),
            Self::BadMagic => formatter.write_str("case has the wrong magic"),
            Self::UnsupportedVersion { actual } => {
                write!(formatter, "case version {actual} is unsupported")
            }
            Self::InvalidExpressionTag { actual } => {
                write!(formatter, "expression tag {actual} is invalid")
            }
            Self::InvalidUnaryTag { actual } => {
                write!(formatter, "unary operation tag {actual} is invalid")
            }
            Self::InvalidBinaryTag { actual } => {
                write!(formatter, "binary operation tag {actual} is invalid")
            }
            Self::ExpressionTooLarge => formatter.write_str("encoded expression is too large"),
            Self::ExpressionTooDeep => formatter.write_str("encoded expression is too deep"),
            Self::TrailingBytes => formatter.write_str("case contains trailing bytes"),
            Self::NonCanonical => formatter.write_str("case is not canonically encoded"),
            Self::InvalidCase(error) => write!(formatter, "case is invalid: {error}"),
        }
    }
}

impl std::error::Error for CodecError {}
