// Whole operation-wire refinement, composing the actual shared field bodies.
// This root adds no trusted identity or digest adapter. The exact safe native
// projection/construction bodies are source-bound and proved below.
include!("distributed_codec_fields_v1.rs");
include!("../src/distributed_publication_contract/codec_operation_body.rs");

macro_rules! operation_identity_constructor_bridge_v1 {
    ($($name:ident),+ $(,)?) => { $(verus! {
        impl $name {
            fn from_untrusted_digest(digest: IdentityDigestV1) -> (result: Self)
                ensures result.0 == digest,
            { Self(digest) }
        }
    })+ };
}

operation_identity_constructor_bridge_v1!(
    DistributedRuntimeInstanceIdV1, DistributedParticipantIdV1,
    DistributedMembershipIdV1, DistributedRunIdV1, DistributedOperationIdV1,
    DistributedExecutionPlanIdV1, DistributedPlacementPlanIdV1,
    DistributedTargetDescriptionIdV1, RuntimeArtifactIdV1, RuntimeModelIdV1,
);

// The native hook is debug_assert_eq!. Its predicate is proved unconditionally,
// including release builds where the native debug assertion is compiled out.
macro_rules! operation_checked_offset_v1 {
    ($left:expr, $right:expr) => {
        verus_exec_expr!({ assert($left == $right); })
    };
}

