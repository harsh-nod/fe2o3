// Only the Hash/SHA adapter is trusted. The transcript denotes the existing
// usize Hash call followed by derived binding Hash calls, not a canonical wire
// encoding. No injectivity, collision resistance, SHA implementation, compiler,
// or ISA theorem is claimed. Validation and traversal are outside this adapter.
verus! {
#[derive(Clone, Copy)]
enum RosterHashTokenV1 {
    Length(usize), Binding(CompletionDispatchGenerationBindingV1),
}
struct RosterHasherV1 { ghost transcript: Seq<RosterHashTokenV1> }

uninterp spec fn rust_hash_transcript_v1(transcript: Seq<RosterHashTokenV1>) -> [u8; 32];

#[verifier::external_body]
fn roster_hasher_new_v1() -> (out: RosterHasherV1)
    ensures out.transcript == Seq::<RosterHashTokenV1>::empty(),
{ unimplemented!() }

#[verifier::external_body]
fn roster_hash_length_v1(length: usize, hasher: &mut RosterHasherV1)
    ensures final(hasher).transcript == old(hasher).transcript.push(RosterHashTokenV1::Length(length)),
{ unimplemented!() }

#[verifier::external_body]
fn roster_hash_binding_v1(binding: CompletionDispatchGenerationBindingV1, hasher: &mut RosterHasherV1)
    ensures final(hasher).transcript == old(hasher).transcript.push(RosterHashTokenV1::Binding(binding)),
{ unimplemented!() }

#[verifier::external_body]
fn roster_hasher_finalize_v1(hasher: RosterHasherV1) -> (out: [u8; 32])
    ensures out == rust_hash_transcript_v1(hasher.transcript),
{ unimplemented!() }
}
