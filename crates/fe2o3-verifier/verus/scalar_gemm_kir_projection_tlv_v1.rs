// Verified TLV framing for the exact scalar GEMM semantic projection.
//
// This layer proves record boundaries and exact end-of-input consumption. It
// does not assign context-sensitive meanings to tags; typed projection decoding
// and correspondence to the operational state machine are separate theorems.

pub mod scalar_gemm_kir_projection_tlv_v1 {

use vstd::prelude::*;

use super::scalar_gemm_kir_projection_generated_v1::FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1;

verus! {

pub enum ScalarKirTlvFramingV1 {
    Invalid,
    Complete { records: nat },
}

pub open spec fn scalar_kir_tlv_u32_le_v1(bytes: Seq<u8>, cursor: nat) -> nat
    recommends cursor + 4 <= bytes.len(),
{
    bytes[cursor as int] as nat
        + 256 * bytes[(cursor + 1) as int] as nat
        + 65_536 * bytes[(cursor + 2) as int] as nat
        + 16_777_216 * bytes[(cursor + 3) as int] as nat
}

/// Total framing from one cursor to exact end-of-input.
pub open spec fn scalar_kir_tlv_frame_suffix_v1(
    bytes: Seq<u8>,
    cursor: nat,
) -> ScalarKirTlvFramingV1
    decreases bytes.len() - cursor,
{
    if cursor > bytes.len() {
        ScalarKirTlvFramingV1::Invalid
    } else if cursor == bytes.len() {
        ScalarKirTlvFramingV1::Complete { records: 0 }
    } else if cursor + 5 > bytes.len() {
        ScalarKirTlvFramingV1::Invalid
    } else {
        let payload_len = scalar_kir_tlv_u32_le_v1(bytes, cursor + 1);
        let next = cursor + 5 + payload_len;
        if next > bytes.len() {
            ScalarKirTlvFramingV1::Invalid
        } else {
            match scalar_kir_tlv_frame_suffix_v1(bytes, next) {
                ScalarKirTlvFramingV1::Invalid => ScalarKirTlvFramingV1::Invalid,
                ScalarKirTlvFramingV1::Complete { records } => {
                    ScalarKirTlvFramingV1::Complete { records: records + 1 }
                },
            }
        }
    }
}

pub open spec fn scalar_kir_tlv_frame_v1(bytes: Seq<u8>) -> ScalarKirTlvFramingV1 {
    scalar_kir_tlv_frame_suffix_v1(bytes, 0)
}

pub proof fn generated_scalar_kir_projection_has_exact_tlv_framing_v1()
    ensures
        FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1.len() == 2_927,
        scalar_kir_tlv_frame_v1(FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@)
            == (ScalarKirTlvFramingV1::Complete { records: 370 }),
{
    assert(
        scalar_kir_tlv_frame_v1(FE2O3_GENERATED_SCALAR_KIR_PROJECTION_V1@)
            == (ScalarKirTlvFramingV1::Complete { records: 370 })
    ) by (compute);
}

} // verus!

} // mod scalar_gemm_kir_projection_tlv_v1
