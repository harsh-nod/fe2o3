//! Synthetic inert inventory tests. These do not establish approval or execution.
use fe2o3_build_authority::*;
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

type Error = CompilerRuntimeManifestErrorV1<u8>;
type Manifest = CompilerRuntimeManifestV1;
type Role = CompilerRuntimeRoleV1;
const HEADER: usize = COMPILER_RUNTIME_MANIFEST_HEADER_BYTES_V1;
const ENTRY: usize = COMPILER_RUNTIME_MANIFEST_ENTRY_BYTES_V1;
fn closure() -> CompilerClosureV2 {
    CompilerClosureV2::new([1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]).unwrap()
}
fn entries() -> [CompilerRuntimeEntryV1<'static>; 6] {
    [
        (Role::ProofExecutorHelper, "bin/helper", [7; 32]),
        (Role::Rustc, "bin/rustc", [4; 32]),
        (Role::ElfInterpreter, "lib/ld.so", [8; 32]),
        (Role::SharedLibrary, "lib/libc.so", [9; 32]),
        (Role::CodegenBackend, "lib/libfe2o3.so", [6; 32]),
        (Role::Fe2o3ProcMacro, "lib/libfe2o3_macros.so", [10; 32]),
    ]
    .map(|(role, path, sha256)| CompilerRuntimeEntryV1 {
        role,
        path,
        length: 17,
        sha256,
    })
}
fn fixture() -> Manifest {
    Manifest::new(closure(), [11; 32], &entries(), |_| Ok::<_, u8>(())).unwrap()
}
fn decode(bytes: &[u8]) -> Result<Manifest, Error> {
    Manifest::decode(bytes, |_| Ok(()))
}
fn reseal(bytes: &mut [u8]) {
    let end = bytes.len() - 32;
    let mut hash = Sha256::new();
    hash.update(COMPILER_RUNTIME_MANIFEST_IDENTITY_DOMAIN_V1);
    hash.update((end as u64).to_le_bytes());
    hash.update(&bytes[..end]);
    bytes[end..].copy_from_slice(&hash.finalize());
}
fn mutated(at: usize, value: &[u8]) -> Vec<u8> {
    let mut bytes = fixture().canonical_bytes().to_vec();
    bytes[at..at + value.len()].copy_from_slice(value);
    reseal(&mut bytes);
    bytes
}

#[test]
fn exact_canonical_inventory_is_distinct_from_six_pins_and_inert() {
    let manifest = fixture();
    assert_eq!(manifest.canonical_bytes().len(), HEADER + 6 * ENTRY + 32);
    assert_eq!(
        &manifest.canonical_bytes()[..12],
        b"F2CRM1\0\0\x01\0\x20\x01"
    );
    assert_eq!(manifest.compiler_closure(), closure());
    assert_eq!(manifest.proof_runtime_identity(), &[11; 32]);
    assert_eq!(manifest.total_file_bytes(), 102);
    assert_eq!(manifest.entries().collect::<Vec<_>>(), entries());
    assert_eq!(decode(manifest.canonical_bytes()).unwrap(), manifest);
    assert!(!manifest.grants_authority());
    let mut bytes = manifest.canonical_bytes().to_vec();
    reseal(&mut bytes);
    assert_eq!(bytes, manifest.canonical_bytes());
    assert_ne!(manifest.identity(), &closure().identity_sha256());
    for entry in manifest.entries() {
        assert_eq!(
            entry.role.protected_mode(),
            if entry.path.starts_with("bin/") || entry.role == Role::ElfInterpreter {
                0o555
            } else {
                0o444
            }
        );
    }
}

