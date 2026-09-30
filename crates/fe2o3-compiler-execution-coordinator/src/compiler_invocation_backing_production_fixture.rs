//! Fixed-origin admission only. No fixture writes the installed policy or runtime.
use super::*;
use fe2o3_build_authority::COMPILER_RUNTIME_MANIFEST_MAX_ENTRIES_V1 as MAX_ENTRIES;
use fe2o3_compiler_closure_capability::{
    ApprovedCompilerPolicyV2 as Approval, COMPILER_RUNTIME_ROOT_V1 as ROOT,
};
use std::{
    fs,
    os::fd::{AsFd, AsRawFd, RawFd},
    os::unix::fs::MetadataExt,
};

// Finite logical test limits, not physical allocations or production cap changes.
// The schema permits 4 GiB of runtime bytes; multiple retained copies coexist.
pub(super) const WORK: usize = 1024 * 1024 * 1024 * 1024;
pub(super) const STORAGE: usize = 16 * 1024 * 1024 * 1024;
pub(super) const HARNESS: usize = 1024 * 1024;

pub(super) fn account(operation: impl FnOnce(&mut Budget<'_>)) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    // Includes diagnostic arrays, temporary paths/descriptors, inert descriptor
    // construction and overlapping test assertions, outside all owner charges.
    budget.reserve_storage(HARNESS).unwrap();
    budget.charge_work(37).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let address = &budget as *const Budget<'_>;
    operation(&mut budget);
    assert!(ledger == budget.work_ledger_identity_v1());
    assert_eq!(address, &budget as *const Budget<'_>);
    assert_eq!(budget.storage(), HARNESS);
    assert_eq!(budget.storage_limit(), STORAGE);
}

pub(super) fn history(b: &Budget<'_>) -> (usize, usize, Option<usize>, Option<usize>) {
    (
        b.work(),
        b.peak_storage(),
        b.failed_work(),
        b.failed_storage(),
    )
}

pub(super) struct Inputs {
    runtime: Runtime,
    capture: Capture,
    output: Output,
    pub(super) storage: usize,
}
impl Inputs {
    pub(super) fn prepare(
        self,
        b: &mut Budget<'_>,
    ) -> Result<(CompilerInvocationBacking, CompilerInvocationBackingCharge)> {
        CompilerInvocationBacking::prepare(self.runtime, self.capture, self.output, b)
    }
}

// Diagnostic FD counts, not another inventory representation or source owner.
#[derive(Clone, Copy)]
struct InodeCount {
    identity: (u64, u64),
    baseline: usize,
    multiplicity: usize,
}

struct Census {
    inodes: [Option<InodeCount>; MAX_ENTRIES],
    entries: usize,
}
impl Census {
    fn new() -> Self {
        Self {
            inodes: [None; MAX_ENTRIES],
            entries: 0,
        }
    }

    fn record(&mut self, id: (u64, u64), references: impl FnOnce((u64, u64)) -> usize) {
        assert!(
            self.entries < MAX_ENTRIES,
            "fixture manifest census exceeds its bound"
        );
        // Aliased paths still own one descriptor per entry; take the baseline
        // only once per inode and preserve the complete entry multiplicity.
        if let Some(count) = self.inodes.iter_mut().flatten().find(|c| c.identity == id) {
            let multiplicity = count.multiplicity.checked_add(1).unwrap();
            assert!(
                multiplicity <= count.baseline,
                "missing original runtime descriptors"
            );
            count.multiplicity = multiplicity;
        } else {
            let baseline = references(id);
            assert!(
                baseline > 0,
                "admitted runtime must retain every code descriptor"
            );
            let slot = self.inodes.iter_mut().find(|slot| slot.is_none()).unwrap();
            *slot = Some(InodeCount {
                identity: id,
                baseline,
                multiplicity: 1,
            });
        }
        self.entries = self.entries.checked_add(1).unwrap();
    }

    fn check(&self, live: bool, references: impl Fn((u64, u64)) -> usize) {
        for count in self.inodes.iter().flatten() {
            let expected = if live {
                count.baseline.checked_add(count.multiplicity)
            } else {
                count.baseline.checked_sub(count.multiplicity)
            }
            .unwrap();
            assert_eq!(references(count.identity), expected);
        }
    }
}

pub(super) struct Witness {
    counts: Census,
    invocation: File,
    received_fd: RawFd,
    argv0: *const u8,
    cwd: *const u8,
    output: File,
    _directory: tempfile::TempDir,
}
impl Witness {
    pub(super) fn assert_live(&self) {
        self.counts.check(true, references); // Original runtime plus complete transfers.
        assert_eq!(references(identity(&self.output.metadata().unwrap())), 2);
        let invocation = identity(&self.invocation.metadata().unwrap());
        assert_eq!(references(invocation), 2);
        assert_eq!(
            identity(&fs::metadata(format!("/proc/self/fd/{}", self.received_fd)).unwrap()),
            invocation
        );
    }

    pub(super) fn assert_capture_retained(&self, owner: &CompilerInvocationBacking) {
        self.assert_live();
        let descriptor = owner.descriptor();
        assert!(std::ptr::eq(descriptor, owner.capture.descriptor()));
        assert_eq!(
            descriptor.rustc().argv().next().unwrap().as_ptr(),
            self.argv0
        );
        assert_eq!(descriptor.rustc().working_directory().as_ptr(), self.cwd);
        let staged = owner.invocation();
        assert_eq!(staged.arguments().len(), descriptor.rustc().argv().len());
        for (actual, expected) in staged.arguments().iter().zip(descriptor.rustc().argv()) {
            assert_eq!(actual.as_bytes(), expected.as_bytes());
        }
        assert_eq!(
            staged.environment().len(),
            descriptor.compile_environment().entries().len()
        );
        for (actual, expected) in staged
            .environment()
            .iter()
            .zip(descriptor.compile_environment().entries())
        {
            assert_eq!(
                actual.as_bytes(),
                format!("{}={}", expected.key(), expected.value()).as_bytes()
            );
        }
        assert_eq!(
            staged.working_directory().to_bytes(),
            descriptor.rustc().working_directory().as_bytes()
        );
    }

    pub(super) fn invocation_mode(&self, mode: rustix::fs::Mode) {
        // Only this fixture-created sealed memfd changes; installed code and
        // policy remain untouched. This alias observes the retained real inode.
        rustix::fs::fchmod(&self.invocation, mode).unwrap();
    }

    pub(super) fn assert_dropped(&self) {
        self.counts.check(false, references); // Neither originals nor transfers survive.
        assert_eq!(references(identity(&self.output.metadata().unwrap())), 1);
        assert_eq!(
            references(identity(&self.invocation.metadata().unwrap())),
            1
        );
    }
}

fn identity(metadata: &fs::Metadata) -> (u64, u64) {
    (metadata.dev(), metadata.ino())
}

fn references(id: (u64, u64)) -> usize {
    // Run serially in the isolated fixture process: the census is not authority.
    let mut count = 0;
    for (index, entry) in fs::read_dir("/proc/self/fd").unwrap().enumerate() {
        assert!(index < 4096, "fixture descriptor census exceeds its bound");
        if let Ok(metadata) = fs::metadata(entry.unwrap().path()) {
            count += usize::from(identity(&metadata) == id);
        }
    }
    count
}

pub(super) fn admit(b: &mut Budget<'_>, wrong_closure: bool) -> (Inputs, Witness) {
    let floor = b.storage();
    let (approval, charge) = Approval::from_production_policy(b)
        .expect("requires actual immutable policy-v2 and matching fixed client-profile-v3");
    b.reserve_storage(charge.retained_storage()).unwrap();
    let (runtime, charge) = Runtime::from_production_runtime(approval, b)
        .expect("requires actual fixed-origin immutable compiler-runtime-manifest-v1 and code");
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(b.storage(), floor + runtime.required_retained_storage());

    let mut counts = Census::new();
    for entry in runtime.manifest().entries() {
        let metadata = fs::metadata(std::path::Path::new(ROOT).join(entry.path)).unwrap();
        counts.record(identity(&metadata), references);
    }
    // Inert local fixture, not authenticated cargo capture. Only admission of
    // the installed policy/runtime above carries approval. Hand the preparation
    // path an actual natively received sealed owner, never a plain descriptor.
    let source = Capture::create(descriptor(&runtime, wrong_closure)).unwrap();
    let source_storage = source.native_retained_storage().unwrap();
    b.reserve_storage(source_storage).unwrap();
    let (received, file_charge) = source.try_clone_for_transfer_native(b).unwrap();
    b.reserve_storage(file_charge.additional_storage()).unwrap();
    let received_fd = received.as_raw_fd();
    let invocation = received.try_clone().unwrap(); // Diagnostic alias funded by HARNESS.
    let (capture, growth) = Capture::from_file_native(received, b).unwrap();
    b.reserve_storage(growth.additional_storage()).unwrap();
    let capture_storage = capture.native_retained_storage().unwrap();
    assert_eq!(
        capture_storage,
        file_charge.additional_storage() + growth.additional_storage()
    );
    drop(source);
    b.release_storage(source_storage).unwrap();
    let argv0 = capture.descriptor().rustc().argv().next().unwrap().as_ptr();
    let cwd = capture.descriptor().rustc().working_directory().as_ptr();
    let directory = tempfile::tempdir().unwrap();
    let output = File::open(directory.path()).unwrap();
    assert_eq!(references(identity(&output.metadata().unwrap())), 1);
    let retained_output = Output::capture(
        output.as_fd(),
        identity(&output.metadata().unwrap()),
        Output::CHILD_PATH,
        b,
    )
    .unwrap();
    b.reserve_storage(Output::STORAGE).unwrap();
    let storage = runtime.required_retained_storage() + capture_storage + Output::STORAGE;
    assert_eq!(b.storage(), floor + storage);
    (
        Inputs {
            runtime,
            capture,
            output: retained_output,
            storage,
        },
        Witness {
            counts,
            invocation,
            received_fd,
            argv0,
            cwd,
            output,
            _directory: directory,
        },
    )
}

fn descriptor(runtime: &Runtime, wrong_closure: bool) -> Descriptor {
    let original = runtime.manifest().compiler_closure();
    let closure = if wrong_closure {
        // Keep rustc/backend pins identical: reject a different FULL closure.
        let mut tree = original.rustc_runtime_tree_sha256();
        tree[0] ^= 1;
        if tree == [0; 32] {
            tree[1] = 1;
        }
        CompilerClosureV2::new(
            original.cargo_executable_sha256(),
            original.cargo_binding_trampoline_sha256(),
            original.cargo_fe2o3_binding_wrapper_sha256(),
            original.rustc_executable_sha256(),
            tree,
            original.codegen_backend_sha256(),
        )
        .unwrap()
    } else {
        original
    };
    let rustc = runtime
        .manifest()
        .entries()
        .find(|e| e.role == Role::Rustc)
        .unwrap();
    // Inert matching process data only, not authenticated cargo capture or exec.
    let unit = RustcUnitV2::new(
        "/workspace/project",
        vec![
            format!("{ROOT}/{}", rustc.path),
            "-Zcodegen-backend=/proc/./self/fd/198".into(),
        ],
    )
    .unwrap();
    let environment = CompileEnvironmentV2::from_child_environment([
        ("FE2O3_TARGET".into(), "gfx942:xnack-".into()),
        ("FE2O3_HSACO_DIR".into(), Output::CHILD_PATH.into()),
    ])
    .unwrap();
    Descriptor::new(
        RustcInvocationDescriptorV2::new(
            closure.rustc_executable_sha256(),
            closure.codegen_backend_sha256(),
            unit,
            environment,
        )
        .unwrap(),
        closure,
    )
    .unwrap()
}

#[test]
fn fd_census_preserves_shared_inode_multiplicity_and_terminal_baseline() {
    account(|_| {
        let mut counts = Census::new();
        let shared = (1, 2);
        let distinct = (1, 3);
        let mut baseline_reads = 0;
        for id in [shared, distinct, shared] {
            counts.record(id, |id| {
                baseline_reads += 1;
                if id == shared { 5 } else { 3 }
            });
        }
        assert_eq!(baseline_reads, 2);
        assert_eq!(counts.entries, 3);
        assert_eq!(counts.inodes.iter().flatten().count(), 2);
        counts.check(true, |id| if id == shared { 7 } else { 4 });
        counts.check(false, |id| if id == shared { 3 } else { 2 });
        // The former +/-1 assumption must fail for the shared inode in both states.
        assert!(
            catch_unwind(|| counts.check(true, |id| if id == shared { 6 } else { 4 })).is_err()
        );
        assert!(
            catch_unwind(|| counts.check(false, |id| if id == shared { 4 } else { 2 })).is_err()
        );
    });
}

#[test]
fn fd_census_enforces_manifest_entry_bound_even_for_shared_inodes() {
    account(|_| {
        assert!(2 * size_of::<Census>() < HARNESS);
        for shared in [false, true] {
            let mut counts = Census::new();
            let multiplicity = if shared { MAX_ENTRIES } else { 1 };
            for index in 0..MAX_ENTRIES {
                let inode = if shared {
                    0
                } else {
                    u64::try_from(index).unwrap()
                };
                counts.record((1, inode), |_| multiplicity);
            }
            assert_eq!(counts.entries, MAX_ENTRIES);
            counts.check(true, |_| 2 * multiplicity);
            counts.check(false, |_| 0);
            let mut queried = false;
            let refused = catch_unwind(AssertUnwindSafe(|| {
                counts.record((2, 0), |_| {
                    queried = true;
                    1
                });
            }));
            assert!(refused.is_err());
            assert!(
                !queried,
                "bound must reject before another descriptor census"
            );
            assert_eq!(counts.entries, MAX_ENTRIES);
            counts.check(true, |_| 2 * multiplicity);
            counts.check(false, |_| 0);
        }
    });
}