verus! {

const DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1: &'static [u8; 43] =
    b"FE2O3/DISTRIBUTED-OPERATION-DESCRIPTION/V1\0";
const COORDINATE_BYTES: usize = 11 * 32 + 4 * 8;
const DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1: usize =
    43 + 4 + COORDINATE_BYTES;

impl IdentityDigestV1 {
    fn as_bytes(&self) -> (result: &[u8; 32])
        ensures result@ == self.0@,
    { &self.0 }
}

spec fn operation_payload(c: UntrustedDistributedOperationCoordinatesV1) -> Seq<u8> {
    c.runtime_instance.0.0@ + c.participant.0.0@ + little_u64(c.participant_incarnation)@
        + c.coordinator.0.0@ + little_u64(c.coordinator_epoch)@
        + c.membership.0.0@ + little_u64(c.membership_epoch)@
        + c.run.0.0@ + c.operation.0.0@ + little_u64(c.attempt)@
        + c.artifact.0.0@ + c.execution_plan.0.0@ + c.placement_plan.0.0@
        + c.target.0.0@ + c.runtime_model.0.0@
}

spec fn operation_wire(c: UntrustedDistributedOperationCoordinatesV1) -> Seq<u8> {
    DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1@ + seq![1u8, 0u8, 0u8, 0u8]
        + operation_payload(c)
}

spec fn digest_at(bytes: Seq<u8>, start: int) -> IdentityDigestV1
    recommends 0 <= start && start + 32 <= bytes.len(),
{
    IdentityDigestV1([
        bytes[start], bytes[start + 1], bytes[start + 2], bytes[start + 3],
        bytes[start + 4], bytes[start + 5], bytes[start + 6], bytes[start + 7],
        bytes[start + 8], bytes[start + 9], bytes[start + 10], bytes[start + 11],
        bytes[start + 12], bytes[start + 13], bytes[start + 14], bytes[start + 15],
        bytes[start + 16], bytes[start + 17], bytes[start + 18], bytes[start + 19],
        bytes[start + 20], bytes[start + 21], bytes[start + 22], bytes[start + 23],
        bytes[start + 24], bytes[start + 25], bytes[start + 26], bytes[start + 27],
        bytes[start + 28], bytes[start + 29], bytes[start + 30], bytes[start + 31],
    ])
}

proof fn digest_at_view(bytes: Seq<u8>, start: int)
    requires 0 <= start, start + 32 <= bytes.len(),
    ensures digest_at(bytes, start).0@ == bytes.subrange(start, start + 32),
{
    assert(digest_at(bytes, start).0@ =~= bytes.subrange(start, start + 32)) by {
        assert forall|i: int| 0 <= i < 32 implies
            #[trigger] digest_at(bytes, start).0@[i] == bytes[start + i] by {}
    }
}

spec fn coordinates_at(bytes: Seq<u8>) -> UntrustedDistributedOperationCoordinatesV1
    recommends bytes.len() == 431,
{
    UntrustedDistributedOperationCoordinatesV1 {
        runtime_instance: DistributedRuntimeInstanceIdV1(digest_at(bytes, 47)),
        participant: DistributedParticipantIdV1(digest_at(bytes, 79)),
        participant_incarnation: u64_at(bytes, 111),
        coordinator: DistributedParticipantIdV1(digest_at(bytes, 119)),
        coordinator_epoch: u64_at(bytes, 151),
        membership: DistributedMembershipIdV1(digest_at(bytes, 159)),
        membership_epoch: u64_at(bytes, 191),
        run: DistributedRunIdV1(digest_at(bytes, 199)),
        operation: DistributedOperationIdV1(digest_at(bytes, 231)),
        attempt: u64_at(bytes, 263),
        artifact: RuntimeArtifactIdV1(digest_at(bytes, 271)),
        execution_plan: DistributedExecutionPlanIdV1(digest_at(bytes, 303)),
        placement_plan: DistributedPlacementPlanIdV1(digest_at(bytes, 335)),
        target: DistributedTargetDescriptionIdV1(digest_at(bytes, 367)),
        runtime_model: RuntimeModelIdV1(digest_at(bytes, 399)),
    }
}

spec fn operation_decision(bytes: Seq<u8>) -> Result<ModelDistributedOperationBindingV1, E> {
    if bytes.len() != DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1 {
        Err(E::WrongLength)
    } else {
        match header_read_result(bytes, 0, DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1@).0 {
            Err(error) => Err(error),
            Ok(()) => {
                let c = coordinates_at(bytes);
                if zero_identity(c) { Err(E::ZeroIdentity) }
                else if zero_integer(c) { Err(E::ZeroEpochOrAttempt) }
                else { Ok(ModelDistributedOperationBindingV1 { coordinates: c }) }
            },
        }
    }
}

fn write_digest(bytes: &mut [u8], offset: &mut usize, value: IdentityDigestV1)
    requires
        *old(offset) <= old(bytes)@.len(),
        32 <= old(bytes)@.len() - *old(offset) as int,
    ensures
        *final(offset) as int == *old(offset) as int + 32,
        final(bytes)@.len() == old(bytes)@.len(),
        final(bytes)@.subrange(*old(offset) as int, *final(offset) as int) == value.0@,
        forall|i: int| 0 <= i < old(bytes)@.len()
            && !(*old(offset) as int <= i < *final(offset) as int)
            ==> #[trigger] final(bytes)@[i] == old(bytes)@[i],
{
    distributed_codec_write_digest_body_v1!(verus_exec_expr, bytes, offset, value)
}

fn read_digest(bytes: &[u8], offset: &mut usize) -> (result: Result<IdentityDigestV1, E>)
    ensures
        result.is_ok() == readable(bytes@, *old(offset), 32),
        match result {
            Ok(value) => value == digest_at(bytes@, *old(offset) as int)
                && *final(offset) as int == *old(offset) as int + 32,
            Err(error) => error == E::WrongLength && *final(offset) == *old(offset),
        },
{
    let result = distributed_codec_read_digest_body_v1!(verus_exec_expr, bytes, offset);
    proof {
        if let Ok(value) = result {
            let start = *offset as int - 32;
            digest_at_view(bytes@, start);
            assert(value.0@ == digest_at(bytes@, start).0@);
            assert(value.0 == digest_at(bytes@, start).0);
        }
    }
    result
}

fn encode(binding: ModelDistributedOperationBindingV1)
    -> (result: [u8; DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1])
    ensures result@ == operation_wire(binding.coordinates),
{
    distributed_codec_encode_operation_body_v1!(
        verus_exec_expr, operation_checked_offset_v1, binding)
}

fn decode(bytes: &[u8]) -> (result: Result<ModelDistributedOperationBindingV1, E>)
    ensures result == operation_decision(bytes@),
{
    hide(digest_at);
    hide(u64_at);
    hide(zero_identity);
    hide(zero_integer);
    distributed_codec_decode_operation_body_v1!(verus_exec_expr, bytes)
}

proof fn little_u64_value_inverse(value: u64)
    ensures u64_at(little_u64(value)@, 0) == value,
{
    let bytes = little_u64(value);
    assert forall|i: int| 0 <= i < 8 implies #[trigger] bytes@[i] == bytes[i] by {
        vstd::array::lemma_array_index(bytes, i);
    }
    assert(((value as u8) as u64)
        | ((((value >> 8) as u8) as u64) << 8)
        | ((((value >> 16) as u8) as u64) << 16)
        | ((((value >> 24) as u8) as u64) << 24)
        | ((((value >> 32) as u8) as u64) << 32)
        | ((((value >> 40) as u8) as u64) << 40)
        | ((((value >> 48) as u8) as u64) << 48)
        | ((((value >> 56) as u8) as u64) << 56) == value) by (bit_vector);
}

