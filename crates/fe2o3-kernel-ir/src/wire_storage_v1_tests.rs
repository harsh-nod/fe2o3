use super::*;
use crate::{StorageCopyOverlapV1, StorageLayoutIdV1, StorageOperationV1, StorageProjectionV1};

const LEGACY_VERSIONS: [u16; 15] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 15, 16, 17];

fn unsupported(version: u16, feature: &'static str) -> KernelIrEncodeError {
    KernelIrEncodeError::UnsupportedInVersion { version, feature }
}

fn operations() -> [StorageOperationV1; 7] {
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    let a = ValueId(0);
    let b = ValueId(1);
    [
        StorageOperationV1::Project {
            base: a,
            step: StorageProjectionV1::Field(0),
        },
        StorageOperationV1::Project {
            base: a,
            step: StorageProjectionV1::ArrayIndex(b),
        },
        StorageOperationV1::Project {
            base: a,
            step: StorageProjectionV1::Variant { index: 0, access },
        },
        StorageOperationV1::ReadValue { address: a, access },
        StorageOperationV1::WriteValue {
            address: a,
            value: b,
            access,
        },
        StorageOperationV1::CopyObject {
            source: a,
            destination: b,
            source_access: access,
            destination_access: access,
            overlap: StorageCopyOverlapV1::NonOverlapping,
        },
        StorageOperationV1::CopyObject {
            source: a,
            destination: a,
            source_access: access,
            destination_access: access,
            overlap: StorageCopyOverlapV1::MayOverlap,
        },
    ]
}

#[test]
fn legacy_storage_operations_refuse_before_any_tag_bytes_or_work_in_all_writers() {
    for version in LEGACY_VERSIONS {
        for storage in operations() {
            for mode in 0..3 {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
                let mut writer = match mode {
                    0 => Writer::new(version, Some(&mut work)),
                    1 => Writer::counter(version, &mut work),
                    _ => Writer::comparing(version, &[], Some(&mut work)),
                };
                assert_eq!(
                    encode_operation_kind(&mut writer, &OperationKind::Storage(storage)),
                    Err(unsupported(version, "module-owned storage operation")),
                );
                assert_eq!(writer.length(), 0);
                assert_eq!(writer.bytes.capacity(), 0);
                drop(writer);
                assert_eq!(work.work(), 0);
            }
        }
    }
}

#[test]
fn legacy_storage_type_refuses_before_any_tag_bytes_or_work_in_all_writers() {
    for version in LEGACY_VERSIONS {
        for mode in 0..3 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
            let mut writer = match mode {
                0 => Writer::new(version, Some(&mut work)),
                1 => Writer::counter(version, &mut work),
                _ => Writer::comparing(version, &[], Some(&mut work)),
            };
            assert_eq!(
                encode_type(&mut writer, &Type::StorageObject(StorageLayoutIdV1(0)), 0),
                Err(unsupported(version, "module-owned storage type")),
            );
            assert_eq!(writer.length(), 0);
            assert_eq!(writer.bytes.capacity(), 0);
            drop(writer);
            assert_eq!(work.work(), 0);
        }
    }
}

#[test]
fn legacy_type_walk_cannot_hide_storage_beneath_pointer_or_slice_layers() {
    for version in LEGACY_VERSIONS {
        let mut ty = Type::StorageObject(StorageLayoutIdV1(0));
        for depth in 0..=MAX_TYPE_DEPTH_V1 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut writer = Writer::counter(version, &mut work);
            assert_eq!(
                encode_type(&mut writer, &ty, 0),
                Err(unsupported(version, "module-owned storage type")),
                "version {version}, depth {depth}",
            );
            assert_eq!(writer.bytes.capacity(), 0);
            ty = if depth % 2 == 0 {
                Type::pointer(ty, AddressSpace::Private, AccessMode::ReadWrite)
            } else {
                Type::slice(ty, AddressSpace::Global, AccessMode::ReadOnly)
            };
        }
    }
}

#[test]
fn legacy_whole_module_writers_reject_storage_even_with_an_empty_table() {
    for version in LEGACY_VERSIONS {
        for storage in operations() {
            let mut block = BasicBlock::new(BlockId(0));
            block
                .operations
                .push(Operation::new(vec![], OperationKind::Storage(storage)));
            block.terminator = Some(Terminator::Return { values: vec![] });
            let mut module = Module::new("raw-storage");
            module.functions.push(Function::internal_helper(
                "f",
                Signature::new(vec![], vec![]),
                vec![],
                vec![block],
            ));
            assert!(module.storage_layouts.is_empty());
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            assert_eq!(
                count_module_with_work_v1(&module, version, &mut work, false).unwrap_err(),
                unsupported(version, "module-owned storage operation"),
            );
            assert_eq!(
                compare_module_encoding_v1(&module, version, &[], None),
                Err(unsupported(version, "module-owned storage operation")),
            );
            let mut writer = Writer::new(version, None);
            assert_eq!(
                write_module_v1(&module, &mut writer, false),
                Err(unsupported(version, "module-owned storage operation")),
            );
        }
    }
}
