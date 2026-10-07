// Ordinary Rust and the source-bound proof execute these same forwarding calls.
macro_rules! producer_observe_active_lookup_body_v1 {
    ($observer:expr, $reference:expr) => {
        $observer.versions.journal.lookup_producer_read($reference)
    };
}

macro_rules! producer_observe_active_status_body_v1 {
    ($observer:expr, $reference:expr) => {
        $observer.versions.journal.producer_read_status($reference)
    };
}

macro_rules! producer_observe_queued_lookup_body_v1 {
    ($observer:expr, $reference:expr) => {
        $observer.versions.journal.lookup_queued_producer_read($reference)
    };
}

macro_rules! producer_observe_queued_status_body_v1 {
    ($observer:expr, $reference:expr) => {
        $observer.versions.journal.queued_producer_read_status($reference)
    };
}
