use crate::collector::{CollectedFunction, CollectionResult};
use crate::record_lowering::{
    RecordAccessSketch, RecordBinaryOp, RecordExpression, RecordExpressionOperand,
    RecordExpressionSketch, RecordLinearIndex, RecordLoweringFunction, RecordLoweringLocal,
    RecordLoweringPlan, RecordSliceAccess, RecordUnaryOp,
};
use crate::trusted_device_items::{self, TrustedDeviceItem};
use crate::{AmdGpuTarget, compile_llvm_ir_to_hsaco};
use dialect_mir::MirOp;
use fe2o3_artifact_transaction::{
    BuildAttempt, ProducerIdentity, emit_artifact_transaction,
    emit_artifact_transaction_after_preflight,
    emit_artifact_transaction_after_preflight_for_attempt, emit_artifact_transaction_for_attempt,
};
pub use fe2o3_artifact_transaction::{DeviceArtifact, EmitError};
use rustc_middle::mir::{
    BinOp, Body, ConstOperand, Local, Operand, Place, ProjectionElem, Rvalue, StatementKind,
    TerminatorKind, UnOp, VarDebugInfoContents,
};
use rustc_middle::ty::{
    EarlyBinder, FloatTy, IntTy, Mutability, Ty, TyCtxt, TyKind, TypingEnv, UintTy,
};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub fn emit_collection<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    producer: &ProducerIdentity,
    lowering_plan: Option<&RecordLoweringPlan>,
    output_dir: &Path,
    target: &AmdGpuTarget,
    attempt: Option<BuildAttempt>,
) -> Result<Vec<DeviceArtifact>, EmitError> {
    let kernels = prepare_collection(tcx, collection, lowering_plan)?;

    let compile = |llvm_ir_path: &Path, hsaco_path: &Path| {
        compile_llvm_ir_to_hsaco(llvm_ir_path, hsaco_path, target)
            .map_err(|error| EmitError::Compilation(Box::new(error)))
    };
    match attempt {
        Some(attempt) => emit_artifact_transaction_for_attempt(
            output_dir,
            producer,
            attempt,
            &kernels,
            |kernel| &kernel.name,
            |kernel| Ok(kernel.llvm_ir.clone()),
            compile,
        ),
        None => emit_artifact_transaction(
            output_dir,
            producer,
            &kernels,
            |kernel| &kernel.name,
            |kernel| Ok(kernel.llvm_ir.clone()),
            compile,
        ),
    }
}

pub(crate) fn emit_collection_after_preflight(
    producer: &ProducerIdentity,
    output_dir: &Path,
    target: &AmdGpuTarget,
    attempt: Option<BuildAttempt>,
    preflight: impl FnOnce() -> Result<Vec<PreparedDeviceKernel>, EmitError>,
) -> Result<Vec<DeviceArtifact>, EmitError> {
    let compile = |llvm_ir_path: &Path, hsaco_path: &Path| {
        compile_llvm_ir_to_hsaco(llvm_ir_path, hsaco_path, target)
            .map_err(|error| EmitError::Compilation(Box::new(error)))
    };
    match attempt {
        Some(attempt) => emit_artifact_transaction_after_preflight_for_attempt(
            output_dir,
            producer,
            attempt,
            preflight,
            |kernel| &kernel.name,
            |kernel| Ok(kernel.llvm_ir.clone()),
            compile,
        ),
        None => emit_artifact_transaction_after_preflight(
            output_dir,
            producer,
            preflight,
            |kernel| &kernel.name,
            |kernel| Ok(kernel.llvm_ir.clone()),
            compile,
        ),
    }
}

#[derive(Debug)]
pub(crate) struct PreparedDeviceKernel {
    pub(crate) name: String,
    pub(crate) llvm_ir: String,
}

pub(crate) fn prepare_collection<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    lowering_plan: Option<&RecordLoweringPlan>,
) -> Result<Vec<PreparedDeviceKernel>, EmitError> {
    collection
        .functions
        .iter()
        .filter(|function| function.is_kernel_entry())
        .map(|kernel| {
            let record_function = lowering_plan.and_then(|plan| plan.function(&kernel.export_name));
            Ok(PreparedDeviceKernel {
                name: kernel.export_name.clone(),
                llvm_ir: emit_kernel(tcx, kernel, record_function)?,
            })
        })
        .collect()
}

fn emit_kernel<'tcx>(
    tcx: TyCtxt<'tcx>,
    kernel: &CollectedFunction<'tcx>,
    record_function: Option<&RecordLoweringFunction>,
) -> Result<String, EmitError> {
    let mir = tcx.instance_mir(kernel.instance.def);
    let abi = analyze_kernel_abi(tcx, kernel)?;
    if let Some(record_function) = record_function {
        validate_record_abi(&kernel.export_name, &abi, record_function)?;
    }
    let mut elementwise = analyze_elementwise_shape(tcx, mir).map_err(|reason| {
        unsupported_kernel(
            &kernel.export_name,
            format!("unsupported MIR body for elementwise lowering: {reason}"),
        )
    })?;
    if let Some(record_function) = record_function {
        validate_record_elementwise_shape(&kernel.export_name, &elementwise, record_function)?;
        if let Some(record_expr) = record_elementwise_expr(&elementwise, record_function) {
            elementwise.expr = record_expr;
        }
    }

    emit_elementwise_kernel(&abi, &elementwise)
}

#[derive(Clone, Debug)]
struct KernelAbi {
    name: String,
    args: Vec<KernelArg>,
}

