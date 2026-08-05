//! Deterministic, verification-only lowering from imported MIR to kernel IR.
//!
//! This core layer establishes diagnostics, scalar signatures, constants, and
//! module/function structure. Later layers add general CFG and memory lowering.

use crate::mir_import::{
    MirBlock, MirConstant, MirFunction, MirFunctionKind, MirLocalRole, MirModule, MirOperandRef,
    MirRvalueKind, MirSourceLocation, MirStatement, MirStatementKind, MirTerminator,
    MirTerminatorKind, MirTypeShape,
};
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, Constant, Function, Kernel, LaunchDomain, LaunchExtent, Module, Operation,
    OperationKind, ScalarType, Signature, Terminator, Type, ValueDef, ValueId, verify_module,
};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

const MODULE_ID: &str = "rustc_codegen_fe2o3::mir_analysis";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TranslationDiagnosticCode {
    MalformedMir,
    UnsupportedType,
    UnsupportedStatement,
    UnsupportedRvalue,
    UnsupportedProjection,
    UnsupportedCall,
    VerificationFailed,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TranslationLocation {
    pub function: Option<String>,
    pub block: Option<usize>,
    pub statement: Option<usize>,
    pub terminator: bool,
    pub operation: Option<usize>,
    pub source: Option<Box<MirSourceLocation>>,
}

impl TranslationLocation {
    fn function(function: &MirFunction) -> Self {
        Self {
            function: Some(function.rust_path.clone()),
            block: None,
            statement: None,
            terminator: false,
            operation: None,
            source: None,
        }
    }

    fn block(function: &MirFunction, block: &MirBlock) -> Self {
        Self {
            block: Some(block.index),
            ..Self::function(function)
        }
    }

    fn statement(function: &MirFunction, block: usize, statement: &MirStatement) -> Self {
        Self {
            function: Some(function.rust_path.clone()),
            block: Some(block),
            statement: Some(statement.index),
            terminator: false,
            operation: None,
            source: statement.source.clone().map(Box::new),
        }
    }

    fn terminator(function: &MirFunction, block: usize, terminator: &MirTerminator) -> Self {
        Self {
            function: Some(function.rust_path.clone()),
            block: Some(block),
            statement: None,
            terminator: true,
            operation: None,
            source: terminator.source.clone().map(Box::new),
        }
    }
}

impl fmt::Display for TranslationLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(source) = &self.source {
            write!(
                formatter,
                "{}:{}:{}",
                source.file, source.line, source.column
            )?;
        } else {
            formatter.write_str("<unknown source>")?;
        }
        if let Some(function) = &self.function {
            write!(formatter, " in {function}")?;
        }
        if let Some(block) = self.block {
            write!(formatter, " bb{block}")?;
        }
        if let Some(statement) = self.statement {
            write!(formatter, " stmt{statement}")?;
        } else if self.terminator {
            formatter.write_str(" terminator")?;
        }
        if let Some(operation) = self.operation {
            write!(formatter, " op{operation}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TranslationDiagnostic {
    pub location: TranslationLocation,
    pub code: TranslationDiagnosticCode,
    pub message: String,
}

impl fmt::Display for TranslationDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {:?}: {}",
            self.location, self.code, self.message
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TranslationErrors {
    diagnostics: Vec<TranslationDiagnostic>,
}

impl TranslationErrors {
    #[cfg(test)]
    fn diagnostics(&self) -> &[TranslationDiagnostic] {
        &self.diagnostics
    }

    #[cfg(test)]
    fn contains(&self, code: TranslationDiagnosticCode) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code)
    }
}

impl fmt::Display for TranslationErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "MIR to kernel IR translation failed with {} diagnostic(s)",
            self.diagnostics.len()
        )?;
        for diagnostic in &self.diagnostics {
            writeln!(formatter, "  {diagnostic}")?;
        }
        Ok(())
    }
}

impl Error for TranslationErrors {}

