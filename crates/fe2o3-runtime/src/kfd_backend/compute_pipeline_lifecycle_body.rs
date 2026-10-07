// Promotion moves custody into the separate Active slot. It is not physical
// retirement. The caller establishes occupancy before this private operation.
macro_rules! compute_pipeline_first_epoch_body {
    ($syntax:ident, $slots:ident, $epoch:ident) => {
        compute_pipeline_first_epoch_body!(@annotated $syntax, $slots, $epoch, i, [])
    };
    (@annotated $syntax:ident, $slots:ident, $epoch:ident, $i:ident, [$($invariants:tt)*]) => {
        $syntax!({
            let mut $i = 0usize;
            while $i < $slots.len() $($invariants)* {
                if let Some(entry) = $slots[$i].entry.as_ref() {
                    if entry.identity.logical_epoch == $epoch { return Some($i); }
                }
                $i += 1;
            }
            None
        })
    };
}

macro_rules! compute_pipeline_take_frontier_body {
    ($syntax:ident, $slots:ident, $live:ident, $frontier:ident, $staged:ident) => {
        compute_pipeline_take_frontier_body!(@annotated $syntax, $slots, $live, $frontier,
            $staged, epoch, index, entry, out, [], [], [])
    };
    (@annotated $syntax:ident, $slots:ident, $live:ident, $frontier:ident, $staged:ident,
     $epoch:ident, $index:ident, $entry:ident, $out:ident,
     [$($snapshot:tt)*], [$($before:tt)*], [$($after:tt)*]) => {
        $syntax!({
            $($snapshot)*
            if $staged.is_some() { return None; }
            let $epoch = (*$frontier)?;
            let $index = first_epoch($slots, $epoch)?;
            $($before)*
            let $entry = $slots[$index].entry.take().expect("selected commit-frontier entry");
            *$live = $live.checked_sub(1).expect("pipeline entry was live");
            *$frontier = if *$live == 0 { None } else { $epoch.checked_add(1) };
            let $out = Some(($entry.phase, $entry.active));
            $($after)*
            $out
        })
    };
}

macro_rules! compute_pipeline_quarantine_body {
    ($syntax:ident, $slots:ident) => {
        compute_pipeline_quarantine_body!(@annotated $syntax, $slots, i, [], [], [])
    };
    (@annotated $syntax:ident, $slots:ident, $i:ident,
     [$($snapshot:tt)*], [$($invariants:tt)*], [$($step:tt)*]) => {
        $syntax!({
            $($snapshot)*
            let mut $i = 0usize;
            while $i < $slots.len() $($invariants)* {
                if let Some(entry) = $slots[$i].entry.as_mut() {
                    entry.phase = RuntimeComputePipelinePhaseV1::Quarantined;
                }
                $i += 1;
                $($step)*
            }
        })
    };
}
