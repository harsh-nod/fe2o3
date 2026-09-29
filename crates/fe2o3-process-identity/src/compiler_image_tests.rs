use super::*;
use std::{
    convert::Infallible,
    ffi::OsStr,
    fs::{self, OpenOptions, Permissions},
    io::{Seek, SeekFrom, Write},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Image {
    path: PathBuf,
    file: File,
}

impl Image {
    fn new(bytes: &[u8]) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "fe2o3-image-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(bytes).unwrap();
        file.set_permissions(Permissions::from_mode(0o600)).unwrap();
        Self { path, file }
    }
}

impl Drop for Image {
    fn drop(&mut self) {
        fs::remove_file(&self.path).unwrap();
    }
}

fn allow(_: usize) -> std::result::Result<(), Infallible> {
    Ok(())
}

#[test]
fn path_and_descriptor_hashes_match_across_chunk_boundaries_without_moving_offset() {
    for length in [
        1,
        HASH_CHUNK_BYTES - 1,
        HASH_CHUNK_BYTES,
        HASH_CHUNK_BYTES + 1,
        2 * HASH_CHUNK_BYTES + 7,
    ] {
        let bytes = (0..length).map(|i| (i % 251) as u8).collect::<Vec<_>>();
        let mut image = Image::new(&bytes);
        image.file.seek(SeekFrom::Start(1)).unwrap();
        let expected: [u8; 32] = Sha256::digest(&bytes).into();
        let mut charges = Vec::new();
        assert_eq!(
            measure_compiler_image_sha256_v1(
                &image.path,
                CompilerImageRoleV1::CodegenBackend,
                |n| {
                    charges.push(n);
                    allow(n)
                }
            )
            .unwrap(),
            expected
        );
        assert_eq!(
            charges,
            [
                8,
                image.path.as_os_str().len(),
                IO_WORK,
                IO_WORK,
                (length.div_ceil(HASH_CHUNK_BYTES) + 2) * IO_WORK + length
            ]
        );
        assert_eq!(
            measure_compiler_image_file_sha256_v1(
                &image.file,
                CompilerImageRoleV1::CodegenBackend,
                allow
            )
            .unwrap(),
            expected
        );
        assert_eq!(image.file.stream_position().unwrap(), 1);
    }
}

#[test]
fn executable_role_requires_execute_bits_but_backend_does_not() {
    let image = Image::new(b"image");
    assert!(matches!(
        measure_compiler_image_sha256_v1(&image.path, CompilerImageRoleV1::Executable, allow),
        Err(Failure::Invalid(_))
    ));
    image
        .file
        .set_permissions(Permissions::from_mode(0o700))
        .unwrap();
    assert!(
        measure_compiler_image_sha256_v1(&image.path, CompilerImageRoleV1::Executable, allow)
            .is_ok()
    );
}

#[test]
fn measurement_work_quote_matches_exact_and_one_short_budgets() {
    for length in [1, HASH_CHUNK_BYTES, HASH_CHUNK_BYTES + 1] {
        let image = Image::new(&vec![7; length]);
        let quote = compiler_image_measurement_work_v1(length as u64).unwrap();
        for available in [quote - 1, quote] {
            let mut remaining = available;
            let result = measure_compiler_image_file_sha256_v1(
                &image.file,
                CompilerImageRoleV1::CodegenBackend,
                |work| {
                    remaining = remaining.checked_sub(work).ok_or("work exhausted")?;
                    Ok::<_, &'static str>(())
                },
            );
            if available == quote {
                assert!(result.is_ok());
                assert_eq!(remaining, 0);
            } else {
                assert!(matches!(result, Err(Failure::Work("work exhausted"))));
            }
        }
    }
}

#[test]
fn measurement_work_quote_rejects_invalid_extents_and_arithmetic_overflow() {
    for length in [0, MAX_EXECUTABLE_BYTES_V3 + 1, u64::MAX] {
        assert_eq!(compiler_image_measurement_work_v1(length), None);
    }
    assert!(compiler_image_measurement_work_v1(MAX_EXECUTABLE_BYTES_V3).is_some());
    assert_eq!(measurement_payload_work(usize::MAX), None);
}

