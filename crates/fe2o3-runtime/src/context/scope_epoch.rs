//! Private persistent Context gate for lexical owners that may be forgotten.
use super::RuntimeValidationErrorV1 as Error;
use std::{
    marker::PhantomData,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

struct State {
    active: AtomicBool,
    entered: AtomicBool,
}

#[derive(Default)]
pub(super) struct Anchor {
    state: Option<Arc<State>>,
}

impl Anchor {
    pub(super) fn active(&self) -> bool {
        self.state
            .as_ref()
            .is_some_and(|s| s.active.load(Ordering::Acquire))
    }

    pub(super) fn require_access(&self) -> Result<(), Error> {
        if self
            .state
            .as_ref()
            .is_some_and(|s| s.active.load(Ordering::Acquire) && !s.entered.load(Ordering::Acquire))
        {
            Err(Error::ContextReserved)
        } else {
            Ok(())
        }
    }

    pub(super) fn begin(&mut self) -> Result<Owner, Error> {
        if self.active() {
            return Err(Error::ContextReserved);
        }
        let state = Arc::new(State {
            active: AtomicBool::new(true),
            entered: AtomicBool::new(false),
        });
        self.state = Some(Arc::clone(&state));
        Ok(Owner {
            state,
            same_thread: PhantomData,
        })
    }
}

impl Drop for Anchor {
    fn drop(&mut self) {
        if self.active() {
            // This field precedes the backend and every native Context owner.
            std::process::abort();
        }
    }
}

pub(super) struct Owner {
    state: Arc<State>,
    same_thread: PhantomData<Rc<()>>,
}

impl Owner {
    /// Only synchronous private scope operations hold this permit. It cannot be
    /// returned to callers, sent to another thread or retained across an await.
    pub(super) fn enter(&self) -> Result<Permit, Error> {
        if !self.state.active.load(Ordering::Acquire)
            || self
                .state
                .entered
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return Err(Error::ContextReserved);
        }
        Ok(Permit {
            state: Arc::clone(&self.state),
            same_thread: PhantomData,
        })
    }

    /// Caller must have checked both original rosters after native release and
    /// decoder disposal. This private bit is a gate, never settlement evidence.
    pub(super) fn close(&mut self) {
        if self.state.entered.load(Ordering::Acquire)
            || !self.state.active.swap(false, Ordering::AcqRel)
        {
            std::process::abort();
        }
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        if self.state.active.load(Ordering::Acquire) {
            std::process::abort();
        }
    }
}

pub(super) struct Permit {
    state: Arc<State>,
    same_thread: PhantomData<Rc<()>>,
}

impl Drop for Permit {
    fn drop(&mut self) {
        if !self.state.active.load(Ordering::Acquire)
            || !self.state.entered.swap(false, Ordering::AcqRel)
        {
            std::process::abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_anchor_is_send_sync_and_private_permits_are_bounded_and_nonreentrant() {
        fn send_sync<T: Send + Sync>() {}
        send_sync::<Anchor>();
        let mut anchor = Anchor::default();
        assert!(!anchor.active());
        assert!(anchor.require_access().is_ok());
        let mut owner = anchor.begin().unwrap();
        assert!(anchor.active());
        assert!(anchor.require_access().is_err());
        assert!(anchor.begin().is_err());
        {
            let permit = owner.enter().unwrap();
            assert!(anchor.require_access().is_ok());
            assert!(owner.enter().is_err());
            // Cleanup/teardown consult active, not the permit-aware access gate.
            assert!(anchor.active());
            drop(permit);
        }
        assert!(anchor.require_access().is_err());
        owner.close();
        assert!(!anchor.active());
        assert!(owner.enter().is_err());
        drop(owner);
        let mut next = anchor.begin().unwrap();
        assert!(anchor.require_access().is_err());
        next.close();
        assert!(anchor.require_access().is_ok());
    }
}
