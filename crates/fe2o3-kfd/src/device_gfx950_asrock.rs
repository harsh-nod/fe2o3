//! Exact engineering-only asrock platform provenance. The selected profile
//! grants checked observations, not protected runtime or dispatch authority.

const KERNEL: &str = "5.15.160+";
const MODULE: &str = "6.16.15";
const SRCVERSION: &str = "9462451703604FCD7EC2365";

const MANIFEST: &str = concat!(
    "profile_id=fe2o3-linux-x86_64-mi350x-gfx950-xnack-minus-spx-nps1-asrock-observation-v3\n",
    "kfd_schema_sha256=e4aad5d8e3177ea6d70298adab7741c377cb091373553ce689f3525e7514d9b4\n",
    "drm_schema_sha256=800569fe9b467b389bcfc6e5d65b23d66a0386a90fc2a669fac8c83800e76d8b\n",
    "kernel_release=5.15.160+\n",
    "amdgpu_module=6.16.15\n",
    "amdgpu_srcversion=9462451703604FCD7EC2365\n",
    "source_package=amdgpu-6.16.15-2267428.22.04\n",
    "source.kfd_ioctl.h=fb192f1da891b1c860b83ae2e42a2e614585c6eff5c9b45f79d959f79b191d3b\n",
    "source.amdgpu_drm.h=73345e88d09d15ef7bbd73d2ba546aed87c6c7fd2ea11929cbe2b2182c073b5b\n",
    "source.core_drm.h=44bfe472eca993bad4afe5cd1f087fbcc42fc8bad0e7f7c5f59e2a594de378b4\n",
    "source.core_drm_mode.h=4464ce6c971b8753963778f3e3f911c59dcc8e75e67e526f5094c2f8392e2200\n",
    "source.kfd_chardev.c=f646ec2f64a8eb88dd66e13b2894c438ac620c6486227623caa83724f16e2be4\n",
    "source.kfd_process.c=bf9ac62441b5e2ed5aa5f9c565186f22668f1a84cf1f3c3ad2bc3c90597814ae\n",
    "source.kfd_events.c=17a5de4c7fd41564079e6777ac820e2f6c0017fd3cee351d0031e2c84d0535e8\n",
    "source.kfd_events.h=de275617babe153c015f22de23d4f3ed013759c0a63da96e061454114f0dd119\n",
    "source.kfd_debug.c=e1bdcf26bfe3ad76e7a107514a3c1ee038152c807a3ef2a5619a011801d380e6\n",
    "source.kfd_priv.h=3374d168bfc43b0b34617faa06d123cf5b98200ea311bde9ac7c932c6574fd90\n",
    "source.kfd_smi_events.c=2d786562fe1e97b8257841b755106c8bce47658a2aa3b439ce4e0178323004bd\n",
    "source.amdgpu_amdkfd_gpuvm.c=c7cca2ee47a08c99bb73906662d82dd7d0b5738468fbef54848e5e6dd62ba50d\n",
    "source.amdgpu_kms.c=ef2375c3f35ad4a24b560326b55676a907d6d2ba248e469a62e84e877435101c\n",
    "source.kfd_device_queue_manager.c=823650c1a8e9f1072ae9794a82814d6be55b8f95d5c29e5a494a00aa7604521c\n",
    "source.kfd_device_queue_manager_v9.c=53021a6f8211212f872545403e200d34d2e8c49b1cbdd17e382ae7baa43e52f2\n",
    "source.kfd_queue.c=fb4b2a5c9e6981222873bcd7aca7e9c1397cba8f1a6b33634d2a48d4427fe062\n",
    "source.kfd_doorbell.c=de30437ee1ed9ccbdaf855899482c0bebb7f55adc120ac712c96cadef1a0ec6d\n",
    "source.kfd_mqd_manager_v9.c=8ed0dc9b3421cc005eb19adf9f1e9bf63c1fbb483a4d3aafbd0753b561c0999a\n",
    "source.kfd_process_queue_manager.c=2aa82855d39ddf89dd324bfddd407b0cad43f02dc26f2e5eb9c23235ac924e9a\n",
    "source.kfd_topology.c=88e323660fd9113285f7c08450996c723e401ecb4abf275ff38053eee34d09fe\n",
    "source.kfd_device.c=973810be606feeee501faf5d841ac76387ff1a454f1a73e91956b3667e80d9d3\n",
    "source.amdgpu_xcp.c=4043b92fddc8caf29d5db98768dbffd53495fcd18bc15b0f8fdafa9583a21307\n",
    "source.aqua_vanjaram.c=116c82dd86caaa700f40fecfc007718d49374c3484d3d9eb068c3d9fbd68453e\n",
    "source.amdgpu_gfx.c=1e836f04083038b9913f1411de560bbbc24d297cdae1a3d78200ea8681545f94\n",
    "source.amdgpu_amdkfd.c=50c6cdf095070a24a09ae33cc7e6d2745a34fec9e7396bbfc16edb81989f0672\n",
    "source.soc15.c=019777285c19a83a8a81835b427780cdcbc2234266da820bed425245fcdd6c5b\n",
    "source.kernel_config=8206007cbe9d40a1683a79efc2469ed5732a3db7fd6805b7fd9e36cad77ef6c2\n",
    "source.dkms_make_log=3ec6c1e60177d4784378b678822c5829b5619d11223a0aedb965f4f345ccfd8c\n",
    "uapi=kfd:1.18,drm:3.64.0\n",
    "target=gfx950:90500,wavefront:64,simd:1024,xcc:8\n",
    "pci=vendor:1002,device:75a0,revision:00\n",
    "drm_device=chip_rev:0,external_rev:80,family:141,acceleration:1\n",
    "partition=SPX/NPS1\n",
    "render_correlation=same-device-uid,spx-kfd-and-pci-board-uid-equality,exact-render-and-pci-identity,no-xcp-uid-exception\n",
    "firmware_observation=compute:41,sdma:12\n",
    "xnack=disabled-query-only,no-setter,no-no-queue-barrier-claim\n",
    "apertures=max:16,count-fill-count,complete-topology-inventory,page-aligned,inclusive,record-disjoint\n",
    "currentness=process,retained-fds,full-topology,xnack,apertures,prospective-smi-reset-events,vram-loss-counter,poison-on-error,no-all-reset-or-aba-proof\n",
    "source_contract=reviewed-used-driver-paths,retained-trusted-kernel-driver-assumption,no-running-module-attestation\n",
    "authority=checked-observation-only,no-model-admission,no-explicit-vm-acquisition,no-vm-authority,no-memory,no-queue,no-dispatch,no-gfx942-conversion\n",
);

