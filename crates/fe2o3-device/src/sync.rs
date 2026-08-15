//! Managed barrier authority and target lifecycle contracts.
//!
//! Reserved low-level hooks remain panic stubs until a trusted frontend
//! recognizes them; only direct Kernel IR barrier lowering is executable.

use core::marker::PhantomData;
use core::num::NonZeroU32;

mod sealed {
    pub trait AmdBarrierTarget {}
    pub trait NativeSplitBarrierTarget {}
    pub trait ValidNamedBarrierSlot {}
}

/// Reviewed CDNA3 target marker.
#[derive(Debug)]
pub enum Gfx942 {}

/// AMD gfx12-family target marker.
#[derive(Debug)]
pub enum Gfx12 {}

/// Sealed architecture contract for managed workgroup barriers.
pub trait AmdBarrierTarget: sealed::AmdBarrierTarget {
    const NAME: &'static str;
    const MAX_PARTICIPANTS: u32;
    const NATIVE_SPLIT_BARRIERS: bool;
}

impl sealed::AmdBarrierTarget for Gfx942 {}
impl AmdBarrierTarget for Gfx942 {
    const NAME: &'static str = "gfx942";
    const MAX_PARTICIPANTS: u32 = 1024;
    const NATIVE_SPLIT_BARRIERS: bool = false;
}

impl sealed::AmdBarrierTarget for Gfx12 {}
impl AmdBarrierTarget for Gfx12 {
    const NAME: &'static str = "gfx12";
    const MAX_PARTICIPANTS: u32 = 1024;
    const NATIVE_SPLIT_BARRIERS: bool = true;
}

/// Sealed proof that a target provides AMD split/named barrier instructions.
///
/// gfx942 deliberately does not implement this trait.
pub trait NativeSplitBarrierTarget: AmdBarrierTarget + sealed::NativeSplitBarrierTarget {}

impl sealed::NativeSplitBarrierTarget for Gfx12 {}
impl NativeSplitBarrierTarget for Gfx12 {}

/// Type-level named barrier slot.
#[derive(Debug)]
pub struct NamedBarrierSlot<const SLOT: u8>;

/// Sealed proof that a slot is representable by the bounded AMD abstraction.
pub trait ValidNamedBarrierSlot: sealed::ValidNamedBarrierSlot {}

macro_rules! valid_named_barrier_slots {
    ($($slot:literal),+ $(,)?) => {
        $(
            impl sealed::ValidNamedBarrierSlot for NamedBarrierSlot<$slot> {}
            impl ValidNamedBarrierSlot for NamedBarrierSlot<$slot> {}
        )+
    };
}

valid_named_barrier_slots!(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);

/// Barrier storage has not been initialized for an epoch.
#[derive(Debug)]
pub enum BarrierUninitialized {}

/// Barrier storage is ready for one synchronization epoch.
#[derive(Debug)]
pub enum BarrierReady {}

/// The current invocation has signaled and must wait before reuse or teardown.
#[derive(Debug)]
pub enum BarrierPending {}

/// Failure to initialize a bounded managed barrier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BarrierInitializationError {
    pub participants: u32,
    pub maximum: u32,
}

