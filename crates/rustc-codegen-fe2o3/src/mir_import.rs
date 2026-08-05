use crate::collector::CollectionResult;
use crate::trusted_device_items::{self, TrustedDeviceItem};
use dialect_mir::{MirAttr, MirOp, MirOpRecord, MirType};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_middle::mir::{
    BasicBlock, BinOp, Body, ConstOperand, Local, Operand, Place, ProjectionElem, Rvalue,
    SourceInfo, StatementKind, TerminatorKind, UnOp,
};
use rustc_middle::ty::{FloatTy, IntTy, Mutability, Ty, TyCtxt, TyKind, TypingEnv, UintTy};
use std::fmt::Write;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirModule {
    pub functions: Vec<MirFunction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFunction {
    pub export_name: String,
    pub rust_path: String,
    pub kind: MirFunctionKind,
    pub arg_count: usize,
    pub local_count: usize,
    pub locals: Vec<MirLocal>,
    pub blocks: Vec<MirBlock>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirFunctionKind {
    Kernel,
    Device,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirLocal {
    pub index: usize,
    pub role: MirLocalRole,
    pub ty: MirImportedType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirLocalRole {
    Return,
    Arg,
    Temp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirImportedType {
    pub kind: MirType,
    pub rust: String,
    pub shape: MirTypeShape,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirTypeShape {
    Unit,
    Bool,
    I32,
    I64,
    ISize,
    USize,
    F32,
    F64,
    Slice {
        element: Box<MirTypeShape>,
        mutable: bool,
    },
    DisjointSlice {
        element: Box<MirTypeShape>,
    },
    Reference {
        pointee: Box<MirTypeShape>,
        mutable: bool,
    },
    RawPointer {
        pointee: Box<MirTypeShape>,
        mutable: bool,
    },
    Adt {
        identity: String,
    },
    Tuple(Vec<MirTypeShape>),
    Unknown,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MirSourceLocation {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirBlock {
    pub index: usize,
    pub statements: Vec<MirStatement>,
    pub terminator: Option<MirTerminator>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStatement {
    pub index: usize,
    pub kind: MirStatementKind,
    pub destination: Option<MirPlaceRef>,
    pub operands: Vec<MirOperandRef>,
    pub rvalue: Option<MirRvalueKind>,
    /// Compatibility spelling consumed by the legacy record recognizer.
    pub operation: Option<String>,
    pub source: Option<MirSourceLocation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirStatementKind {
    Assign,
    StorageLive,
    StorageDead,
    SetDiscriminant,
    Intrinsic,
    Retag,
    Coverage,
    Nop,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirPlaceRef {
    pub local: usize,
    pub projection: Vec<MirProjectionElem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirProjectionElem {
    Deref,
    Field(usize),
    Index {
        local: usize,
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
        variant: usize,
    },
    OpaqueCast,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirOperandRef {
    Place(MirPlaceRef),
    Constant {
        ty: MirImportedType,
        literal: MirConstant,
        /// Compatibility spelling consumed by the legacy record recognizer.
        value: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirConstant {
    Bool(bool),
    I32(i32),
    I64(i64),
    ISize(i64),
    USize(u64),
    F32Bits(u32),
    F64Bits(u64),
    Unevaluated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirTerminator {
    pub kind: MirTerminatorKind,
    pub source: Option<MirSourceLocation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirCallee {
    identity: MirCalleeIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MirCalleeIdentity {
    Trusted(TrustedDeviceItem),
    Untrusted(String),
}

impl MirCallee {
    fn trusted(item: TrustedDeviceItem) -> Self {
        Self {
            identity: MirCalleeIdentity::Trusted(item),
        }
    }

    fn untrusted(identity: String) -> Self {
        Self {
            identity: MirCalleeIdentity::Untrusted(identity),
        }
    }

    pub(crate) fn identity(&self) -> &str {
        match &self.identity {
            MirCalleeIdentity::Trusted(item) => item.canonical_path(),
            MirCalleeIdentity::Untrusted(identity) => identity,
        }
    }

    pub(crate) fn trusted_item(&self) -> Option<TrustedDeviceItem> {
        match &self.identity {
            MirCalleeIdentity::Trusted(item) => Some(*item),
            MirCalleeIdentity::Untrusted(_) => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn trusted_for_test(item: TrustedDeviceItem) -> Self {
        Self::trusted(item)
    }

    #[cfg(test)]
    pub(crate) fn untrusted_for_test(identity: impl Into<String>) -> Self {
        Self::untrusted(identity.into())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSwitchTarget {
    pub value: u128,
    pub target: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirTerminatorKind {
    Return,
    Unreachable,
    Goto {
        target: usize,
    },
    SwitchInt {
        discriminant: MirOperandRef,
        targets: Vec<MirSwitchTarget>,
        otherwise: usize,
    },
    Call {
        callee: Option<MirCallee>,
        target: Option<usize>,
        destination: Option<MirPlaceRef>,
        operands: Vec<MirOperandRef>,
    },
    Assert {
        condition: MirOperandRef,
        expected: bool,
        target: usize,
    },
    Drop {
        target: usize,
    },
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirRvalueKind {
    Use,
    Repeat,
    Ref,
    RawPointer,
    Cast,
    Binary(MirBinaryOp),
    Unary(MirUnaryOp),
    Discriminant,
    Aggregate,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    Lt,
    Le,
    Ne,
    Ge,
    Gt,
    Cmp,
    Offset,
    AddUnchecked,
    SubUnchecked,
    MulUnchecked,
    ShlUnchecked,
    ShrUnchecked,
    AddWithOverflow,
    SubWithOverflow,
    MulWithOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirUnaryOp {
    Not,
    Neg,
    PtrMetadata,
}

pub fn import_collection<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
) -> MirModule {
    let functions = collection
        .functions
        .iter()
        .filter_map(|function| {
            let def_id = function.instance.def_id();
            if !tcx.is_mir_available(def_id) {
                return None;
            }

            let body = tcx.instance_mir(function.instance.def);
            let rust_path = if def_id.krate == LOCAL_CRATE {
                format!(
                    "{}::{}",
                    tcx.crate_name(LOCAL_CRATE),
                    tcx.def_path_str(def_id)
                )
            } else {
                tcx.def_path_str(def_id)
            };
            Some(import_body(
                tcx,
                body,
                function.export_name.clone(),
                rust_path,
                if function.is_kernel {
                    MirFunctionKind::Kernel
                } else {
                    MirFunctionKind::Device
                },
            ))
        })
        .collect();

    MirModule { functions }
}

impl MirModule {
    pub fn summary(&self) -> String {
        let record_count = self.dialect_records().len();
        let mut output = format!(
            "\n=== fe2o3 MIR import scaffold ({}, {record_count} op records) ===\n",
            MirOp::Module.name(),
        );
        for function in &self.functions {
            let kind = match function.kind {
                MirFunctionKind::Kernel => "kernel",
                MirFunctionKind::Device => "device",
            };
            let _ = writeln!(
                output,
                "  [{kind}] {} ({})",
                function.export_name,
                MirOp::Func.name()
            );
            let _ = writeln!(output, "      path: {}", function.rust_path);
            let _ = writeln!(
                output,
                "      MIR:  {} bb, {} locals, {} args",
                function.blocks.len(),
                function.local_count,
                function.arg_count
            );
            for local in function
                .locals
                .iter()
                .filter(|local| local.role != MirLocalRole::Temp)
            {
                let role = match local.role {
                    MirLocalRole::Return => "return",
                    MirLocalRole::Arg => "arg",
                    MirLocalRole::Temp => "temp",
                };
                let _ = writeln!(
                    output,
                    "      local{}: {role} {} ({})",
                    local.index,
                    local.ty.kind.name(),
                    local.ty.rust
                );
            }
            for block in &function.blocks {
                let terminator = block
                    .terminator
                    .as_ref()
                    .map(|terminator| terminator.kind.summary())
                    .unwrap_or("missing terminator".to_string());
                let _ = writeln!(
                    output,
                    "      bb{} ({}): {} stmt(s), {terminator}",
                    block.index,
                    MirOp::Block.name(),
                    block.statements.len()
                );
                for statement in &block.statements {
                    if let Some(summary) = statement.summary() {
                        let _ = writeln!(output, "          {summary}");
                    }
                }
            }
        }
        output.push_str("===================================\n");
        output
    }

    pub fn dialect_records(&self) -> Vec<MirOpRecord> {
        let mut records = vec![
            MirOpRecord::new(MirOp::Module)
                .with_attr(MirAttr::usize("functions", self.functions.len())),
        ];

        for function in &self.functions {
            let kind = match function.kind {
                MirFunctionKind::Kernel => "kernel",
                MirFunctionKind::Device => "device",
            };
            records.push(
                MirOpRecord::new(MirOp::Func)
                    .with_attr(MirAttr::string("symbol", &function.export_name))
                    .with_attr(MirAttr::string("kind", kind))
                    .with_attr(MirAttr::usize("args", function.arg_count))
                    .with_attr(MirAttr::usize("locals", function.local_count))
                    .with_attr(MirAttr::usize("blocks", function.blocks.len())),
            );

            for local in &function.locals {
                let role = match local.role {
                    MirLocalRole::Return => "return",
                    MirLocalRole::Arg => "arg",
                    MirLocalRole::Temp => "temp",
                };
                let op = match local.role {
                    MirLocalRole::Arg => MirOp::Arg,
                    MirLocalRole::Return | MirLocalRole::Temp => MirOp::Local,
                };
                records.push(
                    MirOpRecord::new(op)
                        .with_attr(MirAttr::string("function", &function.export_name))
                        .with_attr(MirAttr::usize("index", local.index))
                        .with_attr(MirAttr::string("role", role))
                        .with_attr(MirAttr::string("type", local.ty.kind.name()))
                        .with_attr(MirAttr::string("rust_type", &local.ty.rust)),
                );
            }

            for block in &function.blocks {
                records.push(
                    MirOpRecord::new(MirOp::Block)
                        .with_attr(MirAttr::string("function", &function.export_name))
                        .with_attr(MirAttr::usize("index", block.index))
                        .with_attr(MirAttr::usize("statements", block.statements.len())),
                );

                for statement in &block.statements {
                    records.push(statement.record(&function.export_name, block.index));
                    if let Some(record) =
                        statement.lowering_record(&function.export_name, block.index)
                    {
                        records.push(record);
                    }
                }

                if let Some(terminator) = &block.terminator {
                    records.push(terminator.kind.record(&function.export_name, block.index));
                }
            }
        }

        records
    }
}

impl MirStatement {
    fn summary(&self) -> Option<String> {
        if self.kind != MirStatementKind::Assign {
            return None;
        }

        let destination = self
            .destination
            .as_ref()
            .map(MirPlaceRef::label)
            .unwrap_or_else(|| "_".to_string());
        let operation = self.operation.as_deref().unwrap_or(self.kind.name());
        let dialect_op = self.lowering_op().unwrap_or_else(|| self.dialect_op());
        if self.operands.is_empty() {
            return Some(format!(
                "stmt{}: {} {destination} = {operation}",
                self.index,
                dialect_op.name()
            ));
        }

        let operands = self
            .operands
            .iter()
            .map(MirOperandRef::label)
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!(
            "stmt{}: {} {destination} = {operation}({operands})",
            self.index,
            dialect_op.name()
        ))
    }

    fn record(&self, function: &str, block: usize) -> MirOpRecord {
        let mut record = MirOpRecord::new(self.dialect_op())
            .with_attr(MirAttr::string("function", function))
            .with_attr(MirAttr::usize("block", block))
            .with_attr(MirAttr::usize("index", self.index))
            .with_attr(MirAttr::string("kind", self.kind.name()))
            .with_attr(MirAttr::usize("operand_count", self.operands.len()));

        if let Some(destination) = &self.destination {
            record
                .attrs
                .push(MirAttr::usize("destination_local", destination.local));
            record
                .attrs
                .push(MirAttr::string("destination", destination.label()));
        }
        if let Some(operation) = &self.operation {
            record.attrs.push(MirAttr::string("operation", operation));
        }
        if !self.operands.is_empty() {
            let operands = self
                .operands
                .iter()
                .map(MirOperandRef::label)
                .collect::<Vec<_>>()
                .join(", ");
            record.attrs.push(MirAttr::string("operands", operands));
        }

        record
    }

    fn lowering_record(&self, function: &str, block: usize) -> Option<MirOpRecord> {
        let op = self.lowering_op()?;
        let mut record = MirOpRecord::new(op)
            .with_attr(MirAttr::string("function", function))
            .with_attr(MirAttr::usize("block", block))
            .with_attr(MirAttr::usize("statement", self.index))
            .with_attr(MirAttr::string("source", self.dialect_op().name()))
            .with_attr(MirAttr::usize("operand_count", self.operands.len()));

        if let Some(destination) = &self.destination {
            record
                .attrs
                .push(MirAttr::usize("destination_local", destination.local));
            record
                .attrs
                .push(MirAttr::string("destination", destination.label()));
        }
        if let Some(operation) = &self.operation {
            record.attrs.push(MirAttr::string("operation", operation));
        }
        if !self.operands.is_empty() {
            let operands = self
                .operands
                .iter()
                .map(MirOperandRef::label)
                .collect::<Vec<_>>()
                .join(", ");
            record.attrs.push(MirAttr::string("operands", operands));
        }

        Some(record)
    }

    fn dialect_op(&self) -> MirOp {
        match self.kind {
            MirStatementKind::Assign => MirOp::Assign,
            MirStatementKind::StorageLive
            | MirStatementKind::StorageDead
            | MirStatementKind::SetDiscriminant
            | MirStatementKind::Intrinsic
            | MirStatementKind::Retag
            | MirStatementKind::Coverage
            | MirStatementKind::Nop
            | MirStatementKind::Other => MirOp::Statement,
        }
    }

    fn lowering_op(&self) -> Option<MirOp> {
        if self.kind != MirStatementKind::Assign {
            return None;
        }
        if self
            .destination
            .as_ref()
            .is_some_and(MirPlaceRef::is_memory_projection)
        {
            return Some(MirOp::Store);
        }

        match self.rvalue? {
            MirRvalueKind::Binary(
                MirBinaryOp::Add | MirBinaryOp::AddUnchecked | MirBinaryOp::AddWithOverflow,
            ) => Some(MirOp::Add),
            MirRvalueKind::Binary(
                MirBinaryOp::Sub | MirBinaryOp::SubUnchecked | MirBinaryOp::SubWithOverflow,
            ) => Some(MirOp::Sub),
            MirRvalueKind::Binary(
                MirBinaryOp::Mul | MirBinaryOp::MulUnchecked | MirBinaryOp::MulWithOverflow,
            ) => Some(MirOp::Mul),
            MirRvalueKind::Binary(MirBinaryOp::Div) => Some(MirOp::Div),
            MirRvalueKind::Binary(MirBinaryOp::Eq) => Some(MirOp::Eq),
            MirRvalueKind::Binary(MirBinaryOp::Lt) => Some(MirOp::Lt),
            MirRvalueKind::Binary(MirBinaryOp::Le) => Some(MirOp::Le),
            MirRvalueKind::Binary(MirBinaryOp::Ne) => Some(MirOp::Ne),
            MirRvalueKind::Binary(MirBinaryOp::Ge) => Some(MirOp::Ge),
            MirRvalueKind::Binary(MirBinaryOp::Gt) => Some(MirOp::Gt),
            MirRvalueKind::Binary(MirBinaryOp::Cmp) => Some(MirOp::Cmp),
            MirRvalueKind::Cast => Some(MirOp::Cast),
            MirRvalueKind::Binary(MirBinaryOp::Offset) => Some(MirOp::Gep),
            MirRvalueKind::Unary(MirUnaryOp::PtrMetadata) => Some(MirOp::SliceLen),
            MirRvalueKind::Use if self.operands.iter().any(MirOperandRef::is_memory_place) => {
                Some(MirOp::Load)
            }
            _ => None,
        }
    }
}

impl MirStatementKind {
    fn name(self) -> &'static str {
        match self {
            Self::Assign => "assign",
            Self::StorageLive => "storage_live",
            Self::StorageDead => "storage_dead",
            Self::SetDiscriminant => "set_discriminant",
            Self::Intrinsic => "intrinsic",
            Self::Retag => "retag",
            Self::Coverage => "coverage",
            Self::Nop => "nop",
            Self::Other => "other",
        }
    }
}

impl MirPlaceRef {
    fn local(local: Local) -> Self {
        Self {
            local: local.as_usize(),
            projection: Vec::new(),
        }
    }

    fn label(&self) -> String {
        let mut label = format!("local{}", self.local);
        for projection in &self.projection {
            label.push('.');
            label.push_str(&projection.label());
        }
        label
    }

    fn is_memory_projection(&self) -> bool {
        self.projection.iter().any(|projection| {
            matches!(
                projection,
                MirProjectionElem::Deref
                    | MirProjectionElem::Index { .. }
                    | MirProjectionElem::ConstantIndex { .. }
            )
        })
    }
}

impl MirProjectionElem {
    fn label(&self) -> String {
        match self {
            Self::Deref => "deref".to_string(),
            Self::Field(field) => format!("field{field}"),
            Self::Index { local } => format!("index_local{local}"),
            Self::ConstantIndex {
                offset,
                min_length,
                from_end,
            } => format!("constant_index{offset}_min{min_length}_from_end{from_end}"),
            Self::Subslice { from, to, from_end } => {
                format!("subslice{from}_{to}_from_end{from_end}")
            }
            Self::Downcast { variant } => format!("downcast{variant}"),
            Self::OpaqueCast => "opaque_cast".to_string(),
            Self::Other => "projection".to_string(),
        }
    }
}

impl MirOperandRef {
    fn label(&self) -> String {
        match self {
            Self::Place(place) => place.label(),
            Self::Constant { ty, value, .. } => format!("const:{}={value}", ty.kind.name()),
        }
    }

    fn is_memory_place(&self) -> bool {
        matches!(self, Self::Place(place) if place.is_memory_projection())
    }
}

impl MirTerminatorKind {
    fn record(&self, function: &str, block: usize) -> MirOpRecord {
        let mut record = MirOpRecord::new(self.dialect_op())
            .with_attr(MirAttr::string("function", function))
            .with_attr(MirAttr::usize("block", block));

        match self {
            Self::Goto { target } | Self::Assert { target, .. } | Self::Drop { target } => {
                record.attrs.push(MirAttr::usize("target", *target));
            }
            Self::SwitchInt { targets, .. } => {
                record
                    .attrs
                    .push(MirAttr::usize("targets", targets.len() + 1));
            }
            Self::Call {
                callee,
                target,
                destination,
                operands,
            } => {
                if let Some(callee) = callee {
                    record
                        .attrs
                        .push(MirAttr::string("callee", callee.identity()));
                }
                if let Some(target) = target {
                    record.attrs.push(MirAttr::usize("target", *target));
                }
                if let Some(destination) = destination {
                    record
                        .attrs
                        .push(MirAttr::usize("destination_local", destination.local));
                    record
                        .attrs
                        .push(MirAttr::string("destination", destination.label()));
                }
                record
                    .attrs
                    .push(MirAttr::usize("operand_count", operands.len()));
                if !operands.is_empty() {
                    let operands = operands
                        .iter()
                        .map(MirOperandRef::label)
                        .collect::<Vec<_>>()
                        .join(", ");
                    record.attrs.push(MirAttr::string("operands", operands));
                }
            }
            Self::Return | Self::Unreachable | Self::Other => {}
        }

        record
    }

    fn dialect_op(&self) -> MirOp {
        match self {
            Self::Return => MirOp::Return,
            Self::Unreachable => MirOp::Unreachable,
            Self::Goto { .. } => MirOp::Branch,
            Self::SwitchInt { .. } => MirOp::Switch,
            Self::Call { .. } => MirOp::Call,
            Self::Assert { .. } => MirOp::Assert,
            Self::Drop { .. } => MirOp::Drop,
            Self::Other => MirOp::Other,
        }
    }

    fn summary(&self) -> String {
        match self {
            Self::Return => MirOp::Return.name().to_string(),
            Self::Unreachable => MirOp::Unreachable.name().to_string(),
            Self::Goto { target } => format!("{} -> bb{target}", MirOp::Branch.name()),
            Self::SwitchInt { targets, .. } => {
                format!("{} ({} target(s))", MirOp::Switch.name(), targets.len() + 1)
            }
            Self::Call { callee, target, .. } => {
                let callee = callee
                    .as_ref()
                    .map(MirCallee::identity)
                    .unwrap_or("<dynamic>");
                match target {
                    Some(target) => format!("{} {callee} -> bb{target}", MirOp::Call.name()),
                    None => format!("{} {callee} -> return", MirOp::Call.name()),
                }
            }
            Self::Assert { target, .. } => format!("{} -> bb{target}", MirOp::Assert.name()),
            Self::Drop { target } => format!("{} -> bb{target}", MirOp::Drop.name()),
            Self::Other => "other".to_string(),
        }
    }
}

fn import_body<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    export_name: String,
    rust_path: String,
    kind: MirFunctionKind,
) -> MirFunction {
    let blocks = body
        .basic_blocks
        .iter_enumerated()
        .map(|(index, block)| MirBlock {
            index: index.as_usize(),
            statements: block
                .statements
                .iter()
                .enumerate()
                .map(|(statement_index, statement)| {
                    import_statement(tcx, statement_index, &statement.kind, statement.source_info)
                })
                .collect(),
            terminator: block.terminator.as_ref().map(|terminator| MirTerminator {
                kind: terminator_kind(tcx, &terminator.kind),
                source: Some(import_source_location(tcx, terminator.source_info)),
            }),
        })
        .collect();
    let locals = body
        .local_decls
        .iter_enumerated()
        .map(|(local, decl)| {
            let index = local.as_usize();
            let role = if index == 0 {
                MirLocalRole::Return
            } else if index <= body.arg_count {
                MirLocalRole::Arg
            } else {
                MirLocalRole::Temp
            };
            MirLocal {
                index,
                role,
                ty: import_type(tcx, decl.ty),
            }
        })
        .collect();

    MirFunction {
        export_name,
        rust_path,
        kind,
        arg_count: body.arg_count,
        local_count: body.local_decls.len(),
        locals,
        blocks,
    }
}

fn import_type<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> MirImportedType {
    let kind = match ty.kind() {
        TyKind::Bool => MirType::I1,
        TyKind::Int(IntTy::I32) => MirType::I32,
        TyKind::Int(IntTy::I64) => MirType::I64,
        TyKind::Uint(UintTy::Usize) => MirType::USize,
        TyKind::Float(FloatTy::F32) => MirType::F32,
        TyKind::Float(FloatTy::F64) => MirType::F64,
        TyKind::Ref(_, pointee, _) => match pointee.kind() {
            TyKind::Slice(_) => MirType::Slice,
            _ => MirType::Ptr,
        },
        TyKind::RawPtr(_, _) => MirType::Ptr,
        TyKind::Adt(adt, _)
            if trusted_device_items::classify(tcx, adt.did())
                == Some(TrustedDeviceItem::DisjointSlice) =>
        {
            MirType::DisjointSlice
        }
        TyKind::Tuple(elements) if elements.is_empty() => MirType::Unit,
        _ => MirType::Unknown,
    };

    MirImportedType {
        kind,
        rust: ty.to_string(),
        shape: import_type_shape(tcx, ty),
    }
}

fn import_type_shape<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> MirTypeShape {
    match ty.kind() {
        TyKind::Bool => MirTypeShape::Bool,
        TyKind::Int(IntTy::I32) => MirTypeShape::I32,
        TyKind::Int(IntTy::I64) => MirTypeShape::I64,
        TyKind::Int(IntTy::Isize) => MirTypeShape::ISize,
        TyKind::Uint(UintTy::Usize) => MirTypeShape::USize,
        TyKind::Float(FloatTy::F32) => MirTypeShape::F32,
        TyKind::Float(FloatTy::F64) => MirTypeShape::F64,
        TyKind::Ref(_, pointee, mutability) => match pointee.kind() {
            TyKind::Slice(element) => MirTypeShape::Slice {
                element: Box::new(import_type_shape(tcx, *element)),
                mutable: *mutability == Mutability::Mut,
            },
            _ => MirTypeShape::Reference {
                pointee: Box::new(import_type_shape(tcx, *pointee)),
                mutable: *mutability == Mutability::Mut,
            },
        },
        TyKind::RawPtr(pointee, mutability) => MirTypeShape::RawPointer {
            pointee: Box::new(import_type_shape(tcx, *pointee)),
            mutable: *mutability == Mutability::Mut,
        },
        TyKind::Adt(adt, args) if is_disjoint_slice(tcx, adt.did()) => {
            MirTypeShape::DisjointSlice {
                element: Box::new(import_type_shape(tcx, args.type_at(0))),
            }
        }
        TyKind::Adt(adt, _) => MirTypeShape::Adt {
            identity: tcx.def_path_str(adt.did()),
        },
        TyKind::Tuple(elements) if elements.is_empty() => MirTypeShape::Unit,
        TyKind::Tuple(elements) => MirTypeShape::Tuple(
            elements
                .iter()
                .map(|element| import_type_shape(tcx, element))
                .collect(),
        ),
        _ => MirTypeShape::Unknown,
    }
}

fn is_disjoint_slice(tcx: TyCtxt<'_>, def_id: rustc_hir::def_id::DefId) -> bool {
    trusted_device_items::classify(tcx, def_id) == Some(TrustedDeviceItem::DisjointSlice)
}

fn import_statement<'tcx>(
    tcx: TyCtxt<'tcx>,
    index: usize,
    kind: &StatementKind<'tcx>,
    source_info: SourceInfo,
) -> MirStatement {
    MirStatement {
        index,
        kind: statement_kind(kind),
        destination: statement_destination(kind),
        operands: statement_operands(tcx, kind),
        rvalue: statement_rvalue(kind),
        operation: statement_operation(kind),
        source: Some(import_source_location(tcx, source_info)),
    }
}

fn import_source_location(tcx: TyCtxt<'_>, source_info: SourceInfo) -> MirSourceLocation {
    let location = tcx.sess.source_map().lookup_char_pos(source_info.span.lo());
    MirSourceLocation {
        file: location
            .file
            .name
            .prefer_remapped_unconditionally()
            .to_string_lossy()
            .into_owned(),
        line: location.line,
        column: location.col.0 + 1,
    }
}

fn statement_kind(kind: &StatementKind<'_>) -> MirStatementKind {
    match kind {
        StatementKind::Assign(_) => MirStatementKind::Assign,
        StatementKind::StorageLive(_) => MirStatementKind::StorageLive,
        StatementKind::StorageDead(_) => MirStatementKind::StorageDead,
        StatementKind::SetDiscriminant { .. } => MirStatementKind::SetDiscriminant,
        StatementKind::Intrinsic(_) => MirStatementKind::Intrinsic,
        StatementKind::Retag(_, _) => MirStatementKind::Retag,
        StatementKind::Coverage(_) => MirStatementKind::Coverage,
        StatementKind::Nop => MirStatementKind::Nop,
        _ => MirStatementKind::Other,
    }
}

fn statement_destination(kind: &StatementKind<'_>) -> Option<MirPlaceRef> {
    match kind {
        StatementKind::Assign(assign) => {
            let (place, _) = &**assign;
            Some(import_place(*place))
        }
        StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
            Some(MirPlaceRef::local(*local))
        }
        StatementKind::SetDiscriminant { place, .. } => Some(import_place(**place)),
        _ => None,
    }
}

fn statement_operands<'tcx>(tcx: TyCtxt<'tcx>, kind: &StatementKind<'tcx>) -> Vec<MirOperandRef> {
    let StatementKind::Assign(assign) = kind else {
        return Vec::new();
    };
    let (_, rvalue) = &**assign;
    rvalue_operands(tcx, rvalue)
}

fn statement_operation(kind: &StatementKind<'_>) -> Option<String> {
    let StatementKind::Assign(assign) = kind else {
        return None;
    };
    let (_, rvalue) = &**assign;
    Some(rvalue_operation(rvalue).to_string())
}

fn statement_rvalue(kind: &StatementKind<'_>) -> Option<MirRvalueKind> {
    let StatementKind::Assign(assign) = kind else {
        return None;
    };
    let (_, rvalue) = &**assign;
    Some(import_rvalue_kind(rvalue))
}

fn rvalue_operands<'tcx>(tcx: TyCtxt<'tcx>, rvalue: &Rvalue<'tcx>) -> Vec<MirOperandRef> {
    match rvalue {
        Rvalue::Use(operand)
        | Rvalue::Repeat(operand, _)
        | Rvalue::Cast(_, operand, _)
        | Rvalue::UnaryOp(_, operand) => vec![import_operand(tcx, operand)],
        Rvalue::BinaryOp(_, operands) => vec![
            import_operand(tcx, &operands.0),
            import_operand(tcx, &operands.1),
        ],
        Rvalue::Ref(_, _, place) | Rvalue::RawPtr(_, place) | Rvalue::Discriminant(place) => {
            vec![MirOperandRef::Place(import_place(*place))]
        }
        Rvalue::Aggregate(_, operands) => operands
            .iter()
            .map(|operand| import_operand(tcx, operand))
            .collect(),
        _ => Vec::new(),
    }
}

fn rvalue_operation(rvalue: &Rvalue<'_>) -> &'static str {
    match rvalue {
        Rvalue::Use(_) => "use",
        Rvalue::Repeat(_, _) => "repeat",
        Rvalue::Ref(_, _, _) => "ref",
        Rvalue::RawPtr(_, _) => "raw_ptr",
        Rvalue::Cast(_, _, _) => "cast",
        Rvalue::BinaryOp(op, _) => bin_op_name(*op),
        Rvalue::UnaryOp(op, _) => unary_op_name(*op),
        Rvalue::Discriminant(_) => "discriminant",
        Rvalue::Aggregate(_, _) => "aggregate",
        _ => "other",
    }
}

fn import_rvalue_kind(rvalue: &Rvalue<'_>) -> MirRvalueKind {
    match rvalue {
        Rvalue::Use(_) => MirRvalueKind::Use,
        Rvalue::Repeat(_, _) => MirRvalueKind::Repeat,
        Rvalue::Ref(_, _, _) => MirRvalueKind::Ref,
        Rvalue::RawPtr(_, _) => MirRvalueKind::RawPointer,
        Rvalue::Cast(_, _, _) => MirRvalueKind::Cast,
        Rvalue::BinaryOp(op, _) => MirRvalueKind::Binary(import_binary_op(*op)),
        Rvalue::UnaryOp(op, _) => MirRvalueKind::Unary(import_unary_op(*op)),
        Rvalue::Discriminant(_) => MirRvalueKind::Discriminant,
        Rvalue::Aggregate(_, _) => MirRvalueKind::Aggregate,
        _ => MirRvalueKind::Other,
    }
}

fn import_operand<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>) -> MirOperandRef {
    if let Some(place) = operand.place() {
        return MirOperandRef::Place(import_place(place));
    }

    let Operand::Constant(constant) = operand else {
        return MirOperandRef::Constant {
            ty: MirImportedType {
                kind: MirType::Unknown,
                rust: "<unknown>".to_string(),
                shape: MirTypeShape::Unknown,
            },
            literal: MirConstant::Unevaluated,
            value: "<unknown>".to_string(),
        };
    };

    MirOperandRef::Constant {
        ty: import_type(tcx, constant.const_.ty()),
        literal: import_constant(tcx, constant),
        value: constant_value_label(tcx, constant),
    }
}

fn import_constant<'tcx>(tcx: TyCtxt<'tcx>, constant: &ConstOperand<'tcx>) -> MirConstant {
    let typing_env = TypingEnv::fully_monomorphized();
    match constant.const_.ty().kind() {
        TyKind::Uint(UintTy::Usize) => constant
            .const_
            .try_eval_target_usize(tcx, typing_env)
            .map(MirConstant::USize)
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Int(IntTy::Isize) => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .map(|value| MirConstant::ISize(value.to_target_isize(tcx)))
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Bool => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .and_then(|value| value.try_to_bool().ok())
            .map(MirConstant::Bool)
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Int(IntTy::I32) => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .map(|value| MirConstant::I32(value.to_i32()))
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Int(IntTy::I64) => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .map(|value| MirConstant::I64(value.to_i64()))
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Float(FloatTy::F32) => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .map(|value| MirConstant::F32Bits(value.to_u32()))
            .unwrap_or(MirConstant::Unevaluated),
        TyKind::Float(FloatTy::F64) => constant
            .const_
            .try_eval_scalar_int(tcx, typing_env)
            .map(|value| MirConstant::F64Bits(value.to_u64()))
            .unwrap_or(MirConstant::Unevaluated),
        _ => MirConstant::Unevaluated,
    }
}

fn constant_value_label<'tcx>(tcx: TyCtxt<'tcx>, constant: &ConstOperand<'tcx>) -> String {
    let debug = format!("{:?}", constant.const_);
    match constant.const_.ty().kind() {
        TyKind::Uint(UintTy::Usize) => constant
            .const_
            .try_eval_target_usize(tcx, TypingEnv::fully_monomorphized())
            .map(|value| format!("{debug};eval_u64={value}"))
            .unwrap_or(debug),
        TyKind::Int(IntTy::Isize) => constant
            .const_
            .try_eval_scalar_int(tcx, TypingEnv::fully_monomorphized())
            .map(|value| format!("{debug};eval_i64={}", value.to_target_isize(tcx)))
            .unwrap_or(debug),
        _ => debug,
    }
}

fn import_place(place: Place<'_>) -> MirPlaceRef {
    MirPlaceRef {
        local: place.local.as_usize(),
        projection: place
            .projection
            .iter()
            .map(import_projection_elem)
            .collect(),
    }
}

fn import_projection_elem(element: ProjectionElem<Local, Ty<'_>>) -> MirProjectionElem {
    match element {
        ProjectionElem::Deref => MirProjectionElem::Deref,
        ProjectionElem::Field(field, _) => MirProjectionElem::Field(field.index()),
        ProjectionElem::Index(local) => MirProjectionElem::Index {
            local: local.as_usize(),
        },
        ProjectionElem::ConstantIndex {
            offset,
            min_length,
            from_end,
        } => MirProjectionElem::ConstantIndex {
            offset,
            min_length,
            from_end,
        },
        ProjectionElem::Subslice { from, to, from_end } => {
            MirProjectionElem::Subslice { from, to, from_end }
        }
        ProjectionElem::Downcast(_, variant) => MirProjectionElem::Downcast {
            variant: variant.index(),
        },
        ProjectionElem::OpaqueCast(_) => MirProjectionElem::OpaqueCast,
        _ => MirProjectionElem::Other,
    }
}

fn import_binary_op(op: BinOp) -> MirBinaryOp {
    match op {
        BinOp::Add => MirBinaryOp::Add,
        BinOp::Sub => MirBinaryOp::Sub,
        BinOp::Mul => MirBinaryOp::Mul,
        BinOp::Div => MirBinaryOp::Div,
        BinOp::Rem => MirBinaryOp::Rem,
        BinOp::BitXor => MirBinaryOp::BitXor,
        BinOp::BitAnd => MirBinaryOp::BitAnd,
        BinOp::BitOr => MirBinaryOp::BitOr,
        BinOp::Shl => MirBinaryOp::Shl,
        BinOp::Shr => MirBinaryOp::Shr,
        BinOp::Eq => MirBinaryOp::Eq,
        BinOp::Lt => MirBinaryOp::Lt,
        BinOp::Le => MirBinaryOp::Le,
        BinOp::Ne => MirBinaryOp::Ne,
        BinOp::Ge => MirBinaryOp::Ge,
        BinOp::Gt => MirBinaryOp::Gt,
        BinOp::Cmp => MirBinaryOp::Cmp,
        BinOp::Offset => MirBinaryOp::Offset,
        BinOp::AddUnchecked => MirBinaryOp::AddUnchecked,
        BinOp::SubUnchecked => MirBinaryOp::SubUnchecked,
        BinOp::MulUnchecked => MirBinaryOp::MulUnchecked,
        BinOp::ShlUnchecked => MirBinaryOp::ShlUnchecked,
        BinOp::ShrUnchecked => MirBinaryOp::ShrUnchecked,
        BinOp::AddWithOverflow => MirBinaryOp::AddWithOverflow,
        BinOp::SubWithOverflow => MirBinaryOp::SubWithOverflow,
        BinOp::MulWithOverflow => MirBinaryOp::MulWithOverflow,
    }
}

fn import_unary_op(op: UnOp) -> MirUnaryOp {
    match op {
        UnOp::Not => MirUnaryOp::Not,
        UnOp::Neg => MirUnaryOp::Neg,
        UnOp::PtrMetadata => MirUnaryOp::PtrMetadata,
    }
}

fn bin_op_name(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div => "div",
        BinOp::Rem => "rem",
        BinOp::BitXor => "bitxor",
        BinOp::BitAnd => "bitand",
        BinOp::BitOr => "bitor",
        BinOp::Shl => "shl",
        BinOp::Shr => "shr",
        BinOp::Eq => "eq",
        BinOp::Lt => "lt",
        BinOp::Le => "le",
        BinOp::Ne => "ne",
        BinOp::Ge => "ge",
        BinOp::Gt => "gt",
        BinOp::Cmp => "cmp",
        BinOp::Offset => "offset",
        BinOp::AddUnchecked => "add_unchecked",
        BinOp::SubUnchecked => "sub_unchecked",
        BinOp::MulUnchecked => "mul_unchecked",
        BinOp::ShlUnchecked => "shl_unchecked",
        BinOp::ShrUnchecked => "shr_unchecked",
        BinOp::AddWithOverflow => "add_with_overflow",
        BinOp::SubWithOverflow => "sub_with_overflow",
        BinOp::MulWithOverflow => "mul_with_overflow",
    }
}

fn unary_op_name(op: UnOp) -> &'static str {
    match op {
        UnOp::Not => "not",
        UnOp::Neg => "neg",
        UnOp::PtrMetadata => "ptr_metadata",
    }
}

fn terminator_kind<'tcx>(tcx: TyCtxt<'tcx>, kind: &TerminatorKind<'tcx>) -> MirTerminatorKind {
    match kind {
        TerminatorKind::Return => MirTerminatorKind::Return,
        TerminatorKind::Unreachable => MirTerminatorKind::Unreachable,
        TerminatorKind::Goto { target } => MirTerminatorKind::Goto {
            target: target.as_usize(),
        },
        TerminatorKind::SwitchInt { discr, targets } => MirTerminatorKind::SwitchInt {
            discriminant: import_operand(tcx, discr),
            targets: targets
                .iter()
                .map(|(value, target)| MirSwitchTarget {
                    value,
                    target: target.as_usize(),
                })
                .collect(),
            otherwise: targets.otherwise().as_usize(),
        },
        TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            ..
        } => MirTerminatorKind::Call {
            callee: call_identity(tcx, func),
            target: target.map(BasicBlock::as_usize),
            destination: Some(import_place(*destination)),
            operands: args
                .iter()
                .map(|arg| import_operand(tcx, &arg.node))
                .collect(),
        },
        TerminatorKind::Assert {
            cond,
            expected,
            target,
            ..
        } => MirTerminatorKind::Assert {
            condition: import_operand(tcx, cond),
            expected: *expected,
            target: target.as_usize(),
        },
        TerminatorKind::Drop { target, .. } => MirTerminatorKind::Drop {
            target: target.as_usize(),
        },
        _ => MirTerminatorKind::Other,
    }
}

fn call_identity<'tcx>(tcx: TyCtxt<'tcx>, func: &Operand<'tcx>) -> Option<MirCallee> {
    let Operand::Constant(constant) = func else {
        return None;
    };
    let TyKind::FnDef(def_id, _) = constant.const_.ty().kind() else {
        return None;
    };
    Some(
        trusted_device_items::classify(tcx, *def_id)
            .map(MirCallee::trusted)
            .unwrap_or_else(|| MirCallee::untrusted(tcx.def_path_str(*def_id))),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_includes_function_and_block_shape() {
        let module = MirModule {
            functions: vec![MirFunction {
                export_name: "vecadd".to_string(),
                rust_path: "fe2o3_vecadd::fe2o3_kernel_vecadd".to_string(),
                kind: MirFunctionKind::Kernel,
                arg_count: 3,
                local_count: 17,
                locals: vec![
                    MirLocal {
                        index: 0,
                        role: MirLocalRole::Return,
                        ty: MirImportedType {
                            kind: MirType::Unit,
                            rust: "()".to_string(),
                            shape: MirTypeShape::Unit,
                        },
                    },
                    MirLocal {
                        index: 1,
                        role: MirLocalRole::Arg,
                        ty: MirImportedType {
                            kind: MirType::Slice,
                            rust: "&[f32]".to_string(),
                            shape: MirTypeShape::Slice {
                                element: Box::new(MirTypeShape::F32),
                                mutable: false,
                            },
                        },
                    },
                ],
                blocks: vec![MirBlock {
                    index: 0,
                    statements: vec![
                        simple_statement(0, MirStatementKind::StorageLive),
                        simple_statement(1, MirStatementKind::Assign),
                    ],
                    terminator: Some(MirTerminator {
                        kind: MirTerminatorKind::Goto { target: 1 },
                        source: None,
                    }),
                }],
            }],
        };

        let summary = module.summary();

        assert!(summary.contains("[kernel] vecadd (mir.func)"));
        assert!(summary.contains("fe2o3_vecadd::fe2o3_kernel_vecadd"));
        assert!(summary.contains("1 bb, 17 locals, 3 args"));
        assert!(summary.contains("local1: arg mir.slice (&[f32])"));
        assert!(summary.contains("bb0 (mir.block): 2 stmt(s), mir.br -> bb1"));
    }

    #[test]
    fn dialect_records_include_function_blocks_and_terminators() {
        let module = MirModule {
            functions: vec![MirFunction {
                export_name: "vecadd".to_string(),
                rust_path: "fe2o3_vecadd::fe2o3_kernel_vecadd".to_string(),
                kind: MirFunctionKind::Kernel,
                arg_count: 3,
                local_count: 17,
                locals: vec![MirLocal {
                    index: 1,
                    role: MirLocalRole::Arg,
                    ty: MirImportedType {
                        kind: MirType::Slice,
                        rust: "&[f32]".to_string(),
                        shape: MirTypeShape::Slice {
                            element: Box::new(MirTypeShape::F32),
                            mutable: false,
                        },
                    },
                }],
                blocks: vec![MirBlock {
                    index: 0,
                    statements: vec![MirStatement {
                        index: 0,
                        kind: MirStatementKind::Assign,
                        destination: Some(MirPlaceRef {
                            local: 3,
                            projection: Vec::new(),
                        }),
                        operands: vec![MirOperandRef::Place(MirPlaceRef {
                            local: 1,
                            projection: vec![
                                MirProjectionElem::Deref,
                                MirProjectionElem::Index { local: 2 },
                            ],
                        })],
                        rvalue: Some(MirRvalueKind::Use),
                        operation: Some("use".to_string()),
                        source: None,
                    }],
                    terminator: Some(MirTerminator {
                        kind: MirTerminatorKind::Return,
                        source: None,
                    }),
                }],
            }],
        };

        let records = module.dialect_records();

        assert_eq!(records[0].op, MirOp::Module);
        assert_eq!(records[1].op, MirOp::Func);
        assert_eq!(records[2].op, MirOp::Arg);
        assert_eq!(records[3].op, MirOp::Block);
        assert_eq!(records[4].op, MirOp::Assign);
        assert_eq!(records[5].op, MirOp::Load);
        assert_eq!(records[6].op, MirOp::Return);
        assert_eq!(record_usize(&records[4], "destination_local"), Some(3));
        assert_eq!(record_string(&records[4], "operation"), Some("use"));
        assert_eq!(
            record_string(&records[4], "operands"),
            Some("local1.deref.index_local2")
        );
        assert_eq!(record_usize(&records[5], "statement"), Some(0));
        assert_eq!(record_string(&records[5], "source"), Some("mir.assign"));
    }

    #[test]
    fn call_records_include_destination_and_operands() {
        let module = MirModule {
            functions: vec![MirFunction {
                export_name: "copy".to_string(),
                rust_path: "fe2o3_copy::fe2o3_kernel_copy".to_string(),
                kind: MirFunctionKind::Kernel,
                arg_count: 1,
                local_count: 3,
                locals: Vec::new(),
                blocks: vec![MirBlock {
                    index: 0,
                    statements: Vec::new(),
                    terminator: Some(MirTerminator {
                        kind: MirTerminatorKind::Call {
                            callee: Some(MirCallee::trusted_for_test(
                                TrustedDeviceItem::ThreadIndex1d,
                            )),
                            target: Some(1),
                            destination: Some(local_place(2)),
                            operands: vec![MirOperandRef::Place(local_place(1))],
                        },
                        source: None,
                    }),
                }],
            }],
        };

        let records = module.dialect_records();

        assert_eq!(records[3].op, MirOp::Call);
        assert_eq!(
            record_string(&records[3], "callee"),
            Some("fe2o3_device::thread::index_1d")
        );
        assert_eq!(record_usize(&records[3], "target"), Some(1));
        assert_eq!(record_usize(&records[3], "destination_local"), Some(2));
        assert_eq!(record_string(&records[3], "destination"), Some("local2"));
        assert_eq!(record_usize(&records[3], "operand_count"), Some(1));
        assert_eq!(record_string(&records[3], "operands"), Some("local1"));
        assert_eq!(record_string(&records[3], "trusted_callee"), None);
    }

    #[test]
    fn callee_identity_cannot_mismatch_trusted_authority() {
        let items = [
            TrustedDeviceItem::DisjointSlice,
            TrustedDeviceItem::ThreadIndex,
            TrustedDeviceItem::ThreadIndex1d,
            TrustedDeviceItem::ThreadIndexGet,
            TrustedDeviceItem::ThreadIndexOffset,
            TrustedDeviceItem::ThreadIndexOffsetSigned,
            TrustedDeviceItem::ThreadIndexStride,
            TrustedDeviceItem::ThreadIndexStrideOffset,
            TrustedDeviceItem::DisjointSliceGetMut,
            TrustedDeviceItem::DisjointSliceGetMutAt,
        ];

        for item in items {
            let trusted = MirCallee::trusted_for_test(item);
            assert_eq!(trusted.identity(), item.canonical_path());
            assert_eq!(trusted.trusted_item(), Some(item));

            let same_spelling = MirCallee::untrusted_for_test(item.canonical_path());
            assert_eq!(same_spelling.identity(), item.canonical_path());
            assert_eq!(same_spelling.trusted_item(), None);
        }
    }

    #[test]
    fn assignments_classify_lowering_ops() {
        let arithmetic = MirStatement {
            index: 0,
            kind: MirStatementKind::Assign,
            destination: Some(local_place(3)),
            operands: vec![
                MirOperandRef::Place(local_place(1)),
                MirOperandRef::Place(local_place(2)),
            ],
            rvalue: Some(MirRvalueKind::Binary(MirBinaryOp::MulWithOverflow)),
            operation: Some("mul_with_overflow".to_string()),
            source: None,
        };
        let load = MirStatement {
            index: 1,
            kind: MirStatementKind::Assign,
            destination: Some(local_place(4)),
            operands: vec![MirOperandRef::Place(MirPlaceRef {
                local: 1,
                projection: vec![
                    MirProjectionElem::Deref,
                    MirProjectionElem::Index { local: 2 },
                ],
            })],
            rvalue: Some(MirRvalueKind::Use),
            operation: Some("use".to_string()),
            source: None,
        };
        let store = MirStatement {
            index: 2,
            kind: MirStatementKind::Assign,
            destination: Some(MirPlaceRef {
                local: 5,
                projection: vec![MirProjectionElem::Deref],
            }),
            operands: vec![MirOperandRef::Place(local_place(4))],
            rvalue: Some(MirRvalueKind::Use),
            operation: Some("use".to_string()),
            source: None,
        };
        let compare = MirStatement {
            index: 3,
            kind: MirStatementKind::Assign,
            destination: Some(local_place(6)),
            operands: vec![
                MirOperandRef::Place(local_place(1)),
                MirOperandRef::Place(local_place(2)),
            ],
            rvalue: Some(MirRvalueKind::Binary(MirBinaryOp::Lt)),
            operation: Some("lt".to_string()),
            source: None,
        };

        assert_eq!(arithmetic.lowering_op(), Some(MirOp::Mul));
        assert_eq!(load.lowering_op(), Some(MirOp::Load));
        assert_eq!(store.lowering_op(), Some(MirOp::Store));
        assert_eq!(compare.lowering_op(), Some(MirOp::Lt));
    }

    fn simple_statement(index: usize, kind: MirStatementKind) -> MirStatement {
        MirStatement {
            index,
            kind,
            destination: None,
            operands: Vec::new(),
            rvalue: None,
            operation: None,
            source: None,
        }
    }

    fn local_place(local: usize) -> MirPlaceRef {
        MirPlaceRef {
            local,
            projection: Vec::new(),
        }
    }

    fn record_usize(record: &MirOpRecord, name: &'static str) -> Option<usize> {
        record.attrs.iter().find_map(|attr| {
            if attr.name == name
                && let dialect_mir::MirAttrValue::Usize(value) = &attr.value
            {
                return Some(*value);
            }
            None
        })
    }

    fn record_string<'a>(record: &'a MirOpRecord, name: &'static str) -> Option<&'a str> {
        record.attrs.iter().find_map(|attr| {
            if attr.name == name
                && let dialect_mir::MirAttrValue::String(value) = &attr.value
            {
                return Some(value.as_str());
            }
            None
        })
    }
}
