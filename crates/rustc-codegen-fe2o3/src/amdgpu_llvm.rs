use crate::collector::{CollectedFunction, CollectionResult};
use crate::{AmdGpuTarget, HsacoError, compile_llvm_ir_to_hsaco};
use rustc_middle::mir::{
    BinOp, Body, ConstOperand, Local, Operand, Place, ProjectionElem, Rvalue, StatementKind,
    TerminatorKind, UnOp, VarDebugInfoContents,
};
use rustc_middle::ty::{EarlyBinder, FloatTy, Mutability, Ty, TyCtxt, TyKind, TypingEnv};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct DeviceArtifact {
    pub kernel_name: String,
    pub llvm_ir_path: PathBuf,
    pub hsaco_path: PathBuf,
}

#[derive(Debug)]
pub enum EmitError {
    Io(std::io::Error),
    Hsaco(HsacoError),
    UnsupportedKernel { kernel: String, reason: String },
}

impl fmt::Display for EmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Hsaco(error) => write!(f, "{error}"),
            Self::UnsupportedKernel { kernel, reason } => {
                write!(
                    f,
                    "unsupported kernel shape for AMDGPU LLVM IR MVP: {kernel}: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for EmitError {}

impl From<std::io::Error> for EmitError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<HsacoError> for EmitError {
    fn from(error: HsacoError) -> Self {
        Self::Hsaco(error)
    }
}

pub fn emit_collection<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    output_dir: &Path,
    target: &AmdGpuTarget,
) -> Result<Vec<DeviceArtifact>, EmitError> {
    std::fs::create_dir_all(output_dir)?;

    let mut artifacts = Vec::new();
    for kernel in collection
        .functions
        .iter()
        .filter(|function| function.is_kernel)
    {
        let llvm_ir = emit_kernel(tcx, kernel)?;
        let llvm_ir_path = output_dir.join(format!("{}.ll", kernel.export_name));
        let hsaco_path = output_dir.join(format!("{}.hsaco", kernel.export_name));

        std::fs::write(&llvm_ir_path, llvm_ir)?;
        compile_llvm_ir_to_hsaco(&llvm_ir_path, &hsaco_path, target)?;

        artifacts.push(DeviceArtifact {
            kernel_name: kernel.export_name.clone(),
            llvm_ir_path,
            hsaco_path,
        });
    }

    Ok(artifacts)
}

fn emit_kernel<'tcx>(
    tcx: TyCtxt<'tcx>,
    kernel: &CollectedFunction<'tcx>,
) -> Result<String, EmitError> {
    let mir = tcx.instance_mir(kernel.instance.def);
    let abi = analyze_kernel_abi(tcx, kernel)?;
    let elementwise = analyze_elementwise_shape(tcx, mir).map_err(|reason| {
        unsupported_kernel(
            &kernel.export_name,
            format!("unsupported MIR body for elementwise lowering: {reason}"),
        )
    })?;

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

#[derive(Clone, Debug)]
struct ElementwiseShape {
    expr: ElementwiseExpr,
    output_arg: usize,
}

#[derive(Clone, Debug)]
struct ElementwiseExpr {
    nodes: Vec<ExprNode>,
    root: ExprRef,
}

#[derive(Clone, Debug)]
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
    SliceElement(usize),
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
    Literal(FloatLiteral),
    Node(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FloatLiteral {
    bits: u32,
}

#[derive(Clone, Debug)]
enum KernelArgKind {
    Scalar(ScalarType),
    Slice { element: ScalarType, mutable: bool },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarType {
    F32,
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
        }
    }

    fn llvm_align(self) -> usize {
        match self {
            Self::F32 => 4,
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
        _ => Err("expected `f32`, `&[T]`, `&mut [T]`, or `DisjointSlice<T>`"),
    }
}

fn classify_scalar(ty: Ty<'_>) -> Result<ScalarType, &'static str> {
    match ty.kind() {
        TyKind::Float(FloatTy::F32) => Ok(ScalarType::F32),
        _ => Err("only `f32` elements are supported"),
    }
}

fn is_disjoint_slice(tcx: TyCtxt<'_>, def_id: rustc_hir::def_id::DefId) -> bool {
    tcx.def_path_str(def_id)
        .ends_with("fe2o3_device::DisjointSlice")
}

fn analyze_elementwise_shape<'tcx>(
    tcx: TyCtxt<'tcx>,
    mir: &Body<'tcx>,
) -> Result<ElementwiseShape, String> {
    let arg_locals = mir.args_iter().collect::<Vec<_>>();
    let mut borrowed_args = HashMap::new();

    for block in mir.basic_blocks.iter() {
        for statement in &block.statements {
            let StatementKind::Assign(assign) = &statement.kind else {
                continue;
            };
            let (place, rvalue) = &**assign;
            if let Rvalue::Ref(_, _, borrowed_place) = rvalue
                && place.projection.is_empty()
                && let Some(arg_index) = local_arg_index(borrowed_place.local, &arg_locals)
            {
                borrowed_args.insert(place.local, arg_index);
            }
        }
    }

    let mut thread_index_local = None;
    let mut index_value_locals = HashSet::new();
    let mut output_arg = None;

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
        let path = tcx.def_path_str(def_id);

        if path.ends_with("fe2o3_device::thread::index_1d") {
            if destination.projection.is_empty() {
                thread_index_local = Some(destination.local);
            }
            continue;
        }

        if path.ends_with("fe2o3_device::ThreadIndex::get") {
            let Some(thread_index_local) = thread_index_local else {
                continue;
            };
            if destination.projection.is_empty()
                && args
                    .first()
                    .and_then(|arg| operand_local(&arg.node))
                    .is_some_and(|local| local == thread_index_local)
            {
                index_value_locals.insert(destination.local);
            }
            continue;
        }

        if path.ends_with("::get_mut") && path.contains("DisjointSlice") {
            if args
                .get(1)
                .and_then(|arg| operand_local(&arg.node))
                .is_some_and(|local| Some(local) == thread_index_local)
                && let Some(receiver_local) = args.first().and_then(|arg| operand_local(&arg.node))
                && let Some(arg_index) = borrowed_args.get(&receiver_local).copied()
            {
                output_arg = Some(arg_index);
            }
        }
    }

    thread_index_local.ok_or_else(|| "missing `thread::index_1d` call".to_string())?;
    if index_value_locals.is_empty() {
        return Err("missing `ThreadIndex::get` calls for slice indexing".to_string());
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
                        && let Some(arg_index) =
                            indexed_arg_place(source_place, &arg_locals, &index_value_locals)
                    {
                        Some(ExprRef::Value(ValueSource::SliceElement(arg_index)))
                    } else {
                        operand_expr(tcx, operand, &local_exprs, &arg_locals)
                    };

                    let Some(expr) = expr else {
                        continue;
                    };

                    if place.projection.is_empty() {
                        local_exprs.insert(place.local, expr);
                    } else if let Some(arg_index) =
                        store_output_arg(place, output_arg, &arg_locals, &index_value_locals)
                    {
                        output_arg = Some(arg_index);
                        elementwise_store = Some(expr);
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
                        } else if let Some(arg_index) =
                            store_output_arg(place, output_arg, &arg_locals, &index_value_locals)
                        {
                            output_arg = Some(arg_index);
                            elementwise_store = Some(expr);
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
                    } else if let Some(arg_index) =
                        store_output_arg(place, output_arg, &arg_locals, &index_value_locals)
                    {
                        output_arg = Some(arg_index);
                        elementwise_store = Some(expr);
                    }
                }
                _ => {}
            }
        }
    }

    let root = elementwise_store
        .ok_or_else(|| "missing `output[idx] = <elementwise expression>` store".to_string())?;
    let output_arg = output_arg.ok_or_else(|| {
        "missing mutable slice or `DisjointSlice::get_mut` output path".to_string()
    })?;
    let expr = ElementwiseExpr {
        nodes: expr_nodes,
        root,
    };

    Ok(ElementwiseShape { expr, output_arg })
}

