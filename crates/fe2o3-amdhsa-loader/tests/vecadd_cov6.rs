use std::{env, fs};

use fe2o3_amdhsa_loader::{
    AdmittedProfile, UnboundGpuF32SliceV1, VECADD_COV6_ARTIFACT_SHA256, VecaddCov6BindError,
    VecaddCov6KernargInputsV1, validate,
};
use sha2::{Digest, Sha256};

const PROFILE: AdmittedProfile = AdmittedProfile::Gfx942XnackOffCov6;

#[test]
#[ignore = "requires FE2O3_TEST_VECADD_COV6 to name the exact durable gfx942 COV6 vecadd"]
fn exact_real_artifact_binds_address_plan_and_kernarg_oracle() {
    let bytes = real_vecadd();
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(&bytes)),
        VECADD_COV6_ARTIFACT_SHA256
    );
    let vecadd = validate(&bytes, PROFILE)
        .unwrap()
        .bind_kernel("vecadd")
        .unwrap()
        .bind_vecadd_cov6()
        .unwrap();
    let address = vecadd.address_plan();
    assert_eq!(address.link_time_image_base(), 0);
    assert_eq!(address.image_byte_len(), 0x3000);
    assert_eq!(address.descriptor_offset(), 0x9c0);
    assert_eq!(address.descriptor_byte_len(), 64);
    assert_eq!(address.entry_offset(), 0x1a00);
    assert_eq!(address.entry_byte_len(), 188);

    let kernarg = vecadd
        .encode_kernarg(VecaddCov6KernargInputsV1::new(
            UnboundGpuF32SliceV1::new(0x1_0000, 1024),
            UnboundGpuF32SliceV1::new(0x2_0000, 1024),
            UnboundGpuF32SliceV1::new(0x3_0000, 1024),
        ))
        .unwrap();
    assert_eq!(kernarg.block_counts(), [4, 1, 1]);
    assert_eq!(kernarg.aql_grid_size(), [1024, 1, 1]);
    assert_eq!(oracle_u64(kernarg.as_bytes(), 0), 0x1_0000);
    assert_eq!(oracle_u64(kernarg.as_bytes(), 8), 1024);
    assert_eq!(oracle_u64(kernarg.as_bytes(), 16), 0x2_0000);
    assert_eq!(oracle_u64(kernarg.as_bytes(), 24), 1024);
    assert_eq!(oracle_u64(kernarg.as_bytes(), 32), 0x3_0000);
    assert_eq!(oracle_u64(kernarg.as_bytes(), 40), 1024);
    assert_eq!(oracle_u32(kernarg.as_bytes(), 48), 4);
    assert_eq!(oracle_u16(kernarg.as_bytes(), 60), 256);
    assert_eq!(oracle_u16(kernarg.as_bytes(), 112), 1);
    assert!(kernarg.as_bytes()[114..].iter().all(|byte| *byte == 0));

    let mut image = vec![0xa5; address.image_byte_len() as usize];
    vecadd.materialize_into(&mut image).unwrap();
    assert_eq!(
        &image[address.descriptor_offset() as usize
            ..(address.descriptor_offset() + address.descriptor_byte_len()) as usize],
        vecadd.closure().descriptor_bytes()
    );
}

#[test]
#[ignore = "requires FE2O3_TEST_VECADD_COV6 to name the exact durable gfx942 COV6 vecadd"]
fn hostile_metadata_entry_and_truncation_substitution_fail_closed() {
    let bytes = real_vecadd();

    let mut metadata_drift = bytes.clone();
    let name = unique_subslice_offset(&metadata_drift, b"arg0.data");
    metadata_drift[name + 3] = b'x';
    let result = validate(&metadata_drift, PROFILE)
        .unwrap()
        .bind_kernel("vecadd")
        .unwrap()
        .bind_vecadd_cov6();
    assert!(matches!(
        result,
        Err(VecaddCov6BindError::ExplicitArgument {
            index: 0,
            field: "name"
        })
    ));

    let closure = validate(&bytes, PROFILE)
        .unwrap()
        .bind_kernel("vecadd")
        .unwrap();
    let entry = closure.selected_binding().entry_file_offset() as usize;
    drop(closure);
    let mut entry_drift = bytes.clone();
    entry_drift[entry] ^= 1;
    let result = validate(&entry_drift, PROFILE)
        .unwrap()
        .bind_kernel("vecadd")
        .unwrap()
        .bind_vecadd_cov6();
    assert!(matches!(
        result,
        Err(VecaddCov6BindError::ArtifactIdentity { .. })
    ));

    assert!(validate(&bytes[..bytes.len() - 1], PROFILE).is_err());
}

fn real_vecadd() -> Vec<u8> {
    let path = env::var("FE2O3_TEST_VECADD_COV6").expect("set FE2O3_TEST_VECADD_COV6");
    fs::read(path).unwrap()
}

fn unique_subslice_offset(bytes: &[u8], needle: &[u8]) -> usize {
    let matches = bytes
        .windows(needle.len())
        .enumerate()
        .filter_map(|(offset, candidate)| (candidate == needle).then_some(offset))
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "expected one exact subslice occurrence");
    matches[0]
}

fn oracle_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn oracle_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn oracle_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}
