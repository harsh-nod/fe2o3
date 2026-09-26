use super::super::storage_native_v18::tests::*;
use super::*;

fn copy(overlap: Overlap, source_volatile: bool, destination_volatile: bool) -> Operation {
    let mut source_access = MemoryAccess::new(Space::Private, 4);
    let mut destination_access = source_access;
    source_access.volatile = source_volatile;
    destination_access.volatile = destination_volatile;
    Operation::new(
        vec![],
        OperationKind::Storage(Storage::CopyObject {
            source: ValueId(0),
            destination: ValueId(1),
            source_access,
            destination_access,
            overlap,
        }),
    )
}

#[test]
fn may_overlap_uses_memmove_and_disjoint_uses_memcpy() {
    for overlap in [Overlap::MayOverlap, Overlap::NonOverlapping] {
        let module = module(
            vec![scalar()],
            vec![
                allocate(0, 0, Space::Private, 4),
                allocate(1, 0, Space::Private, 4),
                copy(overlap, false, false),
            ],
            Profile::Gfx942,
        );
        let text = emit(&module, Profile::Gfx942);
        let name = if overlap == Overlap::MayOverlap {
            "memmove"
        } else {
            "memcpy"
        };
        assert!(text.contains(&format!("call void @llvm.{name}.p5.p5.i64(ptr addrspace(5) align 4 %v1, ptr addrspace(5) align 4 %v0, i64 4, i1 false)")));
        assert_eq!(
            text.matches(&format!("declare void @llvm.{name}.p5.p5.i64"))
                .count(),
            1
        );
    }
}

#[test]
fn independent_volatile_copy_endpoints_are_never_or_combined() {
    for (source, destination) in [(true, false), (false, true), (true, true)] {
        let module = module(
            vec![scalar()],
            vec![
                allocate(0, 0, Space::Private, 4),
                allocate(1, 0, Space::Private, 4),
                copy(Overlap::MayOverlap, source, destination),
            ],
            Profile::Gfx942,
        );
        with_owner(&module, |owner, budget| {
            assert!(lower_canonical_storage_module_v18(owner, Profile::Gfx942, budget).is_err())
        });
    }
}

#[test]
fn pointer_containing_object_copy_does_not_scalarize_or_expose_address_bits() {
    let rows = vec![
        scalar(),
        Row {
            size: 8,
            alignment: 8,
            kind: Kind::Pointer(StoragePointerV1 {
                pointee: Id(0),
                value_space: Space::Private,
                encoded_space: Space::Generic,
                access: AccessMode::ReadWrite,
                stored_bits: 64,
            }),
        },
    ];
    let module = module(
        rows,
        vec![
            allocate(0, 1, Space::Private, 8),
            allocate(1, 1, Space::Private, 8),
            copy(Overlap::MayOverlap, false, false),
        ],
        Profile::Gfx950,
    );
    let text = emit(&module, Profile::Gfx950);
    assert!(text.contains("i64 8, i1 false"));
    assert!(!text.contains(" = load "));
    assert!(!text.contains("  store "));
    assert!(!text.contains("ptrtoint"));
}
