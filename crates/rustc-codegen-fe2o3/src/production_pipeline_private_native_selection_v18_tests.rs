use super::*;
use fe2o3_kernel_ir::{
    AddressSpace, Constant, MemoryAccess, OperationKind as Op, ScalarType, StorageLayoutIdV1,
    StorageOperationV1, StorageProjectionV1, Type, ValueId,
};

#[test]
fn private_native_selection_is_a_carrier_route_not_an_access_permit() {
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    for operation in [
        Op::Alloca {
            element: Type::StorageObject(StorageLayoutIdV1(0)),
            count: None,
            address_space: AddressSpace::Private,
            alignment: 4,
        },
        // Malformed/unadmitted shapes still select the strict private checker.
        // Selection cannot make their allocation or source proof succeed.
        Op::Alloca {
            element: Type::Scalar(ScalarType::U32),
            count: Some(ValueId(99)),
            address_space: AddressSpace::Global,
            alignment: 0,
        },
        Op::Storage(StorageOperationV1::ReadValue {
            address: ValueId(0),
            access,
        }),
        Op::Storage(StorageOperationV1::WriteValue {
            address: ValueId(0),
            value: ValueId(1),
            access,
        }),
        Op::Storage(StorageOperationV1::Project {
            base: ValueId(0),
            step: StorageProjectionV1::Field(7),
        }),
    ] {
        assert!(private_memory_carrier(&operation));
    }
    for operation in [
        Op::Constant(Constant::U32(7)),
        Op::Load {
            pointer: ValueId(0),
            access,
        },
        Op::Store {
            pointer: ValueId(0),
            value: ValueId(1),
            access,
        },
        Op::Cast {
            kind: fe2o3_kernel_ir::CastKind::RestrictPointerAccess,
            value: ValueId(0),
            to: Type::Scalar(ScalarType::U32),
        },
    ] {
        // These operations still need their own source roles. Not selecting
        // private memory is not evidence that lifecycle admission accepts them.
        assert!(!private_memory_carrier(&operation));
    }
}
