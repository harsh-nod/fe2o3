//! Separate, feature-gated MI350-2 platform provenance. All device, UAPI,
//! currentness, memory and queue checks remain in their existing owners.

const KERNEL: &str = "5.18.2-mi300-build-140423-ubuntu-22.04+";
const MODULE: &str = "6.16.13";
const SRCVERSION: &str = "975C4B2AA8AD01E2EA472C0";

const MANIFEST: &str = concat!(
    "profile_id=fe2o3-linux-x86_64-mi350x-gfx950-xnack-minus-spx-nps1-mi350-2-observation-v2\n",
    "kfd_schema_sha256=e4aad5d8e3177ea6d70298adab7741c377cb091373553ce689f3525e7514d9b4\n",
    "drm_schema_sha256=800569fe9b467b389bcfc6e5d65b23d66a0386a90fc2a669fac8c83800e76d8b\n",
    "kernel_release=5.18.2-mi300-build-140423-ubuntu-22.04+\n",
    "amdgpu_module=6.16.13\n",
    "amdgpu_srcversion=975C4B2AA8AD01E2EA472C0\n",
    "source_package=amdgpu-6.16.13-2278356.22.04\n",
    "source.kfd_ioctl.h=b3721c1a428a32bb9994af579432af48c44fa65abb860049f11a63a5c093235d\n",
    "source.amdgpu_drm.h=9d7ff60a211d2aa73a6c15b2da49e050cebe518fc059ee93e31d61288f7b60dc\n",
    "source.core_drm.h=0c0cdd010647597ad419ce1321a1a7a9f6844018121b455029cfc01b9c226d38\n",
    "source.core_drm_mode.h=7abe7b569908ec66ccf33a33b939a02a35b52798aaf087735c3e26b79944c7e7\n",
    "source.kfd_chardev.c=1b7cd9127be8d9042ea8a8c654ccb1cecacdf5f7ccb88f771a6b0e7af9873c99\n",
    "source.kfd_smi_events.c=2d786562fe1e97b8257841b755106c8bce47658a2aa3b439ce4e0178323004bd\n",
    "source.kfd_events.c=17a5de4c7fd41564079e6777ac820e2f6c0017fd3cee351d0031e2c84d0535e8\n",
    "source.kfd_events.h=de275617babe153c015f22de23d4f3ed013759c0a63da96e061454114f0dd119\n",
    "source.kfd_process.c=d9b48993dfc5ce196d19028e85d22652938038730782a278902fc2624fd52984\n",
    "source.kfd_debug.c=f6c688b75fd25ead43ce3c3961bd0af210f873bad1b29dce8e84bb7fb968fe4d\n",
    "source.kfd_priv.h=f1b0b07bc63f0c060509e1b03fe1e621880fd4b2a101f31d23b414ad873b3a5a\n",
    "source.kfd_device_queue_manager.c=e29aec7ae2effd531e6ba7dd9286e6202ea7a222ce0466372f4e43b114f29d2a\n",
    "source.kfd_device_queue_manager_v9.c=53021a6f8211212f872545403e200d34d2e8c49b1cbdd17e382ae7baa43e52f2\n",
    "source.kfd_queue.c=fb4b2a5c9e6981222873bcd7aca7e9c1397cba8f1a6b33634d2a48d4427fe062\n",
    "source.kfd_doorbell.c=de30437ee1ed9ccbdaf855899482c0bebb7f55adc120ac712c96cadef1a0ec6d\n",
    "source.kfd_mqd_manager_v9.c=21166e9dbe2a4c24cbcd6f9ff6193aa093230e91fbafc8b4ac4eee1465cd2c9e\n",
    "source.kfd_process_queue_manager.c=8526e258824dbe145e4209cf0fed26463729234ba24369f39e3413e7e6e028db\n",
    "source.amdgpu_amdkfd_gpuvm.c=c7cca2ee47a08c99bb73906662d82dd7d0b5738468fbef54848e5e6dd62ba50d\n",
    "source.amdgpu_kms.c=ef2375c3f35ad4a24b560326b55676a907d6d2ba248e469a62e84e877435101c\n",
    "source.soc15.c=e51e3c71fd479abe29b1ab667474de1a8416243e6a83bc9ce743306b1b17aea8\n",
    "source.amdgpu_discovery.c=f931372a3632e2c753897c80e90c300f570ebeb51a2c8021bda311cdbdde5c85\n",
    "source.kfd_topology.c=47f2ed7121b64af169cbaf1edbdd408be46c2e07efaae5ffa6c009b469ea9737\n",
    "source.amdgpu_xcp.c=4043b92fddc8caf29d5db98768dbffd53495fcd18bc15b0f8fdafa9583a21307\n",
    "source.amdgpu_gfx.c=205523d5c4649470e9bb71220f2628fde29a8719b49610af3e81d2ce33389b3d\n",
    "source.aqua_vanjaram.c=76f5ee0b5fbc4b5ebe5886893f968dc8a2956c0b52cc6384c00570253b656758\n",
    "uapi=kfd:1.18,drm:3.64.0\n",
    "target=gfx950:90500,wavefront:64,simd:1024,xcc:8\n",
    "pci=vendor:1002,device:75a0,revision:00\n",
    "drm_device=chip_rev:0,external_rev:80,family:141,acceleration:1\n",
    "partition=SPX/NPS1\n",
    "render_correlation=contracted-xcp0-parent-render-v1,exact-platform-and-full-device-geometry,canonical-pci-parent,unique-endpoint,visible-xcp-metrics,distinct-kfd-xcd-and-pci-board-uid-domains,recheck-both,no-uid-equivalence-claim\n",
    "firmware_observation=compute:41,sdma:12\n",
    "xnack=disabled-query-only,no-setter,no-no-queue-barrier-claim\n",
    "apertures=max:16,count-fill-count,complete-topology-inventory,page-aligned,inclusive,record-disjoint\n",
    "currentness=process,retained-fds,full-topology,xnack,apertures,prospective-smi-reset-events,vram-loss-counter,poison-on-error,no-all-reset-or-aba-proof\n",
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
    fn exact_engineering_profile_has_distinct_retained_provenance() {
        assert_eq!(
            validate_platform(KERNEL, Some(MODULE), Some(SRCVERSION)).unwrap(),
            MANIFEST
        );
        assert_ne!(
            Sha256::digest(MANIFEST),
            Sha256::digest(GFX950_DEVICE_OBSERVATION_PROFILE_MANIFEST_V1)
        );
        assert!(MANIFEST.contains("no-memory,no-queue,no-dispatch,no-gfx942-conversion"));
        assert!(MANIFEST.contains("no-all-reset-or-aba-proof"));
    }

    #[test]
    fn platform_fields_cannot_be_mixed_between_profiles() {
        assert!(
            validate_platform(
                KERNEL,
                Some(MODULE),
                Some(GFX950_ADMITTED_AMDGPU_MODULE_SRCVERSION_V1)
            )
            .is_err()
        );
        assert!(
            validate_platform(
                GFX950_ADMITTED_KERNEL_RELEASE_V1,
                Some(MODULE),
                Some(SRCVERSION)
            )
            .is_err()
        );
        assert_eq!(
            validate_platform(
                GFX950_ADMITTED_KERNEL_RELEASE_V1,
                Some(MODULE),
                Some(GFX950_ADMITTED_AMDGPU_MODULE_SRCVERSION_V1)
            )
            .unwrap(),
            GFX950_DEVICE_OBSERVATION_PROFILE_MANIFEST_V1,
        );
    }

    #[test]
    fn every_engineering_platform_field_is_exact() {
        for kernel in ["", "5.18.2", "5.18.2-mi300-build-140423-ubuntu-22.04"] {
            assert!(validate_platform(kernel, Some(MODULE), Some(SRCVERSION)).is_err());
        }
        for version in [None, Some(""), Some("6.16.12"), Some("6.16.13-extra")] {
            assert!(validate_platform(KERNEL, version, Some(SRCVERSION)).is_err());
        }
        for source in [
            None,
            Some(""),
            Some("975C4B2AA8AD01E2EA472C1"),
            Some("975c4b2aa8ad01e2ea472c0"),
        ] {
            assert!(validate_platform(KERNEL, Some(MODULE), source).is_err());
        }
    }
}