proof fn little_u64_bytes_inverse(bytes: Seq<u8>, start: int)
    requires 0 <= start, start + 8 <= bytes.len(),
    ensures little_u64(u64_at(bytes, start))@ == bytes.subrange(start, start + 8),
{
    let a = bytes[start];
    let b = bytes[start + 1];
    let c = bytes[start + 2];
    let d = bytes[start + 3];
    let e = bytes[start + 4];
    let f = bytes[start + 5];
    let g = bytes[start + 6];
    let h = bytes[start + 7];
    let value = u64_at(bytes, start);
    assert(value == (a as u64) | ((b as u64) << 8) | ((c as u64) << 16)
        | ((d as u64) << 24) | ((e as u64) << 32) | ((f as u64) << 40)
        | ((g as u64) << 48) | ((h as u64) << 56));
    assert((value == (a as u64) | ((b as u64) << 8) | ((c as u64) << 16)
        | ((d as u64) << 24) | ((e as u64) << 32) | ((f as u64) << 40)
        | ((g as u64) << 48) | ((h as u64) << 56))
        ==> ((value as u8) == a && ((value >> 8) as u8) == b
            && ((value >> 16) as u8) == c && ((value >> 24) as u8) == d
            && ((value >> 32) as u8) == e && ((value >> 40) as u8) == f
            && ((value >> 48) as u8) == g && ((value >> 56) as u8) == h)) by (bit_vector);
    let encoded = little_u64(value);
    assert(encoded@ =~= bytes.subrange(start, start + 8)) by {
        assert forall|i: int| 0 <= i < 8 implies
            #[trigger] encoded@[i] == bytes[start + i] by {
            vstd::array::lemma_array_index(encoded, i);
            assert(i == 0 || i == 1 || i == 2 || i == 3
                || i == 4 || i == 5 || i == 6 || i == 7);
        }
    }
}

proof fn digest_subrange_inverse(bytes: Seq<u8>, start: int, value: IdentityDigestV1)
    requires
        0 <= start, start + 32 <= bytes.len(),
        bytes.subrange(start, start + 32) == value.0@,
    ensures digest_at(bytes, start) == value,
{
    hide(digest_at);
    digest_at_view(bytes, start);
    assert(digest_at(bytes, start).0 == value.0);
}

proof fn u64_subrange_inverse(bytes: Seq<u8>, start: int, value: u64)
    requires
        0 <= start, start + 8 <= bytes.len(),
        bytes.subrange(start, start + 8) == little_u64(value)@,
    ensures u64_at(bytes, start) == value,
{
    hide(little_u64);
    little_u64_value_inverse(value);
    assert forall|i: int| 0 <= i < 8 implies
        #[trigger] bytes[start + i] == little_u64(value)@[i] by {}
    assert(u64_at(bytes, start) == u64_at(little_u64(value)@, 0));
}

