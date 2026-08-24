use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use fe2o3_amdgcn_model::{
    GFX942_XNACK_MINUS_DATA_LAYOUT, TiledGemmLdsGridLoweringErrorV1,
    lower_kernel_to_gfx942_xnack_minus_llvm_ir, lower_tiled_gemm_lds_grid_v1_to_gfx942_llvm_ir,
};
use fe2o3_kernel_ir::{
    Axis, Constant, IndexKind, IntrinsicKind, LaunchDomain, LaunchExtent, MatrixOperationKind,
    Operation, OperationKind, TILED_GEMM_LDS_GRID_V1_KERNEL_ID,
    TILED_GEMM_LDS_GRID_V1_STATIC_LDS_BYTES, TargetCapability, TiledGemmLdsGridV1Error,
    TiledGemmLdsGridV1Profile, ValueId, WaveWidth, WorkgroupMemoryExtent, WorkgroupSize,
    tiled_gemm_lds_grid_v1_module,
};

fn profile() -> TiledGemmLdsGridV1Profile {
    TiledGemmLdsGridV1Profile::exact_gfx942_xnack_minus_cov6()
}

fn operations_mut(module: &mut fe2o3_kernel_ir::Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}

fn assert_ir_rejected(module: &fe2o3_kernel_ir::Module) {
    assert!(matches!(
        lower_tiled_gemm_lds_grid_v1_to_gfx942_llvm_ir(module, profile()),
        Err(TiledGemmLdsGridLoweringErrorV1::Profile(
            TiledGemmLdsGridV1Error::InvalidKernelIr(_)
                | TiledGemmLdsGridV1Error::NonCanonicalKernelIr
        ))
    ));
}

#[test]
fn lowers_only_the_exact_padded_grid_graph_to_strict_gfx942_llvm() {
    let expected_profile = profile();
    let output = lower_tiled_gemm_lds_grid_v1_to_gfx942_llvm_ir(
        &tiled_gemm_lds_grid_v1_module(),
        expected_profile.clone(),
    )
    .expect("canonical tiled GEMM LDS grid LLVM");
    assert_eq!(output.profile(), &expected_profile);
    let llvm = output.as_str();

    assert!(llvm.contains(GFX942_XNACK_MINUS_DATA_LAYOUT), "{llvm}");
    for required in [
        "target triple = \"amdgcn-amd-amdhsa\"",
        "define amdgpu_kernel void @tiled_gemm_lds_grid_v1(",
        "ptr addrspace(1) %arg0.data, i64 %arg0.len",
        "ptr addrspace(1) %arg1.data, i64 %arg1.len",
        "ptr addrspace(1) %arg2.data, i64 %arg2.len",
        "call i32 @llvm.amdgcn.workgroup.id.x()",
        "call i32 @llvm.amdgcn.workgroup.id.y()",
        "%v16 = mul i64 %v9, 16",
        "%v17 = mul i64 %v10, 16",
        "\"target-cpu\"=\"gfx942\"",
        "\"target-features\"=\"-wavefrontsize32,+wavefrontsize64,-xnack\"",
        "\"amdgpu-flat-work-group-size\"=\"64,64\"",
        "\"fp-contract\"=\"off\"",
        "!0 = !{i32 64, i32 1, i32 1}",
        "fence syncscope(\"workgroup\") release",
        "call void asm sideeffect \"s_barrier\", \"\"()",
        "fence syncscope(\"workgroup\") acquire",
        "call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k(",
    ] {
        assert!(llvm.contains(required), "missing {required:?}\n{llvm}");
    }

    let lds_tiles = llvm
        .lines()
        .filter(|line| {
            line.starts_with('@')
                && line.contains("internal addrspace(3) global [256 x i16] undef, align 16")
        })
        .collect::<Vec<_>>();
    assert_eq!(lds_tiles.len(), 2, "{llvm}");
    assert_ne!(lds_tiles[0], lds_tiles[1], "{llvm}");
    assert_eq!(llvm.matches(" = load i16, ptr addrspace(1)").count(), 8);
    assert_eq!(llvm.matches("store i16 ").count(), 8);
    assert_eq!(llvm.matches(" = load i16, ptr addrspace(3)").count(), 8);
    assert_eq!(llvm.matches(" = xor i32 ").count(), 16);
    assert_eq!(llvm.matches("store float ").count(), 4);
    assert_eq!(
        llvm.matches("call void asm sideeffect \"s_barrier\", \"\"()")
            .count(),
        1
    );
    assert_eq!(
        llvm.matches("call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k(")
            .count(),
        1
    );
    for (stride, count) in [(33, 1), (79, 4), (96, 4)] {
        let suffix = format!(", {stride}");
        assert_eq!(
            llvm.lines()
                .filter(|line| line.contains(" = mul i64 ") && line.ends_with(&suffix))
                .count(),
            count,
            "stride {stride}\n{llvm}"
        );
    }
    for forbidden in [
        " fast ",
        " contract ",
        "atomicrmw",
        "cmpxchg",
        "alloca",
        "addrspace(5)",
        "@__ocml_",
        "@__ockl_",
        "COMGR",
        "comgr",
    ] {
        assert!(!llvm.contains(forbidden), "found {forbidden:?}\n{llvm}");
    }
}