/// Linear workgroup barrier authority with target, slot, and lifecycle state.
///
/// The value is neither cloneable nor transferable. Only a ready value can be
/// destroyed or synchronized; split arrival consumes it into a pending value,
/// and only waiting recovers the ready state.
#[must_use = "managed barrier state must be consumed by its next lifecycle transition"]
pub struct ManagedBarrier<'workgroup, Target, State, const SLOT: u8 = 0>
where
    Target: AmdBarrierTarget,
    NamedBarrierSlot<SLOT>: ValidNamedBarrierSlot,
{
    participants: u32,
    _workgroup: PhantomData<&'workgroup mut &'workgroup ()>,
    _target: PhantomData<fn() -> Target>,
    _state: PhantomData<fn() -> State>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'workgroup, Target, const SLOT: u8>
    ManagedBarrier<'workgroup, Target, BarrierUninitialized, SLOT>
where
    Target: AmdBarrierTarget,
    NamedBarrierSlot<SLOT>: ValidNamedBarrierSlot,
{
    /// Creates compiler-owned authority for one workgroup barrier slot.
    ///
    /// # Safety
    ///
    /// The compiler must issue at most one live value for this target, slot,
    /// workgroup, and lifetime. The lifetime must not outlive the workgroup.
    #[doc(hidden)]
    pub unsafe fn from_compiler() -> Self {
        Self {
            participants: 0,
            _workgroup: PhantomData,
            _target: PhantomData,
            _state: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    /// Initializes one barrier epoch with a statically bounded participant count.
    pub fn initialize(
        self,
        participants: NonZeroU32,
    ) -> Result<ManagedBarrier<'workgroup, Target, BarrierReady, SLOT>, BarrierInitializationError>
    where
        Target: NativeSplitBarrierTarget,
    {
        if participants.get() > Target::MAX_PARTICIPANTS {
            return Err(BarrierInitializationError {
                participants: participants.get(),
                maximum: Target::MAX_PARTICIPANTS,
            });
        }
        Ok(self.transition(participants.get()))
    }
}

impl<'workgroup> ManagedBarrier<'workgroup, Gfx942, BarrierUninitialized, 0> {
    /// Admits the reviewed gfx942 full-workgroup barrier lifecycle.
    ///
    /// # Safety
    ///
    /// `participants` must equal the launch's complete workgroup size. The
    /// current frontend does not authenticate this value from launch metadata.
    pub unsafe fn initialize_full_workgroup(
        self,
        participants: NonZeroU32,
    ) -> Result<ManagedBarrier<'workgroup, Gfx942, BarrierReady, 0>, BarrierInitializationError>
    {
        if participants.get() > Gfx942::MAX_PARTICIPANTS {
            return Err(BarrierInitializationError {
                participants: participants.get(),
                maximum: Gfx942::MAX_PARTICIPANTS,
            });
        }
        Ok(self.transition(participants.get()))
    }
}

impl<'workgroup, Target, const SLOT: u8> ManagedBarrier<'workgroup, Target, BarrierReady, SLOT>
where
    Target: AmdBarrierTarget,
    NamedBarrierSlot<SLOT>: ValidNamedBarrierSlot,
{
    pub const fn participants(&self) -> u32 {
        self.participants
    }

    /// Ends the initialized epoch and recovers uninitialized authority.
    pub fn destroy(self) -> ManagedBarrier<'workgroup, Target, BarrierUninitialized, SLOT> {
        self.transition(0)
    }
}

impl<'workgroup> ManagedBarrier<'workgroup, Gfx942, BarrierReady, 0> {
    /// Executes the reviewed gfx942 full workgroup barrier and remains ready.
    ///
    /// # Safety
    ///
    /// All active work-items must execute this dynamic call in uniform order.
    pub unsafe fn arrive_and_wait(self) -> Self {
        // SAFETY: the caller establishes the convergence contract.
        unsafe { syncthreads() };
        self
    }

    /// Begins a gfx942 deferred full-workgroup synchronization epoch.
    ///
    /// gfx942 has no physical split/named-barrier instruction. This operation
    /// therefore performs the release half only. Every participating work-item
    /// must later call `ManagedBarrier::wait`, which executes the physical
    /// full-workgroup barrier and acquire half. Producer/consumer subsets and
    /// work-items that arrive without waiting are not supported.
    ///
    /// # Safety
    ///
    /// All declared participants must execute this call and its matching wait
    /// in uniform dynamic order.
    pub unsafe fn arrive(self) -> ManagedBarrier<'workgroup, Gfx942, BarrierPending, 0> {
        // SAFETY: the caller establishes the uniform full-workgroup epoch.
        unsafe { gfx942_barrier_arrive() };
        let participants = self.participants;
        self.transition(participants)
    }
}

impl<'workgroup> ManagedBarrier<'workgroup, Gfx942, BarrierPending, 0> {
    /// Completes a gfx942 deferred full-workgroup synchronization epoch.
    ///
    /// # Safety
    ///
    /// Every declared participant must have executed the matching arrival and
    /// must execute this wait in uniform dynamic order.
    pub unsafe fn wait(self) -> ManagedBarrier<'workgroup, Gfx942, BarrierReady, 0> {
        // SAFETY: the caller establishes the uniform full-workgroup epoch.
        unsafe { gfx942_barrier_wait() };
        let participants = self.participants;
        self.transition(participants)
    }
}

