// Composition of the actual sequential header and integer accessor bodies.
// This does not refine complete Reader/Writer objects or either full wire codec.
include!("distributed_codec_primitives_v1.rs");
include!("../src/distributed_publication_contract/codec_fields_body.rs");

verus! {

// Bound to the identical production declaration by the field source guard.
const DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1: u16 = 1;

proof fn two_byte_sequence_view(value: Seq<u8>)
    ensures value.len() == 2 ==> value == seq![value[0], value[1]],
{
    if value.len() == 2 {
        assert(value =~= seq![value[0], value[1]]) by {
            assert forall|i: int| 0 <= i < 2
                implies value[i] == seq![value[0], value[1]][i] by {
                assert(i == 0 || i == 1);
            }
        }
    }
}

fn bytes_equal(left: &[u8], right: &[u8]) -> (result: bool)
    ensures result == (left@ == right@),
{
    let result = distributed_codec_bytes_equal_body_v1!(verus_exec_expr, left, right, index, [
        invariant
            index <= left@.len(),
            left@.len() == right@.len(),
            forall|i: int| 0 <= i < index ==> #[trigger] left@[i] == right@[i],
        decreases left@.len() - index,
    ]);
    proof {
        assert(left@ =~= right@);
    }
    result
}

spec fn header_read_result(bytes: Seq<u8>, start: int, domain: Seq<u8>)
    -> (Result<(), E>, int)
{
    let schema_start = start + domain.len();
    let reserved_start = schema_start + 2;
    let end = reserved_start + 2;
    if schema_start > bytes.len() {
        (Err(E::WrongLength), start)
    } else if bytes.subrange(start, schema_start) != domain {
        (Err(E::WrongDomain), schema_start)
    } else if reserved_start > bytes.len() {
        (Err(E::WrongLength), schema_start)
    } else if ((bytes[schema_start] as u16) | ((bytes[schema_start + 1] as u16) << 8))
        != DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1
    {
        (Err(E::WrongSchema), reserved_start)
    } else if end > bytes.len() {
        (Err(E::WrongLength), reserved_start)
    } else if bytes.subrange(reserved_start, end) != seq![0u8, 0u8] {
        (Err(E::NonzeroReserved), end)
    } else {
        (Ok(()), end)
    }
}

fn read_header(bytes: &[u8], offset: &mut usize, domain: &[u8]) -> (result: Result<(), E>)
    ensures
        result == header_read_result(bytes@, *old(offset) as int, domain@).0,
        *final(offset) as int == header_read_result(bytes@, *old(offset) as int, domain@).1,
{
    let result =
        distributed_codec_read_header_body_v1!(verus_exec_expr, bytes, offset, domain);
    proof {
        let end = *offset as int;
        two_byte_sequence_view(bytes@.subrange(end - 2, end));
    }
    result
}

fn write_header(bytes: &mut [u8], offset: &mut usize, domain: &[u8])
    requires
        *old(offset) <= old(bytes)@.len(),
        domain@.len() + 4 <= old(bytes)@.len() - *old(offset) as int,
    ensures
        *final(offset) as int == *old(offset) as int + domain@.len() + 4,
        final(bytes)@.len() == old(bytes)@.len(),
        final(bytes)@.subrange(*old(offset) as int, *final(offset) as int)
            == domain@ + seq![1u8, 0u8, 0u8, 0u8],
        forall|i: int| 0 <= i < old(bytes)@.len()
            && !(*old(offset) as int <= i < *final(offset) as int)
            ==> #[trigger] final(bytes)@[i] == old(bytes)@[i],
{
    let ghost start = *offset as int;
    distributed_codec_write_header_body_v1!(verus_exec_expr, bytes, offset, domain);
    proof {
        let end = *offset as int;
        let schema_start = start + domain@.len();
        assert(DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1 == 1u16);
        assert((1u16 as u8) == 1u8 && ((1u16 >> 8) as u8) == 0u8) by (bit_vector);
        let encoded = little_u16(DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1);
        assert(encoded[0] == 1u8);
        assert(encoded[1] == 0u8);
        assert(encoded@.len() == 2);
        vstd::array::lemma_array_index(encoded, 0);
        vstd::array::lemma_array_index(encoded, 1);
        assert(encoded@[0] == 1u8);
        assert(encoded@[1] == 0u8);
        two_byte_sequence_view(encoded@);
        assert(encoded@ == seq![1u8, 0u8]);
        two_byte_sequence_view(little_u16(DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1)@);
        assert(little_u16(DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1)@ == seq![1u8, 0u8]);
        assert(bytes@.subrange(start, schema_start) =~= domain@) by {
            assert forall|i: int| 0 <= i < domain@.len()
                implies bytes@[start + i] == domain@[i] by {}
        }
        assert(end == schema_start + 4);
        two_byte_sequence_view(bytes@.subrange(schema_start, schema_start + 2));
        two_byte_sequence_view(bytes@.subrange(schema_start + 2, end));
        assert(bytes@.subrange(schema_start, schema_start + 2) == seq![1u8, 0u8]);
        assert(bytes@.subrange(schema_start + 2, end) == seq![0u8, 0u8]);
        let suffix = bytes@.subrange(schema_start, end);
        suffix.lemma_split_at(2);
        assert(suffix.subrange(0, 2) =~= bytes@.subrange(schema_start, schema_start + 2));
        assert(suffix.subrange(2, 4) =~= bytes@.subrange(schema_start + 2, end));
        assert(bytes@.subrange(schema_start, end) =~= seq![1u8, 0u8, 0u8, 0u8]);
        assert(bytes@.subrange(start, end) =~= domain@ + seq![1u8, 0u8, 0u8, 0u8]) by {
            assert forall|i: int| 0 <= i < end - start
                implies #[trigger] bytes@[start + i]
                    == (domain@ + seq![1u8, 0u8, 0u8, 0u8])[i] by {
                if i < domain@.len() {
                    assert(bytes@[start + i] == domain@[i]);
                } else {
                    assert(bytes@[start + i] == seq![1u8, 0u8, 0u8, 0u8][i - domain@.len()]);
                }
            }
        }
    }
}

spec fn u64_at(bytes: Seq<u8>, start: int) -> u64 {
    (bytes[start] as u64) | ((bytes[start + 1] as u64) << 8)
        | ((bytes[start + 2] as u64) << 16) | ((bytes[start + 3] as u64) << 24)
        | ((bytes[start + 4] as u64) << 32) | ((bytes[start + 5] as u64) << 40)
        | ((bytes[start + 6] as u64) << 48) | ((bytes[start + 7] as u64) << 56)
}

fn read_u64(bytes: &[u8], offset: &mut usize) -> (result: Result<u64, E>)
    ensures
        result.is_ok() == readable(bytes@, *old(offset), 8),
        match result {
            Ok(value) => value == u64_at(bytes@, *old(offset) as int)
                && *final(offset) as int == *old(offset) as int + 8,
            Err(error) => error == E::WrongLength && *final(offset) == *old(offset),
        },
{
    distributed_codec_read_u64_body_v1!(verus_exec_expr, bytes, offset)
}

fn write_u64(bytes: &mut [u8], offset: &mut usize, value: u64)
    requires
        *old(offset) <= old(bytes)@.len(),
        8 <= old(bytes)@.len() - *old(offset) as int,
    ensures
        *final(offset) as int == *old(offset) as int + 8,
        final(bytes)@.len() == old(bytes)@.len(),
        final(bytes)@.subrange(*old(offset) as int, *final(offset) as int) == little_u64(value)@,
        forall|i: int| 0 <= i < old(bytes)@.len()
            && !(*old(offset) as int <= i < *final(offset) as int)
            ==> #[trigger] final(bytes)@[i] == old(bytes)@[i],
{
    distributed_codec_write_u64_body_v1!(verus_exec_expr, bytes, offset, value)
}

}
