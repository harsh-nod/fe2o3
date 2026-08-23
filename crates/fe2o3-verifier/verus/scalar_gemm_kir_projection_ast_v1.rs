// Structural AST decoding for the retained scalar GEMM projection profile.
//
// The lexical decoder owns byte framing and context-dependent payload widths.
// This layer consumes those typed tokens according to the projection grammar:
// counts control repetitions, presence fields control optional productions,
// operation and terminator discriminants control their operands, and successful
// module decoding requires exact end-of-token-stream consumption.

pub mod scalar_gemm_kir_projection_ast_v1 {

use vstd::prelude::*;

use super::scalar_gemm_kir_projection_generated_v1::FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1;
use super::scalar_gemm_kir_projection_typed_v1::{
    ScalarKirTypedDecodeV1,
    ScalarKirTypedTokenV1,
    scalar_kir_typed_decode_v1,
};

verus! {

pub enum ScalarKirProjectionTypeV1 {
    Unit,
    Scalar { kind: nat },
    Pointer {
        pointee: Seq<ScalarKirProjectionTypeV1>,
        address_space: nat,
        access: nat,
    },
    Slice {
        element: Seq<ScalarKirProjectionTypeV1>,
        address_space: nat,
        access: nat,
    },
}

pub enum ScalarKirProjectionCapabilityV1 {
    Simple { kind: nat },
    SubgroupSize { size: nat },
    Atomic { width: nat, address_space: nat, scope: nat },
    Extension { namespace: Seq<u8>, name: Seq<u8> },
    WaveWidth { width: nat },
}

pub struct ScalarKirProjectionValueDefV1 {
    pub id: nat,
    pub ty: ScalarKirProjectionTypeV1,
}

pub struct ScalarKirProjectionMemoryAccessV1 {
    pub address_space: nat,
    pub alignment: nat,
    pub volatile: bool,
}

pub enum ScalarKirProjectionOperationKindV1 {
    Constant { kind: nat, width: nat, bits: nat },
    InvocationIndex { index_kind: nat, axis: nat, result_type: ScalarKirProjectionTypeV1 },
    LaunchExtentIntrinsic { axis: nat, result_type: ScalarKirProjectionTypeV1 },
    Binary { op: nat, lhs: nat, rhs: nat },
    Compare { predicate: nat, lhs: nat, rhs: nat },
    Cast { kind: nat, value: nat, to: ScalarKirProjectionTypeV1 },
    SliceData { slice: nat },
    GetElementPointer { base: nat, offset: nat },
    Load { pointer: nat, access: ScalarKirProjectionMemoryAccessV1 },
    Store { pointer: nat, value: nat, access: ScalarKirProjectionMemoryAccessV1 },
}

pub struct ScalarKirProjectionOperationV1 {
    pub results: Seq<ScalarKirProjectionValueDefV1>,
    pub kind: ScalarKirProjectionOperationKindV1,
}

pub enum ScalarKirProjectionTerminatorV1 {
    Branch { target: nat, arguments: Seq<nat> },
    ConditionalBranch {
        condition: nat,
        then_target: nat,
        then_arguments: Seq<nat>,
        else_target: nat,
        else_arguments: Seq<nat>,
    },
    Return { values: Seq<nat> },
}

pub struct ScalarKirProjectionBlockV1 {
    pub id: nat,
    pub parameters: Seq<ScalarKirProjectionValueDefV1>,
    pub operations: Seq<ScalarKirProjectionOperationV1>,
    pub terminator: Option<ScalarKirProjectionTerminatorV1>,
}

pub struct ScalarKirProjectionBodyV1 {
    pub parameters: Seq<nat>,
    pub blocks: Seq<ScalarKirProjectionBlockV1>,
}

pub struct ScalarKirProjectionSignatureV1 {
    pub parameters: Seq<ScalarKirProjectionTypeV1>,
    pub results: Seq<ScalarKirProjectionTypeV1>,
}

pub struct ScalarKirProjectionFunctionV1 {
    pub id: Seq<u8>,
    pub signature: ScalarKirProjectionSignatureV1,
    pub role: nat,
    pub body: Option<ScalarKirProjectionBodyV1>,
    pub capabilities: Seq<ScalarKirProjectionCapabilityV1>,
}

pub enum ScalarKirProjectionLaunchExtentV1 {
    Dynamic,
    Static { value: nat },
}

pub enum ScalarKirProjectionLaunchDomainV1 {
    D1 { x: ScalarKirProjectionLaunchExtentV1 },
    D2 { x: ScalarKirProjectionLaunchExtentV1, y: ScalarKirProjectionLaunchExtentV1 },
    D3 {
        x: ScalarKirProjectionLaunchExtentV1,
        y: ScalarKirProjectionLaunchExtentV1,
        z: ScalarKirProjectionLaunchExtentV1,
    },
}

pub struct ScalarKirProjectionWorkgroupV1 {
    pub x: nat,
    pub y: nat,
    pub z: nat,
}

pub struct ScalarKirProjectionKernelV1 {
    pub id: Seq<u8>,
    pub entry: Seq<u8>,
    pub domain: ScalarKirProjectionLaunchDomainV1,
    pub workgroup: Option<ScalarKirProjectionWorkgroupV1>,
    pub capabilities: Seq<ScalarKirProjectionCapabilityV1>,
}

pub struct ScalarKirProjectionModuleV1 {
    pub schema: Seq<u8>,
    pub policy: nat,
    pub id: Seq<u8>,
    pub capabilities: Seq<ScalarKirProjectionCapabilityV1>,
    pub functions: Seq<ScalarKirProjectionFunctionV1>,
    pub kernels: Seq<ScalarKirProjectionKernelV1>,
}

pub enum ScalarKirProjectionAstDecodeV1 {
    Invalid,
    Complete { module: ScalarKirProjectionModuleV1 },
}

pub open spec fn scalar_kir_structural_bytes_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
    expected_tag: nat,
) -> Option<(Seq<u8>, nat)> {
    if cursor >= tokens.len() {
        None
    } else {
        match tokens[cursor as int] {
            ScalarKirTypedTokenV1::Bytes { tag, value } => {
                if tag == expected_tag { Some((value, cursor + 1)) } else { None }
            },
            _ => None,
        }
    }
}

pub open spec fn scalar_kir_structural_count_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
    expected_tag: nat,
) -> Option<(nat, nat)> {
    if cursor >= tokens.len() {
        None
    } else {
        match tokens[cursor as int] {
            ScalarKirTypedTokenV1::Count { tag, value } => {
                if tag == expected_tag { Some((value, cursor + 1)) } else { None }
            },
            _ => None,
        }
    }
}

