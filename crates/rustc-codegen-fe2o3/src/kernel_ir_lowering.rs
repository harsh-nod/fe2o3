//! Deterministic, verification-only lowering from imported MIR to kernel IR.
//!
//! This layer establishes diagnostics, scalar signatures, SSA values, and
//! basic control flow. Slice, memory, and device-helper semantics are added by
//! the vecadd vertical slice; unsupported constructs fail explicitly here.

use crate::mir_import::{
    MirBinaryOp, MirBlock, MirConstant, MirFunction, MirFunctionKind, MirLocalRole, MirModule,
    MirOperandRef, MirRvalueKind, MirSourceLocation, MirStatement, MirStatementKind, MirTerminator,
    MirTerminatorKind, MirTypeShape,
};
use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, BlockId, ComparePredicate, Constant, Function, Kernel, LaunchDomain,
    LaunchExtent, Module, Operation, OperationKind, ScalarType, Signature, SwitchCase, Terminator,
    Type, ValueDef, ValueId, verify_module,
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

    if let Err(verification_errors) = verify_module(&module) {
        let diagnostics = verification_errors
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
            .collect();
        return Err(errors(diagnostics));
    }
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
    value_types: BTreeMap<ValueId, Type>,
    next_value: u32,
    trap_block: Option<BlockId>,
}

impl<'function> FunctionLowerer<'function> {
    fn new(function: &'function MirFunction) -> Self {
        Self {
            function,
            locals: BTreeMap::new(),
            value_types: BTreeMap::new(),
            next_value: 0,
            trap_block: None,
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
            self.value_types.insert(id, ty.clone());
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

        let mut source_blocks = self.function.blocks.iter().collect::<Vec<_>>();
        source_blocks.sort_by_key(|block| block.index);
        if source_blocks.first().map(|block| block.index) != Some(0) {
            return Err(diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                TranslationLocation::function(self.function),
                "kernel must contain entry block bb0",
            ));
        }
        let mut block_indices = BTreeSet::new();
        for block in &source_blocks {
            if !block_indices.insert(block.index) {
                return Err(diagnostic(
                    TranslationDiagnosticCode::MalformedMir,
                    TranslationLocation::block(self.function, block),
                    format!("basic block bb{} is imported more than once", block.index),
                ));
            }
            self.block_id(
                block.index,
                TranslationLocation::block(self.function, block),
            )?;
        }
        if source_blocks.iter().any(|block| {
            matches!(
                block.terminator.as_ref().map(|terminator| &terminator.kind),
                Some(MirTerminatorKind::Assert { .. })
            )
        }) {
            let next = source_blocks
                .last()
                .expect("entry block checked")
                .index
                .checked_add(1)
                .ok_or_else(|| {
                    diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        TranslationLocation::function(self.function),
                        "cannot allocate assertion failure block",
                    )
                })?;
            self.trap_block =
                Some(self.block_id(next, TranslationLocation::function(self.function))?);
        }

