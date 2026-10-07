// Host composition only. Heap conversion and roster hashing have explicit
// adapter contracts in the proof; native authority and publication are excluded.
macro_rules! dispatch_bind_templates_body {
    ($syntax:ident, $owner:ident, $n:ident, $queue:ident) => {
        dispatch_bind_templates_body!(@annotated $syntax, $owner, $n, $queue,
            generation, templates, expected_roster, identity, [], [], [], [], [])
    };
    (@annotated $syntax:ident, $owner:ident, $n:ident, $queue:ident,
     $generation:ident, $templates:ident, $roster:ident, $identity:ident,
     [$($initial:tt)*], [$($prepared:tt)*], [$($boxed:tt)*],
     [$($hashed:tt)*], [$($reserved:tt)*]) => {
        $syntax!({
            $($initial)*
            let $generation = $owner.preflight_templates::<$n>($queue)?;
            let $templates = prepare_dispatch_templates_v1(
                &$owner.packets,
                &$owner.code_identity,
                $queue,
                $generation,
            )?;
            $($prepared)*
            // Keep large batches heap-backed through the return ABI; do not
            // materialize the fixed array on the thread's stack.
            let $templates: Box<[CompletionPacketTemplateV1; $n]> = $templates
                .into_boxed_slice()
                .try_into()
                .map_err(|_rejected| Gfx942DispatchBindingErrorV1::InvalidKernarg {
                    packet: 0,
                    detail: "prepared packet cardinality",
                })?;
            $($boxed)*
            let $roster = match completion_template_dispatch_roster_v1($templates.as_slice()) {
                Ok(roster) => roster,
                Err(error) => return Err(Gfx942DispatchBindingErrorV1::Completion(error)),
            };
            $($hashed)*
            let $identity = $owner.generation.reserve($queue, $roster)?;
            $($reserved)*
            debug_assert_eq!($identity.dispatch_generation, $generation);
            Ok(($templates, $identity))
        })
    };
}