pub fn translate_and_verify(mir: &MirModule) -> Result<Module, TranslationErrors> {
    let mut kernels = mir
        .functions
        .iter()
        .filter(|function| function.kind == MirFunctionKind::Kernel)
        .collect::<Vec<_>>();
    kernels.sort_by(|lhs, rhs| {
        (&lhs.export_name, &lhs.rust_path).cmp(&(&rhs.export_name, &rhs.rust_path))
    });

    let mut diagnostics = Vec::new();
    let mut functions = Vec::new();
    let mut entries = Vec::new();
    let mut kernel_ids = BTreeSet::new();
    for function in kernels {
        if !kernel_ids.insert(function.export_name.as_str()) {
            diagnostics.push(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                TranslationLocation::function(function),
                format!("duplicate kernel export name `{}`", function.export_name),
            ));
            continue;
        }
        match FunctionLowerer::new(function).lower() {
            Ok(definition) => {
                entries.push((function.export_name.clone(), definition.id.clone()));
                functions.push(definition);
            }
            Err(error) => diagnostics.push(error),
        }
    }
    if !diagnostics.is_empty() {
        return Err(errors(diagnostics));
    }

    let mut module = Module::new(MODULE_ID);
    module.functions = functions;
    module.kernels = entries
        .into_iter()
        .map(|(kernel, entry)| {
            Kernel::new(
                kernel,
                entry,
                LaunchDomain::D1 {
                    x: LaunchExtent::Dynamic,
                },
            )
        })
        .collect();
    verify_module(&module).map_err(|verification_errors| {
        errors(
            verification_errors
                .diagnostics()
                .iter()
                .map(|verification| TranslationDiagnostic {
                    location: TranslationLocation {
                        function: verification
                            .location
                            .function
                            .as_ref()
                            .map(|function| function.as_str().to_string()),
                        block: verification.location.block.map(|block| block.0 as usize),
                        statement: None,
                        terminator: false,
                        operation: verification.location.operation,
                        source: None,
                    },
                    code: TranslationDiagnosticCode::VerificationFailed,
                    message: format!("{:?}: {}", verification.code, verification.message),
                })
                .collect(),
        )
    })?;
    Ok(module)
}

fn errors(mut diagnostics: Vec<TranslationDiagnostic>) -> TranslationErrors {
    diagnostics.sort();
    TranslationErrors { diagnostics }
}

fn diagnostic(
    code: TranslationDiagnosticCode,
    location: TranslationLocation,
    message: impl Into<String>,
) -> TranslationDiagnostic {
    TranslationDiagnostic {
        location,
        code,
        message: message.into(),
    }
}

struct FunctionLowerer<'function> {
    function: &'function MirFunction,
    locals: BTreeMap<usize, ValueId>,
    next_value: u32,
}

impl<'function> FunctionLowerer<'function> {
    fn new(function: &'function MirFunction) -> Self {
        Self {
            function,
            locals: BTreeMap::new(),
            next_value: 0,
        }
    }

    fn lower(mut self) -> Result<Function, TranslationDiagnostic> {
        let mut args = self
            .function
            .locals
            .iter()
            .filter(|local| local.role == MirLocalRole::Arg)
            .collect::<Vec<_>>();
        args.sort_by_key(|local| local.index);
        if args.len() != self.function.arg_count {
            return Err(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                TranslationLocation::function(self.function),
                format!(
                    "function declares {} arguments but imports {} argument locals",
                    self.function.arg_count,
                    args.len()
                ),
            ));
        }

