// Shared scans and metadata writes over the retained slot table. Native outcome
// authentication and logical settlement remain separate caller obligations.
macro_rules! compute_pipeline_vacant_generation_body {
    ($syntax:ident, $slot:ident) => {
        $syntax!({
            if $slot.entry.is_some() { return None; }
            match $slot.generation.checked_add(1) {
                Some(generation) if generation != 0 => Some(generation),
                _ => None,
            }
        })
    };
}

macro_rules! compute_pipeline_first_vacant_body {
    ($syntax:ident, $slots:ident) => {
        compute_pipeline_first_vacant_body!(@annotated $syntax, $slots, i, [], [])
    };
    (@annotated $syntax:ident, $slots:ident, $i:ident,
     [$($invariants:tt)*], [$($step:tt)*]) => {
        $syntax!({
            let mut $i = 0usize;
            while $i < $slots.len() $($invariants)* {
                if let Some(generation) = vacant_generation(&$slots[$i]) {
                    return Some(($i, generation));
                }
                $i += 1;
                $($step)*
            }
            None
        })
    };
}

macro_rules! compute_pipeline_capacity_body {
    ($syntax:ident, $slots:ident, $live:ident, $next:ident, $staged:ident) => {
        $syntax!({
            $staged.is_none() && $live < $slots.len() - 1
                && $next.is_some() && first_vacant($slots).is_some()
        })
    };
}

macro_rules! compute_pipeline_entry_body {
    ($syntax:ident, $slots:ident, $identity:ident) => {
        $syntax!({
            let index = $identity.slot as usize;
            if index >= $slots.len() { return None; }
            let slot = &$slots[index];
            let entry = slot.entry.as_ref()?;
            if slot.generation == $identity.slot_generation
                && entry.identity == $identity && entry.active.id == $identity.submission
            { Some(entry) } else { None }
        })
    };
}

macro_rules! compute_pipeline_entry_mut_body {
    ($syntax:ident, $slots:ident, $identity:ident) => {
        $syntax!({
            exact_entry($slots, $identity)?;
            $slots[$identity.slot as usize].entry.as_mut()
        })
    };
}

macro_rules! compute_pipeline_frontier_body {
    ($syntax:ident, $slots:ident, $live:ident, $frontier:ident, $staged:ident) => {
        compute_pipeline_frontier_body!(@annotated $syntax, $slots, $live, $frontier, $staged,
            i, occupied, found, next_count, successor, entry, [], [], [])
    };
    (@annotated $syntax:ident, $slots:ident, $live:ident, $frontier:ident, $staged:ident,
     $i:ident, $occupied:ident, $found:ident, $next_count:ident, $successor:ident, $entry:ident,
     [$($invariants:tt)*], [$($step:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            if $staged.is_some() { return Err(()); }
            let $successor = match $frontier {
                Some(epoch) => epoch.checked_add(1),
                None => None,
            };
            let mut $occupied = 0usize;
            let mut $found = None;
            let mut $next_count = 0usize;
            let mut $i = 0usize;
            while $i < $slots.len() $($invariants)* {
                let slot = &$slots[$i];
                if let Some($entry) = slot.entry.as_ref() {
                    $occupied += 1;
                    if $entry.identity.slot as usize != $i
                        || $entry.identity.slot_generation != slot.generation
                        || $entry.identity.submission != $entry.active.id
                        || $entry.identity.submission == 0
                    { return Err(()); }
                    if Some($entry.identity.logical_epoch) == $frontier {
                        if $found.is_some() { return Err(()); }
                        $found = Some($i);
                    }
                    if Some($entry.identity.logical_epoch) == $successor { $next_count += 1; }
                }
                $i += 1;
                $($step)*
            }
            $($finish)*
            if $occupied != $live { return Err(()); }
            if $live == 0 {
                return if $frontier.is_none() { Ok(None) } else { Err(()) };
            }
            if $live > 1 && $next_count != 1 { return Err(()); }
            match $found { Some(index) => Ok(Some(index)), None => Err(()) }
        })
    };
}

macro_rules! compute_pipeline_staged_intact_body {
    ($syntax:ident, $slots:ident, $live:ident, $next:ident, $frontier:ident,
     $staged:ident, $identity:ident) => {
        compute_pipeline_staged_intact_body!(@annotated $syntax, $slots, $live, $next,
            $frontier, $staged, $identity, i, occupied, frontier_count, entry, [], [], [])
    };
    (@annotated $syntax:ident, $slots:ident, $live:ident, $next:ident, $frontier:ident,
     $staged:ident, $identity:ident, $i:ident, $occupied:ident, $frontier_count:ident, $entry:ident,
     [$($invariants:tt)*], [$($step:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            if $staged != Some($identity) || $identity.slot_generation == 0
                || $identity.logical_epoch == 0 || $identity.submission == 0
                || $next != Some($identity.logical_epoch) || $live == 0
            { return false; }
            match exact_entry($slots, $identity) {
                Some(entry) if entry.phase == RuntimeComputePipelinePhaseV1::Publishing => {}
                _ => return false,
            }
            let mut $i = 0usize;
            let mut $occupied = 0usize;
            let mut $frontier_count = 0usize;
            while $i < $slots.len() $($invariants)* {
                if let Some($entry) = $slots[$i].entry.as_ref() {
                    $occupied += 1;
                    if $entry.identity != $identity
                        && $entry.identity.logical_epoch >= $identity.logical_epoch
                    { return false; }
                    if Some($entry.identity.logical_epoch) == $frontier { $frontier_count += 1; }
                }
                $i += 1;
                $($step)*
            }
            $($finish)*
            if $occupied != $live { return false; }
            if $live == 1 { return $frontier.is_none(); }
            match $frontier {
                Some(epoch) => epoch < $identity.logical_epoch && $frontier_count == 1,
                None => false,
            }
        })
    };
}