#[derive(Clone, Debug)]
struct KernelArg {
    name: String,
    kind: KernelArgKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ElementwiseShape {
    expr: ElementwiseExpr,
    output_arg: usize,
    output_index: IndexExpr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ElementwiseExpr {
    nodes: Vec<ExprNode>,
    root: ExprRef,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ExprNode {
    Binary {
        lhs: ExprRef,
        rhs: ExprRef,
        op: ElementwiseBinaryOp,
    },
    Unary {
        operand: ExprRef,
        op: ElementwiseUnaryOp,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ValueSource {
    Arg(usize),
    SliceElement(SliceElementSource),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SliceElementSource {
    arg_index: usize,
    index: IndexExpr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
enum IndexExpr {
    Thread,
    Offset(i64),
    Stride(i64),
    StrideOffset { stride: i64, offset: i64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ElementwiseBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ElementwiseUnaryOp {
    Neg,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExprRef {
    Value(ValueSource),
    Literal(ScalarLiteral),
    Node(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarLiteral {
    F32(u32),
    F64(u64),
}

#[derive(Clone, Debug)]
enum KernelArgKind {
    Scalar(ScalarType),
    Slice { element: ScalarType, mutable: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarType {
    F32,
    F64,
}

impl KernelArg {
    fn llvm_base(&self) -> &str {
        &self.name
    }

    fn llvm_params(&self) -> Vec<String> {
        match self.kind {
            KernelArgKind::Scalar(element) => {
                vec![format!("{} %{}", element.llvm_type(), self.llvm_base())]
            }
            KernelArgKind::Slice { .. } => {
                let base = self.llvm_base();
                vec![
                    format!("ptr addrspace(1) %{base}_ptr"),
                    format!("i64 %{base}_len"),
                ]
            }
        }
    }

    fn element(&self) -> ScalarType {
        match self.kind {
            KernelArgKind::Scalar(element) => element,
            KernelArgKind::Slice { element, .. } => element,
        }
    }

    fn is_mutable(&self) -> bool {
        match self.kind {
            KernelArgKind::Scalar(_) => false,
            KernelArgKind::Slice { mutable, .. } => mutable,
        }
    }

    fn is_scalar(&self) -> bool {
        matches!(self.kind, KernelArgKind::Scalar(_))
    }

    fn is_slice(&self) -> bool {
        matches!(self.kind, KernelArgKind::Slice { .. })
    }
}

impl ScalarType {
    fn llvm_type(self) -> &'static str {
        match self {
            Self::F32 => "float",
            Self::F64 => "double",
        }
    }

    fn llvm_align(self) -> usize {
        match self {
            Self::F32 => 4,
            Self::F64 => 8,
        }
    }
}

impl ScalarLiteral {
    fn scalar_type(self) -> ScalarType {
        match self {
            Self::F32(_) => ScalarType::F32,
            Self::F64(_) => ScalarType::F64,
        }
    }
}

impl ElementwiseBinaryOp {
    fn from_mir(op: BinOp) -> Option<Self> {
        match op {
            BinOp::Add => Some(Self::Add),
            BinOp::Sub => Some(Self::Sub),
            BinOp::Mul => Some(Self::Mul),
            BinOp::Div => Some(Self::Div),
            _ => None,
        }
    }

    fn llvm_opcode(self) -> &'static str {
        match self {
            Self::Add => "fadd",
            Self::Sub => "fsub",
            Self::Mul => "fmul",
            Self::Div => "fdiv",
        }
    }
}

impl ElementwiseUnaryOp {
    fn from_mir(op: UnOp) -> Option<Self> {
        match op {
            UnOp::Neg => Some(Self::Neg),
            _ => None,
        }
    }

    fn llvm_opcode(self) -> &'static str {
        match self {
            Self::Neg => "fneg",
        }
    }
}

impl ElementwiseExpr {
    fn sources(&self) -> Vec<ValueSource> {
        let mut sources = Vec::new();
        self.collect_sources(self.root, &mut sources);
        sources
    }

    fn literals(&self) -> Vec<ScalarLiteral> {
        let mut literals = Vec::new();
        self.collect_literals(self.root, &mut literals);
        literals
    }

    fn collect_sources(&self, expr: ExprRef, sources: &mut Vec<ValueSource>) {
        match expr {
            ExprRef::Value(source) => {
                if !sources.contains(&source) {
                    sources.push(source);
                }
            }
            ExprRef::Literal(_) => {}
            ExprRef::Node(index) => match &self.nodes[index] {
                ExprNode::Binary { lhs, rhs, .. } => {
                    self.collect_sources(*lhs, sources);
                    self.collect_sources(*rhs, sources);
                }
                ExprNode::Unary { operand, .. } => {
                    self.collect_sources(*operand, sources);
                }
            },
        }
    }

    fn collect_literals(&self, expr: ExprRef, literals: &mut Vec<ScalarLiteral>) {
        match expr {
            ExprRef::Value(_) => {}
            ExprRef::Literal(literal) => literals.push(literal),
            ExprRef::Node(index) => match &self.nodes[index] {
                ExprNode::Binary { lhs, rhs, .. } => {
                    self.collect_literals(*lhs, literals);
                    self.collect_literals(*rhs, literals);
                }
                ExprNode::Unary { operand, .. } => {
                    self.collect_literals(*operand, literals);
                }
            },
        }
    }
}

fn analyze_kernel_abi<'tcx>(
    tcx: TyCtxt<'tcx>,
    kernel: &CollectedFunction<'tcx>,
) -> Result<KernelAbi, EmitError> {
    let mir = tcx.instance_mir(kernel.instance.def);
    let arg_names = source_arg_names(mir);
    let mut used_names = HashSet::new();
    let mut args = Vec::new();

    for (source_index, local) in mir.args_iter().enumerate() {
        let ty = tcx.instantiate_and_normalize_erasing_regions(
            kernel.instance.args,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(mir.local_decls[local].ty),
        );
        let kind = classify_kernel_arg(tcx, ty).map_err(|reason| EmitError::UnsupportedKernel {
            kernel: kernel.export_name.clone(),
            reason: format!("argument {source_index} has unsupported type `{ty}`: {reason}"),
        })?;

        let raw_name = arg_names
            .get(source_index)
            .and_then(|name| name.as_deref())
            .unwrap_or("arg");
        let name = unique_llvm_name(raw_name, source_index, &mut used_names);

        args.push(KernelArg { name, kind });
    }

    Ok(KernelAbi {
        name: kernel.export_name.clone(),
        args,
    })
}

fn source_arg_names<'tcx>(mir: &Body<'tcx>) -> Vec<Option<String>> {
    let mut names = vec![None; mir.arg_count];

    for debug_info in &mir.var_debug_info {
        if let Some(argument_index) = debug_info.argument_index {
            let source_index = usize::from(argument_index.saturating_sub(1));
            if source_index < names.len() && names[source_index].is_none() {
                names[source_index] = Some(debug_info.name.to_string());
            }
            continue;
        }

        let VarDebugInfoContents::Place(place) = debug_info.value else {
            continue;
        };
        if !place.projection.is_empty() {
            continue;
        }

        for (source_index, local) in mir.args_iter().enumerate() {
            if place.local == local && names[source_index].is_none() {
                names[source_index] = Some(debug_info.name.to_string());
                break;
            }
        }
    }

    names
}

fn unique_llvm_name(raw_name: &str, source_index: usize, used: &mut HashSet<String>) -> String {
    let base = sanitize_llvm_name(raw_name)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("arg{source_index}"));

    if used.insert(base.clone()) {
        return base;
    }

    for suffix in 1.. {
        let candidate = format!("{base}_{suffix}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }

    unreachable!("unbounded suffix loop must return");
}

fn sanitize_llvm_name(raw_name: &str) -> Option<String> {
    let mut sanitized = String::new();
    for character in raw_name.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            sanitized.push(character);
        } else if !sanitized.ends_with('_') {
            sanitized.push('_');
        }
    }

    while sanitized.ends_with('_') {
        sanitized.pop();
    }

    if sanitized
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        sanitized.insert(0, '_');
    }

    (!sanitized.is_empty()).then_some(sanitized)
}

fn validate_record_abi(
    kernel_name: &str,
    abi: &KernelAbi,
    record_function: &RecordLoweringFunction,
) -> Result<(), EmitError> {
    if record_function.kind != "kernel" {
        return Err(unsupported_kernel(
            kernel_name,
            format!(
                "record lowering plan expected `{}` to be a kernel, found `{}`",
                record_function.symbol, record_function.kind
            ),
        ));
    }

    if record_function.arg_count != abi.args.len() {
        return Err(unsupported_kernel(
            kernel_name,
            format!(
                "record lowering plan ABI has {} args, MIR ABI has {} args",
                record_function.arg_count,
                abi.args.len()
            ),
        ));
    }

    let record_args = record_function.args();
    if record_args.len() != abi.args.len() {
        return Err(unsupported_kernel(
            kernel_name,
            format!(
                "record lowering plan has {} typed arg locals, MIR ABI has {} args",
                record_args.len(),
                abi.args.len()
            ),
        ));
    }

    for (index, (record_arg, abi_arg)) in record_args.iter().zip(&abi.args).enumerate() {
        if !record_arg_matches_abi(record_arg, abi_arg) {
            return Err(unsupported_kernel(
                kernel_name,
                format!(
                    "record lowering arg {index} type `{}` does not match MIR ABI",
                    record_arg.rust_ty
                ),
            ));
        }
    }

    if !record_function.has_op(MirOp::Store) {
        return Err(unsupported_kernel(
            kernel_name,
            "record lowering plan is missing an output store",
        ));
    }
    if !record_function.has_op(MirOp::Return) {
        return Err(unsupported_kernel(
            kernel_name,
            "record lowering plan is missing a return terminator",
        ));
    }

    Ok(())
}

fn record_arg_matches_abi(record_arg: &RecordLoweringLocal, abi_arg: &KernelArg) -> bool {
    match &abi_arg.kind {
        KernelArgKind::Scalar(ScalarType::F32) => record_arg.ty == "mir.f32",
        KernelArgKind::Scalar(ScalarType::F64) => record_arg.ty == "mir.f64",
        KernelArgKind::Slice { .. } => {
            record_arg.ty == "mir.slice" || record_arg.ty == "mir.disjoint_slice"
        }
    }
}

fn validate_record_elementwise_shape(
    kernel_name: &str,
    shape: &ElementwiseShape,
    record_function: &RecordLoweringFunction,
) -> Result<(), EmitError> {
    if !record_function.has_trusted_call(TrustedDeviceItem::ThreadIndex1d) {
        return Err(unsupported_kernel(
            kernel_name,
            "record lowering plan is missing a `thread::index_1d` call",
        ));
    }

    let access_sketch = record_function.access_sketch();
    let expected_loads = read_slice_sources(shape).len();
    let record_loads = access_sketch.loads.len();
    if record_loads < expected_loads {
        return Err(unsupported_kernel(
            kernel_name,
            format!(
                "record lowering plan has {record_loads} load(s), elementwise shape needs at least {expected_loads}"
            ),
        ));
    }
    validate_record_slice_accesses(kernel_name, shape, record_function, &access_sketch)?;
    validate_record_expression_shape(kernel_name, &shape.expr, record_function)?;

    let binary_nodes = shape
        .expr
        .nodes
        .iter()
        .filter(|node| matches!(node, ExprNode::Binary { .. }))
        .count();
    let arithmetic_ops = record_arithmetic_op_count(record_function);
    if binary_nodes > 0 && arithmetic_ops == 0 {
        return Err(unsupported_kernel(
            kernel_name,
            "record lowering plan is missing arithmetic for an elementwise expression",
        ));
    }

    if shape.output_index != IndexExpr::Thread && !record_has_index_transform(record_function) {
        return Err(unsupported_kernel(
            kernel_name,
            "record lowering plan is missing index transform operations for shifted output",
        ));
    }

    Ok(())
}

struct RecordExpressionRequirements {
    binary_ops: Vec<RecordBinaryOp>,
    unary_ops: Vec<RecordUnaryOp>,
    scalar_args: HashSet<usize>,
    literals: Vec<ScalarLiteral>,
}

fn validate_record_expression_shape(
    kernel_name: &str,
    expr: &ElementwiseExpr,
    record_function: &RecordLoweringFunction,
) -> Result<(), EmitError> {
    let requirements = record_expression_requirements(expr);
    let sketch = record_function.expression_sketch();

    for op in [
        RecordBinaryOp::Add,
        RecordBinaryOp::Sub,
        RecordBinaryOp::Mul,
        RecordBinaryOp::Div,
    ] {
        let expected = requirements
            .binary_ops
            .iter()
            .filter(|candidate| **candidate == op)
            .count();
        let actual = sketch.binary_op_count(op);
        if actual < expected {
            return Err(unsupported_kernel(
                kernel_name,
                format!(
                    "record lowering plan has {actual} {:?} expression op(s), elementwise shape needs {expected}",
                    op
                ),
            ));
        }
    }

    let expected_neg = requirements
        .unary_ops
        .iter()
        .filter(|candidate| **candidate == RecordUnaryOp::Neg)
        .count();
    let actual_neg = sketch.unary_op_count(RecordUnaryOp::Neg);
    if actual_neg < expected_neg {
        return Err(unsupported_kernel(
            kernel_name,
            format!(
                "record lowering plan has {actual_neg} Neg expression op(s), elementwise shape needs {expected_neg}"
            ),
        ));
    }

    for arg_index in requirements.scalar_args {
        if !sketch.uses_scalar_arg(arg_index) {
            return Err(unsupported_kernel(
                kernel_name,
                format!("record lowering plan is missing scalar arg{arg_index} expression use"),
            ));
        }
    }

    for literal in requirements.literals {
        let (ty, value_fragment) = literal_record_parts(literal);
        if !sketch.uses_constant(ty, &value_fragment) {
            return Err(unsupported_kernel(
                kernel_name,
                format!(
                    "record lowering plan is missing {ty} literal expression use containing {value_fragment}"
                ),
            ));
        }
    }

    Ok(())
}

fn record_expression_requirements(expr: &ElementwiseExpr) -> RecordExpressionRequirements {
    let mut requirements = RecordExpressionRequirements {
        binary_ops: Vec::new(),
        unary_ops: Vec::new(),
        scalar_args: HashSet::new(),
        literals: Vec::new(),
    };
    collect_expr_ref_requirements(expr, expr.root, &mut requirements);
    requirements
}

fn collect_expr_ref_requirements(
    expr: &ElementwiseExpr,
    expr_ref: ExprRef,
    requirements: &mut RecordExpressionRequirements,
) {
    match expr_ref {
        ExprRef::Value(ValueSource::Arg(arg_index)) => {
            requirements.scalar_args.insert(arg_index);
        }
        ExprRef::Value(ValueSource::SliceElement(_)) => {}
        ExprRef::Literal(literal) => {
            if !requirements.literals.contains(&literal) {
                requirements.literals.push(literal);
            }
        }
        ExprRef::Node(index) => {
            let Some(node) = expr.nodes.get(index) else {
                return;
            };
            match node {
                ExprNode::Binary { lhs, rhs, op } => {
                    requirements
                        .binary_ops
                        .push(record_binary_op_from_elementwise(*op));
                    collect_expr_ref_requirements(expr, *lhs, requirements);
                    collect_expr_ref_requirements(expr, *rhs, requirements);
                }
                ExprNode::Unary { operand, op } => {
                    requirements
                        .unary_ops
                        .push(record_unary_op_from_elementwise(*op));
                    collect_expr_ref_requirements(expr, *operand, requirements);
                }
            }
        }
    }
}

fn record_binary_op_from_elementwise(op: ElementwiseBinaryOp) -> RecordBinaryOp {
    match op {
        ElementwiseBinaryOp::Add => RecordBinaryOp::Add,
        ElementwiseBinaryOp::Sub => RecordBinaryOp::Sub,
        ElementwiseBinaryOp::Mul => RecordBinaryOp::Mul,
        ElementwiseBinaryOp::Div => RecordBinaryOp::Div,
    }
}

fn record_unary_op_from_elementwise(op: ElementwiseUnaryOp) -> RecordUnaryOp {
    match op {
        ElementwiseUnaryOp::Neg => RecordUnaryOp::Neg,
    }
}

fn literal_record_parts(literal: ScalarLiteral) -> (&'static str, String) {
    match literal {
        ScalarLiteral::F32(bits) => ("mir.f32", format!("0x{bits:08x}")),
        ScalarLiteral::F64(bits) => ("mir.f64", format!("0x{bits:016x}")),
    }
}

fn record_elementwise_expr(
    shape: &ElementwiseShape,
    record_function: &RecordLoweringFunction,
) -> Option<ElementwiseExpr> {
    let sketch = record_function.expression_sketch();
    let record_args = record_function.args();
    let output_local = record_args.get(shape.output_arg).map(|arg| arg.index);
    let store = output_local
        .and_then(|output_local| {
            let mut stores = sketch
                .stores
                .iter()
                .filter(|store| store.destination.local == output_local);
            let store = stores.next()?;
            stores.next().is_none().then_some(store)
        })
        .or_else(|| {
            let mut stores = sketch.stores.iter();
            let store = stores.next()?;
            stores.next().is_none().then_some(store)
        })?;

    let mut nodes = Vec::new();
    let mut local_cache = HashMap::new();
    let mut visiting = HashSet::new();
    let root = record_expr_ref(
        &sketch,
        &store.expr,
        &mut nodes,
        &mut local_cache,
        &mut visiting,
    )?;
    Some(ElementwiseExpr { nodes, root })
}

fn record_expr_ref(
    sketch: &RecordExpressionSketch,
    expr: &RecordExpression,
    nodes: &mut Vec<ExprNode>,
    local_cache: &mut HashMap<usize, ExprRef>,
    visiting: &mut HashSet<usize>,
) -> Option<ExprRef> {
    match expr {
        RecordExpression::Use(operand) => {
            record_operand_expr_ref(sketch, operand, nodes, local_cache, visiting)
        }
        RecordExpression::Binary { lhs, rhs, op } => {
            let lhs = record_operand_expr_ref(sketch, lhs, nodes, local_cache, visiting)?;
            let rhs = record_operand_expr_ref(sketch, rhs, nodes, local_cache, visiting)?;
            let node = ExprRef::Node(nodes.len());
            nodes.push(ExprNode::Binary {
                lhs,
                rhs,
                op: elementwise_binary_op_from_record(*op),
            });
            Some(node)
        }
        RecordExpression::Unary { operand, op } => {
            let operand = record_operand_expr_ref(sketch, operand, nodes, local_cache, visiting)?;
            let node = ExprRef::Node(nodes.len());
            nodes.push(ExprNode::Unary {
                operand,
                op: elementwise_unary_op_from_record(*op),
            });
            Some(node)
        }
    }
}

fn record_operand_expr_ref(
    sketch: &RecordExpressionSketch,
    operand: &RecordExpressionOperand,
    nodes: &mut Vec<ExprNode>,
    local_cache: &mut HashMap<usize, ExprRef>,
    visiting: &mut HashSet<usize>,
) -> Option<ExprRef> {
    match operand {
        RecordExpressionOperand::Local(local) => {
            if let Some(expr) = local_cache.get(local).copied() {
                return Some(expr);
            }
            if !visiting.insert(*local) {
                return None;
            }
            let binding = sketch
                .local_bindings
                .iter()
                .find(|binding| binding.local == *local)?;
            let expr = record_expr_ref(sketch, &binding.expr, nodes, local_cache, visiting)?;
            visiting.remove(local);
            local_cache.insert(*local, expr);
            Some(expr)
        }
        RecordExpressionOperand::ScalarArg { arg_index, .. } => {
            Some(ExprRef::Value(ValueSource::Arg(*arg_index)))
        }
        RecordExpressionOperand::SliceElement(access) => {
            let index = index_expr_from_record(access.index)?;
            Some(ExprRef::Value(ValueSource::SliceElement(
                SliceElementSource {
                    arg_index: access.arg_index,
                    index,
                },
            )))
        }
        RecordExpressionOperand::Constant { ty, value } => {
            record_constant_literal(ty, value).map(ExprRef::Literal)
        }
    }
}

fn elementwise_binary_op_from_record(op: RecordBinaryOp) -> ElementwiseBinaryOp {
    match op {
        RecordBinaryOp::Add => ElementwiseBinaryOp::Add,
        RecordBinaryOp::Sub => ElementwiseBinaryOp::Sub,
        RecordBinaryOp::Mul => ElementwiseBinaryOp::Mul,
        RecordBinaryOp::Div => ElementwiseBinaryOp::Div,
    }
}

fn elementwise_unary_op_from_record(op: RecordUnaryOp) -> ElementwiseUnaryOp {
    match op {
        RecordUnaryOp::Neg => ElementwiseUnaryOp::Neg,
    }
}

fn index_expr_from_record(index: RecordLinearIndex) -> Option<IndexExpr> {
    IndexExpr::strided_offset(index.stride, index.offset)
}

fn record_constant_literal(ty: &str, value: &str) -> Option<ScalarLiteral> {
    let bits = parse_record_hex_fragment(value)?;
    match ty {
        "mir.f32" => u32::try_from(bits).ok().map(ScalarLiteral::F32),
        "mir.f64" => Some(ScalarLiteral::F64(bits)),
        _ => None,
    }
}

fn parse_record_hex_fragment(value: &str) -> Option<u64> {
    let start = value.find("0x")? + 2;
    let hex = value[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_hexdigit())
        .collect::<String>();
    u64::from_str_radix(&hex, 16).ok()
}

fn validate_record_slice_accesses(
    kernel_name: &str,
    shape: &ElementwiseShape,
    record_function: &RecordLoweringFunction,
    access_sketch: &RecordAccessSketch,
) -> Result<(), EmitError> {
    if access_sketch.stores.is_empty() {
        return Err(unsupported_kernel(
            kernel_name,
            "record lowering plan is missing a parsed output store",
        ));
    }

    let record_args = record_function.args();
    let slice_access_sketch = record_function.slice_access_sketch();
    let mut expected_loads_by_local = HashMap::new();
    for source in read_slice_sources(shape) {
        let Some(arg) = record_args.get(source.arg_index) else {
            continue;
        };
        if source.arg_index == shape.output_arg && arg.ty == "mir.disjoint_slice" {
            continue;
        }
        *expected_loads_by_local.entry(arg.index).or_insert(0usize) += 1;
        validate_record_slice_access_index(
            kernel_name,
            "load",
            &slice_access_sketch.reads,
            source.arg_index,
            arg.index,
            source.index,
        )?;
    }

    let Some(output_arg) = record_args.get(shape.output_arg) else {
        return Ok(());
    };
    if output_arg.ty == "mir.slice" && output_arg.rust_ty.starts_with("&mut ") {
        validate_record_slice_access_index(
            kernel_name,
            "store",
            &slice_access_sketch.writes,
            shape.output_arg,
            output_arg.index,
            shape.output_index,
        )?;
        if !slice_access_sketch
            .writes
            .iter()
            .any(|store| store.local == output_arg.index)
        {
            return Err(unsupported_kernel(
                kernel_name,
                format!(
                    "record lowering plan is missing a store to output arg local{}",
                    output_arg.index
                ),
            ));
        }
    }

    for (local, expected) in expected_loads_by_local {
        let actual = access_sketch
            .loads
            .iter()
            .filter(|load| load.place.local == local)
            .count();
        if actual < expected {
            return Err(unsupported_kernel(
                kernel_name,
                format!(
                    "record lowering plan has {actual} load(s) from local{local}, elementwise shape needs {expected}"
                ),
            ));
        }
    }

    Ok(())
}

fn validate_record_slice_access_index(
    kernel_name: &str,
    access_kind: &str,
    accesses: &[RecordSliceAccess],
    arg_index: usize,
    local: usize,
    expected: IndexExpr,
) -> Result<(), EmitError> {
    let expected = RecordLinearIndex::from(expected);
    if accesses.iter().any(|access| {
        access.arg_index == arg_index && access.local == local && access.index == expected
    }) {
        return Ok(());
    }

    Err(unsupported_kernel(
        kernel_name,
        format!(
            "record lowering plan is missing a {access_kind} on local{local} (arg{arg_index}) with index stride {} offset {}",
            expected.stride, expected.offset
        ),
    ))
}

impl From<IndexExpr> for RecordLinearIndex {
    fn from(index: IndexExpr) -> Self {
        match index {
            IndexExpr::Thread => Self {
                stride: 1,
                offset: 0,
            },
            IndexExpr::Offset(offset) => Self { stride: 1, offset },
            IndexExpr::Stride(stride) => Self { stride, offset: 0 },
            IndexExpr::StrideOffset { stride, offset } => Self { stride, offset },
        }
    }
}

fn record_arithmetic_op_count(record_function: &RecordLoweringFunction) -> usize {
    record_function
        .ops
        .iter()
        .filter(|op| {
            matches!(op.op, MirOp::Add | MirOp::Sub | MirOp::Mul | MirOp::Div)
                || op.operation.as_deref().is_some_and(is_arithmetic_operation)
        })
        .count()
}

fn is_arithmetic_operation(operation: &str) -> bool {
    matches!(
        operation,
        "add"
            | "add_unchecked"
            | "add_with_overflow"
            | "sub"
            | "sub_unchecked"
            | "sub_with_overflow"
            | "mul"
            | "mul_unchecked"
            | "mul_with_overflow"
            | "div"
    )
}

fn record_has_index_transform(record_function: &RecordLoweringFunction) -> bool {
    record_function.has_op(MirOp::Add)
        || record_function.has_op(MirOp::Sub)
        || record_function.has_op(MirOp::Mul)
        || record_function.has_op(MirOp::Gep)
        || record_function.has_trusted_call(TrustedDeviceItem::ThreadIndexOffset)
        || record_function.has_trusted_call(TrustedDeviceItem::ThreadIndexOffsetSigned)
        || record_function.has_trusted_call(TrustedDeviceItem::ThreadIndexStride)
        || record_function.has_trusted_call(TrustedDeviceItem::ThreadIndexStrideOffset)
}

fn classify_kernel_arg<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Result<KernelArgKind, &'static str> {
    match ty.kind() {
        TyKind::Ref(_, pointee, mutability) => {
            let TyKind::Slice(element) = pointee.kind() else {
                return Err("only slice references are supported");
            };
            let element = classify_scalar(*element)?;
            Ok(KernelArgKind::Slice {
                element,
                mutable: *mutability == Mutability::Mut,
            })
        }
        TyKind::Adt(adt, args) if is_disjoint_slice(tcx, adt.did()) => {
            let element = classify_scalar(args.type_at(0))?;
            Ok(KernelArgKind::Slice {
                element,
                mutable: true,
            })
        }
        TyKind::Float(FloatTy::F32) => Ok(KernelArgKind::Scalar(ScalarType::F32)),
        TyKind::Float(FloatTy::F64) => Ok(KernelArgKind::Scalar(ScalarType::F64)),
        _ => Err("expected `f32`, `f64`, `&[T]`, `&mut [T]`, or `DisjointSlice<T>`"),
    }
}

fn classify_scalar(ty: Ty<'_>) -> Result<ScalarType, &'static str> {
    match ty.kind() {
        TyKind::Float(FloatTy::F32) => Ok(ScalarType::F32),
        TyKind::Float(FloatTy::F64) => Ok(ScalarType::F64),
        _ => Err("only `f32` and `f64` elements are supported"),
    }
}

fn is_disjoint_slice(tcx: TyCtxt<'_>, def_id: rustc_hir::def_id::DefId) -> bool {
    trusted_device_items::classify(tcx, def_id) == Some(TrustedDeviceItem::DisjointSlice)
}

fn analyze_elementwise_shape<'tcx>(
    tcx: TyCtxt<'tcx>,
    mir: &Body<'tcx>,
) -> Result<ElementwiseShape, String> {
    let arg_locals = mir.args_iter().collect::<Vec<_>>();
    let mut borrowed_args = HashMap::new();
    let mut borrowed_locals = HashMap::new();

    for block in mir.basic_blocks.iter() {
        for statement in &block.statements {
            let StatementKind::Assign(assign) = &statement.kind else {
                continue;
            };
            let (place, rvalue) = &**assign;
            if let Rvalue::Ref(_, _, borrowed_place) = rvalue
                && place.projection.is_empty()
            {
                if borrowed_place.projection.is_empty() {
                    borrowed_locals.insert(place.local, borrowed_place.local);
                }
                if let Some(arg_index) = local_arg_index(borrowed_place.local, &arg_locals) {
                    borrowed_args.insert(place.local, arg_index);
                }
            }
        }
    }

    let mut thread_index_local = None;
    let mut index_expr_locals = HashMap::new();
    let mut overflow_index_expr_locals = HashMap::new();
    let mut disjoint_output_source = None;

    for block in mir.basic_blocks.iter() {
        let Some(terminator) = &block.terminator else {
            continue;
        };
        let TerminatorKind::Call {
            func,
            args,
            destination,
            ..
        } = &terminator.kind
        else {
            continue;
        };
        let Some((def_id, _)) = func.const_fn_def() else {
            continue;
        };
        let trusted_item = trusted_device_items::classify(tcx, def_id);

        if trusted_item == Some(TrustedDeviceItem::ThreadIndex1d) {
            if destination.projection.is_empty() {
                thread_index_local = Some(destination.local);
            }
            continue;
        }

        if trusted_item == Some(TrustedDeviceItem::ThreadIndexGet) {
            let Some(thread_index_local) = thread_index_local else {
                continue;
            };
            if destination.projection.is_empty()
                && args.first().is_some_and(|arg| {
                    operand_refers_to_local(&arg.node, &borrowed_locals, thread_index_local)
                })
            {
                index_expr_locals.insert(destination.local, IndexExpr::Thread);
            }
            continue;
        }

        if trusted_item == Some(TrustedDeviceItem::ThreadIndexOffset) {
            let Some(thread_index_local) = thread_index_local else {
                continue;
            };
            if destination.projection.is_empty()
                && args.first().is_some_and(|arg| {
                    operand_refers_to_local(&arg.node, &borrowed_locals, thread_index_local)
                })
                && let Some(offset) = args
                    .get(1)
                    .and_then(|arg| operand_usize_const(tcx, &arg.node))
                    .and_then(|offset| i64::try_from(offset).ok())
                && let Some(index) = IndexExpr::Thread.offset(offset)
            {
                index_expr_locals.insert(destination.local, index);
            }
            continue;
        }

        if trusted_item == Some(TrustedDeviceItem::ThreadIndexOffsetSigned) {
            let Some(thread_index_local) = thread_index_local else {
                continue;
            };
            if destination.projection.is_empty()
                && args.first().is_some_and(|arg| {
                    operand_refers_to_local(&arg.node, &borrowed_locals, thread_index_local)
                })
                && let Some(offset) = args
                    .get(1)
                    .and_then(|arg| operand_isize_const(tcx, &arg.node))
                && let Some(index) = IndexExpr::Thread.offset(offset)
            {
                index_expr_locals.insert(destination.local, index);
            }
            continue;
        }

        if trusted_item == Some(TrustedDeviceItem::ThreadIndexStride) {
            let Some(thread_index_local) = thread_index_local else {
                continue;
            };
            if destination.projection.is_empty()
                && args.first().is_some_and(|arg| {
                    operand_refers_to_local(&arg.node, &borrowed_locals, thread_index_local)
                })
                && let Some(stride) = args
                    .get(1)
                    .and_then(|arg| operand_usize_const(tcx, &arg.node))
                    .and_then(|stride| i64::try_from(stride).ok())
            {
                index_expr_locals.insert(destination.local, IndexExpr::Stride(stride));
            }
            continue;
        }

        if trusted_item == Some(TrustedDeviceItem::ThreadIndexStrideOffset) {
            let Some(thread_index_local) = thread_index_local else {
                continue;
            };
            if destination.projection.is_empty()
                && args.first().is_some_and(|arg| {
                    operand_refers_to_local(&arg.node, &borrowed_locals, thread_index_local)
                })
                && let Some(stride) = args
                    .get(1)
                    .and_then(|arg| operand_usize_const(tcx, &arg.node))
                    .and_then(|stride| i64::try_from(stride).ok())
                && let Some(offset) = args
                    .get(2)
                    .and_then(|arg| operand_isize_const(tcx, &arg.node))
                && let Some(index) = IndexExpr::strided_offset(stride, offset)
            {
                index_expr_locals.insert(destination.local, index);
            }
            continue;
        }
    }

    thread_index_local.ok_or_else(|| "missing `thread::index_1d` call".to_string())?;
    propagate_index_exprs(
        tcx,
        mir,
        &mut index_expr_locals,
        &mut overflow_index_expr_locals,
    );

    for block in mir.basic_blocks.iter() {
        let Some(terminator) = &block.terminator else {
            continue;
        };
        let TerminatorKind::Call { func, args, .. } = &terminator.kind else {
            continue;
        };
        let Some((def_id, _)) = func.const_fn_def() else {
            continue;
        };
        let trusted_item = trusted_device_items::classify(tcx, def_id);
        if !matches!(
            trusted_item,
            Some(TrustedDeviceItem::DisjointSliceGetMut | TrustedDeviceItem::DisjointSliceGetMutAt)
        ) {
            continue;
        }
        let Some(receiver_local) = args.first().and_then(|arg| operand_local(&arg.node)) else {
            continue;
        };
        let Some(arg_index) = borrowed_args.get(&receiver_local).copied() else {
            continue;
        };

        if trusted_item == Some(TrustedDeviceItem::DisjointSliceGetMut) {
            if args
                .get(1)
                .and_then(|arg| operand_local(&arg.node))
                .is_some_and(|local| Some(local) == thread_index_local)
            {
                disjoint_output_source = Some(SliceElementSource {
                    arg_index,
                    index: IndexExpr::Thread,
                });
            }
        } else if trusted_item == Some(TrustedDeviceItem::DisjointSliceGetMutAt)
            && let Some(index) = args.get(1).and_then(|arg| {
                operand_index_expr(&arg.node, &index_expr_locals, &overflow_index_expr_locals)
            })
        {
            disjoint_output_source = Some(SliceElementSource { arg_index, index });
        }
    }

    let mut local_exprs = HashMap::new();
    let mut expr_nodes = Vec::new();
    let mut elementwise_store = None;

    for block in mir.basic_blocks.iter() {
        for statement in &block.statements {
            let StatementKind::Assign(assign) = &statement.kind else {
                continue;
            };
            let (place, rvalue) = &**assign;

            match rvalue {
                Rvalue::Use(operand) => {
                    let expr = if let Some(source_place) = operand.place()
                        && let Some(source) =
                            indexed_arg_place(source_place, &arg_locals, &index_expr_locals)
                    {
                        Some(ExprRef::Value(ValueSource::SliceElement(source)))
                    } else if let Some(source_place) = operand.place()
                        && is_deref_place(&source_place)
                        && let Some(output_source) = disjoint_output_source
                    {
                        Some(ExprRef::Value(ValueSource::SliceElement(
                            SliceElementSource {
                                arg_index: output_source.arg_index,
                                index: output_source.index,
                            },
                        )))
                    } else {
                        operand_expr(tcx, operand, &local_exprs, &arg_locals)
                    };

                    let Some(expr) = expr else {
                        continue;
                    };

                    if place.projection.is_empty() {
                        local_exprs.insert(place.local, expr);
                    } else if let Some(output) = store_output_source(
                        place,
                        disjoint_output_source,
                        &arg_locals,
                        &index_expr_locals,
                    ) {
                        disjoint_output_source = Some(output);
                        elementwise_store = Some((expr, output));
                    }
                }
                Rvalue::BinaryOp(op, operands) => {
                    let Some(op) = ElementwiseBinaryOp::from_mir(*op) else {
                        continue;
                    };
                    let lhs = operand_expr(tcx, &operands.0, &local_exprs, &arg_locals);
                    let rhs = operand_expr(tcx, &operands.1, &local_exprs, &arg_locals);

                    if let (Some(lhs), Some(rhs)) = (lhs, rhs) {
                        let expr = ExprRef::Node(expr_nodes.len());
                        expr_nodes.push(ExprNode::Binary { lhs, rhs, op });

                        if place.projection.is_empty() {
                            local_exprs.insert(place.local, expr);
                        } else if let Some(output) = store_output_source(
                            place,
                            disjoint_output_source,
                            &arg_locals,
                            &index_expr_locals,
                        ) {
                            disjoint_output_source = Some(output);
                            elementwise_store = Some((expr, output));
                        }
                    }
                }
                Rvalue::UnaryOp(op, operand) => {
                    let Some(op) = ElementwiseUnaryOp::from_mir(*op) else {
                        continue;
                    };
                    let Some(operand) = operand_expr(tcx, operand, &local_exprs, &arg_locals)
                    else {
                        continue;
                    };

                    let expr = ExprRef::Node(expr_nodes.len());
                    expr_nodes.push(ExprNode::Unary { operand, op });

                    if place.projection.is_empty() {
                        local_exprs.insert(place.local, expr);
                    } else if let Some(output) = store_output_source(
                        place,
                        disjoint_output_source,
                        &arg_locals,
                        &index_expr_locals,
                    ) {
                        disjoint_output_source = Some(output);
                        elementwise_store = Some((expr, output));
                    }
                }
                _ => {}
            }
        }
    }

    let (root, output) = elementwise_store
        .ok_or_else(|| "missing `output[idx] = <elementwise expression>` store".to_string())?;
    let output_arg = output.arg_index;
    let expr = ElementwiseExpr {
        nodes: expr_nodes,
        root,
    };

    Ok(ElementwiseShape {
        expr,
        output_arg,
        output_index: output.index,
    })
}

impl ValueSource {
    fn arg_index(self) -> usize {
        match self {
            Self::Arg(index) => index,
            Self::SliceElement(source) => source.arg_index,
        }
    }
}

impl IndexExpr {
    fn constant(value: i64) -> Option<Self> {
        Self::strided_offset(0, value)
    }

    fn strided_offset(stride: i64, offset: i64) -> Option<Self> {
        if stride == 1 && offset == 0 {
            Some(Self::Thread)
        } else if stride == 1 {
            Some(Self::Offset(offset))
        } else if offset == 0 {
            Some(Self::Stride(stride))
        } else {
            Some(Self::StrideOffset { stride, offset })
        }
    }

    fn scale(self, factor: i64) -> Option<Self> {
        match self {
            Self::Thread => Self::strided_offset(factor, 0),
            Self::Offset(offset) => offset
                .checked_mul(factor)
                .and_then(|offset| Self::strided_offset(factor, offset)),
            Self::Stride(stride) => stride
                .checked_mul(factor)
                .and_then(|stride| Self::strided_offset(stride, 0)),
            Self::StrideOffset { stride, offset } => {
                let stride = stride.checked_mul(factor)?;
                let offset = offset.checked_mul(factor)?;
                Self::strided_offset(stride, offset)
            }
        }
    }

    fn add_index(self, rhs: Self) -> Option<Self> {
        let (lhs_stride, lhs_offset) = self.linear();
        let (rhs_stride, rhs_offset) = rhs.linear();
        let stride = lhs_stride.checked_add(rhs_stride)?;
        let offset = lhs_offset.checked_add(rhs_offset)?;
        Self::strided_offset(stride, offset)
    }

    fn sub_index(self, rhs: Self) -> Option<Self> {
        let (lhs_stride, lhs_offset) = self.linear();
        let (rhs_stride, rhs_offset) = rhs.linear();
        let stride = lhs_stride.checked_sub(rhs_stride)?;
        let offset = lhs_offset.checked_sub(rhs_offset)?;
        Self::strided_offset(stride, offset)
    }

    fn linear(self) -> (i64, i64) {
        match self {
            Self::Thread => (1, 0),
            Self::Offset(offset) => (1, offset),
            Self::Stride(stride) => (stride, 0),
            Self::StrideOffset { stride, offset } => (stride, offset),
        }
    }

    fn offset(self, offset: i64) -> Option<Self> {
        if offset == 0 {
            return Some(self);
        }

        match self {
            Self::Thread => Some(Self::Offset(offset)),
            Self::Offset(base) => base.checked_add(offset).map(Self::Offset),
            Self::Stride(stride) => Self::strided_offset(stride, offset),
            Self::StrideOffset {
                stride,
                offset: base,
            } => base
                .checked_add(offset)
                .and_then(|offset| Self::strided_offset(stride, offset)),
        }
    }

    fn llvm_value(self) -> String {
        match self {
            Self::Thread => "%idx".to_string(),
            Self::Offset(offset) => format!("%idx{}", Self::llvm_suffix_for_offset(offset)),
            Self::Stride(stride) => format!("%idx{}", Self::llvm_suffix_for_stride(stride)),
            Self::StrideOffset { stride, offset } => {
                format!(
                    "%idx{}{}",
                    Self::llvm_suffix_for_stride(stride),
                    Self::llvm_suffix_for_offset(offset)
                )
            }
        }
    }

    fn llvm_suffix(self) -> String {
        match self {
            Self::Thread => String::new(),
            Self::Offset(offset) => Self::llvm_suffix_for_offset(offset),
            Self::Stride(stride) => Self::llvm_suffix_for_stride(stride),
            Self::StrideOffset { stride, offset } => {
                format!(
                    "{}{}",
                    Self::llvm_suffix_for_stride(stride),
                    Self::llvm_suffix_for_offset(offset)
                )
            }
        }
    }

    fn llvm_suffix_for_stride(stride: i64) -> String {
        if stride >= 0 {
            format!("_s{stride}")
        } else {
            format!("_sm{}", stride.unsigned_abs())
        }
    }

    fn llvm_suffix_for_offset(offset: i64) -> String {
        if offset >= 0 {
            format!("_p{offset}")
        } else {
            format!("_m{}", offset.unsigned_abs())
        }
    }

    fn is_unique_per_thread(self) -> bool {
        match self {
            Self::Thread | Self::Offset(_) => true,
            Self::Stride(stride) | Self::StrideOffset { stride, .. } => stride != 0,
        }
    }
}

fn propagate_index_exprs<'tcx>(
    tcx: TyCtxt<'tcx>,
    mir: &Body<'tcx>,
    index_expr_locals: &mut HashMap<Local, IndexExpr>,
    overflow_index_expr_locals: &mut HashMap<Local, IndexExpr>,
) {
    let mut changed = true;
    while changed {
        changed = false;
        for block in mir.basic_blocks.iter() {
            for statement in &block.statements {
                let StatementKind::Assign(assign) = &statement.kind else {
                    continue;
                };
                let (place, rvalue) = &**assign;
                if !place.projection.is_empty() {
                    continue;
                }

                match rvalue {
                    Rvalue::Use(operand) => {
                        if let Some(index) = operand_index_expr(
                            operand,
                            index_expr_locals,
                            overflow_index_expr_locals,
                        ) {
                            changed |= insert_index_expr(index_expr_locals, place.local, index);
                        }
                    }
                    Rvalue::BinaryOp(op, operands) => {
                        let Some(index) = index_binary_op(
                            tcx,
                            *op,
                            &operands.0,
                            &operands.1,
                            index_expr_locals,
                            overflow_index_expr_locals,
                        ) else {
                            continue;
                        };

                        if bin_op_returns_overflow_tuple(*op) {
                            changed |=
                                insert_index_expr(overflow_index_expr_locals, place.local, index);
                        } else {
                            changed |= insert_index_expr(index_expr_locals, place.local, index);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn insert_index_expr(
    indexes: &mut HashMap<Local, IndexExpr>,
    local: Local,
    index: IndexExpr,
) -> bool {
    if indexes.get(&local).copied() == Some(index) {
        return false;
    }
    indexes.insert(local, index);
    true
}

fn index_binary_op<'tcx>(
    tcx: TyCtxt<'tcx>,
    op: BinOp,
    lhs: &Operand<'tcx>,
    rhs: &Operand<'tcx>,
    index_expr_locals: &HashMap<Local, IndexExpr>,
    overflow_index_expr_locals: &HashMap<Local, IndexExpr>,
) -> Option<IndexExpr> {
    let lhs_index = operand_index_expr(lhs, index_expr_locals, overflow_index_expr_locals);
    let rhs_index = operand_index_expr(rhs, index_expr_locals, overflow_index_expr_locals);
    let lhs_const = operand_usize_i64_const(tcx, lhs);
    let rhs_const = operand_usize_i64_const(tcx, rhs);

    match op {
        BinOp::Add | BinOp::AddWithOverflow => match (lhs_index, rhs_index, lhs_const, rhs_const) {
            (Some(lhs), Some(rhs), _, _) => lhs.add_index(rhs),
            (Some(index), None, _, Some(offset)) => index.offset(offset),
            (None, Some(index), Some(offset), _) => index.offset(offset),
            _ => None,
        },
        BinOp::Sub | BinOp::SubWithOverflow => match (lhs_index, rhs_index, lhs_const, rhs_const) {
            (Some(lhs), Some(rhs), _, _) => lhs.sub_index(rhs),
            (Some(index), None, _, Some(offset)) => index.offset(offset.checked_neg()?),
            (None, Some(index), Some(offset), _) => IndexExpr::constant(offset)?.sub_index(index),
            _ => None,
        },
        BinOp::Mul | BinOp::MulWithOverflow => match (lhs_index, rhs_index, lhs_const, rhs_const) {
            (Some(index), None, _, Some(factor)) => index.scale(factor),
            (None, Some(index), Some(factor), _) => index.scale(factor),
            _ => None,
        },
        _ => None,
    }
}

fn bin_op_returns_overflow_tuple(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::AddWithOverflow | BinOp::SubWithOverflow | BinOp::MulWithOverflow
    )
}

fn operand_index_expr(
    operand: &Operand<'_>,
    index_expr_locals: &HashMap<Local, IndexExpr>,
    overflow_index_expr_locals: &HashMap<Local, IndexExpr>,
) -> Option<IndexExpr> {
    let place = operand.place()?;
    if place.projection.is_empty() {
        return index_expr_locals.get(&place.local).copied();
    }

    overflow_value_place(place, overflow_index_expr_locals)
}

fn overflow_value_place(
    place: Place<'_>,
    overflow_index_expr_locals: &HashMap<Local, IndexExpr>,
) -> Option<IndexExpr> {
    let [ProjectionElem::Field(field, _)] = &place.projection[..] else {
        return None;
    };
    if field.index() != 0 {
        return None;
    }
    overflow_index_expr_locals.get(&place.local).copied()
}

fn operand_local(operand: &Operand<'_>) -> Option<Local> {
    let place = operand.place()?;
    place.projection.is_empty().then_some(place.local)
}

fn operand_refers_to_local(
    operand: &Operand<'_>,
    borrowed_locals: &HashMap<Local, Local>,
    expected: Local,
) -> bool {
    let Some(mut local) = operand_local(operand) else {
        return false;
    };
    let mut remaining = borrowed_locals.len();
    loop {
        if local == expected {
            return true;
        }
        if remaining == 0 {
            return false;
        }
        let Some(next) = borrowed_locals.get(&local).copied() else {
            return false;
        };
        local = next;
        remaining -= 1;
    }
}

fn local_arg_index(local: Local, arg_locals: &[Local]) -> Option<usize> {
    arg_locals.iter().position(|candidate| *candidate == local)
}

fn indexed_arg_place(
    place: Place<'_>,
    arg_locals: &[Local],
    index_expr_locals: &HashMap<Local, IndexExpr>,
) -> Option<SliceElementSource> {
    let [ProjectionElem::Deref, ProjectionElem::Index(index_local)] = &place.projection[..] else {
        return None;
    };
    let index = index_expr_locals.get(index_local).copied()?;
    let arg_index = local_arg_index(place.local, arg_locals)?;
    Some(SliceElementSource { arg_index, index })
}

fn store_output_source(
    place: &Place<'_>,
    disjoint_output_source: Option<SliceElementSource>,
    arg_locals: &[Local],
    index_expr_locals: &HashMap<Local, IndexExpr>,
) -> Option<SliceElementSource> {
    if is_deref_place(place) {
        return disjoint_output_source;
    }

    indexed_arg_place(*place, arg_locals, index_expr_locals)
}

fn operand_expr<'tcx>(
    tcx: TyCtxt<'tcx>,
    operand: &Operand<'tcx>,
    local_exprs: &HashMap<Local, ExprRef>,
    arg_locals: &[Local],
) -> Option<ExprRef> {
    if let Operand::Constant(constant) = operand {
        return constant_float(tcx, constant).map(ExprRef::Literal);
    }

    let local = operand_local(operand)?;
    local_exprs.get(&local).copied().or_else(|| {
        local_arg_index(local, arg_locals).map(|arg| ExprRef::Value(ValueSource::Arg(arg)))
    })
}

fn operand_usize_const<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>) -> Option<u64> {
    let Operand::Constant(constant) = operand else {
        return None;
    };
    if !matches!(constant.const_.ty().kind(), TyKind::Uint(UintTy::Usize)) {
        return None;
    }
    constant
        .const_
        .try_eval_target_usize(tcx, TypingEnv::fully_monomorphized())
}

fn operand_usize_i64_const<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>) -> Option<i64> {
    operand_usize_const(tcx, operand).and_then(|value| i64::try_from(value).ok())
}

fn operand_isize_const<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>) -> Option<i64> {
    let Operand::Constant(constant) = operand else {
        return None;
    };
    if !matches!(constant.const_.ty().kind(), TyKind::Int(IntTy::Isize)) {
        return None;
    }
    Some(
        constant
            .const_
            .try_eval_scalar_int(tcx, TypingEnv::fully_monomorphized())?
            .to_target_isize(tcx),
    )
}

fn constant_float<'tcx>(tcx: TyCtxt<'tcx>, constant: &ConstOperand<'tcx>) -> Option<ScalarLiteral> {
    let int = constant
        .const_
        .try_eval_scalar_int(tcx, TypingEnv::fully_monomorphized())?;
    match constant.const_.ty().kind() {
        TyKind::Float(FloatTy::F32) => Some(ScalarLiteral::F32(int.to_u32())),
        TyKind::Float(FloatTy::F64) => Some(ScalarLiteral::F64(int.to_u64())),
        _ => None,
    }
}

fn is_deref_place(place: &Place<'_>) -> bool {
    matches!(&place.projection[..], [ProjectionElem::Deref])
}

fn emit_elementwise_kernel(abi: &KernelAbi, shape: &ElementwiseShape) -> Result<String, EmitError> {
    validate_elementwise_abi(abi, shape)?;

    let params = abi
        .args
        .iter()
        .flat_map(KernelArg::llvm_params)
        .collect::<Vec<_>>()
        .join(", ");
    let c = &abi.args[shape.output_arg];
    let c_base = c.llvm_base();
    let element = c.element();
    let llvm_type = element.llvm_type();
    let align = element.llvm_align();
    let read_slice_sources = read_slice_sources(shape);
    let slice_sources = bounds_slice_sources(shape);
    let index_lines = emit_index_calculations(&slice_sources);
    let check_lines = emit_bounds_checks(abi, &slice_sources);
    let inbounds = emit_inbounds_reduce(&slice_sources, abi);
    let load_lines = emit_loads(abi, &read_slice_sources, llvm_type, align);
    let compute_lines = emit_expr_ops(&shape.expr, abi, llvm_type);
    let result_value = expr_ref_value(shape.expr.root, abi);
    let output_index = shape.output_index.llvm_value();

    // This is the first AMDGPU LLVM IR milestone, intentionally scoped to
    // simple f32/f64 elementwise expression kernels. It matches
    // `LaunchConfig::for_num_elems`, which uses a 256-thread block. General
    // block_dim lowering will read the AMD dispatch packet instead of
    // hard-coding this constant.
    Ok(format!(
        r#"target triple = "{triple}"

declare i32 @llvm.amdgcn.workitem.id.x() #1
declare i32 @llvm.amdgcn.workgroup.id.x() #1

define amdgpu_kernel void @{kernel_name}({params}) #0 {{
entry:
  %tid = call i32 @llvm.amdgcn.workitem.id.x()
  %bid = call i32 @llvm.amdgcn.workgroup.id.x()
  %base = mul i32 %bid, 256
  %idx32 = add i32 %base, %tid
  %idx = zext i32 %idx32 to i64
{index_lines}{check_lines}{inbounds}
  br i1 %inbounds, label %body, label %exit

body:
{load_lines}{compute_lines}
  %{c_base}_store_ptr = getelementptr inbounds {llvm_type}, ptr addrspace(1) %{c_base}_ptr, i64 {output_index}
  store {llvm_type} {result_value}, ptr addrspace(1) %{c_base}_store_ptr, align {align}
  br label %exit

exit:
  ret void
}}

attributes #0 = {{ nounwind }}
attributes #1 = {{ nounwind readnone speculatable }}
"#,
        triple = dialect_amdgcn::AMDGPU_TRIPLE,
        kernel_name = abi.name,
        params = params,
        check_lines = check_lines,
        index_lines = index_lines,
        inbounds = inbounds,
        load_lines = load_lines,
        c_base = c_base,
        llvm_type = llvm_type,
        align = align,
        compute_lines = compute_lines,
        output_index = output_index,
        result_value = result_value,
    ))
}

fn validate_elementwise_abi(abi: &KernelAbi, shape: &ElementwiseShape) -> Result<(), EmitError> {
    let sources = shape.expr.sources();

    for arg_index in sources
        .iter()
        .map(|source| source.arg_index())
        .chain(std::iter::once(shape.output_arg))
    {
        if arg_index >= abi.args.len() {
            return Err(unsupported_kernel(
                &abi.name,
                "MIR elementwise argument index is outside the kernel ABI",
            ));
        }
    }

    if !abi.args[shape.output_arg].is_slice() || !abi.args[shape.output_arg].is_mutable() {
        return Err(unsupported_kernel(
            &abi.name,
            "elementwise lowering expects one mutable output slice",
        ));
    }

    if !shape.output_index.is_unique_per_thread() {
        return Err(unsupported_kernel(
            &abi.name,
            "mutable output index must be unique per thread",
        ));
    }

    let output_element = abi.args[shape.output_arg].element();

    for literal in shape.expr.literals() {
        if literal.scalar_type() != output_element {
            return Err(unsupported_kernel(
                &abi.name,
                "literal elementwise operands must match the output element type",
            ));
        }
    }

    for source in sources {
        let arg = &abi.args[source.arg_index()];
        if arg.element() != output_element {
            return Err(unsupported_kernel(
                &abi.name,
                "elementwise operands must match the output element type",
            ));
        }

        match source {
            ValueSource::Arg(_) if !arg.is_scalar() => {
                return Err(unsupported_kernel(
                    &abi.name,
                    "direct elementwise operands must be scalar arguments",
                ));
            }
            ValueSource::SliceElement(_) if !arg.is_slice() => {
                return Err(unsupported_kernel(
                    &abi.name,
                    "indexed elementwise operands must be slices",
                ));
            }
            ValueSource::SliceElement(source)
                if arg.is_mutable() && source.arg_index != shape.output_arg =>
            {
                return Err(unsupported_kernel(
                    &abi.name,
                    "mutable indexed elementwise operands must be the output slice",
                ));
            }
            _ => {}
        }
    }

    Ok(())
}

fn read_slice_sources(shape: &ElementwiseShape) -> Vec<SliceElementSource> {
    let mut sources = Vec::new();
    for source in shape.expr.sources() {
        if let ValueSource::SliceElement(source) = source
            && !sources.contains(&source)
        {
            sources.push(source);
        }
    }
    sources
}

fn bounds_slice_sources(shape: &ElementwiseShape) -> Vec<SliceElementSource> {
    let mut sources = read_slice_sources(shape);
    let output_source = SliceElementSource {
        arg_index: shape.output_arg,
        index: shape.output_index,
    };
    if !sources.contains(&output_source) {
        sources.push(output_source);
    }
    sources
}

fn emit_index_calculations(slice_sources: &[SliceElementSource]) -> String {
    let mut indexes = Vec::new();
    for source in slice_sources {
        if source.index != IndexExpr::Thread && !indexes.contains(&source.index) {
            indexes.push(source.index);
        }
    }

    let mut emitted = Vec::new();
    let mut lines = String::new();
    for index in indexes {
        if emitted.contains(&index) {
            continue;
        }

        let line = match index {
            IndexExpr::Thread => String::new(),
            IndexExpr::Offset(offset) => {
                format!("  {} = add i64 %idx, {offset}\n", index.llvm_value())
            }
            IndexExpr::Stride(stride) => {
                format!("  {} = mul i64 %idx, {stride}\n", index.llvm_value())
            }
            IndexExpr::StrideOffset { stride, offset } => {
                let stride_index = IndexExpr::Stride(stride);
                if !emitted.contains(&stride_index) {
                    lines.push_str(&format!(
                        "  {} = mul i64 %idx, {stride}\n",
                        stride_index.llvm_value()
                    ));
                    emitted.push(stride_index);
                }
                format!(
                    "  {} = add i64 {}, {offset}\n",
                    index.llvm_value(),
                    stride_index.llvm_value()
                )
            }
        };
        lines.push_str(&line);
        emitted.push(index);
    }
    lines
}

fn emit_bounds_checks(abi: &KernelAbi, slice_sources: &[SliceElementSource]) -> String {
    slice_sources
        .iter()
        .map(|source| {
            let base = abi.args[source.arg_index].llvm_base();
            let suffix = source.index.llvm_suffix();
            let index = source.index.llvm_value();
            format!("  %in_{base}{suffix} = icmp ult i64 {index}, %{base}_len\n")
        })
        .collect::<String>()
}

fn emit_inbounds_reduce(slice_sources: &[SliceElementSource], abi: &KernelAbi) -> String {
    let check_names = slice_sources
        .iter()
        .map(|source| {
            format!(
                "%in_{}{}",
                abi.args[source.arg_index].llvm_base(),
                source.index.llvm_suffix()
            )
        })
        .collect::<Vec<_>>();

    match check_names.as_slice() {
        [] => "  %inbounds = and i1 true, true\n".to_string(),
        [only] => format!("  %inbounds = and i1 {only}, true\n"),
        [first, second] => format!("  %inbounds = and i1 {first}, {second}\n"),
        [first, second, rest @ ..] => {
            let mut lines = format!("  %inbounds_0 = and i1 {first}, {second}\n");
            let mut previous = "%inbounds_0".to_string();
            for (offset, name) in rest.iter().enumerate() {
                let current = if offset == rest.len() - 1 {
                    "%inbounds".to_string()
                } else {
                    format!("%inbounds_{}", offset + 1)
                };
                lines.push_str(&format!("  {current} = and i1 {previous}, {name}\n"));
                previous = current;
            }
            lines
        }
    }
}

fn emit_loads(
    abi: &KernelAbi,
    slice_sources: &[SliceElementSource],
    llvm_type: &str,
    align: usize,
) -> String {
    slice_sources
        .iter()
        .map(|source| {
            let base = abi.args[source.arg_index].llvm_base();
            let suffix = source.index.llvm_suffix();
            let index = source.index.llvm_value();
            format!(
                "  %{base}{suffix}_elem_ptr = getelementptr inbounds {llvm_type}, ptr addrspace(1) %{base}_ptr, i64 {index}\n  %{base}{suffix}_value = load {llvm_type}, ptr addrspace(1) %{base}{suffix}_elem_ptr, align {align}\n"
            )
        })
        .collect::<String>()
}

fn emit_expr_ops(expr: &ElementwiseExpr, abi: &KernelAbi, llvm_type: &str) -> String {
    expr.nodes
        .iter()
        .enumerate()
        .map(|(index, node)| match node {
            ExprNode::Binary { lhs, rhs, op } => {
                let lhs = expr_ref_value(*lhs, abi);
                let rhs = expr_ref_value(*rhs, abi);
                let op = op.llvm_opcode();
                format!("  %expr{index} = {op} {llvm_type} {lhs}, {rhs}\n")
            }
            ExprNode::Unary { operand, op } => {
                let operand = expr_ref_value(*operand, abi);
                let op = op.llvm_opcode();
                format!("  %expr{index} = {op} {llvm_type} {operand}\n")
            }
        })
        .collect::<String>()
}

fn expr_ref_value(expr: ExprRef, abi: &KernelAbi) -> String {
    match expr {
        ExprRef::Value(source) => source_value_expr(source, abi),
        ExprRef::Literal(literal) => literal.llvm_value(),
        ExprRef::Node(index) => format!("%expr{index}"),
    }
}

impl ScalarLiteral {
    fn llvm_value(self) -> String {
        let bits = match self {
            Self::F32(bits) => f64::from(f32::from_bits(bits)).to_bits(),
            Self::F64(bits) => bits,
        };
        format!("0x{bits:016X}")
    }
}

fn source_value_expr(source: ValueSource, abi: &KernelAbi) -> String {
    let base = abi.args[source.arg_index()].llvm_base();
    match source {
        ValueSource::Arg(_) => format!("%{base}"),
        ValueSource::SliceElement(source) => format!("%{base}{}_value", source.index.llvm_suffix()),
    }
}

fn unsupported_kernel(kernel: &str, reason: impl Into<String>) -> EmitError {
    EmitError::UnsupportedKernel {
        kernel: kernel.to_string(),
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_abi() -> KernelAbi {
        KernelAbi {
            name: "test_kernel".to_string(),
            args: vec![
                KernelArg {
                    name: "x".to_string(),
                    kind: KernelArgKind::Slice {
                        element: ScalarType::F32,
                        mutable: false,
                    },
                },
                KernelArg {
                    name: "out".to_string(),
                    kind: KernelArgKind::Slice {
                        element: ScalarType::F32,
                        mutable: true,
                    },
                },
            ],
        }
    }

    fn test_shape(output_index: IndexExpr) -> ElementwiseShape {
        test_shape_with_input(IndexExpr::Thread, output_index)
    }

    fn test_shape_with_input(input_index: IndexExpr, output_index: IndexExpr) -> ElementwiseShape {
        ElementwiseShape {
            expr: ElementwiseExpr {
                nodes: Vec::new(),
                root: ExprRef::Value(ValueSource::SliceElement(SliceElementSource {
                    arg_index: 0,
                    index: input_index,
                })),
            },
            output_arg: 1,
            output_index,
        }
    }

    fn binary_test_shape() -> ElementwiseShape {
        ElementwiseShape {
            expr: ElementwiseExpr {
                nodes: vec![ExprNode::Binary {
                    lhs: ExprRef::Value(ValueSource::SliceElement(SliceElementSource {
                        arg_index: 0,
                        index: IndexExpr::Thread,
                    })),
                    rhs: ExprRef::Value(ValueSource::SliceElement(SliceElementSource {
                        arg_index: 0,
                        index: IndexExpr::Thread,
                    })),
                    op: ElementwiseBinaryOp::Add,
                }],
                root: ExprRef::Node(0),
            },
            output_arg: 1,
            output_index: IndexExpr::Thread,
        }
    }

    fn test_record_function() -> RecordLoweringFunction {
        RecordLoweringFunction {
            symbol: "test_kernel".to_string(),
            kind: "kernel".to_string(),
            arg_count: 2,
            local_count: 3,
            block_count: 1,
            locals: vec![
                RecordLoweringLocal {
                    index: 0,
                    role: "return".to_string(),
                    ty: "mir.unit".to_string(),
                    rust_ty: "()".to_string(),
                },
                RecordLoweringLocal {
                    index: 1,
                    role: "arg".to_string(),
                    ty: "mir.slice".to_string(),
                    rust_ty: "&[f32]".to_string(),
                },
                RecordLoweringLocal {
                    index: 2,
                    role: "arg".to_string(),
                    ty: "mir.disjoint_slice".to_string(),
                    rust_ty: "fe2o3_device::DisjointSlice<f32>".to_string(),
                },
            ],
            ops: vec![
                record_call("fe2o3_device::thread::index_1d"),
                record_thread_get(4),
                record_load("local1.deref.index_local4"),
                record_store(),
                record_op(MirOp::Return),
            ],
        }
    }

    fn record_op(op: MirOp) -> crate::record_lowering::RecordLoweringOp {
        crate::record_lowering::RecordLoweringOp::new_for_test(op)
    }

    fn record_call(callee: &str) -> crate::record_lowering::RecordLoweringOp {
        let mut op = record_op(MirOp::Call);
        op.callee = Some(callee.to_string());
        op.set_trusted_callee_for_test(TrustedDeviceItem::ThreadIndex1d);
        op.destination_local = Some(3);
        op.destination = Some("local3".to_string());
        op
    }

    fn record_thread_get(destination: usize) -> crate::record_lowering::RecordLoweringOp {
        let mut op = record_op(MirOp::Call);
        op.callee = Some("fe2o3_device::ThreadIndex::get".to_string());
        op.set_trusted_callee_for_test(TrustedDeviceItem::ThreadIndexGet);
        op.destination_local = Some(destination);
        op.destination = Some(format!("local{destination}"));
        op.operand_count = Some(1);
        op.operands = Some("local3".to_string());
        op
    }

    fn record_load(operands: &str) -> crate::record_lowering::RecordLoweringOp {
        let mut op = record_op(MirOp::Load);
        op.statement = Some(0);
        op.operation = Some("use".to_string());
        op.destination_local = Some(5);
        op.destination = Some("local5".to_string());
        op.operand_count = Some(1);
        op.operands = Some(operands.to_string());
        op
    }

    fn record_store() -> crate::record_lowering::RecordLoweringOp {
        let mut op = record_op(MirOp::Store);
        op.statement = Some(0);
        op.operation = Some("use".to_string());
        op.destination = Some("local2.deref".to_string());
        op.operands = Some("local5".to_string());
        op
    }

    #[test]
    fn validates_matching_record_abi() {
        validate_record_abi("test_kernel", &test_abi(), &test_record_function()).unwrap();
    }

    #[test]
    fn rejects_mismatched_record_abi() {
        let mut record_function = test_record_function();
        record_function.locals[1].ty = "mir.f64".to_string();

        let error = validate_record_abi("test_kernel", &test_abi(), &record_function).unwrap_err();

        assert!(error.to_string().contains("does not match MIR ABI"));
    }

    #[test]
    fn validates_matching_record_elementwise_shape() {
        validate_record_elementwise_shape(
            "test_kernel",
            &test_shape(IndexExpr::Thread),
            &test_record_function(),
        )
        .unwrap();
    }

    #[test]
    fn builds_elementwise_expr_from_record_expression_sketch() {
        let shape = test_shape(IndexExpr::Thread);
        let expr = record_elementwise_expr(&shape, &test_record_function()).unwrap();

        assert_eq!(expr, shape.expr);
    }

    #[test]
    fn validates_record_arithmetic_on_store_operation() {
        let mut record_function = test_record_function();
        record_function.ops[3].operation = Some("add".to_string());
        record_function.ops[3].operands = Some("local5, local5".to_string());

        validate_record_elementwise_shape("test_kernel", &binary_test_shape(), &record_function)
            .unwrap();
    }

    #[test]
    fn builds_binary_elementwise_expr_from_record_expression_sketch() {
        let mut record_function = test_record_function();
        record_function.ops[3].operation = Some("add".to_string());
        record_function.ops[3].operands = Some("local5, local5".to_string());
        let shape = binary_test_shape();
        let expr = record_elementwise_expr(&shape, &record_function).unwrap();

        assert_eq!(expr, shape.expr);
    }

    #[test]
    fn rejects_record_shape_without_thread_index() {
        let mut record_function = test_record_function();
        record_function.ops.retain(|op| op.op != MirOp::Call);

        let error = validate_record_elementwise_shape(
            "test_kernel",
            &test_shape(IndexExpr::Thread),
            &record_function,
        )
        .unwrap_err();

        assert!(error.to_string().contains("thread::index_1d"));
    }

    #[test]
    fn rejects_record_shape_without_expected_source_load() {
        let mut record_function = test_record_function();
        record_function.ops[2].operands = Some("local9.deref.index_local4".to_string());

        let error = validate_record_elementwise_shape(
            "test_kernel",
            &test_shape(IndexExpr::Thread),
            &record_function,
        )
        .unwrap_err();

        assert!(error.to_string().contains("load on local1"));
    }

    #[test]
    fn rejects_shifted_record_shape_without_index_transform() {
        let error = validate_record_elementwise_shape(
            "test_kernel",
            &test_shape(IndexExpr::Offset(1)),
            &test_record_function(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("index transform"));
    }

    #[test]
    fn emits_derived_output_index_store() {
        let llvm = emit_elementwise_kernel(&test_abi(), &test_shape(IndexExpr::Offset(1))).unwrap();

        assert!(llvm.contains("  %idx_p1 = add i64 %idx, 1\n"));
        assert!(llvm.contains("  %in_out_p1 = icmp ult i64 %idx_p1, %out_len\n"));
        assert!(llvm.contains(
            "  %out_store_ptr = getelementptr inbounds float, ptr addrspace(1) %out_ptr, i64 %idx_p1\n"
        ));
    }

    #[test]
    fn rejects_non_unique_output_index() {
        let error =
            validate_elementwise_abi(&test_abi(), &test_shape(IndexExpr::Stride(0))).unwrap_err();

        assert!(matches!(error, EmitError::UnsupportedKernel { .. }));
        assert!(error.to_string().contains("unique per thread"));
    }

    #[test]
    fn emits_negative_stride_index_names() {
        let output_index = IndexExpr::StrideOffset {
            stride: -1,
            offset: 1023,
        };
        let llvm = emit_elementwise_kernel(&test_abi(), &test_shape(output_index)).unwrap();

        assert!(llvm.contains("  %idx_sm1 = mul i64 %idx, -1\n"));
        assert!(llvm.contains("  %idx_sm1_p1023 = add i64 %idx_sm1, 1023\n"));
        assert!(llvm.contains("  %in_out_sm1_p1023 = icmp ult i64 %idx_sm1_p1023, %out_len\n"));
        assert!(llvm.contains(
            "  %out_store_ptr = getelementptr inbounds float, ptr addrspace(1) %out_ptr, i64 %idx_sm1_p1023\n"
        ));
    }

    #[test]
    fn emits_zero_stride_read_index_names() {
        let input_index = IndexExpr::StrideOffset {
            stride: 0,
            offset: 1,
        };
        let llvm = emit_elementwise_kernel(
            &test_abi(),
            &test_shape_with_input(input_index, IndexExpr::Thread),
        )
        .unwrap();

        assert!(llvm.contains("  %idx_s0 = mul i64 %idx, 0\n"));
        assert!(llvm.contains("  %idx_s0_p1 = add i64 %idx_s0, 1\n"));
        assert!(llvm.contains("  %in_x_s0_p1 = icmp ult i64 %idx_s0_p1, %x_len\n"));
        assert!(llvm.contains(
            "  %x_s0_p1_elem_ptr = getelementptr inbounds float, ptr addrspace(1) %x_ptr, i64 %idx_s0_p1\n"
        ));
    }

    #[test]
    fn combines_non_constant_index_operands() {
        assert_eq!(
            IndexExpr::Thread.add_index(IndexExpr::Thread),
            Some(IndexExpr::Stride(2))
        );
        assert_eq!(
            IndexExpr::Offset(1).add_index(IndexExpr::Thread),
            Some(IndexExpr::StrideOffset {
                stride: 2,
                offset: 1,
            })
        );
        assert_eq!(
            IndexExpr::Offset(1).sub_index(IndexExpr::Thread),
            Some(IndexExpr::StrideOffset {
                stride: 0,
                offset: 1,
            })
        );
        assert_eq!(
            IndexExpr::constant(1023).and_then(|index| index.sub_index(IndexExpr::Thread)),
            Some(IndexExpr::StrideOffset {
                stride: -1,
                offset: 1023,
            })
        );
    }
}
