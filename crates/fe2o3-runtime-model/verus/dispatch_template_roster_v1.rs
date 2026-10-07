// Pure-template specialization of the actual shared generic slice traversal.
// The production closure is pinned to template.generations(); arbitrary FnMut
// callbacks are not covered by this specialization. Hash effects are recorded
// by the separately pinned adapter; no valid-roster premise is required.
include!("../../fe2o3-kfd/src/queue_completion/dispatch_roster_body.rs");

macro_rules! template_roster_project {
    ($project:ident, $value:expr) => { $value.generations() };
}
macro_rules! template_roster_hash_length {
    ($value:expr, $hasher:expr) => { roster_hash_length_v1($value, $hasher) };
}
macro_rules! template_roster_hash_binding {
    ($value:expr, $hasher:expr) => { roster_hash_binding_v1($value, $hasher) };
}

verus! {
impl CompletionPacketTemplateV1 {
    fn generations(self) -> (out: CompletionDispatchGenerationBindingV1)
        ensures out == self.generations,
    { self.generations }
}

spec fn template_generations_v1(templates: Seq<CompletionPacketTemplateV1>)
    -> Seq<CompletionDispatchGenerationBindingV1>
{
    Seq::new(templates.len(), |i: int| templates[i].generations)
}

spec fn roster_entry_valid_v1(templates: Seq<CompletionPacketTemplateV1>, i: int) -> bool {
    templates[i].generations.queue == templates[0].generations.queue
        && templates[i].generations.dispatch_generation == templates[0].generations.dispatch_generation
}

spec fn roster_prefix_end_v1(templates: Seq<CompletionPacketTemplateV1>, index: int) -> int
    decreases templates.len() - index,
{
    if index < 0 || index >= templates.len() { templates.len() as int }
    else if !roster_entry_valid_v1(templates, index) { index }
    else { roster_prefix_end_v1(templates, index + 1) }
}

spec fn roster_binding_tokens_v1(templates: Seq<CompletionPacketTemplateV1>, count: int)
    -> Seq<RosterHashTokenV1>
{
    Seq::new(count as nat, |i: int| RosterHashTokenV1::Binding(templates[i].generations))
}

spec fn roster_feed_v1(templates: Seq<CompletionPacketTemplateV1>) -> Seq<RosterHashTokenV1> {
    if templates.len() == 0 || templates[0].generations.dispatch_generation == 0 { Seq::empty() }
    else { seq![RosterHashTokenV1::Length(templates.len() as usize)]
        + roster_binding_tokens_v1(templates, roster_prefix_end_v1(templates, 1)) }
}

spec fn projected_roster_error_v1(templates: Seq<CompletionPacketTemplateV1>)
    -> Option<Gfx942CompletionErrorV1>
{
    if templates.len() == 0 { Some(Gfx942CompletionErrorV1::ZeroPacketCount) }
    else if templates[0].generations.dispatch_generation == 0
        || roster_prefix_end_v1(templates, 1) != templates.len() {
        Some(Gfx942CompletionErrorV1::StaleBatchGeneration)
    } else { None }
}

spec fn projected_roster_value_v1(templates: Seq<CompletionPacketTemplateV1>)
    -> CompletionDispatchRosterV1
{
    CompletionDispatchRosterV1 {
        queue: templates[0].generations.queue,
        packet_count: templates.len() as usize,
        dispatch_generation: templates[0].generations.dispatch_generation,
        roster_sha256: rust_hash_transcript_v1(roster_feed_v1(templates)),
    }
}

fn hash_completion_dispatch_roster_projected_v1(values: &[CompletionPacketTemplateV1], hasher: &mut RosterHasherV1)
    -> (out: Result<(QueueKeyV1, u64), Gfx942CompletionErrorV1>)
    ensures final(hasher).transcript == old(hasher).transcript + roster_feed_v1(values@),
        out == match projected_roster_error_v1(values@) {
            Some(error) => Err(error),
            None => Ok((values@[0].generations.queue, values@[0].generations.dispatch_generation)),
        },
{
    completion_hash_roster_body!(@annotated verus_exec_expr, values, template_projection, hasher,
        template_roster_project, template_roster_hash_length, template_roster_hash_binding,
        first, index, dispatch,
        [let ghost initial = hasher.transcript;],
        [proof {
            assert(roster_binding_tokens_v1(values@, 1) =~= seq![RosterHashTokenV1::Binding(first)]);
        }],
        [invariant 1 <= index <= values.len(), values.len() > 0,
            initial == old(hasher).transcript,
            first == values@[0].generations, first.dispatch_generation > 0,
            roster_prefix_end_v1(values@, 1) == roster_prefix_end_v1(values@, index as int),
            hasher.transcript == initial + seq![RosterHashTokenV1::Length(values.len())]
                + roster_binding_tokens_v1(values@, index as int),
         decreases values.len() - index,],
        [proof {
            reveal(roster_prefix_end_v1);
            assert(hasher.transcript =~= initial + (seq![RosterHashTokenV1::Length(values.len())]
                + roster_binding_tokens_v1(values@, index as int)));
            if !roster_entry_valid_v1(values@, index as int) {
                assert(roster_prefix_end_v1(values@, index as int) == index);
                assert(roster_feed_v1(values@) == seq![RosterHashTokenV1::Length(values.len())]
                    + roster_binding_tokens_v1(values@, index as int));
                assert(hasher.transcript == initial + roster_feed_v1(values@));
            }
        }],
        [proof {
            assert(roster_binding_tokens_v1(values@, index as int).push(RosterHashTokenV1::Binding(dispatch))
                =~= roster_binding_tokens_v1(values@, index as int + 1));
        }],
        [proof {
            reveal(roster_prefix_end_v1);
            assert(hasher.transcript =~= initial + (seq![RosterHashTokenV1::Length(values.len())]
                + roster_binding_tokens_v1(values@, index as int)));
        }])
}

fn completion_template_dispatch_roster_v1(templates: &[CompletionPacketTemplateV1])
    -> (out: Result<CompletionDispatchRosterV1, Gfx942CompletionErrorV1>)
    ensures out == match projected_roster_error_v1(templates@) {
        Some(error) => Err(error), None => Ok(projected_roster_value_v1(templates@)),
    },
{
    let mut hasher = roster_hasher_new_v1();
    let (queue, dispatch_generation) = hash_completion_dispatch_roster_projected_v1(templates, &mut hasher)?;
    let roster_sha256 = roster_hasher_finalize_v1(hasher);
    Ok(CompletionDispatchRosterV1 { queue, packet_count: templates.len(), dispatch_generation, roster_sha256 })
}
}