#[test]
fn rejects_every_nonexact_grid_profile_before_lowering() {
    let mut mutations = Vec::new();
    macro_rules! mutated {
        ($field:ident, $value:expr) => {{
            let mut candidate = profile();
            candidate.$field = $value;
            mutations.push(candidate);
        }};
    }

    mutated!(target, TargetCapability::WaveWidth(WaveWidth::Wave64));
    mutated!(code_object_version, 5);
    mutated!(m, 48);
    mutated!(n, 64);
    mutated!(k, 32);
    mutated!(lda, 32);
    mutated!(ldb, 78);
    mutated!(ldc, 95);
    mutated!(a_elements, 2_094);
    mutated!(b_elements, 1_232);
    mutated!(c_elements, 6_095);
    mutated!(a_bytes, 4_188);
    mutated!(b_bytes, 2_464);
    mutated!(c_bytes, 24_380);
    mutated!(tile_rows, 3);
    mutated!(tile_columns, 4);
    mutated!(depth_tiles, 2);
    mutated!(workgroup_count, 11);
    mutated!(wave_width, WaveWidth::Wave32);
    mutated!(launch_extent_x, 128);
    mutated!(launch_extent_y, 3);
    mutated!(workgroup_size, WorkgroupSize::new(32, 2, 1));
    mutated!(lds_allocations, 1);
    mutated!(lds_elements_per_allocation, 512);
    mutated!(lds_bytes_per_allocation, 1_024);
    mutated!(static_lds_bytes, 512);
    mutated!(lds_alignment, 32);
    mutated!(output_elements_per_lane, 8);

    assert_eq!(mutations.len(), 28);
    for mutation in mutations {
        assert!(matches!(
            lower_tiled_gemm_lds_grid_v1_to_gfx942_llvm_ir(
                &tiled_gemm_lds_grid_v1_module(),
                mutation
            ),
            Err(TiledGemmLdsGridLoweringErrorV1::Profile(
                TiledGemmLdsGridV1Error::UnsupportedProfile
            ))
        ));
    }
}

