use crate::collector::CollectionResult;
use dialect_mir::{MirAttr, MirOp, MirOpRecord, MirType};
use rustc_hir::def_id::LOCAL_CRATE;
use rustc_middle::mir::{
    BasicBlock, BinOp, Body, ConstOperand, Local, Operand, Place, ProjectionElem, Rvalue,
    StatementKind, TerminatorKind, UnOp,
};
use rustc_middle::ty::{FloatTy, IntTy, Ty, TyCtxt, TyKind, TypingEnv, UintTy};
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
    pub operation: Option<String>,
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
    pub projection: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirOperandRef {
    Place(MirPlaceRef),
    Constant { ty: MirImportedType, value: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirTerminator {
    pub kind: MirTerminatorKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirTerminatorKind {
    Return,
    Unreachable,
    Goto {
        target: usize,
    },
    SwitchInt {
        targets: usize,
    },
    Call {
        callee: Option<String>,
        target: Option<usize>,
        destination: Option<MirPlaceRef>,
        operands: Vec<MirOperandRef>,
    },
    Assert {
        target: usize,
    },
    Drop {
        target: usize,
    },
    Other,
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

        match self.operation.as_deref()? {
            "add" | "add_unchecked" | "add_with_overflow" => Some(MirOp::Add),
            "sub" | "sub_unchecked" | "sub_with_overflow" => Some(MirOp::Sub),
            "mul" | "mul_unchecked" | "mul_with_overflow" => Some(MirOp::Mul),
            "div" => Some(MirOp::Div),
            "eq" => Some(MirOp::Eq),
            "lt" => Some(MirOp::Lt),
            "le" => Some(MirOp::Le),
            "ne" => Some(MirOp::Ne),
            "ge" => Some(MirOp::Ge),
            "gt" => Some(MirOp::Gt),
            "cmp" => Some(MirOp::Cmp),
            "cast" => Some(MirOp::Cast),
            "offset" => Some(MirOp::Gep),
            "ptr_metadata" => Some(MirOp::SliceLen),
            "use" if self.operands.iter().any(MirOperandRef::is_memory_place) => Some(MirOp::Load),
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
            label.push_str(projection);
        }
        label
    }

    fn is_memory_projection(&self) -> bool {
        self.projection.iter().any(|projection| {
            projection == "deref"
                || projection.starts_with("index_")
                || projection.starts_with("constant_index")
        })
    }
}

impl MirOperandRef {
    fn label(&self) -> String {
        match self {
            Self::Place(place) => place.label(),
            Self::Constant { ty, value } => format!("const:{}={value}", ty.kind.name()),
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
            Self::Goto { target } | Self::Assert { target } | Self::Drop { target } => {
                record.attrs.push(MirAttr::usize("target", *target));
            }
            Self::SwitchInt { targets } => {
                record.attrs.push(MirAttr::usize("targets", *targets));
            }
            Self::Call {
                callee,
                target,
                destination,
                operands,
            } => {
                if let Some(callee) = callee {
                    record.attrs.push(MirAttr::string("callee", callee));
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
            Self::SwitchInt { targets } => {
                format!("{} ({targets} target(s))", MirOp::Switch.name())
            }
            Self::Call { callee, target, .. } => {
                let callee = callee.as_deref().unwrap_or("<dynamic>");
                match target {
                    Some(target) => format!("{} {callee} -> bb{target}", MirOp::Call.name()),
                    None => format!("{} {callee} -> return", MirOp::Call.name()),
                }
            }
            Self::Assert { target } => format!("{} -> bb{target}", MirOp::Assert.name()),
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
                    import_statement(tcx, statement_index, &statement.kind)
                })
                .collect(),
            terminator: block.terminator.as_ref().map(|terminator| MirTerminator {
                kind: terminator_kind(tcx, &terminator.kind),
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
        TyKind::Adt(adt, _) if tcx.def_path_str(adt.did()).ends_with("DisjointSlice") => {
            MirType::DisjointSlice
        }
        TyKind::Tuple(elements) if elements.is_empty() => MirType::Unit,
        _ => MirType::Unknown,
    };

    MirImportedType {
        kind,
        rust: ty.to_string(),
    }
}

fn import_statement<'tcx>(
    tcx: TyCtxt<'tcx>,
    index: usize,
    kind: &StatementKind<'tcx>,
) -> MirStatement {
    MirStatement {
        index,
        kind: statement_kind(kind),
        destination: statement_destination(kind),
        operands: statement_operands(tcx, kind),
        operation: statement_operation(kind),
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

fn import_operand<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>) -> MirOperandRef {
    if let Some(place) = operand.place() {
        return MirOperandRef::Place(import_place(place));
    }

    let Operand::Constant(constant) = operand else {
        return MirOperandRef::Constant {
            ty: MirImportedType {
                kind: MirType::Unknown,
                rust: "<unknown>".to_string(),
            },
            value: "<unknown>".to_string(),
        };
    };

    MirOperandRef::Constant {
        ty: import_type(tcx, constant.const_.ty()),
        value: constant_value_label(tcx, constant),
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
        projection: place.projection.iter().map(projection_elem_name).collect(),
    }
}

fn projection_elem_name(element: ProjectionElem<Local, Ty<'_>>) -> String {
    match element {
        ProjectionElem::Deref => "deref".to_string(),
        ProjectionElem::Field(field, _) => format!("field{}", field.index()),
        ProjectionElem::Index(local) => format!("index_local{}", local.as_usize()),
        ProjectionElem::ConstantIndex {
            offset,
            min_length,
            from_end,
        } => format!("constant_index{offset}_min{min_length}_from_end{from_end}"),
        ProjectionElem::Subslice { from, to, from_end } => {
            format!("subslice{from}_{to}_from_end{from_end}")
        }
        ProjectionElem::Downcast(_, variant) => format!("downcast{}", variant.index()),
        ProjectionElem::OpaqueCast(_) => "opaque_cast".to_string(),
        _ => "projection".to_string(),
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
        TerminatorKind::SwitchInt { targets, .. } => MirTerminatorKind::SwitchInt {
            targets: targets.all_targets().len(),
        },
        TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            ..
        } => MirTerminatorKind::Call {
            callee: call_name(tcx, func),
            target: target.map(BasicBlock::as_usize),
            destination: Some(import_place(*destination)),
            operands: args
                .iter()
                .map(|arg| import_operand(tcx, &arg.node))
                .collect(),
        },
        TerminatorKind::Assert { target, .. } => MirTerminatorKind::Assert {
            target: target.as_usize(),
        },
        TerminatorKind::Drop { target, .. } => MirTerminatorKind::Drop {
            target: target.as_usize(),
        },
        _ => MirTerminatorKind::Other,
    }
}

fn call_name<'tcx>(tcx: TyCtxt<'tcx>, func: &Operand<'tcx>) -> Option<String> {
    let Operand::Constant(constant) = func else {
        return None;
    };
    let TyKind::FnDef(def_id, _) = constant.const_.ty().kind() else {
        return None;
    };
    Some(tcx.def_path_str(*def_id))
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
                        },
                    },
                    MirLocal {
                        index: 1,
                        role: MirLocalRole::Arg,
                        ty: MirImportedType {
                            kind: MirType::Slice,
                            rust: "&[f32]".to_string(),
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
                            projection: vec!["deref".to_string(), "index_local2".to_string()],
                        })],
                        operation: Some("use".to_string()),
                    }],
                    terminator: Some(MirTerminator {
                        kind: MirTerminatorKind::Return,
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
                            callee: Some("fe2o3_device::thread::index_1d".to_string()),
                            target: Some(1),
                            destination: Some(local_place(2)),
                            operands: vec![MirOperandRef::Place(local_place(1))],
                        },
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
            operation: Some("mul_with_overflow".to_string()),
        };
        let load = MirStatement {
            index: 1,
            kind: MirStatementKind::Assign,
            destination: Some(local_place(4)),
            operands: vec![MirOperandRef::Place(MirPlaceRef {
                local: 1,
                projection: vec!["deref".to_string(), "index_local2".to_string()],
            })],
            operation: Some("use".to_string()),
        };
        let store = MirStatement {
            index: 2,
            kind: MirStatementKind::Assign,
            destination: Some(MirPlaceRef {
                local: 5,
                projection: vec!["deref".to_string()],
            }),
            operands: vec![MirOperandRef::Place(local_place(4))],
            operation: Some("use".to_string()),
        };
        let compare = MirStatement {
            index: 3,
            kind: MirStatementKind::Assign,
            destination: Some(local_place(6)),
            operands: vec![
                MirOperandRef::Place(local_place(1)),
                MirOperandRef::Place(local_place(2)),
            ],
            operation: Some("lt".to_string()),
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
            operation: None,
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
            if attr.name == name {
                if let dialect_mir::MirAttrValue::Usize(value) = &attr.value {
                    return Some(*value);
                }
            }
            None
        })
    }

    fn record_string<'a>(record: &'a MirOpRecord, name: &'static str) -> Option<&'a str> {
        record.attrs.iter().find_map(|attr| {
            if attr.name == name {
                if let dialect_mir::MirAttrValue::String(value) = &attr.value {
                    return Some(value.as_str());
                }
            }
            None
        })
    }
}
