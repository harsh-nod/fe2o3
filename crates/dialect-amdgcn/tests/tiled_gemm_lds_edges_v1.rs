use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use dialect_amdgcn::{
    GFX942_XNACK_MINUS_DATA_LAYOUT, TiledGemmLdsEdgesLoweringErrorV1,
    lower_kernel_to_gfx942_xnack_minus_llvm_ir, lower_tiled_gemm_lds_edges_v1_to_gfx942_llvm_ir,
};
use fe2o3_kernel_ir::{
    Axis, BlockId, Constant, IndexKind, IntrinsicKind, LaunchDomain, LaunchExtent,
    MatrixOperationKind, Operation, OperationKind, TILED_GEMM_LDS_EDGES_V1_KERNEL_ID,
    TILED_GEMM_LDS_EDGES_V1_STATIC_LDS_BYTES, TargetCapability, Terminator,
    TiledGemmLdsEdgesV1Error, TiledGemmLdsEdgesV1Profile, ValueId, WaveWidth,
    WorkgroupMemoryExtent, WorkgroupSize, tiled_gemm_lds_edges_v1_module,
};

fn profile() -> TiledGemmLdsEdgesV1Profile {
    TiledGemmLdsEdgesV1Profile::exact_gfx942_xnack_minus_cov6()
}

fn operations_mut(module: &mut fe2o3_kernel_ir::Module) -> impl Iterator<Item = &mut Operation> {
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| block.operations.iter_mut())
}

fn block_mut(
    module: &mut fe2o3_kernel_ir::Module,
    id: BlockId,
) -> &mut fe2o3_kernel_ir::BasicBlock {
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .find(|block| block.id == id)
        .unwrap()
}

fn assert_ir_rejected(module: &fe2o3_kernel_ir::Module) {
    assert!(matches!(
        lower_tiled_gemm_lds_edges_v1_to_gfx942_llvm_ir(module, profile()),
        Err(TiledGemmLdsEdgesLoweringErrorV1::Profile(
            TiledGemmLdsEdgesV1Error::InvalidKernelIr(_)
                | TiledGemmLdsEdgesV1Error::NonCanonicalKernelIr
        ))
    ));
}