pub(super) fn match_platform(
    kernel: &str,
    version: Option<&str>,
    source: Option<&str>,
) -> Option<&'static str> {
    (kernel == KERNEL && version == Some(MODULE) && source == Some(SRCVERSION)).then_some(MANIFEST)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::gfx950::{
        GFX950_ADMITTED_AMDGPU_MODULE_SRCVERSION_V1, GFX950_ADMITTED_KERNEL_RELEASE_V1,
        GFX950_DEVICE_OBSERVATION_PROFILE_MANIFEST_V1, validate_platform,
    };
    use sha2::{Digest, Sha256};

    #[test]
    fn asrock_has_distinct_observation_provenance_without_protected_authority() {
        assert_eq!(
            validate_platform(KERNEL, Some(MODULE), Some(SRCVERSION)).unwrap(),
            MANIFEST
        );
        let old_engineering = validate_platform(
            "5.18.2-mi300-build-140423-ubuntu-22.04+",
            Some("6.16.13"),
            Some("975C4B2AA8AD01E2EA472C0"),
        )
        .unwrap();
        for old in [
            GFX950_DEVICE_OBSERVATION_PROFILE_MANIFEST_V1,
            old_engineering,
        ] {
            assert_ne!(Sha256::digest(MANIFEST), Sha256::digest(old));
        }
        let digest: [u8; 32] = Sha256::digest(MANIFEST).into();
        assert_ne!(digest, crate::DEVICE_ADMISSION_PROFILE_SHA256_BYTES_V1);
        for retained in [
            "no-model-admission,no-explicit-vm-acquisition,no-vm-authority,no-memory,no-queue,no-dispatch,no-gfx942-conversion",
            "no-all-reset-or-aba-proof",
            "same-device-uid,spx-kfd-and-pci-board-uid-equality",
            "no-running-module-attestation",
        ] {
            assert!(MANIFEST.contains(retained));
        }
    }

    #[test]
    fn asrock_requires_every_exact_platform_field() {
        for kernel in ["", "5.15.160", "5.15.160++", "5.15.161+", "5.15.160+ "] {
            assert!(validate_platform(kernel, Some(MODULE), Some(SRCVERSION)).is_err());
        }
        for version in [None, Some(""), Some("6.16.13"), Some("6.16.15-extra")] {
            assert!(validate_platform(KERNEL, version, Some(SRCVERSION)).is_err());
        }
        for source in [
            None,
            Some(""),
            Some("9462451703604FCD7EC2366"),
            Some("9462451703604fcd7ec2365"),
        ] {
            assert!(validate_platform(KERNEL, Some(MODULE), source).is_err());
        }
    }

    #[test]
    fn asrock_cannot_mix_fields_with_either_earlier_profile() {
        let profiles = [
            (
                GFX950_ADMITTED_KERNEL_RELEASE_V1,
                "6.16.13",
                GFX950_ADMITTED_AMDGPU_MODULE_SRCVERSION_V1,
            ),
            (
                "5.18.2-mi300-build-140423-ubuntu-22.04+",
                "6.16.13",
                "975C4B2AA8AD01E2EA472C0",
            ),
            (KERNEL, MODULE, SRCVERSION),
        ];
        for (kernel, _, _) in profiles {
            for (_, version, _) in profiles {
                for (_, _, source) in profiles {
                    let exact = profiles.contains(&(kernel, version, source));
                    assert_eq!(
                        validate_platform(kernel, Some(version), Some(source)).is_ok(),
                        exact
                    );
                    assert_eq!(
                        match_platform(kernel, Some(version), Some(source)).is_some(),
                        (kernel, version, source) == (KERNEL, MODULE, SRCVERSION)
                    );
                }
            }
        }
    }
}
