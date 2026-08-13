//! Bounded executable-MIR scalar control flow through Kernel IR and gfx942 LLVM.
//!
//! This first adapter is intentionally a helper-only prerequisite. It accepts
//! one validated place-form executable-MIR function, promotes it with the
//! verified `dialect-mir` mem2reg pass, admits its scalar expressions through
//! Scalar V2, and emits a verified Kernel IR device function plus direct LLVM
//! text. It does not grant linking, code-object, loading, or launch authority.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use dialect_amdgcn::{LoweringErrors, lower_device_module_to_gfx942_llvm_ir};
use dialect_mir::{
    MirBasicBlock, MirBinaryOp, MirBlockId, MirBodyForm, MirConstant, MirConstantValue, MirEdge,
    MirFunction, MirLocalId, MirLocalKind, MirMem2RegError, MirMem2RegReport, MirOperand,
    MirRvalue, MirScalarType, MirStatement, MirStatementKind, MirTerminatorKind, MirTypeId,
    MirTypeKind, MirValueId, ValidatedMirExecutableModule, analyze_mir_control_flow,
    promote_module_to_ssa,
};
use fe2o3_kernel_ir::scalar_ops_v2::{
    IntBinary, IntMode, IntWidth, Operation as ScalarOperation, Predicate, ScalarOperationV2,
    ScalarType as ScalarTypeV2,
};
use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, BlockId, ComparePredicate, Constant, Function, IntegerSwitchCase, Module,
    Operation, OperationKind, ScalarType, Signature, TargetCapability, Terminator, Type, ValueDef,
    ValueId, VerificationErrors, WaveWidth, verify_module,
};

use crate::AmdGpuTarget;
use crate::scalar_mir_v2::{
    EXACT_SCALAR_V2_TARGET, RustcMirBinaryV2, RustcScalarAdmissionErrorV2, RustcScalarExpressionV2,
    RustcScalarRequestV2, admit_rustc_scalar_operation_v2,
};

pub const MAX_SCALAR_CONTROL_FLOW_BLOCKS_V1: usize = 128;
pub const MAX_SCALAR_CONTROL_FLOW_LOOPS_V1: usize = 16;
pub const MAX_SCALAR_CONTROL_FLOW_LOOP_DEPTH_V1: usize = 8;
pub const MAX_SCALAR_CONTROL_FLOW_OPERATIONS_V1: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarControlFlowLocationV1 {
    pub block: MirBlockId,
    pub statement: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocatedScalarOperationV2 {
    pub location: ScalarControlFlowLocationV1,
    pub operation: ScalarOperationV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarControlFlowSummaryV1 {
    pub blocks: usize,
    pub loops: usize,
    pub maximum_loop_depth: usize,
    pub kernel_ir_operations: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableScalarControlFlowArtifactV1 {
    pub kernel_ir: Module,
    pub scalar_operations: Vec<LocatedScalarOperationV2>,
    pub mem2reg: MirMem2RegReport,
    pub summary: ScalarControlFlowSummaryV1,
    pub gfx942_llvm: String,
}

#[derive(Debug)]
pub enum ExecutableScalarControlFlowErrorV1 {
    ResourceLimit {
        resource: &'static str,
        limit: usize,
        actual: usize,
    },
    Unsupported {
        location: String,
        detail: String,
    },
    Mem2Reg(MirMem2RegError),
    Scalar {
        location: ScalarControlFlowLocationV1,
        source: RustcScalarAdmissionErrorV2,
    },
    InvalidKernelIr(VerificationErrors),
    Backend(LoweringErrors),
}

impl fmt::Display for ExecutableScalarControlFlowErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit {
                resource,
                limit,
                actual,
            } => write!(
                formatter,
                "executable scalar control-flow {resource} limit is {limit}, found {actual}"
            ),
            Self::Unsupported { location, detail } => {
                write!(
                    formatter,
                    "unsupported executable scalar control flow at {location}: {detail}"
                )
            }
            Self::Mem2Reg(error) => write!(formatter, "executable MIR mem2reg failed: {error}"),
            Self::Scalar { location, source } => write!(
                formatter,
                "Scalar V2 rejected bb{} statement {}: {source}",
                location.block.0, location.statement
            ),
            Self::InvalidKernelIr(error) => {
                write!(
                    formatter,
                    "generated scalar control-flow Kernel IR is invalid: {error}"
                )
            }
            Self::Backend(error) => {
                write!(
                    formatter,
                    "gfx942 scalar control-flow lowering failed: {error}"
                )
            }
        }
    }
}

impl Error for ExecutableScalarControlFlowErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Mem2Reg(error) => Some(error),
            Self::Scalar { source, .. } => Some(source),
            Self::InvalidKernelIr(error) => Some(error),
            Self::Backend(error) => Some(error),
            Self::ResourceLimit { .. } | Self::Unsupported { .. } => None,
        }
    }
}

/// Lowers the first closed executable-MIR scalar control-flow profile.
///
/// The returned LLVM is produced only after mem2reg, Scalar V2 admission, and
/// complete Kernel IR verification all succeed.
pub fn lower_executable_scalar_control_flow_v1(
    source: &ValidatedMirExecutableModule,
) -> Result<ExecutableScalarControlFlowArtifactV1, ExecutableScalarControlFlowErrorV1> {
    if source.functions.len() != 1 {
        return Err(resource_limit("function count", 1, source.functions.len()));
    }
    let source_function = &source.functions[0];
    if !matches!(source_function.body.form, MirBodyForm::Places) {
        return Err(unsupported(
            "module.functions[0].body.form",
            "V1 accepts validated source place form and owns the sole mem2reg transition",
        ));
    }
    check_limit(
        "block count",
        source_function.body.blocks.len(),
        MAX_SCALAR_CONTROL_FLOW_BLOCKS_V1,
    )?;

    let (ssa, mem2reg) =
        promote_module_to_ssa(source).map_err(ExecutableScalarControlFlowErrorV1::Mem2Reg)?;
    let function = &ssa.functions[0];
    let analysis = analyze_mir_control_flow(&function.body).map_err(|error| {
        unsupported(
            "module.functions[0].body",
            format!("control-flow analysis failed after mem2reg: {error}"),
        )
    })?;
    let loop_headers = analysis.loop_headers().collect::<Vec<_>>();
    check_limit(
        "natural loop count",
        loop_headers.len(),
        MAX_SCALAR_CONTROL_FLOW_LOOPS_V1,
    )?;
    let maximum_loop_depth = (0..analysis.block_count())
        .map(|block| {
            let block = MirBlockId(block as u32);
            loop_headers
                .iter()
                .filter(|header| {
                    analysis
                        .loop_body(**header)
                        .is_some_and(|body| body.contains(&block))
                })
                .count()
        })
        .max()
        .unwrap_or(0);
    check_limit(
        "natural loop nesting depth",
        maximum_loop_depth,
        MAX_SCALAR_CONTROL_FLOW_LOOP_DEPTH_V1,
    )?;

    let target = AmdGpuTarget::new(EXACT_SCALAR_V2_TARGET);
    let (mut kernel_function, scalar_operations, operation_count) =
        FunctionLowerer::new(ssa.as_module(), function, &target)?.lower()?;
    kernel_function
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));

    let mut kernel_ir = Module::new(format!("{}::scalar_control_flow_v1", function.identity));
    kernel_ir.functions.push(kernel_function);
    verify_module(&kernel_ir).map_err(ExecutableScalarControlFlowErrorV1::InvalidKernelIr)?;
    let gfx942_llvm = lower_device_module_to_gfx942_llvm_ir(&kernel_ir)
        .map_err(ExecutableScalarControlFlowErrorV1::Backend)?;

    Ok(ExecutableScalarControlFlowArtifactV1 {
        kernel_ir,
        scalar_operations,
        mem2reg,
        summary: ScalarControlFlowSummaryV1 {
            blocks: analysis.block_count(),
            loops: loop_headers.len(),
            maximum_loop_depth,
            kernel_ir_operations: operation_count,
        },
        gfx942_llvm,
    })
}

