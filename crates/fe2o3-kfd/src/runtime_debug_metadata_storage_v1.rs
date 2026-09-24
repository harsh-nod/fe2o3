//! Stable owned rendezvous storage and private no-queue activation transaction.
//! Its only production caller retains the actual trap, Context and Kernel.

use core::fmt::{self, Write};
use core::sync::atomic::fence;
use core::sync::atomic::{AtomicU8, Ordering};
use std::marker::PhantomData;
use std::rc::Rc;

#[path = "runtime_debug_abi_v11.rs"]
pub(super) mod abi;

const URI_CAPACITY: usize = 128;
#[path = "runtime_debug_empty_metadata_v1.rs"]
mod empty;
pub(super) use empty::DebugEmptyRetirementTransportV1;

static NOTIFICATION_SIDE_EFFECT: AtomicU8 = AtomicU8::new(0);

/// A genuine executable rendezvous function. Preparation never calls it.
/// Volatile protocol stores and fences surround the future active callbacks.
#[inline(never)]
extern "C" fn fe2o3_runtime_debug_state_v1() {
    NOTIFICATION_SIDE_EFFECT.fetch_add(1, Ordering::Relaxed);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum MetadataErrorV1 {
    ArtifactBound,
    Allocation,
    Identity,
    Mapping,
    LoadBias,
    UriBound,
    Currentness,
    NativeTrap,
    NativeRuntime,
    RuntimeOutput,
    ProcessChanged,
    Transition,
    #[cfg(test)]
    Notification,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Prepared,
    ActiveAbsent,
    ActivePresent,
    LocalRuntimeDisabled,
    LocalTrapCleared,
    #[cfg(test)]
    Detached,
    Poisoned,
}

/// Private transport selected only by the actual consuming native owner.
/// Tests inject failures here; no implementation or raw address can escape.
pub(super) trait DebugActivationTransportV1 {
    fn check_currentness(&mut self) -> Result<(), MetadataErrorV1>;
    fn register_trap(&mut self, trap_base: u64, gpu_id: u32) -> Result<(), MetadataErrorV1>;
    fn enable_runtime(&mut self, root_address: u64) -> Result<(), MetadataErrorV1>;

    // Observe copied state, never a saved pointer across mutable publication.
    #[cfg(test)]
    fn observe_transition(&mut self, _state: i32, _linked: bool) {}
}

struct Record {
    root: abi::RDebugAbiV11,
    link: abi::LinkMapAbiV1,
    uri: [u8; URI_CAPACITY],
}

/// No allocation is resized after its address is derived. The private, exactly
/// one-element Vec supplies a fallible allocation and keeps pointees stable
/// when this owner moves; no elements or mutable slices can escape.
pub(super) struct MetadataStorageV1 {
    record: Vec<Record>,
    original_elf: Vec<u8>,
    opener_pid: u32,
    phase: Phase,
    not_send_sync: PhantomData<Rc<()>>,
}

impl MetadataStorageV1 {
    /// Storage-only helper: callers cannot obtain native pointer authority.
    /// The sole production caller derives bias from its actual Kernel mapping.
    pub(super) fn prepare(original_elf: &[u8], load_bias: i64) -> Result<Self, MetadataErrorV1> {
        require_elf_bound(original_elf.len())?;
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(original_elf.len())
            .map_err(|_| MetadataErrorV1::Allocation)?;
        retained.extend_from_slice(original_elf);
        let mut record = Vec::new();
        record
            .try_reserve_exact(1)
            .map_err(|_| MetadataErrorV1::Allocation)?;
        record.push(Record {
            root: abi::RDebugAbiV11 {
                // No version-11 runtime is advertised by preparation.
                version: 0,
                reserved0: 0,
                map: 0,
                breakpoint: fe2o3_runtime_debug_state_v1 as *const () as usize as u64,
                state: abi::RT_CONSISTENT_V1,
                reserved1: 0,
                loader_base: 0,
            },
            link: abi::LinkMapAbiV1 {
                load_bias: load_bias as u64,
                name: 0,
                dynamic: 0,
                next: 0,
                previous: 0,
            },
            uri: [0; URI_CAPACITY],
        });
        let pid = std::process::id();
        let item = &mut record[0];
        write_uri(
            &mut item.uri,
            pid,
            retained.as_ptr() as usize as u64,
            retained.len(),
        )?;
        item.link.name = item.uri.as_ptr() as usize as u64;
        Ok(Self {
            record,
            original_elf: retained,
            opener_pid: pid,
            phase: Phase::Prepared,
            not_send_sync: PhantomData,
        })
    }

    pub(super) fn retained_elf(&self) -> &[u8] {
        &self.original_elf
    }

    pub(super) fn prepared_bytes(&self) -> usize {
        core::mem::size_of::<Record>() + self.original_elf.len()
    }

    pub(super) fn is_prepared(&self) -> bool {
        self.opener_pid == std::process::id()
            && self.phase == Phase::Prepared
            && self.record[0].root.version == 0
            && self.record[0].root.map == 0
            && self.record[0].root.state == abi::RT_CONSISTENT_V1
    }

    /// Called only while actual Context, trap and Kernel custody is retained.
    /// No runtime/queue token is produced here; successful metadata publication
    /// cannot be converted to execution authority.
    pub(super) fn activate_no_queue(
        &mut self,
        trap_base: u64,
        gpu_id: u32,
        transport: &mut impl DebugActivationTransportV1,
    ) -> Result<(), MetadataErrorV1> {
        if !self.is_prepared() {
            return Err(MetadataErrorV1::Transition);
        }
        if trap_base == 0 || !trap_base.is_multiple_of(4096) || gpu_id == 0 {
            return Err(MetadataErrorV1::Mapping);
        }
        transport.check_currentness()?;
        // Retain metadata as well as the outer native custody before the first
        // attempted syscall. No rollback, reuse or retry follows ambiguity.
        self.phase = Phase::Poisoned;
        transport.register_trap(trap_base, gpu_id)?;
        transport.check_currentness()?;
        // Advertise the ABI only after trap registration acknowledgment, while
        // the code-object list is absent and no queue exists in this owner.
        // SAFETY: this exclusively owned, initialized ABI word remains in its
        // immovable allocation until process exit after any native attempt.
        unsafe {
            core::ptr::write_volatile(
                &mut self.record[0].root.version,
                abi::REQUIRED_ROCR_DEBUG_VERSION_V11,
            );
        }
        fence(Ordering::Release);
        let root = &self.record[0].root as *const _ as usize as u64;
        transport.enable_runtime(root)?;
        #[cfg(test)]
        {
            let snapshot = self.snapshot();
            transport.observe_transition(snapshot.state, snapshot.linked);
        }
        transport.check_currentness()?;
        self.phase = Phase::ActiveAbsent;
        self.transition(true, |snapshot| {
            // Snapshot is descriptive test instrumentation, never native authority.
            let _ = (snapshot.state, snapshot.linked);
            #[cfg(test)]
            transport.observe_transition(snapshot.state, snapshot.linked);
            transport.check_currentness()?;
            fe2o3_runtime_debug_state_v1();
            #[cfg(test)]
            transport.observe_transition(snapshot.state, snapshot.linked);
            transport.check_currentness()
        })
    }

    // Synthetic wrappers exercise the same private publication engine.
    #[cfg(test)]
    fn publish_link(&mut self) -> Result<(), MetadataErrorV1> {
        self.transition(true, |_| {
            fe2o3_runtime_debug_state_v1();
            Ok(())
        })
    }

    #[cfg(test)]
    fn retire_link(&mut self) -> Result<(), MetadataErrorV1> {
        self.transition(false, |_| {
            fe2o3_runtime_debug_state_v1();
            Ok(())
        })
    }

    fn transition(
        &mut self,
        add: bool,
        mut notify: impl FnMut(TransitionSnapshotV1) -> Result<(), MetadataErrorV1>,
    ) -> Result<(), MetadataErrorV1> {
        if self.opener_pid != std::process::id() {
            self.phase = Phase::Poisoned;
            return Err(MetadataErrorV1::ProcessChanged);
        }
        let expected = if add {
            Phase::ActiveAbsent
        } else {
            Phase::ActivePresent
        };
        if self.phase != expected {
            return Err(MetadataErrorV1::Transition);
        }

        // A refusal or unwind after the first publication must retain all
        // metadata/URI/original-ELF storage. Actual GPU mapping retention belongs
        // to the outer consuming owner and cannot be replaced by this list.
        self.phase = Phase::Poisoned;
        self.store_state(if add {
            abi::RT_ADD_V1
        } else {
            abi::RT_DELETE_V1
        });
        fence(Ordering::AcqRel);
        notify(self.snapshot())?;
        fence(Ordering::AcqRel);
        let next = if add {
            &self.record[0].link as *const _ as usize as u64
        } else {
            0
        };
        // SAFETY: one retained initialized Record, exclusively borrowed; the
        // volatile store is observable by an external stopped-process debugger.
        unsafe { core::ptr::write_volatile(&mut self.record[0].root.map, next) };
        self.store_state(abi::RT_CONSISTENT_V1);
        fence(Ordering::Release);
        notify(self.snapshot())?;
        if self.opener_pid != std::process::id() {
            return Err(MetadataErrorV1::ProcessChanged);
        }
        self.phase = if add {
            Phase::ActivePresent
        } else {
            Phase::ActiveAbsent
        };
        Ok(())
    }

    fn store_state(&mut self, state: i32) {
        // SAFETY: exclusive access to this initialized ABI field. Other Rust
        // threads cannot share this owner; external debugger reads are not Rust
        // references. Native publication retains this allocation process-wide.
        unsafe { core::ptr::write_volatile(&mut self.record[0].root.state, state) };
    }

    fn snapshot(&self) -> TransitionSnapshotV1 {
        TransitionSnapshotV1 {
            state: self.record[0].root.state,
            linked: self.record[0].root.map != 0,
        }
    }

    fn take_storage_to_retain(&mut self) -> Option<(Vec<Record>, Vec<u8>)> {
        if matches!(
            self.phase,
            Phase::ActiveAbsent
                | Phase::ActivePresent
                | Phase::LocalRuntimeDisabled
                | Phase::Poisoned
        ) {
            Some((
                core::mem::take(&mut self.record),
                core::mem::take(&mut self.original_elf),
            ))
        } else {
            None
        }
    }

    #[cfg(test)]
    fn activate_for_test(&mut self) {
        assert!(self.is_prepared());
        self.record[0].root.version = abi::REQUIRED_ROCR_DEBUG_VERSION_V11;
        self.phase = Phase::ActiveAbsent;
    }

    #[cfg(test)]
    fn acknowledge_detached_for_test(&mut self) {
        assert_eq!(self.phase, Phase::ActiveAbsent);
        self.phase = Phase::Detached;
    }
}

impl Drop for MetadataStorageV1 {
    fn drop(&mut self) {
        if let Some((record, original_elf)) = self.take_storage_to_retain() {
            // Independent fallback retention of every metadata pointee. The
            // consuming outer custody also retains all GPU mappings and FDs.
            // Neither layer interprets Drop as a native teardown acknowledgment.
            core::mem::forget(record);
            core::mem::forget(original_elf);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TransitionSnapshotV1 {
    state: i32,
    linked: bool,
}

fn require_elf_bound(len: usize) -> Result<(), MetadataErrorV1> {
    if len == 0 || len > fe2o3_hsaco::MAX_HSACO_BYTES {
        Err(MetadataErrorV1::ArtifactBound)
    } else {
        Ok(())
    }
}

struct UriWriter<'a> {
    bytes: &'a mut [u8; URI_CAPACITY],
    used: usize,
}
impl Write for UriWriter<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let end = self.used.checked_add(value.len()).ok_or(fmt::Error)?;
        if !value.is_ascii() || end >= self.bytes.len() {
            return Err(fmt::Error);
        }
        self.bytes[self.used..end].copy_from_slice(value.as_bytes());
        self.used = end;
        Ok(())
    }
}

fn write_uri(
    output: &mut [u8; URI_CAPACITY],
    pid: u32,
    address: u64,
    bytes: usize,
) -> Result<(), MetadataErrorV1> {
    output.fill(0);
    if pid == 0 || address == 0 || bytes == 0 {
        return Err(MetadataErrorV1::UriBound);
    }
    let mut writer = UriWriter {
        bytes: output,
        used: 0,
    };
    write!(
        &mut writer,
        "memory://{pid}#offset=0x{address:x}&size={bytes}"
    )
    .map_err(|_| MetadataErrorV1::UriBound)
}

pub(super) fn checked_load_bias(
    actual_mapping: u64,
    image_start: u64,
) -> Result<i64, MetadataErrorV1> {
    if actual_mapping == 0 {
        return Err(MetadataErrorV1::Mapping);
    }
    i64::try_from(i128::from(actual_mapping) - i128::from(image_start))
        .map_err(|_| MetadataErrorV1::LoadBias)
}

#[cfg(test)]
#[path = "runtime_debug_metadata_storage_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "runtime_debug_noqueue_transaction_v1_tests.rs"]
mod noqueue_tests;
