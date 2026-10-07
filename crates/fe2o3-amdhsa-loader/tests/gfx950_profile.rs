use std::{env, fs, io::Cursor};

use fe2o3_amdhsa_loader::{
    AdmittedProfile, GFX950_LOADER_PROFILE_ID, KernelClosureError, LOADER_PROFILE_ID, PlanError,
    ValidatedKernelEnvelope, validate, validate_owned,
};
use rmpv::{Value, decode::read_value, encode::write_value};
use sha2::{Digest, Sha256};

const GFX942: AdmittedProfile = AdmittedProfile::Gfx942XnackOffCov6;
const GFX950: AdmittedProfile = AdmittedProfile::Gfx950XnackOffCov6;
const KERNEL: &str = "fill_write_only";
const ORIGINAL: &[u8] =
    include_bytes!("../../fe2o3-hsaco/tests/fixtures/rust-fill-write-only-gfx942/kernel.hsaco");

// Only envelope/metadata/descriptor test data. Retagging this gfx942 fixture does not
// establish gfx950 instruction semantics, proof authority, or native executability.
fn synthetic_gfx950() -> Vec<u8> {
    let mut bytes = ORIGINAL.to_vec();
    change_metadata(&mut bytes, GFX942, |root| {
        *field(root, "amdhsa.target") = Value::from("amdgcn-amd-amdhsa--gfx950:xnack-");
    });
    write_u32(&mut bytes, 48, 0x64f);
    bytes
}

fn field<'a>(value: &'a mut Value, key: &str) -> &'a mut Value {
    let Value::Map(fields) = value else {
        panic!("expected metadata map");
    };
    &mut fields
        .iter_mut()
        .find(|(name, _)| name.as_str() == Some(key))
        .expect("fixture metadata field")
        .1
}

fn change_metadata(bytes: &mut [u8], profile: AdmittedProfile, edit: impl FnOnce(&mut Value)) {
    let envelope = validate(bytes, profile).unwrap();
    let range = envelope.plan().metadata_note();
    let mut cursor = Cursor::new(envelope.metadata_descriptor());
    let mut document = read_value(&mut cursor).unwrap();
    assert_eq!(cursor.position(), range.byte_len());
    edit(&mut document);
    let mut encoded = Vec::new();
    write_value(&mut encoded, &document).unwrap();
    assert_eq!(encoded.len() as u64, range.byte_len());
    let start = range.file_offset() as usize;
    bytes[start..start + encoded.len()].copy_from_slice(&encoded);
}

fn set_kernel_field(bytes: &mut [u8], key: &str, value: u64) {
    change_metadata(bytes, GFX950, |root| {
        let Value::Array(kernels) = field(root, "amdhsa.kernels") else {
            panic!("expected kernel array");
        };
        assert_eq!(kernels.len(), 1);
        *field(&mut kernels[0], key) = Value::from(value);
    });
}