        let mut parameter_types = Vec::with_capacity(args.len());
        let mut parameter_values = Vec::with_capacity(args.len());
        for arg in args {
            let location = TranslationLocation::function(self.function);
            let ty = lower_scalar_type(&arg.ty.shape).ok_or_else(|| {
                diagnostic(
                    TranslationDiagnosticCode::UnsupportedType,
                    location.clone(),
                    format!(
                        "argument local{} has unsupported type `{}`",
                        arg.index, arg.ty.rust
                    ),
                )
            })?;
            let id = self.fresh_id(&location)?;
            self.bind_local(arg.index, id, location)?;
            parameter_types.push(ty);
            parameter_values.push(id);
        }
        if let Some(return_local) = self.function.locals.iter().find(|local| local.index == 0)
            && return_local.ty.shape != MirTypeShape::Unit
        {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                TranslationLocation::function(self.function),
                format!(
                    "kernel return type `{}` is not supported",
                    return_local.ty.rust
                ),
            ));
        }

        let mut sources = self.function.blocks.iter().collect::<Vec<_>>();
        sources.sort_by_key(|block| block.index);
        if sources.first().map(|block| block.index) != Some(0) {
            return Err(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                TranslationLocation::function(self.function),
                "kernel must contain entry block bb0",
            ));
        }
        let mut seen = BTreeSet::new();
        let mut blocks = Vec::with_capacity(sources.len());
        for source in sources {
            if !seen.insert(source.index) {
                return Err(diagnostic(
                    TranslationDiagnosticCode::MalformedMir,
                    TranslationLocation::block(self.function, source),
                    format!("basic block bb{} is imported more than once", source.index),
                ));
            }
            blocks.push(self.lower_block(source)?);
        }
        Ok(Function::definition(
            self.function.rust_path.clone(),
            Signature::new(parameter_types, Vec::new()),
            parameter_values,
            blocks,
        ))
    }

    fn lower_block(&mut self, source: &MirBlock) -> Result<BasicBlock, TranslationDiagnostic> {
        let mut block = BasicBlock::new(self.block_id(
            source.index,
            TranslationLocation::block(self.function, source),
        )?);
        for statement in &source.statements {
            self.lower_statement(source.index, statement, &mut block)?;
        }
        let terminator = source.terminator.as_ref().ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                TranslationLocation::block(self.function, source),
                "basic block has no terminator",
            )
        })?;
        let location = TranslationLocation::terminator(self.function, source.index, terminator);
        block.terminator = Some(match terminator.kind {
            MirTerminatorKind::Return => Terminator::Return { values: Vec::new() },
            MirTerminatorKind::Unreachable => Terminator::Unreachable,
            _ => {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedStatement,
                    location,
                    format!("unsupported MIR terminator: {:?}", terminator.kind),
                ));
            }
        });
        Ok(block)
    }

    fn lower_statement(
        &mut self,
        block_index: usize,
        statement: &MirStatement,
        block: &mut BasicBlock,
    ) -> Result<(), TranslationDiagnostic> {
        let location = TranslationLocation::statement(self.function, block_index, statement);
        match statement.kind {
            MirStatementKind::StorageLive
            | MirStatementKind::StorageDead
            | MirStatementKind::Retag
            | MirStatementKind::Coverage
            | MirStatementKind::Nop => return Ok(()),
            MirStatementKind::Assign => {}
            _ => {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedStatement,
                    location,
                    format!("unsupported MIR statement kind: {:?}", statement.kind),
                ));
            }
        }
        let destination = statement.destination.as_ref().ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location.clone(),
                "assignment has no destination",
            )
        })?;
        if !destination.projection.is_empty() {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedProjection,
                location,
                "projected assignment destinations are not supported",
            ));
        }
        let [MirOperandRef::Constant { literal, .. }] = statement.operands.as_slice() else {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedRvalue,
                location,
                "core lowering only supports constant assignments",
            ));
        };
        if statement.rvalue != Some(MirRvalueKind::Use) {
            return Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedRvalue,
                location,
                format!("unsupported structured MIR rvalue: {:?}", statement.rvalue),
            ));
        }
        let constant = lower_constant(literal).ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::UnsupportedType,
                location.clone(),
                format!("unsupported or unevaluated constant: {literal:?}"),
            )
        })?;
        let id = self.fresh_id(&location)?;
        block.operations.push(Operation::effect_free(
            ValueDef::new(id, constant.ty()),
            OperationKind::Constant(constant),
        ));
        self.bind_local(destination.local, id, location)
    }

    fn fresh_id(
        &mut self,
        location: &TranslationLocation,
    ) -> Result<ValueId, TranslationDiagnostic> {
        let id = ValueId(self.next_value);
        self.next_value = self.next_value.checked_add(1).ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location.clone(),
                "function has too many SSA values",
            )
        })?;
        Ok(id)
    }

    fn bind_local(
        &mut self,
        local: usize,
        value: ValueId,
        location: TranslationLocation,
    ) -> Result<(), TranslationDiagnostic> {
        if self.locals.insert(local, value).is_some() {
            return Err(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location,
                format!("local{local} is assigned more than once in the supported SSA subset"),
            ));
        }
        Ok(())
    }

    fn block_id(
        &self,
        index: usize,
        location: TranslationLocation,
    ) -> Result<BlockId, TranslationDiagnostic> {
        u32::try_from(index).map(BlockId).map_err(|_| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location,
                format!("basic block index {index} exceeds kernel IR limits"),
            )
        })
    }
}

