//! Inert file/range mechanics only, not installed runtime or process admission.
use super::*;
use fe2o3_protected_service_spawn::trace_runtime::mapping_file_range_is_allowed;
use std::io::Write;

fn elf_file() -> File {
    let mut file = tempfile::tempfile().unwrap();
    let mut bytes = [0_u8; elf::HEADER_BYTES + 2 * elf::PROGRAM_HEADER_BYTES];
    bytes[..7].copy_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1]);
    bytes[16..18].copy_from_slice(&3_u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
    bytes[32..40].copy_from_slice(&(elf::HEADER_BYTES as u64).to_le_bytes());
    bytes[54..56].copy_from_slice(&(elf::PROGRAM_HEADER_BYTES as u16).to_le_bytes());
    bytes[56..58].copy_from_slice(&2_u16.to_le_bytes());
    for (index, offset) in [0_u64, 0x2000].into_iter().enumerate() {
        let start = elf::HEADER_BYTES + index * elf::PROGRAM_HEADER_BYTES;
        let program = &mut bytes[start..start + elf::PROGRAM_HEADER_BYTES];
        program[..4].copy_from_slice(&1_u32.to_le_bytes());
        program[4..8].copy_from_slice(&5_u32.to_le_bytes());
        program[8..16].copy_from_slice(&offset.to_le_bytes());
        program[32..40].copy_from_slice(&0x100_u64.to_le_bytes());
    }
    file.write_all(&bytes).unwrap();
    file.set_len(0x4000).unwrap();
    file
}

fn entry(role: Role, file: &File) -> Entry<'static> {
    Entry {
        role,
        path: "inert",
        length: file.metadata().unwrap().len(),
        sha256: [0; 32],
    }
}

fn views(entries: &[Executable]) -> impl Iterator<Item = ExecutableObjectRanges<'_>> + Clone {
    entries
        .iter()
        .map(|e| ExecutableObjectRanges::new(e.device, e.inode, e.ranges.as_slice()))
}

#[test]
fn native_inventory_preserves_original_identity_holes_and_source_indices() {
    let file = elf_file();
    let helper = tempfile::tempfile().unwrap();
    let sources = [
        (entry(Role::ProofExecutorHelper, &helper), &helper),
        (entry(Role::Rustc, &file), &file),
    ];
    let entries = derive_entries(sources.into_iter()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].source_index, 1);
    assert_eq!(
        entries[0].ranges.as_slice(),
        &[(0, 0x1000), (0x2000, 0x3000)]
    );
    let original = fs::fstat(&file).unwrap();
    assert_eq!(
        (entries[0].device, entries[0].inode),
        (original.st_dev, original.st_ino)
    );
    let device = format!(
        "{:x}:{:x}",
        fs::major(original.st_dev),
        fs::minor(original.st_dev)
    );
    let inode = original.st_ino.to_string();
    for (offset, length, accepted) in [
        (0, 0x1000, true),
        (0x1000, 1, false),
        (0, 0x3000, false),
        (0x2000, 0x1000, true),
        (0x3000, 1, false),
        (0, 0, false),
    ] {
        assert_eq!(
            mapping_file_range_is_allowed(&device, &inode, offset, length, views(&entries))
                .unwrap(),
            accepted
        );
    }
    let equal_bytes = elf_file();
    let other = fs::fstat(&equal_bytes).unwrap();
    assert_ne!(other.st_ino, original.st_ino);
    assert!(
        !mapping_file_range_is_allowed(&device, &other.st_ino.to_string(), 0, 1, views(&entries))
            .unwrap()
    );
}

#[test]
fn native_inventory_rejects_unsupported_and_truncated_compiler_images() {
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(b"not executable").unwrap();
    assert!(matches!(
        derive_entries([(entry(Role::SharedLibrary, &file), &file)].into_iter()),
        Err(Error::Invalid(_))
    ));
    let file = elf_file();
    file.set_len(7).unwrap();
    assert!(matches!(
        derive_entries([(entry(Role::Rustc, &file), &file)].into_iter()),
        Err(Error::Policy(_))
    ));
}

