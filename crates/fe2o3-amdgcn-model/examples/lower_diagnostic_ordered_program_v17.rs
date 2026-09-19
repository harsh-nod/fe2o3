//! Bounded diagnostic V17 -> unchanged LLVM text observation, Linux only.
//!
//! Experimental observation helper; this is not a production artifact path.
//! Usage: lower_diagnostic_ordered_program_v17 INPUT.kir NEW_OUTPUT.ll
//!
//! The exact admitted immutable owner is borrowed by the existing typed emitter.
//! Raw bytes do not authenticate source or grant proof, final-artifact, loading,
//! hardware or production-resume authority. Canonical accounting and the existing
//! emitter's separate 16 MiB text bound are not a combined allocator/RSS cap.
//! The 64 KiB published-text limit is checked AFTER bounded text generation.
//!
//! Anonymous output is fully written, checked and synced before create-new
//! publication. Failed writes leave no named partial result. A post-link failure
//! may leave a complete file but produces no success observation. No pathname is
//! removed on failure. Filesystems lacking O_TMPFILE or authenticated procfs fail
//! closed, with no weaker fallback. Paths may be ordinary relative/absolute
//! paths, but openat2 refuses symlink/magiclink traversal in all components.

#![forbid(unsafe_code)]

#[cfg(target_os = "linux")]
mod linux {
    use std::ffi::OsString;
    use std::fs::File;
    use std::io::{self, Read, Seek, SeekFrom, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};

