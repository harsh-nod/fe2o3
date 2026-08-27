use std::{env, fs};

use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_kfd::{Gfx942KfdDispatchBufferV1, Gfx942KfdDispatchPointerFixupV1};
use fe2o3_runtime::{Gfx942RuntimeDispatchInputsV1, prepare_gfx942_runtime_dispatch_v1};
use sha2::{Digest, Sha256};

const HSACO_ENV: &str = "FE2O3_TEST_SOURCE_AUTH_LDS_GFX942_HSACO";
const EXPECTED_HSACO_SHA256: [u8; 32] = [
    0xab, 0x6b, 0xda, 0x1e, 0x8a, 0xf0, 0x5b, 0x61, 0xc2, 0x27, 0x53, 0x38, 0x2e, 0x75, 0xdd, 0x6a,
    0x99, 0x52, 0xdb, 0x8e, 0x59, 0x8e, 0xaa, 0xc3, 0xcb, 0x57, 0x69, 0x86, 0x3a, 0x61, 0x8e, 0xd0,
];

#[test]
#[ignore = "requires the exact source-authenticated gfx942 LDS reduction HSACO"]
fn prepares_exact_source_authenticated_lds_reduction_without_native_work() {
    let hsaco = fs::read(env::var(HSACO_ENV).expect("set exact LDS HSACO path")).unwrap();
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(&hsaco)),
        EXPECTED_HSACO_SHA256
    );
    let mut explicit = vec![0_u8; 32];
    explicit[8..16].copy_from_slice(&64_u64.to_le_bytes());
    explicit[24..32].copy_from_slice(&1_u64.to_le_bytes());
    let input = (1_i32..=64).flat_map(i32::to_le_bytes).collect::<Vec<_>>();
    let output = 0x55aa_33cc_i32.to_le_bytes().to_vec();
    let prepared = prepare_gfx942_runtime_dispatch_v1(
        &hsaco,
        "lds_publish_read_reduce_i32_v1",
        Gfx942RuntimeDispatchInputsV1::new(
            explicit,
            vec![
                Gfx942KfdDispatchBufferV1::new(input).unwrap(),
                Gfx942KfdDispatchBufferV1::new(output).unwrap(),
            ],
            vec![
                Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 4),
                Gfx942KfdDispatchPointerFixupV1::new(16, 1, 0, 4),
            ],
            AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            0,
            5_000,
        ),
    )
    .unwrap();

    assert_eq!(prepared.identity().object_sha256(), EXPECTED_HSACO_SHA256);
    assert_eq!(prepared.kernel_name(), "lds_publish_read_reduce_i32_v1");
    assert_eq!(prepared.descriptor_offset() % 64, 0);
    assert_eq!(prepared.static_group_segment_bytes(), 256);
    assert_eq!(prepared.dynamic_group_segment_bytes(), 0);
    assert_eq!(prepared.packet_group_segment_bytes(), 256);
    let _request = prepared.into_unchecked_kfd_request();
}
