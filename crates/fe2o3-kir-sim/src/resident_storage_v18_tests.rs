use super::*;
use fe2o3_kernel_ir::{
    FixedVectorTypeV12, StorageFieldV1, StorageLayoutIdV1, StorageLayoutKindV1 as Kind,
    StorageLayoutV1, StoragePointerV1, StorageVariantEncodingV1, StorageVariantV1, VectorLayoutV12,
};

#[test]
fn v18_layout_census_counts_spare_rows_and_boxed_payloads_once() {
    let mut module = Module::new("layout-payload-census");
    let base = module_retained_heap_bytes(&module).unwrap();
    module.storage_layouts.reserve_exact(16);
    let field = StorageFieldV1 {
        offset: 0,
        layout: StorageLayoutIdV1(0),
    };
    let row = |kind| StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind,
    };
    module.storage_layouts.extend([
        row(Kind::Scalar(fe2o3_kernel_ir::ScalarType::U32)),
        row(Kind::Record(vec![field; 3].into_boxed_slice())),
        row(Kind::Union(vec![field; 2].into_boxed_slice())),
        row(Kind::Variants {
            encoding: StorageVariantEncodingV1::Direct { tag: field },
            variants: [1, 2]
                .map(|tag| StorageVariantV1 {
                    discriminant: tag,
                    direct_tag_bits: Some(tag),
                    uninhabited: false,
                    layout: StorageLayoutIdV1(0),
                })
                .into(),
        }),
        row(Kind::Array {
            element: StorageLayoutIdV1(0),
            length: u64::MAX,
            stride: 4,
        }),
        row(Kind::Pointer(StoragePointerV1 {
            pointee: StorageLayoutIdV1(0),
            value_space: AddressSpace::Global,
            encoded_space: AddressSpace::Global,
            access: fe2o3_kernel_ir::AccessMode::ReadOnly,
            stored_bits: 64,
        })),
        row(Kind::Slice {
            element: StorageLayoutIdV1(0),
            value_space: AddressSpace::Global,
            access: fe2o3_kernel_ir::AccessMode::ReadOnly,
            data: field,
            length: field,
        }),
        row(Kind::Vector(FixedVectorTypeV12::new(
            fe2o3_kernel_ir::ScalarType::U32,
            4,
            VectorLayoutV12::Contiguous,
        ))),
    ]);
    // This is a container-allocation census, not layout validity admission.
    // Referenced object extents/row IDs never allocate or recurse here.
    assert!(module.storage_layouts.capacity() > module.storage_layouts.len());
    let rows = module.storage_layouts.capacity() * size_of::<StorageLayoutV1>();
    let boxes = 5 * size_of::<StorageFieldV1>() + 2 * size_of::<StorageVariantV1>();
    assert_eq!(
        module_retained_heap_bytes(&module),
        Some(base + rows + boxes)
    );
    module.storage_layouts.clear();
    assert_eq!(module_retained_heap_bytes(&module), Some(base + rows));
}

#[test]
fn storage_census_arithmetic_remains_checked() {
    let mut ledger = ResidentLedger::new(usize::MAX);
    assert!(
        ledger
            .add_product(1, size_of::<StorageLayoutV1>())
            .is_none()
    );
    let mut ledger = ResidentLedger::new(0);
    assert!(
        ledger
            .add_product(usize::MAX, size_of::<StorageFieldV1>())
            .is_none()
    );
    assert!(
        ledger
            .add_product(usize::MAX, size_of::<StorageVariantV1>())
            .is_none()
    );
}
