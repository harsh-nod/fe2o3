use super::*;
use fe2o3_kfd_uapi::{KFD_MMAP_GPU_ID_HASH_SHIFT, KFD_MMAP_TYPE_DOORBELL, KFD_MMAP_TYPE_SHIFT};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Call {
    Preflight,
    Current,
    Step(Step),
}
#[derive(Default)]
struct Script {
    calls: Vec<Call>,
    fail_at: Option<usize>,
    quarantines: usize,
}
impl Script {
    fn call(&mut self, call: Call) -> QueueResult<()> {
        self.calls.push(call);
        if self.fail_at == Some(self.calls.len()) {
            Err("injected".into())
        } else {
            Ok(())
        }
    }
}
impl LifecycleBackend for Script {
    fn preflight(&mut self) -> QueueResult<()> {
        self.call(Call::Preflight)
    }
    fn currentness(&mut self) -> QueueResult<()> {
        self.call(Call::Current)
    }
    fn step(&mut self, step: Step) -> QueueResult<()> {
        self.call(Call::Step(step))
    }
    fn quarantine(&mut self) {
        self.quarantines += 1;
    }
}

#[test]
fn actual_coordinator_brackets_every_native_boundary_including_final_release() {
    let mut script = Script::default();
    run_lifecycle(&mut script).unwrap();
    let mut expected = vec![Call::Preflight];
    for step in STEPS {
        expected.extend([Call::Current, Call::Step(step), Call::Current]);
    }
    assert_eq!(script.calls, expected);
    assert_eq!(script.quarantines, 0);
    assert_eq!(
        &STEPS[6..],
        &[
            Step::Destroy,
            Step::DestroyEvent,
            Step::DisableRuntime,
            Step::ReleaseDoorbell,
            Step::ReleaseMemory,
            Step::Finish
        ]
    );
}

#[test]
fn actual_coordinator_stops_and_quarantines_at_every_step_and_both_fences() {
    let mut healthy = Script::default();
    run_lifecycle(&mut healthy).unwrap();
    for fail_at in 1..=healthy.calls.len() {
        let mut script = Script {
            fail_at: Some(fail_at),
            ..Script::default()
        };
        assert!(run_lifecycle(&mut script).is_err());
        assert_eq!(script.calls, healthy.calls[..fail_at]);
        assert_eq!(script.quarantines, 1);
    }
}

fn args() -> KfdIoctlCreateQueueArgs {
    create_args(0x10000, 0x20000, 0x30000, 0x10000000, 7).unwrap()
}
fn returned() -> KfdIoctlCreateQueueArgs {
    let mut value = args();
    value.queue_id = 5;
    value.doorbell_offset =
        (KFD_MMAP_TYPE_DOORBELL << KFD_MMAP_TYPE_SHIFT) | (7u64 << KFD_MMAP_GPU_ID_HASH_SHIFT) | 40;
    value
}

#[test]
fn create_arguments_use_exact_950_per_xcc_geometry_and_private_zero_counters() {
    let value = args();
    assert_eq!(value.ctx_save_restore_size, 0x15a3000);
    assert_eq!(value.ctl_stack_size, 0x3000);
    assert_eq!(value.ring_size, 4096);
    assert_eq!(value.eop_buffer_size, 4096);
    assert_eq!(value.write_pointer_address, 0x20038);
    assert_eq!(value.read_pointer_address, 0x20080);
    assert_eq!(value.queue_type, KFD_IOC_QUEUE_TYPE_COMPUTE_AQL);
    assert_eq!(value.queue_id, u32::MAX);
    assert_eq!(value.doorbell_offset, u64::MAX);
    assert_ne!(value.ctx_save_restore_size as usize, GEOMETRY.total_bytes());
}

#[test]
fn resource_argument_ranges_reject_alias_alignment_null_and_overflow() {
    for values in [
        [0, 0x20000, 0x30000, 0x10000000],
        [0x10001, 0x20000, 0x30000, 0x10000000],
        [0x10000, 0x10000, 0x30000, 0x10000000],
        [0x10000, 0x20000, 0x10000000, 0x10000000],
        [0x10000, 0x20000, 0x30000, u64::MAX - 4095],
    ] {
        assert!(create_args(values[0], values[1], values[2], values[3], 7).is_err());
    }
}

#[test]
fn create_output_checks_all_non_output_fields_and_exact_doorbell_target() {
    let plan = validate_create_output(args(), returned()).unwrap();
    assert_eq!(plan.queue_byte_offset, 40);
    for mutation in 0..9 {
        let mut value = returned();
        match mutation {
            0 => value.gpu_id ^= 1,
            1 => value.ring_base_address += 4096,
            2 => value.ctx_save_restore_size = 0x1621000,
            3 => value.ctl_stack_size = 4096,
            4 => value.pad = 1,
            5 => value.queue_id = u32::MAX,
            6 => value.doorbell_offset ^= 1u64 << KFD_MMAP_GPU_ID_HASH_SHIFT,
            7 => value.doorbell_offset += 4,
            _ => value.doorbell_offset = 0,
        }
        assert!(
            validate_create_output(args(), value).is_err(),
            "mutation {mutation}"
        );
    }
}