impl ValueSource {
    fn arg_index(self) -> usize {
        match self {
            Self::Arg(index) | Self::SliceElement(index) => index,
        }
    }
}

fn operand_local(operand: &Operand<'_>) -> Option<Local> {
    let place = operand.place()?;
    place.projection.is_empty().then_some(place.local)
}

fn local_arg_index(local: Local, arg_locals: &[Local]) -> Option<usize> {
    arg_locals.iter().position(|candidate| *candidate == local)
}

fn indexed_arg_place(
    place: Place<'_>,
    arg_locals: &[Local],
    index_value_locals: &HashSet<Local>,
) -> Option<usize> {
    let [ProjectionElem::Deref, ProjectionElem::Index(index_local)] = &place.projection[..] else {
        return None;
    };
    if !index_value_locals.contains(index_local) {
        return None;
    }
    local_arg_index(place.local, arg_locals)
}

fn store_output_arg(
    place: &Place<'_>,
    disjoint_output_arg: Option<usize>,
    arg_locals: &[Local],
    index_value_locals: &HashSet<Local>,
) -> Option<usize> {
    if is_deref_store(place) {
        return disjoint_output_arg;
    }

    indexed_arg_place(*place, arg_locals, index_value_locals)
}

fn operand_expr<'tcx>(
    tcx: TyCtxt<'tcx>,
    operand: &Operand<'tcx>,
    local_exprs: &HashMap<Local, ExprRef>,
    arg_locals: &[Local],
) -> Option<ExprRef> {
    if let Operand::Constant(constant) = operand {
        return constant_f32(tcx, constant).map(ExprRef::Literal);
    }

    let local = operand_local(operand)?;
    local_exprs.get(&local).copied().or_else(|| {
        local_arg_index(local, arg_locals).map(|arg| ExprRef::Value(ValueSource::Arg(arg)))
    })
}