        let mut blocks =
            Vec::with_capacity(source_blocks.len() + usize::from(self.trap_block.is_some()));
        for source in source_blocks {
            blocks.push(self.lower_block(source)?);
        }
        if let Some(trap) = self.trap_block {
            let mut block = BasicBlock::new(trap);
            block.terminator = Some(Terminator::Unreachable);
            blocks.push(block);
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
        block.terminator = Some(self.lower_terminator(source.index, terminator, &mut block)?);
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
        let rvalue = statement.rvalue.ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location.clone(),
                "assignment has no structured rvalue",
            )
        })?;

        let value = match rvalue {
            MirRvalueKind::Use => {
                let [operand] = statement.operands.as_slice() else {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location,
                        "use assignment must have one operand",
                    ));
                };
                self.lower_operand(operand, block, &location)?
            }
            MirRvalueKind::Binary(MirBinaryOp::Add) => {
                let [lhs, rhs] = statement.operands.as_slice() else {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location,
                        "add must have two operands",
                    ));
                };
                let lhs = self.lower_operand(lhs, block, &location)?;
                let rhs = self.lower_operand(rhs, block, &location)?;
                let ty = self.value_type(lhs, &location)?.clone();
                self.emit_result(
                    block,
                    ty,
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs,
                        rhs,
                    },
                    &location,
                )?
            }
            MirRvalueKind::Binary(MirBinaryOp::Lt) => {
                let [lhs, rhs] = statement.operands.as_slice() else {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location,
                        "less-than comparison must have two operands",
                    ));
                };
                let lhs = self.lower_operand(lhs, block, &location)?;
                let rhs = self.lower_operand(rhs, block, &location)?;
                self.emit_result(
                    block,
                    Type::BOOL,
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs,
                        rhs,
                    },
                    &location,
                )?
            }
            unsupported => {
                return Err(diagnostic(
                    TranslationDiagnosticCode::UnsupportedRvalue,
                    location,
                    format!("unsupported structured MIR rvalue: {unsupported:?}"),
                ));
            }
        };
        self.bind_local(destination.local, value, location)
    }

    fn lower_terminator(
        &mut self,
        block_index: usize,
        terminator: &MirTerminator,
        block: &mut BasicBlock,
    ) -> Result<Terminator, TranslationDiagnostic> {
        let location = TranslationLocation::terminator(self.function, block_index, terminator);
        match &terminator.kind {
            MirTerminatorKind::Return => Ok(Terminator::Return { values: Vec::new() }),
            MirTerminatorKind::Unreachable => Ok(Terminator::Unreachable),
            MirTerminatorKind::Goto { target } => Ok(Terminator::Branch {
                target: self.block_id(*target, location)?,
                arguments: Vec::new(),
            }),
            MirTerminatorKind::SwitchInt {
                discriminant,
                targets,
                otherwise,
            } => {
                let selector = self.lower_operand(discriminant, block, &location)?;
                let mut cases = targets
                    .iter()
                    .map(|target| {
                        Ok(SwitchCase {
                            value: u64::try_from(target.value).map_err(|_| {
                                diagnostic(
                                    TranslationDiagnosticCode::UnsupportedType,
                                    location.clone(),
                                    format!(
                                        "switch value {} does not fit kernel IR's u64 cases",
                                        target.value
                                    ),
                                )
                            })?,
                            target: self.block_id(target.target, location.clone())?,
                            arguments: Vec::new(),
                        })
                    })
                    .collect::<Result<Vec<_>, TranslationDiagnostic>>()?;
                cases.sort_by_key(|case| (case.value, case.target));
                Ok(Terminator::Switch {
                    selector,
                    cases,
                    default_target: self.block_id(*otherwise, location)?,
                    default_arguments: Vec::new(),
                })
            }
            MirTerminatorKind::Assert {
                condition,
                expected,
                target,
            } => {
                let condition = self.lower_operand(condition, block, &location)?;
                let success = self.block_id(*target, location.clone())?;
                let failure = self.trap_block.ok_or_else(|| {
                    diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location.clone(),
                        "assertion failure block was not allocated",
                    )
                })?;
                let (then_target, else_target) = if *expected {
                    (success, failure)
                } else {
                    (failure, success)
                };
                Ok(Terminator::ConditionalBranch {
                    condition,
                    then_target,
                    then_arguments: Vec::new(),
                    else_target,
                    else_arguments: Vec::new(),
                })
            }
            MirTerminatorKind::Call { .. } => Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedCall,
                location,
                "MIR calls are not supported by the scalar framework",
            )),
            unsupported => Err(diagnostic(
                TranslationDiagnosticCode::UnsupportedStatement,
                location,
                format!("unsupported MIR terminator: {unsupported:?}"),
            )),
        }
    }

    fn lower_operand(
        &mut self,
        operand: &MirOperandRef,
        block: &mut BasicBlock,
        location: &TranslationLocation,
    ) -> Result<ValueId, TranslationDiagnostic> {
        match operand {
            MirOperandRef::Place(place) => {
                if !place.projection.is_empty() {
                    return Err(diagnostic(
                        TranslationDiagnosticCode::UnsupportedProjection,
                        location.clone(),
                        format!("unsupported place projection: {:?}", place.projection),
                    ));
                }
                self.locals.get(&place.local).copied().ok_or_else(|| {
                    diagnostic(
                        TranslationDiagnosticCode::MalformedMir,
                        location.clone(),
                        format!("local{} is used before it is defined", place.local),
                    )
                })
            }
            MirOperandRef::Constant { literal, .. } => {
                let constant = lower_constant(literal).ok_or_else(|| {
                    diagnostic(
                        TranslationDiagnosticCode::UnsupportedType,
                        location.clone(),
                        format!("unsupported or unevaluated constant: {literal:?}"),
                    )
                })?;
                self.emit_result(
                    block,
                    constant.ty(),
                    OperationKind::Constant(constant),
                    location,
                )
            }
        }
    }

    fn emit_result(
        &mut self,
        block: &mut BasicBlock,
        ty: Type,
        kind: OperationKind,
        location: &TranslationLocation,
    ) -> Result<ValueId, TranslationDiagnostic> {
        let id = self.fresh_id(location)?;
        self.value_types.insert(id, ty.clone());
        block
            .operations
            .push(Operation::effect_free(ValueDef::new(id, ty), kind));
        Ok(id)
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

    fn value_type(
        &self,
        value: ValueId,
        location: &TranslationLocation,
    ) -> Result<&Type, TranslationDiagnostic> {
        self.value_types.get(&value).ok_or_else(|| {
            diagnostic(
                TranslationDiagnosticCode::MalformedMir,
                location.clone(),
                format!("SSA value {value} has no imported type"),
            )
        })
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
    use crate::mir_import::{MirImportedType, MirLocal, MirPlaceRef, MirProjectionElem};
    use dialect_mir::MirType;

    #[test]
    fn empty_kernels_are_sorted_and_verify() {
        let mut alpha = scalar_fixture().functions.remove(0);
        alpha.export_name = "alpha".to_string();
        alpha.rust_path = "tests::alpha".to_string();
        alpha.blocks.truncate(1);
        alpha.blocks[0].statements.clear();
        alpha.blocks[0].terminator = Some(terminator(MirTerminatorKind::Return));
        let mut zeta = alpha.clone();
        zeta.export_name = "zeta".to_string();
        zeta.rust_path = "tests::zeta".to_string();

        let module = translate_and_verify(&MirModule {
            functions: vec![zeta, alpha],
        })
        .expect("empty kernels");

        assert_eq!(module.kernels[0].id.as_str(), "alpha");
        assert_eq!(module.kernels[1].id.as_str(), "zeta");
    }

    #[test]
    fn constant_kernel_has_typed_operation() {
        let mut fixture = scalar_fixture();
        fixture.functions[0].blocks.truncate(1);
        fixture.functions[0].blocks[0].statements = vec![assign(
            0,
            3,
            vec![MirOperandRef::Constant {
                ty: MirImportedType {
                    kind: MirType::I32,
                    rust: "i32".to_string(),
                    shape: MirTypeShape::I32,
                },
                literal: MirConstant::I32(7),
                value: "7_i32".to_string(),
            }],
            MirRvalueKind::Use,
        )];
        fixture.functions[0].blocks[0].terminator = Some(terminator(MirTerminatorKind::Return));

        let module = translate_and_verify(&fixture).expect("constant kernel");
        assert!(matches!(
            module.functions[0].body.as_ref().expect("body").blocks[0].operations[0].kind,
            OperationKind::Constant(Constant::I32(7))
        ));
    }

    #[test]
    fn scalar_framework_builds_and_verifies_typed_control_flow() {
        let module = translate_and_verify(&scalar_fixture()).expect("scalar fixture");
        verify_module(&module).expect("framework output should verify");

        assert_eq!(module.kernels.len(), 1);
        let body = module.functions[0].body.as_ref().expect("body");
        assert_eq!(body.blocks.len(), 3, "two MIR blocks plus assert trap");
        assert!(body.blocks[0].operations.iter().any(|operation| matches!(
            operation.kind,
            OperationKind::Binary {
                op: BinaryOp::Add,
                ..
            }
        )));
        assert!(matches!(
            body.blocks[0].terminator,
            Some(Terminator::ConditionalBranch { .. })
        ));
    }

    #[test]
    fn scalar_translation_is_deterministic() {
        let fixture = scalar_fixture();
        assert_eq!(
            translate_and_verify(&fixture).expect("first"),
            translate_and_verify(&fixture).expect("second")
        );
    }

    #[test]
    fn malformed_block_fails_explicitly() {
        let mut fixture = scalar_fixture();
        fixture.functions[0].blocks[1].terminator = None;

        let errors = translate_and_verify(&fixture).expect_err("missing terminator");
        assert!(errors.contains(TranslationDiagnosticCode::MalformedMir));
        assert_eq!(errors.diagnostics()[0].location.block, Some(1));
    }

    #[test]
    fn projected_scalar_operand_reports_source_location() {
        let mut fixture = scalar_fixture();
        fixture.functions[0].blocks[0].statements[0].operands[0] =
            MirOperandRef::Place(MirPlaceRef {
                local: 1,
                projection: vec![MirProjectionElem::Deref],
            });

        let errors = translate_and_verify(&fixture).expect_err("projection must fail");
        assert!(errors.contains(TranslationDiagnosticCode::UnsupportedProjection));
        assert_eq!(
            errors.diagnostics()[0]
                .location
                .source
                .as_ref()
                .map(|source| source.file.as_str()),
            Some("tests/scalar.rs")
        );
    }

    fn scalar_fixture() -> MirModule {
        MirModule {
            functions: vec![MirFunction {
                export_name: "scalar".to_string(),
                rust_path: "tests::scalar".to_string(),
                kind: MirFunctionKind::Kernel,
                arg_count: 2,
                local_count: 5,
                locals: vec![
                    local(0, MirLocalRole::Return, MirTypeShape::Unit),
                    local(1, MirLocalRole::Arg, MirTypeShape::F32),
                    local(2, MirLocalRole::Arg, MirTypeShape::F32),
                    local(3, MirLocalRole::Temp, MirTypeShape::F32),
                    local(4, MirLocalRole::Temp, MirTypeShape::Bool),
                ],
                blocks: vec![
                    MirBlock {
                        index: 0,
                        statements: vec![
                            assign(
                                0,
                                3,
                                vec![operand(1), operand(2)],
                                MirRvalueKind::Binary(MirBinaryOp::Add),
                            ),
                            assign(
                                1,
                                4,
                                vec![operand(1), operand(2)],
                                MirRvalueKind::Binary(MirBinaryOp::Lt),
                            ),
                        ],
                        terminator: Some(terminator(MirTerminatorKind::Assert {
                            condition: operand(4),
                            expected: true,
                            target: 1,
                        })),
                    },
                    MirBlock {
                        index: 1,
                        statements: Vec::new(),
                        terminator: Some(terminator(MirTerminatorKind::Return)),
                    },
                ],
            }],
        }
    }

    fn local(index: usize, role: MirLocalRole, shape: MirTypeShape) -> MirLocal {
        let (kind, rust) = match shape {
            MirTypeShape::Unit => (MirType::Unit, "()"),
            MirTypeShape::Bool => (MirType::I1, "bool"),
            MirTypeShape::F32 => (MirType::F32, "f32"),
            _ => (MirType::Unknown, "<unknown>"),
        };
        MirLocal {
            index,
            role,
            ty: MirImportedType {
                kind,
                rust: rust.to_string(),
                shape,
            },
        }
    }

    fn assign(
        index: usize,
        destination: usize,
        operands: Vec<MirOperandRef>,
        rvalue: MirRvalueKind,
    ) -> MirStatement {
        MirStatement {
            index,
            kind: MirStatementKind::Assign,
            destination: Some(place(destination)),
            operands,
            rvalue: Some(rvalue),
            operation: Some("structured".to_string()),
            source: Some(source()),
        }
    }

    fn terminator(kind: MirTerminatorKind) -> MirTerminator {
        MirTerminator {
            kind,
            source: Some(source()),
        }
    }

    fn operand(local: usize) -> MirOperandRef {
        MirOperandRef::Place(place(local))
    }

    fn place(local: usize) -> MirPlaceRef {
        MirPlaceRef {
            local,
            projection: Vec::new(),
        }
    }

    fn source() -> MirSourceLocation {
        MirSourceLocation {
            file: "tests/scalar.rs".to_string(),
            line: 1,
            column: 1,
        }
    }
}
