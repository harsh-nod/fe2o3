//! Bounded, explicitly selected full-forward diagnostics. No wire shape changes.

use super::*;
use fe2o3_aql::{
    AmdQueueProfilingPolicyV1, GpuSystemClockBracketV1, GpuSystemClockSampleV1,
    parse_amd_busy_dispatch_timestamp_snapshot_v1,
};
use serde::Serialize;
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{Seek, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::Path;

const MAX_FRAMES: u64 = 36;
const MAX_FRAME_BYTES: usize = 1024 * 1024;
const MAX_FILE_BYTES: u64 = 38 * 1024 * 1024;
const COUNT: usize = FULL_FORWARD_DISPATCHES_V1;

struct BoundedJson(Vec<u8>);
impl Write for BoundedJson {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self
            .0
            .len()
            .checked_add(bytes.len())
            .is_none_or(|end| end > MAX_FRAME_BYTES)
        {
            return Err(std::io::Error::other("full-forward diagnostic frame cap"));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct DiagnosticFile {
    parent: File,
    file: File,
    basename: CString,
    parent_identity: (u64, u64),
    file_identity: (u64, u64),
    uid: u32,
    written: u64,
    frames: u64,
    closed: bool,
}

impl DiagnosticFile {
    fn open(path: &Path, unique_id: u64, policy: AmdQueueProfilingPolicyV1) -> Result<Self> {
        if !path.is_absolute() {
            return Err("diagnostic output must be absolute".into());
        }
        let parent_path = path.parent().ok_or("diagnostic parent")?;
        if parent_path.canonicalize().map_err(explain)? != parent_path {
            return Err("diagnostic parent must be canonical without symlinks".into());
        }
        let name = path.file_name().ok_or("diagnostic basename")?.as_bytes();
        if name.is_empty()
            || name.len() > 128
            || !name[0].is_ascii_alphanumeric()
            || !name
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(byte))
        {
            return Err("diagnostic basename outside closed grammar".into());
        }
        let basename = CString::new(name).map_err(explain)?;
        let parent = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(parent_path)
            .map_err(explain)?;
        let uid = rustix::process::geteuid().as_raw();
        let metadata = parent.metadata().map_err(explain)?;
        if !metadata.is_dir()
            || metadata.uid() != uid
            || metadata.mode() & 0o022 != 0
            || metadata.nlink() == 0
        {
            return Err("diagnostic parent ownership or mode".into());
        }
        let parent_identity = (metadata.dev(), metadata.ino());
        // SAFETY: retained validated directory FD and a single NUL-terminated
        // basename; never reopen an absolute path after parent validation.
        let descriptor = unsafe {
            libc::openat(
                parent.as_raw_fd(),
                basename.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if descriptor < 0 {
            return Err(explain(std::io::Error::last_os_error()));
        }
        // SAFETY: openat returned one newly owned descriptor.
        let file = unsafe { File::from_raw_fd(descriptor) };
        let metadata = file.metadata().map_err(explain)?;
        let mut writer = Self {
            parent,
            file,
            basename,
            parent_identity,
            file_identity: (metadata.dev(), metadata.ino()),
            uid,
            written: 0,
            frames: 0,
            closed: false,
        };
        writer.check_identity()?;
        writer.write_record(&serde_json::json!({
            "schema":"NativeFullForwardTimestampHeaderV1", "authority":"none",
            "device_unique_id":unique_id, "queue_profiling_enabled":policy==AmdQueueProfilingPolicyV1::EnableDispatchTimestamps,
            "packets_per_forward":COUNT, "max_forward_frames":MAX_FRAMES,
            "max_frame_bytes":MAX_FRAME_BYTES, "max_file_bytes":MAX_FILE_BYTES,
            "host_diagnostics_enabled":true, "host_elapsed_is_gpu_time":false,
            "clock_accuracy_qualified":false, "performance_qualified":false,
            "interval_scope":"AMD dispatch packet processing, not pure instruction body",
            "host_overhead":"clock/snapshot work is inside dispatch elapsed; file writes before reply are outside dispatch elapsed but inside command latency"
        }))?;
        Ok(writer)
    }

    fn check_identity(&mut self) -> Result<()> {
        let parent = self.parent.metadata().map_err(explain)?;
        let file = self.file.metadata().map_err(explain)?;
        if !parent.is_dir()
            || parent.uid() != self.uid
            || parent.mode() & 0o022 != 0
            || parent.nlink() == 0
            || (parent.dev(), parent.ino()) != self.parent_identity
            || !file.is_file()
            || file.uid() != self.uid
            || file.mode() & 0o777 != 0o600
            || file.nlink() != 1
            || (file.dev(), file.ino()) != self.file_identity
            || file.len() != self.written
            || self.file.stream_position().map_err(explain)? != self.written
        {
            return Err("diagnostic descriptor identity, extent or position changed".into());
        }
        // SAFETY: fstatat writes one initialized stat on success; the retained
        // parent and basename bind this lookup to the original directory.
        let mut status = std::mem::MaybeUninit::<libc::stat>::uninit();
        if unsafe {
            libc::fstatat(
                self.parent.as_raw_fd(),
                self.basename.as_ptr(),
                status.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } != 0
        {
            return Err(explain(std::io::Error::last_os_error()));
        }
        // SAFETY: successful fstatat initialized the stat.
        let status = unsafe { status.assume_init() };
        if (status.st_dev, status.st_ino) != self.file_identity
            || status.st_mode & libc::S_IFMT != libc::S_IFREG
        {
            return Err("diagnostic directory entry changed".into());
        }
        Ok(())
    }

    fn reserve_frame(&mut self) -> Result<()> {
        self.check_identity()?;
        if self.closed
            || self.frames >= MAX_FRAMES
            || self
                .written
                .checked_add(MAX_FRAME_BYTES as u64 + 65536)
                .is_none_or(|end| end > MAX_FILE_BYTES)
        {
            return Err("diagnostic forward/file budget exhausted".into());
        }
        Ok(())
    }

    fn write_record(&mut self, record: &impl Serialize) -> Result<()> {
        self.check_identity()?;
        if self.closed {
            return Err("diagnostic file already closed".into());
        }
        let mut raw = BoundedJson(Vec::new());
        serde_json::to_writer(&mut raw, record).map_err(explain)?;
        raw.write_all(b"\n").map_err(explain)?;
        let next = self
            .written
            .checked_add(raw.0.len() as u64)
            .filter(|value| *value <= MAX_FILE_BYTES)
            .ok_or("diagnostic file cap")?;
        self.file.write_all(&raw.0).map_err(explain)?;
        self.file.flush().map_err(explain)?;
        self.written = next;
        self.check_identity()
    }

    fn forward(&mut self, frame: &ForwardFrame) -> Result<()> {
        self.reserve_frame()?;
        if frame.generation
            != self
                .frames
                .checked_add(1)
                .ok_or("diagnostic frame counter")?
        {
            return Err("diagnostic frame generation changed".into());
        }
        self.write_record(frame)?;
        self.frames += 1;
        Ok(())
    }

    fn close(&mut self, last_next: u64) -> Result<()> {
        self.write_record(
            &serde_json::json!({"schema":"NativeFullForwardTimestampClosedV1", "authority":"none",
            "forward_frames":self.frames, "last_completed_packet_frontier":last_next,
            "clean_worker_close":true, "performance_qualified":false}),
        )?;
        self.closed = true;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct StageBinding {
    pub(super) kernel_id: u64,
    pub(super) kernel_symbol: String,
    pub(super) kernel_sha256: [u8; 32],
    pub(super) grid: [u32; 3],
    pub(super) workgroup: [u16; 3],
    pub(super) kernarg_bytes: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct BatchIdentity {
    pub(super) unique: u64,
    pub(super) gpu: u32,
    pub(super) epoch: u64,
    pub(super) first: u64,
    pub(super) next: u64,
    pub(super) generation: u64,
    pub(super) signal_handle: u64,
    pub(super) signal_va: u64,
}

#[derive(Serialize)]
struct PacketObservation {
    slot: u32,
    packet_id: u64,
    signal_generation: u64,
    acquired_value: i64,
    stage: StageBinding,
    signal_snapshot: [[u8; 32]; 2],
    correlated: Option<DispatchCorrelatedIntervalV1>,
}

#[derive(Serialize)]
struct ForwardFrame {
    schema: &'static str,
    authority: &'static str,
    device_unique_id: u64,
    gpu_id: u32,
    queue_epoch: u64,
    generation: u64,
    first_packet_id: u64,
    next_packet_id: u64,
    queue_profiling_enabled: bool,
    queue_properties: u32,
    packets_per_forward: usize,
    payload_sha256: [u8; 32],
    before: DispatchClockSampleV1,
    after: DispatchClockSampleV1,
    packets: Vec<PacketObservation>,
    host_diagnostics_enabled: bool,
    gpu_overlap_measured: bool,
    performance_qualified: bool,
}

pub(super) struct FullForwardTimestampOwner {
    pub(super) policy: AmdQueueProfilingPolicyV1,
    unique: u64,
    writer: DiagnosticFile,
    generation: u64,
    active: Option<BatchIdentity>,
    staged: usize,
    bindings: Vec<StageBinding>,
    payload_sha256: [u8; 32],
    before: Option<DispatchClockSampleV1>,
    ready: Option<ForwardFrame>,
    last_next: u64,
}

impl FullForwardTimestampOwner {
    pub(super) fn open(
        path: &Path,
        unique: u64,
        policy: AmdQueueProfilingPolicyV1,
    ) -> Result<Self> {
        let writer = DiagnosticFile::open(path, unique, policy)?;
        Ok(Self {
            policy,
            unique,
            writer,
            generation: 0,
            active: None,
            staged: 0,
            bindings: Vec::new(),
            payload_sha256: [0; 32],
            before: None,
            ready: None,
            last_next: 0,
        })
    }

    pub(super) fn begin(
        &mut self,
        mut identity: BatchIdentity,
        payload_sha256: [u8; 32],
        bindings: Vec<StageBinding>,
    ) -> Result<()> {
        self.writer.reserve_frame()?;
        if self.active.is_some()
            || self.ready.is_some()
            || self.before.is_some()
            || bindings.len() != COUNT
            || identity.unique != self.unique
            || identity.first != self.last_next
            || identity.first.checked_add(COUNT as u64) != Some(identity.next)
        {
            return Err("full-forward timestamp staging identity or phase".into());
        }
        for binding in &bindings {
            if binding.kernel_symbol.is_empty()
                || binding.kernel_symbol.len() > 256
                || binding.kernarg_bytes > MAX_KERNARG_BYTES_V1
            {
                return Err("full-forward timestamp binding bound".into());
            }
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or("full-forward timestamp generation exhausted")?;
        identity.generation = self.generation;
        self.active = Some(identity);
        self.staged = 0;
        self.bindings = bindings;
        self.payload_sha256 = payload_sha256;
        Ok(())
    }

    pub(super) fn stage_slot(&mut self, slot: usize) -> Result<()> {
        if self.active.is_none()
            || self.before.is_some()
            || self.ready.is_some()
            || slot != self.staged
            || slot >= COUNT
        {
            return Err("full-forward timestamp signal staging order".into());
        }
        self.staged += 1;
        Ok(())
    }

    pub(super) fn identity(&self) -> Result<BatchIdentity> {
        self.active
            .ok_or("full-forward timestamp identity absent".into())
    }

    pub(super) fn published(
        &mut self,
        identity: BatchIdentity,
        before: DispatchClockSampleV1,
    ) -> Result<()> {
        if self.active != Some(identity)
            || self.staged != COUNT
            || self.before.is_some()
            || self.ready.is_some()
            || before.gpu_id != identity.gpu
        {
            return Err("full-forward timestamp publication identity or phase".into());
        }
        clock_value(before)?;
        self.before = Some(before);
        Ok(())
    }

    pub(super) fn capture(
        &mut self,
        identity: BatchIdentity,
        snapshots: Vec<[u8; 64]>,
        after: DispatchClockSampleV1,
    ) -> Result<()> {
        if self.active != Some(identity)
            || self.ready.is_some()
            || snapshots.len() != COUNT
            || self.bindings.len() != COUNT
            || after.gpu_id != identity.gpu
        {
            return Err("full-forward timestamp capture identity or extent".into());
        }
        let before = self
            .before
            .ok_or("full-forward timestamp before-clock absent")?;
        let bracket = GpuSystemClockBracketV1::new(clock_value(before)?, clock_value(after)?)
            .map_err(explain)?;
        let enabled = self.policy == AmdQueueProfilingPolicyV1::EnableDispatchTimestamps;
        let mut packets = Vec::with_capacity(COUNT);
        for (slot, (raw, stage)) in snapshots.into_iter().zip(&self.bindings).enumerate() {
            let correlated = if enabled {
                let ticks = parse_amd_busy_dispatch_timestamp_snapshot_v1(&raw).map_err(explain)?;
                let interval = bracket.interpolate(ticks).map_err(explain)?;
                Some(DispatchCorrelatedIntervalV1 {
                    start_system_ticks: interval.start_system_ticks(),
                    end_system_ticks: interval.end_system_ticks(),
                    system_frequency_hz: interval.system_frequency_hz(),
                    duration_ns_floor: interval.duration_ns_floor(),
                })
            } else {
                let mut expected = [0; 64];
                expected[0] = 1;
                if raw != expected {
                    return Err("full-forward profile-off nonzero or malformed signal".into());
                }
                None
            };
            packets.push(PacketObservation {
                slot: slot as u32,
                packet_id: identity
                    .first
                    .checked_add(slot as u64)
                    .ok_or("timestamp packet overflow")?,
                signal_generation: identity.generation,
                acquired_value: 0,
                stage: stage.clone(),
                signal_snapshot: [
                    raw[..32].try_into().map_err(explain)?,
                    raw[32..].try_into().map_err(explain)?,
                ],
                correlated,
            });
        }
        self.ready = Some(ForwardFrame {
            schema: "NativeFullForwardTimestampFrameV1",
            authority: "none",
            device_unique_id: identity.unique,
            gpu_id: identity.gpu,
            queue_epoch: identity.epoch,
            generation: identity.generation,
            first_packet_id: identity.first,
            next_packet_id: identity.next,
            queue_profiling_enabled: enabled,
            queue_properties: self.policy.properties(0),
            packets_per_forward: COUNT,
            payload_sha256: self.payload_sha256,
            before,
            after,
            packets,
            host_diagnostics_enabled: true,
            gpu_overlap_measured: false,
            performance_qualified: false,
        });
        Ok(())
    }

    pub(super) fn flush_completed(&mut self, identity: BatchIdentity) -> Result<()> {
        if self.active != Some(identity) {
            return Err("full-forward timestamp flush identity".into());
        }
        let frame = self
            .ready
            .as_ref()
            .ok_or("full-forward timestamps not validated")?;
        self.writer.forward(frame)?;
        self.last_next = identity.next;
        self.active = None;
        self.before = None;
        self.ready = None;
        self.bindings.clear();
        Ok(())
    }

    pub(super) fn close(&mut self) -> Result<()> {
        if self.active.is_some() || self.ready.is_some() || self.before.is_some() {
            return Err("full-forward diagnostic incomplete at close".into());
        }
        self.writer.close(self.last_next)
    }
}

fn clock_value(value: DispatchClockSampleV1) -> Result<GpuSystemClockSampleV1> {
    GpuSystemClockSampleV1::new(
        value.gpu_ticks,
        value.system_ticks,
        value.system_frequency_hz,
    )
    .map_err(explain)
}

pub(super) fn clock_sample(
    value: crate::KfdClockCorrelationObservationV1,
) -> DispatchClockSampleV1 {
    DispatchClockSampleV1 {
        gpu_ticks: value.gpu_clock_counter(),
        cpu_ticks: value.cpu_clock_counter(),
        system_ticks: value.system_clock_counter(),
        system_frequency_hz: value.system_clock_frequency_hz(),
        gpu_id: value.gpu_id(),
    }
}

pub(super) fn admit_command(enabled: bool, command: &CommandV1) -> Result<()> {
    if enabled
        && matches!(
            command,
            CommandV1::Dispatch { .. }
                | CommandV1::DispatchSequence { .. }
                | CommandV1::DispatchOrderedBatch { .. }
                | CommandV1::RolloverQueue { .. }
                | CommandV1::TakeDispatchTimestampObservation { .. }
                | CommandV1::PerformanceSnapshot
                | CommandV1::ConfigurePerformance { profile: true, .. }
        )
    {
        return Err("full-forward timestamp mode admits exact-616 transactions only".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct Directory(std::path::PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "fe2o3-full-ts-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            Self(path.canonicalize().unwrap())
        }
        fn path(&self) -> std::path::PathBuf {
            self.0.join("frames.ndjson")
        }
        fn owner(&self, policy: AmdQueueProfilingPolicyV1) -> FullForwardTimestampOwner {
            FullForwardTimestampOwner::open(&self.path(), 123, policy).unwrap()
        }
        fn records(&self) -> Vec<serde_json::Value> {
            std::fs::read_to_string(self.path())
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect()
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn identity(first: u64) -> BatchIdentity {
        BatchIdentity {
            unique: 123,
            gpu: 7,
            epoch: 0,
            first,
            next: first + COUNT as u64,
            generation: 0,
            signal_handle: 40,
            signal_va: 0x10000,
        }
    }
    fn bindings() -> Vec<StageBinding> {
        vec![
            StageBinding {
                kernel_id: 1,
                kernel_symbol: "test_kernel".into(),
                kernel_sha256: [5; 32],
                grid: [4096, 1, 1],
                workgroup: [64, 1, 1],
                kernarg_bytes: 312
            };
            COUNT
        ]
    }
    fn clock(ticks: u64) -> DispatchClockSampleV1 {
        DispatchClockSampleV1 {
            gpu_ticks: ticks,
            cpu_ticks: ticks + 1,
            system_ticks: ticks * 10,
            system_frequency_hz: 1_000_000_000,
            gpu_id: 7,
        }
    }
    fn snapshots(on: bool) -> Vec<[u8; 64]> {
        let mut raw = [0; 64];
        raw[0] = 1;
        if on {
            raw[32..40].copy_from_slice(&200u64.to_le_bytes());
            raw[40..48].copy_from_slice(&400u64.to_le_bytes());
        }
        vec![raw; COUNT]
    }
    fn publish(owner: &mut FullForwardTimestampOwner, first: u64) -> BatchIdentity {
        owner.begin(identity(first), [6; 32], bindings()).unwrap();
        for slot in 0..COUNT {
            owner.stage_slot(slot).unwrap();
        }
        let id = owner.identity().unwrap();
        owner.published(id, clock(100)).unwrap();
        id
    }

    #[test]
    fn full_timestamp_three_generations_preserve_every_slot_and_close_only_after_flush() {
        for policy in [
            AmdQueueProfilingPolicyV1::Preserve,
            AmdQueueProfilingPolicyV1::EnableDispatchTimestamps,
        ] {
            let directory = Directory::new();
            let mut owner = directory.owner(policy);
            for generation in 1..=3 {
                let id = publish(&mut owner, (generation - 1) * COUNT as u64);
                assert_eq!(id.generation, generation);
                assert!(owner.close().is_err());
                assert!(owner.flush_completed(id).is_err());
                owner
                    .capture(
                        id,
                        snapshots(policy == AmdQueueProfilingPolicyV1::EnableDispatchTimestamps),
                        clock(1000),
                    )
                    .unwrap();
                assert!(owner.close().is_err());
                owner.flush_completed(id).unwrap();
                assert!(owner.flush_completed(id).is_err());
            }
            owner.close().unwrap();
            assert!(owner.close().is_err());
            let records = directory.records();
            assert_eq!(records.len(), 5);
            assert_eq!(records[0]["host_elapsed_is_gpu_time"], false);
            for (index, frame) in records[1..4].iter().enumerate() {
                assert_eq!(frame["generation"], index + 1);
                assert_eq!(frame["packets"].as_array().unwrap().len(), COUNT);
                for (slot, packet) in frame["packets"].as_array().unwrap().iter().enumerate() {
                    assert_eq!(packet["slot"], slot);
                    assert_eq!(packet["packet_id"], index * COUNT + slot);
                    assert_eq!(packet["signal_generation"], index + 1);
                    assert_eq!(packet["acquired_value"], 0);
                    assert_eq!(packet["signal_snapshot"].as_array().unwrap().len(), 2);
                    if policy == AmdQueueProfilingPolicyV1::EnableDispatchTimestamps {
                        assert_eq!(packet["correlated"]["duration_ns_floor"], 2000);
                    } else {
                        assert!(packet["correlated"].is_null());
                    }
                }
            }
            assert_eq!(records[4]["forward_frames"], 3);
            assert_eq!(records[4]["last_completed_packet_frontier"], 1848);
        }
    }

    #[test]
    fn full_timestamp_phase_and_substituted_identity_fail_closed() {
        let directory = Directory::new();
        let mut owner = directory.owner(AmdQueueProfilingPolicyV1::EnableDispatchTimestamps);
        assert!(owner.stage_slot(0).is_err());
        assert!(owner.published(identity(0), clock(100)).is_err());
        assert!(owner.begin(identity(616), [0; 32], bindings()).is_err());
        owner.begin(identity(0), [0; 32], bindings()).unwrap();
        let id = owner.identity().unwrap();
        assert!(owner.published(id, clock(100)).is_err());
        assert!(owner.stage_slot(1).is_err());
        assert!(owner.capture(id, snapshots(true), clock(1000)).is_err());
        for slot in 0..COUNT {
            owner.stage_slot(slot).unwrap();
        }
        assert!(owner.stage_slot(COUNT).is_err());
        for field in 0..9 {
            let mut other = id;
            match field {
                0 => other.unique += 1,
                1 => other.gpu += 1,
                2 => other.epoch += 1,
                3 => other.first += 1,
                4 => other.next += 1,
                5 => other.generation += 1,
                6 => other.signal_handle += 1,
                7 => other.signal_va += 64,
                _ => other.generation = 0,
            }
            assert!(owner.published(other, clock(100)).is_err());
            assert!(owner.capture(other, snapshots(true), clock(1000)).is_err());
            assert!(owner.flush_completed(other).is_err());
        }
        owner.published(id, clock(100)).unwrap();
        assert!(owner.published(id, clock(100)).is_err());
        assert!(owner.begin(identity(0), [0; 32], bindings()).is_err());
        assert!(
            owner
                .capture(id, snapshots(true)[..615].to_vec(), clock(1000))
                .is_err()
        );
        assert!(owner.ready.is_none());
        assert_eq!(directory.records().len(), 1);
    }

    #[test]
    fn full_timestamp_all_snapshots_must_pass_without_partial_frame() {
        for enabled in [false, true] {
            for (slot, offset, value) in [
                (0, 0, 0),
                (615, 8, 1),
                (300, 16, 1),
                (615, 32, 0),
                (615, 40, 0),
                (10, 56, 1),
            ] {
                let directory = Directory::new();
                let policy = if enabled {
                    AmdQueueProfilingPolicyV1::EnableDispatchTimestamps
                } else {
                    AmdQueueProfilingPolicyV1::Preserve
                };
                let mut owner = directory.owner(policy);
                let id = publish(&mut owner, 0);
                let mut raw = snapshots(enabled);
                if !enabled && (offset == 32 || offset == 40) {
                    raw[slot][offset] = 1;
                } else {
                    raw[slot][offset..offset + 8].copy_from_slice(&(value as u64).to_le_bytes());
                }
                assert!(
                    owner.capture(id, raw, clock(1000)).is_err(),
                    "{enabled} {slot} {offset}"
                );
                assert!(owner.ready.is_none());
                assert!(owner.close().is_err());
                assert_eq!(directory.records().len(), 1);
            }
        }
    }

    #[test]
    fn full_timestamp_clock_bounds_frequency_and_gpu_are_checked() {
        let directory = Directory::new();
        let mut owner = directory.owner(AmdQueueProfilingPolicyV1::EnableDispatchTimestamps);
        let id = publish(&mut owner, 0);
        for changed in 0..7 {
            let mut after = clock(1000);
            match changed {
                0 => after.gpu_id += 1,
                1 => after.gpu_ticks = 100,
                2 => after.system_ticks = 1000,
                3 => after.system_frequency_hz = 0,
                4 => after.system_frequency_hz += 1,
                5 => after.gpu_ticks = 0,
                _ => after.system_ticks = 0,
            }
            assert!(owner.capture(id, snapshots(true), after).is_err());
        }
        for (start, end) in [(99u64, 400u64), (200, 1001), (400, 200), (0, 400)] {
            let mut raw = snapshots(true);
            raw[0][32..40].copy_from_slice(&start.to_le_bytes());
            raw[0][40..48].copy_from_slice(&end.to_le_bytes());
            assert!(owner.capture(id, raw, clock(1000)).is_err());
        }
        assert!(owner.ready.is_none());
    }

    #[test]
    fn full_timestamp_generation_and_binding_bounds_are_checked() {
        let directory = Directory::new();
        let mut owner = directory.owner(AmdQueueProfilingPolicyV1::Preserve);
        assert!(
            owner
                .begin(identity(0), [0; 32], bindings()[..615].to_vec())
                .is_err()
        );
        for (name, size) in [
            (String::new(), 312),
            ("a".repeat(257), 312),
            ("x".into(), MAX_KERNARG_BYTES_V1 + 1),
        ] {
            let mut bindings = bindings();
            bindings[0].kernel_symbol = name;
            bindings[0].kernarg_bytes = size;
            assert!(owner.begin(identity(0), [0; 32], bindings).is_err());
        }
        owner.generation = u64::MAX;
        assert!(owner.begin(identity(0), [0; 32], bindings()).is_err());
        assert!(owner.active.is_none());
    }

    #[test]
    fn full_timestamp_output_is_create_new_private_and_rejects_symlinks() {
        let directory = Directory::new();
        let _owner = directory.owner(AmdQueueProfilingPolicyV1::Preserve);
        assert_eq!(
            std::fs::metadata(directory.path()).unwrap().mode() & 0o777,
            0o600
        );
        assert!(
            FullForwardTimestampOwner::open(
                &directory.path(),
                123,
                AmdQueueProfilingPolicyV1::Preserve
            )
            .is_err()
        );
        let symlink_path = directory.0.join("link");
        symlink(directory.path(), &symlink_path).unwrap();
        assert!(
            DiagnosticFile::open(&symlink_path, 123, AmdQueueProfilingPolicyV1::Preserve).is_err()
        );
        let alias = directory.0.join("alias");
        symlink(&directory.0, &alias).unwrap();
        assert!(
            DiagnosticFile::open(&alias.join("new"), 123, AmdQueueProfilingPolicyV1::Preserve)
                .is_err()
        );
        for name in [".hidden", "bad name", "../x"] {
            assert!(
                DiagnosticFile::open(
                    &directory.0.join(name),
                    123,
                    AmdQueueProfilingPolicyV1::Preserve
                )
                .is_err()
            );
        }
        assert!(
            DiagnosticFile::open(
                Path::new("relative"),
                123,
                AmdQueueProfilingPolicyV1::Preserve
            )
            .is_err()
        );
    }

    #[test]
    fn full_timestamp_output_rejects_mutation_and_hardlinks() {
        for mutation in 0..5 {
            let directory = Directory::new();
            let mut owner = directory.owner(AmdQueueProfilingPolicyV1::Preserve);
            match mutation {
                0 => {
                    std::fs::hard_link(directory.path(), directory.0.join("hardlink")).unwrap();
                }
                1 => {
                    OpenOptions::new()
                        .write(true)
                        .open(directory.path())
                        .unwrap()
                        .set_len(0)
                        .unwrap();
                }
                2 => {
                    std::fs::set_permissions(
                        directory.path(),
                        std::fs::Permissions::from_mode(0o644),
                    )
                    .unwrap();
                }
                3 => {
                    std::fs::rename(directory.path(), directory.0.join("old")).unwrap();
                    File::create(directory.path()).unwrap();
                }
                _ => {
                    std::fs::set_permissions(&directory.0, std::fs::Permissions::from_mode(0o777))
                        .unwrap();
                }
            }
            assert!(owner.begin(identity(0), [0; 32], bindings()).is_err());
            assert!(owner.close().is_err());
        }
    }

    #[test]
    fn full_timestamp_retained_parent_fd_does_not_follow_replaced_absolute_parent() {
        let mut directory = Directory::new();
        let original = directory.0.clone();
        let moved = original.with_extension("moved");
        let mut owner = directory.owner(AmdQueueProfilingPolicyV1::Preserve);
        std::fs::rename(&original, &moved).unwrap();
        directory.0 = moved;
        std::fs::create_dir(&original).unwrap();
        owner.close().unwrap();
        assert!(!original.join("frames.ndjson").exists());
        assert_eq!(directory.records().len(), 2);
        std::fs::remove_dir(original).unwrap();
    }

    #[test]
    fn full_timestamp_file_and_frame_budgets_are_finite() {
        let directory = Directory::new();
        let mut owner = directory.owner(AmdQueueProfilingPolicyV1::Preserve);
        owner.writer.frames = MAX_FRAMES;
        assert!(owner.begin(identity(0), [0; 32], bindings()).is_err());
        let mut bounded = BoundedJson(vec![0; MAX_FRAME_BYTES - 1]);
        assert!(bounded.write_all(&[0, 0]).is_err());
        assert_eq!(bounded.0.len(), MAX_FRAME_BYTES - 1);
        bounded.write_all(&[0]).unwrap();
        assert!(bounded.write_all(&[0]).is_err());
        owner.writer.frames = 0;
        owner.writer.written = MAX_FILE_BYTES;
        owner.writer.file.set_len(MAX_FILE_BYTES).unwrap();
        owner
            .writer
            .file
            .seek(std::io::SeekFrom::Start(MAX_FILE_BYTES))
            .unwrap();
        assert!(owner.begin(identity(0), [0; 32], bindings()).is_err());
        assert!(owner.close().is_err());
    }

    #[test]
    fn full_timestamp_mode_admits_only_full_forward_and_profile_false() {
        let rejected = [
            CommandV1::PerformanceSnapshot,
            CommandV1::RolloverQueue {
                expected_epoch: 0,
                expected_completed_packets: 0,
            },
            CommandV1::ConfigurePerformance {
                cache_kernel_admission: true,
                operational_currentness: true,
                profile: true,
            },
            CommandV1::DispatchSequence {
                dispatches: Vec::new(),
            },
            CommandV1::DispatchOrderedBatch {
                dispatches: Vec::new(),
                timeout_ms: 1,
            },
            CommandV1::TakeDispatchTimestampObservation {
                expected_queue_epoch: 0,
                expected_packet_id: 0,
                expected_signal_generation: 1,
            },
        ];
        for command in rejected {
            assert!(admit_command(true, &command).is_err());
            assert!(admit_command(false, &command).is_ok());
        }
        for command in [
            CommandV1::Close,
            CommandV1::ConfigurePerformance {
                cache_kernel_admission: true,
                operational_currentness: true,
                profile: false,
            },
            CommandV1::DispatchFullForward {
                dispatch_count: 616,
                plan_bytes: 1,
                kernarg_bytes: 1,
                timeout_ms: 1,
            },
        ] {
            assert!(admit_command(true, &command).is_ok());
        }
    }
}
