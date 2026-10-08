// Shared executable transactions. Proof arguments add only erased annotations.
macro_rules! graph_version_begin_body_v1 {
    ($syntax:ident, $ledger:ident, $node:ident,
     ($check:ident, $write:ident, $count:ident),
     [$($check_invariants:tt)*], [$($checked:tt)*],
     [$($write_invariants:tt)*], [$($before_write:tt)*], [$($after_write:tt)*]) => {
        $syntax!({
            let $count = $ledger.uses[$node].len();
            let mut $check = 0usize;
            while $check < $count
                $($check_invariants)*
            {
                let usage = &$ledger.uses[$node][$check];
                if let Some(input) = usage.input {
                    if !r65_version_input_ready_v1(
                        $ledger.current[usage.segment], input, $ledger.records[input].state,
                    ) {
                        return false;
                    }
                }
                if let Some(output) = usage.output {
                    let Some(prior) = $ledger.records[output].predecessor else {
                        return false;
                    };
                    if !r65_version_begin_write_v1(
                        $ledger.records[output].state, $ledger.pending[usage.segment],
                        $ledger.current[usage.segment], prior,
                    ) {
                        return false;
                    }
                }
                $check += 1;
            }
            if $ledger.started[$node] {
                return false;
            }
            $($checked)*
            $ledger.started[$node] = true;
            let mut $write = 0usize;
            while $write < $count
                $($write_invariants)*
            {
                $($before_write)*
                let usage = &$ledger.uses[$node][$write];
                if let Some(output) = usage.output {
                    $ledger.current[usage.segment] = None;
                    $ledger.pending[usage.segment] = Some(output);
                    $ledger.records[output].state = RuntimeGraphVersionStateV1::InFlight;
                }
                $($after_write)*
                $write += 1;
            }
            true
        })
    };
}

macro_rules! graph_version_commit_body_v1 {
    ($syntax:ident, $ledger:ident, $node:ident,
     ($check:ident, $write:ident, $count:ident),
     [$($check_invariants:tt)*], [$($checked:tt)*],
     [$($write_invariants:tt)*], [$($before_write:tt)*], [$($after_write:tt)*]) => {
        $syntax!({
            let $count = $ledger.uses[$node].len();
            let mut $check = 0usize;
            while $check < $count
                $($check_invariants)*
            {
                let usage = &$ledger.uses[$node][$check];
                if let Some(output) = usage.output {
                    if !r65_version_commit_write_v1(
                        $ledger.records[output].state, $ledger.pending[usage.segment],
                        output, $ledger.current[usage.segment],
                    ) {
                        return false;
                    }
                }
                $check += 1;
            }
            $($checked)*
            let mut $write = 0usize;
            while $write < $count
                $($write_invariants)*
            {
                $($before_write)*
                let usage = &$ledger.uses[$node][$write];
                if let Some(output) = usage.output {
                    $ledger.records[output].state = RuntimeGraphVersionStateV1::Committed;
                    $ledger.current[usage.segment] = Some(output);
                    $ledger.pending[usage.segment] = None;
                }
                $($after_write)*
                $write += 1;
            }
            true
        })
    };
}