fn descriptor_offset(bytes: &[u8]) -> usize {
    validate(bytes, GFX950)
        .unwrap()
        .bind_kernel(KERNEL)
        .unwrap()
        .selected_binding()
        .descriptor_file_offset() as usize
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn descriptor_rejects(bytes: &[u8]) {
    assert!(matches!(
        validate(bytes, GFX950).unwrap().bind_kernel(KERNEL),
        Err(KernelClosureError::Inspection(_))
    ));
}

#[test]
fn separate_profiles_reject_cross_target_and_feature_substitution() {
    assert_eq!(GFX942.profile_id(), LOADER_PROFILE_ID);
    assert_eq!(GFX950.profile_id(), GFX950_LOADER_PROFILE_ID);
    assert_ne!(GFX942.profile_id(), GFX950.profile_id());
    assert_eq!(GFX950.processor(), "gfx950");
    assert_eq!(GFX950.target(), "gfx950:xnack-");
    assert_eq!(GFX950.elf_flags(), 0x64f);
    let bytes = synthetic_gfx950();
    assert!(matches!(
        validate(ORIGINAL, GFX950),
        Err(PlanError::UnsupportedElfFlags(0x64c))
    ));
    assert!(matches!(
        validate(&bytes, GFX942),
        Err(PlanError::UnsupportedElfFlags(0x64f))
    ));
    for xnack in [0, 0x100, 0x200, 0x300] {
        for sramecc in [0, 0x400, 0x800, 0xc00] {
            let flags = 0x4f | xnack | sramecc;
            let mut changed = bytes.clone();
            write_u32(&mut changed, 48, flags);
            assert_eq!(validate(&changed, GFX950).is_ok(), flags == 0x64f);
        }
    }
    let mut reserved = bytes.clone();
    write_u32(&mut reserved, 48, 0x1_064f);
    assert!(matches!(
        validate(&reserved, GFX950),
        Err(PlanError::UnsupportedElfFlags(0x1_064f))
    ));
    for version in [0, 1, 2, 3, 5, u8::MAX] {
        let mut wrong_cov = bytes.clone();
        wrong_cov[8] = version;
        assert!(
            matches!(validate(&wrong_cov, GFX950), Err(PlanError::UnsupportedAbiVersion(actual)) if actual == version)
        );
    }
}

#[test]
fn metadata_must_match_the_selected_profile_and_cov6_schema() {
    for target in [
        "amdgcn-amd-amdhsa--gfx942:xnack-",
        "amdgcn-amd-amdhsa--gfx950:xnack+",
    ] {
        let mut bytes = synthetic_gfx950();
        change_metadata(&mut bytes, GFX950, |root| {
            *field(root, "amdhsa.target") = Value::from(target);
        });
        assert!(
            validate(&bytes, GFX950)
                .unwrap()
                .bind_kernel(KERNEL)
                .is_err()
        );
    }
    let mut old_schema = synthetic_gfx950();
    change_metadata(&mut old_schema, GFX950, |root| {
        *field(root, "amdhsa.version") = Value::Array(vec![Value::from(1), Value::from(1)]);
    });
    assert!(
        validate(&old_schema, GFX950)
            .unwrap()
            .bind_kernel(KERNEL)
            .is_err()
    );
    let mut flags_only = ORIGINAL.to_vec();
    write_u32(&mut flags_only, 48, 0x64f);
    assert!(
        validate(&flags_only, GFX950)
            .unwrap()
            .bind_kernel(KERNEL)
            .is_err()
    );
}

#[test]
fn gfx950_closure_materializes_and_owned_validation_retains_its_profile() {
    let bytes = synthetic_gfx950();
    let object_pointer = bytes.as_ptr();
    let owned = validate_owned(bytes, GFX950).unwrap();
    assert_eq!(owned.bytes().as_ptr(), object_pointer);
    assert_eq!(owned.validation_passes(), 1);
    let selected = owned.bind_kernel(KERNEL).unwrap();
    let kernel = selected.validated();
    assert_eq!(kernel.envelope().plan().profile(), GFX950);
    assert_eq!(kernel.selected_kernel().name(), KERNEL);
    assert_eq!(kernel.resources().wavefront_size(), 64);
    assert_eq!(kernel.resources().kernarg_segment_size(), 16);
    assert_eq!(kernel.resources().descriptor().compute_pgm_rsrc2(), 0x84);
    let mut image = vec![0xa5; kernel.envelope().materialization().image_len() as usize];
    kernel.materialize_into(&mut image).unwrap();
    let binding = kernel.selected_binding();
    let offset = (binding.entry_address() - kernel.envelope().plan().image_start()) as usize;
    assert_eq!(
        &image[offset..offset + kernel.entry_bytes().len()],
        kernel.entry_bytes()
    );
    assert_eq!(owned.validation_passes(), 1);
}

#[test]
fn gfx950_descriptor_reserved_fields_and_architected_scratch_reject() {
    let original = synthetic_gfx950();
    let descriptor = descriptor_offset(&original);
    for (offset, bit) in [
        (12, 1),
        (44, 1 << 6),
        (44, 1 << 17),
        (48, 1 << 10),
        (48, 1 << 27),
        (48, 1 << 29),
        (52, 1 << 6),
        (52, 1 << 15),
        (56, 1),
        (56, 1 << 5),
        (56, 1 << 7),
        (56, 1 << 10),
        (60, 1),
    ] {
        let mut bytes = original.clone();
        let position = descriptor + offset;
        let value = u32::from_le_bytes(bytes[position..position + 4].try_into().unwrap());
        write_u32(&mut bytes, position, value | bit);
        descriptor_rejects(&bytes);
    }
    let mut preload = original;
    preload[descriptor + 58] = 1;
    descriptor_rejects(&preload);
}

#[test]
fn gfx950_register_capacity_and_accumulator_offset_are_checked() {
    let original = synthetic_gfx950();
    let descriptor = descriptor_offset(&original);
    let mut vgpr_boundary = original.clone();
    set_kernel_field(&mut vgpr_boundary, ".vgpr_count", 8);
    write_u32(&mut vgpr_boundary, descriptor + 44, 1);
    validate(&vgpr_boundary, GFX950)
        .unwrap()
        .bind_kernel(KERNEL)
        .unwrap();
    let mut vgpr_overflow = original.clone();
    set_kernel_field(&mut vgpr_overflow, ".vgpr_count", 9);
    write_u32(&mut vgpr_overflow, descriptor + 44, 2);
    descriptor_rejects(&vgpr_overflow);
    let mut wrong_accum = vgpr_boundary;
    write_u32(&mut wrong_accum, descriptor + 44, 0);
    descriptor_rejects(&wrong_accum);
    let rsrc1 = u32::from_le_bytes(
        original[descriptor + 48..descriptor + 52]
            .try_into()
            .unwrap(),
    );
    let mut sgpr_overflow = original.clone();
    let sgpr_capacity = (((rsrc1 >> 6) & 0xf) + 1) * 8;
    set_kernel_field(
        &mut sgpr_overflow,
        ".sgpr_count",
        u64::from(sgpr_capacity + 1),
    );
    descriptor_rejects(&sgpr_overflow);
    let mut sgpr_blocks = original.clone();
    write_u32(
        &mut sgpr_blocks,
        descriptor + 48,
        (rsrc1 & !(0xf << 6)) | (14 << 6),
    );
    descriptor_rejects(&sgpr_blocks);
    for (vgprs, agprs) in [(3, 4), (6, 1)] {
        let mut bytes = original.clone();
        set_kernel_field(&mut bytes, ".vgpr_count", vgprs);
        set_kernel_field(&mut bytes, ".agpr_count", agprs);
        descriptor_rejects(&bytes);
    }
}

#[test]
fn profile_identity_is_distinct_and_gfx942_digest_is_unchanged() {
    let old = validate(ORIGINAL, GFX942)
        .unwrap()
        .bind_kernel(KERNEL)
        .unwrap();
    // Captured from the pre-change loader rlib, before adding the gfx950 variant.
    assert_eq!(
        old.identity_inputs().closure_sha256(),
        [
            0x49, 0xfe, 0x32, 0x18, 0xf7, 0xd4, 0xb8, 0x2a, 0x97, 0x5e, 0x13, 0x7a, 0xf5, 0x28,
            0x7a, 0xa1, 0xb8, 0xa3, 0xd3, 0x1d, 0x7a, 0x5a, 0xf8, 0x5a, 0x13, 0x31, 0xc3, 0xfa,
            0x93, 0x1d, 0x9d, 0x64,
        ]
    );
    assert_eq!(
        old.identity_inputs().closure_sha256(),
        closure_digest(&old, LOADER_PROFILE_ID)
    );
    let bytes = synthetic_gfx950();
    let new = validate(&bytes, GFX950)
        .unwrap()
        .bind_kernel(KERNEL)
        .unwrap();
    assert_eq!(
        new.identity_inputs().closure_sha256(),
        closure_digest(&new, GFX950_LOADER_PROFILE_ID)
    );
    assert_ne!(
        new.identity_inputs().closure_sha256(),
        closure_digest(&new, LOADER_PROFILE_ID)
    );
}

fn closure_digest(kernel: &ValidatedKernelEnvelope<'_>, profile: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    let identity = kernel.identity_inputs();
    let binding = kernel.selected_binding();
    let metadata = kernel.envelope().plan().metadata_note();
    let mut update = |label: &[u8], value: &[u8]| {
        hasher.update((label.len() as u64).to_le_bytes());
        hasher.update(label);
        hasher.update((value.len() as u64).to_le_bytes());
        hasher.update(value);
    };
    update(b"domain", b"fe2o3.amdhsa.loaded-kernel-identity-inputs.v1");
    update(b"loader-profile", profile.as_bytes());
    update(
        b"relocation-policy",
        kernel.relocation_evidence().policy_id().as_bytes(),
    );
    update(b"object-sha256", &identity.object_sha256());
    update(
        b"object-length",
        &kernel.envelope().input_len().to_le_bytes(),
    );
    update(b"metadata-sha256", &identity.metadata_sha256());
    update(
        b"metadata-file-offset",
        &metadata.file_offset().to_le_bytes(),
    );
    update(b"metadata-length", &metadata.byte_len().to_le_bytes());
    update(
        b"kernel-index",
        &(kernel.selected_kernel_index() as u64).to_le_bytes(),
    );
    update(b"kernel-name", kernel.selected_kernel().name().as_bytes());
    update(
        b"kernel-symbol",
        kernel.selected_kernel().symbol().as_bytes(),
    );
    update(b"descriptor-sha256", &identity.descriptor_sha256());
    update(
        b"descriptor-file-offset",
        &binding.descriptor_file_offset().to_le_bytes(),
    );
    update(
        b"descriptor-address",
        &binding.descriptor_address().to_le_bytes(),
    );
    update(b"entry-sha256", &identity.entry_sha256());
    update(
        b"entry-file-offset",
        &binding.entry_file_offset().to_le_bytes(),
    );
    update(b"entry-address", &binding.entry_address().to_le_bytes());
    update(b"entry-length", &binding.entry_size().to_le_bytes());
    hasher.finalize().into()
}

#[test]
#[ignore = "requires FE2O3_TEST_GFX950_COV6 and FE2O3_TEST_GFX950_KERNEL; loader evidence only"]
fn closes_real_gfx950_cov6_envelope_without_execution_authority() {
    let bytes =
        fs::read(env::var("FE2O3_TEST_GFX950_COV6").expect("set FE2O3_TEST_GFX950_COV6")).unwrap();
    let name = env::var("FE2O3_TEST_GFX950_KERNEL").expect("set FE2O3_TEST_GFX950_KERNEL");
    let kernel = validate(&bytes, GFX950)
        .unwrap()
        .bind_kernel(&name)
        .unwrap();
    assert_eq!(kernel.envelope().plan().profile(), GFX950);
    assert_eq!(kernel.selected_kernel().name(), name);
    assert_eq!(kernel.resources().wavefront_size(), 64);
    assert_eq!(
        kernel.identity_inputs().closure_sha256(),
        closure_digest(&kernel, GFX950_LOADER_PROFILE_ID)
    );
    assert!(matches!(
        validate(&bytes, GFX942),
        Err(PlanError::UnsupportedElfFlags(0x64f))
    ));
    let mut image = vec![0xa5; kernel.envelope().materialization().image_len() as usize];
    kernel.materialize_into(&mut image).unwrap();
}