pub open spec fn scalar_kir_structural_u16_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
    expected_tag: nat,
) -> Option<(nat, nat)> {
    if cursor >= tokens.len() {
        None
    } else {
        match tokens[cursor as int] {
            ScalarKirTypedTokenV1::U16 { tag, value } => {
                if tag == expected_tag { Some((value, cursor + 1)) } else { None }
            },
            _ => None,
        }
    }
}

pub open spec fn scalar_kir_structural_u32_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
    expected_tag: nat,
) -> Option<(nat, nat)> {
    if cursor >= tokens.len() {
        None
    } else {
        match tokens[cursor as int] {
            ScalarKirTypedTokenV1::U32 { tag, value } => {
                if tag == expected_tag { Some((value, cursor + 1)) } else { None }
            },
            _ => None,
        }
    }
}

pub open spec fn scalar_kir_structural_boolean_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
    expected_tag: nat,
) -> Option<(bool, nat)> {
    if cursor >= tokens.len() {
        None
    } else {
        match tokens[cursor as int] {
            ScalarKirTypedTokenV1::Boolean { tag, value } => {
                if tag == expected_tag { Some((value, cursor + 1)) } else { None }
            },
            _ => None,
        }
    }
}

pub open spec fn scalar_kir_structural_enumeration_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
    expected_tag: nat,
) -> Option<(nat, nat)> {
    if cursor >= tokens.len() {
        None
    } else {
        match tokens[cursor as int] {
            ScalarKirTypedTokenV1::Enumeration { tag, value } => {
                if tag == expected_tag { Some((value, cursor + 1)) } else { None }
            },
            _ => None,
        }
    }
}

pub open spec fn scalar_kir_structural_value_id_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(nat, nat)> {
    scalar_kir_structural_u32_v1(tokens, cursor, 23)
}

pub open spec fn scalar_kir_structural_type_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionTypeV1, nat)>
    decreases tokens.len() - cursor,
{
    match scalar_kir_structural_enumeration_v1(tokens, cursor, 24) {
        None => None,
        Some((outer, next)) => {
            if outer == 1 {
                Some((ScalarKirProjectionTypeV1::Unit, next))
            } else if outer == 2 {
                match scalar_kir_structural_enumeration_v1(tokens, next, 25) {
                    Some((kind, end)) => Some((ScalarKirProjectionTypeV1::Scalar { kind }, end)),
                    None => None,
                }
            } else if outer == 3 || outer == 4 {
                match scalar_kir_structural_type_v1(tokens, next) {
                    Some((child, address_next)) => {
                        match scalar_kir_structural_enumeration_v1(tokens, address_next, 26) {
                            Some((address_space, access_next)) => {
                                match scalar_kir_structural_enumeration_v1(tokens, access_next, 27) {
                                    Some((access, end)) => {
                                        if outer == 3 {
                                            Some((ScalarKirProjectionTypeV1::Pointer {
                                                pointee: seq![child],
                                                address_space,
                                                access,
                                            }, end))
                                        } else {
                                            Some((ScalarKirProjectionTypeV1::Slice {
                                                element: seq![child],
                                                address_space,
                                                access,
                                            }, end))
                                        }
                                    },
                                    None => None,
                                }
                            },
                            None => None,
                        }
                    },
                    None => None,
                }
            } else {
                None
            }
        },
    }
}

