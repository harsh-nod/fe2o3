use super::*;
use sha2::{Digest, Sha256};
use std::fmt::Write;

fn facts() -> TargetFacts {
    TargetFacts {
        unique_id: 16366993098680759275,
        gpu_id: 11429,
        generation: 17,
        target: GfxTarget::Gfx950,
        compute: ComputePartition::Spx,
        memory: MemoryPartition::Nps1,
        geometry: GeometryInputs {
            simd: 1024,
            simd_per_cu: 4,
            xcc: 8,
            arrays: 32,
            arrays_per_engine: 1,
            lds_kib: 160,
        },
        pci: 0x75a0,
        firmware: 41,
        sdma_firmware: 12,
        wavefront: 64,
        max_waves: 8,
        queues: 24,
    }
}

fn digest_hex(text: &str) -> String {
    let mut result = String::new();
    for byte in Sha256::digest(text) {
        write!(result, "{byte:02x}").unwrap();
    }
    result
}

#[test]
fn independent_manifest_digest_is_frozen() {
    assert_eq!(
        digest_hex(GFX950_QUEUE_RESOURCE_PROFILE_MANIFEST_V1),
        GFX950_QUEUE_RESOURCE_PROFILE_SHA256_V1
    );
    assert_ne!(
        GFX950_QUEUE_RESOURCE_PROFILE_SHA256_V1,
        crate::GFX942_QUEUE_RESOURCE_PROFILE_SHA256_V1
    );
    assert_eq!(
        crate::GFX942_QUEUE_RESOURCE_PROFILE_SHA256_V1,
        "37d45132916d2ecefdec8f53ecab817cbdbaa9b9863440353163bd460626ab02"
    );
    assert_eq!(
        digest_hex(crate::GFX942_QUEUE_RESOURCE_PROFILE_MANIFEST_V1),
        crate::GFX942_QUEUE_RESOURCE_PROFILE_SHA256_V1
    );
}

#[test]
fn observed_gfx950_dimensions_are_not_gfx942_dimensions() {
    let plan = plan_from_facts(facts(), 4096).unwrap();
    let c = plan.context_save();
    assert_eq!(plan.unique_id(), facts().unique_id);
    assert_eq!(plan.gpu_id(), 11429);
    assert_eq!(plan.topology_generation(), 17);
    assert_eq!(plan.target(), GfxTarget::Gfx950);
    assert_eq!(
        plan.profile_sha256(),
        GFX950_QUEUE_RESOURCE_PROFILE_SHA256_V1
    );
    assert_eq!((c.cu_per_xcc(), c.waves_per_xcc()), (32, 1280));
    assert_eq!(
        (
            c.control_bytes_per_xcc(),
            c.workgroup_bytes_per_xcc(),
            c.context_bytes_per_xcc()
        ),
        (0x3000, 0x15a0000, 0x15a3000)
    );
    assert_eq!(
        (c.debug_bytes_per_xcc(), c.mapping_bytes()),
        (0xa000, 0xad68000)
    );
    assert_ne!(
        u64::from(c.mapping_bytes()),
        crate::GFX942_CONTEXT_SAVE_MAPPING_BYTES_V1
    );
    assert_eq!(
        (
            plan.packet_bytes(),
            plan.counter_bytes(),
            plan.end_of_pipe_mapping_bytes()
        ),
        (64, 8, 4096)
    );
    assert_eq!(
        (
            plan.write_dispatch_id_offset(),
            plan.read_dispatch_id_offset(),
            plan.read_base_offset_field()
        ),
        (0x38, 0x80, 0x88)
    );
    assert_eq!(
        (
            plan.resource_alignment_bytes(),
            plan.control_mapping_bytes_per_pointer()
        ),
        (4096, 4096)
    );
}