#[test]
fn each_prepaid_boundary_propagates_denial_without_further_work() {
    let image = Image::new(b"image");
    for stop in 1..=5 {
        let mut calls = 0;
        let result = measure_compiler_image_sha256_v1(
            &image.path,
            CompilerImageRoleV1::CodegenBackend,
            |_| {
                calls += 1;
                if calls == stop { Err(stop) } else { Ok(()) }
            },
        );
        assert!(matches!(result, Err(Failure::Work(n)) if n == stop));
        assert_eq!(calls, stop);
    }
    let missing = image.path.with_extension("absent");
    for stop in 1..=3 {
        let mut calls = 0;
        let result =
            measure_compiler_image_sha256_v1(&missing, CompilerImageRoleV1::CodegenBackend, |_| {
                calls += 1;
                if calls == stop { Err(stop) } else { Ok(()) }
            });
        assert!(matches!(result, Err(Failure::Work(n)) if n == stop));
    }
    assert!(matches!(
        measure_compiler_image_sha256_v1(&missing, CompilerImageRoleV1::CodegenBackend, allow),
        Err(Failure::Io(_))
    ));
}

#[test]
fn malformed_paths_refuse_before_open() {
    for (bytes, expected_calls) in [
        (Vec::new(), 1),
        (vec![b'x'; MAX_PATH + 1], 1),
        (b"/invalid\0path".to_vec(), 2),
    ] {
        let mut calls = 0;
        let result = measure_compiler_image_sha256_v1(
            Path::new(OsStr::from_bytes(&bytes)),
            CompilerImageRoleV1::CodegenBackend,
            |n| {
                calls += 1;
                allow(n)
            },
        );
        assert!(matches!(result, Err(Failure::Invalid(_))));
        assert_eq!(calls, expected_calls);
    }
}

#[test]
fn nonregular_empty_and_oversized_files_refuse_before_payload_work() {
    let empty = Image::new(b"");
    let oversized = Image::new(b"x");
    oversized.file.set_len(MAX_EXECUTABLE_BYTES_V3 + 1).unwrap();
    for file in [
        &empty.file,
        &oversized.file,
        &File::open("/dev/null").unwrap(),
        &File::open(std::env::temp_dir()).unwrap(),
    ] {
        let mut charges = Vec::new();
        assert!(matches!(
            measure_compiler_image_file_sha256_v1(file, CompilerImageRoleV1::CodegenBackend, |n| {
                charges.push(n);
                allow(n)
            }),
            Err(Failure::Invalid(_))
        ));
        assert_eq!(charges, [IO_WORK]);
    }
}

#[test]
fn mutation_between_snapshot_and_payload_refuses_without_a_digest() {
    for mutation in 0..3 {
        let image = Image::new(b"compiler image");
        let mut calls = 0;
        let result = measure_compiler_image_file_sha256_v1(
            &image.file,
            CompilerImageRoleV1::CodegenBackend,
            |n| {
                calls += 1;
                if calls == 2 {
                    match mutation {
                        0 => image.file.set_len(1).unwrap(),
                        1 => image.file.set_len(100).unwrap(),
                        _ => image
                            .file
                            .set_permissions(Permissions::from_mode(0o700))
                            .unwrap(),
                    }
                }
                allow(n)
            },
        );
        let expected = [
            "image read was short",
            "image grew while hashing",
            "image changed while hashing",
        ];
        assert!(matches!(result, Err(Failure::Invalid(s)) if s == expected[mutation]));
        assert_eq!(calls, 2);
    }
}

#[test]
fn descriptor_measurement_survives_path_replacement_and_preserves_borrowed_owner() {
    let mut image = Image::new(b"pinned image");
    fs::remove_file(&image.path).unwrap();
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&image.path)
        .unwrap()
        .write_all(b"different image")
        .unwrap();
    let pinned: [u8; 32] = Sha256::digest(b"pinned image").into();
    assert_eq!(
        measure_compiler_image_file_sha256_v1(
            &image.file,
            CompilerImageRoleV1::CodegenBackend,
            allow
        )
        .unwrap(),
        pinned
    );
    assert_ne!(
        measure_compiler_image_sha256_v1(&image.path, CompilerImageRoleV1::CodegenBackend, allow)
            .unwrap(),
        pinned
    );
    let offset = image.file.stream_position().unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        measure_compiler_image_file_sha256_v1(
            &image.file,
            CompilerImageRoleV1::CodegenBackend,
            |_| -> std::result::Result<(), Infallible> { panic!("caller refuses") },
        )
    }));
    assert!(result.is_err());
    assert_eq!(image.file.stream_position().unwrap(), offset);
}