proof fn operation_wire_layout(c: UntrustedDistributedOperationCoordinatesV1)
    ensures
        operation_wire(c).len() == DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1,
        operation_wire(c).subrange(0, 47)
            == DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1@ + seq![1u8, 0u8, 0u8, 0u8],
        operation_wire(c).subrange(47, 79) == c.runtime_instance.0.0@,
        operation_wire(c).subrange(79, 111) == c.participant.0.0@,
        operation_wire(c).subrange(111, 119) == little_u64(c.participant_incarnation)@,
        operation_wire(c).subrange(119, 151) == c.coordinator.0.0@,
        operation_wire(c).subrange(151, 159) == little_u64(c.coordinator_epoch)@,
        operation_wire(c).subrange(159, 191) == c.membership.0.0@,
        operation_wire(c).subrange(191, 199) == little_u64(c.membership_epoch)@,
        operation_wire(c).subrange(199, 231) == c.run.0.0@,
        operation_wire(c).subrange(231, 263) == c.operation.0.0@,
        operation_wire(c).subrange(263, 271) == little_u64(c.attempt)@,
        operation_wire(c).subrange(271, 303) == c.artifact.0.0@,
        operation_wire(c).subrange(303, 335) == c.execution_plan.0.0@,
        operation_wire(c).subrange(335, 367) == c.placement_plan.0.0@,
        operation_wire(c).subrange(367, 399) == c.target.0.0@,
        operation_wire(c).subrange(399, 431) == c.runtime_model.0.0@,
{
    hide(little_u64);
    let bytes = operation_wire(c);
    assert(bytes.subrange(0, 47) =~=
        DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1@ + seq![1u8, 0u8, 0u8, 0u8]);
    assert(bytes.subrange(47, 79) =~= c.runtime_instance.0.0@);
    assert(bytes.subrange(79, 111) =~= c.participant.0.0@);
    assert(bytes.subrange(111, 119) =~= little_u64(c.participant_incarnation)@);
    assert(bytes.subrange(119, 151) =~= c.coordinator.0.0@);
    assert(bytes.subrange(151, 159) =~= little_u64(c.coordinator_epoch)@);
    assert(bytes.subrange(159, 191) =~= c.membership.0.0@);
    assert(bytes.subrange(191, 199) =~= little_u64(c.membership_epoch)@);
    assert(bytes.subrange(199, 231) =~= c.run.0.0@);
    assert(bytes.subrange(231, 263) =~= c.operation.0.0@);
    assert(bytes.subrange(263, 271) =~= little_u64(c.attempt)@);
    assert(bytes.subrange(271, 303) =~= c.artifact.0.0@);
    assert(bytes.subrange(303, 335) =~= c.execution_plan.0.0@);
    assert(bytes.subrange(335, 367) =~= c.placement_plan.0.0@);
    assert(bytes.subrange(367, 399) =~= c.target.0.0@);
    assert(bytes.subrange(399, 431) =~= c.runtime_model.0.0@);
}

proof fn adjacent_subranges(bytes: Seq<u8>, start: int, middle: int, end: int)
    requires 0 <= start <= middle <= end <= bytes.len(),
    ensures bytes.subrange(start, middle) + bytes.subrange(middle, end)
        == bytes.subrange(start, end),
{
    assert(bytes.subrange(start, middle) + bytes.subrange(middle, end)
        =~= bytes.subrange(start, end)) by {
        assert forall|i: int| 0 <= i < end - start implies
            (bytes.subrange(start, middle) + bytes.subrange(middle, end))[i]
                == bytes[start + i] by {
            if i < middle - start {
                assert(bytes.subrange(start, middle)[i] == bytes[start + i]);
            } else {
                assert(bytes.subrange(middle, end)[i - (middle - start)] == bytes[start + i]);
            }
        }
    }
}

