use crate::collector::{CollectedFunction, CollectionResult};
use crate::{AmdGpuTarget, HsacoError, compile_llvm_ir_to_hsaco};
use rustc_middle::mir::{
    BinOp, Body, Local, Operand, Place, ProjectionElem, Rvalue, StatementKind, TerminatorKind,
    VarDebugInfoContents,
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
    let vector_add = analyze_vector_add_shape(tcx, mir).map_err(|reason| {
        unsupported_kernel(
            &kernel.export_name,
            format!("unsupported MIR body for vector-add lowering: {reason}"),
        )
    })?;

    emit_vector_add_kernel(&abi, &vector_add)
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
struct VectorAddShape {
    lhs_arg: usize,
    rhs_arg: usize,
    output_arg: usize,
}

#[derive(Clone, Debug)]
enum KernelArgKind {
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
            KernelArgKind::Slice { element, .. } => element,
        }
    }

    fn is_mutable(&self) -> bool {
        match self.kind {
            KernelArgKind::Slice { mutable, .. } => mutable,
        }
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
        _ => Err("expected `&[T]`, `&mut [T]`, or `DisjointSlice<T>`"),
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

fn analyze_vector_add_shape<'tcx>(
    tcx: TyCtxt<'tcx>,
    mir: &Body<'tcx>,
) -> Result<VectorAddShape, String> {
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
    let output_arg = output_arg
        .ok_or_else(|| "missing `DisjointSlice::get_mut(thread_index)` output path".to_string())?;

    let mut loaded_values = HashMap::new();
    let mut add_store = None;

    for block in mir.basic_blocks.iter() {
        for statement in &block.statements {
            let StatementKind::Assign(assign) = &statement.kind else {
                continue;
            };
            let (place, rvalue) = &**assign;

            match rvalue {
                Rvalue::Use(operand) if place.projection.is_empty() => {
                    if let Some(source_place) = operand.place()
                        && let Some(arg_index) =
                            indexed_arg_place(source_place, &arg_locals, &index_value_locals)
                    {
                        loaded_values.insert(place.local, arg_index);
                    }
                }
                Rvalue::BinaryOp(BinOp::Add, operands) if is_deref_store(place) => {
                    let lhs_arg = operand_local(&operands.0)
                        .and_then(|local| loaded_values.get(&local).copied());
                    let rhs_arg = operand_local(&operands.1)
                        .and_then(|local| loaded_values.get(&local).copied());

                    if let (Some(lhs_arg), Some(rhs_arg)) = (lhs_arg, rhs_arg) {
                        add_store = Some(VectorAddShape {
                            lhs_arg,
                            rhs_arg,
                            output_arg,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    let shape = add_store
        .ok_or_else(|| "missing `output[idx] = input_a[idx] + input_b[idx]` store".to_string())?;
    if shape.lhs_arg == shape.rhs_arg {
        return Err("vector add inputs resolve to the same slice argument".to_string());
    }
    if shape.lhs_arg == shape.output_arg || shape.rhs_arg == shape.output_arg {
        return Err("vector add output aliases an input argument in MIR".to_string());
    }
    Ok(shape)
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

fn is_deref_store(place: &Place<'_>) -> bool {
    matches!(&place.projection[..], [ProjectionElem::Deref])
}

fn emit_vector_add_kernel(abi: &KernelAbi, shape: &VectorAddShape) -> Result<String, EmitError> {
    validate_vector_add_abi(abi, shape)?;

    let params = abi
        .args
        .iter()
        .flat_map(KernelArg::llvm_params)
        .collect::<Vec<_>>()
        .join(", ");
    let a = &abi.args[shape.lhs_arg];
    let b = &abi.args[shape.rhs_arg];
    let c = &abi.args[shape.output_arg];
    let a_base = a.llvm_base();
    let b_base = b.llvm_base();
    let c_base = c.llvm_base();
    let element = c.element();
    let llvm_type = element.llvm_type();
    let align = element.llvm_align();

    // This is the first AMDGPU LLVM IR milestone, intentionally scoped to the
    // vecadd example. It matches `LaunchConfig::for_num_elems`, which uses a
    // 256-thread block. General block_dim lowering will read the AMD dispatch
    // packet instead of hard-coding this constant.
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
  %in_{a_base} = icmp ult i64 %idx, %{a_base}_len
  %in_{b_base} = icmp ult i64 %idx, %{b_base}_len
  %in_{c_base} = icmp ult i64 %idx, %{c_base}_len
  %in_inputs = and i1 %in_{a_base}, %in_{b_base}
  %inbounds = and i1 %in_inputs, %in_{c_base}
  br i1 %inbounds, label %body, label %exit

body:
  %{a_base}_elem_ptr = getelementptr inbounds {llvm_type}, ptr addrspace(1) %{a_base}_ptr, i64 %idx
  %{b_base}_elem_ptr = getelementptr inbounds {llvm_type}, ptr addrspace(1) %{b_base}_ptr, i64 %idx
  %{c_base}_elem_ptr = getelementptr inbounds {llvm_type}, ptr addrspace(1) %{c_base}_ptr, i64 %idx
  %{a_base}_value = load {llvm_type}, ptr addrspace(1) %{a_base}_elem_ptr, align {align}
  %{b_base}_value = load {llvm_type}, ptr addrspace(1) %{b_base}_elem_ptr, align {align}
  %sum = fadd {llvm_type} %{a_base}_value, %{b_base}_value
  store {llvm_type} %sum, ptr addrspace(1) %{c_base}_elem_ptr, align {align}
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
        a_base = a_base,
        b_base = b_base,
        c_base = c_base,
        llvm_type = llvm_type,
        align = align,
    ))
}

fn validate_vector_add_abi(abi: &KernelAbi, shape: &VectorAddShape) -> Result<(), EmitError> {
    if abi.args.len() != 3 {
        return Err(unsupported_kernel(
            &abi.name,
            "vector-add lowering requires exactly three slice-like arguments",
        ));
    }

    for arg_index in [shape.lhs_arg, shape.rhs_arg, shape.output_arg] {
        if arg_index >= abi.args.len() {
            return Err(unsupported_kernel(
                &abi.name,
                "MIR vector-add argument index is outside the kernel ABI",
            ));
        }
    }

    if abi.args.iter().any(|arg| arg.element() != ScalarType::F32) {
        return Err(unsupported_kernel(
            &abi.name,
            "vector-add lowering currently supports only f32 slice arguments",
        ));
    }

    if abi.args[shape.lhs_arg].is_mutable()
        || abi.args[shape.rhs_arg].is_mutable()
        || !abi.args[shape.output_arg].is_mutable()
    {
        return Err(unsupported_kernel(
            &abi.name,
            "vector-add lowering expects two read-only input slices and one mutable output slice",
        ));
    }

    Ok(())
}

fn unsupported_kernel(kernel: &str, reason: impl Into<String>) -> EmitError {
    EmitError::UnsupportedKernel {
        kernel: kernel.to_string(),
        reason: reason.into(),
    }
}
