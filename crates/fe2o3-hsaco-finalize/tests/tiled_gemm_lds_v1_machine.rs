use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use dialect_amdgcn::lower_tiled_gemm_lds_v1_to_gfx942_llvm_ir;
use fe2o3_kernel_ir::{
    TILED_GEMM_LDS_V1_KERNEL_ID, TILED_GEMM_LDS_V1_STATIC_LDS_BYTES, TiledGemmLdsV1Profile,
    tiled_gemm_lds_v1_module,
};

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fe2o3-tiled-gemm-lds-v1-{}-{nonce}",
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
#[ignore = "requires upstream LLVM tools with gfx942 support"]
fn upstream_llvm_lld_final_artifact_has_the_exact_slice_1_machine_shape() {
    let llc = std::env::var("FE2O3_LLC").expect("set FE2O3_LLC to upstream llc");
    let lld = std::env::var("FE2O3_LLD").expect("set FE2O3_LLD to upstream ld.lld");
    let objdump = std::env::var("FE2O3_LLVM_OBJDUMP")
        .expect("set FE2O3_LLVM_OBJDUMP to upstream llvm-objdump");
    let directory = TemporaryDirectory::new();
    let input = directory.join("tiled_gemm_lds_v1.ll");
    let object = directory.join("tiled_gemm_lds_v1.o");
    let hsaco = directory.join("tiled_gemm_lds_v1.hsaco");
    fs::write(
        &input,
        lower_tiled_gemm_lds_v1_to_gfx942_llvm_ir(
            &tiled_gemm_lds_v1_module(),
            TiledGemmLdsV1Profile::exact_gfx942_xnack_minus_cov6(),
        )
        .unwrap()
        .as_str(),
    )
    .unwrap();

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
        "upstream llc rejected Slice 1:\n{}",
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
        "upstream ld.lld rejected Slice 1:\n{}",
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
    let bound = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(&bytes)
        .expect("inspect and bind final Slice 1 HSACO");
    let inspection = bound.inspection();
    assert_eq!(
        inspection.code_object_version(),
        fe2o3_hsaco::CodeObjectVersion::V6
    );
    assert_eq!(inspection.target().to_string(), "gfx942:xnack-");
    let [kernel] = inspection.kernels() else {
        panic!(
            "expected exactly one kernel, found {}",
            inspection.kernels().len()
        );
    };
    assert_eq!(kernel.name(), TILED_GEMM_LDS_V1_KERNEL_ID);
    assert_eq!(kernel.symbol(), "tiled_gemm_lds_v1.kd");
    assert_eq!(
        kernel.group_segment_fixed_size(),
        u64::from(TILED_GEMM_LDS_V1_STATIC_LDS_BYTES)
    );
    assert_eq!(kernel.private_segment_fixed_size(), 0);
    assert_eq!(kernel.wavefront_size(), 64);
    assert_eq!(kernel.required_workgroup_size(), Some([64, 1, 1]));
    assert_eq!(kernel.max_flat_workgroup_size(), 64);
    assert!(!kernel.uses_dynamic_stack());
    assert!(matches!(kernel.sgpr_spill_count(), None | Some(0)));
    assert!(matches!(kernel.vgpr_spill_count(), None | Some(0)));
    assert_eq!(bound.bindings().len(), 1);
    assert_eq!(
        bound.bindings()[0].descriptor().group_segment_fixed_size(),
        TILED_GEMM_LDS_V1_STATIC_LDS_BYTES
    );
    assert_eq!(
        bound.bindings()[0]
            .descriptor()
            .private_segment_fixed_size(),
        0
    );
    assert_eq!(bound.bindings()[0].descriptor().wavefront_size(), 64);

    let disassembly = Command::new(&objdump)
        .args(["-d", "--mcpu=gfx942"])
        .arg(&hsaco)
        .output()
        .unwrap();
    assert!(
        disassembly.status.success(),
        "llvm-objdump rejected Slice 1:\n{}",
        String::from_utf8_lossy(&disassembly.stderr)
    );
    let disassembly = String::from_utf8(disassembly.stdout).unwrap();
    assert!(disassembly.contains("ds_write"), "{disassembly}");
    assert!(disassembly.contains("ds_read"), "{disassembly}");
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
    assert_eq!(disassembly.matches("s_barrier").count(), 1, "{disassembly}");
    assert_eq!(
        disassembly.matches("v_mfma_f32_16x16x16_bf16").count(),
        1,
        "{disassembly}"
    );
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
}
