//! Fixed host-only breakpoint locations. No owner, pointer or GPU value escapes.
use std::sync::atomic::{AtomicU8, Ordering};
static HOST_RENDEZVOUS: AtomicU8 = AtomicU8::new(0);

// SAFETY: fixed unique entry symbol in the standalone observer. No pointer or
// native runtime capability is exposed; only a host atomic is modified.
#[allow(
    unsafe_code,
    reason = "fixed unique host-only process-entry rendezvous symbol"
)]
#[unsafe(no_mangle)]
#[inline(never)]
pub(super) extern "C" fn fe2o3_gfx950_noqueue_process_entry_v1() {
    HOST_RENDEZVOUS.fetch_or(4, Ordering::SeqCst);
}

// SAFETY: unique exported names are confined to this standalone observer;
// functions are ordinary no-argument host code with no pointer/runtime API.
#[allow(
    unsafe_code,
    reason = "fixed unique host-only debugger rendezvous symbol"
)]
#[unsafe(no_mangle)]
#[inline(never)]
pub(super) extern "C" fn fe2o3_gfx950_noqueue_pre_activation_v1() {
    HOST_RENDEZVOUS.fetch_or(1, Ordering::SeqCst);
}

// SAFETY: same standalone symbol-uniqueness contract; no GPU operation occurs.
#[allow(
    unsafe_code,
    reason = "fixed unique host-only debugger rendezvous symbol"
)]
#[unsafe(no_mangle)]
#[inline(never)]
pub(super) extern "C" fn fe2o3_gfx950_noqueue_post_publication_v1() {
    HOST_RENDEZVOUS.fetch_or(2, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_host_markers_have_real_observable_side_effects_without_native_io() {
        fe2o3_gfx950_noqueue_process_entry_v1();
        fe2o3_gfx950_noqueue_pre_activation_v1();
        fe2o3_gfx950_noqueue_post_publication_v1();
        assert_eq!(HOST_RENDEZVOUS.load(Ordering::SeqCst) & 7, 7);
    }
}