proof fn operation_success_header(bytes: Seq<u8>)
    requires
        bytes.len() == DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1,
        header_read_result(bytes, 0, DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1@).0 == Ok(()),
    ensures bytes.subrange(0, 47)
        == DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1@ + seq![1u8, 0u8, 0u8, 0u8],
{
    let low = bytes[43];
    let high = bytes[44];
    assert(((low as u16) | ((high as u16) << 8)) == 1u16);
    assert((((low as u16) | ((high as u16) << 8)) == 1u16)
        ==> (low == 1u8 && high == 0u8)) by (bit_vector);
    assert(bytes.subrange(0, 47) =~=
        DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1@ + seq![1u8, 0u8, 0u8, 0u8]) by {
        assert forall|i: int| 0 <= i < 47 implies bytes[i] ==
            (DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1@ + seq![1u8, 0u8, 0u8, 0u8])[i] by {
            if i < 43 {
                assert(bytes[i] == DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1@[i]);
            }
        }
    }
}

proof fn operation_coordinates_payload_inverse(bytes: Seq<u8>)
    requires bytes.len() == DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1,
    ensures operation_payload(coordinates_at(bytes)) == bytes.subrange(47, 431),
{
    hide(digest_at);
    hide(u64_at);
    hide(little_u64);
    digest_at_view(bytes, 47);
    digest_at_view(bytes, 79);
    digest_at_view(bytes, 119);
    digest_at_view(bytes, 159);
    digest_at_view(bytes, 199);
    digest_at_view(bytes, 231);
    digest_at_view(bytes, 271);
    digest_at_view(bytes, 303);
    digest_at_view(bytes, 335);
    digest_at_view(bytes, 367);
    digest_at_view(bytes, 399);
    little_u64_bytes_inverse(bytes, 111);
    little_u64_bytes_inverse(bytes, 151);
    little_u64_bytes_inverse(bytes, 191);
    little_u64_bytes_inverse(bytes, 263);
    adjacent_subranges(bytes, 47, 79, 111);
    adjacent_subranges(bytes, 47, 111, 119);
    adjacent_subranges(bytes, 47, 119, 151);
    adjacent_subranges(bytes, 47, 151, 159);
    adjacent_subranges(bytes, 47, 159, 191);
    adjacent_subranges(bytes, 47, 191, 199);
    adjacent_subranges(bytes, 47, 199, 231);
    adjacent_subranges(bytes, 47, 231, 263);
    adjacent_subranges(bytes, 47, 263, 271);
    adjacent_subranges(bytes, 47, 271, 303);
    adjacent_subranges(bytes, 47, 303, 335);
    adjacent_subranges(bytes, 47, 335, 367);
    adjacent_subranges(bytes, 47, 367, 399);
    adjacent_subranges(bytes, 47, 399, 431);
}

proof fn operation_header_layout_success(bytes: Seq<u8>)
    requires
        bytes.len() == DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1,
        bytes.subrange(0, 47)
            == DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1@ + seq![1u8, 0u8, 0u8, 0u8],
    ensures header_read_result(bytes, 0, DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1@)
        == (Ok(()), 47),
{
    hide(header_read_result);
    let domain = DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1@;
    assert(domain.len() == 43);
    let schema_start: int = domain.len() as int;
    let reserved_start = schema_start + 2;
    let end = reserved_start + 2;
    assert(schema_start == 43 && reserved_start == 45 && end == 47);
    assert(schema_start <= bytes.len());
    assert(bytes.subrange(0, schema_start) =~= domain);
    assert(reserved_start <= bytes.len());
    let header = bytes.subrange(0, 47);
    assert(header[43] == 1u8 && header[44] == 0u8
        && header[45] == 0u8 && header[46] == 0u8);
    assert(bytes[43] == 1u8 && bytes[44] == 0u8
        && bytes[45] == 0u8 && bytes[46] == 0u8);
    assert(DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1 == 1u16);
    assert(((1u8 as u16) | ((0u8 as u16) << 8)) == 1u16) by (bit_vector);
    assert(((bytes[schema_start] as u16) | ((bytes[schema_start + 1] as u16) << 8))
        == DISTRIBUTED_PUBLICATION_CONTRACT_SCHEMA_V1);
    assert(end <= bytes.len());
    let reserved = bytes.subrange(reserved_start, end);
    two_byte_sequence_view(reserved);
    assert(reserved[0] == 0u8 && reserved[1] == 0u8);
    assert(reserved == seq![0u8, 0u8]);
    reveal(header_read_result);
    assert(header_read_result(bytes, 0, domain) == (Ok(()), end));
}