pub open spec fn scalar_kir_structural_capability_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionCapabilityV1, nat)> {
    if cursor >= tokens.len() {
        None
    } else {
        match tokens[cursor as int] {
            ScalarKirTypedTokenV1::CapabilityKind { kind } => {
                let next = cursor + 1;
                if kind == 1 || kind == 2 || kind == 3 || kind == 4 || kind == 5
                    || kind == 7 || kind == 8 || kind == 10
                {
                    Some((ScalarKirProjectionCapabilityV1::Simple { kind }, next))
                } else if kind == 6 && next < tokens.len() {
                    match tokens[next as int] {
                        ScalarKirTypedTokenV1::CapabilitySubgroupSize { value } => {
                            Some((ScalarKirProjectionCapabilityV1::SubgroupSize { size: value }, next + 1))
                        },
                        _ => None,
                    }
                } else if kind == 9 && next + 2 < tokens.len() {
                    match (
                        tokens[next as int],
                        tokens[(next + 1) as int],
                        tokens[(next + 2) as int],
                    ) {
                        (
                            ScalarKirTypedTokenV1::CapabilityAtomicWidth { value: width },
                            ScalarKirTypedTokenV1::CapabilityAtomicAddressSpace { value: address_space },
                            ScalarKirTypedTokenV1::CapabilityAtomicScope { value: scope },
                        ) => Some((ScalarKirProjectionCapabilityV1::Atomic {
                            width,
                            address_space,
                            scope,
                        }, next + 3)),
                        _ => None,
                    }
                } else if kind == 11 && next + 1 < tokens.len() {
                    match (tokens[next as int], tokens[(next + 1) as int]) {
                        (
                            ScalarKirTypedTokenV1::CapabilityExtensionNamespace { value: namespace },
                            ScalarKirTypedTokenV1::CapabilityExtensionName { value: name },
                        ) => Some((ScalarKirProjectionCapabilityV1::Extension {
                            namespace,
                            name,
                        }, next + 2)),
                        _ => None,
                    }
                } else if kind == 12 && next < tokens.len() {
                    match tokens[next as int] {
                        ScalarKirTypedTokenV1::CapabilityWaveWidth { value: width } => {
                            Some((ScalarKirProjectionCapabilityV1::WaveWidth { width }, next + 1))
                        },
                        _ => None,
                    }
                } else {
                    None
                }
            },
            _ => None,
        }
    }
}

pub open spec fn scalar_kir_structural_capabilities_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
    remaining: nat,
    capabilities: Seq<ScalarKirProjectionCapabilityV1>,
) -> Option<(Seq<ScalarKirProjectionCapabilityV1>, nat)>
    decreases remaining,
{
    if remaining == 0 {
        Some((capabilities, cursor))
    } else if cursor >= tokens.len() || remaining > tokens.len() - cursor {
        None
    } else {
        match scalar_kir_structural_capability_v1(tokens, cursor) {
            Some((capability, next)) => scalar_kir_structural_capabilities_v1(
                tokens,
                next,
                (remaining - 1) as nat,
                capabilities.push(capability),
            ),
            None => None,
        }
    }
}

pub open spec fn scalar_kir_structural_types_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
    remaining: nat,
    types: Seq<ScalarKirProjectionTypeV1>,
) -> Option<(Seq<ScalarKirProjectionTypeV1>, nat)>
    decreases remaining,
{
    if remaining == 0 {
        Some((types, cursor))
    } else if cursor >= tokens.len() || remaining > tokens.len() - cursor {
        None
    } else {
        match scalar_kir_structural_type_v1(tokens, cursor) {
            Some((ty, next)) => scalar_kir_structural_types_v1(
                tokens,
                next,
                (remaining - 1) as nat,
                types.push(ty),
            ),
            None => None,
        }
    }
}

pub open spec fn scalar_kir_structural_value_ids_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
    remaining: nat,
    values: Seq<nat>,
) -> Option<(Seq<nat>, nat)>
    decreases remaining,
{
    if remaining == 0 {
        Some((values, cursor))
    } else if cursor >= tokens.len() || remaining > tokens.len() - cursor {
        None
    } else {
        match scalar_kir_structural_value_id_v1(tokens, cursor) {
            Some((value, next)) => scalar_kir_structural_value_ids_v1(
                tokens,
                next,
                (remaining - 1) as nat,
                values.push(value),
            ),
            None => None,
        }
    }
}

