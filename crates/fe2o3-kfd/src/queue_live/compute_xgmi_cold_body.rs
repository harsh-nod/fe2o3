// Shared executable admission predicate; this grants no dispatch or DMA authority.
macro_rules! compute_xgmi_cold_endpoint_body_v1 {
    ($facts:ident) => {{
        $facts.completion_releasable
            && $facts.submission_pristine
            && !$facts.dispatch_attached
            && $facts.unpublished_clear
            && $facts.detached_data_count == 0
            && !$facts.detached_generation_present
            && $facts.detached_identity_count == 0
            && !$facts.detached_insertion_present
            && $facts.next_persistent_generation == 1
    }};
}