#[test]
fn host_profile_rejects_wrong_source_driver_parameters_and_page_size() {
    let good = (
        "6.8.0-124-generic",
        Some("6.16.13"),
        Some("703B1127E578BC5D4BD6615"),
        [Some(0), Some(0), Some(1)],
        4096,
    );
    assert!(host_profile(good.0, good.1, good.2, good.3, good.4).is_ok());
    for value in ["", "6.8.0-123-generic"] {
        assert!(host_profile(value, good.1, good.2, good.3, good.4).is_err());
    }
    for value in [None, Some("6.16.12")] {
        assert!(host_profile(good.0, value, good.2, good.3, good.4).is_err());
    }
    for value in [None, Some("A6F143BEC60C0AFC3263226")] {
        assert!(host_profile(good.0, good.1, value, good.3, good.4).is_err());
    }
    for index in 0..3 {
        for value in [None, Some(-1), Some(2)] {
            let mut parameters = good.3;
            parameters[index] = value;
            assert!(host_profile(good.0, good.1, good.2, parameters, good.4).is_err());
        }
    }
    for page in [0, 2048, 8192, 65536] {
        assert!(host_profile(good.0, good.1, good.2, good.3, page).is_err());
    }
}

#[test]
fn target_partition_and_each_capacity_premise_are_closed() {
    let mut wrong = facts();
    wrong.target = GfxTarget::Gfx942;
    assert_eq!(plan_from_facts(wrong, 4096), Err(Error::TargetMismatch));
    wrong = facts();
    wrong.compute = ComputePartition::Cpx;
    assert_eq!(plan_from_facts(wrong, 4096), Err(Error::PartitionMismatch));
    wrong = facts();
    wrong.memory = MemoryPartition::Nps4;
    assert_eq!(plan_from_facts(wrong, 4096), Err(Error::PartitionMismatch));
    let changes: [fn(&mut TargetFacts); 12] = [
        |f| f.pci ^= 1,
        |f| f.firmware += 1,
        |f| f.sdma_firmware += 1,
        |f| f.geometry.simd = 1216,
        |f| f.geometry.simd_per_cu = 2,
        |f| f.geometry.xcc = 4,
        |f| f.geometry.arrays = 16,
        |f| f.geometry.arrays_per_engine = 2,
        |f| f.geometry.lds_kib = 64,
        |f| f.wavefront = 32,
        |f| f.max_waves = 10,
        |f| f.queues = 32,
    ];
    for change in changes {
        let mut wrong = facts();
        change(&mut wrong);
        assert!(matches!(
            plan_from_facts(wrong, 4096),
            Err(Error::CapacityMismatch { .. })
        ));
    }
    wrong = facts();
    wrong.gpu_id = u64::MAX;
    assert_eq!(plan_from_facts(wrong, 4096), Err(Error::ArithmeticOverflow));
}

#[test]
fn every_supported_ring_power_and_its_neighbors_are_checked() {
    for exponent in 12..=31 {
        let bytes = 1_u32 << exponent;
        assert_eq!(
            plan_from_facts(facts(), bytes)
                .unwrap()
                .ring_mapping_bytes(),
            bytes
        );
        for bad in [bytes - 1, bytes + 1] {
            assert_eq!(
                plan_from_facts(facts(), bad),
                Err(Error::RingSizeUnsupported)
            );
        }
    }
    for bad in [0, 1, 64, 2048, u32::MAX] {
        assert_eq!(
            plan_from_facts(facts(), bad),
            Err(Error::RingSizeUnsupported)
        );
    }
}

