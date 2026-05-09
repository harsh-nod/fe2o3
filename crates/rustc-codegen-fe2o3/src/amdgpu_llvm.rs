use crate::collector::CollectionResult;
use crate::{AmdGpuTarget, HsacoError, compile_llvm_ir_to_hsaco};
use rustc_middle::ty::TyCtxt;
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
    UnsupportedKernel(String),
}

impl fmt::Display for EmitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Hsaco(error) => write!(f, "{error}"),
            Self::UnsupportedKernel(kernel) => {
                write!(
                    f,
                    "unsupported kernel shape for AMDGPU LLVM IR MVP: {kernel}"
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
        let llvm_ir = emit_kernel(tcx, collection, &kernel.export_name)?;
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
    _tcx: TyCtxt<'tcx>,
    _collection: &CollectionResult<'tcx>,
    kernel_name: &str,
) -> Result<String, EmitError> {
    match kernel_name {
        "vecadd" => Ok(emit_vecadd_kernel(kernel_name)),
        other => Err(EmitError::UnsupportedKernel(other.to_string())),
    }
}

fn emit_vecadd_kernel(kernel_name: &str) -> String {
    // This is the first AMDGPU LLVM IR milestone, intentionally scoped to the
    // vecadd example. It matches `LaunchConfig::for_num_elems`, which uses a
    // 256-thread block. General block_dim lowering will read the AMD dispatch
    // packet instead of hard-coding this constant.
    format!(
        r#"target triple = "{triple}"

declare i32 @llvm.amdgcn.workitem.id.x() #1
declare i32 @llvm.amdgcn.workgroup.id.x() #1

define amdgpu_kernel void @{kernel_name}(ptr addrspace(1) %a_ptr, i64 %a_len, ptr addrspace(1) %b_ptr, i64 %b_len, ptr addrspace(1) %c_ptr, i64 %c_len) #0 {{
entry:
  %tid = call i32 @llvm.amdgcn.workitem.id.x()
  %bid = call i32 @llvm.amdgcn.workgroup.id.x()
  %base = mul i32 %bid, 256
  %idx32 = add i32 %base, %tid
  %idx = zext i32 %idx32 to i64
  %inbounds = icmp ult i64 %idx, %c_len
  br i1 %inbounds, label %body, label %exit

body:
  %a_elem_ptr = getelementptr inbounds float, ptr addrspace(1) %a_ptr, i64 %idx
  %b_elem_ptr = getelementptr inbounds float, ptr addrspace(1) %b_ptr, i64 %idx
  %c_elem_ptr = getelementptr inbounds float, ptr addrspace(1) %c_ptr, i64 %idx
  %a = load float, ptr addrspace(1) %a_elem_ptr, align 4
  %b = load float, ptr addrspace(1) %b_elem_ptr, align 4
  %sum = fadd float %a, %b
  store float %sum, ptr addrspace(1) %c_elem_ptr, align 4
  br label %exit

exit:
  ret void
}}

attributes #0 = {{ nounwind }}
attributes #1 = {{ nounwind readnone speculatable }}
"#,
        triple = dialect_amdgcn::AMDGPU_TRIPLE,
        kernel_name = kernel_name,
    )
}
