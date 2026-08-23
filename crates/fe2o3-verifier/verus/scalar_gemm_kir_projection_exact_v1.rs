// Independently reviewed exact scalar GEMM projection AST.
//
// This specification is transcribed from the reviewed six-block scalar GEMM
// algorithm. It is intentionally not generated from the Rust projection
// writer. Equality with this value binds every decoded structural field.

pub mod scalar_gemm_kir_projection_exact_v1 {

use vstd::prelude::*;

use super::scalar_gemm_kir_projection_ast_v1::{
    ScalarKirProjectionAstDecodeV1,
    ScalarKirProjectionBlockV1,
    ScalarKirProjectionBodyV1,
    ScalarKirProjectionCapabilityV1,
    ScalarKirProjectionFunctionV1,
    ScalarKirProjectionKernelV1,
    ScalarKirProjectionLaunchDomainV1,
    ScalarKirProjectionLaunchExtentV1,
    ScalarKirProjectionMemoryAccessV1,
    ScalarKirProjectionModuleV1,
    ScalarKirProjectionOperationKindV1,
    ScalarKirProjectionOperationV1,
    ScalarKirProjectionSignatureV1,
    ScalarKirProjectionTerminatorV1,
    ScalarKirProjectionTypeV1,
    ScalarKirProjectionValueDefV1,
    ScalarKirProjectionWorkgroupV1,
    scalar_kir_projection_ast_decode_v1,
};
use super::scalar_gemm_kir_projection_generated_v1::FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1;

verus! {

pub open spec fn reviewed_scalar_kir_schema_v1() -> Seq<u8> {
    seq![
        102u8, 101u8, 50u8, 111u8, 51u8, 46u8, 115u8, 99u8, 97u8, 108u8,
        97u8, 114u8, 45u8, 103u8, 101u8, 109u8, 109u8, 46u8, 115u8, 101u8,
        109u8, 97u8, 110u8, 116u8, 105u8, 99u8, 45u8, 112u8, 114u8, 111u8,
        106u8, 101u8, 99u8, 116u8, 105u8, 111u8, 110u8, 46u8, 118u8, 49u8,
    ]
}

pub open spec fn reviewed_scalar_kir_module_id_v1() -> Seq<u8> {
    seq![
        102u8, 101u8, 50u8, 111u8, 51u8, 58u8, 58u8, 115u8, 99u8, 97u8,
        108u8, 97u8, 114u8, 95u8, 103u8, 101u8, 109u8, 109u8, 95u8, 118u8,
        49u8,
    ]
}

pub open spec fn reviewed_scalar_kir_function_id_v1() -> Seq<u8> {
    seq![
        95u8, 95u8, 102u8, 101u8, 50u8, 111u8, 51u8, 95u8, 115u8, 99u8,
        97u8, 108u8, 97u8, 114u8, 95u8, 103u8, 101u8, 109u8, 109u8, 95u8,
        118u8, 49u8, 95u8, 105u8, 109u8, 112u8, 108u8,
    ]
}

pub open spec fn reviewed_scalar_kir_kernel_id_v1() -> Seq<u8> {
    seq![
        115u8, 99u8, 97u8, 108u8, 97u8, 114u8, 95u8, 103u8, 101u8, 109u8,
        109u8, 95u8, 118u8, 49u8,
    ]
}

pub open spec fn reviewed_scalar_kir_target_namespace_v1() -> Seq<u8> {
    seq![
        102u8, 101u8, 50u8, 111u8, 51u8, 46u8, 97u8, 109u8, 100u8, 103u8,
        112u8, 117u8, 46u8, 116u8, 97u8, 114u8, 103u8, 101u8, 116u8,
    ]
}

pub open spec fn reviewed_scalar_kir_target_name_v1() -> Seq<u8> {
    seq![
        103u8, 102u8, 120u8, 57u8, 52u8, 50u8, 58u8, 120u8, 110u8, 97u8,
        99u8, 107u8, 45u8,
    ]
}

pub open spec fn reviewed_scalar_kir_capabilities_v1()
    -> Seq<ScalarKirProjectionCapabilityV1>
{
    seq![
        ScalarKirProjectionCapabilityV1::Extension {
            namespace: reviewed_scalar_kir_target_namespace_v1(),
            name: reviewed_scalar_kir_target_name_v1(),
        },
        ScalarKirProjectionCapabilityV1::WaveWidth { width: 2 },
    ]
}

pub open spec fn reviewed_scalar_kir_scalar_type_v1(kind: nat)
    -> ScalarKirProjectionTypeV1
{
    ScalarKirProjectionTypeV1::Scalar { kind }
}

pub open spec fn reviewed_scalar_kir_pointer_type_v1(access: nat)
    -> ScalarKirProjectionTypeV1
{
    ScalarKirProjectionTypeV1::Pointer {
        pointee: seq![reviewed_scalar_kir_scalar_type_v1(15)],
        address_space: 3,
        access,
    }
}

pub open spec fn reviewed_scalar_kir_slice_type_v1(access: nat)
    -> ScalarKirProjectionTypeV1
{
    ScalarKirProjectionTypeV1::Slice {
        element: seq![reviewed_scalar_kir_scalar_type_v1(15)],
        address_space: 3,
        access,
    }
}

pub open spec fn reviewed_scalar_kir_value_v1(
    id: nat,
    ty: ScalarKirProjectionTypeV1,
) -> ScalarKirProjectionValueDefV1 {
    ScalarKirProjectionValueDefV1 { id, ty }
}

pub open spec fn reviewed_scalar_kir_operation_v1(
    id: nat,
    ty: ScalarKirProjectionTypeV1,
    kind: ScalarKirProjectionOperationKindV1,
) -> ScalarKirProjectionOperationV1 {
    ScalarKirProjectionOperationV1 {
        results: seq![reviewed_scalar_kir_value_v1(id, ty)],
        kind,
    }
}

pub open spec fn reviewed_scalar_kir_memory_access_v1()
    -> ScalarKirProjectionMemoryAccessV1
{
    ScalarKirProjectionMemoryAccessV1 {
        address_space: 3,
        alignment: 4,
        volatile: false,
    }
}

pub open spec fn reviewed_scalar_kir_binary_v1(
    id: nat,
    ty: ScalarKirProjectionTypeV1,
    op: nat,
    lhs: nat,
    rhs: nat,
) -> ScalarKirProjectionOperationV1 {
    reviewed_scalar_kir_operation_v1(
        id,
        ty,
        ScalarKirProjectionOperationKindV1::Binary { op, lhs, rhs },
    )
}

pub open spec fn reviewed_scalar_kir_cast_v1(id: nat, value: nat)
    -> ScalarKirProjectionOperationV1
{
    reviewed_scalar_kir_operation_v1(
        id,
        reviewed_scalar_kir_scalar_type_v1(12),
        ScalarKirProjectionOperationKindV1::Cast {
            kind: 2,
            value,
            to: reviewed_scalar_kir_scalar_type_v1(12),
        },
    )
}

pub open spec fn reviewed_scalar_kir_entry_block_v1() -> ScalarKirProjectionBlockV1 {
    ScalarKirProjectionBlockV1 {
        id: 0,
        parameters: seq![],
        operations: seq![
            reviewed_scalar_kir_operation_v1(
                6,
                reviewed_scalar_kir_scalar_type_v1(12),
                ScalarKirProjectionOperationKindV1::InvocationIndex {
                    index_kind: 1,
                    axis: 1,
                    result_type: reviewed_scalar_kir_scalar_type_v1(12),
                },
            ),
            reviewed_scalar_kir_cast_v1(7, 3),
            reviewed_scalar_kir_cast_v1(8, 4),
            reviewed_scalar_kir_cast_v1(9, 5),
            reviewed_scalar_kir_binary_v1(
                10,
                reviewed_scalar_kir_scalar_type_v1(12),
                3,
                7,
                8,
            ),
            reviewed_scalar_kir_operation_v1(
                11,
                reviewed_scalar_kir_scalar_type_v1(1),
                ScalarKirProjectionOperationKindV1::Compare {
                    predicate: 3,
                    lhs: 6,
                    rhs: 10,
                },
            ),
            reviewed_scalar_kir_operation_v1(
                12,
                reviewed_scalar_kir_pointer_type_v1(1),
                ScalarKirProjectionOperationKindV1::SliceData { slice: 0 },
            ),
            reviewed_scalar_kir_operation_v1(
                13,
                reviewed_scalar_kir_pointer_type_v1(1),
                ScalarKirProjectionOperationKindV1::SliceData { slice: 1 },
            ),
            reviewed_scalar_kir_operation_v1(
                14,
                reviewed_scalar_kir_pointer_type_v1(2),
                ScalarKirProjectionOperationKindV1::SliceData { slice: 2 },
            ),
        ],
        terminator: Some(ScalarKirProjectionTerminatorV1::ConditionalBranch {
            condition: 11,
            then_target: 1,
            then_arguments: seq![],
            else_target: 5,
            else_arguments: seq![],
        }),
    }
}

pub open spec fn reviewed_scalar_kir_coordinates_block_v1() -> ScalarKirProjectionBlockV1 {
    ScalarKirProjectionBlockV1 {
        id: 1,
        parameters: seq![],
        operations: seq![
            reviewed_scalar_kir_binary_v1(
                15,
                reviewed_scalar_kir_scalar_type_v1(12),
                4,
                6,
                8,
            ),
            reviewed_scalar_kir_binary_v1(
                16,
                reviewed_scalar_kir_scalar_type_v1(12),
                5,
                6,
                8,
            ),
            reviewed_scalar_kir_operation_v1(
                17,
                reviewed_scalar_kir_scalar_type_v1(9),
                ScalarKirProjectionOperationKindV1::Constant {
                    kind: 8,
                    width: 4,
                    bits: 0,
                },
            ),
            reviewed_scalar_kir_operation_v1(
                18,
                reviewed_scalar_kir_scalar_type_v1(15),
                ScalarKirProjectionOperationKindV1::Constant {
                    kind: 13,
                    width: 4,
                    bits: 0,
                },
            ),
        ],
        terminator: Some(ScalarKirProjectionTerminatorV1::Branch {
            target: 2,
            arguments: seq![17nat, 18nat],
        }),
    }
}

pub open spec fn reviewed_scalar_kir_header_block_v1() -> ScalarKirProjectionBlockV1 {
    ScalarKirProjectionBlockV1 {
        id: 2,
        parameters: seq![
            reviewed_scalar_kir_value_v1(19, reviewed_scalar_kir_scalar_type_v1(9)),
            reviewed_scalar_kir_value_v1(20, reviewed_scalar_kir_scalar_type_v1(15)),
        ],
        operations: seq![
            reviewed_scalar_kir_operation_v1(
                21,
                reviewed_scalar_kir_scalar_type_v1(1),
                ScalarKirProjectionOperationKindV1::Compare {
                    predicate: 3,
                    lhs: 19,
                    rhs: 5,
                },
            ),
        ],
        terminator: Some(ScalarKirProjectionTerminatorV1::ConditionalBranch {
            condition: 21,
            then_target: 3,
            then_arguments: seq![],
            else_target: 4,
            else_arguments: seq![20nat],
        }),
    }
}

pub open spec fn reviewed_scalar_kir_body_block_v1() -> ScalarKirProjectionBlockV1 {
    ScalarKirProjectionBlockV1 {
        id: 3,
        parameters: seq![],
        operations: seq![
            reviewed_scalar_kir_cast_v1(22, 19),
            reviewed_scalar_kir_binary_v1(
                23,
                reviewed_scalar_kir_scalar_type_v1(12),
                3,
                15,
                9,
            ),
            reviewed_scalar_kir_binary_v1(
                24,
                reviewed_scalar_kir_scalar_type_v1(12),
                1,
                23,
                22,
            ),
            reviewed_scalar_kir_binary_v1(
                25,
                reviewed_scalar_kir_scalar_type_v1(12),
                3,
                22,
                8,
            ),
            reviewed_scalar_kir_binary_v1(
                26,
                reviewed_scalar_kir_scalar_type_v1(12),
                1,
                25,
                16,
            ),
            reviewed_scalar_kir_operation_v1(
                27,
                reviewed_scalar_kir_pointer_type_v1(1),
                ScalarKirProjectionOperationKindV1::GetElementPointer {
                    base: 12,
                    offset: 24,
                },
            ),
            reviewed_scalar_kir_operation_v1(
                28,
                reviewed_scalar_kir_scalar_type_v1(15),
                ScalarKirProjectionOperationKindV1::Load {
                    pointer: 27,
                    access: reviewed_scalar_kir_memory_access_v1(),
                },
            ),
            reviewed_scalar_kir_operation_v1(
                29,
                reviewed_scalar_kir_pointer_type_v1(1),
                ScalarKirProjectionOperationKindV1::GetElementPointer {
                    base: 13,
                    offset: 26,
                },
            ),
            reviewed_scalar_kir_operation_v1(
                30,
                reviewed_scalar_kir_scalar_type_v1(15),
                ScalarKirProjectionOperationKindV1::Load {
                    pointer: 29,
                    access: reviewed_scalar_kir_memory_access_v1(),
                },
            ),
            reviewed_scalar_kir_binary_v1(
                31,
                reviewed_scalar_kir_scalar_type_v1(15),
                3,
                28,
                30,
            ),
            reviewed_scalar_kir_binary_v1(
                32,
                reviewed_scalar_kir_scalar_type_v1(15),
                1,
                20,
                31,
            ),
            reviewed_scalar_kir_operation_v1(
                33,
                reviewed_scalar_kir_scalar_type_v1(9),
                ScalarKirProjectionOperationKindV1::Constant {
                    kind: 8,
                    width: 4,
                    bits: 1,
                },
            ),
            reviewed_scalar_kir_binary_v1(
                34,
                reviewed_scalar_kir_scalar_type_v1(9),
                1,
                19,
                33,
            ),
        ],
        terminator: Some(ScalarKirProjectionTerminatorV1::Branch {
            target: 2,
            arguments: seq![34nat, 32nat],
        }),
    }
}

pub open spec fn reviewed_scalar_kir_store_block_v1() -> ScalarKirProjectionBlockV1 {
    ScalarKirProjectionBlockV1 {
        id: 4,
        parameters: seq![
            reviewed_scalar_kir_value_v1(35, reviewed_scalar_kir_scalar_type_v1(15)),
        ],
        operations: seq![
            reviewed_scalar_kir_operation_v1(
                36,
                reviewed_scalar_kir_pointer_type_v1(2),
                ScalarKirProjectionOperationKindV1::GetElementPointer {
                    base: 14,
                    offset: 6,
                },
            ),
            ScalarKirProjectionOperationV1 {
                results: seq![],
                kind: ScalarKirProjectionOperationKindV1::Store {
                    pointer: 36,
                    value: 35,
                    access: reviewed_scalar_kir_memory_access_v1(),
                },
            },
        ],
        terminator: Some(ScalarKirProjectionTerminatorV1::Return {
            values: seq![],
        }),
    }
}

pub open spec fn reviewed_scalar_kir_inactive_block_v1() -> ScalarKirProjectionBlockV1 {
    ScalarKirProjectionBlockV1 {
        id: 5,
        parameters: seq![],
        operations: seq![],
        terminator: Some(ScalarKirProjectionTerminatorV1::Return {
            values: seq![],
        }),
    }
}

pub open spec fn reviewed_scalar_kir_function_v1() -> ScalarKirProjectionFunctionV1 {
    ScalarKirProjectionFunctionV1 {
        id: reviewed_scalar_kir_function_id_v1(),
        signature: ScalarKirProjectionSignatureV1 {
            parameters: seq![
                reviewed_scalar_kir_slice_type_v1(1),
                reviewed_scalar_kir_slice_type_v1(1),
                reviewed_scalar_kir_slice_type_v1(2),
                reviewed_scalar_kir_scalar_type_v1(9),
                reviewed_scalar_kir_scalar_type_v1(9),
                reviewed_scalar_kir_scalar_type_v1(9),
            ],
            results: seq![],
        },
        role: 1,
        body: Some(ScalarKirProjectionBodyV1 {
            parameters: seq![0nat, 1nat, 2nat, 3nat, 4nat, 5nat],
            blocks: seq![
                reviewed_scalar_kir_entry_block_v1(),
                reviewed_scalar_kir_coordinates_block_v1(),
                reviewed_scalar_kir_header_block_v1(),
                reviewed_scalar_kir_body_block_v1(),
                reviewed_scalar_kir_store_block_v1(),
                reviewed_scalar_kir_inactive_block_v1(),
            ],
        }),
        capabilities: reviewed_scalar_kir_capabilities_v1(),
    }
}

pub open spec fn reviewed_scalar_kir_kernel_v1() -> ScalarKirProjectionKernelV1 {
    ScalarKirProjectionKernelV1 {
        id: reviewed_scalar_kir_kernel_id_v1(),
        entry: reviewed_scalar_kir_function_id_v1(),
        domain: ScalarKirProjectionLaunchDomainV1::D1 {
            x: ScalarKirProjectionLaunchExtentV1::Dynamic,
        },
        workgroup: Some(ScalarKirProjectionWorkgroupV1 {
            x: 256,
            y: 1,
            z: 1,
        }),
        capabilities: reviewed_scalar_kir_capabilities_v1(),
    }
}

pub open spec fn reviewed_scalar_kir_projection_ast_v1() -> ScalarKirProjectionModuleV1 {
    ScalarKirProjectionModuleV1 {
        schema: reviewed_scalar_kir_schema_v1(),
        policy: 1,
        id: reviewed_scalar_kir_module_id_v1(),
        capabilities: reviewed_scalar_kir_capabilities_v1(),
        functions: seq![reviewed_scalar_kir_function_v1()],
        kernels: seq![reviewed_scalar_kir_kernel_v1()],
    }
}

pub open spec fn scalar_kir_projection_ast_is_exact_v1(bytes: Seq<u8>) -> bool {
    match scalar_kir_projection_ast_decode_v1(bytes) {
        ScalarKirProjectionAstDecodeV1::Invalid => false,
        ScalarKirProjectionAstDecodeV1::Complete { module } => {
            module == reviewed_scalar_kir_projection_ast_v1()
        },
    }
}

pub proof fn generated_scalar_kir_projection_decodes_to_exact_ast_v1()
    ensures
        scalar_kir_projection_ast_is_exact_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        ),
{
    assert(
        scalar_kir_projection_ast_is_exact_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        )
    ) by (compute);
}

} // verus!

} // mod scalar_gemm_kir_projection_exact_v1