#[test]
fn rejects_group_stride_resource_layout_mfma_and_store_drift() {
    let canonical = tiled_gemm_lds_grid_v1_module();

    let mut wrong_group_axis = canonical.clone();
    let group_y = operations_mut(&mut wrong_group_axis)
        .iter_mut()
        .find_map(|operation| match &mut operation.kind {
            OperationKind::Intrinsic(intrinsic)
                if matches!(
                    intrinsic.kind,
                    IntrinsicKind::InvocationIndex {
                        kind: IndexKind::Workgroup,
                        axis: Axis::Y,
                    }
                ) =>
            {
                Some(intrinsic)
            }
            _ => None,
        })
        .unwrap();
    group_y.kind = IntrinsicKind::InvocationIndex {
        kind: IndexKind::Workgroup,
        axis: Axis::X,
    };
    assert_ir_rejected(&wrong_group_axis);

    let mut wrong_launch = canonical.clone();
    wrong_launch.kernels[0].domain = LaunchDomain::D2 {
        x: LaunchExtent::Static(128),
        y: LaunchExtent::Static(4),
    };
    assert_ir_rejected(&wrong_launch);

    for stride in [33_u64, 79, 96] {
        let mut wrong_stride = canonical.clone();
        let constant = operations_mut(&mut wrong_stride)
            .iter_mut()
            .find_map(|operation| match &mut operation.kind {
                OperationKind::Constant(Constant::Index(value)) if *value == stride => Some(value),
                _ => None,
            })
            .unwrap();
        *constant += 1;
        assert_ir_rejected(&wrong_stride);
    }

    let mut wrong_extent = canonical.clone();
    let memory = operations_mut(&mut wrong_extent)
        .iter_mut()
        .find_map(|operation| match &mut operation.kind {
            OperationKind::WorkgroupMemory(memory) => Some(memory),
            _ => None,
        })
        .unwrap();
    memory.extent = WorkgroupMemoryExtent::Static(128);
    assert_ir_rejected(&wrong_extent);

    let mut wrong_alignment = canonical.clone();
    let memory = operations_mut(&mut wrong_alignment)
        .iter_mut()
        .find_map(|operation| match &mut operation.kind {
            OperationKind::WorkgroupMemory(memory) => Some(memory),
            _ => None,
        })
        .unwrap();
    memory.alignment = 8;
    assert_ir_rejected(&wrong_alignment);

    let first_lds = canonical.functions[0].body.as_ref().unwrap().blocks[0]
        .operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::WorkgroupMemory(_) => Some(operation.results[0].id),
            _ => None,
        })
        .unwrap();
    let mut aliasing_lds = canonical.clone();
    let second_store_base = operations_mut(&mut aliasing_lds)
        .iter_mut()
        .filter_map(|operation| match &mut operation.kind {
            OperationKind::Matrix(matrix) => match &mut matrix.kind {
                MatrixOperationKind::LdsStore { base, .. } => Some(base),
                _ => None,
            },
            _ => None,
        })
        .nth(1)
        .unwrap();
    *second_store_base = first_lds;
    assert_ir_rejected(&aliasing_lds);

    let mut wrong_layout = canonical.clone();
    let lds_profile = operations_mut(&mut wrong_layout)
        .iter_mut()
        .find_map(|operation| match &mut operation.kind {
            OperationKind::Matrix(matrix) => match &mut matrix.kind {
                MatrixOperationKind::LdsLoad { profile, .. } => Some(profile),
                _ => None,
            },
            _ => None,
        })
        .unwrap();
    lds_profile.rows = 8;
    assert_ir_rejected(&wrong_layout);

    let mut wrong_mfma = canonical.clone();
    let mfma_profile = operations_mut(&mut wrong_mfma)
        .iter_mut()
        .find_map(|operation| match &mut operation.kind {
            OperationKind::Matrix(matrix) => match &mut matrix.kind {
                MatrixOperationKind::MultiplyAccumulate { profile, .. } => Some(profile),
                _ => None,
            },
            _ => None,
        })
        .unwrap();
    mfma_profile.k = 8;
    assert_ir_rejected(&wrong_mfma);

    let c_base = canonical.functions[0].body.as_ref().unwrap().blocks[0]
        .operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::SliceData { slice: ValueId(2) } => Some(operation.results[0].id),
            _ => None,
        })
        .unwrap();
    let c_offsets = canonical.functions[0].body.as_ref().unwrap().blocks[0]
        .operations
        .iter()
        .filter_map(|operation| match operation.kind {
            OperationKind::GetElementPointer { base, offset } if base == c_base => Some(offset),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut duplicate_store_owner = canonical.clone();
    let first_c_offset = operations_mut(&mut duplicate_store_owner)
        .iter_mut()
        .find_map(|operation| match &mut operation.kind {
            OperationKind::GetElementPointer { base, offset } if *base == c_base => Some(offset),
            _ => None,
        })
        .unwrap();
    *first_c_offset = c_offsets[1];
    assert_ir_rejected(&duplicate_store_owner);
}

#[test]
fn rejects_barrier_removal_extra_functions_and_generic_lowering_bypass() {
    let mut no_barrier = tiled_gemm_lds_grid_v1_module();
    operations_mut(&mut no_barrier)
        .retain(|operation| !matches!(operation.kind, OperationKind::WorkgroupBarrier(_)));
    assert_ir_rejected(&no_barrier);

    let mut extra_function = tiled_gemm_lds_grid_v1_module();
    extra_function
        .functions
        .push(fe2o3_kernel_ir::Function::external_import(
            "hostile_call",
            fe2o3_kernel_ir::Signature::new(vec![], vec![]),
        ));
    assert_ir_rejected(&extra_function);

    let error = lower_kernel_to_gfx942_xnack_minus_llvm_ir(
        &tiled_gemm_lds_grid_v1_module(),
        &TILED_GEMM_LDS_GRID_V1_KERNEL_ID.into(),
    )
    .expect_err("generic exact-target lowering must reject unsupported grid indexing");
    assert!(
        error.contains(fe2o3_amdgcn_model::LoweringDiagnosticCode::UnsupportedOperation),
        "{error}"
    );
}

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-tiled-gemm-lds-grid-v1-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "requires upstream LLVM 22 tools with gfx942 support"]
fn upstream_llvm_lld_final_artifact_has_the_exact_grid_machine_shape() {
    let opt = std::env::var("FE2O3_OPT").expect("set FE2O3_OPT to upstream opt");
    let llc = std::env::var("FE2O3_LLC").expect("set FE2O3_LLC to upstream llc");
    let lld = std::env::var("FE2O3_LLD").expect("set FE2O3_LLD to upstream ld.lld");
    let objdump = std::env::var("FE2O3_LLVM_OBJDUMP")
        .expect("set FE2O3_LLVM_OBJDUMP to upstream llvm-objdump");
    let readobj = std::env::var("FE2O3_LLVM_READOBJ")
        .expect("set FE2O3_LLVM_READOBJ to upstream llvm-readobj");
    let directory = TemporaryDirectory::new();
    let input = directory.join("tiled_gemm_lds_grid_v1.ll");
    let assembly = directory.join("tiled_gemm_lds_grid_v1.s");
    let object = directory.join("tiled_gemm_lds_grid_v1.o");
    let hsaco = directory.join("tiled_gemm_lds_grid_v1.hsaco");
    fs::write(
        &input,
        lower_tiled_gemm_lds_grid_v1_to_gfx942_llvm_ir(&tiled_gemm_lds_grid_v1_module(), profile())
            .unwrap()
            .as_str(),
    )
    .unwrap();

    let verify = Command::new(&opt)
        .args(["-passes=verify", "-disable-output"])
        .arg(&input)
        .output()
        .unwrap();
    assert!(
        verify.status.success(),
        "upstream opt rejected Slice 3:\n{}",
        String::from_utf8_lossy(&verify.stderr)
    );

    let target_arguments = [
        "-mtriple=amdgcn-amd-amdhsa",
        "-mcpu=gfx942",
        "-mattr=-xnack",
        "--amdhsa-code-object-version=6",
        "-O=2",
    ];
    let compile_assembly = Command::new(&llc)
        .args(target_arguments)
        .arg("-filetype=asm")
        .arg(&input)
        .arg("-o")
        .arg(&assembly)
        .output()
        .unwrap();
    assert!(
        compile_assembly.status.success(),
        "upstream llc rejected Slice 3 assembly:\n{}",
        String::from_utf8_lossy(&compile_assembly.stderr)
    );
    let assembly = fs::read_to_string(&assembly).unwrap();
    for required in [
        ".amdhsa_code_object_version 6",
        ".amdhsa_system_sgpr_workgroup_id_x 1",
        ".amdhsa_system_sgpr_workgroup_id_y 1",
    ] {
        assert!(
            assembly.contains(required),
            "missing {required:?}\n{assembly}"
        );
    }

    let compile_object = Command::new(&llc)
        .args(target_arguments)
        .arg("-filetype=obj")
        .arg(&input)
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(
        compile_object.status.success(),
        "upstream llc rejected Slice 3 object:\n{}",
        String::from_utf8_lossy(&compile_object.stderr)
    );

    let link = Command::new(&lld)
        .args(["-shared", "--no-undefined"])
        .arg(&object)
        .arg("-o")
        .arg(&hsaco)
        .output()
        .unwrap();
    assert!(
        link.status.success(),
        "upstream ld.lld rejected Slice 3:\n{}",
        String::from_utf8_lossy(&link.stderr)
    );

    let bytes = fs::read(&hsaco).unwrap();
    for forbidden in [b"amd_comgr".as_slice(), b"libamd_comgr".as_slice()] {
        assert!(
            !bytes
                .windows(forbidden.len())
                .any(|window| window == forbidden),
            "final HSACO contains a forbidden COMGR reference"
        );
    }

    let notes = Command::new(&readobj)
        .args(["--file-headers", "--notes", "--symbols"])
        .arg(&hsaco)
        .output()
        .unwrap();
    assert!(
        notes.status.success(),
        "llvm-readobj rejected Slice 3:\n{}",
        String::from_utf8_lossy(&notes.stderr)
    );
    let notes = String::from_utf8(notes.stdout).unwrap();
    let compact_notes = notes.split_whitespace().collect::<Vec<_>>().join(" ");
    for required in [
        "OS/ABI: AMDGPU_HSA",
        "ABIVersion: 4",
        ".group_segment_fixed_size: 1024",
        ".private_segment_fixed_size: 0",
        ".max_flat_workgroup_size: 64",
        ".reqd_workgroup_size: - 64 - 1 - 1",
        ".sgpr_spill_count: 0",
        ".vgpr_spill_count: 0",
        ".uses_dynamic_stack: false",
        ".wavefront_size: 64",
        ".symbol: tiled_gemm_lds_grid_v1.kd",
        "amdhsa.target: 'amdgcn-amd-amdhsa--gfx942:xnack-'",
    ] {
        assert!(
            compact_notes.contains(required),
            "missing {required:?}\n{notes}"
        );
    }

    let disassembly = Command::new(&objdump)
        .args(["-d", "--mcpu=gfx942"])
        .arg(&hsaco)
        .output()
        .unwrap();
    assert!(
        disassembly.status.success(),
        "llvm-objdump rejected Slice 3:\n{}",
        String::from_utf8_lossy(&disassembly.stderr)
    );
    let disassembly = String::from_utf8(disassembly.stdout).unwrap();
    assert!(disassembly.contains("ds_write"), "{disassembly}");
    assert!(disassembly.contains("ds_read"), "{disassembly}");
    assert_eq!(disassembly.matches("s_barrier").count(), 1, "{disassembly}");
    assert_eq!(
        disassembly.matches("v_mfma_f32_16x16x16_bf16").count(),
        1,
        "{disassembly}"
    );
    for instruction in disassembly
        .lines()
        .flat_map(|line| line.split_ascii_whitespace())
        .filter(|token| token.starts_with("ds_"))
    {
        assert!(
            instruction.starts_with("ds_read") || instruction.starts_with("ds_write"),
            "unexpected LDS instruction {instruction:?}\n{disassembly}"
        );
    }
    for forbidden in [
        "scratch_",
        "flat_scratch",
        "buffer_atomic",
        "flat_atomic",
        "global_atomic",
        "s_call_b64",
        "s_swappc_b64",
    ] {
        assert!(
            !disassembly.contains(forbidden),
            "found forbidden {forbidden:?}\n{disassembly}"
        );
    }
    assert_eq!(TILED_GEMM_LDS_GRID_V1_STATIC_LDS_BYTES, 1024);
}