#[test]
fn native_inventory_refuses_manifest_extent_and_file_kind_mismatches() {
    let file = elf_file();
    let mut descriptor = entry(Role::CodegenBackend, &file);
    descriptor.length -= 1;
    assert!(matches!(
        derive_entries([(descriptor, &file)].into_iter()),
        Err(Error::Invalid(_))
    ));
    let directory = tempfile::tempdir().unwrap();
    let file = File::open(directory.path()).unwrap();
    assert!(matches!(
        derive_entries([(entry(Role::Fe2o3ProcMacro, &file), &file)].into_iter()),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn native_inventory_bounds_allocation_and_excludes_helper_only_sets() {
    let file = elf_file();
    let descriptor = entry(Role::Rustc, &file);
    assert!(derive_entries(std::iter::empty()).is_err());
    assert!(derive_entries(std::iter::repeat_n((descriptor, &file), MAX_ENTRIES + 1)).is_err());
    assert!(
        derive_entries([(entry(Role::ProofExecutorHelper, &file), &file)].into_iter()).is_err()
    );
    let entries = derive_entries(std::iter::repeat_n((descriptor, &file), MAX_ENTRIES)).unwrap();
    assert_eq!(entries.len(), MAX_ENTRIES);
    assert!(
        storage_for(entries.capacity()).unwrap() <= NativeCompilerExecutableInventory::MAX_STORAGE
    );
    assert!(storage_for(MAX_ENTRIES + 1).is_err());
    assert!(storage_for(usize::MAX).is_err());
}

#[test]
fn native_inventory_uses_retained_file_not_replacement_path() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("image");
    let source = elf_file();
    let mut original = File::create(&path).unwrap();
    let mut bytes = vec![0; source.metadata().unwrap().len() as usize];
    assert_eq!(io::pread(&source, &mut bytes, 0).unwrap(), bytes.len());
    original.write_all(&bytes).unwrap();
    drop(original);
    let original = File::open(&path).unwrap();
    std::fs::remove_file(&path).unwrap();
    std::fs::write(&path, b"replacement is not ELF").unwrap();
    let entries =
        derive_entries([(entry(Role::ElfInterpreter, &original), &original)].into_iter()).unwrap();
    assert_eq!(entries[0].inode, fs::fstat(&original).unwrap().st_ino);
    assert_ne!(entries[0].inode, fs::stat(&path).unwrap().st_ino);
}

#[test]
fn native_inventory_preserves_original_read_failure() {
    let directory = tempfile::tempdir().unwrap();
    let file = File::create(directory.path().join("write_only")).unwrap();
    file.set_len(100).unwrap();
    assert!(matches!(
        derive_entries([(entry(Role::Rustc, &file), &file)].into_iter()),
        Err(Error::Io {
            source: io::Errno::BADF,
            ..
        })
    ));
}

#[test]
fn scoped_range_identity_refuses_foreign_accounts_changed_objects_and_rosters() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    let file = elf_file();
    let descriptor = entry(Role::Rustc, &file);
    let entries = derive_entries([(descriptor, &file)].into_iter()).unwrap();
    let mut work = Work::new(1_000_000);
    let budget = Budget::new(&mut work, 1_000_000);
    let inventory = NativeCompilerExecutableInventory {
        retained: storage_for(entries.capacity()).unwrap(),
        entries,
        ledger: budget.work_ledger_identity_v1(),
        address: &budget as *const Budget<'_> as usize,
    };
    inventory.check_original_account(&budget).unwrap();
    inventory
        .check_objects([(descriptor, &file)].into_iter())
        .unwrap();
    let mut foreign_work = Work::new(1_000_000);
    let foreign = Budget::new(&mut foreign_work, 1_000_000);
    assert!(matches!(
        inventory.check_original_account(&foreign),
        Err(Error::Resource(Resource::Accounting))
    ));
    let replacement = elf_file();
    assert!(
        inventory
            .check_objects([(descriptor, &replacement)].into_iter())
            .is_err()
    );
    assert!(inventory.check_objects(std::iter::empty()).is_err());
    assert!(
        inventory
            .check_objects([(descriptor, &file), (descriptor, &file)].into_iter())
            .is_err()
    );
    file.set_len(descriptor.length - 1).unwrap();
    assert!(
        inventory
            .check_objects([(descriptor, &file)].into_iter())
            .is_err()
    );
}