pub open spec fn scalar_kir_structural_value_def_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionValueDefV1, nat)> {
    match scalar_kir_structural_value_id_v1(tokens, cursor) {
        Some((id, next)) => {
            match scalar_kir_structural_type_v1(tokens, next) {
                Some((ty, end)) => Some((ScalarKirProjectionValueDefV1 { id, ty }, end)),
                None => None,
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_value_defs_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
    remaining: nat,
    values: Seq<ScalarKirProjectionValueDefV1>,
) -> Option<(Seq<ScalarKirProjectionValueDefV1>, nat)>
    decreases remaining,
{
    if remaining == 0 {
        Some((values, cursor))
    } else if cursor >= tokens.len() || remaining > tokens.len() - cursor {
        None
    } else {
        match scalar_kir_structural_value_def_v1(tokens, cursor) {
            Some((value, next)) => scalar_kir_structural_value_defs_v1(
                tokens,
                next,
                (remaining - 1) as nat,
                values.push(value),
            ),
            None => None,
        }
    }
}

pub open spec fn scalar_kir_structural_memory_access_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionMemoryAccessV1, nat)> {
    match scalar_kir_structural_enumeration_v1(tokens, cursor, 38) {
        Some((address_space, address_next)) => {
            match scalar_kir_structural_u32_v1(tokens, address_next, 39) {
                Some((alignment, alignment_next)) => {
                    match scalar_kir_structural_boolean_v1(tokens, alignment_next, 40) {
                        Some((volatile, end)) => Some((ScalarKirProjectionMemoryAccessV1 {
                            address_space,
                            alignment,
                            volatile,
                        }, end)),
                        None => None,
                    }
                },
                None => None,
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_constant_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionOperationKindV1, nat)> {
    if cursor + 1 >= tokens.len() {
        None
    } else {
        match (tokens[cursor as int], tokens[(cursor + 1) as int]) {
            (
                ScalarKirTypedTokenV1::ConstantKind { kind },
                ScalarKirTypedTokenV1::ConstantBits {
                    kind: bits_kind,
                    width,
                    value,
                },
            ) => {
                if kind == bits_kind {
                    Some((ScalarKirProjectionOperationKindV1::Constant {
                        kind,
                        width,
                        bits: value,
                    }, cursor + 2))
                } else {
                    None
                }
            },
            _ => None,
        }
    }
}

pub open spec fn scalar_kir_structural_intrinsic_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionOperationKindV1, nat)> {
    match scalar_kir_structural_enumeration_v1(tokens, cursor, 32) {
        Some((kind, next)) => {
            if kind == 1 {
                match scalar_kir_structural_enumeration_v1(tokens, next, 33) {
                    Some((index_kind, index_next)) => {
                        match scalar_kir_structural_enumeration_v1(tokens, index_next, 34) {
                            Some((axis, axis_next)) => {
                                match scalar_kir_structural_type_v1(tokens, axis_next) {
                                    Some((result_type, end)) => Some((
                                        ScalarKirProjectionOperationKindV1::InvocationIndex {
                                            index_kind,
                                            axis,
                                            result_type,
                                        },
                                        end,
                                    )),
                                    None => None,
                                }
                            },
                            None => None,
                        }
                    },
                    None => None,
                }
            } else if kind == 2 {
                match scalar_kir_structural_enumeration_v1(tokens, next, 34) {
                    Some((axis, axis_next)) => {
                        match scalar_kir_structural_type_v1(tokens, axis_next) {
                            Some((result_type, end)) => Some((
                                ScalarKirProjectionOperationKindV1::LaunchExtentIntrinsic {
                                    axis,
                                    result_type,
                                },
                                end,
                            )),
                            None => None,
                        }
                    },
                    None => None,
                }
            } else {
                None
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_binary_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionOperationKindV1, nat)> {
    match scalar_kir_structural_enumeration_v1(tokens, cursor, 35) {
        Some((op, op_next)) => {
            match scalar_kir_structural_value_id_v1(tokens, op_next) {
                Some((lhs, lhs_next)) => {
                    match scalar_kir_structural_value_id_v1(tokens, lhs_next) {
                        Some((rhs, end)) => Some((
                            ScalarKirProjectionOperationKindV1::Binary { op, lhs, rhs },
                            end,
                        )),
                        None => None,
                    }
                },
                None => None,
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_compare_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionOperationKindV1, nat)> {
    match scalar_kir_structural_enumeration_v1(tokens, cursor, 36) {
        Some((predicate, predicate_next)) => {
            match scalar_kir_structural_value_id_v1(tokens, predicate_next) {
                Some((lhs, lhs_next)) => {
                    match scalar_kir_structural_value_id_v1(tokens, lhs_next) {
                        Some((rhs, end)) => Some((
                            ScalarKirProjectionOperationKindV1::Compare {
                                predicate,
                                lhs,
                                rhs,
                            },
                            end,
                        )),
                        None => None,
                    }
                },
                None => None,
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_cast_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionOperationKindV1, nat)> {
    match scalar_kir_structural_enumeration_v1(tokens, cursor, 37) {
        Some((kind, kind_next)) => {
            match scalar_kir_structural_value_id_v1(tokens, kind_next) {
                Some((value, value_next)) => {
                    match scalar_kir_structural_type_v1(tokens, value_next) {
                        Some((to, end)) => Some((
                            ScalarKirProjectionOperationKindV1::Cast { kind, value, to },
                            end,
                        )),
                        None => None,
                    }
                },
                None => None,
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_slice_data_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionOperationKindV1, nat)> {
    match scalar_kir_structural_value_id_v1(tokens, cursor) {
        Some((slice, end)) => Some((ScalarKirProjectionOperationKindV1::SliceData { slice }, end)),
        None => None,
    }
}

pub open spec fn scalar_kir_structural_gep_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionOperationKindV1, nat)> {
    match scalar_kir_structural_value_id_v1(tokens, cursor) {
        Some((base, base_next)) => {
            match scalar_kir_structural_value_id_v1(tokens, base_next) {
                Some((offset, end)) => Some((
                    ScalarKirProjectionOperationKindV1::GetElementPointer { base, offset },
                    end,
                )),
                None => None,
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_load_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionOperationKindV1, nat)> {
    match scalar_kir_structural_value_id_v1(tokens, cursor) {
        Some((pointer, pointer_next)) => {
            match scalar_kir_structural_memory_access_v1(tokens, pointer_next) {
                Some((access, end)) => Some((
                    ScalarKirProjectionOperationKindV1::Load { pointer, access },
                    end,
                )),
                None => None,
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_store_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionOperationKindV1, nat)> {
    match scalar_kir_structural_value_id_v1(tokens, cursor) {
        Some((pointer, pointer_next)) => {
            match scalar_kir_structural_value_id_v1(tokens, pointer_next) {
                Some((value, value_next)) => {
                    match scalar_kir_structural_memory_access_v1(tokens, value_next) {
                        Some((access, end)) => Some((
                            ScalarKirProjectionOperationKindV1::Store {
                                pointer,
                                value,
                                access,
                            },
                            end,
                        )),
                        None => None,
                    }
                },
                None => None,
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_operation_kind_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionOperationKindV1, nat)> {
    match scalar_kir_structural_enumeration_v1(tokens, cursor, 29) {
        Some((kind, next)) => {
            if kind == 1 {
                scalar_kir_structural_constant_v1(tokens, next)
            } else if kind == 2 {
                scalar_kir_structural_intrinsic_v1(tokens, next)
            } else if kind == 3 {
                scalar_kir_structural_binary_v1(tokens, next)
            } else if kind == 4 {
                scalar_kir_structural_compare_v1(tokens, next)
            } else if kind == 5 {
                scalar_kir_structural_cast_v1(tokens, next)
            } else if kind == 6 {
                scalar_kir_structural_slice_data_v1(tokens, next)
            } else if kind == 7 {
                scalar_kir_structural_gep_v1(tokens, next)
            } else if kind == 8 {
                scalar_kir_structural_load_v1(tokens, next)
            } else if kind == 9 {
                scalar_kir_structural_store_v1(tokens, next)
            } else {
                None
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_operation_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionOperationV1, nat)> {
    match scalar_kir_structural_count_v1(tokens, cursor, 28) {
        Some((result_count, result_next)) => {
            match scalar_kir_structural_value_defs_v1(tokens, result_next, result_count, seq![]) {
                Some((results, kind_next)) => {
                    match scalar_kir_structural_operation_kind_v1(tokens, kind_next) {
                        Some((kind, end)) => Some((ScalarKirProjectionOperationV1 {
                            results,
                            kind,
                        }, end)),
                        None => None,
                    }
                },
                None => None,
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_operations_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
    remaining: nat,
    operations: Seq<ScalarKirProjectionOperationV1>,
) -> Option<(Seq<ScalarKirProjectionOperationV1>, nat)>
    decreases remaining,
{
    if remaining == 0 {
        Some((operations, cursor))
    } else if cursor >= tokens.len() || remaining > tokens.len() - cursor {
        None
    } else {
        match scalar_kir_structural_operation_v1(tokens, cursor) {
            Some((operation, next)) => scalar_kir_structural_operations_v1(
                tokens,
                next,
                (remaining - 1) as nat,
                operations.push(operation),
            ),
            None => None,
        }
    }
}

pub open spec fn scalar_kir_structural_counted_value_ids_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
    count_tag: nat,
) -> Option<(Seq<nat>, nat)> {
    match scalar_kir_structural_count_v1(tokens, cursor, count_tag) {
        Some((count, next)) => scalar_kir_structural_value_ids_v1(tokens, next, count, seq![]),
        None => None,
    }
}

pub open spec fn scalar_kir_structural_terminator_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionTerminatorV1, nat)> {
    match scalar_kir_structural_enumeration_v1(tokens, cursor, 41) {
        Some((kind, next)) => {
            if kind == 1 {
                match scalar_kir_structural_u32_v1(tokens, next, 19) {
                    Some((target, target_next)) => {
                        match scalar_kir_structural_counted_value_ids_v1(tokens, target_next, 42) {
                            Some((arguments, end)) => Some((
                                ScalarKirProjectionTerminatorV1::Branch { target, arguments },
                                end,
                            )),
                            None => None,
                        }
                    },
                    None => None,
                }
            } else if kind == 2 {
                match scalar_kir_structural_value_id_v1(tokens, next) {
                    Some((condition, condition_next)) => {
                        match scalar_kir_structural_u32_v1(tokens, condition_next, 19) {
                            Some((then_target, then_target_next)) => {
                                match scalar_kir_structural_counted_value_ids_v1(
                                    tokens,
                                    then_target_next,
                                    43,
                                ) {
                                    Some((then_arguments, else_target_next)) => {
                                        match scalar_kir_structural_u32_v1(
                                            tokens,
                                            else_target_next,
                                            19,
                                        ) {
                                            Some((else_target, else_arguments_next)) => {
                                                match scalar_kir_structural_counted_value_ids_v1(
                                                    tokens,
                                                    else_arguments_next,
                                                    44,
                                                ) {
                                                    Some((else_arguments, end)) => Some((
                                                        ScalarKirProjectionTerminatorV1::ConditionalBranch {
                                                            condition,
                                                            then_target,
                                                            then_arguments,
                                                            else_target,
                                                            else_arguments,
                                                        },
                                                        end,
                                                    )),
                                                    None => None,
                                                }
                                            },
                                            None => None,
                                        }
                                    },
                                    None => None,
                                }
                            },
                            None => None,
                        }
                    },
                    None => None,
                }
            } else if kind == 3 {
                match scalar_kir_structural_counted_value_ids_v1(tokens, next, 42) {
                    Some((values, end)) => Some((
                        ScalarKirProjectionTerminatorV1::Return { values },
                        end,
                    )),
                    None => None,
                }
            } else {
                None
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_block_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionBlockV1, nat)> {
    match scalar_kir_structural_u32_v1(tokens, cursor, 19) {
        Some((id, id_next)) => {
            match scalar_kir_structural_count_v1(tokens, id_next, 20) {
                Some((parameter_count, parameter_next)) => {
                    match scalar_kir_structural_value_defs_v1(
                        tokens,
                        parameter_next,
                        parameter_count,
                        seq![],
                    ) {
                        Some((parameters, operation_count_next)) => {
                            match scalar_kir_structural_count_v1(
                                tokens,
                                operation_count_next,
                                21,
                            ) {
                                Some((operation_count, operation_next)) => {
                                    match scalar_kir_structural_operations_v1(
                                        tokens,
                                        operation_next,
                                        operation_count,
                                        seq![],
                                    ) {
                                        Some((operations, present_next)) => {
                                            match scalar_kir_structural_boolean_v1(
                                                tokens,
                                                present_next,
                                                22,
                                            ) {
                                                Some((false, end)) => Some((
                                                    ScalarKirProjectionBlockV1 {
                                                        id,
                                                        parameters,
                                                        operations,
                                                        terminator: None,
                                                    },
                                                    end,
                                                )),
                                                Some((true, terminator_next)) => {
                                                    match scalar_kir_structural_terminator_v1(
                                                        tokens,
                                                        terminator_next,
                                                    ) {
                                                        Some((terminator, end)) => Some((
                                                            ScalarKirProjectionBlockV1 {
                                                                id,
                                                                parameters,
                                                                operations,
                                                                terminator: Some(terminator),
                                                            },
                                                            end,
                                                        )),
                                                        None => None,
                                                    }
                                                },
                                                None => None,
                                            }
                                        },
                                        None => None,
                                    }
                                },
                                None => None,
                            }
                        },
                        None => None,
                    }
                },
                None => None,
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_blocks_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
    remaining: nat,
    blocks: Seq<ScalarKirProjectionBlockV1>,
) -> Option<(Seq<ScalarKirProjectionBlockV1>, nat)>
    decreases remaining,
{
    if remaining == 0 {
        Some((blocks, cursor))
    } else if cursor >= tokens.len() || remaining > tokens.len() - cursor {
        None
    } else {
        match scalar_kir_structural_block_v1(tokens, cursor) {
            Some((block, next)) => scalar_kir_structural_blocks_v1(
                tokens,
                next,
                (remaining - 1) as nat,
                blocks.push(block),
            ),
            None => None,
        }
    }
}

pub open spec fn scalar_kir_structural_signature_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionSignatureV1, nat)> {
    match scalar_kir_structural_count_v1(tokens, cursor, 13) {
        Some((parameter_count, parameter_next)) => {
            match scalar_kir_structural_types_v1(
                tokens,
                parameter_next,
                parameter_count,
                seq![],
            ) {
                Some((parameters, result_count_next)) => {
                    match scalar_kir_structural_count_v1(tokens, result_count_next, 14) {
                        Some((result_count, result_next)) => {
                            match scalar_kir_structural_types_v1(
                                tokens,
                                result_next,
                                result_count,
                                seq![],
                            ) {
                                Some((results, end)) => Some((ScalarKirProjectionSignatureV1 {
                                    parameters,
                                    results,
                                }, end)),
                                None => None,
                            }
                        },
                        None => None,
                    }
                },
                None => None,
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_body_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionBodyV1, nat)> {
    match scalar_kir_structural_count_v1(tokens, cursor, 16) {
        Some((parameter_count, parameter_next)) => {
            match scalar_kir_structural_value_ids_v1(
                tokens,
                parameter_next,
                parameter_count,
                seq![],
            ) {
                Some((parameters, block_count_next)) => {
                    match scalar_kir_structural_count_v1(tokens, block_count_next, 17) {
                        Some((block_count, block_next)) => {
                            match scalar_kir_structural_blocks_v1(
                                tokens,
                                block_next,
                                block_count,
                                seq![],
                            ) {
                                Some((blocks, end)) => Some((ScalarKirProjectionBodyV1 {
                                    parameters,
                                    blocks,
                                }, end)),
                                None => None,
                            }
                        },
                        None => None,
                    }
                },
                None => None,
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_counted_capabilities_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
    count_tag: nat,
) -> Option<(Seq<ScalarKirProjectionCapabilityV1>, nat)> {
    match scalar_kir_structural_count_v1(tokens, cursor, count_tag) {
        Some((count, next)) => scalar_kir_structural_capabilities_v1(tokens, next, count, seq![]),
        None => None,
    }
}

pub open spec fn scalar_kir_structural_function_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionFunctionV1, nat)> {
    match scalar_kir_structural_bytes_v1(tokens, cursor, 11) {
        Some((id, signature_next)) => {
            match scalar_kir_structural_signature_v1(tokens, signature_next) {
                Some((signature, role_next)) => {
                    match scalar_kir_structural_enumeration_v1(tokens, role_next, 12) {
                        Some((role, present_next)) => {
                            match scalar_kir_structural_boolean_v1(tokens, present_next, 15) {
                                Some((false, capability_next)) => {
                                    match scalar_kir_structural_counted_capabilities_v1(
                                        tokens,
                                        capability_next,
                                        18,
                                    ) {
                                        Some((capabilities, end)) => Some((
                                            ScalarKirProjectionFunctionV1 {
                                                id,
                                                signature,
                                                role,
                                                body: None,
                                                capabilities,
                                            },
                                            end,
                                        )),
                                        None => None,
                                    }
                                },
                                Some((true, body_next)) => {
                                    match scalar_kir_structural_body_v1(tokens, body_next) {
                                        Some((body, capability_next)) => {
                                            match scalar_kir_structural_counted_capabilities_v1(
                                                tokens,
                                                capability_next,
                                                18,
                                            ) {
                                                Some((capabilities, end)) => Some((
                                                    ScalarKirProjectionFunctionV1 {
                                                        id,
                                                        signature,
                                                        role,
                                                        body: Some(body),
                                                        capabilities,
                                                    },
                                                    end,
                                                )),
                                                None => None,
                                            }
                                        },
                                        None => None,
                                    }
                                },
                                None => None,
                            }
                        },
                        None => None,
                    }
                },
                None => None,
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_functions_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
    remaining: nat,
    functions: Seq<ScalarKirProjectionFunctionV1>,
) -> Option<(Seq<ScalarKirProjectionFunctionV1>, nat)>
    decreases remaining,
{
    if remaining == 0 {
        Some((functions, cursor))
    } else if cursor >= tokens.len() || remaining > tokens.len() - cursor {
        None
    } else {
        match scalar_kir_structural_function_v1(tokens, cursor) {
            Some((function, next)) => scalar_kir_structural_functions_v1(
                tokens,
                next,
                (remaining - 1) as nat,
                functions.push(function),
            ),
            None => None,
        }
    }
}

pub open spec fn scalar_kir_structural_launch_extent_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionLaunchExtentV1, nat)> {
    match scalar_kir_structural_enumeration_v1(tokens, cursor, 48) {
        Some((kind, next)) => {
            if kind == 1 {
                Some((ScalarKirProjectionLaunchExtentV1::Dynamic, next))
            } else if kind == 2 {
                match scalar_kir_structural_u32_v1(tokens, next, 49) {
                    Some((value, end)) => Some((
                        ScalarKirProjectionLaunchExtentV1::Static { value },
                        end,
                    )),
                    None => None,
                }
            } else {
                None
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_launch_domain_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionLaunchDomainV1, nat)> {
    match scalar_kir_structural_enumeration_v1(tokens, cursor, 47) {
        Some((kind, next)) => {
            if kind == 1 {
                match scalar_kir_structural_launch_extent_v1(tokens, next) {
                    Some((x, end)) => Some((ScalarKirProjectionLaunchDomainV1::D1 { x }, end)),
                    None => None,
                }
            } else if kind == 2 {
                match scalar_kir_structural_launch_extent_v1(tokens, next) {
                    Some((x, y_next)) => {
                        match scalar_kir_structural_launch_extent_v1(tokens, y_next) {
                            Some((y, end)) => Some((
                                ScalarKirProjectionLaunchDomainV1::D2 { x, y },
                                end,
                            )),
                            None => None,
                        }
                    },
                    None => None,
                }
            } else if kind == 3 {
                match scalar_kir_structural_launch_extent_v1(tokens, next) {
                    Some((x, y_next)) => {
                        match scalar_kir_structural_launch_extent_v1(tokens, y_next) {
                            Some((y, z_next)) => {
                                match scalar_kir_structural_launch_extent_v1(tokens, z_next) {
                                    Some((z, end)) => Some((
                                        ScalarKirProjectionLaunchDomainV1::D3 { x, y, z },
                                        end,
                                    )),
                                    None => None,
                                }
                            },
                            None => None,
                        }
                    },
                    None => None,
                }
            } else {
                None
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_workgroup_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(Option<ScalarKirProjectionWorkgroupV1>, nat)> {
    match scalar_kir_structural_boolean_v1(tokens, cursor, 50) {
        Some((false, end)) => Some((None, end)),
        Some((true, x_next)) => {
            match scalar_kir_structural_u32_v1(tokens, x_next, 51) {
                Some((x, y_next)) => {
                    match scalar_kir_structural_u32_v1(tokens, y_next, 52) {
                        Some((y, z_next)) => {
                            match scalar_kir_structural_u32_v1(tokens, z_next, 53) {
                                Some((z, end)) => Some((Some(ScalarKirProjectionWorkgroupV1 {
                                    x,
                                    y,
                                    z,
                                }), end)),
                                None => None,
                            }
                        },
                        None => None,
                    }
                },
                None => None,
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_kernel_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
) -> Option<(ScalarKirProjectionKernelV1, nat)> {
    match scalar_kir_structural_bytes_v1(tokens, cursor, 45) {
        Some((id, entry_next)) => {
            match scalar_kir_structural_bytes_v1(tokens, entry_next, 46) {
                Some((entry, domain_next)) => {
                    match scalar_kir_structural_launch_domain_v1(tokens, domain_next) {
                        Some((domain, workgroup_next)) => {
                            match scalar_kir_structural_workgroup_v1(tokens, workgroup_next) {
                                Some((workgroup, capability_next)) => {
                                    match scalar_kir_structural_counted_capabilities_v1(
                                        tokens,
                                        capability_next,
                                        54,
                                    ) {
                                        Some((capabilities, end)) => Some((
                                            ScalarKirProjectionKernelV1 {
                                                id,
                                                entry,
                                                domain,
                                                workgroup,
                                                capabilities,
                                            },
                                            end,
                                        )),
                                        None => None,
                                    }
                                },
                                None => None,
                            }
                        },
                        None => None,
                    }
                },
                None => None,
            }
        },
        None => None,
    }
}

pub open spec fn scalar_kir_structural_kernels_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
    cursor: nat,
    remaining: nat,
    kernels: Seq<ScalarKirProjectionKernelV1>,
) -> Option<(Seq<ScalarKirProjectionKernelV1>, nat)>
    decreases remaining,
{
    if remaining == 0 {
        Some((kernels, cursor))
    } else if cursor >= tokens.len() || remaining > tokens.len() - cursor {
        None
    } else {
        match scalar_kir_structural_kernel_v1(tokens, cursor) {
            Some((kernel, next)) => scalar_kir_structural_kernels_v1(
                tokens,
                next,
                (remaining - 1) as nat,
                kernels.push(kernel),
            ),
            None => None,
        }
    }
}

pub open spec fn scalar_kir_structural_module_v1(
    tokens: Seq<ScalarKirTypedTokenV1>,
) -> ScalarKirProjectionAstDecodeV1 {
    match scalar_kir_structural_bytes_v1(tokens, 0, 1) {
        Some((schema, policy_next)) => {
            match scalar_kir_structural_u16_v1(tokens, policy_next, 2) {
                Some((policy, id_next)) => {
                    match scalar_kir_structural_bytes_v1(tokens, id_next, 3) {
                        Some((id, capability_count_next)) => {
                            match scalar_kir_structural_counted_capabilities_v1(
                                tokens,
                                capability_count_next,
                                4,
                            ) {
                                Some((capabilities, function_count_next)) => {
                                    match scalar_kir_structural_count_v1(
                                        tokens,
                                        function_count_next,
                                        5,
                                    ) {
                                        Some((function_count, function_next)) => {
                                            match scalar_kir_structural_functions_v1(
                                                tokens,
                                                function_next,
                                                function_count,
                                                seq![],
                                            ) {
                                                Some((functions, kernel_count_next)) => {
                                                    match scalar_kir_structural_count_v1(
                                                        tokens,
                                                        kernel_count_next,
                                                        6,
                                                    ) {
                                                        Some((kernel_count, kernel_next)) => {
                                                            match scalar_kir_structural_kernels_v1(
                                                                tokens,
                                                                kernel_next,
                                                                kernel_count,
                                                                seq![],
                                                            ) {
                                                                Some((kernels, end)) => {
                                                                    if end == tokens.len() {
                                                                        ScalarKirProjectionAstDecodeV1::Complete {
                                                                            module: ScalarKirProjectionModuleV1 {
                                                                                schema,
                                                                                policy,
                                                                                id,
                                                                                capabilities,
                                                                                functions,
                                                                                kernels,
                                                                            },
                                                                        }
                                                                    } else {
                                                                        ScalarKirProjectionAstDecodeV1::Invalid
                                                                    }
                                                                },
                                                                None => ScalarKirProjectionAstDecodeV1::Invalid,
                                                            }
                                                        },
                                                        None => ScalarKirProjectionAstDecodeV1::Invalid,
                                                    }
                                                },
                                                None => ScalarKirProjectionAstDecodeV1::Invalid,
                                            }
                                        },
                                        None => ScalarKirProjectionAstDecodeV1::Invalid,
                                    }
                                },
                                None => ScalarKirProjectionAstDecodeV1::Invalid,
                            }
                        },
                        None => ScalarKirProjectionAstDecodeV1::Invalid,
                    }
                },
                None => ScalarKirProjectionAstDecodeV1::Invalid,
            }
        },
        None => ScalarKirProjectionAstDecodeV1::Invalid,
    }
}

pub open spec fn scalar_kir_projection_ast_decode_v1(
    bytes: Seq<u8>,
) -> ScalarKirProjectionAstDecodeV1 {
    match scalar_kir_typed_decode_v1(bytes) {
        ScalarKirTypedDecodeV1::Invalid => ScalarKirProjectionAstDecodeV1::Invalid,
        ScalarKirTypedDecodeV1::Complete { records } => scalar_kir_structural_module_v1(records),
    }
}

pub open spec fn scalar_kir_projection_ast_is_complete_v1(bytes: Seq<u8>) -> bool {
    match scalar_kir_projection_ast_decode_v1(bytes) {
        ScalarKirProjectionAstDecodeV1::Complete { .. } => true,
        ScalarKirProjectionAstDecodeV1::Invalid => false,
    }
}

pub open spec fn scalar_kir_projection_ast_has_scalar_shape_v1(bytes: Seq<u8>) -> bool {
    match scalar_kir_projection_ast_decode_v1(bytes) {
        ScalarKirProjectionAstDecodeV1::Invalid => false,
        ScalarKirProjectionAstDecodeV1::Complete { module } => {
            if module.policy != 1
                || module.capabilities.len() != 2
                || module.functions.len() != 1
                || module.kernels.len() != 1
            {
                false
            } else {
                match module.functions[0].body {
                    None => false,
                    Some(body) => {
                        body.parameters.len() == 6
                            && body.blocks.len() == 6
                            && body.blocks[0].id == 0
                            && body.blocks[0].operations.len() == 9
                            && body.blocks[1].id == 1
                            && body.blocks[1].operations.len() == 4
                            && body.blocks[2].id == 2
                            && body.blocks[2].operations.len() == 1
                            && body.blocks[3].id == 3
                            && body.blocks[3].operations.len() == 13
                            && body.blocks[4].id == 4
                            && body.blocks[4].operations.len() == 2
                            && body.blocks[5].id == 5
                            && body.blocks[5].operations.len() == 0
                    },
                }
            }
        },
    }
}

pub proof fn generated_scalar_kir_projection_decodes_to_structural_ast_v1()
    ensures
        scalar_kir_projection_ast_is_complete_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        ),
        scalar_kir_projection_ast_has_scalar_shape_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        ),
{
    assert(
        scalar_kir_projection_ast_is_complete_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        )
    ) by (compute);
    assert(
        scalar_kir_projection_ast_has_scalar_shape_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        )
    ) by (compute);
}

} // verus!

} // mod scalar_gemm_kir_projection_ast_v1
