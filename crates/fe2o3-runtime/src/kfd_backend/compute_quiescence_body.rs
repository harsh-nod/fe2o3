// Shared scalar control; exact physical quiescence and retained ownership are adapter obligations.
macro_rules! compute_quiescence_step_body {
    ($syntax:ident, $cursor:ident, $len:ident, $polled:ident, $exact:ident) => {
        $syntax!({
            let mut next_cursor = $cursor;
            let mut next_polled = $polled;
            let action = if $cursor > $len {
                QuiescenceActionV1::Invalid
            } else if $cursor == $len {
                QuiescenceActionV1::Complete
            } else if $exact {
                next_cursor += 1;
                QuiescenceActionV1::Advance
            } else if $polled {
                QuiescenceActionV1::Wait
            } else {
                next_polled = true;
                QuiescenceActionV1::Poll
            };
            QuiescenceStepV1 { cursor: next_cursor, polled: next_polled, action }
        })
    };
}

macro_rules! compute_quiescence_complete_body {
    ($syntax:ident, $cursor:ident, $len:ident) => {
        $syntax!({ $cursor == $len })
    };
}
