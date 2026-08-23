// Total typed decoding for the scalar GEMM semantic projection.
//
// The decoder validates every TLV tag, payload width, finite enum range, and
// boolean value. Capability arguments and constant payloads are interpreted
// using the preceding kind, so equal but contextually malformed TLV streams
// fail closed. Structural graph-to-state-machine correspondence is proved in a
// separate layer over the typed token stream returned here.

pub mod scalar_gemm_kir_projection_typed_v1 {

use vstd::prelude::*;

use super::scalar_gemm_kir_projection_generated_v1::FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1;
use super::scalar_gemm_kir_projection_tlv_v1::scalar_kir_tlv_u32_le_v1;

verus! {

pub open spec fn max_scalar_kir_typed_projection_bytes_v1() -> nat {
    16_384
}

#[allow(inconsistent_fields)]
pub enum ScalarKirTypedTokenV1 {
    Bytes { tag: nat, value: Seq<u8> },
    Count { tag: nat, value: nat },
    U16 { tag: nat, value: nat },
    U32 { tag: nat, value: nat },
    Boolean { tag: nat, value: bool },
    Enumeration { tag: nat, value: nat },
    CapabilityKind { kind: nat },
    CapabilitySubgroupSize { value: nat },
    CapabilityAtomicWidth { value: nat },
    CapabilityAtomicAddressSpace { value: nat },
    CapabilityAtomicScope { value: nat },
    CapabilityExtensionNamespace { value: Seq<u8> },
    CapabilityExtensionName { value: Seq<u8> },
    CapabilityWaveWidth { value: nat },
    ConstantKind { kind: nat },
    ConstantBits { kind: nat, width: nat, value: nat },
}

pub enum ScalarKirTypedContextV1 {
    Idle,
    CapabilitySubgroupSize,
    CapabilityAtomicWidth,
    CapabilityAtomicAddressSpace,
    CapabilityAtomicScope,
    CapabilityExtensionNamespace,
    CapabilityExtensionName,
    CapabilityWaveWidth,
    ConstantValue { kind: nat, width: nat },
}

pub enum ScalarKirTypedRecordDecodeV1 {
    Invalid,
    Decoded {
        token: ScalarKirTypedTokenV1,
        next_context: ScalarKirTypedContextV1,
    },
}

pub enum ScalarKirTypedDecodeV1 {
    Invalid,
    Complete { records: Seq<ScalarKirTypedTokenV1> },
}

pub open spec fn scalar_kir_typed_u8_v1(payload: Seq<u8>) -> nat
    recommends payload.len() == 1,
{
    payload[0] as nat
}

pub open spec fn scalar_kir_typed_u16_le_v1(payload: Seq<u8>) -> nat
    recommends payload.len() == 2,
{
    payload[0] as nat + 256 * payload[1] as nat
}

pub open spec fn scalar_kir_typed_u32_le_v1(payload: Seq<u8>) -> nat
    recommends payload.len() == 4,
{
    payload[0] as nat
        + 256 * payload[1] as nat
        + 65_536 * payload[2] as nat
        + 16_777_216 * payload[3] as nat
}

pub open spec fn scalar_kir_typed_u64_le_v1(payload: Seq<u8>) -> nat
    recommends payload.len() == 8,
{
    payload[0] as nat
        + 256 * payload[1] as nat
        + 65_536 * payload[2] as nat
        + 16_777_216 * payload[3] as nat
        + 4_294_967_296 * payload[4] as nat
        + 1_099_511_627_776 * payload[5] as nat
        + 281_474_976_710_656 * payload[6] as nat
        + 72_057_594_037_927_936 * payload[7] as nat
}

pub open spec fn scalar_kir_typed_payload_value_v1(payload: Seq<u8>) -> nat
{
    if payload.len() == 1 {
        scalar_kir_typed_u8_v1(payload)
    } else if payload.len() == 2 {
        scalar_kir_typed_u16_le_v1(payload)
    } else if payload.len() == 4 {
        scalar_kir_typed_u32_le_v1(payload)
    } else if payload.len() == 8 {
        scalar_kir_typed_u64_le_v1(payload)
    } else {
        0
    }
}

pub open spec fn scalar_kir_typed_is_count_tag_v1(tag: nat) -> bool {
    tag == 4
        || tag == 5
        || tag == 6
        || tag == 13
        || tag == 14
        || tag == 16
        || tag == 17
        || tag == 18
        || tag == 20
        || tag == 21
        || tag == 28
        || tag == 42
        || tag == 43
        || tag == 44
        || tag == 54
}

pub open spec fn scalar_kir_typed_is_u32_tag_v1(tag: nat) -> bool {
    tag == 19
        || tag == 23
        || tag == 39
        || tag == 49
        || tag == 51
        || tag == 52
        || tag == 53
}

pub open spec fn scalar_kir_typed_is_boolean_tag_v1(tag: nat) -> bool {
    tag == 15 || tag == 22 || tag == 40 || tag == 50
}

pub open spec fn scalar_kir_typed_enum_limit_v1(tag: nat) -> nat {
    if tag == 12 {
        4
    } else if tag == 24 {
        4
    } else if tag == 25 {
        16
    } else if tag == 26 {
        5
    } else if tag == 27 {
        2
    } else if tag == 29 {
        9
    } else if tag == 32 {
        2
    } else if tag == 33 {
        5
    } else if tag == 34 {
        3
    } else if tag == 35 {
        10
    } else if tag == 36 {
        6
    } else if tag == 37 {
        8
    } else if tag == 38 {
        5
    } else if tag == 41 {
        3
    } else if tag == 47 {
        3
    } else if tag == 48 {
        2
    } else {
        0
    }
}

pub open spec fn scalar_kir_typed_constant_width_v1(kind: nat) -> nat {
    if kind == 1 || kind == 2 || kind == 6 {
        1
    } else if kind == 3 || kind == 7 || kind == 11 || kind == 12 {
        2
    } else if kind == 4 || kind == 8 || kind == 13 {
        4
    } else if kind == 5 || kind == 9 || kind == 10 || kind == 14 {
        8
    } else {
        0
    }
}

pub open spec fn scalar_kir_typed_decode_idle_record_v1(
    tag: nat,
    payload: Seq<u8>,
) -> ScalarKirTypedRecordDecodeV1 {
    if tag == 1 || tag == 3 || tag == 11 || tag == 45 || tag == 46 {
        ScalarKirTypedRecordDecodeV1::Decoded {
            token: ScalarKirTypedTokenV1::Bytes { tag, value: payload },
            next_context: ScalarKirTypedContextV1::Idle,
        }
    } else if tag == 2 && payload.len() == 2 {
        ScalarKirTypedRecordDecodeV1::Decoded {
            token: ScalarKirTypedTokenV1::U16 {
                tag,
                value: scalar_kir_typed_u16_le_v1(payload),
            },
            next_context: ScalarKirTypedContextV1::Idle,
        }
    } else if scalar_kir_typed_is_count_tag_v1(tag) && payload.len() == 4 {
        ScalarKirTypedRecordDecodeV1::Decoded {
            token: ScalarKirTypedTokenV1::Count {
                tag,
                value: scalar_kir_typed_u32_le_v1(payload),
            },
            next_context: ScalarKirTypedContextV1::Idle,
        }
    } else if scalar_kir_typed_is_u32_tag_v1(tag) && payload.len() == 4 {
        ScalarKirTypedRecordDecodeV1::Decoded {
            token: ScalarKirTypedTokenV1::U32 {
                tag,
                value: scalar_kir_typed_u32_le_v1(payload),
            },
            next_context: ScalarKirTypedContextV1::Idle,
        }
    } else if scalar_kir_typed_is_boolean_tag_v1(tag)
        && payload.len() == 1
        && scalar_kir_typed_u8_v1(payload) <= 1
    {
        ScalarKirTypedRecordDecodeV1::Decoded {
            token: ScalarKirTypedTokenV1::Boolean {
                tag,
                value: scalar_kir_typed_u8_v1(payload) == 1,
            },
            next_context: ScalarKirTypedContextV1::Idle,
        }
    } else if scalar_kir_typed_enum_limit_v1(tag) > 0
        && payload.len() == 1
        && scalar_kir_typed_u8_v1(payload) >= 1
        && scalar_kir_typed_u8_v1(payload) <= scalar_kir_typed_enum_limit_v1(tag)
    {
        ScalarKirTypedRecordDecodeV1::Decoded {
            token: ScalarKirTypedTokenV1::Enumeration {
                tag,
                value: scalar_kir_typed_u8_v1(payload),
            },
            next_context: ScalarKirTypedContextV1::Idle,
        }
    } else if tag == 7
        && payload.len() == 1
        && scalar_kir_typed_u8_v1(payload) >= 1
        && scalar_kir_typed_u8_v1(payload) <= 12
    {
        let kind = scalar_kir_typed_u8_v1(payload);
        let next_context = if kind == 6 {
            ScalarKirTypedContextV1::CapabilitySubgroupSize
        } else if kind == 9 {
            ScalarKirTypedContextV1::CapabilityAtomicWidth
        } else if kind == 11 {
            ScalarKirTypedContextV1::CapabilityExtensionNamespace
        } else if kind == 12 {
            ScalarKirTypedContextV1::CapabilityWaveWidth
        } else {
            ScalarKirTypedContextV1::Idle
        };
        ScalarKirTypedRecordDecodeV1::Decoded {
            token: ScalarKirTypedTokenV1::CapabilityKind { kind },
            next_context,
        }
    } else if tag == 30
        && payload.len() == 1
        && scalar_kir_typed_constant_width_v1(scalar_kir_typed_u8_v1(payload)) > 0
    {
        let kind = scalar_kir_typed_u8_v1(payload);
        ScalarKirTypedRecordDecodeV1::Decoded {
            token: ScalarKirTypedTokenV1::ConstantKind { kind },
            next_context: ScalarKirTypedContextV1::ConstantValue {
                kind,
                width: scalar_kir_typed_constant_width_v1(kind),
            },
        }
    } else {
        ScalarKirTypedRecordDecodeV1::Invalid
    }
}

pub open spec fn scalar_kir_typed_decode_record_v1(
    tag: nat,
    payload: Seq<u8>,
    context: ScalarKirTypedContextV1,
) -> ScalarKirTypedRecordDecodeV1 {
    match context {
        ScalarKirTypedContextV1::Idle => scalar_kir_typed_decode_idle_record_v1(tag, payload),
        ScalarKirTypedContextV1::CapabilitySubgroupSize => {
            if tag == 8 && payload.len() == 4 {
                ScalarKirTypedRecordDecodeV1::Decoded {
                    token: ScalarKirTypedTokenV1::CapabilitySubgroupSize {
                        value: scalar_kir_typed_u32_le_v1(payload),
                    },
                    next_context: ScalarKirTypedContextV1::Idle,
                }
            } else {
                ScalarKirTypedRecordDecodeV1::Invalid
            }
        },
        ScalarKirTypedContextV1::CapabilityAtomicWidth => {
            if tag == 8 && payload.len() == 2 {
                ScalarKirTypedRecordDecodeV1::Decoded {
                    token: ScalarKirTypedTokenV1::CapabilityAtomicWidth {
                        value: scalar_kir_typed_u16_le_v1(payload),
                    },
                    next_context: ScalarKirTypedContextV1::CapabilityAtomicAddressSpace,
                }
            } else {
                ScalarKirTypedRecordDecodeV1::Invalid
            }
        },
        ScalarKirTypedContextV1::CapabilityAtomicAddressSpace => {
            if tag == 26
                && payload.len() == 1
                && scalar_kir_typed_u8_v1(payload) >= 1
                && scalar_kir_typed_u8_v1(payload) <= 5
            {
                ScalarKirTypedRecordDecodeV1::Decoded {
                    token: ScalarKirTypedTokenV1::CapabilityAtomicAddressSpace {
                        value: scalar_kir_typed_u8_v1(payload),
                    },
                    next_context: ScalarKirTypedContextV1::CapabilityAtomicScope,
                }
            } else {
                ScalarKirTypedRecordDecodeV1::Invalid
            }
        },
        ScalarKirTypedContextV1::CapabilityAtomicScope => {
            if tag == 8
                && payload.len() == 1
                && scalar_kir_typed_u8_v1(payload) >= 1
                && scalar_kir_typed_u8_v1(payload) <= 5
            {
                ScalarKirTypedRecordDecodeV1::Decoded {
                    token: ScalarKirTypedTokenV1::CapabilityAtomicScope {
                        value: scalar_kir_typed_u8_v1(payload),
                    },
                    next_context: ScalarKirTypedContextV1::Idle,
                }
            } else {
                ScalarKirTypedRecordDecodeV1::Invalid
            }
        },
        ScalarKirTypedContextV1::CapabilityExtensionNamespace => {
            if tag == 9 {
                ScalarKirTypedRecordDecodeV1::Decoded {
                    token: ScalarKirTypedTokenV1::CapabilityExtensionNamespace {
                        value: payload,
                    },
                    next_context: ScalarKirTypedContextV1::CapabilityExtensionName,
                }
            } else {
                ScalarKirTypedRecordDecodeV1::Invalid
            }
        },
        ScalarKirTypedContextV1::CapabilityExtensionName => {
            if tag == 10 {
                ScalarKirTypedRecordDecodeV1::Decoded {
                    token: ScalarKirTypedTokenV1::CapabilityExtensionName { value: payload },
                    next_context: ScalarKirTypedContextV1::Idle,
                }
            } else {
                ScalarKirTypedRecordDecodeV1::Invalid
            }
        },
        ScalarKirTypedContextV1::CapabilityWaveWidth => {
            if tag == 8
                && payload.len() == 1
                && scalar_kir_typed_u8_v1(payload) >= 1
                && scalar_kir_typed_u8_v1(payload) <= 2
            {
                ScalarKirTypedRecordDecodeV1::Decoded {
                    token: ScalarKirTypedTokenV1::CapabilityWaveWidth {
                        value: scalar_kir_typed_u8_v1(payload),
                    },
                    next_context: ScalarKirTypedContextV1::Idle,
                }
            } else {
                ScalarKirTypedRecordDecodeV1::Invalid
            }
        },
        ScalarKirTypedContextV1::ConstantValue { kind, width } => {
            if tag == 31
                && width == scalar_kir_typed_constant_width_v1(kind)
                && width > 0
                && payload.len() == width
            {
                let value = scalar_kir_typed_payload_value_v1(payload);
                if kind == 1 && value > 1 {
                    ScalarKirTypedRecordDecodeV1::Invalid
                } else {
                    ScalarKirTypedRecordDecodeV1::Decoded {
                        token: ScalarKirTypedTokenV1::ConstantBits {
                            kind,
                            width,
                            value,
                        },
                        next_context: ScalarKirTypedContextV1::Idle,
                    }
                }
            } else {
                ScalarKirTypedRecordDecodeV1::Invalid
            }
        },
    }
}

pub open spec fn scalar_kir_typed_decode_suffix_v1(
    bytes: Seq<u8>,
    cursor: nat,
    context: ScalarKirTypedContextV1,
    records: Seq<ScalarKirTypedTokenV1>,
) -> ScalarKirTypedDecodeV1
    decreases bytes.len() - cursor,
{
    if cursor > bytes.len() {
        ScalarKirTypedDecodeV1::Invalid
    } else if cursor == bytes.len() {
        match context {
            ScalarKirTypedContextV1::Idle => ScalarKirTypedDecodeV1::Complete { records },
            _ => ScalarKirTypedDecodeV1::Invalid,
        }
    } else if cursor + 5 > bytes.len() {
        ScalarKirTypedDecodeV1::Invalid
    } else {
        let payload_len = scalar_kir_tlv_u32_le_v1(bytes, cursor + 1);
        let next = cursor + 5 + payload_len;
        if next > bytes.len() {
            ScalarKirTypedDecodeV1::Invalid
        } else {
            let tag = bytes[cursor as int] as nat;
            let payload = bytes.subrange((cursor + 5) as int, next as int);
            match scalar_kir_typed_decode_record_v1(tag, payload, context) {
                ScalarKirTypedRecordDecodeV1::Invalid => ScalarKirTypedDecodeV1::Invalid,
                ScalarKirTypedRecordDecodeV1::Decoded { token, next_context } => {
                    scalar_kir_typed_decode_suffix_v1(
                        bytes,
                        next,
                        next_context,
                        records.push(token),
                    )
                },
            }
        }
    }
}

pub open spec fn scalar_kir_typed_decode_v1(bytes: Seq<u8>) -> ScalarKirTypedDecodeV1 {
    if bytes.len() == 0 || bytes.len() > max_scalar_kir_typed_projection_bytes_v1() {
        ScalarKirTypedDecodeV1::Invalid
    } else {
        scalar_kir_typed_decode_suffix_v1(
            bytes,
            0,
            ScalarKirTypedContextV1::Idle,
            seq![],
        )
    }
}

pub open spec fn scalar_kir_typed_decode_is_complete_v1(bytes: Seq<u8>) -> bool {
    match scalar_kir_typed_decode_v1(bytes) {
        ScalarKirTypedDecodeV1::Complete { .. } => true,
        ScalarKirTypedDecodeV1::Invalid => false,
    }
}

pub open spec fn scalar_kir_typed_records_v1(
    bytes: Seq<u8>,
) -> Seq<ScalarKirTypedTokenV1> {
    match scalar_kir_typed_decode_v1(bytes) {
        ScalarKirTypedDecodeV1::Complete { records } => records,
        ScalarKirTypedDecodeV1::Invalid => seq![],
    }
}

pub proof fn scalar_kir_typed_constant_contexts_fail_closed_v1()
    ensures
        scalar_kir_typed_decode_record_v1(
            31,
            seq![2u8],
            ScalarKirTypedContextV1::ConstantValue { kind: 1, width: 1 },
        ) == ScalarKirTypedRecordDecodeV1::Invalid,
        scalar_kir_typed_decode_record_v1(
            31,
            seq![0u8, 0u8, 0u8],
            ScalarKirTypedContextV1::ConstantValue { kind: 8, width: 3 },
        ) == ScalarKirTypedRecordDecodeV1::Invalid,
{
    assert(
        scalar_kir_typed_decode_record_v1(
            31,
            seq![2u8],
            ScalarKirTypedContextV1::ConstantValue { kind: 1, width: 1 },
        ) == ScalarKirTypedRecordDecodeV1::Invalid
    ) by (compute);
    assert(
        scalar_kir_typed_decode_record_v1(
            31,
            seq![0u8, 0u8, 0u8],
            ScalarKirTypedContextV1::ConstantValue { kind: 8, width: 3 },
        ) == ScalarKirTypedRecordDecodeV1::Invalid
    ) by (compute);
}

pub proof fn generated_scalar_kir_projection_decodes_to_typed_records_v1()
    ensures
        scalar_kir_typed_decode_is_complete_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        ),
        scalar_kir_typed_records_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        ).len() == 370,
        scalar_kir_typed_records_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        )[8] == (ScalarKirTypedTokenV1::CapabilityWaveWidth { value: 2 }),
        scalar_kir_typed_records_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        )[159] == (ScalarKirTypedTokenV1::ConstantBits { kind: 8, width: 4, value: 0 }),
        scalar_kir_typed_records_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        )[166] == (ScalarKirTypedTokenV1::ConstantBits { kind: 13, width: 4, value: 0 }),
        scalar_kir_typed_records_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        )[302] == (ScalarKirTypedTokenV1::ConstantBits { kind: 8, width: 4, value: 1 }),
        scalar_kir_typed_records_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        )[354] == (ScalarKirTypedTokenV1::CapabilityWaveWidth { value: 2 }),
        scalar_kir_typed_records_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        )[369] == (ScalarKirTypedTokenV1::CapabilityWaveWidth { value: 2 }),
{
    assert(
        scalar_kir_typed_decode_is_complete_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        )
    ) by (compute);
    assert(
        scalar_kir_typed_records_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        ).len() == 370
    ) by (compute);
    assert(
        scalar_kir_typed_records_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        )[8] == (ScalarKirTypedTokenV1::CapabilityWaveWidth { value: 2 })
    ) by (compute);
    assert(
        scalar_kir_typed_records_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        )[159] == (ScalarKirTypedTokenV1::ConstantBits { kind: 8, width: 4, value: 0 })
    ) by (compute);
    assert(
        scalar_kir_typed_records_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        )[166] == (ScalarKirTypedTokenV1::ConstantBits { kind: 13, width: 4, value: 0 })
    ) by (compute);
    assert(
        scalar_kir_typed_records_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        )[302] == (ScalarKirTypedTokenV1::ConstantBits { kind: 8, width: 4, value: 1 })
    ) by (compute);
    assert(
        scalar_kir_typed_records_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        )[354] == (ScalarKirTypedTokenV1::CapabilityWaveWidth { value: 2 })
    ) by (compute);
    assert(
        scalar_kir_typed_records_v1(
            FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@
        )[369] == (ScalarKirTypedTokenV1::CapabilityWaveWidth { value: 2 })
    ) by (compute);
}

} // verus!

} // mod scalar_gemm_kir_projection_typed_v1
