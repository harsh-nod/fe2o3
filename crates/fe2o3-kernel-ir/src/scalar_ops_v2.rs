//! Bounded target-neutral scalar contracts for Kernel IR V2.
//!
//! The schema is public and can be carried through kernel IR as a reserved,
//! canonical call. Constructing a carrier proves structural well-formedness;
//! target admission and backend semantics remain separate obligations.

use std::fmt::{self, Write as _};

use crate::{
    Function, FunctionId, Operation as KernelOperation, OperationKind, ScalarType as IrScalarType,
    Signature, Type, ValueDef, ValueId,
};

pub const MAGIC: [u8; 8] = *b"FE2OSV2\0";
pub const VERSION: u16 = 3;
pub const MAX_ENCODED_BYTES: usize = 96;
pub const MAX_DIAGNOSTICS: usize = 16;
pub const INTRINSIC_PREFIX: &str = "__fe2o3_ir_scalar_v2_";
pub const MAX_INTRINSIC_SYMBOL_BYTES: usize = INTRINSIC_PREFIX.len() + MAX_ENCODED_BYTES * 2;
pub const GFX942_FLOAT_CAPABILITIES: FloatCapabilities =
    FloatCapabilities::new(false, true, true, false);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IntWidth {
    W8,
    W16,
    W32,
    W64,
    W128,
}
impl IntWidth {
    pub const fn bits(self) -> u16 {
        match self {
            Self::W8 => 8,
            Self::W16 => 16,
            Self::W32 => 32,
            Self::W64 => 64,
            Self::W128 => 128,
        }
    }
    const fn tag(self) -> u8 {
        match self {
            Self::W8 => 1,
            Self::W16 => 2,
            Self::W32 => 3,
            Self::W64 => 4,
            Self::W128 => 5,
        }
    }
    const fn from_tag(t: u8) -> Option<Self> {
        match t {
            1 => Some(Self::W8),
            2 => Some(Self::W16),
            3 => Some(Self::W32),
            4 => Some(Self::W64),
            5 => Some(Self::W128),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FloatWidth {
    F16,
    F32,
    F64,
    F128,
}
impl FloatWidth {
    pub const fn bits(self) -> u16 {
        match self {
            Self::F16 => 16,
            Self::F32 => 32,
            Self::F64 => 64,
            Self::F128 => 128,
        }
    }
    const fn tag(self) -> u8 {
        match self {
            Self::F16 => 1,
            Self::F32 => 2,
            Self::F64 => 3,
            Self::F128 => 4,
        }
    }
    const fn from_tag(t: u8) -> Option<Self> {
        match t {
            1 => Some(Self::F16),
            2 => Some(Self::F32),
            3 => Some(Self::F64),
            4 => Some(Self::F128),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScalarType {
    Bool,
    Char,
    Int { width: IntWidth, signed: bool },
    Float(FloatWidth),
    Pointer { address_space: u16, width: IntWidth },
}
impl ScalarType {
    pub const fn bit_width(self) -> u16 {
        match self {
            Self::Bool => 1,
            Self::Char => 32,
            Self::Int { width, .. } | Self::Pointer { width, .. } => width.bits(),
            Self::Float(w) => w.bits(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FloatCapabilities(u8);
impl FloatCapabilities {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(15);
    pub const fn new(f16: bool, f32: bool, f64: bool, f128: bool) -> Self {
        Self(f16 as u8 | ((f32 as u8) << 1) | ((f64 as u8) << 2) | ((f128 as u8) << 3))
    }
    pub const fn supports(self, w: FloatWidth) -> bool {
        self.0
            & (1 << match w {
                FloatWidth::F16 => 0,
                FloatWidth::F32 => 1,
                FloatWidth::F64 => 2,
                FloatWidth::F128 => 3,
            })
            != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IntMode {
    Checked,
    Wrapping,
    Overflowing,
    Saturating,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IntBinary {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    And,
    Or,
    Xor,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IntUnary {
    Neg,
    Not,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ShiftDirection {
    Left,
    Right,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ShiftPolicy {
    /// Rust's checked shift methods: an invalid RHS returns no value.
    Checked,
    /// Rust's wrapping shift methods: the RHS bit pattern is reduced modulo the LHS width.
    Wrapping,
    /// Rust's overflowing shift methods: wrapping value plus an invalid-RHS flag.
    Overflowing,
    /// A source `<<` or `>>`; invalid RHS values trap only when overflow checks are enabled.
    RustOperator { overflow_checks: bool },
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Predicate {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FloatBinary {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FloatArithmeticSemantics {
    RustIeee754,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FloatComparisonPolicy {
    /// Rust `PartialEq`; valid only for `Eq` and `Ne`.
    RustPartialEq,
    /// Rust `PartialOrd`; valid only for the four ordering predicates.
    RustPartialOrd,
    /// IEEE ordered predicate: every predicate is false when either input is NaN.
    IeeeOrdered,
    /// IEEE unordered predicate: every predicate is true when either input is NaN.
    IeeeUnordered,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum FloatToIntSemantics {
    RustSaturatingAs,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IntToFloatSemantics {
    RustAs,
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum UnsafeProvenancePolicy {
    ExplicitProvenanceLoss,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Cast {
    IntExtend {
        signed: bool,
    },
    IntNarrow,
    FloatExtend,
    FloatNarrow,
    IntToFloat {
        semantics: IntToFloatSemantics,
    },
    FloatToInt {
        semantics: FloatToIntSemantics,
    },
    BoolToInt,
    IntToBoolChecked,
    CharToInt,
    IntToCharChecked,
    Bitcast,
    PointerAddressSpace,
    PointerToInt {
        unsafe_policy: UnsafeProvenancePolicy,
    },
    IntToPointer {
        unsafe_policy: UnsafeProvenancePolicy,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Operation {
    IntegerBinary {
        ty: ScalarType,
        op: IntBinary,
        mode: IntMode,
    },
    IntegerUnary {
        ty: ScalarType,
        op: IntUnary,
        mode: IntMode,
    },
    Shift {
        ty: ScalarType,
        rhs_ty: ScalarType,
        direction: ShiftDirection,
        policy: ShiftPolicy,
    },
    IntegerCompare {
        ty: ScalarType,
        predicate: Predicate,
    },
    FloatBinary {
        ty: ScalarType,
        op: FloatBinary,
        semantics: FloatArithmeticSemantics,
    },
    FloatNeg {
        ty: ScalarType,
        semantics: FloatArithmeticSemantics,
    },
    FloatCompare {
        ty: ScalarType,
        predicate: Predicate,
        policy: FloatComparisonPolicy,
    },
    FloatTotalCompare {
        /// The result is a three-way `Ordering`, never a boolean predicate.
        ty: ScalarType,
    },
    Cast {
        from: ScalarType,
        to: ScalarType,
        cast: Cast,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Diagnostic {
    ResourceLimit {
        resource: &'static str,
        limit: usize,
        actual: usize,
    },
    NonIntegerType,
    NonShiftRhsIntegerType,
    NonFloatType,
    UnsupportedFloat(FloatWidth),
    InvalidIntMode,
    InvalidUnary,
    InvalidFloatComparison,
    InvalidCast,
    WidthRelation,
    SignednessMismatch,
    BitcastWidthMismatch {
        from: u16,
        to: u16,
    },
    PointerWidthMismatch,
    UnsupportedCarrierType(ScalarType),
    UnsupportedProvenance,
}
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

pub fn verify(op: Operation, caps: FloatCapabilities) -> Result<(), Vec<Diagnostic>> {
    let mut ds = Vec::new();
    match op {
        Operation::IntegerBinary { ty, op, mode } => {
            if int_parts(ty).is_none() {
                ds.push(Diagnostic::NonIntegerType)
            }
            if matches!(op, IntBinary::And | IntBinary::Or | IntBinary::Xor)
                && mode != IntMode::Wrapping
            {
                ds.push(Diagnostic::InvalidIntMode)
            }
            if op == IntBinary::Rem && mode == IntMode::Saturating {
                ds.push(Diagnostic::InvalidIntMode)
            }
        }
        Operation::IntegerUnary { ty, op, mode } => match int_parts(ty) {
            None => ds.push(Diagnostic::NonIntegerType),
            Some((_, signed)) => {
                if (op == IntUnary::Neg && !signed)
                    || (op == IntUnary::Not && mode != IntMode::Wrapping)
                {
                    ds.push(Diagnostic::InvalidUnary)
                }
            }
        },
        Operation::Shift { ty, rhs_ty, .. } => {
            if int_parts(ty).is_none() {
                ds.push(Diagnostic::NonIntegerType)
            }
            if int_parts(rhs_ty).is_none() {
                ds.push(Diagnostic::NonShiftRhsIntegerType)
            }
        }
        Operation::IntegerCompare { ty, .. } => {
            if int_parts(ty).is_none() {
                ds.push(Diagnostic::NonIntegerType)
            }
        }
        Operation::FloatBinary { ty, .. }
        | Operation::FloatNeg { ty, .. }
        | Operation::FloatTotalCompare { ty }
        | Operation::FloatCompare { ty, .. } => {
            check_float(ty, caps, &mut ds);
            if let Operation::FloatCompare {
                predicate, policy, ..
            } = op
                && !valid_float_comparison(predicate, policy)
            {
                ds.push(Diagnostic::InvalidFloatComparison)
            }
        }
        Operation::Cast { from, to, cast } => verify_cast(from, to, cast, caps, &mut ds),
    }
    if ds.len() > MAX_DIAGNOSTICS {
        ds.truncate(MAX_DIAGNOSTICS)
    }
    if ds.is_empty() { Ok(()) } else { Err(ds) }
}

const fn valid_float_comparison(predicate: Predicate, policy: FloatComparisonPolicy) -> bool {
    match policy {
        FloatComparisonPolicy::RustPartialEq => {
            matches!(predicate, Predicate::Eq | Predicate::Ne)
        }
        FloatComparisonPolicy::RustPartialOrd => {
            matches!(
                predicate,
                Predicate::Lt | Predicate::Le | Predicate::Gt | Predicate::Ge
            )
        }
        FloatComparisonPolicy::IeeeOrdered | FloatComparisonPolicy::IeeeUnordered => true,
    }
}

fn verify_cast(
    from: ScalarType,
    to: ScalarType,
    cast: Cast,
    caps: FloatCapabilities,
    ds: &mut Vec<Diagnostic>,
) {
    let valid = match cast {
        Cast::IntExtend { signed } => match (int_parts(from), int_parts(to)) {
            (Some((a, sa)), Some((b, _))) => {
                if a.bits() >= b.bits() {
                    ds.push(Diagnostic::WidthRelation)
                }
                if sa != signed {
                    ds.push(Diagnostic::SignednessMismatch)
                }
                a.bits() < b.bits() && sa == signed
            }
            _ => false,
        },
        Cast::IntNarrow => match (int_parts(from), int_parts(to)) {
            (Some((a, _)), Some((b, _))) => {
                if a.bits() <= b.bits() {
                    ds.push(Diagnostic::WidthRelation)
                }
                a.bits() > b.bits()
            }
            _ => false,
        },
        Cast::FloatExtend => float_relation(from, to, true, caps, ds),
        Cast::FloatNarrow => float_relation(from, to, false, caps, ds),
        Cast::IntToFloat { .. } => int_parts(from).is_some() && float_ok(to, caps, ds),
        Cast::FloatToInt { .. } => float_ok(from, caps, ds) && int_parts(to).is_some(),
        Cast::BoolToInt => from == ScalarType::Bool && int_parts(to).is_some(),
        Cast::IntToBoolChecked => int_parts(from).is_some() && to == ScalarType::Bool,
        Cast::CharToInt => from == ScalarType::Char && int_parts(to).is_some(),
        Cast::IntToCharChecked => {
            matches!(int_parts(from),Some((w,false))if w.bits()>=32) && to == ScalarType::Char
        }
        Cast::Bitcast => {
            if from.bit_width() != to.bit_width() {
                ds.push(Diagnostic::BitcastWidthMismatch {
                    from: from.bit_width(),
                    to: to.bit_width(),
                });
                false
            } else {
                !matches!(
                    from,
                    ScalarType::Bool | ScalarType::Char | ScalarType::Pointer { .. }
                ) && !matches!(
                    to,
                    ScalarType::Bool | ScalarType::Char | ScalarType::Pointer { .. }
                )
            }
        }
        Cast::PointerAddressSpace => pointer_pair(from, to, ds),
        Cast::PointerToInt { .. } => match (from, int_parts(to)) {
            (ScalarType::Pointer { width, .. }, Some((w, _))) => same_pointer_width(width, w, ds),
            _ => false,
        },
        Cast::IntToPointer { .. } => match (int_parts(from), to) {
            (Some((w, _)), ScalarType::Pointer { width, .. }) => same_pointer_width(width, w, ds),
            _ => false,
        },
    };
    if !valid {
        ds.push(Diagnostic::InvalidCast)
    }
}
fn int_parts(t: ScalarType) -> Option<(IntWidth, bool)> {
    match t {
        ScalarType::Int { width, signed } => Some((width, signed)),
        _ => None,
    }
}
fn check_float(t: ScalarType, c: FloatCapabilities, ds: &mut Vec<Diagnostic>) {
    if !float_ok(t, c, ds) && !matches!(t, ScalarType::Float(_)) {
        ds.push(Diagnostic::NonFloatType)
    }
}
fn float_ok(t: ScalarType, c: FloatCapabilities, ds: &mut Vec<Diagnostic>) -> bool {
    match t {
        ScalarType::Float(w) => {
            if !c.supports(w) {
                ds.push(Diagnostic::UnsupportedFloat(w));
                false
            } else {
                true
            }
        }
        _ => false,
    }
}
fn float_relation(
    a: ScalarType,
    b: ScalarType,
    up: bool,
    c: FloatCapabilities,
    ds: &mut Vec<Diagnostic>,
) -> bool {
    let (ScalarType::Float(x), ScalarType::Float(y)) = (a, b) else {
        return false;
    };
    let cap = float_ok(a, c, ds) & float_ok(b, c, ds);
    let rel = if up {
        x.bits() < y.bits()
    } else {
        x.bits() > y.bits()
    };
    if !rel {
        ds.push(Diagnostic::WidthRelation)
    }
    cap && rel
}
fn same_pointer_width(a: IntWidth, b: IntWidth, ds: &mut Vec<Diagnostic>) -> bool {
    if a != b {
        ds.push(Diagnostic::PointerWidthMismatch)
    }
    a == b
}
fn pointer_pair(a: ScalarType, b: ScalarType, ds: &mut Vec<Diagnostic>) -> bool {
    match (a, b) {
        (ScalarType::Pointer { width: x, .. }, ScalarType::Pointer { width: y, .. }) => {
            same_pointer_width(x, y, ds)
        }
        _ => false,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodeError {
    ResourceLimit { limit: usize, actual: usize },
    Truncated,
    TrailingBytes,
    InvalidMagic,
    UnknownVersion(u16),
    ReservedNonZero,
    UnknownTag { field: &'static str, tag: u8 },
    Invalid(Vec<Diagnostic>),
}
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Identity(Vec<u8>);
impl Identity {
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.0
    }
}
pub fn identity(op: Operation, c: FloatCapabilities) -> Result<Identity, Vec<Diagnostic>> {
    encode(op, c).map(Identity)
}

pub fn encode(op: Operation, caps: FloatCapabilities) -> Result<Vec<u8>, Vec<Diagnostic>> {
    verify(op, caps)?;
    let mut p = Vec::with_capacity(12);
    encode_op(op, &mut p);
    let total = 16 + p.len();
    if total > MAX_ENCODED_BYTES {
        return Err(vec![Diagnostic::ResourceLimit {
            resource: "encoded bytes",
            limit: MAX_ENCODED_BYTES,
            actual: total,
        }]);
    }
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&(p.len() as u16).to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&p);
    Ok(out)
}
pub fn decode(bytes: &[u8], caps: FloatCapabilities) -> Result<Operation, DecodeError> {
    if bytes.len() > MAX_ENCODED_BYTES {
        return Err(DecodeError::ResourceLimit {
            limit: MAX_ENCODED_BYTES,
            actual: bytes.len(),
        });
    }
    if bytes.len() < 16 {
        return Err(DecodeError::Truncated);
    }
    if bytes[..8] != MAGIC {
        return Err(DecodeError::InvalidMagic);
    }
    let v = u16::from_le_bytes([bytes[8], bytes[9]]);
    if v != VERSION {
        return Err(DecodeError::UnknownVersion(v));
    }
    let n = u16::from_le_bytes([bytes[10], bytes[11]]) as usize;
    if bytes[12..16] != [0; 4] {
        return Err(DecodeError::ReservedNonZero);
    }
    let end = 16usize.checked_add(n).ok_or(DecodeError::ResourceLimit {
        limit: MAX_ENCODED_BYTES,
        actual: usize::MAX,
    })?;
    if bytes.len() < end {
        return Err(DecodeError::Truncated);
    }
    if bytes.len() > end {
        return Err(DecodeError::TrailingBytes);
    }
    let mut d = Decoder {
        b: &bytes[16..],
        at: 0,
    };
    let op = decode_op(&mut d)?;
    if d.at != d.b.len() {
        return Err(DecodeError::TrailingBytes);
    }
    verify(op, caps).map_err(DecodeError::Invalid)?;
    Ok(op)
}

/// A closed scalar operation plus its SSA operands.
///
/// The operation is encoded into a reserved function identity. Backends must
/// decode and revalidate that identity; the spelling alone grants no authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarOperationV2 {
    operation: Operation,
    arguments: Vec<ValueId>,
}

impl ScalarOperationV2 {
    pub fn new(operation: Operation, arguments: Vec<ValueId>) -> Result<Self, Vec<Diagnostic>> {
        verify(operation, GFX942_FLOAT_CAPABILITIES)?;
        if operation_uses_pointer(operation) {
            return Err(vec![Diagnostic::UnsupportedProvenance]);
        }
        let expected = operand_types(operation)?;
        if arguments.len() != expected.len() {
            return Err(vec![Diagnostic::ResourceLimit {
                resource: "scalar operands",
                limit: expected.len(),
                actual: arguments.len(),
            }]);
        }
        Ok(Self {
            operation,
            arguments,
        })
    }

    pub const fn operation(&self) -> Operation {
        self.operation
    }

    pub fn arguments(&self) -> &[ValueId] {
        &self.arguments
    }

    pub fn operand_types(&self) -> Vec<Type> {
        operand_types(self.operation).expect("validated carrier types")
    }

    pub fn result_types(&self) -> Vec<Type> {
        result_types(self.operation).expect("validated carrier types")
    }

    pub fn intrinsic_function_id(&self) -> FunctionId {
        intrinsic_function_id(self.operation).expect("validated operation has canonical bytes")
    }

    pub fn declaration(&self) -> Function {
        Function::external_import(
            self.intrinsic_function_id(),
            Signature::new(self.operand_types(), self.result_types()),
        )
    }

    pub fn kernel_operation(
        &self,
        results: &[ValueId],
    ) -> Result<KernelOperation, Vec<Diagnostic>> {
        let result_types = self.result_types();
        if results.len() != result_types.len() {
            return Err(vec![Diagnostic::ResourceLimit {
                resource: "scalar results",
                limit: result_types.len(),
                actual: results.len(),
            }]);
        }
        Ok(KernelOperation::new(
            results
                .iter()
                .copied()
                .zip(result_types)
                .map(|(id, ty)| ValueDef::new(id, ty))
                .collect(),
            OperationKind::Call {
                callee: self.intrinsic_function_id(),
                arguments: self.arguments.clone(),
            },
        ))
    }

    pub fn from_intrinsic_call(callee: &FunctionId, arguments: &[ValueId]) -> Option<Self> {
        Self::new(operation_from_intrinsic_id(callee)?, arguments.to_vec()).ok()
    }
}

pub fn operation_from_intrinsic_id(id: &FunctionId) -> Option<Operation> {
    let encoded = id.as_str().strip_prefix(INTRINSIC_PREFIX)?;
    if encoded.is_empty()
        || encoded.len() % 2 != 0
        || id.as_str().len() > MAX_INTRINSIC_SYMBOL_BYTES
    {
        return None;
    }
    let mut bytes = Vec::with_capacity(encoded.len() / 2);
    for pair in encoded.as_bytes().chunks_exact(2) {
        bytes.push((hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?);
    }
    let operation = decode(&bytes, GFX942_FLOAT_CAPABILITIES).ok()?;
    (encode(operation, GFX942_FLOAT_CAPABILITIES)
        .ok()?
        .as_slice()
        == bytes.as_slice())
    .then_some(operation)
}

fn intrinsic_function_id(operation: Operation) -> Result<FunctionId, Vec<Diagnostic>> {
    let bytes = encode(operation, GFX942_FLOAT_CAPABILITIES)?;
    let mut symbol = String::with_capacity(INTRINSIC_PREFIX.len() + bytes.len() * 2);
    symbol.push_str(INTRINSIC_PREFIX);
    for byte in bytes {
        write!(&mut symbol, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(FunctionId::new(symbol))
}

const fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn operation_uses_pointer(operation: Operation) -> bool {
    match operation {
        Operation::Cast { from, to, .. } => {
            matches!(from, ScalarType::Pointer { .. }) || matches!(to, ScalarType::Pointer { .. })
        }
        Operation::IntegerBinary { .. }
        | Operation::IntegerUnary { .. }
        | Operation::Shift { .. }
        | Operation::IntegerCompare { .. }
        | Operation::FloatBinary { .. }
        | Operation::FloatNeg { .. }
        | Operation::FloatCompare { .. }
        | Operation::FloatTotalCompare { .. } => false,
    }
}

fn operand_types(operation: Operation) -> Result<Vec<Type>, Vec<Diagnostic>> {
    let types = match operation {
        Operation::IntegerBinary { ty, .. }
        | Operation::IntegerCompare { ty, .. }
        | Operation::FloatBinary { ty, .. }
        | Operation::FloatCompare { ty, .. }
        | Operation::FloatTotalCompare { ty } => vec![carrier_type(ty)?, carrier_type(ty)?],
        Operation::IntegerUnary { ty, .. } | Operation::FloatNeg { ty, .. } => {
            vec![carrier_type(ty)?]
        }
        Operation::Shift { ty, rhs_ty, .. } => vec![carrier_type(ty)?, carrier_type(rhs_ty)?],
        Operation::Cast { from, .. } => vec![carrier_type(from)?],
    };
    Ok(types)
}

fn result_types(operation: Operation) -> Result<Vec<Type>, Vec<Diagnostic>> {
    let types = match operation {
        Operation::IntegerBinary { ty, mode, .. } => match mode {
            IntMode::Checked | IntMode::Overflowing => vec![carrier_type(ty)?, Type::BOOL],
            IntMode::Wrapping | IntMode::Saturating => vec![carrier_type(ty)?],
        },
        Operation::IntegerUnary { ty, mode, .. } => match mode {
            IntMode::Checked | IntMode::Overflowing => vec![carrier_type(ty)?, Type::BOOL],
            IntMode::Wrapping | IntMode::Saturating => vec![carrier_type(ty)?],
        },
        Operation::Shift { ty, policy, .. } => match policy {
            ShiftPolicy::Checked | ShiftPolicy::Overflowing => {
                vec![carrier_type(ty)?, Type::BOOL]
            }
            ShiftPolicy::Wrapping | ShiftPolicy::RustOperator { .. } => vec![carrier_type(ty)?],
        },
        Operation::IntegerCompare { .. } | Operation::FloatCompare { .. } => vec![Type::BOOL],
        Operation::FloatBinary { ty, .. } | Operation::FloatNeg { ty, .. } => {
            vec![carrier_type(ty)?]
        }
        Operation::FloatTotalCompare { .. } => vec![Type::Scalar(IrScalarType::I8)],
        Operation::Cast { to, cast, .. } => match cast {
            Cast::IntToBoolChecked | Cast::IntToCharChecked => {
                vec![carrier_type(to)?, Type::BOOL]
            }
            _ => vec![carrier_type(to)?],
        },
    };
    Ok(types)
}

fn carrier_type(ty: ScalarType) -> Result<Type, Vec<Diagnostic>> {
    let scalar = match ty {
        ScalarType::Bool => IrScalarType::Bool,
        ScalarType::Char => IrScalarType::U32,
        ScalarType::Int { width, signed } => match (width, signed) {
            (IntWidth::W8, true) => IrScalarType::I8,
            (IntWidth::W16, true) => IrScalarType::I16,
            (IntWidth::W32, true) => IrScalarType::I32,
            (IntWidth::W64, true) => IrScalarType::I64,
            (IntWidth::W128, true) => IrScalarType::I128,
            (IntWidth::W8, false) => IrScalarType::U8,
            (IntWidth::W16, false) => IrScalarType::U16,
            (IntWidth::W32, false) => IrScalarType::U32,
            (IntWidth::W64, false) => IrScalarType::U64,
            (IntWidth::W128, false) => IrScalarType::U128,
        },
        ScalarType::Float(FloatWidth::F32) => IrScalarType::F32,
        ScalarType::Float(FloatWidth::F64) => IrScalarType::F64,
        ScalarType::Float(width @ (FloatWidth::F16 | FloatWidth::F128)) => {
            return Err(vec![Diagnostic::UnsupportedCarrierType(ScalarType::Float(
                width,
            ))]);
        }
        pointer @ ScalarType::Pointer { .. } => {
            return Err(vec![Diagnostic::UnsupportedCarrierType(pointer)]);
        }
    };
    Ok(Type::Scalar(scalar))
}
fn encode_type(t: ScalarType, o: &mut Vec<u8>) {
    match t {
        ScalarType::Bool => o.extend_from_slice(&[1, 0, 0, 0]),
        ScalarType::Char => o.extend_from_slice(&[2, 0, 0, 0]),
        ScalarType::Int { width, signed } => {
            o.extend_from_slice(&[3, width.tag(), signed as u8, 0])
        }
        ScalarType::Float(w) => o.extend_from_slice(&[4, w.tag(), 0, 0]),
        ScalarType::Pointer {
            address_space,
            width,
        } => {
            o.extend_from_slice(&[5, width.tag()]);
            o.extend_from_slice(&address_space.to_le_bytes())
        }
    }
}
fn decode_type(d: &mut Decoder<'_>) -> Result<ScalarType, DecodeError> {
    let t = d.byte()?;
    let a = d.byte()?;
    let b = d.byte()?;
    let c = d.byte()?;
    match t {
        1 if a | b | c == 0 => Ok(ScalarType::Bool),
        2 if a | b | c == 0 => Ok(ScalarType::Char),
        3 if b <= 1 && c == 0 => Ok(ScalarType::Int {
            width: iw(a)?,
            signed: b != 0,
        }),
        4 if b == 0 && c == 0 => Ok(ScalarType::Float(fw(a)?)),
        5 => Ok(ScalarType::Pointer {
            width: iw(a)?,
            address_space: u16::from_le_bytes([b, c]),
        }),
        1..=5 => Err(DecodeError::ReservedNonZero),
        _ => Err(DecodeError::UnknownTag {
            field: "type",
            tag: t,
        }),
    }
}
fn encode_op(x: Operation, o: &mut Vec<u8>) {
    match x {
        Operation::IntegerBinary { ty, op, mode } => {
            o.extend_from_slice(&[1, ib_tag(op), mode_tag(mode), 0]);
            encode_type(ty, o)
        }
        Operation::IntegerUnary { ty, op, mode } => {
            o.extend_from_slice(&[2, iu_tag(op), mode_tag(mode), 0]);
            encode_type(ty, o)
        }
        Operation::Shift {
            ty,
            rhs_ty,
            direction,
            policy,
        } => {
            o.extend_from_slice(&[3, shift_tag(direction), sp_tag(policy), 0]);
            encode_type(ty, o);
            encode_type(rhs_ty, o)
        }
        Operation::IntegerCompare { ty, predicate } => {
            o.extend_from_slice(&[4, pred_tag(predicate), 0, 0]);
            encode_type(ty, o)
        }
        Operation::FloatBinary { ty, op, .. } => {
            o.extend_from_slice(&[5, fb_tag(op), 1, 0]);
            encode_type(ty, o)
        }
        Operation::FloatNeg { ty, .. } => {
            o.extend_from_slice(&[6, 1, 0, 0]);
            encode_type(ty, o)
        }
        Operation::FloatCompare {
            ty,
            predicate,
            policy,
        } => {
            o.extend_from_slice(&[7, pred_tag(predicate), fcp_tag(policy), 0]);
            encode_type(ty, o)
        }
        Operation::Cast { from, to, cast } => {
            o.extend_from_slice(&[8, cast_tag(cast), cast_aux(cast), 0]);
            encode_type(from, o);
            encode_type(to, o)
        }
        Operation::FloatTotalCompare { ty } => {
            o.extend_from_slice(&[9, 1, 0, 0]);
            encode_type(ty, o)
        }
    }
}
fn decode_op(d: &mut Decoder<'_>) -> Result<Operation, DecodeError> {
    let t = d.byte()?;
    let a = d.byte()?;
    let b = d.byte()?;
    if d.byte()? != 0 {
        return Err(DecodeError::ReservedNonZero);
    }
    Ok(match t {
        1 => Operation::IntegerBinary {
            op: ib(a)?,
            mode: mode(b)?,
            ty: decode_type(d)?,
        },
        2 => Operation::IntegerUnary {
            op: iu(a)?,
            mode: mode(b)?,
            ty: decode_type(d)?,
        },
        3 => Operation::Shift {
            direction: shift(a)?,
            policy: sp(b)?,
            ty: decode_type(d)?,
            rhs_ty: decode_type(d)?,
        },
        4 => {
            if b != 0 {
                return Err(DecodeError::ReservedNonZero);
            }
            Operation::IntegerCompare {
                predicate: pred(a)?,
                ty: decode_type(d)?,
            }
        }
        5 => {
            if b != 1 {
                return Err(DecodeError::ReservedNonZero);
            }
            Operation::FloatBinary {
                op: fb(a)?,
                semantics: FloatArithmeticSemantics::RustIeee754,
                ty: decode_type(d)?,
            }
        }
        6 => {
            if a != 1 || b != 0 {
                return Err(DecodeError::ReservedNonZero);
            }
            Operation::FloatNeg {
                semantics: FloatArithmeticSemantics::RustIeee754,
                ty: decode_type(d)?,
            }
        }
        7 => Operation::FloatCompare {
            predicate: pred(a)?,
            policy: fcp(b)?,
            ty: decode_type(d)?,
        },
        8 => Operation::Cast {
            cast: cast(a, b)?,
            from: decode_type(d)?,
            to: decode_type(d)?,
        },
        9 => {
            if a != 1 || b != 0 {
                return Err(DecodeError::ReservedNonZero);
            }
            Operation::FloatTotalCompare {
                ty: decode_type(d)?,
            }
        }
        _ => {
            return Err(DecodeError::UnknownTag {
                field: "operation",
                tag: t,
            });
        }
    })
}
struct Decoder<'a> {
    b: &'a [u8],
    at: usize,
}
impl Decoder<'_> {
    fn byte(&mut self) -> Result<u8, DecodeError> {
        let v = *self.b.get(self.at).ok_or(DecodeError::Truncated)?;
        self.at += 1;
        Ok(v)
    }
}
macro_rules! tags{($to:ident,$from:ident,$ty:ty,{$($v:path=>$n:literal),+},$field:literal)=>{const fn $to(x:$ty)->u8{match x{$($v=>$n),+}}fn $from(t:u8)->Result<$ty,DecodeError>{match t{$($n=>Ok($v),)+_=>Err(DecodeError::UnknownTag{field:$field,tag:t})}}};}
tags!(ib_tag,ib,IntBinary,{IntBinary::Add=>1,IntBinary::Sub=>2,IntBinary::Mul=>3,IntBinary::Div=>4,IntBinary::Rem=>5,IntBinary::And=>6,IntBinary::Or=>7,IntBinary::Xor=>8},"integer op");
tags!(iu_tag,iu,IntUnary,{IntUnary::Neg=>1,IntUnary::Not=>2},"unary op");
tags!(mode_tag,mode,IntMode,{IntMode::Checked=>1,IntMode::Wrapping=>2,IntMode::Overflowing=>3,IntMode::Saturating=>4},"mode");
tags!(shift_tag,shift,ShiftDirection,{ShiftDirection::Left=>1,ShiftDirection::Right=>2},"shift");
const fn sp_tag(policy: ShiftPolicy) -> u8 {
    match policy {
        ShiftPolicy::Checked => 1,
        ShiftPolicy::Wrapping => 2,
        ShiftPolicy::Overflowing => 3,
        ShiftPolicy::RustOperator {
            overflow_checks: true,
        } => 4,
        ShiftPolicy::RustOperator {
            overflow_checks: false,
        } => 5,
    }
}
fn sp(tag: u8) -> Result<ShiftPolicy, DecodeError> {
    match tag {
        1 => Ok(ShiftPolicy::Checked),
        2 => Ok(ShiftPolicy::Wrapping),
        3 => Ok(ShiftPolicy::Overflowing),
        4 => Ok(ShiftPolicy::RustOperator {
            overflow_checks: true,
        }),
        5 => Ok(ShiftPolicy::RustOperator {
            overflow_checks: false,
        }),
        _ => Err(DecodeError::UnknownTag {
            field: "shift policy",
            tag,
        }),
    }
}
tags!(pred_tag,pred,Predicate,{Predicate::Eq=>1,Predicate::Ne=>2,Predicate::Lt=>3,Predicate::Le=>4,Predicate::Gt=>5,Predicate::Ge=>6},"predicate");
tags!(fb_tag,fb,FloatBinary,{FloatBinary::Add=>1,FloatBinary::Sub=>2,FloatBinary::Mul=>3,FloatBinary::Div=>4,FloatBinary::Rem=>5},"float op");
tags!(fcp_tag,fcp,FloatComparisonPolicy,{FloatComparisonPolicy::RustPartialEq=>1,FloatComparisonPolicy::RustPartialOrd=>2,FloatComparisonPolicy::IeeeOrdered=>3,FloatComparisonPolicy::IeeeUnordered=>4},"float comparison policy");
fn iw(t: u8) -> Result<IntWidth, DecodeError> {
    IntWidth::from_tag(t).ok_or(DecodeError::UnknownTag {
        field: "integer width",
        tag: t,
    })
}
fn fw(t: u8) -> Result<FloatWidth, DecodeError> {
    FloatWidth::from_tag(t).ok_or(DecodeError::UnknownTag {
        field: "float width",
        tag: t,
    })
}
const fn cast_tag(c: Cast) -> u8 {
    match c {
        Cast::IntExtend { .. } => 1,
        Cast::IntNarrow => 2,
        Cast::FloatExtend => 3,
        Cast::FloatNarrow => 4,
        Cast::IntToFloat { .. } => 5,
        Cast::FloatToInt { .. } => 6,
        Cast::BoolToInt => 7,
        Cast::IntToBoolChecked => 8,
        Cast::CharToInt => 9,
        Cast::IntToCharChecked => 10,
        Cast::Bitcast => 11,
        Cast::PointerAddressSpace => 12,
        Cast::PointerToInt { .. } => 13,
        Cast::IntToPointer { .. } => 14,
    }
}
const fn cast_aux(c: Cast) -> u8 {
    match c {
        Cast::IntExtend { signed } => signed as u8,
        Cast::IntToFloat { .. }
        | Cast::FloatToInt { .. }
        | Cast::PointerToInt { .. }
        | Cast::IntToPointer { .. } => 1,
        _ => 0,
    }
}
fn cast(t: u8, a: u8) -> Result<Cast, DecodeError> {
    Ok(match (t, a) {
        (1, 0) => Cast::IntExtend { signed: false },
        (1, 1) => Cast::IntExtend { signed: true },
        (2, 0) => Cast::IntNarrow,
        (3, 0) => Cast::FloatExtend,
        (4, 0) => Cast::FloatNarrow,
        (5, 1) => Cast::IntToFloat {
            semantics: IntToFloatSemantics::RustAs,
        },
        (6, 1) => Cast::FloatToInt {
            semantics: FloatToIntSemantics::RustSaturatingAs,
        },
        (7, 0) => Cast::BoolToInt,
        (8, 0) => Cast::IntToBoolChecked,
        (9, 0) => Cast::CharToInt,
        (10, 0) => Cast::IntToCharChecked,
        (11, 0) => Cast::Bitcast,
        (12, 0) => Cast::PointerAddressSpace,
        (13, 1) => Cast::PointerToInt {
            unsafe_policy: UnsafeProvenancePolicy::ExplicitProvenanceLoss,
        },
        (14, 1) => Cast::IntToPointer {
            unsafe_policy: UnsafeProvenancePolicy::ExplicitProvenanceLoss,
        },
        (1..=14, _) => return Err(DecodeError::ReservedNonZero),
        _ => {
            return Err(DecodeError::UnknownTag {
                field: "cast",
                tag: t,
            });
        }
    })
}

pub const fn valid_bool_bits(bits: u128) -> bool {
    bits <= 1
}
pub const fn valid_char_bits(bits: u128) -> bool {
    bits <= 0x10ffff && !(bits >= 0xd800 && bits <= 0xdfff)
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntOutcome {
    Value(u128),
    CheckedNone,
    Overflowing { value: u128, overflowed: bool },
    Trap,
}
pub fn evaluate_integer_binary(
    ty: ScalarType,
    op: IntBinary,
    mode: IntMode,
    left: u128,
    right: u128,
) -> Option<IntOutcome> {
    if verify(
        Operation::IntegerBinary { ty, op, mode },
        FloatCapabilities::NONE,
    )
    .is_err()
    {
        return None;
    }
    let (w, signed) = int_parts(ty)?;
    let m = mask(w);
    let a = left & m;
    let b = right & m;
    if matches!(op, IntBinary::And | IntBinary::Or | IntBinary::Xor) {
        return Some(IntOutcome::Value(match op {
            IntBinary::And => a & b,
            IntBinary::Or => a | b,
            IntBinary::Xor => a ^ b,
            _ => unreachable!(),
        }));
    }
    if matches!(op, IntBinary::Div | IntBinary::Rem) && b == 0 {
        return Some(if mode == IntMode::Checked {
            IntOutcome::CheckedNone
        } else {
            IntOutcome::Trap
        });
    }
    let (raw, overflow, saturated) = if signed {
        let x = decode_signed(a, w);
        let y = decode_signed(b, w);
        let (min, max) = signed_bounds(w);
        if matches!(op, IntBinary::Div | IntBinary::Rem) && x == min && y == -1 {
            let sat = if op == IntBinary::Div { max } else { 0 };
            (
                encode_signed(if op == IntBinary::Div { min } else { 0 }, w),
                true,
                encode_signed(sat, w),
            )
        } else {
            let (v, of) = match op {
                IntBinary::Add => x.overflowing_add(y),
                IntBinary::Sub => x.overflowing_sub(y),
                IntBinary::Mul => x.overflowing_mul(y),
                IntBinary::Div => (x / y, false),
                IntBinary::Rem => (x % y, false),
                _ => unreachable!(),
            };
            let range = of || v < min || v > max;
            let sat = if range {
                match op {
                    IntBinary::Add => {
                        if y >= 0 {
                            max
                        } else {
                            min
                        }
                    }
                    IntBinary::Sub => {
                        if y < 0 {
                            max
                        } else {
                            min
                        }
                    }
                    IntBinary::Mul => {
                        if (x < 0) == (y < 0) {
                            max
                        } else {
                            min
                        }
                    }
                    _ => v,
                }
            } else {
                v
            };
            (encode_signed(v, w), range, encode_signed(sat, w))
        }
    } else {
        let (v, of) = match op {
            IntBinary::Add => a.overflowing_add(b),
            IntBinary::Sub => a.overflowing_sub(b),
            IntBinary::Mul => a.overflowing_mul(b),
            IntBinary::Div => (a / b, false),
            IntBinary::Rem => (a % b, false),
            _ => unreachable!(),
        };
        let range = of || v > m;
        let sat = if range {
            match op {
                IntBinary::Sub => 0,
                _ => m,
            }
        } else {
            v
        };
        (v & m, range, sat & m)
    };
    Some(match mode {
        IntMode::Checked => {
            if overflow {
                IntOutcome::CheckedNone
            } else {
                IntOutcome::Value(raw)
            }
        }
        IntMode::Wrapping => IntOutcome::Value(raw),
        IntMode::Overflowing => IntOutcome::Overflowing {
            value: raw,
            overflowed: overflow,
        },
        IntMode::Saturating => IntOutcome::Value(if overflow { saturated } else { raw }),
    })
}
/// Evaluates a shift without narrowing the RHS to `u32`.
///
/// `amount_raw` is first interpreted as the complete bit pattern of `rhs_ty`.
/// A negative signed value or a nonnegative value at least as wide as the LHS is
/// invalid; the selected policy determines the result for that condition.
pub fn evaluate_shift(
    ty: ScalarType,
    rhs_ty: ScalarType,
    direction: ShiftDirection,
    policy: ShiftPolicy,
    value: u128,
    amount_raw: u128,
) -> Option<IntOutcome> {
    let (w, signed) = int_parts(ty)?;
    let (rhs_width, rhs_signed) = int_parts(rhs_ty)?;
    let bits = u128::from(w.bits());
    let amount = amount_raw & mask(rhs_width);
    let invalid = if rhs_signed {
        let signed_amount = decode_signed(amount, rhs_width);
        signed_amount < 0 || (signed_amount as u128) >= bits
    } else {
        amount >= bits
    };
    let n = (amount % bits) as u32;
    let v = value & mask(w);
    let out = match direction {
        ShiftDirection::Left => (v << n) & mask(w),
        ShiftDirection::Right if signed => encode_signed(decode_signed(v, w) >> n, w),
        ShiftDirection::Right => v >> n,
    };
    Some(match policy {
        ShiftPolicy::Checked if invalid => IntOutcome::CheckedNone,
        ShiftPolicy::Checked | ShiftPolicy::Wrapping => IntOutcome::Value(out),
        ShiftPolicy::Overflowing => IntOutcome::Overflowing {
            value: out,
            overflowed: invalid,
        },
        ShiftPolicy::RustOperator {
            overflow_checks: true,
        } if invalid => IntOutcome::Trap,
        ShiftPolicy::RustOperator { .. } => IntOutcome::Value(out),
    })
}
const fn decode_signed(v: u128, w: IntWidth) -> i128 {
    if w.bits() == 128 {
        v as i128
    } else {
        let sign = 1u128 << (w.bits() - 1);
        if v & sign == 0 {
            v as i128
        } else {
            (v as i128) - (1i128 << w.bits())
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExceptionalSemantics {
    pub zero_returns_none: bool,
    pub zero_traps: bool,
    pub min_neg_one_returns_none: bool,
    pub min_neg_one_wraps: bool,
    pub min_neg_one_overflows: bool,
    pub min_neg_one_saturates: bool,
}
pub const fn exceptional_semantics(m: IntMode) -> ExceptionalSemantics {
    ExceptionalSemantics {
        zero_returns_none: matches!(m, IntMode::Checked),
        zero_traps: !matches!(m, IntMode::Checked),
        min_neg_one_returns_none: matches!(m, IntMode::Checked),
        min_neg_one_wraps: matches!(m, IntMode::Wrapping),
        min_neg_one_overflows: matches!(m, IntMode::Overflowing),
        min_neg_one_saturates: matches!(m, IntMode::Saturating),
    }
}
pub fn rust_saturating_f32_to_int(value: f32, width: IntWidth, signed: bool) -> u128 {
    rust_saturating_float_to_int(value.into(), width, signed)
}
pub fn rust_saturating_float_to_int(value: f64, width: IntWidth, signed: bool) -> u128 {
    if value.is_nan() {
        return 0;
    }
    let bits = i32::from(width.bits());
    if signed {
        let lower = -2.0f64.powi(bits - 1);
        let upper = 2.0f64.powi(bits - 1);
        if value <= lower {
            1u128 << (width.bits() - 1)
        } else if value >= upper {
            (1u128 << (width.bits() - 1)) - 1
        } else {
            encode_signed(value.trunc() as i128, width)
        }
    } else {
        if value <= 0.0 {
            0
        } else if value >= 2.0f64.powi(bits) {
            mask(width)
        } else {
            value.trunc() as u128
        }
    }
}

pub fn evaluate_float_compare_f32(
    policy: FloatComparisonPolicy,
    predicate: Predicate,
    left: f32,
    right: f32,
) -> Option<bool> {
    evaluate_float_compare(policy, predicate, left.into(), right.into())
}
pub fn evaluate_float_compare_f64(
    policy: FloatComparisonPolicy,
    predicate: Predicate,
    left: f64,
    right: f64,
) -> Option<bool> {
    evaluate_float_compare(policy, predicate, left, right)
}
fn evaluate_float_compare(
    policy: FloatComparisonPolicy,
    predicate: Predicate,
    left: f64,
    right: f64,
) -> Option<bool> {
    if !valid_float_comparison(predicate, policy) {
        return None;
    }
    let unordered = left.is_nan() || right.is_nan();
    let base = match predicate {
        Predicate::Eq => left == right,
        Predicate::Ne => left != right,
        Predicate::Lt => left < right,
        Predicate::Le => left <= right,
        Predicate::Gt => left > right,
        Predicate::Ge => left >= right,
    };
    Some(match policy {
        FloatComparisonPolicy::IeeeOrdered if unordered => false,
        FloatComparisonPolicy::IeeeUnordered if unordered => true,
        _ => base,
    })
}

pub fn evaluate_float_total_cmp_f32(left: f32, right: f32) -> std::cmp::Ordering {
    left.total_cmp(&right)
}
pub fn evaluate_float_total_cmp_f64(left: f64, right: f64) -> std::cmp::Ordering {
    left.total_cmp(&right)
}
const fn mask(w: IntWidth) -> u128 {
    if w.bits() == 128 {
        u128::MAX
    } else {
        (1u128 << w.bits()) - 1
    }
}
const fn signed_bounds(w: IntWidth) -> (i128, i128) {
    if w.bits() == 128 {
        (i128::MIN, i128::MAX)
    } else {
        let t = 1i128 << (w.bits() - 1);
        (-t, t - 1)
    }
}
const fn encode_signed(v: i128, w: IntWidth) -> u128 {
    (v as u128) & mask(w)
}
