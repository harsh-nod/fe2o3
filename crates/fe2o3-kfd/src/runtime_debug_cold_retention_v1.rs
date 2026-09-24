//! Host-only custody mechanism; not a native cleanup acknowledgment.

/// This owner deliberately has no disarm operation after a possible native
/// effect. A later typed teardown design must not replace this with a bool.
pub(in super::super) struct RetainNativeOnDropV1<T> {
    value: Option<T>,
    opener_pid: u32,
    may_have_native_effects: bool,
}

impl<T> RetainNativeOnDropV1<T> {
    pub(in super::super) fn new(value: T) -> Self {
        Self {
            value: Some(value),
            opener_pid: std::process::id(),
            may_have_native_effects: false,
        }
    }

    pub(in super::super) fn get(&self) -> &T {
        self.value.as_ref().expect("retained owner remains present")
    }

    pub(in super::super) fn get_mut(&mut self) -> &mut T {
        self.value.as_mut().expect("retained owner remains present")
    }

    pub(in super::super) fn retain_before_native_effect(&mut self) {
        self.may_have_native_effects = true;
    }
}

fn must_retain(possible_effects: bool, owner_pid: u32, current_pid: u32) -> bool {
    possible_effects || owner_pid != current_pid || owner_pid == 0
}

impl<T> Drop for RetainNativeOnDropV1<T> {
    fn drop(&mut self) {
        if must_retain(
            self.may_have_native_effects,
            self.opener_pid,
            std::process::id(),
        ) && let Some(value) = self.value.take()
        {
            // Retain FDs, Context, actual mappings, original ELF and metadata
            // together. No ioctl, close, free retry or inherited mutex access.
            core::mem::forget(value);
        }
    }
}

#[cfg(test)]
#[path = "runtime_debug_cold_retention_v1_tests.rs"]
mod tests;
