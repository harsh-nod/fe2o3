// Normal-return host binder composition. Actual shared bodies perform preflight,
// preparation, heap conversion, roster admission and reservation in source order.
// The vstd Vec/slice contracts, std Box conversion contracts and one explicit
// Hash/SHA transcript adapters are trusted;
// this proof must not be run or described as a no-cheating qualification.
// The included preflight schema unwraps numeric identity newtypes losslessly;
// all packet/code fields and arbitrary non-Copy custody payloads are retained.
// Allocation/OOM/unwind/Drop, mapping currentness, digest correctness/collisions,
// compiler/ISA semantics and native execution/publication remain outside scope.
// The theorem starts at the reached shared binder macro, after the production
// conditional-fill revalidation. It preserves that arbitrary owned payload but
// does not refine the preceding revalidation or its error/Drop behavior.
#![feature(allocator_api)]
#![allow(unused_macros)]
macro_rules! debug_assert_eq {
    ($left:expr, $right:expr) => { verus_exec_expr!({ assert($left == $right); }) };
}
include!("dispatch_template_preflight_v1.rs");
include!("completion_box_contracts_v1.rs");
include!("../../fe2o3-kfd/src/queue_dispatch_binding/template_prepare_body.rs");
include!("../../fe2o3-kfd/src/queue_dispatch_binding/template_bind_body.rs");

verus! {
#[derive(Clone, Copy)]
struct CompletionDispatchGenerationBindingV1 {
    queue: QueueKeyV1, code: MemoryMappingKeyV1, kernarg: MemoryMappingKeyV1,
    dispatch_generation: u64,
}
#[derive(Clone, Copy)]
struct CompletionPacketTemplateV1 {
    geometry: AqlDispatchGeometryV1, ordering: AqlDispatchOrderingV1,
    private_segment_size: u32, group_segment_size: u32,
    kernel_object: ObservedGpuAddressV1, kernarg_address: ObservedGpuAddressV1,
    kernarg_alignment: u64, generations: CompletionDispatchGenerationBindingV1,
}
}

include!("dispatch_template_hash_contract_v1.rs");
include!("dispatch_template_roster_v1.rs");