    use fe2o3_amdgcn_model::lower_canonical_v17_compiler_module_to_gfx942_xnack_minus_llvm_ir;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1,
        FunctionRole, OperationKind, VerifiedCanonicalKernelIrModuleV17,
    };
    use rustix::fs::{
        AtFlags, FileType, Mode, OFlags, PROC_SUPER_MAGIC, ResolveFlags, Stat, fchmod, fstat,
        fstatfs, fsync, linkat, openat, openat2, statat,
    };
    use sha2::{Digest, Sha256};

    const MAX_INPUT: usize = 64 * 1024;
    const MAX_PUBLISHED_LLVM: usize = 64 * 1024;
    const MAX_WORK: usize = 1 << 26;
    const MAX_STORAGE: usize = 64 * 1024 * 1024;
    const MAX_BLOCKS: usize = 128;
    const MAX_OPERATIONS: usize = 4096;
    const MAX_SSA: usize = 8192;
    const MAX_REPORT: usize = 4096;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(super) enum Failure {
        Usage,
        InputOpen,
        InputShape,
        InputTooLarge,
        InputRead,
        InputChanged,
        Allocation,
        Accounting,
        CanonicalAdmission,
        Profile,
        Lowering,
        OutputTooLarge,
        OutputParent,
        OutputTemporary,
        OutputWrite,
        OutputLink,
        OutputPublishedUncertain,
        ReportWrite,
    }

    impl Failure {
        pub(super) const fn message(self) -> &'static str {
            match self {
                Self::Usage => {
                    "usage: lower_diagnostic_ordered_program_v17 INPUT.kir NEW_OUTPUT.ll"
                }
                Self::InputOpen => "input secure open failed",
                Self::InputShape => "input must be a nonempty regular file",
                Self::InputTooLarge => "input exceeds the 64 KiB limit",
                Self::InputRead => "input bounded read failed",
                Self::InputChanged => "input metadata changed while retained",
                Self::Allocation => "bounded input allocation failed",
                Self::Accounting => "canonical resource budget or accounting failed",
                Self::CanonicalAdmission => "exact canonical V17 admission failed",
                Self::Profile => "input is outside the small single-root program profile",
                Self::Lowering => "exact-owner LLVM lowering refused this input",
                Self::OutputTooLarge => "generated LLVM exceeds the 64 KiB publication limit",
                Self::OutputParent => "output parent secure open failed",
                Self::OutputTemporary => "retained anonymous output is unavailable",
                Self::OutputWrite => {
                    "anonymous output write, verification or sync failed; no output published"
                }
                Self::OutputLink => {
                    "create-new output publication failed; existing files are never replaced"
                }
                Self::OutputPublishedUncertain => {
                    "post-publication check failed; a complete output may exist; no success observation"
                }
                Self::ReportWrite => "success observation write failed; complete output may exist",
            }
        }
    }

    type Result<T> = std::result::Result<T, Failure>;

    fn path_allowed(path: &Path) -> bool {
        let bytes = path.as_os_str().as_bytes();
        !bytes.is_empty() && bytes.len() <= 4096 && !bytes.contains(&0)
    }

    fn arguments(mut args: impl Iterator<Item = OsString>) -> Result<(PathBuf, PathBuf)> {
        let input = PathBuf::from(args.next().ok_or(Failure::Usage)?);
        let output = PathBuf::from(args.next().ok_or(Failure::Usage)?);
        if args.next().is_some()
            || !path_allowed(&input)
            || !path_allowed(&output)
            || output.as_os_str().as_bytes().ends_with(b"/")
            || output.file_name().is_none()
        {
            return Err(Failure::Usage);
        }
        Ok((input, output))
    }

    fn snapshot_equal(left: &Stat, right: &Stat) -> bool {
        left.st_dev == right.st_dev
            && left.st_ino == right.st_ino
            && left.st_mode == right.st_mode
            && left.st_nlink == right.st_nlink
            && left.st_size == right.st_size
            && left.st_mtime == right.st_mtime
            && left.st_mtime_nsec == right.st_mtime_nsec
            && left.st_ctime == right.st_ctime
            && left.st_ctime_nsec == right.st_ctime_nsec
    }

    struct Input {
        file: File,
        snapshot: Stat,
        bytes: Vec<u8>,
    }

    impl Input {
        fn unchanged(&self) -> Result<()> {
            let current = fstat(&self.file).map_err(|_| Failure::InputRead)?;
            if !snapshot_equal(&self.snapshot, &current) {
                return Err(Failure::InputChanged);
            }
            Ok(())
        }
    }

    fn read_input(path: &Path) -> Result<Input> {
        read_input_with_hook(path, || {})
    }

    fn read_input_with_hook(path: &Path, after_read: impl FnOnce()) -> Result<Input> {
        let fd = openat2(
            rustix::fs::CWD,
            path,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
        )
        .map_err(|_| Failure::InputOpen)?;
        let before = fstat(&fd).map_err(|_| Failure::InputRead)?;
        if FileType::from_raw_mode(before.st_mode) != FileType::RegularFile || before.st_size <= 0 {
            return Err(Failure::InputShape);
        }
        let length = usize::try_from(before.st_size).map_err(|_| Failure::InputTooLarge)?;
        if length > MAX_INPUT {
            return Err(Failure::InputTooLarge);
        }
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| Failure::Allocation)?;
        if bytes.capacity() > MAX_INPUT {
            return Err(Failure::Allocation);
        }
        bytes.resize(length, 0);
        let mut file = File::from(fd);
        file.read_exact(&mut bytes)
            .map_err(|_| Failure::InputRead)?;
        let mut extra = [0_u8; 1];
        loop {
            match file.read(&mut extra) {
                Ok(0) => break,
                Ok(_) => return Err(Failure::InputChanged),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(Failure::InputRead),
            }
        }
        after_read();
        let input = Input {
            file,
            snapshot: before,
            bytes,
        };
        input.unchanged()?;
        Ok(input)
    }

    #[derive(Debug)]
    struct Emission {
        llvm: String,
        canonical_identity: [u8; 32],
        canonical_bytes: u64,
        input_sha256: [u8; 32],
        llvm_sha256: [u8; 32],
        program_count: u8,
        descriptors: [u16; 16],
        registers: [u8; 5],
        retained_storage: usize,
    }

    fn checked_profile(
        owner: &VerifiedCanonicalKernelIrModuleV17,
    ) -> Result<(u8, [u16; 16], [u8; 5])> {
        let module = owner.module();
        let [kernel] = module.kernels.as_slice() else {
            return Err(Failure::Profile);
        };
        let [function] = module.functions.as_slice() else {
            return Err(Failure::Profile);
        };
        let body = function.body.as_ref().ok_or(Failure::Profile)?;
        if function.role != FunctionRole::KernelEntry
            || function.id != kernel.entry
            || kernel.id.as_str().len() > 128
            || body.blocks.len() > MAX_BLOCKS
            || function.signature.parameters.len() > MAX_SSA
        {
            return Err(Failure::Profile);
        }
        let mut operations = 0_usize;
        let mut values = body.parameters.len();
        let mut selected = None;
        for block in &body.blocks {
            operations = operations
                .checked_add(block.operations.len())
                .ok_or(Failure::Profile)?;
            values = values
                .checked_add(block.parameters.len())
                .ok_or(Failure::Profile)?;
            if operations > MAX_OPERATIONS || values > MAX_SSA {
                return Err(Failure::Profile);
            }
            for operation in &block.operations {
                values = values
                    .checked_add(operation.results.len())
                    .ok_or(Failure::Profile)?;
                if values > MAX_SSA {
                    return Err(Failure::Profile);
                }
                if let OperationKind::Gfx942OrderedProgram(program) = &operation.kind {
                    if selected.replace(program).is_some() {
                        return Err(Failure::Profile);
                    }
                }
            }
        }
        let program = selected.ok_or(Failure::Profile)?;
        let bindings = program.registers();
        let [a, b, c] = bindings.inputs();
        Ok((
            program.program().count(),
            *program.program().descriptors(),
            [bindings.scratch(), bindings.output(), a, b, c],
        ))
    }

    fn lower_bytes(
        bytes: &[u8],
        input_payload: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Emission> {
        let floor = budget.storage();
        let result = (|| {
            if bytes.is_empty() || bytes.len() > MAX_INPUT || input_payload < bytes.len() {
                return Err(Failure::InputTooLarge);
            }
            budget
                .reserve_storage(input_payload)
                .map_err(|_| Failure::Accounting)?;
            let (owner, receipt) = VerifiedCanonicalKernelIrModuleV17::
                from_canonical_bytes_with_verification_budget_v17(bytes, budget)
                .map_err(|_| Failure::CanonicalAdmission)?;
            budget
                .reserve_storage(receipt.retained_storage())
                .map_err(|_| Failure::Accounting)?;
            // This small additional structural census and hashing are explicitly
            // precharged. Existing text-emission allocations have their own bound.
            budget
                .charge_work(8 * (MAX_BLOCKS + MAX_OPERATIONS + MAX_SSA) + 4 * MAX_INPUT)
                .map_err(|_| Failure::Accounting)?;
            let (program_count, descriptors, registers) = checked_profile(&owner)?;
            let canonical_identity = *owner.identity().digest();
            let canonical_bytes = owner.identity().canonical_length();
            let llvm = lower_canonical_v17_compiler_module_to_gfx942_xnack_minus_llvm_ir(&owner)
                .map_err(|_| Failure::Lowering)?;
            if llvm.is_empty() || llvm.len() > MAX_PUBLISHED_LLVM {
                return Err(Failure::OutputTooLarge);
            }
            budget
                .charge_work(2 * MAX_PUBLISHED_LLVM)
                .map_err(|_| Failure::Accounting)?;
            if owner.canonical_bytes() != bytes {
                return Err(Failure::CanonicalAdmission);
            }
            let emission = Emission {
                input_sha256: Sha256::digest(bytes).into(),
                llvm_sha256: Sha256::digest(llvm.as_bytes()).into(),
                llvm,
                canonical_identity,
                canonical_bytes,
                program_count,
                descriptors,
                registers,
                retained_storage: receipt.retained_storage(),
            };
            drop(owner);
            budget
                .release_storage(receipt.retained_storage())
                .map_err(|_| Failure::Accounting)?;
            Ok(emission)
        })();
        let added = budget
            .storage()
            .checked_sub(floor)
            .ok_or(Failure::Accounting)?;
        budget
            .release_storage(added)
            .map_err(|_| Failure::Accounting)?;
        result
    }

    fn proc_fd_directory() -> Result<File> {
        let root = File::from(
            openat2(
                rustix::fs::CWD,
                "/proc",
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
                ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
            )
            .map_err(|_| Failure::OutputTemporary)?,
        );
        if fstatfs(&root).map_err(|_| Failure::OutputTemporary)?.f_type != PROC_SUPER_MAGIC {
            return Err(Failure::OutputTemporary);
        }
        let fd = File::from(
            openat2(
                &root,
                format!("{}/fd", std::process::id()),
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
                ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
            )
            .map_err(|_| Failure::OutputTemporary)?,
        );
        if fstatfs(&fd).map_err(|_| Failure::OutputTemporary)?.f_type != PROC_SUPER_MAGIC {
            return Err(Failure::OutputTemporary);
        }
        Ok(fd)
    }

    fn published_inode(before: &Stat, linked: &Stat, named: &Stat) -> bool {
        before.st_dev == linked.st_dev
            && before.st_ino == linked.st_ino
            && before.st_mode == linked.st_mode
            && before.st_size == linked.st_size
            && before.st_mtime == linked.st_mtime
            && before.st_mtime_nsec == linked.st_mtime_nsec
            && linked.st_nlink == 1
            && snapshot_equal(linked, named)
    }

    fn publish_new(path: &Path, bytes: &[u8]) -> Result<()> {
        publish_new_with_writer(path, bytes, |file| file.write_all(bytes))
    }

    fn publish_new_with_writer(
        path: &Path,
        expected: &[u8],
        write: impl FnOnce(&mut File) -> io::Result<()>,
    ) -> Result<()> {
        if expected.is_empty() || expected.len() > MAX_PUBLISHED_LLVM {
            return Err(Failure::OutputTooLarge);
        }
        let target = path.file_name().ok_or(Failure::Usage)?;
        let parent_path = path
            .parent()
            .filter(|value| !value.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = File::from(
            openat2(
                rustix::fs::CWD,
                parent_path,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
                ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
            )
            .map_err(|_| Failure::OutputParent)?,
        );
        let mut anonymous = File::from(
            openat(
                &parent,
                ".",
                OFlags::RDWR | OFlags::TMPFILE | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )
            .map_err(|_| Failure::OutputTemporary)?,
        );
        fchmod(&anonymous, Mode::from_raw_mode(0o600)).map_err(|_| Failure::OutputTemporary)?;
        let created = fstat(&anonymous).map_err(|_| Failure::OutputTemporary)?;
        if FileType::from_raw_mode(created.st_mode) != FileType::RegularFile
            || created.st_mode & 0o777 != 0o600
            || created.st_nlink != 0
        {
            return Err(Failure::OutputTemporary);
        }
        write(&mut anonymous).map_err(|_| Failure::OutputWrite)?;
        anonymous
            .seek(SeekFrom::Start(0))
            .map_err(|_| Failure::OutputWrite)?;
        let mut chunk = [0_u8; 4096];
        for expected_chunk in expected.chunks(chunk.len()) {
            anonymous
                .read_exact(&mut chunk[..expected_chunk.len()])
                .map_err(|_| Failure::OutputWrite)?;
            if &chunk[..expected_chunk.len()] != expected_chunk {
                return Err(Failure::OutputWrite);
            }
        }
        let complete = fstat(&anonymous).map_err(|_| Failure::OutputWrite)?;
        if complete.st_size != expected.len() as i64
            || complete.st_nlink != 0
            || complete.st_dev != created.st_dev
            || complete.st_ino != created.st_ino
            || complete.st_mode != created.st_mode
        {
            return Err(Failure::OutputWrite);
        }
        fsync(&anonymous).map_err(|_| Failure::OutputWrite)?;
        let proc_fd = proc_fd_directory()?;
        let fd_name = anonymous.as_raw_fd().to_string();
        let retained =
            statat(&proc_fd, &fd_name, AtFlags::empty()).map_err(|_| Failure::OutputTemporary)?;
        if !snapshot_equal(&complete, &retained) {
            return Err(Failure::OutputTemporary);
        }
        // linkat never replaces an existing name. Until this point the inode is
        // anonymous and every failure simply closes this owned descriptor.
        linkat(&proc_fd, &fd_name, &parent, target, AtFlags::SYMLINK_FOLLOW)
            .map_err(|_| Failure::OutputLink)?;
        let linked = fstat(&anonymous).map_err(|_| Failure::OutputPublishedUncertain)?;
        let named = statat(&parent, target, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| Failure::OutputPublishedUncertain)?;
        if !published_inode(&complete, &linked, &named) {
            return Err(Failure::OutputPublishedUncertain);
        }
        fsync(&parent).map_err(|_| Failure::OutputPublishedUncertain)?;
        let final_named = statat(&parent, target, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| Failure::OutputPublishedUncertain)?;
        if !published_inode(&complete, &linked, &final_named) {
            return Err(Failure::OutputPublishedUncertain);
        }
        Ok(())
    }

    fn hex(bytes: &[u8; 32]) -> String {
        use std::fmt::Write as _;
        let mut text = String::with_capacity(64);
        for byte in bytes {
            write!(text, "{byte:02x}").expect("String formatting");
        }
        text
    }

    fn report(emission: &Emission) -> Result<String> {
        let text = format!(
            concat!(
                "{{\"kind\":\"diagnostic_ordered_program_llvm_observation\",\"authority\":\"observation_only\",",
                "\"canonical_wire_version\":17,\"canonical_identity\":\"{}\",\"canonical_bytes\":{},",
                "\"input_file_sha256\":\"{}\",\"llvm_sha256\":\"{}\",\"llvm_bytes\":{},",
                "\"program_count\":{},\"descriptors\":{:?},\"register_plan\":{:?},",
                "\"canonical_retained_storage_bytes\":{},\"canonical_work_limit\":{},\"canonical_storage_limit\":{},",
                "\"max_input_bytes\":{},\"max_published_llvm_bytes\":{},\"emitter_text_limit_bytes\":{},",
                "\"canonical_and_emitter_accounting_are_separate\":true,",
                "\"source_authentication\":false,\"compiler_closure_attestation\":false,\"proof_authority\":false,",
                "\"protected_admission\":false,\"final_artifact_authority\":false,\"production_resume\":false,",
                "\"physical_register_values\":false,\"hardware_execution\":false}}\n"
            ),
            hex(&emission.canonical_identity),
            emission.canonical_bytes,
            hex(&emission.input_sha256),
            hex(&emission.llvm_sha256),
            emission.llvm.len(),
            emission.program_count,
            emission.descriptors,
            emission.registers,
            emission.retained_storage,
            MAX_WORK,
            MAX_STORAGE,
            MAX_INPUT,
            MAX_PUBLISHED_LLVM,
            fe2o3_amdgcn_model::MAX_COMPILER_MODULE_TEXT_BYTES,
        );
        if text.len() > MAX_REPORT {
            return Err(Failure::ReportWrite);
        }
        Ok(text)
    }

    pub(super) fn run(args: impl Iterator<Item = OsString>) -> Result<()> {
        let (input_path, output_path) = arguments(args)?;
        let input = read_input(&input_path)?;
        let input_payload = input
            .bytes
            .capacity()
            .checked_add(std::mem::size_of::<Vec<u8>>())
            .ok_or(Failure::Accounting)?;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MAX_WORK);
        let mut budget = Budget::new(&mut work, MAX_STORAGE);
        let emission = lower_bytes(&input.bytes, input_payload, &mut budget)?;
        let observation = report(&emission)?;
        input.unchanged()?;
        publish_new(&output_path, emission.llvm.as_bytes())?;
        io::stdout()
            .lock()
            .write_all(observation.as_bytes())
            .map_err(|_| Failure::ReportWrite)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use fe2o3_kernel_ir::*;
        use std::sync::atomic::{AtomicU64, Ordering};

        fn fixture(count: usize) -> Vec<u8> {
            let mut words = [0; 16];
            words[0] = 8; // out = input0; all remaining moves read initialized out.
            words[1..count].fill(0x48);
            let program = Gfx942U32ProgramV1::from_descriptors(count as u8, words).unwrap();
            let program = Gfx942OrderedProgramV1::new(
                AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
                Gfx942OrderedProgramRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
                [ValueId(0), ValueId(1), ValueId(2)],
                program,
            )
            .unwrap();
            let operation = Operation::effect_free(
                ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
                OperationKind::Gfx942OrderedProgram(program),
            );
            let capabilities = operation.required_capabilities();
            let mut block = BasicBlock::new(BlockId(0));
            block.operations.push(operation);
            block.terminator = Some(Terminator::Return { values: vec![] });
            let mut function = Function::kernel_entry(
                "program_impl",
                Signature::new(vec![Type::Scalar(ScalarType::U32); 3], vec![]),
                vec![ValueId(0), ValueId(1), ValueId(2)],
                vec![block],
            );
            function.required_capabilities = capabilities.clone();
            let mut kernel = Kernel::new(
                "program_kernel",
                "program_impl",
                LaunchDomain::D1 {
                    x: LaunchExtent::Dynamic,
                },
            );
            kernel.required_capabilities = capabilities.clone();
            kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
            let mut module = Module::new("synthetic_not_authenticated_source");
            module.required_capabilities = capabilities;
            module.functions.push(function);
            module.kernels.push(kernel);
            encode_module_v17(&module).unwrap()
        }

        fn lower(bytes: &[u8]) -> Result<Emission> {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MAX_WORK);
            let mut budget = Budget::new(&mut work, MAX_STORAGE);
            budget.reserve_storage(17).unwrap();
            let result = lower_bytes(bytes, bytes.len(), &mut budget);
            assert_eq!(budget.storage(), 17);
            result
        }

        #[test]
        fn exact_v17_owner_emits_unchanged_one_three_and_sixteen_step_text() {
            for count in [1, 3, 16] {
                let bytes = fixture(count);
                let snapshot = bytes.clone();
                let emission = lower(&bytes).unwrap();
                assert_eq!(bytes, snapshot);
                assert_eq!(emission.program_count, count as u8);
                assert_eq!(emission.canonical_bytes, bytes.len() as u64);
                assert_eq!(emission.llvm.matches(" asm sideeffect ").count(), 1);
                assert_eq!(emission.llvm.matches("v_mov_b32_e32").count(), count);
                assert!(!emission.llvm.contains("target datalayout = \"e-m:e-"));
                assert!(
                    emission
                        .llvm
                        .contains(fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1)
                );
                assert_eq!(
                    emission.input_sha256,
                    <[u8; 32]>::from(Sha256::digest(&bytes))
                );
                assert_eq!(
                    emission.llvm_sha256,
                    <[u8; 32]>::from(Sha256::digest(emission.llvm.as_bytes()))
                );
                assert!(report(&emission).unwrap().len() <= MAX_REPORT);
            }
        }

        #[test]
        fn old_versions_bad_bytes_shapes_and_budget_refuse_without_a_fallback() {
            let bytes = fixture(1);
            for version in [12_u16, 15, 16] {
                let mut wrong = bytes.clone();
                wrong[8..10].copy_from_slice(&version.to_le_bytes());
                assert_eq!(lower(&wrong).unwrap_err(), Failure::CanonicalAdmission);
            }
            assert_eq!(lower(&[]).unwrap_err(), Failure::InputTooLarge);
            assert_eq!(
                lower(&vec![0; MAX_INPUT + 1]).unwrap_err(),
                Failure::InputTooLarge
            );
            assert_eq!(
                lower(b"not canonical").unwrap_err(),
                Failure::CanonicalAdmission
            );
            let empty = encode_module_v17(&Module::new("empty")).unwrap();
            assert_eq!(lower(&empty).unwrap_err(), Failure::Profile);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
            let mut budget = Budget::new(&mut work, MAX_STORAGE);
            assert!(lower_bytes(&bytes, bytes.len(), &mut budget).is_err());
            assert_eq!(budget.storage(), 0);
        }

        #[test]
        fn one_short_storage_refuses_and_restores_the_nonzero_caller_floor() {
            let bytes = fixture(3);
            let input_payload = bytes.capacity() + std::mem::size_of::<Vec<u8>>();
            let caller_floor = 17;
            let peak = {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(MAX_WORK);
                let mut budget = Budget::new(&mut work, MAX_STORAGE);
                budget.reserve_storage(caller_floor).unwrap();
                lower_bytes(&bytes, input_payload, &mut budget).unwrap();
                assert_eq!(budget.storage(), caller_floor);
                assert_eq!(budget.failed_storage(), None);
                budget.peak_storage()
            };
            assert!(peak > caller_floor + input_payload);

            let mut exact_work = CanonicalKernelIrWorkBudgetV1::new(MAX_WORK);
            let mut exact = Budget::new(&mut exact_work, peak);
            exact.reserve_storage(caller_floor).unwrap();
            lower_bytes(&bytes, input_payload, &mut exact).unwrap();
            assert_eq!(exact.storage(), caller_floor);
            assert_eq!(exact.peak_storage(), peak);
            assert_eq!(exact.failed_storage(), None);

            let mut short_work = CanonicalKernelIrWorkBudgetV1::new(MAX_WORK);
            let mut short = Budget::new(&mut short_work, peak - 1);
            short.reserve_storage(caller_floor).unwrap();
            assert_eq!(
                lower_bytes(&bytes, input_payload, &mut short).unwrap_err(),
                Failure::CanonicalAdmission
            );
            assert_eq!(short.storage(), caller_floor);
            assert_eq!(short.failed_storage(), Some(peak));
        }

        struct Temp(PathBuf);
        impl Temp {
            fn new() -> Self {
                static NEXT: AtomicU64 = AtomicU64::new(0);
                for _ in 0..32 {
                    let path = std::env::temp_dir().join(format!(
                        "fe2o3-v17-llvm-example-{}-{}",
                        std::process::id(),
                        NEXT.fetch_add(1, Ordering::Relaxed)
                    ));
                    match std::fs::create_dir(&path) {
                        Ok(()) => return Self(path),
                        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                        Err(error) => panic!("temporary test directory: {error}"),
                    }
                }
                panic!("temporary test directory collision bound");
            }
        }
        impl Drop for Temp {
            fn drop(&mut self) {
                std::fs::remove_dir_all(&self.0).unwrap();
            }
        }

        #[test]
        fn bounded_file_read_refuses_symlinks_directories_oversize_and_mutation() {
            let temp = Temp::new();
            let file = temp.0.join("input.kir");
            std::fs::write(&file, fixture(1)).unwrap();
            assert!(read_input(&file).is_ok());
            let link = temp.0.join("link.kir");
            std::os::unix::fs::symlink(&file, &link).unwrap();
            assert!(matches!(read_input(&link), Err(Failure::InputOpen)));
            assert!(matches!(read_input(&temp.0), Err(Failure::InputShape)));
            assert!(matches!(
                read_input_with_hook(&file, || std::fs::write(&file, b"changed").unwrap()),
                Err(Failure::InputChanged)
            ));
            File::create(&file)
                .unwrap()
                .set_len((MAX_INPUT + 1) as u64)
                .unwrap();
            assert!(matches!(read_input(&file), Err(Failure::InputTooLarge)));
        }

        #[test]
        fn symlinked_parents_and_fifo_input_refuse_without_following_or_blocking() {
            let temp = Temp::new();
            let input = temp.0.join("input.kir");
            let bytes = fixture(1);
            std::fs::write(&input, &bytes).unwrap();
            let linked_parent = temp.0.join("linked-parent");
            std::os::unix::fs::symlink(&temp.0, &linked_parent).unwrap();
            assert!(matches!(
                read_input(&linked_parent.join("input.kir")),
                Err(Failure::InputOpen)
            ));
            assert_eq!(
                publish_new(&linked_parent.join("output.ll"), b"complete text\n"),
                Err(Failure::OutputParent)
            );
            assert!(!temp.0.join("output.ll").exists());

            let fifo = temp.0.join("input.fifo");
            rustix::fs::mkfifoat(rustix::fs::CWD, &fifo, Mode::from_raw_mode(0o600)).unwrap();
            assert!(matches!(read_input(&fifo), Err(Failure::InputShape)));
            assert_eq!(std::fs::read(&input).unwrap(), bytes);
        }

        #[test]
        fn anonymous_publication_is_create_new_and_failed_writer_leaves_no_name() {
            let temp = Temp::new();
            let path = temp.0.join("output.ll");
            let bytes = b"complete text\n";
            assert_eq!(
                publish_new_with_writer(&path, bytes, |file| {
                    file.write_all(b"partial")?;
                    Err(io::Error::other("injected write failure"))
                }),
                Err(Failure::OutputWrite)
            );
            assert!(!path.exists());
            assert_eq!(std::fs::read_dir(&temp.0).unwrap().count(), 0);
            assert_eq!(
                publish_new_with_writer(&path, bytes, |file| file.write_all(b"wrong bytes")),
                Err(Failure::OutputWrite)
            );
            assert!(!path.exists());
            publish_new(&path, bytes).unwrap();
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            assert_eq!(publish_new(&path, b"replacement"), Err(Failure::OutputLink));
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
            let link = temp.0.join("link.ll");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert_eq!(publish_new(&link, b"replacement"), Err(Failure::OutputLink));
            assert_eq!(std::fs::read(&path).unwrap(), bytes);
        }

        #[test]
        fn same_path_and_hardlinked_input_outputs_preserve_all_input_bytes() {
            let temp = Temp::new();
            let input = temp.0.join("input.kir");
            let alias = temp.0.join("hardlink.kir");
            let bytes = fixture(1);
            std::fs::write(&input, &bytes).unwrap();
            std::fs::hard_link(&input, &alias).unwrap();
            for output in [&input, &alias] {
                assert_eq!(
                    run([
                        input.as_os_str().to_os_string(),
                        output.as_os_str().to_os_string()
                    ]
                    .into_iter()),
                    Err(Failure::OutputLink)
                );
                assert_eq!(std::fs::read(&input).unwrap(), bytes);
                assert_eq!(std::fs::read(&alias).unwrap(), bytes);
                assert_eq!(std::fs::read_dir(&temp.0).unwrap().count(), 2);
            }
        }

        #[test]
        fn cli_is_exactly_two_bounded_paths() {
            assert!(arguments(["input.kir", "output.ll"].map(OsString::from).into_iter()).is_ok());
            for args in [
                vec![],
                vec!["one"],
                vec!["one", "two", "three"],
                vec!["one", "two/"],
            ] {
                assert_eq!(
                    arguments(args.into_iter().map(OsString::from)),
                    Err(Failure::Usage)
                );
            }
        }
    }
}

fn main() -> std::process::ExitCode {
    #[cfg(target_os = "linux")]
    match linux::run(std::env::args_os().skip(1)) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!(
                "diagnostic ordered-program LLVM observation: {}",
                error.message()
            );
            std::process::ExitCode::FAILURE
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("diagnostic ordered-program LLVM observation requires Linux");
        std::process::ExitCode::FAILURE
    }
}
