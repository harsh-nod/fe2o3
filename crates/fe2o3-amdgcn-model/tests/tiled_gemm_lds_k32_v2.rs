use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use fe2o3_amdgcn_model::{
    GFX942_XNACK_MINUS_DATA_LAYOUT, TiledGemmLdsK32LoweringErrorV2,
    lower_kernel_to_gfx942_xnack_minus_llvm_ir, lower_tiled_gemm_lds_k32_v2_to_gfx942_llvm_ir,
};
use fe2o3_kernel_ir::{
    BinaryOp, Constant, MatrixOperationKind, OperationKind, TILED_GEMM_LDS_K32_V2_KERNEL_ID,
    TILED_GEMM_LDS_K32_V2_STATIC_LDS_BYTES, TargetCapability, Terminator, TiledGemmLdsK32V2Error,
    TiledGemmLdsK32V2Profile, ValueId, WaveWidth, WorkgroupMemoryExtent,
    tiled_gemm_lds_k32_v2_module,
};

fn profile() -> TiledGemmLdsK32V2Profile {
    TiledGemmLdsK32V2Profile::exact_gfx942_xnack_minus_cov6()
}

fn assert_ir_rejected(module: &fe2o3_kernel_ir::Module) {
    assert!(matches!(
        lower_tiled_gemm_lds_k32_v2_to_gfx942_llvm_ir(module, profile()),
        Err(TiledGemmLdsK32LoweringErrorV2::Profile(
            TiledGemmLdsK32V2Error::InvalidKernelIr(_)
                | TiledGemmLdsK32V2Error::NonCanonicalKernelIr
        ))
    ));
}

#[test]
fn lowers_only_the_canonical_two_phase_graph_to_exact_gfx942_llvm() {
    let expected_profile = profile();
    let output = lower_tiled_gemm_lds_k32_v2_to_gfx942_llvm_ir(
        &tiled_gemm_lds_k32_v2_module(),
        expected_profile.clone(),
    )
    .expect("canonical K32 LDS LLVM");
    assert_eq!(output.profile(), &expected_profile);
    let llvm = output.as_str();

    assert!(llvm.contains(GFX942_XNACK_MINUS_DATA_LAYOUT), "{llvm}");
    for required in [
        "target triple = \"amdgcn-amd-amdhsa\"",
        "define amdgpu_kernel void @tiled_gemm_lds_k32_v2(",
        "ptr addrspace(1) %arg0.data, i64 %arg0.len",
        "ptr addrspace(1) %arg1.data, i64 %arg1.len",
        "ptr addrspace(1) %arg2.data, i64 %arg2.len",
        "\"target-cpu\"=\"gfx942\"",
        "\"target-features\"=\"-wavefrontsize32,+wavefrontsize64,-xnack\"",
        "\"amdgpu-flat-work-group-size\"=\"64,64\"",
        "!0 = !{i32 64, i32 1, i32 1}",
        "icmp ult i64",
        "br i1",
        "fence syncscope(\"workgroup\") release",
        "call void asm sideeffect \"s_barrier\", \"\"()",
        "fence syncscope(\"workgroup\") acquire",
        "call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k(",
    ] {
        assert!(llvm.contains(required), "missing {required:?}\n{llvm}");
    }

    assert_eq!(llvm.matches(" = phi i64 ").count(), 1, "{llvm}");
    assert_eq!(llvm.matches(" = phi float ").count(), 8, "{llvm}");
    assert_eq!(llvm.matches(" = load i16, ptr addrspace(1)").count(), 8);
    assert_eq!(llvm.matches("store i16 ").count(), 8);
    assert_eq!(llvm.matches(" = load i16, ptr addrspace(3)").count(), 8);
    assert_eq!(llvm.matches("store float ").count(), 4);
    assert_eq!(
        llvm.matches("call void asm sideeffect \"s_barrier\", \"\"()")
            .count(),
        2
    );
    assert_eq!(
        llvm.matches("call <4 x float> @llvm.amdgcn.mfma.f32.16x16x16bf16.1k(")
            .count(),
        1
    );
    for forbidden in [
        "atomicrmw",
        "cmpxchg",
        "alloca",
        "addrspace(5)",
        "@__ocml_",
        "@__ockl_",
        "comgr",
        "COMGR",
    ] {
        assert!(!llvm.contains(forbidden), "found {forbidden:?}\n{llvm}");
    }
}

