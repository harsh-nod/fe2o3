use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{MirMutability, MirScalarType, MirSemanticType, MirTypeKind, MirTypeValidationError};

pub const EXECUTABLE_MIR_VERSION: u16 = 1;
pub const MAX_EXECUTABLE_TYPES: usize = 1_024;
pub const MAX_EXECUTABLE_FUNCTIONS: usize = 256;
pub const MAX_EXECUTABLE_LOCALS: usize = 4_096;
pub const MAX_EXECUTABLE_BLOCKS: usize = 4_096;
pub const MAX_EXECUTABLE_BLOCK_PARAMETERS: usize = 4_096;
pub const MAX_EXECUTABLE_STATEMENTS_PER_BLOCK: usize = 8_192;
pub const MAX_EXECUTABLE_STATEMENTS: usize = 65_536;
pub const MAX_EXECUTABLE_PROJECTIONS: usize = 32;
pub const MAX_EXECUTABLE_EDGE_ARGUMENTS: usize = 4_096;
pub const MAX_EXECUTABLE_CALL_ARGUMENTS: usize = 256;
pub const MAX_EXECUTABLE_SWITCH_TARGETS: usize = 1_024;
pub const MAX_EXECUTABLE_IDENTITY_BYTES: usize = 4_096;
pub const MAX_EXECUTABLE_SOURCE_FILE_BYTES: usize = 4_096;
pub const MAX_EXECUTABLE_TYPE_DEPTH: usize = 64;
pub const MAX_EXECUTABLE_TYPE_NODES: usize = 65_536;
pub const MAX_EXECUTABLE_TYPE_ITEMS: usize = 65_536;
pub const MAX_EXECUTABLE_FIELDS: usize = 4_096;
pub const MAX_EXECUTABLE_VARIANTS: usize = 1_024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirExecutableVersion {
    V1,
}