#[test]
fn headers_debug_regions_and_all_shadow_pages_are_in_bounds_and_disjoint() {
    let c = plan_from_facts(facts(), 4096).unwrap().context_save();
    assert_eq!((c.xcc_count(), c.shadow_page_count()), (8, 24));
    for xcc in 0..c.xcc_count() {
        let header = c.header(xcc).unwrap();
        assert_eq!(header.bytes(), 40);
        assert_eq!(
            header.offset() + u64::from(header.debug_offset()),
            c.debug_region_offset()
        );
        assert_eq!(u64::from(header.debug_size()), c.debug_region_bytes());
        assert!(
            header.offset() + u64::from(header.bytes())
                <= header.offset() + u64::from(c.control_bytes_per_xcc())
        );
    }
    assert!(c.header(8).is_none());
    assert!(c.header(u32::MAX).is_none());
    let mut end = 0;
    for page in 0..c.shadow_page_count() {
        let offset = c.shadow_page_offset(page).unwrap();
        assert!(offset >= end);
        assert_eq!(offset % 4096, 0);
        end = offset + 4096;
        assert!(end <= c.debug_region_offset());
        let context_offset = offset % u64::from(c.context_bytes_per_xcc());
        assert!(context_offset + 4096 <= u64::from(c.control_bytes_per_xcc()));
    }
    assert!(c.shadow_page_offset(24).is_none());
    assert!(c.shadow_page_offset(u64::MAX).is_none());
    assert_eq!(
        c.debug_region_offset() + c.debug_region_bytes(),
        u64::from(c.mapping_bytes())
    );
}

#[test]
fn kernel_size_precedence_is_independent_and_zero_means_absent() {
    let input = facts().geometry;
    let base = derive_cwsr(input, None, None).unwrap();
    assert_eq!(derive_cwsr(input, Some(0), Some(0)).unwrap(), base);
    let changed_context = derive_cwsr(input, Some(0x15b3000), None).unwrap();
    assert_eq!(changed_context.context_bytes_per_xcc(), 0x15b3000);
    assert_eq!(
        changed_context.control_bytes_per_xcc(),
        base.control_bytes_per_xcc()
    );
    let changed_control = derive_cwsr(input, None, Some(0x2000)).unwrap();
    assert_eq!(
        changed_control.context_bytes_per_xcc(),
        base.context_bytes_per_xcc()
    );
    assert_eq!(changed_control.control_bytes_per_xcc(), 0x2000);
    let both = derive_cwsr(input, Some(0x15b3000), Some(0x4000)).unwrap();
    assert_eq!(
        (both.context_bytes_per_xcc(), both.control_bytes_per_xcc()),
        (0x15b3000, 0x4000)
    );
    for sizes in [
        (Some(1), None),
        (Some(0x1000), None),
        (None, Some(1)),
        (None, Some(0x4000)),
        (Some(u64::MAX), None),
        (None, Some(u64::MAX)),
    ] {
        assert!(derive_cwsr(input, sizes.0, sizes.1).is_err());
    }
}

#[test]
fn geometry_arithmetic_rejects_zero_nondivisible_and_overflowing_inputs() {
    let changes: [fn(&mut GeometryInputs); 10] = [
        |g| g.simd = 0,
        |g| g.simd_per_cu = 0,
        |g| g.xcc = 0,
        |g| g.arrays = 0,
        |g| g.arrays_per_engine = 0,
        |g| g.lds_kib = 0,
        |g| g.simd = 1025,
        |g| g.arrays_per_engine = 3,
        |g| g.simd_per_cu = u64::MAX,
        |g| g.lds_kib = u64::MAX,
    ];
    for change in changes {
        let mut input = facts().geometry;
        change(&mut input);
        assert!(derive_cwsr(input, None, None).is_err());
    }
    assert_eq!(align(u64::MAX, 4096), Err(Error::ArithmeticOverflow));
    assert_eq!(mul(u64::MAX, 2), Err(Error::ArithmeticOverflow));
    assert!(align(4096, 0).is_err());
    assert!(align(4096, 3).is_err());
    assert_eq!(
        narrow(u64::from(u32::MAX) + 1),
        Err(Error::ArithmeticOverflow)
    );
    assert_eq!(narrow(u64::from(u32::MAX)), Ok(u32::MAX));
    assert_eq!(
        derive_cwsr(facts().geometry, Some(1 << 32), None),
        Err(Error::ArithmeticOverflow)
    );
}