#[test]
fn rejects_nonexact_phase_launch_resource_and_target_profiles() {
    let mut profiles = Vec::new();

    let mut cov5 = profile();
    cov5.code_object_version = 5;
    profiles.push(cov5);
    let mut k16 = profile();
    k16.k = 16;
    profiles.push(k16);
    let mut one_phase = profile();
    one_phase.depth_tiles = 1;
    profiles.push(one_phase);
    let mut wave32 = profile();
    wave32.wave_width = WaveWidth::Wave32;
    profiles.push(wave32);
    let mut wrong_workgroup = profile();
    wrong_workgroup.workgroup_size.x = 128;
    profiles.push(wrong_workgroup);
    let mut one_allocation = profile();
    one_allocation.lds_allocations = 1;
    profiles.push(one_allocation);
    let mut oversized_lds = profile();
    oversized_lds.static_lds_bytes = 2048;
    profiles.push(oversized_lds);
    let mut weak_alignment = profile();
    weak_alignment.lds_alignment = 8;
    profiles.push(weak_alignment);
    let mut wrong_target_profile = profile();
    wrong_target_profile.target = TargetCapability::Extension {
        namespace: "fe2o3.amdgpu.target".to_owned(),
        name: "gfx950:xnack-".to_owned(),
    };
    profiles.push(wrong_target_profile);

    for nonexact in profiles {
        assert!(matches!(
            lower_tiled_gemm_lds_k32_v2_to_gfx942_llvm_ir(
                &tiled_gemm_lds_k32_v2_module(),
                nonexact
            ),
            Err(TiledGemmLdsK32LoweringErrorV2::Profile(
                TiledGemmLdsK32V2Error::UnsupportedProfile
            ))
        ));
    }

    let mut wrong_target = tiled_gemm_lds_k32_v2_module();
    wrong_target.required_capabilities.clear();
    assert_ir_rejected(&wrong_target);
}

#[test]
fn rejects_mutated_loop_bound_backedge_and_accumulator_carry() {
    let canonical = tiled_gemm_lds_k32_v2_module();

    let mut wrong_phase_count = canonical.clone();
    let phase_count = wrong_phase_count.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .iter_mut()
        .find(|operation| matches!(operation.kind, OperationKind::Constant(Constant::Index(2))))
        .unwrap();
    phase_count.kind = OperationKind::Constant(Constant::Index(3));
    assert_ir_rejected(&wrong_phase_count);

    let mut wrong_phase_step = canonical.clone();
    let body = &mut wrong_phase_step.functions[0].body.as_mut().unwrap().blocks[2];
    let Some(Terminator::Branch { arguments, .. }) = &body.terminator else {
        unreachable!()
    };
    let next_phase = arguments[0];
    let increment = body
        .operations
        .iter_mut()
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: ValueId(_),
                    rhs: ValueId(_),
                }
            ) && operation
                .results
                .first()
                .is_some_and(|result| result.id == next_phase)
        })
        .unwrap();
    let OperationKind::Binary { op, .. } = &mut increment.kind else {
        unreachable!()
    };
    *op = BinaryOp::Subtract;
    assert_ir_rejected(&wrong_phase_step);

    let zero = canonical.functions[0].body.as_ref().unwrap().blocks[0]
        .operations
        .iter()
        .find_map(|operation| match operation.kind {
            OperationKind::Constant(Constant::F32Bits(0)) => Some(operation.results[0].id),
            _ => None,
        })
        .unwrap();
    let mut reset_accumulators = canonical.clone();
    let Terminator::Branch { arguments, .. } = reset_accumulators.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks[2]
        .terminator
        .as_mut()
        .unwrap()
    else {
        unreachable!()
    };
    arguments[1..].fill(zero);
    assert_ir_rejected(&reset_accumulators);
}