#[test]
fn lowers_only_the_exact_edge_graph_to_strict_gfx942_llvm() {
    let expected_profile = profile();
    let output = lower_tiled_gemm_lds_edges_v1_to_gfx942_llvm_ir(
        &tiled_gemm_lds_edges_v1_module(),
        expected_profile.clone(),
    )
    .expect("canonical tiled GEMM LDS edge LLVM");
    assert_eq!(output.profile(), &expected_profile);
    let llvm = output.as_str();

    assert!(llvm.contains(GFX942_XNACK_MINUS_DATA_LAYOUT), "{llvm}");
    for required in [
        "target triple = \"amdgcn-amd-amdhsa\"",
        "define amdgpu_kernel void @tiled_gemm_lds_edges_v1(",
        "ptr addrspace(1) %arg0.data, i64 %arg0.len",
        "ptr addrspace(1) %arg1.data, i64 %arg1.len",
        "ptr addrspace(1) %arg2.data, i64 %arg2.len",
        "call i32 @llvm.amdgcn.workgroup.id.x()",
        "call i32 @llvm.amdgcn.workgroup.id.y()",
        "\"target-cpu\"=\"gfx942\"",
        "\"target-features\"=\"-wavefrontsize32,+wavefrontsize64,-xnack\"",
        "\"amdgpu-flat-work-group-size\"=\"64,64\"",
        "\"fp-contract\"=\"off\"",
        "!0 = !{i32 64, i32 1, i32 1}",
        "fence syncscope(\"workgroup\") release",
        "call void asm sideeffect \"s_barrier\", \"\"()",
        "fence syncscope(\"workgroup\") acquire",
        "call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k(",
        "fmul float 0x4000000000000000",
        "fmul float 0xBFF0000000000000",
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
    for (needle, count) in [
        (" = phi i64 ", 1),
        (" = phi i16 ", 8),
        (" = phi float ", 8),
        ("icmp ult i64", 11),
        (" = and i1 ", 12),
        ("br i1", 13),
        (" = load i16, ptr addrspace(1)", 8),
        (" = load float, ptr addrspace(1)", 4),
        ("store i16 ", 8),
        (" = load i16, ptr addrspace(3)", 8),
        ("store float ", 4),
        (" = fmul float ", 8),
        (" = fadd float ", 4),
        ("call void asm sideeffect \"s_barrier\", \"\"()", 2),
        ("call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k(", 1),
    ] {
        assert_eq!(llvm.matches(needle).count(), count, "{needle:?}\n{llvm}");
    }
    assert_eq!(
        llvm.lines()
            .filter(|line| line.contains(" = phi i16 ") && line.contains("[ 0, "))
            .count(),
        8,
        "{llvm}"
    );
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
fn rejects_every_nonexact_edge_profile_before_lowering() {
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
    mutated!(m, 16);
    mutated!(n, 18);
    mutated!(k, 17);
    mutated!(alpha_bits, 1.0f32.to_bits());
    mutated!(beta_bits, 0.0f32.to_bits());
    mutated!(a_elements, 305);
    mutated!(b_elements, 341);
    mutated!(c_elements, 322);
    mutated!(a_bytes, 610);
    mutated!(b_bytes, 682);
    mutated!(c_bytes, 1_288);
    mutated!(tile_rows, 1);
    mutated!(tile_columns, 1);
    mutated!(depth_tiles, 1);
    mutated!(phase_k, 8);
    mutated!(workgroup_count, 3);
    mutated!(wave_width, WaveWidth::Wave32);
    mutated!(launch_extent_x, 64);
    mutated!(launch_extent_y, 1);
    mutated!(workgroup_size, WorkgroupSize::new(32, 2, 1));
    mutated!(lds_allocations, 1);
    mutated!(lds_elements_per_allocation, 255);
    mutated!(lds_bytes_per_allocation, 1_024);
    mutated!(static_lds_bytes, 512);
    mutated!(lds_alignment, 8);
    mutated!(output_elements_per_lane, 8);

    assert_eq!(mutations.len(), 28);
    for mutation in mutations {
        assert!(matches!(
            lower_tiled_gemm_lds_edges_v1_to_gfx942_llvm_ir(
                &tiled_gemm_lds_edges_v1_module(),
                mutation
            ),
            Err(TiledGemmLdsEdgesLoweringErrorV1::Profile(
                TiledGemmLdsEdgesV1Error::UnsupportedProfile
            ))
        ));
    }
}

#[test]
fn rejects_tail_predicate_phase_epilogue_and_launch_drift() {
    let canonical = tiled_gemm_lds_edges_v1_module();

    let mut wrong_group_axis = canonical.clone();
    let group_y = operations_mut(&mut wrong_group_axis)
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
        x: LaunchExtent::Static(64),
        y: LaunchExtent::Static(2),
    };
    assert_ir_rejected(&wrong_launch);

    let mut unguarded_load = canonical.clone();
    let phase_active = match &block_mut(&mut unguarded_load, BlockId(1)).terminator {
        Some(Terminator::ConditionalBranch { condition, .. }) => *condition,
        _ => unreachable!(),
    };
    let condition = match &mut block_mut(&mut unguarded_load, BlockId(2)).terminator {
        Some(Terminator::ConditionalBranch { condition, .. }) => condition,
        _ => unreachable!(),
    };
    *condition = phase_active;
    assert_ir_rejected(&unguarded_load);

    for (bits, replacement) in [
        (2.0f32.to_bits(), 1.0f32.to_bits()),
        ((-1.0f32).to_bits(), 0.0f32.to_bits()),
    ] {
        let mut wrong_coefficient = canonical.clone();
        let coefficient = operations_mut(&mut wrong_coefficient)
            .find_map(|operation| match &mut operation.kind {
                OperationKind::Constant(Constant::F32Bits(value)) if *value == bits => Some(value),
                _ => None,
            })
            .unwrap();
        *coefficient = replacement;
        assert_ir_rejected(&wrong_coefficient);
    }

    let mut one_phase = canonical.clone();
    let phase_count = operations_mut(&mut one_phase)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::Constant(Constant::Index(2)) => Some(operation),
            _ => None,
        })
        .unwrap();
    phase_count.kind = OperationKind::Constant(Constant::Index(1));
    assert_ir_rejected(&one_phase);
}