proof fn operation_coordinates_inverse(c: UntrustedDistributedOperationCoordinatesV1)
    ensures
        operation_wire(c).len() == DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1,
        header_read_result(operation_wire(c), 0, DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1@)
            == (Ok(()), 47),
        coordinates_at(operation_wire(c)) == c,
{
    hide(operation_wire);
    hide(operation_payload);
    hide(digest_at);
    hide(u64_at);
    hide(little_u64);
    operation_wire_layout(c);
    let bytes = operation_wire(c);
    digest_subrange_inverse(bytes, 47, c.runtime_instance.0);
    digest_subrange_inverse(bytes, 79, c.participant.0);
    digest_subrange_inverse(bytes, 119, c.coordinator.0);
    digest_subrange_inverse(bytes, 159, c.membership.0);
    digest_subrange_inverse(bytes, 199, c.run.0);
    digest_subrange_inverse(bytes, 231, c.operation.0);
    digest_subrange_inverse(bytes, 271, c.artifact.0);
    digest_subrange_inverse(bytes, 303, c.execution_plan.0);
    digest_subrange_inverse(bytes, 335, c.placement_plan.0);
    digest_subrange_inverse(bytes, 367, c.target.0);
    digest_subrange_inverse(bytes, 399, c.runtime_model.0);
    u64_subrange_inverse(bytes, 111, c.participant_incarnation);
    u64_subrange_inverse(bytes, 151, c.coordinator_epoch);
    u64_subrange_inverse(bytes, 191, c.membership_epoch);
    u64_subrange_inverse(bytes, 263, c.attempt);
    operation_header_layout_success(bytes);
    assert(coordinates_at(bytes) == c);
}

proof fn operation_success_wire_inverse(bytes: Seq<u8>)
    requires
        bytes.len() == DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1,
        header_read_result(bytes, 0, DISTRIBUTED_OPERATION_DESCRIPTION_DOMAIN_V1@).0 == Ok(()),
    ensures operation_wire(coordinates_at(bytes)) == bytes,
{
    hide(operation_payload);
    hide(coordinates_at);
    operation_success_header(bytes);
    operation_coordinates_payload_inverse(bytes);
    adjacent_subranges(bytes, 0, 47, 431);
    assert(bytes.subrange(0, 431) =~= bytes);
}

fn operation_roundtrip(coordinates: UntrustedDistributedOperationCoordinatesV1)
    -> (result: Result<ModelDistributedOperationBindingV1, E>)
    ensures result == if zero_identity(coordinates) { Err(E::ZeroIdentity) }
        else if zero_integer(coordinates) { Err(E::ZeroEpochOrAttempt) }
        else { Ok(ModelDistributedOperationBindingV1 { coordinates }) },
{
    let binding = ModelDistributedOperationBindingV1 { coordinates };
    let bytes = encode(binding);
    proof {
        operation_coordinates_inverse(coordinates);
    }
    decode(&bytes)
}

fn successful_decode_canonicality(bytes: &[u8])
    -> (result: Result<[u8; DISTRIBUTED_OPERATION_DESCRIPTION_BYTES_V1], E>)
    ensures match result {
        Ok(wire) => wire@ == bytes@ && operation_decision(bytes@).is_ok(),
        Err(error) => operation_decision(bytes@) == Err(error),
    },
{
    let binding = decode(bytes)?;
    proof {
        operation_success_wire_inverse(bytes@);
    }
    Ok(encode(binding))
}

}