fn lower_scalar_type(shape: &MirTypeShape) -> Option<Type> {
    match shape {
        MirTypeShape::Bool => Some(Type::BOOL),
        MirTypeShape::I32 => Some(Type::Scalar(ScalarType::I32)),
        MirTypeShape::I64 | MirTypeShape::ISize => Some(Type::Scalar(ScalarType::I64)),
        MirTypeShape::USize => Some(Type::INDEX),
        MirTypeShape::F32 => Some(Type::F32),
        MirTypeShape::F64 => Some(Type::F64),
        _ => None,
    }
}

fn lower_constant(constant: &MirConstant) -> Option<Constant> {
    match constant {
        MirConstant::Bool(value) => Some(Constant::Bool(*value)),
        MirConstant::I32(value) => Some(Constant::I32(*value)),
        MirConstant::I64(value) | MirConstant::ISize(value) => Some(Constant::I64(*value)),
        MirConstant::USize(value) => Some(Constant::Index(*value)),
        MirConstant::F32Bits(value) => Some(Constant::F32Bits(*value)),
        MirConstant::F64Bits(value) => Some(Constant::F64Bits(*value)),
        MirConstant::Unevaluated => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mir_import::{MirImportedType, MirLocal, MirPlaceRef};
    use dialect_mir::MirType;

    #[test]
    fn empty_kernels_are_sorted_and_verify() {
        let module = translate_and_verify(&MirModule {
            functions: vec![kernel("zeta", Vec::new()), kernel("alpha", Vec::new())],
        })
        .expect("empty kernels");

        assert_eq!(module.kernels[0].id.as_str(), "alpha");
        assert_eq!(module.kernels[1].id.as_str(), "zeta");
        verify_module(&module).expect("core output should verify");
    }

    #[test]
    fn constant_kernel_has_typed_operation() {
        let statement = MirStatement {
            index: 0,
            kind: MirStatementKind::Assign,
            destination: Some(MirPlaceRef {
                local: 1,
                projection: Vec::new(),
            }),
            operands: vec![MirOperandRef::Constant {
                literal: MirConstant::I32(7),
                ty: MirImportedType {
                    kind: MirType::I32,
                    rust: "i32".to_string(),
                    shape: MirTypeShape::I32,
                },
                value: "7_i32".to_string(),
            }],
            rvalue: Some(MirRvalueKind::Use),
            operation: Some("const 7_i32".to_string()),
            source: None,
        };
        let module = translate_and_verify(&MirModule {
            functions: vec![kernel("constant", vec![statement])],
        })
        .expect("constant kernel");
        let operation = &module.functions[0].body.as_ref().expect("body").blocks[0].operations[0];

        assert!(matches!(
            operation.kind,
            OperationKind::Constant(Constant::I32(7))
        ));
        assert_eq!(operation.results[0].ty, Type::Scalar(ScalarType::I32));
    }

    fn kernel(name: &str, statements: Vec<MirStatement>) -> MirFunction {
        MirFunction {
            export_name: name.to_string(),
            rust_path: format!("tests::{name}"),
            kind: MirFunctionKind::Kernel,
            arg_count: 0,
            local_count: 2,
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
                    role: MirLocalRole::Temp,
                    ty: MirImportedType {
                        kind: MirType::I32,
                        rust: "i32".to_string(),
                        shape: MirTypeShape::I32,
                    },
                },
            ],
            blocks: vec![MirBlock {
                index: 0,
                statements,
                terminator: Some(MirTerminator {
                    kind: MirTerminatorKind::Return,
                    source: None,
                }),
            }],
        }
    }
}