#[test]
fn all_bytes_are_bound_and_resealing_cannot_supply_missing_semantics() {
    let manifest = fixture();
    for i in 0..manifest.canonical_bytes().len() {
        let mut bytes = manifest.canonical_bytes().to_vec();
        bytes[i] ^= 1;
        assert!(decode(&bytes).is_err(), "byte {i}");
    }
    for at in [0, 8, 10, 18] {
        assert_eq!(decode(&mutated(at, &[0xff])), Err(Error::Header));
    }
    for at in [12, 16] {
        assert_eq!(decode(&mutated(at, &[0xff])), Err(Error::Length));
    }
    for at in [22, 31, HEADER + 2, HEADER + 6, HEADER + 303] {
        assert_eq!(decode(&mutated(at, &[1])), Err(Error::Reserved));
    }
    assert_eq!(decode(&mutated(256, &[0; 32])), Err(Error::Digest));
    assert!(matches!(
        decode(&mutated(20, &[2, 0])),
        Err(Error::CompilerClosure(_))
    ));
    for at in (32..256).step_by(32) {
        assert!(matches!(
            decode(&mutated(at, &[0; 32])),
            Err(Error::CompilerClosure(_))
        ));
    }
}

#[test]
fn roles_are_closed_mandatory_and_singletons_do_not_repeat() {
    for i in 0..6 {
        assert_eq!(
            decode(&mutated(HEADER + i * ENTRY, &99_u16.to_le_bytes())),
            Err(Error::Role)
        );
        let mut rows = entries();
        rows[i].role = if i == 3 {
            Role::ProofExecutorHelper
        } else {
            Role::SharedLibrary
        };
        assert_eq!(
            Manifest::new(closure(), [11; 32], &rows, |_| Ok::<_, u8>(())),
            Err(Error::Role)
        );
    }
    // A real dependency closure may contain many SharedLibrary entries.
    let mut rows = entries().to_vec();
    rows.push(CompilerRuntimeEntryV1 {
        role: Role::SharedLibrary,
        path: "lib/libz.so",
        length: 20,
        sha256: [12; 32],
    });
    assert_eq!(
        Manifest::new(closure(), [11; 32], &rows, |_| Ok::<_, u8>(()))
            .unwrap()
            .entries()
            .len(),
        7
    );
}

#[test]
fn paths_refuse_aliases_duplicates_unsorted_entries_and_traversal() {
    for path in [
        "",
        "/bin/helper",
        "../helper",
        "bin/../helper",
        "./bin/helper",
        "bin/./helper",
        "bin//helper",
        "bin/helper/",
        "bin/.",
        "bin/..",
        "bin\\helper",
        "bin/he\0lper",
        "bin/he lper",
        "bin/h\u{e9}lper",
    ] {
        let mut rows = entries();
        rows[0].path = path;
        assert_eq!(
            Manifest::new(closure(), [11; 32], &rows, |_| Ok::<_, u8>(())),
            Err(Error::Path),
            "{path:?}"
        );
    }
    let mut rows = entries();
    rows[1].path = rows[0].path;
    assert_eq!(
        Manifest::new(closure(), [11; 32], &rows, |_| Ok::<_, u8>(())),
        Err(Error::Path)
    );
    rows = entries();
    rows.swap(0, 1);
    assert_eq!(
        Manifest::new(closure(), [11; 32], &rows, |_| Ok::<_, u8>(())),
        Err(Error::Path)
    );
    for path in [
        "a".repeat(257),
        std::iter::repeat_n("a", 17).collect::<Vec<_>>().join("/"),
    ] {
        rows = entries();
        rows[0].path = &path;
        assert_eq!(
            Manifest::new(closure(), [11; 32], &rows, |_| Ok::<_, u8>(())),
            Err(Error::Path)
        );
    }
    let mut bytes = fixture().canonical_bytes().to_vec();
    bytes[HEADER + 48..HEADER + 58].copy_from_slice(b"../helperx");
    reseal(&mut bytes);
    assert_eq!(decode(&bytes), Err(Error::Path));
    assert_eq!(
        decode(&mutated(HEADER + 4, &257_u16.to_le_bytes())),
        Err(Error::Path)
    );
}