#[test]
fn rejects_deleted_or_moved_barriers_and_lds_resource_drift() {
    let canonical = tiled_gemm_lds_k32_v2_module();
    let barrier_positions = canonical.functions[0].body.as_ref().unwrap().blocks[2]
        .operations
        .iter()
        .enumerate()
        .filter_map(|(index, operation)| {
            matches!(operation.kind, OperationKind::WorkgroupBarrier(_)).then_some(index)
        })
        .collect::<Vec<_>>();
    assert_eq!(barrier_positions.len(), 2);

    for position in barrier_positions.iter().rev() {
        let mut deleted = canonical.clone();
        deleted.functions[0].body.as_mut().unwrap().blocks[2]
            .operations
            .remove(*position);
        assert_ir_rejected(&deleted);
    }

    let mut moved = canonical.clone();
    let operations = &mut moved.functions[0].body.as_mut().unwrap().blocks[2].operations;
    let barrier = operations.remove(barrier_positions[0]);
    let mfma = operations
        .iter()
        .position(|operation| {
            matches!(
                operation.kind,
                OperationKind::Matrix(ref matrix)
                    if matches!(matrix.kind, MatrixOperationKind::MultiplyAccumulate { .. })
            )
        })
        .unwrap();
    operations.insert(mfma + 1, barrier);
    assert_ir_rejected(&moved);

    let mut wrong_extent = canonical.clone();
    let memory = wrong_extent.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .iter_mut()
        .find_map(|operation| match &mut operation.kind {
            OperationKind::WorkgroupMemory(memory) => Some(memory),
            _ => None,
        })
        .unwrap();
    memory.extent = WorkgroupMemoryExtent::Static(128);
    assert_ir_rejected(&wrong_extent);

    let mut wrong_alignment = canonical.clone();
    let memory = wrong_alignment.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .iter_mut()
        .find_map(|operation| match &mut operation.kind {
            OperationKind::WorkgroupMemory(memory) => Some(memory),
            _ => None,
        })
        .unwrap();
    memory.alignment = 8;
    assert_ir_rejected(&wrong_alignment);
}

#[test]
fn generic_exact_target_lowering_uses_workload_neutral_proofs() {
    let llvm = lower_kernel_to_gfx942_xnack_minus_llvm_ir(
        &tiled_gemm_lds_k32_v2_module(),
        &TILED_GEMM_LDS_K32_V2_KERNEL_ID.into(),
    )
    .expect("generic lowering admits verified uniform loops independently of workload identity");
    assert!(llvm.contains("llvm.amdgcn.mfma.f32.16x16x16bf16.1k"));
    assert_eq!(
        llvm.matches("call void @llvm.amdgcn.s.barrier()").count(),
        2
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
            "fe2o3-tiled-gemm-lds-k32-v2-{}-{nonce}",
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
fn upstream_llvm_lld_final_artifact_has_the_exact_k32_machine_shape() {
    let opt = std::env::var("FE2O3_OPT").expect("set FE2O3_OPT to upstream opt");
    let llc = std::env::var("FE2O3_LLC").expect("set FE2O3_LLC to upstream llc");
    let lld = std::env::var("FE2O3_LLD").expect("set FE2O3_LLD to upstream ld.lld");
    let objdump = std::env::var("FE2O3_LLVM_OBJDUMP")
        .expect("set FE2O3_LLVM_OBJDUMP to upstream llvm-objdump");
    let readobj = std::env::var("FE2O3_LLVM_READOBJ")
        .expect("set FE2O3_LLVM_READOBJ to upstream llvm-readobj");
    let directory = TemporaryDirectory::new();
    let input = directory.join("tiled_gemm_lds_k32_v2.ll");
    let object = directory.join("tiled_gemm_lds_k32_v2.o");
    let hsaco = directory.join("tiled_gemm_lds_k32_v2.hsaco");
    fs::write(
        &input,
        lower_tiled_gemm_lds_k32_v2_to_gfx942_llvm_ir(&tiled_gemm_lds_k32_v2_module(), profile())
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
        "upstream opt rejected K32 Slice 2:\n{}",
        String::from_utf8_lossy(&verify.stderr)
    );

    let compile = Command::new(&llc)
        .args([
            "-mtriple=amdgcn-amd-amdhsa",
            "-mcpu=gfx942",
            "-mattr=-xnack",
            "--amdhsa-code-object-version=6",
            "-filetype=obj",
            "-O=2",
        ])
        .arg(&input)
        .arg("-o")
        .arg(&object)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "upstream llc rejected K32 Slice 2:\n{}",
        String::from_utf8_lossy(&compile.stderr)
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
        "upstream ld.lld rejected K32 Slice 2:\n{}",
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
        "llvm-readobj rejected K32 Slice 2:\n{}",
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
        ".symbol: tiled_gemm_lds_k32_v2.kd",
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
        "llvm-objdump rejected K32 Slice 2:\n{}",
        String::from_utf8_lossy(&disassembly.stderr)
    );
    let disassembly = String::from_utf8(disassembly.stdout).unwrap();
    assert!(disassembly.contains("ds_write"), "{disassembly}");
    assert!(disassembly.contains("ds_read"), "{disassembly}");
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
    assert_eq!(TILED_GEMM_LDS_K32_V2_STATIC_LDS_BYTES, 1024);
}