struct FunctionLowerer<'module, 'function, 'target> {
    module: &'module dialect_mir::MirExecutableModule,
    function: &'function MirFunction,
    target: &'target AmdGpuTarget,
    values: BTreeMap<MirValueId, (ValueId, Type)>,
    block_locals: BTreeMap<MirLocalId, (ValueId, Type)>,
    next_value: u32,
    scalar_operations: Vec<LocatedScalarOperationV2>,
    operation_count: usize,
}

impl<'module, 'function, 'target> FunctionLowerer<'module, 'function, 'target> {
    fn new(
        module: &'module dialect_mir::MirExecutableModule,
        function: &'function MirFunction,
        target: &'target AmdGpuTarget,
    ) -> Result<Self, ExecutableScalarControlFlowErrorV1> {
        let maximum_value = function
            .body
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .parameters
                    .iter()
                    .map(|parameter| parameter.value)
                    .chain(block.statements.iter().filter_map(|statement| {
                        if let MirStatementKind::Define { value, .. } = statement.kind {
                            Some(value)
                        } else {
                            None
                        }
                    }))
            })
            .map(|value| value.0)
            .max();
        let next_value = match maximum_value {
            Some(value) => value.checked_add(1).ok_or_else(|| {
                resource_limit("SSA value identities", u32::MAX as usize, usize::MAX)
            })?,
            None => 0,
        };
        Ok(Self {
            module,
            function,
            target,
            values: BTreeMap::new(),
            block_locals: BTreeMap::new(),
            next_value,
            scalar_operations: Vec::new(),
            operation_count: 0,
        })
    }

    fn lower(
        mut self,
    ) -> Result<(Function, Vec<LocatedScalarOperationV2>, usize), ExecutableScalarControlFlowErrorV1>
    {
        let return_ty = self.local_type(MirLocalId(0), "function return")?;
        if return_ty != Type::Scalar(ScalarType::U32) {
            return Err(unsupported(
                "module.functions[0].body.locals[0]",
                "V1 requires one u32 scalar return",
            ));
        }

        for (index, local) in self.function.body.locals.iter().enumerate() {
            let ty = self.type_at(local.ty, format!("local{index}"))?;
            if !matches!(ty, Type::Scalar(ScalarType::Bool | ScalarType::U32)) {
                return Err(unsupported(
                    format!("module.functions[0].body.locals[{index}]"),
                    "V1 locals are restricted to bool and u32",
                ));
            }
        }

        let entry = self.function.body.entry;
        let mut parameters = Vec::new();
        let mut parameter_types = Vec::new();
        for (local_index, local) in self.function.body.locals.iter().enumerate() {
            if local.kind != MirLocalKind::Argument {
                continue;
            }
            let local_id = MirLocalId(local_index as u32);
            let parameter = self.function.body.blocks[entry.0 as usize]
                .parameters
                .iter()
                .find(|parameter| parameter.origin == Some(local_id))
                .ok_or_else(|| {
                    unsupported(
                        format!("module.functions[0].body.locals[{local_index}]"),
                        "every V1 argument must be promoted to the entry SSA parameter list",
                    )
                })?;
            let ty = self.type_at(parameter.ty, format!("argument local{local_index}"))?;
            if ty != Type::Scalar(ScalarType::U32) {
                return Err(unsupported(
                    format!("module.functions[0].body.locals[{local_index}]"),
                    "V1 arguments are restricted to u32",
                ));
            }
            let value = ValueId(parameter.value.0);
            self.bind_value(parameter.value, value, ty.clone(), "entry argument")?;
            parameters.push(value);
            parameter_types.push(ty);
        }

        for (block_index, block) in self.function.body.blocks.iter().enumerate() {
            for parameter in &block.parameters {
                if MirBlockId(block_index as u32) == entry
                    && parameter.origin.is_some_and(|origin| {
                        self.function.body.locals[origin.0 as usize].kind == MirLocalKind::Argument
                    })
                {
                    continue;
                }
                let ty = self.type_at(
                    parameter.ty,
                    format!("bb{block_index} parameter v{}", parameter.value.0),
                )?;
                self.bind_value(
                    parameter.value,
                    ValueId(parameter.value.0),
                    ty,
                    &format!("bb{block_index} parameter"),
                )?;
            }
        }

        let mut blocks = Vec::with_capacity(self.function.body.blocks.len());
        for block_index in 0..self.function.body.blocks.len() {
            let block = self.function.body.blocks[block_index].clone();
            blocks.push(self.lower_block(MirBlockId(block_index as u32), &block)?);
        }

        let symbol = self
            .function
            .identity
            .rsplit("::")
            .next()
            .filter(|symbol| !symbol.is_empty())
            .ok_or_else(|| unsupported("module.functions[0].identity", "missing symbol stem"))?;
        let function = Function::device_ffi_export(
            symbol,
            Signature::new(parameter_types, vec![return_ty]),
            parameters,
            blocks,
        );
        Ok((function, self.scalar_operations, self.operation_count))
    }

    fn lower_block(
        &mut self,
        block_id: MirBlockId,
        source: &MirBasicBlock,
    ) -> Result<BasicBlock, ExecutableScalarControlFlowErrorV1> {
        self.block_locals.clear();
        let mut block = BasicBlock::new(BlockId(block_id.0));
        if block_id != self.function.body.entry {
            block.parameters = source
                .parameters
                .iter()
                .map(|parameter| {
                    let (value, ty) = self.value(parameter.value, "block parameter")?;
                    Ok(ValueDef::new(value, ty))
                })
                .collect::<Result<Vec<_>, ExecutableScalarControlFlowErrorV1>>()?;
        }

        let mut return_value = None;
        for (statement_index, statement) in source.statements.iter().enumerate() {
            self.lower_statement(
                block_id,
                statement_index,
                statement,
                &mut block.operations,
                &mut return_value,
            )?;
        }
        block.terminator = Some(self.lower_terminator(
            block_id,
            &source.terminator.kind,
            &mut block.operations,
            return_value,
        )?);
        Ok(block)
    }

    fn lower_statement(
        &mut self,
        block: MirBlockId,
        statement_index: usize,
        statement: &MirStatement,
        operations: &mut Vec<Operation>,
        return_value: &mut Option<ValueId>,
    ) -> Result<(), ExecutableScalarControlFlowErrorV1> {
        let location = ScalarControlFlowLocationV1 {
            block,
            statement: statement_index,
        };
        match &statement.kind {
            MirStatementKind::Define { value, ty, rvalue } => {
                self.lower_definition(location, *value, *ty, rvalue, operations)
            }
            MirStatementKind::Assign { place, value }
                if place.local == MirLocalId(0) && place.projection.is_empty() =>
            {
                if return_value.is_some() {
                    return Err(unsupported(
                        statement_location(location),
                        "a return block may assign the return local only once",
                    ));
                }
                let MirRvalue::Use(operand) = value else {
                    return Err(unsupported(
                        statement_location(location),
                        "the V1 return assignment must be a scalar Use",
                    ));
                };
                let (value, ty) = self.lower_operand(operand, operations, location)?;
                if ty != Type::Scalar(ScalarType::U32) {
                    return Err(unsupported(
                        statement_location(location),
                        "the V1 return value must be u32",
                    ));
                }
                *return_value = Some(value);
                Ok(())
            }
            MirStatementKind::Assign { place, value } if place.projection.is_empty() => {
                let expected = self.local_type(place.local, statement_location(location))?;
                let (value, actual) = match value {
                    MirRvalue::Use(operand) => self.lower_operand(operand, operations, location)?,
                    MirRvalue::BinaryOp { op, lhs, rhs } => {
                        let result = self.fresh_value()?;
                        self.lower_binary(
                            location,
                            result,
                            expected.clone(),
                            *op,
                            (lhs, rhs),
                            operations,
                        )?;
                        (result, expected.clone())
                    }
                    MirRvalue::CheckedBinaryOp { .. } => {
                        return Err(unsupported(
                            statement_location(location),
                            "checked multi-result arithmetic is outside the first control-flow slice",
                        ));
                    }
                    _ => {
                        return Err(unsupported(
                            statement_location(location),
                            "V1 block-local assignments admit only Use and the closed u32 scalar BinaryOp set",
                        ));
                    }
                };
                if actual != expected {
                    return Err(unsupported(
                        statement_location(location),
                        format!(
                            "block-local assignment type {actual:?} does not match {expected:?}"
                        ),
                    ));
                }
                self.block_locals.insert(place.local, (value, actual));
                Ok(())
            }
            MirStatementKind::Nop => Ok(()),
            _ => Err(unsupported(
                statement_location(location),
                "V1 accepts SSA definitions, one whole return-place assignment, and nop only",
            )),
        }
    }

    fn lower_definition(
        &mut self,
        location: ScalarControlFlowLocationV1,
        result: MirValueId,
        result_ty: MirTypeId,
        rvalue: &MirRvalue,
        operations: &mut Vec<Operation>,
    ) -> Result<(), ExecutableScalarControlFlowErrorV1> {
        let expected = self.type_at(result_ty, statement_location(location))?;
        match rvalue {
            MirRvalue::Use(operand) => {
                let (value, actual) = self.lower_operand(operand, operations, location)?;
                if actual != expected {
                    return Err(unsupported(
                        statement_location(location),
                        format!("Use type {actual:?} does not match result type {expected:?}"),
                    ));
                }
                self.bind_value(result, value, actual, &statement_location(location))
            }
            MirRvalue::BinaryOp { op, lhs, rhs } => {
                let value = ValueId(result.0);
                self.lower_binary(
                    location,
                    value,
                    expected.clone(),
                    *op,
                    (lhs, rhs),
                    operations,
                )?;
                self.bind_value(result, value, expected, &statement_location(location))
            }
            MirRvalue::CheckedBinaryOp { .. } => Err(unsupported(
                statement_location(location),
                "checked multi-result arithmetic is outside the first control-flow slice",
            )),
            _ => Err(unsupported(
                statement_location(location),
                "V1 admits only Use and the closed u32 scalar BinaryOp set",
            )),
        }
    }

    fn lower_binary(
        &mut self,
        location: ScalarControlFlowLocationV1,
        result: ValueId,
        result_ty: Type,
        op: MirBinaryOp,
        operands: (&MirOperand, &MirOperand),
        operations: &mut Vec<Operation>,
    ) -> Result<(), ExecutableScalarControlFlowErrorV1> {
        let mir_op = match op {
            MirBinaryOp::Add => RustcMirBinaryV2::Add,
            MirBinaryOp::Div => RustcMirBinaryV2::Div,
            MirBinaryOp::Eq => RustcMirBinaryV2::Eq,
            MirBinaryOp::Lt => RustcMirBinaryV2::Lt,
            _ => {
                return Err(unsupported(
                    statement_location(location),
                    format!("V1 scalar control flow does not admit {op:?}"),
                ));
            }
        };
        let (lhs, lhs_ty) = self.lower_operand(operands.0, operations, location)?;
        let (rhs, rhs_ty) = self.lower_operand(operands.1, operations, location)?;
        if lhs_ty != Type::Scalar(ScalarType::U32) || rhs_ty != Type::Scalar(ScalarType::U32) {
            return Err(unsupported(
                statement_location(location),
                "V1 binary operands must both be u32",
            ));
        }
        let expected_result = if matches!(op, MirBinaryOp::Eq | MirBinaryOp::Lt) {
            Type::BOOL
        } else {
            Type::Scalar(ScalarType::U32)
        };
        if result_ty != expected_result {
            return Err(unsupported(
                statement_location(location),
                format!("{op:?} result type must be {expected_result:?}"),
            ));
        }

        let carrier = admit_rustc_scalar_operation_v2(
            RustcScalarRequestV2 {
                target: self.target,
                custom_llvm_pipeline: false,
                expression: RustcScalarExpressionV2::Binary {
                    op: mir_op,
                    lhs: ScalarTypeV2::Int {
                        width: IntWidth::W32,
                        signed: false,
                    },
                    rhs: ScalarTypeV2::Int {
                        width: IntWidth::W32,
                        signed: false,
                    },
                    overflow_checks: false,
                },
            },
            vec![lhs, rhs],
        )
        .map_err(|source| ExecutableScalarControlFlowErrorV1::Scalar { location, source })?;
        let kind = match carrier.operation() {
            ScalarOperation::IntegerBinary {
                op: IntBinary::Add,
                mode: IntMode::Wrapping,
                ..
            } => OperationKind::Binary {
                op: BinaryOp::Add,
                lhs,
                rhs,
            },
            ScalarOperation::IntegerCompare { predicate, .. } => OperationKind::Compare {
                predicate: match predicate {
                    Predicate::Eq => ComparePredicate::Equal,
                    Predicate::Lt => ComparePredicate::LessThan,
                    _ => {
                        return Err(unsupported(
                            statement_location(location),
                            "Scalar V2 returned a comparison outside the closed V1 set",
                        ));
                    }
                },
                lhs,
                rhs,
            },
            _ => {
                return Err(unsupported(
                    statement_location(location),
                    "Scalar V2 returned an operation outside the closed V1 set",
                ));
            }
        };
        self.push_operation(
            operations,
            Operation::effect_free(ValueDef::new(result, result_ty), kind),
        )?;
        self.scalar_operations.push(LocatedScalarOperationV2 {
            location,
            operation: carrier,
        });
        Ok(())
    }

    fn lower_terminator(
        &mut self,
        block: MirBlockId,
        terminator: &MirTerminatorKind,
        operations: &mut Vec<Operation>,
        return_value: Option<ValueId>,
    ) -> Result<Terminator, ExecutableScalarControlFlowErrorV1> {
        match terminator {
            MirTerminatorKind::Goto(edge) => {
                let (target, arguments) = self.lower_edge(block, edge, operations)?;
                Ok(Terminator::Branch { target, arguments })
            }
            MirTerminatorKind::SwitchInt {
                discr,
                targets,
                otherwise,
            } => {
                let location = ScalarControlFlowLocationV1 {
                    block,
                    statement: self.function.body.blocks[block.0 as usize].statements.len(),
                };
                let (selector, selector_ty) = self.lower_operand(discr, operations, location)?;
                if selector_ty == Type::BOOL {
                    if targets.len() != 1 || !matches!(targets[0].0, 0 | 1) {
                        return Err(unsupported(
                            format!("bb{}.terminator", block.0),
                            "a bool SwitchInt must have exactly one case, 0 or 1",
                        ));
                    }
                    let (case_target, case_arguments) =
                        self.lower_edge(block, &targets[0].1, operations)?;
                    let (default_target, default_arguments) =
                        self.lower_edge(block, otherwise, operations)?;
                    let (then_target, then_arguments, else_target, else_arguments) =
                        if targets[0].0 == 1 {
                            (
                                case_target,
                                case_arguments,
                                default_target,
                                default_arguments,
                            )
                        } else {
                            (
                                default_target,
                                default_arguments,
                                case_target,
                                case_arguments,
                            )
                        };
                    Ok(Terminator::ConditionalBranch {
                        condition: selector,
                        then_target,
                        then_arguments,
                        else_target,
                        else_arguments,
                    })
                } else if selector_ty == Type::Scalar(ScalarType::U32) {
                    let cases = targets
                        .iter()
                        .map(|(value, edge)| {
                            let value = u32::try_from(*value).map_err(|_| {
                                unsupported(
                                    format!("bb{}.terminator", block.0),
                                    "u32 switch case is out of range",
                                )
                            })?;
                            let (target, arguments) = self.lower_edge(block, edge, operations)?;
                            Ok(IntegerSwitchCase {
                                value: Constant::U32(value),
                                target,
                                arguments,
                            })
                        })
                        .collect::<Result<Vec<_>, ExecutableScalarControlFlowErrorV1>>()?;
                    let (default_target, default_arguments) =
                        self.lower_edge(block, otherwise, operations)?;
                    Ok(Terminator::IntegerSwitch {
                        selector,
                        cases,
                        default_target,
                        default_arguments,
                    })
                } else {
                    Err(unsupported(
                        format!("bb{}.terminator", block.0),
                        "V1 SwitchInt selectors must be bool or u32",
                    ))
                }
            }
            MirTerminatorKind::Return => {
                let value = return_value.ok_or_else(|| {
                    unsupported(
                        format!("bb{}.terminator", block.0),
                        "u32 return requires one local0 assignment in the return block",
                    )
                })?;
                Ok(Terminator::Return {
                    values: vec![value],
                })
            }
            MirTerminatorKind::Unreachable => Ok(Terminator::Unreachable),
            _ => Err(unsupported(
                format!("bb{}.terminator", block.0),
                "V1 accepts goto, SwitchInt, return, and unreachable terminators only",
            )),
        }
    }

    fn lower_edge(
        &mut self,
        source: MirBlockId,
        edge: &MirEdge,
        operations: &mut Vec<Operation>,
    ) -> Result<(BlockId, Vec<ValueId>), ExecutableScalarControlFlowErrorV1> {
        let location = ScalarControlFlowLocationV1 {
            block: source,
            statement: self.function.body.blocks[source.0 as usize]
                .statements
                .len(),
        };
        let arguments = edge
            .arguments
            .iter()
            .map(|operand| {
                self.lower_operand(operand, operations, location)
                    .map(|(value, _)| value)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((BlockId(edge.target.0), arguments))
    }

    fn lower_operand(
        &mut self,
        operand: &MirOperand,
        operations: &mut Vec<Operation>,
        location: ScalarControlFlowLocationV1,
    ) -> Result<(ValueId, Type), ExecutableScalarControlFlowErrorV1> {
        match operand {
            MirOperand::Value(value) => self.value(*value, &statement_location(location)),
            MirOperand::Constant(constant) => {
                let (constant, ty) = self.constant(constant, location)?;
                let value = self.fresh_value()?;
                self.push_operation(
                    operations,
                    Operation::effect_free(
                        ValueDef::new(value, ty.clone()),
                        OperationKind::Constant(constant),
                    ),
                )?;
                Ok((value, ty))
            }
            MirOperand::Copy(place) | MirOperand::Move(place) if place.projection.is_empty() => {
                self.block_locals.get(&place.local).cloned().ok_or_else(|| {
                    unsupported(
                        statement_location(location),
                        format!(
                            "local{} is neither promoted nor defined in the current block",
                            place.local.0
                        ),
                    )
                })
            }
            MirOperand::Copy(_) | MirOperand::Move(_) => Err(unsupported(
                statement_location(location),
                "V1 does not admit projected scalar slot operands",
            )),
        }
    }

    fn constant(
        &self,
        constant: &MirConstant,
        location: ScalarControlFlowLocationV1,
    ) -> Result<(Constant, Type), ExecutableScalarControlFlowErrorV1> {
        let ty = self.type_at(constant.ty, statement_location(location))?;
        let value = match (&constant.value, &ty) {
            (MirConstantValue::Bool(value), Type::Scalar(ScalarType::Bool)) => {
                Constant::Bool(*value)
            }
            (MirConstantValue::Integer(bits), Type::Scalar(ScalarType::U32)) => {
                Constant::U32(u32::try_from(*bits).map_err(|_| {
                    unsupported(
                        statement_location(location),
                        "u32 constant bit pattern is out of range",
                    )
                })?)
            }
            _ => {
                return Err(unsupported(
                    statement_location(location),
                    "V1 constants are restricted to canonical bool and u32 values",
                ));
            }
        };
        Ok((value, ty))
    }

    fn type_at(
        &self,
        id: MirTypeId,
        location: impl Into<String>,
    ) -> Result<Type, ExecutableScalarControlFlowErrorV1> {
        match self.module.types[id.0 as usize].kind {
            MirTypeKind::Scalar(MirScalarType::Bool) => Ok(Type::BOOL),
            MirTypeKind::Scalar(MirScalarType::Int {
                signed: false,
                bits: 32,
            }) => Ok(Type::Scalar(ScalarType::U32)),
            _ => Err(unsupported(
                location,
                "V1 types are restricted to bool and u32",
            )),
        }
    }

    fn local_type(
        &self,
        local: MirLocalId,
        location: impl Into<String>,
    ) -> Result<Type, ExecutableScalarControlFlowErrorV1> {
        self.type_at(self.function.body.locals[local.0 as usize].ty, location)
    }

    fn bind_value(
        &mut self,
        source: MirValueId,
        value: ValueId,
        ty: Type,
        location: &str,
    ) -> Result<(), ExecutableScalarControlFlowErrorV1> {
        if self.values.insert(source, (value, ty)).is_some() {
            return Err(unsupported(
                location,
                format!("SSA value v{} was bound more than once", source.0),
            ));
        }
        Ok(())
    }

    fn value(
        &self,
        source: MirValueId,
        location: &str,
    ) -> Result<(ValueId, Type), ExecutableScalarControlFlowErrorV1> {
        self.values.get(&source).cloned().ok_or_else(|| {
            unsupported(
                location,
                format!("SSA value v{} has no lowered definition", source.0),
            )
        })
    }

    fn fresh_value(&mut self) -> Result<ValueId, ExecutableScalarControlFlowErrorV1> {
        let value = ValueId(self.next_value);
        self.next_value = self
            .next_value
            .checked_add(1)
            .ok_or_else(|| resource_limit("SSA value identities", u32::MAX as usize, usize::MAX))?;
        Ok(value)
    }

    fn push_operation(
        &mut self,
        operations: &mut Vec<Operation>,
        operation: Operation,
    ) -> Result<(), ExecutableScalarControlFlowErrorV1> {
        let next = self.operation_count.checked_add(1).ok_or_else(|| {
            resource_limit(
                "Kernel IR operation count",
                MAX_SCALAR_CONTROL_FLOW_OPERATIONS_V1,
                usize::MAX,
            )
        })?;
        check_limit(
            "Kernel IR operation count",
            next,
            MAX_SCALAR_CONTROL_FLOW_OPERATIONS_V1,
        )?;
        self.operation_count = next;
        operations.push(operation);
        Ok(())
    }
}

fn check_limit(
    resource: &'static str,
    actual: usize,
    limit: usize,
) -> Result<(), ExecutableScalarControlFlowErrorV1> {
    if actual > limit {
        Err(resource_limit(resource, limit, actual))
    } else {
        Ok(())
    }
}

fn resource_limit(
    resource: &'static str,
    limit: usize,
    actual: usize,
) -> ExecutableScalarControlFlowErrorV1 {
    ExecutableScalarControlFlowErrorV1::ResourceLimit {
        resource,
        limit,
        actual,
    }
}

fn unsupported(
    location: impl Into<String>,
    detail: impl Into<String>,
) -> ExecutableScalarControlFlowErrorV1 {
    ExecutableScalarControlFlowErrorV1::Unsupported {
        location: location.into(),
        detail: detail.into(),
    }
}

fn statement_location(location: ScalarControlFlowLocationV1) -> String {
    format!("bb{}.statements[{}]", location.block.0, location.statement)
}