verus! {
fn prepared_kernarg_layout_matches_code(code_bound: bool, kernarg_layout_identity: [u8; 32],
    dispatch_abi_identity: [u8; 32]) -> (out: bool)
    ensures out == (!code_bound || kernarg_layout_identity == dispatch_abi_identity),
{
    dispatch_template_abi_matches_body!(@annotated verus_exec_expr, code_bound,
        kernarg_layout_identity, dispatch_abi_identity, difference, index,
        [invariant 0 <= index <= 32, code_bound,
            (difference == 0) == (forall|i: int| 0 <= i < index ==>
                kernarg_layout_identity[i] == dispatch_abi_identity[i]),
         decreases 32 - index,],
        [let ghost previous = difference;],
        [proof {
            byte_difference_zero(previous, kernarg_layout_identity[index as int],
                dispatch_abi_identity[index as int]);
            assert((difference == 0) == (forall|i: int| 0 <= i < index + 1 ==>
                kernarg_layout_identity[i] == dispatch_abi_identity[i]));
        }],
        [proof {
            if difference == 0 { assert(kernarg_layout_identity =~= dispatch_abi_identity); }
        }])
}

proof fn byte_difference_zero(previous: u8, left: u8, right: u8)
    ensures ((previous | (left ^ right)) == 0) == (previous == 0 && left == right),
{
    assert(((previous | (left ^ right)) == 0) == (previous == 0 && left == right)) by(bit_vector);
}

impl CompletionDispatchGenerationBindingV1 {
    fn new(queue: QueueKeyV1, code: MemoryMappingKeyV1, kernarg: MemoryMappingKeyV1,
        dispatch_generation: u64) -> (out: Self)
        ensures out == (Self { queue, code, kernarg, dispatch_generation }),
    {
        dispatch_template_generation_new_body!(verus_exec_expr, queue, code,
            kernarg, dispatch_generation)
    }
}

impl CompletionPacketTemplateV1 {
    fn new(geometry: AqlDispatchGeometryV1, ordering: AqlDispatchOrderingV1,
        private_segment_size: u32, group_segment_size: u32,
        kernel_object: ObservedGpuAddressV1, kernarg_address: ObservedGpuAddressV1,
        kernarg_alignment: u64, generations: CompletionDispatchGenerationBindingV1) -> (out: Self)
        ensures out == (Self { geometry, ordering, private_segment_size, group_segment_size,
            kernel_object, kernarg_address, kernarg_alignment, generations }),
    {
        dispatch_template_new_body!(verus_exec_expr, geometry, ordering,
            private_segment_size, group_segment_size, kernel_object, kernarg_address,
            kernarg_alignment, generations)
    }
}

spec fn template_entry_error(packets: Seq<PreparedDispatchPacketV1>, codes: Seq<ResolvedCodeIdentityV1>,
    index: int) -> Option<Gfx942DispatchBindingErrorV1>
{
    let packet = packets[index];
    if packet.code_index >= codes.len() {
        Some(Gfx942DispatchBindingErrorV1::InvalidCode("packet program index"))
    } else if packet.code_bound_kernarg_layout
        && packet.kernarg_layout_identity != codes[packet.code_index as int].dispatch_abi_identity {
        Some(Gfx942DispatchBindingErrorV1::InvalidKernarg {
            packet: index as usize, detail: "prepared kernarg dispatch ABI identity",
        })
    } else { None }
}

spec fn template_scan_error(packets: Seq<PreparedDispatchPacketV1>, codes: Seq<ResolvedCodeIdentityV1>,
    index: int) -> Option<Gfx942DispatchBindingErrorV1>
    decreases packets.len() - index,
{
    if index < 0 || index >= packets.len() { None }
    else {
        match template_entry_error(packets, codes, index) {
            Some(error) => Some(error),
            None => template_scan_error(packets, codes, index + 1),
        }
    }
}

spec fn template_value(packet: PreparedDispatchPacketV1, code: ResolvedCodeIdentityV1,
    queue: QueueKeyV1, generation: u64) -> CompletionPacketTemplateV1
{
    CompletionPacketTemplateV1 {
        geometry: packet.geometry, ordering: packet.ordering,
        private_segment_size: packet.private_segment_size,
        group_segment_size: packet.group_segment_size,
        kernel_object: code.descriptor_address, kernarg_address: packet.kernarg_address,
        kernarg_alignment: packet.kernarg_alignment,
        generations: CompletionDispatchGenerationBindingV1 {
            queue, code: code.mapping, kernarg: packet.kernarg_mapping,
            dispatch_generation: generation,
        },
    }
}

spec fn prepared_values_v1(packets: Seq<PreparedDispatchPacketV1>, codes: Seq<ResolvedCodeIdentityV1>,
    queue: QueueKeyV1, generation: u64) -> Seq<CompletionPacketTemplateV1>
{
    Seq::new(packets.len(), |i: int| template_value(packets[i], codes[packets[i].code_index as int], queue, generation))
}

fn prepare_dispatch_templates_v1(packets: &[PreparedDispatchPacketV1],
    code_identity: &[ResolvedCodeIdentityV1], queue: QueueKeyV1, generation: u64)
    -> (out: Result<Vec<CompletionPacketTemplateV1>, Gfx942DispatchBindingErrorV1>)
    ensures out.is_ok() == template_scan_error(packets@, code_identity@, 0).is_none(),
        match out {
            Err(error) => Some(error) == template_scan_error(packets@, code_identity@, 0),
            Ok(templates) => {
                &&& templates.len() == packets.len()
                &&& forall|i: int| 0 <= i < packets.len() ==> {
                    &&& packets@[i].code_index < code_identity.len()
                    &&& template_entry_error(packets@, code_identity@, i).is_none()
                    &&& templates@[i] == template_value(packets@[i],
                        code_identity@[packets@[i].code_index as int], queue, generation)
                }
                &&& templates@ == prepared_values_v1(packets@, code_identity@, queue, generation)
            },
        },
{
    let result = dispatch_prepare_templates_body!(@annotated verus_exec_expr, packets, code_identity,
        queue, generation, templates, index,
        [invariant 0 <= index <= packets.len(), templates.len() == index,
            template_scan_error(packets@, code_identity@, 0)
                == template_scan_error(packets@, code_identity@, index as int),
            forall|i: int| 0 <= i < index ==> {
                &&& packets@[i].code_index < code_identity.len()
                &&& template_entry_error(packets@, code_identity@, i).is_none()
                &&& templates@[i] == template_value(packets@[i],
                    code_identity@[packets@[i].code_index as int], queue, generation)
            },
         decreases packets.len() - index,],
        [proof { reveal(template_scan_error); reveal(template_entry_error); }],
        [proof {
            assert(template_entry_error(packets@, code_identity@, index as int).is_none());
            assert(templates@[index as int] == template_value(packets@[index as int],
                code_identity@[packets@[index as int].code_index as int], queue, generation));
        }]);
    proof {
        match &result {
            Ok(templates) => {
                assert(templates@ =~= prepared_values_v1(packets@, code_identity@, queue, generation));
            },
            Err(_) => {},
        }
    }
    result
}

spec fn binder_frame_v1<C, K, D, H, P, T, E, F>(before: &DispatchResourceOwnerV1<C, K, D, H, P, T, E, F>,
    after: &DispatchResourceOwnerV1<C, K, D, H, P, T, E, F>) -> bool
{
    &&& after.code == before.code
    &&& after.code_identity == before.code_identity
    &&& after.kernarg == before.kernarg
    &&& after.packets == before.packets
    &&& after.data == before.data
    &&& after.data_premises == before.data_premises
    &&& after.persistent_control == before.persistent_control
    &&& after.conditional_fill == before.conditional_fill
}

spec fn binder_error_v1<C, K, D, H, P, T, E, F>(owner: &DispatchResourceOwnerV1<C, K, D, H, P, T, E, F>,
    n: usize, queue: QueueKeyV1) -> Option<Gfx942DispatchBindingErrorV1>
{
    if template_preflight_error(owner, n, queue).is_some() { template_preflight_error(owner, n, queue) }
    else if template_scan_error(owner.packets@, owner.code_identity@, 0).is_some() {
        template_scan_error(owner.packets@, owner.code_identity@, 0)
    } else {
        let templates = prepared_values_v1(owner.packets@, owner.code_identity@, queue, owner.generation.next_generation);
        if templates.len() != n {
            Some(Gfx942DispatchBindingErrorV1::InvalidKernarg { packet: 0, detail: "prepared packet cardinality" })
        } else {
            match projected_roster_error_v1(templates) {
                Some(error) => Some(Gfx942DispatchBindingErrorV1::Completion(error)),
                None => reserve_error(owner.generation.state(), queue, projected_roster_value_v1(templates)),
            }
        }
    }
}

impl<C, K, D, H, P, T, E, F> DispatchResourceOwnerV1<C, K, D, H, P, T, E, F> {
    fn bind_templates<const N: usize>(&mut self, queue: QueueKeyV1)
        -> (out: Result<(Box<[CompletionPacketTemplateV1; N]>, DispatchEpochIdentityV1), Gfx942DispatchBindingErrorV1>)
        ensures out.is_ok() == binder_error_v1(old(self), N, queue).is_none(),
            binder_frame_v1(old(self), final(self)),
            match out {
                Err(error) => *final(self) == *old(self)
                    && Some(error) == binder_error_v1(old(self), N, queue),
                Ok((templates, identity)) => {
                    let before = old(self).generation.state();
                    let expected = prepared_values_v1(old(self).packets@, old(self).code_identity@, queue, before.next_generation);
                    &&& templates@ == expected
                    &&& templates@.len() == N
                    &&& identity.queue == queue
                    &&& identity.recipe_occurrence == before.recipe_occurrence
                    &&& first_reusable(before.slots, identity.slot_index as int)
                    &&& identity.slot_generation == before.slots[identity.slot_index as int].slot_generation + 1
                    &&& identity.dispatch_generation == before.next_generation
                    &&& final(self).generation.state() == reserved_state(before, queue,
                        projected_roster_value_v1(expected), identity.slot_index as int)
                    &&& cancellable(final(self).generation.state(), identity)
                },
            },
    {
        dispatch_bind_templates_body!(@annotated verus_exec_expr, self, N, queue,
            generation, templates, expected_roster, identity,
            [let ghost initial = *self;
             proof { reveal(binder_error_v1); }],
            [proof {
                assert(*self == initial);
                assert(templates@ == prepared_values_v1(self.packets@, self.code_identity@, queue, generation));
            }],
            [proof { assert(templates@.len() == N); }],
            [proof { assert(*self == initial); }],
            [proof { assert(identity.dispatch_generation == generation); }])
    }
}
}
