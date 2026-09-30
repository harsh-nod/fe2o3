//! Fixed-origin admission only. No fixture writes the installed policy or runtime.
use super::*;
use fe2o3_build_authority::COMPILER_RUNTIME_MANIFEST_MAX_ENTRIES_V1 as MAX_ENTRIES;
use fe2o3_compiler_closure_capability::{
    ApprovedCompilerPolicyV2 as Approval, COMPILER_RUNTIME_ROOT_V1 as ROOT,
};
use std::{fs, os::fd::AsFd, os::unix::fs::MetadataExt};

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
    descriptor: Descriptor,
    output: Output,
    pub(super) storage: usize,
}
impl Inputs {
    pub(super) fn prepare(
        self,
        b: &mut Budget<'_>,
    ) -> Result<(CompilerInvocationBacking, CompilerInvocationBackingCharge)> {
        CompilerInvocationBacking::prepare(self.runtime, self.descriptor, self.output, b)
    }
}

// Diagnostic FD counts, not another inventory representation or source owner.
pub(super) struct Witness {
    counts: [Option<((u64, u64), usize)>; MAX_ENTRIES],
    output: File,
    _directory: tempfile::TempDir,
}
impl Witness {
    pub(super) fn assert_live(&self) {
        self.check(1); // The original runtime plus its complete transfer set.
        assert_eq!(references(identity(&self.output.metadata().unwrap())), 2);
    }

    pub(super) fn assert_dropped(&self) {
        self.check(-1); // Neither the original runtime nor its transfers survive.
        assert_eq!(references(identity(&self.output.metadata().unwrap())), 1);
    }

    fn check(&self, delta: isize) {
        for (id, count) in self.counts.iter().flatten() {
            assert_eq!(references(*id), count.checked_add_signed(delta).unwrap());
        }
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

    let mut counts = [None; MAX_ENTRIES];
    for (slot, entry) in counts.iter_mut().zip(runtime.manifest().entries()) {
        let metadata = fs::metadata(std::path::Path::new(ROOT).join(entry.path)).unwrap();
        let id = identity(&metadata);
        let count = references(id);
        assert!(
            count > 0,
            "admitted runtime must retain every code descriptor"
        );
        *slot = Some((id, count));
    }
    let descriptor = descriptor(&runtime, wrong_closure);
    let descriptor_storage = descriptor.retained_storage_bytes().unwrap();
    b.reserve_storage(descriptor_storage).unwrap();
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
    let storage = runtime.required_retained_storage() + descriptor_storage + Output::STORAGE;
    assert_eq!(b.storage(), floor + storage);
    (
        Inputs {
            runtime,
            descriptor,
            output: retained_output,
            storage,
        },
        Witness {
            counts,
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