impl MirExecutableVersion {
    pub const fn number(self) -> u16 {
        match self {
            Self::V1 => EXECUTABLE_MIR_VERSION,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MirTypeId(pub u32);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MirLocalId(pub u32);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MirBlockId(pub u32);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct MirValueId(pub u32);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirSourceSpan {
    pub file: String,
    pub byte_start: u32,
    pub byte_end: u32,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirExecutableModule {
    pub version: MirExecutableVersion,
    /// Types are sorted by their canonical semantic representation. All type
    /// references in the module are stable indexes into this table.
    pub types: Vec<MirSemanticType>,
    /// Functions are sorted by monomorphized identity.
    pub functions: Vec<MirFunction>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirFunction {
    pub identity: String,
    pub body: MirBody,
    pub span: Option<MirSourceSpan>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirBody {
    pub form: MirBodyForm,
    pub locals: Vec<MirLocalDecl>,
    pub blocks: Vec<MirBasicBlock>,
    pub entry: MirBlockId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirBodyForm {
    Places,
    /// A mixed slot/SSA form. Promoted locals may appear only as parameter
    /// origins and may never be accessed through a place.
    Ssa {
        promoted_locals: Vec<MirLocalId>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirLocalKind {
    Return,
    Argument,
    User,
    Temporary,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirLocalDecl {
    pub ty: MirTypeId,
    pub kind: MirLocalKind,
    pub mutable: bool,
    pub name: Option<String>,
    pub span: Option<MirSourceSpan>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirBasicBlock {
    pub parameters: Vec<MirBlockParameter>,
    pub statements: Vec<MirStatement>,
    pub terminator: MirTerminator,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirBlockParameter {
    pub value: MirValueId,
    pub ty: MirTypeId,
    /// The source local represented by a mem2reg parameter. `None` is reserved
    /// for values introduced by later verified transformations.
    pub origin: Option<MirLocalId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirPlace {
    pub local: MirLocalId,
    pub projection: Vec<MirProjection>,
    /// The importer records the final projected type. Verification derives it
    /// independently from the local and projection sequence.
    pub ty: MirTypeId,
}

impl MirPlace {
    pub const fn local(local: MirLocalId, ty: MirTypeId) -> Self {
        Self {
            local,
            projection: Vec::new(),
            ty,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirProjection {
    Deref,
    Field {
        index: u32,
    },
    Index {
        local: MirLocalId,
    },
    ConstantIndex {
        offset: u64,
        min_length: u64,
        from_end: bool,
    },
    Subslice {
        from: u64,
        to: u64,
        from_end: bool,
    },
    Downcast {
        variant: u32,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirConstant {
    pub ty: MirTypeId,
    pub value: MirConstantValue,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirConstantValue {
    Unit,
    Bool(bool),
    /// Raw two's-complement bits for signed integers.
    Integer(u128),
    /// Raw IEEE or target-defined bits. The semantic type fixes the width.
    FloatBits(u128),
    ZeroSized,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirOperand {
    Copy(MirPlace),
    Move(MirPlace),
    Constant(MirConstant),
    Value(MirValueId),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitXor,
    BitAnd,
    BitOr,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl MirBinaryOp {
    fn is_comparison(self) -> bool {
        matches!(
            self,
            Self::Eq | Self::Ne | Self::Lt | Self::Le | Self::Gt | Self::Ge
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirUnaryOp {
    Not,
    Neg,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirCastKind {
    IntToInt,
    IntToFloat,
    FloatToInt,
    FloatToFloat,
    PointerToPointer,
    PointerToInt,
    IntToPointer,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirAggregateKind {
    Tuple,
    Array,
    Adt { identity: String, variant: u32 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirRvalue {
    Use(MirOperand),
    BinaryOp {
        op: MirBinaryOp,
        lhs: MirOperand,
        rhs: MirOperand,
    },
    CheckedBinaryOp {
        op: MirBinaryOp,
        lhs: MirOperand,
        rhs: MirOperand,
    },
    UnaryOp {
        op: MirUnaryOp,
        operand: MirOperand,
    },
    Cast {
        kind: MirCastKind,
        operand: MirOperand,
        ty: MirTypeId,
    },
    Ref {
        mutability: MirMutability,
        place: MirPlace,
    },
    AddressOf {
        mutability: MirMutability,
        place: MirPlace,
    },
    Len(MirPlace),
    Discriminant(MirPlace),
    Aggregate {
        kind: MirAggregateKind,
        operands: Vec<MirOperand>,
    },
    Repeat {
        operand: MirOperand,
        count: u64,
    },
    ThreadIndex1d,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirStatement {
    pub kind: MirStatementKind,
    pub span: Option<MirSourceSpan>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirStatementKind {
    Assign {
        place: MirPlace,
        value: MirRvalue,
    },
    Define {
        value: MirValueId,
        ty: MirTypeId,
        rvalue: MirRvalue,
    },
    SetDiscriminant {
        place: MirPlace,
        variant: u32,
    },
    StorageLive(MirLocalId),
    StorageDead(MirLocalId),
    Deinit(MirPlace),
    Nop,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirTerminator {
    pub kind: MirTerminatorKind,
    pub span: Option<MirSourceSpan>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirEdge {
    pub target: MirBlockId,
    pub arguments: Vec<MirOperand>,
}

impl MirEdge {
    pub const fn new(target: MirBlockId) -> Self {
        Self {
            target,
            arguments: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirCallee {
    Direct(String),
    Intrinsic(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirUnwindAction {
    Continue,
    Unreachable,
    Terminate,
    Cleanup(MirEdge),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MirCall {
    pub callee: MirCallee,
    pub arguments: Vec<MirOperand>,
    pub destination: Option<MirPlace>,
    pub target: Option<MirEdge>,
    pub unwind: MirUnwindAction,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirAssertMessage {
    BoundsCheck,
    Overflow,
    DivisionByZero,
    User(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum MirTerminatorKind {
    Goto(MirEdge),
    SwitchInt {
        discr: MirOperand,
        targets: Vec<(u128, MirEdge)>,
        otherwise: MirEdge,
    },
    Return,
    Unreachable,
    Call(MirCall),
    Drop {
        place: MirPlace,
        target: MirEdge,
        unwind: MirUnwindAction,
    },
    Assert {
        condition: MirOperand,
        expected: bool,
        message: MirAssertMessage,
        target: MirEdge,
        unwind: MirUnwindAction,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirExecutableValidationError {
    path: String,
    reason: String,
}

impl MirExecutableValidationError {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    pub(crate) fn new(path: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for MirExecutableValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path, self.reason)
    }
}

impl std::error::Error for MirExecutableValidationError {}

impl MirExecutableModule {
    pub fn validate(&self) -> Result<(), MirExecutableValidationError> {
        if self.version.number() != EXECUTABLE_MIR_VERSION {
            return Err(error(
                "module.version",
                "unsupported executable MIR version",
            ));
        }
        bounded_len("module.types", self.types.len(), 1, MAX_EXECUTABLE_TYPES)?;
        bounded_len(
            "module.functions",
            self.functions.len(),
            1,
            MAX_EXECUTABLE_FUNCTIONS,
        )?;

        validate_type_budget(&self.types)?;
        let mut previous_type = None;
        for (index, ty) in self.types.iter().enumerate() {
            let path = format!("module.types[{index}]");
            ty.validate()
                .map_err(|source| map_type_error(&path, source))?;
            let canonical = ty
                .canonical_text()
                .map_err(|source| map_type_error(&path, source))?;
            if previous_type
                .as_ref()
                .is_some_and(|previous| previous >= &canonical)
            {
                return Err(error(
                    path,
                    "type table must be strictly sorted by canonical type text",
                ));
            }
            previous_type = Some(canonical);
        }

        let mut previous_identity: Option<&str> = None;
        for (index, function) in self.functions.iter().enumerate() {
            let path = format!("module.functions[{index}]");
            validate_identity(&format!("{path}.identity"), &function.identity)?;
            if previous_identity.is_some_and(|previous| previous >= function.identity.as_str()) {
                return Err(error(
                    format!("{path}.identity"),
                    "functions must be strictly sorted by identity",
                ));
            }
            previous_identity = Some(&function.identity);
            validate_span_opt(&format!("{path}.span"), function.span.as_ref())?;
            Verifier::new(self, function, path).verify()?;
        }
        Ok(())
    }

    pub fn type_at(&self, id: MirTypeId) -> Option<&MirSemanticType> {
        self.types.get(id.0 as usize)
    }
}

struct Verifier<'a> {
    module: &'a MirExecutableModule,
    function: &'a MirFunction,
    path: String,
    value_types: BTreeMap<MirValueId, MirTypeId>,
    promoted: BTreeSet<MirLocalId>,
    total_statements: usize,
    next_value: u32,
}

impl<'a> Verifier<'a> {
    fn new(module: &'a MirExecutableModule, function: &'a MirFunction, path: String) -> Self {
        Self {
            module,
            function,
            path,
            value_types: BTreeMap::new(),
            promoted: BTreeSet::new(),
            total_statements: 0,
            next_value: 0,
        }
    }

    fn verify(mut self) -> Result<(), MirExecutableValidationError> {
        let body = &self.function.body;
        bounded_len(
            &format!("{}.body.locals", self.path),
            body.locals.len(),
            1,
            MAX_EXECUTABLE_LOCALS,
        )?;
        bounded_len(
            &format!("{}.body.blocks", self.path),
            body.blocks.len(),
            1,
            MAX_EXECUTABLE_BLOCKS,
        )?;
        if body.entry != MirBlockId(0) {
            return Err(error(
                format!("{}.body.entry", self.path),
                "canonical executable MIR requires block 0 as entry",
            ));
        }
        self.verify_locals()?;
        self.verify_form()?;
        self.collect_values()?;

        for (index, block) in body.blocks.iter().enumerate() {
            self.verify_block(MirBlockId(index as u32), block)?;
        }
        if self.total_statements > MAX_EXECUTABLE_STATEMENTS {
            return Err(error(
                format!("{}.body", self.path),
                format!(
                    "statement count {} exceeds {MAX_EXECUTABLE_STATEMENTS}",
                    self.total_statements
                ),
            ));
        }
        self.verify_reachability()
    }

    fn verify_locals(&self) -> Result<(), MirExecutableValidationError> {
        let mut saw_non_argument = false;
        for (index, local) in self.function.body.locals.iter().enumerate() {
            let path = format!("{}.body.locals[{index}]", self.path);
            self.require_type(&format!("{path}.ty"), local.ty)?;
            validate_name_opt(&format!("{path}.name"), local.name.as_deref())?;
            validate_span_opt(&format!("{path}.span"), local.span.as_ref())?;
            if index == 0 {
                if local.kind != MirLocalKind::Return {
                    return Err(error(path, "local 0 must be the return local"));
                }
            } else if local.kind == MirLocalKind::Return {
                return Err(error(path, "only local 0 may be the return local"));
            }
            if index > 0 {
                match local.kind {
                    MirLocalKind::Argument if saw_non_argument => {
                        return Err(error(
                            path,
                            "argument locals must be a contiguous prefix after local 0",
                        ));
                    }
                    MirLocalKind::Argument => {}
                    _ => saw_non_argument = true,
                }
            }
        }
        Ok(())
    }

    fn verify_form(&mut self) -> Result<(), MirExecutableValidationError> {
        if let MirBodyForm::Ssa { promoted_locals } = &self.function.body.form {
            let mut previous = None;
            for (index, local) in promoted_locals.iter().copied().enumerate() {
                let path = format!("{}.body.form.promoted_locals[{index}]", self.path);
                self.require_local(&path, local)?;
                if local == MirLocalId(0) {
                    return Err(error(path, "the return local cannot be promoted"));
                }
                if previous.is_some_and(|value| value >= local) {
                    return Err(error(path, "promoted locals must be strictly sorted"));
                }
                previous = Some(local);
                self.promoted.insert(local);
            }
        }
        Ok(())
    }

    fn collect_values(&mut self) -> Result<(), MirExecutableValidationError> {
        for (block_index, block) in self.function.body.blocks.iter().enumerate() {
            let block_path = format!("{}.body.blocks[{block_index}]", self.path);
            bounded_len(
                &format!("{block_path}.parameters"),
                block.parameters.len(),
                0,
                MAX_EXECUTABLE_BLOCK_PARAMETERS,
            )?;
            if matches!(self.function.body.form, MirBodyForm::Places)
                && !block.parameters.is_empty()
            {
                return Err(error(
                    format!("{block_path}.parameters"),
                    "place form cannot contain block parameters",
                ));
            }
            let mut origins = BTreeSet::new();
            for (parameter_index, parameter) in block.parameters.iter().enumerate() {
                let path = format!("{block_path}.parameters[{parameter_index}]");
                self.collect_value(&path, parameter.value, parameter.ty)?;
                if let Some(origin) = parameter.origin {
                    self.require_local(&format!("{path}.origin"), origin)?;
                    if !self.promoted.contains(&origin) {
                        return Err(error(
                            format!("{path}.origin"),
                            "parameter origin is not a promoted local",
                        ));
                    }
                    if self.local(origin).ty != parameter.ty {
                        return Err(error(
                            format!("{path}.ty"),
                            "parameter type does not match its source local",
                        ));
                    }
                    if !origins.insert(origin) {
                        return Err(error(
                            format!("{path}.origin"),
                            "a block has duplicate parameters for one promoted local",
                        ));
                    }
                    if block_index == 0 && self.local(origin).kind != MirLocalKind::Argument {
                        return Err(error(
                            format!("{path}.origin"),
                            "entry parameters may originate only from arguments",
                        ));
                    }
                }
            }
            bounded_len(
                &format!("{block_path}.statements"),
                block.statements.len(),
                0,
                MAX_EXECUTABLE_STATEMENTS_PER_BLOCK,
            )?;
            self.total_statements = self
                .total_statements
                .checked_add(block.statements.len())
                .ok_or_else(|| error(&block_path, "statement count overflow"))?;
            for (statement_index, statement) in block.statements.iter().enumerate() {
                if let MirStatementKind::Define { value, ty, .. } = statement.kind {
                    if matches!(self.function.body.form, MirBodyForm::Places) {
                        return Err(error(
                            format!("{block_path}.statements[{statement_index}]"),
                            "place form cannot contain SSA definitions",
                        ));
                    }
                    self.collect_value(
                        &format!("{block_path}.statements[{statement_index}].value"),
                        value,
                        ty,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn collect_value(
        &mut self,
        path: &str,
        value: MirValueId,
        ty: MirTypeId,
    ) -> Result<(), MirExecutableValidationError> {
        self.require_type(&format!("{path}.ty"), ty)?;
        if value != MirValueId(self.next_value) {
            return Err(error(
                path,
                format!(
                    "SSA values must be numbered canonically; expected {}, found {}",
                    self.next_value, value.0
                ),
            ));
        }
        self.next_value = self
            .next_value
            .checked_add(1)
            .ok_or_else(|| error(path, "SSA value identity overflow"))?;
        if self.value_types.insert(value, ty).is_some() {
            return Err(error(path, "duplicate SSA value identity"));
        }
        Ok(())
    }

    fn verify_block(
        &self,
        block_id: MirBlockId,
        block: &MirBasicBlock,
    ) -> Result<(), MirExecutableValidationError> {
        let block_path = format!("{}.body.blocks[{}]", self.path, block_id.0);
        let mut available = BTreeSet::new();
        for parameter in &block.parameters {
            available.insert(parameter.value);
        }
        for (index, statement) in block.statements.iter().enumerate() {
            let path = format!("{block_path}.statements[{index}]");
            validate_span_opt(&format!("{path}.span"), statement.span.as_ref())?;
            match &statement.kind {
                MirStatementKind::Assign { place, value } => {
                    let (destination, writable) =
                        self.verify_place_access(&format!("{path}.place"), place)?;
                    if !writable {
                        return Err(error(
                            format!("{path}.place"),
                            "assignment destination is not writable",
                        ));
                    }
                    let source = self.verify_rvalue(&format!("{path}.value"), value, &available)?;
                    self.require_same_type(&path, destination, source)?;
                }
                MirStatementKind::Define { value, ty, rvalue } => {
                    let actual =
                        self.verify_rvalue(&format!("{path}.rvalue"), rvalue, &available)?;
                    self.require_same_type(&path, *ty, actual)?;
                    available.insert(*value);
                }
                MirStatementKind::SetDiscriminant { place, variant } => {
                    let (ty, writable) =
                        self.verify_place_access(&format!("{path}.place"), place)?;
                    if !writable {
                        return Err(error(
                            format!("{path}.place"),
                            "set-discriminant destination is not writable",
                        ));
                    }
                    let MirTypeKind::Enum(enum_type) = &self.type_at(ty).kind else {
                        return Err(error(path, "set-discriminant place is not an enum"));
                    };
                    if !enum_type.variants.iter().any(|item| item.index == *variant) {
                        return Err(error(path, "set-discriminant variant does not exist"));
                    }
                }
                MirStatementKind::StorageLive(local) | MirStatementKind::StorageDead(local) => {
                    self.require_local(&path, *local)?;
                    if self.promoted.contains(local) {
                        return Err(error(path, "promoted local retains storage markers"));
                    }
                }
                MirStatementKind::Deinit(place) => {
                    let (_, writable) =
                        self.verify_place_access(&format!("{path}.place"), place)?;
                    if !writable {
                        return Err(error(
                            format!("{path}.place"),
                            "deinit destination is not writable",
                        ));
                    }
                }
                MirStatementKind::Nop => {}
            }
        }
        let path = format!("{block_path}.terminator");
        validate_span_opt(&format!("{path}.span"), block.terminator.span.as_ref())?;
        self.verify_terminator(block_id, &path, &block.terminator.kind, &available)
    }

    fn verify_terminator(
        &self,
        source: MirBlockId,
        path: &str,
        terminator: &MirTerminatorKind,
        available: &BTreeSet<MirValueId>,
    ) -> Result<(), MirExecutableValidationError> {
        match terminator {
            MirTerminatorKind::Goto(edge) => self.verify_edge(source, path, edge, available),
            MirTerminatorKind::SwitchInt {
                discr,
                targets,
                otherwise,
            } => {
                let discr_ty = self.verify_operand(&format!("{path}.discr"), discr, available)?;
                if !is_integer_or_bool(&self.type_at(discr_ty).kind) {
                    return Err(error(
                        format!("{path}.discr"),
                        "switch discriminant must be integer or bool",
                    ));
                }
                bounded_len(
                    &format!("{path}.targets"),
                    targets.len(),
                    1,
                    MAX_EXECUTABLE_SWITCH_TARGETS,
                )?;
                let mut previous = None;
                for (index, (value, edge)) in targets.iter().enumerate() {
                    if previous.is_some_and(|previous| previous >= *value) {
                        return Err(error(
                            format!("{path}.targets[{index}]"),
                            "switch values must be strictly increasing",
                        ));
                    }
                    previous = Some(*value);
                    self.verify_edge(source, &format!("{path}.targets[{index}]"), edge, available)?;
                }
                self.verify_edge(source, &format!("{path}.otherwise"), otherwise, available)
            }
            MirTerminatorKind::Return | MirTerminatorKind::Unreachable => Ok(()),
            MirTerminatorKind::Call(call) => {
                match &call.callee {
                    MirCallee::Direct(identity) | MirCallee::Intrinsic(identity) => {
                        validate_identity(&format!("{path}.callee"), identity)?;
                    }
                }
                bounded_len(
                    &format!("{path}.arguments"),
                    call.arguments.len(),
                    0,
                    MAX_EXECUTABLE_CALL_ARGUMENTS,
                )?;
                for (index, operand) in call.arguments.iter().enumerate() {
                    self.verify_operand(&format!("{path}.arguments[{index}]"), operand, available)?;
                }
                if let Some(destination) = &call.destination {
                    let (_, writable) =
                        self.verify_place_access(&format!("{path}.destination"), destination)?;
                    if !writable {
                        return Err(error(
                            format!("{path}.destination"),
                            "call destination is not writable",
                        ));
                    }
                }
                if call.destination.is_some() != call.target.is_some() {
                    return Err(error(
                        path,
                        "call destination and normal target must either both exist or both be absent",
                    ));
                }
                if let Some(target) = &call.target {
                    self.verify_edge(source, &format!("{path}.target"), target, available)?;
                }
                self.verify_unwind(source, &format!("{path}.unwind"), &call.unwind, available)
            }
            MirTerminatorKind::Drop {
                place,
                target,
                unwind,
            } => {
                self.verify_place(&format!("{path}.place"), place)?;
                self.verify_edge(source, &format!("{path}.target"), target, available)?;
                self.verify_unwind(source, &format!("{path}.unwind"), unwind, available)
            }
            MirTerminatorKind::Assert {
                condition,
                message,
                target,
                unwind,
                ..
            } => {
                let condition_ty =
                    self.verify_operand(&format!("{path}.condition"), condition, available)?;
                if !matches!(
                    self.type_at(condition_ty).kind,
                    MirTypeKind::Scalar(MirScalarType::Bool)
                ) {
                    return Err(error(
                        format!("{path}.condition"),
                        "assert condition must be bool",
                    ));
                }
                if let MirAssertMessage::User(message) = message {
                    validate_name(&format!("{path}.message"), message)?;
                }
                self.verify_edge(source, &format!("{path}.target"), target, available)?;
                self.verify_unwind(source, &format!("{path}.unwind"), unwind, available)
            }
        }
    }

    fn verify_unwind(
        &self,
        source: MirBlockId,
        path: &str,
        unwind: &MirUnwindAction,
        available: &BTreeSet<MirValueId>,
    ) -> Result<(), MirExecutableValidationError> {
        if let MirUnwindAction::Cleanup(edge) = unwind {
            self.verify_edge(source, path, edge, available)?;
        }
        Ok(())
    }

    fn verify_edge(
        &self,
        source: MirBlockId,
        path: &str,
        edge: &MirEdge,
        available: &BTreeSet<MirValueId>,
    ) -> Result<(), MirExecutableValidationError> {
        self.require_block(&format!("{path}.target"), edge.target)?;
        if edge.target == self.function.body.entry {
            return Err(error(
                format!("{path}.target"),
                "edges to the canonical entry block are not supported",
            ));
        }
        bounded_len(
            &format!("{path}.arguments"),
            edge.arguments.len(),
            0,
            MAX_EXECUTABLE_EDGE_ARGUMENTS,
        )?;
        let parameters = &self.function.body.blocks[edge.target.0 as usize].parameters;
        if edge.arguments.len() != parameters.len() {
            return Err(error(
                format!("{path}.arguments"),
                format!(
                    "edge from block {} supplies {} arguments for {} parameters",
                    source.0,
                    edge.arguments.len(),
                    parameters.len()
                ),
            ));
        }
        for (index, (argument, parameter)) in
            edge.arguments.iter().zip(parameters.iter()).enumerate()
        {
            let actual =
                self.verify_operand(&format!("{path}.arguments[{index}]"), argument, available)?;
            self.require_same_type(&format!("{path}.arguments[{index}]"), parameter.ty, actual)?;
        }
        Ok(())
    }

    fn verify_rvalue(
        &self,
        path: &str,
        rvalue: &MirRvalue,
        available: &BTreeSet<MirValueId>,
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        match rvalue {
            MirRvalue::Use(operand) => self.verify_operand(path, operand, available),
            MirRvalue::BinaryOp { op, lhs, rhs } => {
                let lhs_ty = self.verify_operand(&format!("{path}.lhs"), lhs, available)?;
                let rhs_ty = self.verify_operand(&format!("{path}.rhs"), rhs, available)?;
                self.require_same_type(path, lhs_ty, rhs_ty)?;
                if !valid_binary_op(*op, &self.type_at(lhs_ty).kind) {
                    return Err(error(
                        path,
                        "binary operation is invalid for its operand type",
                    ));
                }
                if op.is_comparison() {
                    self.bool_type(path)
                } else {
                    Ok(lhs_ty)
                }
            }
            MirRvalue::CheckedBinaryOp { op, lhs, rhs } => {
                let lhs_ty = self.verify_operand(&format!("{path}.lhs"), lhs, available)?;
                let rhs_ty = self.verify_operand(&format!("{path}.rhs"), rhs, available)?;
                self.require_same_type(path, lhs_ty, rhs_ty)?;
                if !matches!(op, MirBinaryOp::Add | MirBinaryOp::Sub | MirBinaryOp::Mul)
                    || !is_machine_integer(&self.type_at(lhs_ty).kind)
                {
                    return Err(error(
                        path,
                        "checked V1 operations are limited to integer add, sub, and mul",
                    ));
                }
                let bool_ty = self.bool_type(path)?;
                self.find_aggregate_type(path, &MirAggregateKind::Tuple, &[lhs_ty, bool_ty])
            }
            MirRvalue::UnaryOp { op, operand } => {
                let ty = self.verify_operand(&format!("{path}.operand"), operand, available)?;
                let kind = &self.type_at(ty).kind;
                let valid = match op {
                    MirUnaryOp::Not => is_integer_or_bool(kind),
                    MirUnaryOp::Neg => is_signed_integer_or_float(kind),
                };
                if !valid {
                    return Err(error(
                        path,
                        "unary operation is invalid for its operand type",
                    ));
                }
                Ok(ty)
            }
            MirRvalue::Cast { kind, operand, ty } => {
                self.require_type(&format!("{path}.ty"), *ty)?;
                let source = self.verify_operand(&format!("{path}.operand"), operand, available)?;
                if !valid_cast(*kind, &self.type_at(source).kind, &self.type_at(*ty).kind) {
                    return Err(error(
                        path,
                        "cast kind does not match source and destination types",
                    ));
                }
                Ok(*ty)
            }
            MirRvalue::Ref { mutability, place } => {
                let place_ty = self.verify_place(&format!("{path}.place"), place)?;
                self.find_reference_type(path, place_ty, *mutability, false)
            }
            MirRvalue::AddressOf { mutability, place } => {
                let place_ty = self.verify_place(&format!("{path}.place"), place)?;
                self.find_reference_type(path, place_ty, *mutability, true)
            }
            MirRvalue::Len(place) => {
                let ty = self.verify_place(&format!("{path}.place"), place)?;
                if !matches!(
                    self.type_at(ty).kind,
                    MirTypeKind::Slice { .. } | MirTypeKind::Array { .. }
                ) {
                    return Err(error(path, "len operand must be a slice or array"));
                }
                self.usize_type(path)
            }
            MirRvalue::Discriminant(place) => {
                let ty = self.verify_place(&format!("{path}.place"), place)?;
                let MirTypeKind::Enum(enum_ty) = &self.type_at(ty).kind else {
                    return Err(error(path, "discriminant operand must be an enum"));
                };
                self.find_scalar_type(path, enum_ty.discriminant)
            }
            MirRvalue::Aggregate { kind, operands } => {
                if let MirAggregateKind::Adt { identity, .. } = kind {
                    validate_identity(&format!("{path}.identity"), identity)?;
                }
                bounded_len(
                    &format!("{path}.operands"),
                    operands.len(),
                    0,
                    MAX_EXECUTABLE_CALL_ARGUMENTS,
                )?;
                let operand_types = operands
                    .iter()
                    .enumerate()
                    .map(|(index, operand)| {
                        self.verify_operand(
                            &format!("{path}.operands[{index}]"),
                            operand,
                            available,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.find_aggregate_type(path, kind, &operand_types)
            }
            MirRvalue::Repeat { operand, count } => {
                let element =
                    self.verify_operand(&format!("{path}.operand"), operand, available)?;
                self.find_array_type(path, element, *count)
            }
            MirRvalue::ThreadIndex1d => self.usize_type(path),
        }
    }

    fn verify_operand(
        &self,
        path: &str,
        operand: &MirOperand,
        available: &BTreeSet<MirValueId>,
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        match operand {
            MirOperand::Copy(place) | MirOperand::Move(place) => self.verify_place(path, place),
            MirOperand::Constant(constant) => self.verify_constant(path, constant),
            MirOperand::Value(value) => {
                let ty =
                    self.value_types.get(value).copied().ok_or_else(|| {
                        error(path, format!("SSA value {} is not defined", value.0))
                    })?;
                if !available.contains(value) {
                    return Err(error(
                        path,
                        format!(
                            "SSA value {} is not a parameter or prior definition in this block",
                            value.0
                        ),
                    ));
                }
                Ok(ty)
            }
        }
    }

    fn verify_constant(
        &self,
        path: &str,
        constant: &MirConstant,
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        self.require_type(&format!("{path}.ty"), constant.ty)?;
        let ty = self.type_at(constant.ty);
        let valid = match (&constant.value, &ty.kind) {
            (MirConstantValue::Unit, MirTypeKind::Unit) => true,
            (MirConstantValue::Bool(_), MirTypeKind::Scalar(MirScalarType::Bool)) => true,
            (
                MirConstantValue::Integer(bits),
                MirTypeKind::Scalar(MirScalarType::Int { bits: width, .. }),
            ) => *width == 128 || *bits < (1_u128 << *width),
            (MirConstantValue::Integer(bits), MirTypeKind::Scalar(MirScalarType::Char)) => {
                *bits <= u128::from(char::MAX as u32) && !(0xd800..=0xdfff).contains(bits)
            }
            (
                MirConstantValue::FloatBits(bits),
                MirTypeKind::Scalar(MirScalarType::Float { bits: width }),
            ) => *width == 128 || *bits < (1_u128 << *width),
            (MirConstantValue::ZeroSized, _) => ty.layout.size == Some(0),
            _ => false,
        };
        if !valid {
            return Err(error(
                path,
                "constant payload does not match its semantic type",
            ));
        }
        Ok(constant.ty)
    }

    fn verify_place(
        &self,
        path: &str,
        place: &MirPlace,
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        self.verify_place_access(path, place).map(|(ty, _)| ty)
    }

    fn verify_place_access(
        &self,
        path: &str,
        place: &MirPlace,
    ) -> Result<(MirTypeId, bool), MirExecutableValidationError> {
        self.require_local(&format!("{path}.local"), place.local)?;
        self.require_type(&format!("{path}.ty"), place.ty)?;
        if self.promoted.contains(&place.local) {
            return Err(error(
                path,
                "promoted local is still accessed through a place",
            ));
        }
        bounded_len(
            &format!("{path}.projection"),
            place.projection.len(),
            0,
            MAX_EXECUTABLE_PROJECTIONS,
        )?;

        let mut current = ProjectionState {
            ty: ProjectionType::Type(self.type_at(self.local(place.local).ty)),
            writable: self.local(place.local).mutable,
        };
        for (index, projection) in place.projection.iter().enumerate() {
            current = self.project(&format!("{path}.projection[{index}]"), current, projection)?;
        }
        let actual = match current.ty {
            ProjectionType::Type(ty) => ty,
            ProjectionType::Variant { .. } => {
                return Err(error(path, "a downcast place must project a variant field"));
            }
        };
        if actual != self.type_at(place.ty) {
            return Err(error(
                path,
                "recorded place type does not match its projection",
            ));
        }
        Ok((place.ty, current.writable))
    }

    fn project<'b>(
        &'b self,
        path: &str,
        current: ProjectionState<'b>,
        projection: &MirProjection,
    ) -> Result<ProjectionState<'b>, MirExecutableValidationError> {
        match projection {
            MirProjection::Deref => {
                let ProjectionType::Type(ty) = current.ty else {
                    return Err(error(path, "cannot dereference an enum downcast"));
                };
                match &ty.kind {
                    MirTypeKind::RawPointer {
                        pointee,
                        mutability,
                        ..
                    } => Ok(ProjectionState {
                        ty: ProjectionType::Type(pointee),
                        writable: *mutability == MirMutability::Mutable,
                    }),
                    MirTypeKind::Reference {
                        referent,
                        mutability,
                        ..
                    } => Ok(ProjectionState {
                        ty: ProjectionType::Type(referent),
                        writable: *mutability == MirMutability::Mutable,
                    }),
                    _ => Err(error(
                        path,
                        "deref projection requires a pointer or reference",
                    )),
                }
            }
            MirProjection::Field { index } => {
                let fields = match current.ty {
                    ProjectionType::Type(ty) => match &ty.kind {
                        MirTypeKind::Tuple(aggregate) => &aggregate.fields,
                        MirTypeKind::Struct(structure) => &structure.aggregate.fields,
                        _ => return Err(error(path, "field projection requires an aggregate")),
                    },
                    ProjectionType::Variant { fields } => fields,
                };
                fields
                    .get(*index as usize)
                    .map(|field| ProjectionState {
                        ty: ProjectionType::Type(&field.ty),
                        writable: current.writable,
                    })
                    .ok_or_else(|| error(path, "field projection index is out of bounds"))
            }
            MirProjection::Index { local } => {
                self.require_local(&format!("{path}.local"), *local)?;
                if self.promoted.contains(local) {
                    return Err(error(path, "projection index cannot name a promoted local"));
                }
                let index_ty = &self.type_at(self.local(*local).ty).kind;
                if !is_unsigned_integer(index_ty) {
                    return Err(error(path, "index local must be an unsigned integer"));
                }
                self.sequence_element(path, current)
            }
            MirProjection::ConstantIndex {
                offset,
                min_length,
                from_end,
            } => {
                if *min_length == 0 || *offset >= *min_length {
                    return Err(error(
                        path,
                        "constant index must be within its minimum length",
                    ));
                }
                if *from_end && offset.checked_add(1).is_none() {
                    return Err(error(path, "constant from-end index overflows"));
                }
                self.sequence_element(path, current)
            }
            MirProjection::Subslice { from, to, from_end } => {
                if !from_end && from > to {
                    return Err(error(path, "subslice start exceeds end"));
                }
                let ProjectionType::Type(ty) = current.ty else {
                    return Err(error(path, "subslice requires an array or slice"));
                };
                match &ty.kind {
                    MirTypeKind::Array { element, length } => {
                        let in_bounds = if *from_end {
                            from.checked_add(*to)
                                .is_some_and(|extent| extent <= *length)
                        } else {
                            *from <= *length && *to <= *length
                        };
                        if !in_bounds {
                            return Err(error(path, "subslice bounds exceed array length"));
                        }
                        self.find_slice_semantic_type(path, element)
                            .map(|ty| ProjectionState {
                                ty: ProjectionType::Type(ty),
                                writable: current.writable,
                            })
                    }
                    MirTypeKind::Slice { .. } => Ok(ProjectionState {
                        ty: ProjectionType::Type(ty),
                        writable: current.writable,
                    }),
                    _ => Err(error(path, "subslice requires an array or slice")),
                }
            }
            MirProjection::Downcast { variant } => {
                let ProjectionType::Type(ty) = current.ty else {
                    return Err(error(path, "nested downcast is invalid"));
                };
                let MirTypeKind::Enum(enum_ty) = &ty.kind else {
                    return Err(error(path, "downcast requires an enum"));
                };
                let variant = enum_ty
                    .variants
                    .iter()
                    .find(|item| item.index == *variant)
                    .ok_or_else(|| error(path, "downcast variant does not exist"))?;
                Ok(ProjectionState {
                    ty: ProjectionType::Variant {
                        fields: &variant.aggregate.fields,
                    },
                    writable: current.writable,
                })
            }
        }
    }

    fn sequence_element<'b>(
        &'b self,
        path: &str,
        current: ProjectionState<'b>,
    ) -> Result<ProjectionState<'b>, MirExecutableValidationError> {
        let ProjectionType::Type(ty) = current.ty else {
            return Err(error(path, "index projection requires an array or slice"));
        };
        match &ty.kind {
            MirTypeKind::Array { element, .. } | MirTypeKind::Slice { element } => {
                Ok(ProjectionState {
                    ty: ProjectionType::Type(element),
                    writable: current.writable,
                })
            }
            _ => Err(error(path, "index projection requires an array or slice")),
        }
    }

    fn verify_reachability(&self) -> Result<(), MirExecutableValidationError> {
        let mut reachable = BTreeSet::new();
        let mut queue = VecDeque::from([self.function.body.entry]);
        while let Some(block) = queue.pop_front() {
            if !reachable.insert(block) {
                continue;
            }
            for edge in
                terminator_edges(&self.function.body.blocks[block.0 as usize].terminator.kind)
            {
                queue.push_back(edge.target);
            }
        }
        if reachable.len() != self.function.body.blocks.len() {
            let missing = (0..self.function.body.blocks.len())
                .map(|index| MirBlockId(index as u32))
                .find(|block| !reachable.contains(block))
                .expect("length mismatch guarantees an unreachable block");
            return Err(error(
                format!("{}.body.blocks[{}]", self.path, missing.0),
                "block is unreachable from the entry",
            ));
        }
        Ok(())
    }

    fn find_reference_type(
        &self,
        path: &str,
        referent: MirTypeId,
        mutability: MirMutability,
        raw: bool,
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        self.module
            .types
            .iter()
            .position(|candidate| match &candidate.kind {
                MirTypeKind::RawPointer {
                    pointee,
                    mutability: candidate_mutability,
                    ..
                } if raw => {
                    **pointee == *self.type_at(referent) && *candidate_mutability == mutability
                }
                MirTypeKind::Reference {
                    referent: candidate_referent,
                    mutability: candidate_mutability,
                    ..
                } if !raw => {
                    **candidate_referent == *self.type_at(referent)
                        && *candidate_mutability == mutability
                }
                _ => false,
            })
            .map(|index| MirTypeId(index as u32))
            .ok_or_else(|| {
                error(
                    path,
                    "result pointer/reference type is absent from the type table",
                )
            })
    }

    fn find_scalar_type(
        &self,
        path: &str,
        scalar: MirScalarType,
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        self.find_type(
            path,
            |kind| matches!(kind, MirTypeKind::Scalar(value) if *value == scalar),
        )
    }

    fn bool_type(&self, path: &str) -> Result<MirTypeId, MirExecutableValidationError> {
        self.find_scalar_type(path, MirScalarType::Bool)
    }

    fn usize_type(&self, path: &str) -> Result<MirTypeId, MirExecutableValidationError> {
        self.find_type(path, is_unsigned_integer)
    }

    fn find_array_type(
        &self,
        path: &str,
        element: MirTypeId,
        length: u64,
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        self.find_type(path, |kind| match kind {
            MirTypeKind::Array {
                element: candidate,
                length: candidate_length,
            } => **candidate == *self.type_at(element) && *candidate_length == length,
            _ => false,
        })
    }

    fn find_aggregate_type(
        &self,
        path: &str,
        aggregate_kind: &MirAggregateKind,
        operand_types: &[MirTypeId],
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        self.find_type(path, |kind| {
            let fields = match (aggregate_kind, kind) {
                (MirAggregateKind::Tuple, MirTypeKind::Tuple(aggregate)) => &aggregate.fields,
                (MirAggregateKind::Array, MirTypeKind::Array { element, length }) => {
                    return *length == operand_types.len() as u64
                        && operand_types
                            .iter()
                            .all(|ty| self.type_at(*ty) == element.as_ref());
                }
                (MirAggregateKind::Adt { identity, variant }, MirTypeKind::Struct(structure))
                    if *variant == 0 && structure.identity == *identity =>
                {
                    &structure.aggregate.fields
                }
                (MirAggregateKind::Adt { identity, variant }, MirTypeKind::Enum(enum_ty))
                    if enum_ty.identity == *identity =>
                {
                    let Some(variant) = enum_ty.variants.iter().find(|item| item.index == *variant)
                    else {
                        return false;
                    };
                    &variant.aggregate.fields
                }
                _ => return false,
            };
            fields.len() == operand_types.len()
                && fields
                    .iter()
                    .zip(operand_types)
                    .all(|(field, ty)| field.ty == *self.type_at(*ty))
        })
    }

    fn find_slice_semantic_type<'b>(
        &'b self,
        path: &str,
        element: &MirSemanticType,
    ) -> Result<&'b MirSemanticType, MirExecutableValidationError> {
        self.module
            .types
            .iter()
            .find(|candidate| {
                matches!(&candidate.kind, MirTypeKind::Slice { element: candidate } if candidate.as_ref() == element)
            })
            .ok_or_else(|| error(path, "projected slice type is absent from the type table"))
    }

    fn find_type(
        &self,
        path: &str,
        predicate: impl Fn(&MirTypeKind) -> bool,
    ) -> Result<MirTypeId, MirExecutableValidationError> {
        self.module
            .types
            .iter()
            .position(|ty| predicate(&ty.kind))
            .map(|index| MirTypeId(index as u32))
            .ok_or_else(|| error(path, "required result type is absent from the type table"))
    }

    fn require_same_type(
        &self,
        path: &str,
        expected: MirTypeId,
        actual: MirTypeId,
    ) -> Result<(), MirExecutableValidationError> {
        if expected != actual {
            return Err(error(
                path,
                format!("type mismatch: expected {}, found {}", expected.0, actual.0),
            ));
        }
        Ok(())
    }

    fn require_type(&self, path: &str, ty: MirTypeId) -> Result<(), MirExecutableValidationError> {
        if ty.0 as usize >= self.module.types.len() {
            return Err(error(path, format!("type {} does not exist", ty.0)));
        }
        Ok(())
    }

    fn require_local(
        &self,
        path: &str,
        local: MirLocalId,
    ) -> Result<(), MirExecutableValidationError> {
        if local.0 as usize >= self.function.body.locals.len() {
            return Err(error(path, format!("local {} does not exist", local.0)));
        }
        Ok(())
    }

    fn require_block(
        &self,
        path: &str,
        block: MirBlockId,
    ) -> Result<(), MirExecutableValidationError> {
        if block.0 as usize >= self.function.body.blocks.len() {
            return Err(error(path, format!("block {} does not exist", block.0)));
        }
        Ok(())
    }

    fn local(&self, id: MirLocalId) -> &MirLocalDecl {
        &self.function.body.locals[id.0 as usize]
    }

    fn type_at(&self, id: MirTypeId) -> &MirSemanticType {
        &self.module.types[id.0 as usize]
    }
}

enum ProjectionType<'a> {
    Type(&'a MirSemanticType),
    Variant { fields: &'a [crate::MirField] },
}

struct ProjectionState<'a> {
    ty: ProjectionType<'a>,
    writable: bool,
}

pub(crate) fn terminator_edges(terminator: &MirTerminatorKind) -> Vec<&MirEdge> {
    let mut edges = Vec::new();
    match terminator {
        MirTerminatorKind::Goto(edge) => edges.push(edge),
        MirTerminatorKind::SwitchInt {
            targets, otherwise, ..
        } => {
            edges.extend(targets.iter().map(|(_, edge)| edge));
            edges.push(otherwise);
        }
        MirTerminatorKind::Call(call) => {
            if let Some(edge) = &call.target {
                edges.push(edge);
            }
            push_unwind(&mut edges, &call.unwind);
        }
        MirTerminatorKind::Drop { target, unwind, .. }
        | MirTerminatorKind::Assert { target, unwind, .. } => {
            edges.push(target);
            push_unwind(&mut edges, unwind);
        }
        MirTerminatorKind::Return | MirTerminatorKind::Unreachable => {}
    }
    edges
}

fn push_unwind<'a>(edges: &mut Vec<&'a MirEdge>, unwind: &'a MirUnwindAction) {
    if let MirUnwindAction::Cleanup(edge) = unwind {
        edges.push(edge);
    }
}

fn valid_cast(kind: MirCastKind, source: &MirTypeKind, destination: &MirTypeKind) -> bool {
    match kind {
        MirCastKind::IntToInt => is_integer(source) && is_integer(destination),
        MirCastKind::IntToFloat => is_integer(source) && is_float(destination),
        MirCastKind::FloatToInt => is_float(source) && is_integer(destination),
        MirCastKind::FloatToFloat => is_float(source) && is_float(destination),
        MirCastKind::PointerToPointer => is_pointer(source) && is_pointer(destination),
        MirCastKind::PointerToInt => is_pointer(source) && is_integer(destination),
        MirCastKind::IntToPointer => is_integer(source) && is_pointer(destination),
    }
}

fn valid_binary_op(operation: MirBinaryOp, kind: &MirTypeKind) -> bool {
    match operation {
        MirBinaryOp::Add
        | MirBinaryOp::Sub
        | MirBinaryOp::Mul
        | MirBinaryOp::Div
        | MirBinaryOp::Rem => is_arithmetic_numeric(kind),
        MirBinaryOp::BitXor | MirBinaryOp::BitAnd | MirBinaryOp::BitOr => {
            is_machine_integer(kind) || matches!(kind, MirTypeKind::Scalar(MirScalarType::Bool))
        }
        MirBinaryOp::Shl | MirBinaryOp::Shr => is_machine_integer(kind),
        MirBinaryOp::Eq
        | MirBinaryOp::Ne
        | MirBinaryOp::Lt
        | MirBinaryOp::Le
        | MirBinaryOp::Gt
        | MirBinaryOp::Ge => matches!(kind, MirTypeKind::Scalar(_)),
    }
}

fn is_arithmetic_numeric(kind: &MirTypeKind) -> bool {
    is_machine_integer(kind) || is_float(kind)
}

fn is_machine_integer(kind: &MirTypeKind) -> bool {
    matches!(kind, MirTypeKind::Scalar(MirScalarType::Int { .. }))
}

fn is_integer_or_bool(kind: &MirTypeKind) -> bool {
    matches!(kind, MirTypeKind::Scalar(MirScalarType::Bool)) || is_integer(kind)
}

fn is_unsigned_integer(kind: &MirTypeKind) -> bool {
    matches!(
        kind,
        MirTypeKind::Scalar(MirScalarType::Int { signed: false, .. })
    )
}

fn is_signed_integer_or_float(kind: &MirTypeKind) -> bool {
    matches!(
        kind,
        MirTypeKind::Scalar(MirScalarType::Int { signed: true, .. })
            | MirTypeKind::Scalar(MirScalarType::Float { .. })
    )
}

fn is_integer(kind: &MirTypeKind) -> bool {
    matches!(
        kind,
        MirTypeKind::Scalar(MirScalarType::Int { .. } | MirScalarType::Char)
    )
}

fn is_float(kind: &MirTypeKind) -> bool {
    matches!(kind, MirTypeKind::Scalar(MirScalarType::Float { .. }))
}

fn is_pointer(kind: &MirTypeKind) -> bool {
    matches!(
        kind,
        MirTypeKind::RawPointer { .. } | MirTypeKind::Reference { .. }
    )
}

fn validate_span_opt(
    path: &str,
    span: Option<&MirSourceSpan>,
) -> Result<(), MirExecutableValidationError> {
    let Some(span) = span else {
        return Ok(());
    };
    if span.file.is_empty() || span.file.len() > MAX_EXECUTABLE_SOURCE_FILE_BYTES {
        return Err(error(
            format!("{path}.file"),
            "source file identity is empty or exceeds its byte bound",
        ));
    }
    if span
        .file
        .bytes()
        .any(|byte| byte == 0 || byte.is_ascii_control())
    {
        return Err(error(
            format!("{path}.file"),
            "source file identity contains a control byte",
        ));
    }
    if span.byte_start > span.byte_end {
        return Err(error(path, "source span start exceeds its end"));
    }
    if span.line == 0 || span.column == 0 {
        return Err(error(path, "source line and column are one-based"));
    }
    Ok(())
}

fn validate_identity(path: &str, identity: &str) -> Result<(), MirExecutableValidationError> {
    validate_bounded_text(path, identity, MAX_EXECUTABLE_IDENTITY_BYTES)
}

fn validate_name_opt(path: &str, name: Option<&str>) -> Result<(), MirExecutableValidationError> {
    if let Some(name) = name {
        validate_name(path, name)?;
    }
    Ok(())
}

fn validate_name(path: &str, name: &str) -> Result<(), MirExecutableValidationError> {
    validate_bounded_text(path, name, MAX_EXECUTABLE_IDENTITY_BYTES)
}

fn validate_bounded_text(
    path: &str,
    text: &str,
    maximum: usize,
) -> Result<(), MirExecutableValidationError> {
    if text.is_empty() || text.len() > maximum {
        return Err(error(path, "text is empty or exceeds its byte bound"));
    }
    if text
        .bytes()
        .any(|byte| byte == 0 || byte.is_ascii_control())
    {
        return Err(error(path, "text contains a control byte"));
    }
    Ok(())
}

fn bounded_len(
    path: &str,
    actual: usize,
    minimum: usize,
    maximum: usize,
) -> Result<(), MirExecutableValidationError> {
    if actual < minimum || actual > maximum {
        return Err(error(
            path,
            format!("length {actual} is outside {minimum}..={maximum}"),
        ));
    }
    Ok(())
}

fn map_type_error(path: &str, source: MirTypeValidationError) -> MirExecutableValidationError {
    error(
        format!("{path}.{}", source.path()),
        source.reason().to_owned(),
    )
}

fn validate_type_budget(types: &[MirSemanticType]) -> Result<(), MirExecutableValidationError> {
    let mut stack = types
        .iter()
        .enumerate()
        .map(|(index, ty)| (format!("module.types[{index}]"), ty, 1_usize))
        .collect::<Vec<_>>();
    let mut nodes = 0_usize;
    let mut items = types.len();
    while let Some((path, ty, depth)) = stack.pop() {
        nodes = nodes
            .checked_add(1)
            .ok_or_else(|| error(&path, "semantic type node count overflow"))?;
        if nodes > MAX_EXECUTABLE_TYPE_NODES {
            return Err(error(
                path,
                format!("semantic type graph exceeds {MAX_EXECUTABLE_TYPE_NODES} nodes"),
            ));
        }
        if depth > MAX_EXECUTABLE_TYPE_DEPTH {
            return Err(error(
                path,
                format!("semantic type graph exceeds depth {MAX_EXECUTABLE_TYPE_DEPTH}"),
            ));
        }
        match &ty.kind {
            MirTypeKind::RawPointer { pointee, .. } => {
                stack.push((format!("{path}.pointee"), pointee, depth + 1));
            }
            MirTypeKind::Reference { referent, .. } => {
                stack.push((format!("{path}.referent"), referent, depth + 1));
            }
            MirTypeKind::Slice { element } | MirTypeKind::Array { element, .. } => {
                stack.push((format!("{path}.element"), element, depth + 1));
            }
            MirTypeKind::Tuple(aggregate) => {
                push_aggregate_types(&path, aggregate, depth, &mut stack, &mut items)?;
            }
            MirTypeKind::Struct(structure) => {
                validate_identity(&format!("{path}.identity"), &structure.identity)?;
                push_aggregate_types(&path, &structure.aggregate, depth, &mut stack, &mut items)?;
            }
            MirTypeKind::Enum(enum_ty) => {
                validate_identity(&format!("{path}.identity"), &enum_ty.identity)?;
                bounded_len(
                    &format!("{path}.variants"),
                    enum_ty.variants.len(),
                    0,
                    MAX_EXECUTABLE_VARIANTS,
                )?;
                add_type_items(&path, &mut items, enum_ty.variants.len())?;
                for (variant_index, variant) in enum_ty.variants.iter().enumerate() {
                    validate_name(
                        &format!("{path}.variants[{variant_index}].name"),
                        &variant.name,
                    )?;
                    push_aggregate_types(
                        &format!("{path}.variants[{variant_index}]"),
                        &variant.aggregate,
                        depth,
                        &mut stack,
                        &mut items,
                    )?;
                }
            }
            MirTypeKind::Unit | MirTypeKind::Scalar(_) => {}
        }
    }
    Ok(())
}

fn push_aggregate_types<'a>(
    path: &str,
    aggregate: &'a crate::MirAggregateLayout,
    depth: usize,
    stack: &mut Vec<(String, &'a MirSemanticType, usize)>,
    items: &mut usize,
) -> Result<(), MirExecutableValidationError> {
    bounded_len(
        &format!("{path}.fields"),
        aggregate.fields.len(),
        0,
        MAX_EXECUTABLE_FIELDS,
    )?;
    add_type_items(path, items, aggregate.fields.len())?;
    add_type_items(path, items, aggregate.padding.len())?;
    bounded_len(
        &format!("{path}.padding"),
        aggregate.padding.len(),
        0,
        MAX_EXECUTABLE_FIELDS,
    )?;
    for (field_index, field) in aggregate.fields.iter().enumerate() {
        validate_name_opt(
            &format!("{path}.fields[{field_index}].name"),
            field.name.as_deref(),
        )?;
        stack.push((
            format!("{path}.fields[{field_index}].type"),
            &field.ty,
            depth + 1,
        ));
    }
    Ok(())
}

fn add_type_items(
    path: &str,
    items: &mut usize,
    additional: usize,
) -> Result<(), MirExecutableValidationError> {
    *items = items
        .checked_add(additional)
        .ok_or_else(|| error(path, "semantic type item count overflow"))?;
    if *items > MAX_EXECUTABLE_TYPE_ITEMS {
        return Err(error(
            path,
            format!("semantic type graph exceeds {MAX_EXECUTABLE_TYPE_ITEMS} structural items"),
        ));
    }
    Ok(())
}

fn error(path: impl Into<String>, reason: impl Into<String>) -> MirExecutableValidationError {
    MirExecutableValidationError::new(path, reason)
}