#[test]
fn exact_file_lengths_and_rustc_backend_pin_joins_are_required() {
    for length in [0, COMPILER_RUNTIME_MANIFEST_MAX_FILE_BYTES_V1 + 1, u64::MAX] {
        assert_eq!(
            decode(&mutated(HEADER + 8, &length.to_le_bytes())),
            Err(Error::FileLength)
        );
    }
    for i in 0..6 {
        assert_eq!(
            decode(&mutated(HEADER + i * ENTRY + 16, &[0; 32])),
            Err(Error::Digest)
        );
    }
    for i in [1, 4] {
        assert_eq!(
            decode(&mutated(HEADER + i * ENTRY + 16, &[19; 32])),
            Err(Error::Digest)
        );
    }
    let mut rows = entries();
    for row in &mut rows {
        row.length = COMPILER_RUNTIME_MANIFEST_MAX_FILE_BYTES_V1;
    }
    assert_eq!(
        Manifest::new(closure(), [11; 32], &rows, |_| Ok::<_, u8>(())),
        Err(Error::FileLength)
    );
    rows[0].length = 1;
    rows[1].length = 1;
    rows[2].length -= 2;
    let exact = Manifest::new(closure(), [11; 32], &rows, |_| Ok::<_, u8>(())).unwrap();
    assert_eq!(
        exact.total_file_bytes(),
        COMPILER_RUNTIME_MANIFEST_MAX_TOTAL_BYTES_V1
    );
}

#[test]
fn extent_and_count_bounds_refuse_truncation_padding_and_missing_entries() {
    let bytes = fixture().canonical_bytes().to_vec();
    for len in 0..bytes.len() {
        assert!(decode(&bytes[..len]).is_err(), "truncation {len}");
    }
    let mut extra = bytes;
    extra.push(0);
    assert_eq!(decode(&extra), Err(Error::Length));
    assert_eq!(
        decode(&vec![0; COMPILER_RUNTIME_MANIFEST_MAX_BYTES_V1 + 1]),
        Err(Error::Length)
    );
    for count in [0, 1, 5, 129, u16::MAX] {
        assert_eq!(
            decode(&mutated(16, &count.to_le_bytes())),
            Err(Error::Length)
        );
    }
    let paths: Vec<_> = (0..128).map(|i| format!("code/{i:03}")).collect();
    let mut rows = entries().to_vec();
    rows.resize(
        128,
        CompilerRuntimeEntryV1 {
            role: Role::SharedLibrary,
            path: "",
            length: 1,
            sha256: [19; 32],
        },
    );
    for (row, path) in rows.iter_mut().zip(&paths) {
        row.path = path;
    }
    let value = Manifest::new(closure(), [11; 32], &rows, |_| Ok::<_, u8>(())).unwrap();
    assert_eq!(
        value.canonical_bytes().len(),
        COMPILER_RUNTIME_MANIFEST_MAX_BYTES_V1
    );
    assert_eq!(decode(value.canonical_bytes()).unwrap(), value);
}

#[test]
fn work_is_prepaid_once_exact_or_one_short_before_payload_and_hashes() {
    let value = fixture();
    for limit in [
        COMPILER_RUNTIME_MANIFEST_WORK_V1 - 1,
        COMPILER_RUNTIME_MANIFEST_WORK_V1,
    ] {
        for construct in [false, true] {
            let calls = Cell::new(0);
            let charge = |n| {
                calls.set(calls.get() + 1);
                assert_eq!(n, COMPILER_RUNTIME_MANIFEST_WORK_V1);
                if n > limit { Err(7) } else { Ok(()) }
            };
            let result = if construct {
                Manifest::new(closure(), [11; 32], &entries(), charge)
            } else {
                Manifest::decode(value.canonical_bytes(), charge)
            };
            assert_eq!(calls.get(), 1);
            if limit < COMPILER_RUNTIME_MANIFEST_WORK_V1 {
                assert_eq!(result, Err(Error::Charge(7)));
            } else {
                assert_eq!(result.unwrap(), value);
            }
        }
    }
    assert_eq!(
        Manifest::decode(&vec![0; value.canonical_bytes().len()], |_| Err(3)),
        Err(Error::Charge(3))
    );
    assert_eq!(
        Manifest::decode(&[], |_| -> Result<(), u8> {
            panic!("invalid extent does not inspect bytes")
        }),
        Err(Error::Length)
    );
    assert!(COMPILER_RUNTIME_MANIFEST_STORAGE_V1 >= 3 * std::mem::size_of::<Manifest>());
    assert!(
        catch_unwind(AssertUnwindSafe(|| Manifest::decode(
            value.canonical_bytes(),
            |_| -> Result<(), u8> { panic!("charge unwind") }
        )))
        .is_err()
    );
}