impl<'workgroup, Target, const SLOT: u8> ManagedBarrier<'workgroup, Target, BarrierReady, SLOT>
where
    Target: NativeSplitBarrierTarget,
    NamedBarrierSlot<SLOT>: ValidNamedBarrierSlot,
{
    /// Signals split-barrier arrival and transfers authority to pending state.
    ///
    /// # Safety
    ///
    /// The participant set, slot identity, and dynamic arrival sequence must be
    /// uniform. Target lowering must preserve release semantics. The returned
    /// pending authority must be waited; dropping or forgetting it violates
    /// this method's safety contract.
    pub unsafe fn arrive(self) -> ManagedBarrier<'workgroup, Target, BarrierPending, SLOT> {
        // SAFETY: this method exists only for a sealed native-split target.
        unsafe { split_barrier_arrive(SLOT) };
        let participants = self.participants;
        self.transition(participants)
    }
}

impl<'workgroup, Target, const SLOT: u8> ManagedBarrier<'workgroup, Target, BarrierPending, SLOT>
where
    Target: NativeSplitBarrierTarget,
    NamedBarrierSlot<SLOT>: ValidNamedBarrierSlot,
{
    /// Waits for the signaled epoch and recovers ready authority.
    ///
    /// # Safety
    ///
    /// The matching arrival must have executed for every declared participant.
    /// Target lowering must preserve acquire semantics.
    pub unsafe fn wait(self) -> ManagedBarrier<'workgroup, Target, BarrierReady, SLOT> {
        // SAFETY: this method exists only for a sealed native-split target.
        unsafe { split_barrier_wait(SLOT) };
        let participants = self.participants;
        self.transition(participants)
    }
}

impl<'workgroup, Target, State, const SLOT: u8> ManagedBarrier<'workgroup, Target, State, SLOT>
where
    Target: AmdBarrierTarget,
    NamedBarrierSlot<SLOT>: ValidNamedBarrierSlot,
{
    fn transition<Next>(self, participants: u32) -> ManagedBarrier<'workgroup, Target, Next, SLOT> {
        ManagedBarrier {
            participants,
            _workgroup: PhantomData,
            _target: PhantomData,
            _state: PhantomData,
            _not_send_sync: PhantomData,
        }
    }
}

/// Low-level gfx12 split-barrier arrival recognized only by a future backend.
///
/// It deliberately panics on host and unsupported compiler paths.
#[inline(never)]
unsafe fn split_barrier_arrive(_slot: u8) {
    unreachable!("split barrier arrival must be lowered by a supported fe2o3 backend")
}

/// Low-level gfx12 split-barrier wait recognized only by a future backend.
///
/// It deliberately panics on host and unsupported compiler paths.
#[inline(never)]
unsafe fn split_barrier_wait(_slot: u8) {
    unreachable!("split barrier wait must be lowered by a supported fe2o3 backend")
}

/// Emits the release half of the bounded gfx942 deferred barrier profile.
///
/// The fe2o3 backend recognizes this exact diagnostic item. It remains
/// fail-closed on hosts and unsupported compiler paths.
#[doc(hidden)]
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_gfx942_barrier_arrive_v1"]
pub unsafe fn gfx942_barrier_arrive() {
    unreachable!("gfx942 barrier arrival must be lowered by the fe2o3 backend")
}

/// Emits the physical barrier and acquire half of the bounded gfx942 profile.
///
/// The fe2o3 backend recognizes this exact diagnostic item. It remains
/// fail-closed on hosts and unsupported compiler paths.
#[doc(hidden)]
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_gfx942_barrier_wait_v1"]
pub unsafe fn gfx942_barrier_wait() {
    unreachable!("gfx942 barrier wait must be lowered by the fe2o3 backend")
}

/// Executes one uniform workgroup barrier with acquire-release ordering over
/// global and workgroup memory.
///
/// This low-level entry point is intentionally unsafe. The typed
/// [`crate::Workgroup::synchronize`] operation remains unsafe for the same reasons.
/// The current compiler does not recognize or lower this function, so calling
/// it on a host or through an unsupported compilation path always panics.
///
/// # Safety
///
/// Every active work-item in the current workgroup must execute this exact
/// dynamic call once and in the same barrier sequence. No work-item may reach
/// it through non-uniform control flow, return before it, or skip it. The
/// compiler must preserve all of the following semantics:
///
/// - workgroup execution scope;
/// - workgroup memory scope;
/// - acquire-release ordering over global and workgroup memory; and
/// - uniform workgroup convergence.
///
/// Calling this function without compiler recognition that preserves those
/// properties does not synchronize a device program.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_workgroup_syncthreads_v1"]
pub unsafe fn syncthreads() {
    unreachable!("syncthreads must be lowered by the fe2o3 backend")
}