macro_rules! compute_pipeline_stage_body {
    ($syntax:ident, $slots:ident, $live:ident, $next:ident, $frontier:ident,
     $staged:ident, $active:ident) => {
        compute_pipeline_stage_body!(@annotated $syntax, $slots, $live, $next, $frontier,
            $staged, $active, i, candidate, epoch, index, generation, identity,
            [], [], [], [], [])
    };
    (@annotated $syntax:ident, $slots:ident, $live:ident, $next:ident, $frontier:ident,
     $staged:ident, $active:ident, $i:ident, $candidate:ident, $epoch:ident,
     $index:ident, $generation:ident, $identity:ident,
     [$($snapshot:tt)*], [$($invariants:tt)*], [$($step:tt)*],
     [$($before_store:tt)*], [$($after_store:tt)*]) => {
        $syntax!({
            $($snapshot)*
            if $active.id == 0 || $staged.is_some() || *$live >= $slots.len() - 1 {
                return Err($active);
            }
            let Some($epoch) = $next else { return Err($active); };
            if $epoch == 0 { return Err($active); }
            if checked_frontier($slots, *$live, $frontier, *$staged).is_err() {
                return Err($active);
            }
            let mut $i = 0usize;
            let mut $candidate = None;
            // Retain the first vacancy, but inspect the entire suffix before any write.
            while $i < $slots.len() $($invariants)* {
                if let Some(entry) = $slots[$i].entry.as_ref() {
                    if entry.active.id == $active.id || entry.identity.logical_epoch >= $epoch {
                        return Err($active);
                    }
                } else if $candidate.is_none() {
                    if let Some(generation) = vacant_generation(&$slots[$i]) {
                        $candidate = Some(($i, generation));
                    }
                }
                $i += 1;
                $($step)*
            }
            let Some(($index, $generation)) = $candidate else { return Err($active); };
            let $identity = RuntimeComputePipelineIdentityV1 {
                slot: u16::try_from($index).expect("closed runtime compute pipeline capacity"),
                slot_generation: $generation,
                logical_epoch: $epoch,
                submission: $active.id,
            };
            $($before_store)*
            let slot = &mut $slots[$index];
            slot.generation = $generation;
            slot.entry = Some(RuntimeComputePipelineEntryV1 {
                identity: $identity, phase: RuntimeComputePipelinePhaseV1::Publishing, active: $active,
            });
            *$live += 1;
            *$staged = Some($identity);
            $($after_store)*
            Ok($identity)
        })
    };
}

macro_rules! compute_pipeline_confirm_body {
    ($syntax:ident, $slots:ident, $live:ident, $next:ident, $frontier:ident,
     $staged:ident, $identity:ident) => {
        compute_pipeline_confirm_body!(@annotated $syntax, $slots, $live, $next, $frontier,
            $staged, $identity, [], [], [])
    };
    (@annotated $syntax:ident, $slots:ident, $live:ident, $next:ident, $frontier:ident,
     $staged:ident, $identity:ident, [$($snapshot:tt)*], [$($before:tt)*], [$($after:tt)*]) => {
        $syntax!({
            $($snapshot)*
            if !staged_intact($slots, $live, *$next, *$frontier, *$staged, $identity) {
                return Err(());
            }
            $($before)*
            exact_entry_mut($slots, $identity).unwrap().phase = RuntimeComputePipelinePhaseV1::Published;
            *$staged = None;
            *$next = $identity.logical_epoch.checked_add(1);
            if $frontier.is_none() { *$frontier = Some($identity.logical_epoch); }
            $($after)*
            Ok(())
        })
    };
}

macro_rules! compute_pipeline_withdraw_body {
    ($syntax:ident, $slots:ident, $live:ident, $next:ident, $frontier:ident,
     $staged:ident, $identity:ident) => {
        compute_pipeline_withdraw_body!(@annotated $syntax, $slots, $live, $next, $frontier,
            $staged, $identity, entry, [], [], [])
    };
    (@annotated $syntax:ident, $slots:ident, $live:ident, $next:ident, $frontier:ident,
     $staged:ident, $identity:ident, $entry:ident,
     [$($snapshot:tt)*], [$($before:tt)*], [$($after:tt)*]) => {
        $syntax!({
            $($snapshot)*
            if !staged_intact($slots, *$live, $next, $frontier, *$staged, $identity) {
                return None;
            }
            $($before)*
            let $entry = $slots[$identity.slot as usize].entry.take().unwrap();
            *$live -= 1;
            *$staged = None;
            $($after)*
            Some($entry.active)
        })
    };
}
