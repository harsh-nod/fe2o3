use crate::collector::{CollectedFunction, CollectionResult};
use crate::{AmdGpuTarget, HsacoError, compile_llvm_ir_to_hsaco};
use rustc_middle::ty::{EarlyBinder, FloatTy, Mutability, Ty, TyCtxt, TyKind, TypingEnv};
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
    let abi = analyze_kernel_abi(tcx, kernel)?;

    match abi.name.as_str() {
        "vecadd" => emit_vecadd_kernel(&abi),
        other => Err(unsupported_kernel(
            other,
            "only the vecadd MIR shape has an LLVM IR template today",
        )),
    }
}

#[derive(Clone, Debug)]
struct KernelAbi {
    name: String,
    args: Vec<KernelArg>,
}

#[derive(Clone, Debug)]
struct KernelArg {
    source_index: usize,
    kind: KernelArgKind,
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
    fn llvm_base(&self) -> String {
        format!("arg{}", self.source_index)
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

        args.push(KernelArg { source_index, kind });
    }

    Ok(KernelAbi {
        name: kernel.export_name.clone(),
        args,
    })
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

fn emit_vecadd_kernel(abi: &KernelAbi) -> Result<String, EmitError> {
    validate_vecadd_abi(abi)?;

    let params = abi
        .args
        .iter()
        .flat_map(KernelArg::llvm_params)
        .collect::<Vec<_>>()
        .join(", ");
    let a = &abi.args[0];
    let b = &abi.args[1];
    let c = &abi.args[2];
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

fn validate_vecadd_abi(abi: &KernelAbi) -> Result<(), EmitError> {
    if abi.args.len() != 3 {
        return Err(unsupported_kernel(
            &abi.name,
            "vecadd requires exactly three slice-like arguments",
        ));
    }

    if abi.args.iter().any(|arg| arg.element() != ScalarType::F32) {
        return Err(unsupported_kernel(
            &abi.name,
            "vecadd currently supports only f32 slice arguments",
        ));
    }

    if abi.args[0].is_mutable() || abi.args[1].is_mutable() || !abi.args[2].is_mutable() {
        return Err(unsupported_kernel(
            &abi.name,
            "vecadd expects two read-only input slices and one mutable output slice",
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
