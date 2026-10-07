// The three public adapters share their executable delegation with Verus.
macro_rules! journal_unknown_wrapper_body {
    ($journal:ident, $writer:ident, $execute:path) => {
        $execute($journal, $writer)
    };
}

macro_rules! stable_unknown_wrapper_body {
    ($owner:ident, $writer:ident) => {
        $owner.journal.mark_unknown($writer)
    };
}

macro_rules! producer_unknown_wrapper_body {
    ($owner:ident, $writer:ident) => {
        $owner.stable.mark_unknown($writer)
    };
}