#[test]
fn derived_geometry_and_header_bytes_match_the_independent_c_oracle() {
    let c = plan_from_facts(facts(), 4096).unwrap().context_save();
    let mut output = String::new();
    writeln!(
        output,
        "geometry cu={} waves={} control={} workgroup={} context={} debug={} mapping={}",
        c.cu_per_xcc(),
        c.waves_per_xcc(),
        c.control_bytes_per_xcc(),
        c.workgroup_bytes_per_xcc(),
        c.context_bytes_per_xcc(),
        c.debug_bytes_per_xcc(),
        c.mapping_bytes()
    )
    .unwrap();
    for xcc in 0..c.xcc_count() {
        let header = c.header(xcc).unwrap();
        // Test data only: no production header writer or native address authority.
        let mut bytes = [0_u8; 40];
        bytes[16..20].copy_from_slice(&header.debug_offset().to_le_bytes());
        bytes[20..24].copy_from_slice(&header.debug_size().to_le_bytes());
        bytes[24..32].copy_from_slice(&0x1122334455667788_u64.to_le_bytes());
        bytes[32..36].copy_from_slice(&23_u32.to_le_bytes());
        write!(
            output,
            "header {xcc} offset={} debug_offset={} debug_size={} bytes=",
            header.offset(),
            header.debug_offset(),
            header.debug_size()
        )
        .unwrap();
        for byte in bytes {
            write!(output, "{byte:02x}").unwrap();
        }
        output.push('\n');
    }
    for page in 0..c.shadow_page_count() {
        writeln!(
            output,
            "shadow {page} offset={}",
            c.shadow_page_offset(page).unwrap()
        )
        .unwrap();
    }
    assert_eq!(output, include_str!("oracle.txt"));
}

#[test]
#[ignore = "requires the explicitly selected reviewed MI350 host; sysfs observations only"]
fn live_gfx950_plans_all_eight_devices_without_native_authority() {
    let snapshot =
        crate::topology::discover_default_topology_for_target(GfxTarget::Gfx950).unwrap();
    assert_eq!(snapshot.topology().gpu_nodes().len(), 8);
    for gpu in snapshot.topology().gpu_nodes() {
        let plan = plan_gfx950_aql_queue_resources_v1(&snapshot, gpu.unique_id(), 4096).unwrap();
        let c = plan.context_save();
        assert_eq!(c.mapping_bytes(), 181829632);
        assert_eq!(c.shadow_page_count(), 24);
        for xcc in 0..c.xcc_count() {
            let header = c.header(xcc).unwrap();
            assert_eq!(
                header.offset() + u64::from(header.debug_offset()),
                c.debug_region_offset()
            );
            assert!(header.offset() + u64::from(header.bytes()) < c.debug_region_offset());
        }
        for page in 0..c.shadow_page_count() {
            let offset = c.shadow_page_offset(page).unwrap();
            assert_eq!(offset % 4096, 0);
            assert!(offset + 4096 <= c.debug_region_offset());
        }
        assert_eq!(
            c.debug_region_offset() + c.debug_region_bytes(),
            u64::from(c.mapping_bytes())
        );
        assert!(crate::plan_gfx942_aql_queue_resources(&snapshot, gpu.unique_id(), 4096).is_err());
        println!(
            "gfx950-plan node={} gpu_id={} uid={} ring={} ctl={} context={} debug={} mapping={} shadow_pages={} profile={}",
            gpu.node_id(),
            plan.gpu_id(),
            plan.unique_id(),
            plan.ring_mapping_bytes(),
            c.control_bytes_per_xcc(),
            c.context_bytes_per_xcc(),
            c.debug_bytes_per_xcc(),
            c.mapping_bytes(),
            c.shadow_page_count(),
            plan.profile_sha256()
        );
    }
}