fn constant_f32<'tcx>(tcx: TyCtxt<'tcx>, constant: &ConstOperand<'tcx>) -> Option<FloatLiteral> {
    if !matches!(constant.const_.ty().kind(), TyKind::Float(FloatTy::F32)) {
        return None;
    }

    let bits = constant
        .const_
        .try_eval_scalar_int(tcx, TypingEnv::fully_monomorphized())?
        .to_u32();
    Some(FloatLiteral { bits })
}

fn is_deref_store(place: &Place<'_>) -> bool {
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
    let check_lines = emit_bounds_checks(abi, &slice_sources);
    let inbounds = emit_inbounds_reduce(&slice_sources, abi);
    let load_lines = emit_loads(abi, &read_slice_sources, llvm_type, align);
    let compute_lines = emit_expr_ops(&shape.expr, abi, llvm_type);
    let result_value = expr_ref_value(shape.expr.root, abi);

    // This is the first AMDGPU LLVM IR milestone, intentionally scoped to
    // simple f32 elementwise expression kernels. It matches
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
{check_lines}{inbounds}
  br i1 %inbounds, label %body, label %exit

body:
{load_lines}{compute_lines}
  %{c_base}_store_ptr = getelementptr inbounds {llvm_type}, ptr addrspace(1) %{c_base}_ptr, i64 %idx
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
        inbounds = inbounds,
        load_lines = load_lines,
        c_base = c_base,
        llvm_type = llvm_type,
        align = align,
        compute_lines = compute_lines,
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

    if abi.args.iter().any(|arg| arg.element() != ScalarType::F32) {
        return Err(unsupported_kernel(
            &abi.name,
            "elementwise lowering currently supports only f32 arguments",
        ));
    }

    for source in sources {
        let arg = &abi.args[source.arg_index()];
        match source {
            ValueSource::Arg(_) if !arg.is_scalar() => {
                return Err(unsupported_kernel(
                    &abi.name,
                    "direct elementwise operands must be scalar f32 arguments",
                ));
            }
            ValueSource::SliceElement(_) if !arg.is_slice() => {
                return Err(unsupported_kernel(
                    &abi.name,
                    "indexed elementwise operands must be f32 slices",
                ));
            }
            ValueSource::SliceElement(index) if arg.is_mutable() && index != shape.output_arg => {
                return Err(unsupported_kernel(
                    &abi.name,
                    "mutable indexed elementwise operands must be the output slice",
                ));
            }
            _ => {}
        }
    }

    if !abi.args[shape.output_arg].is_slice() || !abi.args[shape.output_arg].is_mutable() {
        return Err(unsupported_kernel(
            &abi.name,
            "elementwise lowering expects one mutable output slice",
        ));
    }

    Ok(())
}

fn read_slice_sources(shape: &ElementwiseShape) -> Vec<usize> {
    let mut sources = Vec::new();
    for source in shape.expr.sources() {
        if let ValueSource::SliceElement(index) = source
            && !sources.contains(&index)
        {
            sources.push(index);
        }
    }
    sources
}

fn bounds_slice_sources(shape: &ElementwiseShape) -> Vec<usize> {
    let mut sources = read_slice_sources(shape);
    if !sources.contains(&shape.output_arg) {
        sources.push(shape.output_arg);
    }
    sources
}

fn emit_bounds_checks(abi: &KernelAbi, slice_sources: &[usize]) -> String {
    slice_sources
        .iter()
        .map(|index| {
            let base = abi.args[*index].llvm_base();
            format!("  %in_{base} = icmp ult i64 %idx, %{base}_len\n")
        })
        .collect::<String>()
}

fn emit_inbounds_reduce(slice_sources: &[usize], abi: &KernelAbi) -> String {
    let check_names = slice_sources
        .iter()
        .map(|index| format!("%in_{}", abi.args[*index].llvm_base()))
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

fn emit_loads(abi: &KernelAbi, slice_sources: &[usize], llvm_type: &str, align: usize) -> String {
    slice_sources
        .iter()
        .map(|index| {
            let base = abi.args[*index].llvm_base();
            format!(
                "  %{base}_elem_ptr = getelementptr inbounds {llvm_type}, ptr addrspace(1) %{base}_ptr, i64 %idx\n  %{base}_value = load {llvm_type}, ptr addrspace(1) %{base}_elem_ptr, align {align}\n"
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

impl FloatLiteral {
    fn llvm_value(self) -> String {
        let value = f32::from_bits(self.bits);
        let double_bits = f64::from(value).to_bits();
        format!("0x{double_bits:016X}")
    }
}

fn source_value_expr(source: ValueSource, abi: &KernelAbi) -> String {
    let base = abi.args[source.arg_index()].llvm_base();
    match source {
        ValueSource::Arg(_) => format!("%{base}"),
        ValueSource::SliceElement(_) => format!("%{base}_value"),
    }
}

fn unsupported_kernel(kernel: &str, reason: impl Into<String>) -> EmitError {
    EmitError::UnsupportedKernel {
        kernel: kernel.to_string(),
        reason: reason.into(),
    }
}