#[test]
fn rejects_barrier_resource_layout_mfma_ownership_and_call_drift() {
    let canonical = tiled_gemm_lds_edges_v1_module();

    for ordinal in 0..2 {
        let mut no_barrier = canonical.clone();
        let phase = block_mut(&mut no_barrier, BlockId(18));
        let position = phase
            .operations
            .iter()
            .enumerate()
            .filter_map(|(index, operation)| {
                matches!(operation.kind, OperationKind::WorkgroupBarrier(_)).then_some(index)
            })
            .nth(ordinal)
            .unwrap();
        phase.operations.remove(position);
        assert_ir_rejected(&no_barrier);
    }

    for (extent, alignment) in [(128, 16), (256, 8)] {
        let mut wrong_resource = canonical.clone();
        let memory = operations_mut(&mut wrong_resource)
            .find_map(|operation| match &mut operation.kind {
                OperationKind::WorkgroupMemory(memory) => Some(memory),
                _ => None,
            })
            .unwrap();
        memory.extent = WorkgroupMemoryExtent::Static(extent);
        memory.alignment = alignment;
        assert_ir_rejected(&wrong_resource);
    }

    let first_lds = operations_mut(&mut canonical.clone())
        .find_map(|operation| match operation.kind {
            OperationKind::WorkgroupMemory(_) => Some(operation.results[0].id),
            _ => None,
        })
        .unwrap();
    let mut aliasing_lds = canonical.clone();
    let second_store_base = operations_mut(&mut aliasing_lds)
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
    let mut wrong_owner = canonical.clone();
    let offset = operations_mut(&mut wrong_owner)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::GetElementPointer { base, offset } if *base == c_base => Some(offset),
            _ => None,
        })
        .unwrap();
    *offset = ValueId(0);
    assert_ir_rejected(&wrong_owner);

    let mut extra_function = canonical.clone();
    extra_function
        .functions
        .push(fe2o3_kernel_ir::Function::external_import(
            "hostile_call",
            fe2o3_kernel_ir::Signature::new(vec![], vec![]),
        ));
    assert_ir_rejected(&extra_function);
}

#[test]
fn generic_lowering_cannot_bypass_exact_edge_authentication() {
    let error = lower_kernel_to_gfx942_xnack_minus_llvm_ir(
        &tiled_gemm_lds_edges_v1_module(),
        &TILED_GEMM_LDS_EDGES_V1_KERNEL_ID.into(),
    )
    .expect_err("generic exact-target lowering must reject the 2D edge grid");
    assert!(
        error.contains(dialect_amdgcn::LoweringDiagnosticCode::UnsupportedLaunchDomain),
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
            "fe2o3-tiled-gemm-lds-edges-v1-{}-{nonce}",
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
fn upstream_llvm_lld_final_artifact_has_the_exact_edge_machine_shape() {
    let opt = std::env::var("FE2O3_OPT").expect("set FE2O3_OPT to upstream opt");
    let llc = std::env::var("FE2O3_LLC").expect("set FE2O3_LLC to upstream llc");
    let lld = std::env::var("FE2O3_LLD").expect("set FE2O3_LLD to upstream ld.lld");
    let objdump = std::env::var("FE2O3_LLVM_OBJDUMP")
        .expect("set FE2O3_LLVM_OBJDUMP to upstream llvm-objdump");
    let readobj = std::env::var("FE2O3_LLVM_READOBJ")
        .expect("set FE2O3_LLVM_READOBJ to upstream llvm-readobj");
    let directory = TemporaryDirectory::new();
    let input = directory.join("tiled_gemm_lds_edges_v1.ll");
    let assembly = directory.join("tiled_gemm_lds_edges_v1.s");
    let object = directory.join("tiled_gemm_lds_edges_v1.o");
    let hsaco = directory.join("tiled_gemm_lds_edges_v1.hsaco");
    fs::write(
        &input,
        lower_tiled_gemm_lds_edges_v1_to_gfx942_llvm_ir(
            &tiled_gemm_lds_edges_v1_module(),
            profile(),
        )
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
        "upstream opt rejected Slice 4:\n{}",
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
        "upstream llc rejected Slice 4 assembly:\n{}",
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
        "upstream llc rejected Slice 4 object:\n{}",
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
        "upstream ld.lld rejected Slice 4:\n{}",
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
        "llvm-readobj rejected Slice 4:\n{}",
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
        ".symbol: tiled_gemm_lds_edges_v1.kd",
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
        "llvm-objdump rejected Slice 4:\n{}",
        String::from_utf8_lossy(&disassembly.stderr)
    );
    let disassembly = String::from_utf8(disassembly.stdout).unwrap();
    assert!(disassembly.contains("ds_write"), "{disassembly}");
    assert!(disassembly.contains("ds_read"), "{disassembly}");
    assert!(disassembly.contains("global_load"), "{disassembly}");
    assert!(disassembly.contains("global_store"), "{disassembly}");
    assert!(disassembly.contains("v_cmp"), "{disassembly}");
    assert_eq!(disassembly.matches("s_barrier").count(), 2, "{disassembly}");
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
    assert_eq!(TILED_GEMM_LDS_EDGES_V1_STATIC_LDS_BYTES, 1024);
}
