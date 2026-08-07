use super::*;

use std::ffi::{CString, c_char, c_int, c_long, c_uint, c_void};
use std::fs::{File, Metadata};
use std::io::{Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;

const LOCK_NAME: &str = "ledger.lock";
const STATE_NAME: &str = "ledger.state";
const STATE_TEMPORARY_NAME: &str = "ledger.state.tmp";
const INTENT_NAME: &str = "ledger.intent";
const INTENT_TEMPORARY_NAME: &str = "ledger.intent.tmp";

const AT_FDCWD: c_int = -100;
const SYS_OPENAT2: c_long = 437;
const O_RDONLY: u64 = 0;
const O_RDWR: u64 = 2;
const O_CREAT: u64 = 0o100;
const O_EXCL: u64 = 0o200;
const O_TRUNC: u64 = 0o1000;
const O_NONBLOCK: u64 = 0o4000;
const O_CLOEXEC: u64 = 0o2_000_000;
const O_NOFOLLOW: u64 = 0o400_000;
const O_DIRECTORY: u64 = 0o200_000;
const RESOLVE_NO_MAGICLINKS: u64 = 0x02;
const RESOLVE_NO_SYMLINKS: u64 = 0x04;
const RESOLVE_BENEATH: u64 = 0x08;
const LOCK_EX: c_int = 2;
const LOCK_NB: c_int = 4;
const LOCK_UN: c_int = 8;

#[repr(C)]
struct OpenHow {
    flags: u64,
    mode: u64,
    resolve: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ObjectSnapshot {
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
    owner: u32,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl ObjectSnapshot {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            links: metadata.nlink(),
            owner: metadata.uid(),
            size: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }

    fn same_object(self, other: Self) -> bool {
        self.device == other.device && self.inode == other.inode
    }
}

pub(super) struct LinuxLedger {
    directory: File,
    lock: File,
    owner: u32,
    lock_snapshot: ObjectSnapshot,
}

impl LinuxLedger {
    pub(super) fn create_new(
        path: &Path,
    ) -> Result<(Self, PersistentFreshnessRecoveryV1), PersistentFreshnessLedgerErrorV1> {
        let ledger = match Self::open_with_lock_flags(path, O_CREAT | O_EXCL) {
            Err(PersistentFreshnessLedgerErrorV1::Io {
                kind: io::ErrorKind::AlreadyExists,
                ..
            }) => return Err(PersistentFreshnessLedgerErrorV1::LedgerAlreadyExists),
            result => result?,
        };
        {
            acquire_lock(&ledger.lock)?;
            let initialized = (|| {
                ledger.validate_lock_name()?;
                initialize_new(&ledger)
            })();
            release_lock(&ledger.lock);
            initialized?;
        }
        Ok((ledger, PersistentFreshnessRecoveryV1::Initialized))
    }

    pub(super) fn open_existing(
        path: &Path,
    ) -> Result<(Self, PersistentFreshnessRecoveryV1), PersistentFreshnessLedgerErrorV1> {
        let mut ledger = Self::open_with_lock_flags(path, 0)?;
        let recovery = {
            let transaction = ledger.try_begin_exclusive()?;
            transaction.recovery
        };
        Ok((ledger, recovery))
    }

    fn open_with_lock_flags(
        path: &Path,
        creation_flags: u64,
    ) -> Result<Self, PersistentFreshnessLedgerErrorV1> {
        let directory = open_directory(path)?;
        let metadata = directory.metadata().map_err(|error| {
            io_error(
                PersistentFreshnessLedgerOperationV1::Inspect,
                PersistentFreshnessLedgerFileV1::Directory,
                error,
            )
        })?;
        if !metadata.is_dir() {
            return Err(PersistentFreshnessLedgerErrorV1::NotDirectory);
        }
        let owner = effective_user_id();
        if metadata.uid() != owner {
            return Err(PersistentFreshnessLedgerErrorV1::InsecureDirectoryOwner);
        }
        if metadata.mode() & 0o022 != 0 {
            return Err(PersistentFreshnessLedgerErrorV1::InsecureDirectoryPermissions);
        }

        let lock = open_relative(
            &directory,
            LOCK_NAME,
            O_RDWR | creation_flags | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK,
            if creation_flags == 0 { 0 } else { 0o600 },
            PersistentFreshnessLedgerFileV1::Lock,
        )?;
        let lock_snapshot =
            validate_regular_file(&lock, PersistentFreshnessLedgerFileV1::Lock, owner, 0)?;
        sync_directory(&directory)?;

        let ledger = Self {
            directory,
            lock,
            owner,
            lock_snapshot,
        };
        Ok(ledger)
    }

    pub(super) fn try_begin_exclusive(
        &mut self,
    ) -> Result<LinuxTransaction<'_>, PersistentFreshnessLedgerErrorV1> {
        acquire_lock(&self.lock)?;
        let recovered = (|| {
            self.validate_lock_name()?;
            recover(self)
        })();
        match recovered {
            Ok((state, recovery)) => Ok(LinuxTransaction {
                ledger: self,
                state,
                recovery,
                locked: true,
            }),
            Err(error) => {
                release_lock(&self.lock);
                Err(error)
            }
        }
    }

    fn validate_lock_name(&self) -> Result<(), PersistentFreshnessLedgerErrorV1> {
        let current = open_relative(
            &self.directory,
            LOCK_NAME,
            O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK,
            0,
            PersistentFreshnessLedgerFileV1::Lock,
        )?;
        let current = validate_regular_file(
            &current,
            PersistentFreshnessLedgerFileV1::Lock,
            self.owner,
            0,
        )?;
        if !current.same_object(self.lock_snapshot) {
            return Err(PersistentFreshnessLedgerErrorV1::LockFileSubstituted);
        }
        Ok(())
    }
}

fn initialize_new(ledger: &LinuxLedger) -> Result<(), PersistentFreshnessLedgerErrorV1> {
    for (name, file, max) in [
        (
            STATE_NAME,
            PersistentFreshnessLedgerFileV1::State,
            MAX_PERSISTENT_FRESHNESS_STATE_BYTES_V1,
        ),
        (
            STATE_TEMPORARY_NAME,
            PersistentFreshnessLedgerFileV1::StateTemporary,
            MAX_PERSISTENT_FRESHNESS_STATE_BYTES_V1,
        ),
        (
            INTENT_NAME,
            PersistentFreshnessLedgerFileV1::Intent,
            INTENT_BYTES_V1,
        ),
        (
            INTENT_TEMPORARY_NAME,
            PersistentFreshnessLedgerFileV1::IntentTemporary,
            INTENT_BYTES_V1,
        ),
    ] {
        if read_optional_file(ledger, name, file, max)?.is_some() {
            return Err(PersistentFreshnessLedgerErrorV1::LedgerAlreadyExists);
        }
    }

    let state = FreshnessStateV1::empty(random_namespace()?);
    write_new_file(
        ledger,
        STATE_TEMPORARY_NAME,
        PersistentFreshnessLedgerFileV1::StateTemporary,
        &state
            .encode()
            .map_err(|error| PersistentFreshnessLedgerErrorV1::Record {
                file: PersistentFreshnessLedgerFileV1::State,
                error,
            })?,
    )?;
    rename_file(
        ledger,
        STATE_TEMPORARY_NAME,
        STATE_NAME,
        PersistentFreshnessLedgerFileV1::State,
    )?;
    sync_directory(&ledger.directory)
}

pub(super) struct LinuxTransaction<'a> {
    ledger: &'a mut LinuxLedger,
    state: FreshnessStateV1,
    recovery: PersistentFreshnessRecoveryV1,
    locked: bool,
}

impl LinuxTransaction<'_> {
    pub(super) const fn recovery(&self) -> PersistentFreshnessRecoveryV1 {
        self.recovery
    }

    pub(super) fn state(&self) -> PersistentFreshnessStateInspectionV1 {
        self.state.inspection()
    }

    pub(super) fn consume(
        &mut self,
        identity: PersistentFreshnessIdentityV1,
    ) -> Result<PersistentFreshnessReceiptV1, PersistentFreshnessLedgerErrorV1> {
        let next = self.state.with_consumed(identity)?;
        let intent = FreshnessIntentV1::new(&self.state, &next, identity).map_err(|error| {
            PersistentFreshnessLedgerErrorV1::Record {
                file: PersistentFreshnessLedgerFileV1::Intent,
                error,
            }
        })?;

        write_new_file(
            self.ledger,
            INTENT_TEMPORARY_NAME,
            PersistentFreshnessLedgerFileV1::IntentTemporary,
            &intent
                .encode()
                .map_err(|error| PersistentFreshnessLedgerErrorV1::Record {
                    file: PersistentFreshnessLedgerFileV1::Intent,
                    error,
                })?,
        )?;
        rename_file(
            self.ledger,
            INTENT_TEMPORARY_NAME,
            INTENT_NAME,
            PersistentFreshnessLedgerFileV1::Intent,
        )?;
        sync_directory(&self.ledger.directory)?;

        write_new_file(
            self.ledger,
            STATE_TEMPORARY_NAME,
            PersistentFreshnessLedgerFileV1::StateTemporary,
            &next
                .encode()
                .map_err(|error| PersistentFreshnessLedgerErrorV1::Record {
                    file: PersistentFreshnessLedgerFileV1::State,
                    error,
                })?,
        )?;
        rename_file(
            self.ledger,
            STATE_TEMPORARY_NAME,
            STATE_NAME,
            PersistentFreshnessLedgerFileV1::State,
        )?;
        sync_directory(&self.ledger.directory)?;
        remove_file(
            self.ledger,
            INTENT_NAME,
            PersistentFreshnessLedgerFileV1::Intent,
        )?;
        sync_directory(&self.ledger.directory)?;

        self.state = next;
        Ok(PersistentFreshnessReceiptV1 {
            identity,
            namespace: self.state.namespace,
            generation: self.state.generation,
            state_identity: self.state.identity(),
        })
    }
}

impl Drop for LinuxTransaction<'_> {
    fn drop(&mut self) {
        if self.locked {
            release_lock(&self.ledger.lock);
            self.locked = false;
        }
    }
}

fn recover(
    ledger: &LinuxLedger,
) -> Result<(FreshnessStateV1, PersistentFreshnessRecoveryV1), PersistentFreshnessLedgerErrorV1> {
    let state_bytes = read_optional_file(
        ledger,
        STATE_NAME,
        PersistentFreshnessLedgerFileV1::State,
        MAX_PERSISTENT_FRESHNESS_STATE_BYTES_V1,
    )?;
    let state_temporary_bytes = read_optional_file(
        ledger,
        STATE_TEMPORARY_NAME,
        PersistentFreshnessLedgerFileV1::StateTemporary,
        MAX_PERSISTENT_FRESHNESS_STATE_BYTES_V1,
    )?;
    let intent_bytes = read_optional_file(
        ledger,
        INTENT_NAME,
        PersistentFreshnessLedgerFileV1::Intent,
        INTENT_BYTES_V1,
    )?;
    let intent_temporary_bytes = read_optional_file(
        ledger,
        INTENT_TEMPORARY_NAME,
        PersistentFreshnessLedgerFileV1::IntentTemporary,
        INTENT_BYTES_V1,
    )?;

    let Some(state_bytes) = state_bytes else {
        if intent_bytes.is_some() || intent_temporary_bytes.is_some() {
            return Err(PersistentFreshnessLedgerErrorV1::AmbiguousRecovery);
        }
        if let Some(bytes) = state_temporary_bytes {
            let state = decode_state(PersistentFreshnessLedgerFileV1::StateTemporary, &bytes)?;
            if state.generation != 0 || !state.entries.is_empty() {
                return Err(PersistentFreshnessLedgerErrorV1::RecoveryConflict);
            }
            rename_file(
                ledger,
                STATE_TEMPORARY_NAME,
                STATE_NAME,
                PersistentFreshnessLedgerFileV1::State,
            )?;
            sync_directory(&ledger.directory)?;
            return Ok((state, PersistentFreshnessRecoveryV1::Initialized));
        }
        return Err(PersistentFreshnessLedgerErrorV1::MissingState);
    };

    let state = decode_state(PersistentFreshnessLedgerFileV1::State, &state_bytes)?;
    if let Some(bytes) = intent_temporary_bytes {
        if intent_bytes.is_some() || state_temporary_bytes.is_some() {
            return Err(PersistentFreshnessLedgerErrorV1::AmbiguousRecovery);
        }
        let intent = decode_intent(PersistentFreshnessLedgerFileV1::IntentTemporary, &bytes)?;
        validate_previous_state(&state, intent)?;
        let next = state
            .with_consumed(intent.identity)
            .map_err(|_| PersistentFreshnessLedgerErrorV1::RecoveryConflict)?;
        if next.identity() != intent.next_state_identity
            || next.generation != intent.next_generation
        {
            return Err(PersistentFreshnessLedgerErrorV1::RecoveryConflict);
        }
        remove_file(
            ledger,
            INTENT_TEMPORARY_NAME,
            PersistentFreshnessLedgerFileV1::IntentTemporary,
        )?;
        sync_directory(&ledger.directory)?;
        return Ok((
            state,
            PersistentFreshnessRecoveryV1::DiscardedUncommittedIntent,
        ));
    }

    let Some(intent_bytes) = intent_bytes else {
        if state_temporary_bytes.is_some() {
            return Err(PersistentFreshnessLedgerErrorV1::UnexpectedRecoveryFile {
                file: PersistentFreshnessLedgerFileV1::StateTemporary,
            });
        }
        return Ok((state, PersistentFreshnessRecoveryV1::Clean));
    };
    let intent = decode_intent(PersistentFreshnessLedgerFileV1::Intent, &intent_bytes)?;

    if state.generation == intent.previous_generation
        && state.identity() == intent.previous_state_identity
    {
        let next = state
            .with_consumed(intent.identity)
            .map_err(|_| PersistentFreshnessLedgerErrorV1::RecoveryConflict)?;
        if next.generation != intent.next_generation
            || next.identity() != intent.next_state_identity
        {
            return Err(PersistentFreshnessLedgerErrorV1::RecoveryConflict);
        }
        if let Some(bytes) = state_temporary_bytes {
            let temporary = decode_state(PersistentFreshnessLedgerFileV1::StateTemporary, &bytes)?;
            if temporary != next {
                return Err(PersistentFreshnessLedgerErrorV1::RecoveryConflict);
            }
        } else {
            write_new_file(
                ledger,
                STATE_TEMPORARY_NAME,
                PersistentFreshnessLedgerFileV1::StateTemporary,
                &next
                    .encode()
                    .map_err(|error| PersistentFreshnessLedgerErrorV1::Record {
                        file: PersistentFreshnessLedgerFileV1::State,
                        error,
                    })?,
            )?;
        }
        rename_file(
            ledger,
            STATE_TEMPORARY_NAME,
            STATE_NAME,
            PersistentFreshnessLedgerFileV1::State,
        )?;
        sync_directory(&ledger.directory)?;
        remove_file(ledger, INTENT_NAME, PersistentFreshnessLedgerFileV1::Intent)?;
        sync_directory(&ledger.directory)?;
        return Ok((next, PersistentFreshnessRecoveryV1::AppliedPendingIntent));
    }

    if state.generation == intent.next_generation
        && state.identity() == intent.next_state_identity
        && state.contains(intent.identity)
    {
        if state_temporary_bytes.is_some() {
            return Err(PersistentFreshnessLedgerErrorV1::AmbiguousRecovery);
        }
        remove_file(ledger, INTENT_NAME, PersistentFreshnessLedgerFileV1::Intent)?;
        sync_directory(&ledger.directory)?;
        return Ok((state, PersistentFreshnessRecoveryV1::FinalizedPendingIntent));
    }

    Err(PersistentFreshnessLedgerErrorV1::RecoveryConflict)
}

fn validate_previous_state(
    state: &FreshnessStateV1,
    intent: FreshnessIntentV1,
) -> Result<(), PersistentFreshnessLedgerErrorV1> {
    if state.generation != intent.previous_generation
        || state.identity() != intent.previous_state_identity
    {
        Err(PersistentFreshnessLedgerErrorV1::RecoveryConflict)
    } else {
        Ok(())
    }
}

fn decode_state(
    file: PersistentFreshnessLedgerFileV1,
    bytes: &[u8],
) -> Result<FreshnessStateV1, PersistentFreshnessLedgerErrorV1> {
    FreshnessStateV1::decode(bytes)
        .map_err(|error| PersistentFreshnessLedgerErrorV1::Record { file, error })
}

fn decode_intent(
    file: PersistentFreshnessLedgerFileV1,
    bytes: &[u8],
) -> Result<FreshnessIntentV1, PersistentFreshnessLedgerErrorV1> {
    FreshnessIntentV1::decode(bytes)
        .map_err(|error| PersistentFreshnessLedgerErrorV1::Record { file, error })
}

fn open_directory(path: &Path) -> Result<File, PersistentFreshnessLedgerErrorV1> {
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| PersistentFreshnessLedgerErrorV1::InvalidDirectoryPath)?;
    let fd = openat2(
        AT_FDCWD,
        &path,
        O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_DIRECTORY,
        0,
        RESOLVE_NO_MAGICLINKS | RESOLVE_NO_SYMLINKS,
    )
    .map_err(|error| {
        io_error(
            PersistentFreshnessLedgerOperationV1::Open,
            PersistentFreshnessLedgerFileV1::Directory,
            error,
        )
    })?;
    Ok(File::from(fd))
}

fn open_relative(
    directory: &File,
    name: &str,
    flags: u64,
    mode: u64,
    file: PersistentFreshnessLedgerFileV1,
) -> Result<File, PersistentFreshnessLedgerErrorV1> {
    let name = CString::new(name).expect("fixed ledger names contain no NUL");
    let fd = openat2(
        directory.as_raw_fd(),
        &name,
        flags,
        mode,
        RESOLVE_BENEATH | RESOLVE_NO_MAGICLINKS | RESOLVE_NO_SYMLINKS,
    )
    .map_err(|error| io_error(PersistentFreshnessLedgerOperationV1::Open, file, error))?;
    Ok(File::from(fd))
}

fn openat2(
    directory: RawFd,
    path: &CString,
    flags: u64,
    mode: u64,
    resolve: u64,
) -> io::Result<OwnedFd> {
    let how = OpenHow {
        flags,
        mode,
        resolve,
    };
    // SAFETY: `path` and `how` remain live for the syscall, their lengths are
    // exact, and a successful result is a newly owned descriptor.
    let result = unsafe {
        linux_syscall(
            SYS_OPENAT2,
            directory,
            path.as_ptr(),
            &how,
            std::mem::size_of::<OpenHow>(),
        )
    };
    if result < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: successful `openat2` returns a new owned descriptor.
        Ok(unsafe { OwnedFd::from_raw_fd(result as RawFd) })
    }
}

fn validate_regular_file(
    file: &File,
    kind: PersistentFreshnessLedgerFileV1,
    owner: u32,
    max: usize,
) -> Result<ObjectSnapshot, PersistentFreshnessLedgerErrorV1> {
    let metadata = file
        .metadata()
        .map_err(|error| io_error(PersistentFreshnessLedgerOperationV1::Inspect, kind, error))?;
    if !metadata.is_file() {
        return Err(PersistentFreshnessLedgerErrorV1::FileNotRegular { file: kind });
    }
    if metadata.nlink() != 1 {
        return Err(PersistentFreshnessLedgerErrorV1::FileHasMultipleLinks {
            file: kind,
            links: metadata.nlink(),
        });
    }
    if metadata.uid() != owner {
        return Err(PersistentFreshnessLedgerErrorV1::FileOwnerMismatch { file: kind });
    }
    if metadata.mode() & 0o077 != 0 {
        return Err(PersistentFreshnessLedgerErrorV1::FilePermissionsTooBroad { file: kind });
    }
    if metadata.len() > max as u64 {
        return Err(PersistentFreshnessLedgerErrorV1::FileTooLarge { file: kind, max });
    }
    Ok(ObjectSnapshot::from_metadata(&metadata))
}

fn read_optional_file(
    ledger: &LinuxLedger,
    name: &str,
    kind: PersistentFreshnessLedgerFileV1,
    max: usize,
) -> Result<Option<Vec<u8>>, PersistentFreshnessLedgerErrorV1> {
    let mut file = match open_relative(
        &ledger.directory,
        name,
        O_RDONLY | O_CLOEXEC | O_NOFOLLOW | O_NONBLOCK,
        0,
        kind,
    ) {
        Ok(file) => file,
        Err(PersistentFreshnessLedgerErrorV1::Io {
            kind: io::ErrorKind::NotFound,
            ..
        }) => return Ok(None),
        Err(error) => return Err(error),
    };
    let before = validate_regular_file(&file, kind, ledger.owner, max)?;
    let mut bytes = Vec::with_capacity((before.size as usize).min(max));
    Read::by_ref(&mut file)
        .take(max as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io_error(PersistentFreshnessLedgerOperationV1::Read, kind, error))?;
    if bytes.len() > max {
        return Err(PersistentFreshnessLedgerErrorV1::FileTooLarge { file: kind, max });
    }
    let after = validate_regular_file(&file, kind, ledger.owner, max)?;
    if before != after || bytes.len() as u64 != after.size {
        return Err(PersistentFreshnessLedgerErrorV1::FileChangedDuringRead { file: kind });
    }
    Ok(Some(bytes))
}

fn write_new_file(
    ledger: &LinuxLedger,
    name: &str,
    kind: PersistentFreshnessLedgerFileV1,
    bytes: &[u8],
) -> Result<(), PersistentFreshnessLedgerErrorV1> {
    let max = match kind {
        PersistentFreshnessLedgerFileV1::StateTemporary => MAX_PERSISTENT_FRESHNESS_STATE_BYTES_V1,
        PersistentFreshnessLedgerFileV1::IntentTemporary => INTENT_BYTES_V1,
        _ => bytes.len(),
    };
    if bytes.len() > max {
        return Err(PersistentFreshnessLedgerErrorV1::FileTooLarge { file: kind, max });
    }
    let mut file = open_relative(
        &ledger.directory,
        name,
        O_RDWR | O_CREAT | O_EXCL | O_CLOEXEC | O_NOFOLLOW | O_TRUNC | O_NONBLOCK,
        0o600,
        kind,
    )?;
    validate_regular_file(&file, kind, ledger.owner, max)?;
    file.write_all(bytes)
        .map_err(|error| io_error(PersistentFreshnessLedgerOperationV1::Write, kind, error))?;
    file.sync_all()
        .map_err(|error| io_error(PersistentFreshnessLedgerOperationV1::Sync, kind, error))?;
    let snapshot = validate_regular_file(&file, kind, ledger.owner, max)?;
    if snapshot.size != bytes.len() as u64 {
        return Err(PersistentFreshnessLedgerErrorV1::FileChangedDuringRead { file: kind });
    }
    Ok(())
}

fn rename_file(
    ledger: &LinuxLedger,
    source: &str,
    destination: &str,
    kind: PersistentFreshnessLedgerFileV1,
) -> Result<(), PersistentFreshnessLedgerErrorV1> {
    let source = CString::new(source).expect("fixed ledger names contain no NUL");
    let destination = CString::new(destination).expect("fixed ledger names contain no NUL");
    // SAFETY: both names are live NUL-terminated strings and both descriptors
    // are the retained ledger directory.
    if unsafe {
        linux_renameat(
            ledger.directory.as_raw_fd(),
            source.as_ptr(),
            ledger.directory.as_raw_fd(),
            destination.as_ptr(),
        )
    } < 0
    {
        Err(io_error(
            PersistentFreshnessLedgerOperationV1::Rename,
            kind,
            io::Error::last_os_error(),
        ))
    } else {
        Ok(())
    }
}

fn remove_file(
    ledger: &LinuxLedger,
    name: &str,
    kind: PersistentFreshnessLedgerFileV1,
) -> Result<(), PersistentFreshnessLedgerErrorV1> {
    let name = CString::new(name).expect("fixed ledger names contain no NUL");
    // SAFETY: `name` is a live NUL-terminated fixed filename and the retained
    // directory descriptor remains open.
    if unsafe { linux_unlinkat(ledger.directory.as_raw_fd(), name.as_ptr(), 0) } < 0 {
        Err(io_error(
            PersistentFreshnessLedgerOperationV1::Remove,
            kind,
            io::Error::last_os_error(),
        ))
    } else {
        Ok(())
    }
}

fn sync_directory(directory: &File) -> Result<(), PersistentFreshnessLedgerErrorV1> {
    directory.sync_all().map_err(|error| {
        io_error(
            PersistentFreshnessLedgerOperationV1::Sync,
            PersistentFreshnessLedgerFileV1::Directory,
            error,
        )
    })
}

fn acquire_lock(lock: &File) -> Result<(), PersistentFreshnessLedgerErrorV1> {
    // SAFETY: `lock` owns a live descriptor and the operation is a documented
    // nonblocking exclusive `flock` request.
    if unsafe { linux_flock(lock.as_raw_fd(), LOCK_EX | LOCK_NB) } == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.kind() == io::ErrorKind::WouldBlock {
        Err(PersistentFreshnessLedgerErrorV1::LockBusy)
    } else {
        Err(io_error(
            PersistentFreshnessLedgerOperationV1::Lock,
            PersistentFreshnessLedgerFileV1::Lock,
            error,
        ))
    }
}

fn release_lock(lock: &File) {
    // SAFETY: `lock` remains live through this call. Closing the descriptor also
    // releases the lock if this best-effort explicit unlock fails.
    let _ = unsafe { linux_flock(lock.as_raw_fd(), LOCK_UN) };
}

fn io_error(
    operation: PersistentFreshnessLedgerOperationV1,
    file: PersistentFreshnessLedgerFileV1,
    error: io::Error,
) -> PersistentFreshnessLedgerErrorV1 {
    PersistentFreshnessLedgerErrorV1::Io {
        operation,
        file,
        kind: error.kind(),
    }
}

fn effective_user_id() -> u32 {
    // SAFETY: `geteuid` has no preconditions and returns the effective uid.
    unsafe { linux_geteuid() }
}

fn random_namespace() -> Result<Digest, PersistentFreshnessLedgerErrorV1> {
    let mut bytes = [0_u8; 32];
    let mut offset = 0;
    while offset < bytes.len() {
        // SAFETY: the remaining slice is writable for the exact supplied
        // length. `getrandom` does not retain the pointer.
        let result = unsafe {
            linux_getrandom(
                bytes[offset..].as_mut_ptr().cast::<c_void>(),
                bytes.len() - offset,
                0,
            )
        };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(io_error(
                PersistentFreshnessLedgerOperationV1::Create,
                PersistentFreshnessLedgerFileV1::State,
                error,
            ));
        }
        if result == 0 {
            return Err(io_error(
                PersistentFreshnessLedgerOperationV1::Create,
                PersistentFreshnessLedgerFileV1::State,
                io::Error::new(io::ErrorKind::UnexpectedEof, "getrandom returned no bytes"),
            ));
        }
        offset += result as usize;
    }
    if bytes.iter().all(|byte| *byte == 0) {
        return Err(PersistentFreshnessLedgerErrorV1::ZeroNamespace);
    }
    Ok(Digest::from_bytes(bytes))
}

unsafe extern "C" {
    #[link_name = "syscall"]
    fn linux_syscall(number: c_long, ...) -> c_long;

    #[link_name = "flock"]
    fn linux_flock(fd: c_int, operation: c_int) -> c_int;

    #[link_name = "renameat"]
    fn linux_renameat(
        old_directory: c_int,
        old_path: *const c_char,
        new_directory: c_int,
        new_path: *const c_char,
    ) -> c_int;

    #[link_name = "unlinkat"]
    fn linux_unlinkat(directory: c_int, path: *const c_char, flags: c_int) -> c_int;

    #[link_name = "geteuid"]
    fn linux_geteuid() -> c_uint;

    #[link_name = "getrandom"]
    fn linux_getrandom(buffer: *mut c_void, length: usize, flags: c_uint) -> isize;
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread;
    use std::time::{Duration, Instant};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory {
        path: std::path::PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let nonce = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "fe2o3-proof-freshness-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn file(&self, name: &str) -> std::path::PathBuf {
            self.path.join(name)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn digest(seed: u8) -> Digest {
        Digest::from_bytes([seed; 32])
    }

    fn identity(seed: u8) -> PersistentFreshnessIdentityV1 {
        PersistentFreshnessIdentityV1 {
            challenge: digest(seed),
            transcript: digest(seed.wrapping_add(1)),
            result: digest(seed.wrapping_add(2)),
        }
    }

    fn stage_intent(
        transaction: &mut LinuxTransaction<'_>,
        identity: PersistentFreshnessIdentityV1,
        write_state: bool,
        publish_intent: bool,
    ) {
        let next = transaction.state.with_consumed(identity).unwrap();
        let intent = FreshnessIntentV1::new(&transaction.state, &next, identity).unwrap();
        write_new_file(
            transaction.ledger,
            INTENT_TEMPORARY_NAME,
            PersistentFreshnessLedgerFileV1::IntentTemporary,
            &intent.encode().unwrap(),
        )
        .unwrap();
        if publish_intent {
            rename_file(
                transaction.ledger,
                INTENT_TEMPORARY_NAME,
                INTENT_NAME,
                PersistentFreshnessLedgerFileV1::Intent,
            )
            .unwrap();
            sync_directory(&transaction.ledger.directory).unwrap();
        }
        if write_state {
            write_new_file(
                transaction.ledger,
                STATE_TEMPORARY_NAME,
                PersistentFreshnessLedgerFileV1::StateTemporary,
                &next.encode().unwrap(),
            )
            .unwrap();
            rename_file(
                transaction.ledger,
                STATE_TEMPORARY_NAME,
                STATE_NAME,
                PersistentFreshnessLedgerFileV1::State,
            )
            .unwrap();
            sync_directory(&transaction.ledger.directory).unwrap();
        }
    }

    #[test]
    fn initialization_consumption_and_restart_are_durable() {
        let directory = TestDirectory::new();
        let (mut ledger, recovery) = LinuxLedger::create_new(directory.path()).unwrap();
        assert_eq!(recovery, PersistentFreshnessRecoveryV1::Initialized);
        let initial = ledger.try_begin_exclusive().unwrap().state();
        assert_eq!(initial.generation(), 0);
        assert_ne!(initial.namespace(), digest(0));

        let receipt = ledger
            .try_begin_exclusive()
            .unwrap()
            .consume(identity(1))
            .unwrap();
        assert_eq!(receipt.generation(), 1);
        assert_eq!(receipt.namespace(), initial.namespace());
        drop(ledger);

        let (mut reopened, recovery) = LinuxLedger::open_existing(directory.path()).unwrap();
        assert_eq!(recovery, PersistentFreshnessRecoveryV1::Clean);
        let reopened_state = reopened.try_begin_exclusive().unwrap().state();
        assert_eq!(reopened_state.generation(), 1);
        assert_eq!(reopened_state.namespace(), initial.namespace());
        assert_eq!(
            reopened.try_begin_exclusive().unwrap().consume(identity(1)),
            Err(PersistentFreshnessLedgerErrorV1::Replay {
                field: PersistentFreshnessIdentityFieldV1::Challenge,
            })
        );
    }

    #[test]
    fn deleted_state_cannot_reinitialize_or_replay() {
        let directory = TestDirectory::new();
        let (mut ledger, _) = LinuxLedger::create_new(directory.path()).unwrap();
        ledger
            .try_begin_exclusive()
            .unwrap()
            .consume(identity(3))
            .unwrap();
        drop(ledger);
        fs::remove_file(directory.file(STATE_NAME)).unwrap();

        assert!(matches!(
            LinuxLedger::open_existing(directory.path()),
            Err(PersistentFreshnessLedgerErrorV1::MissingState)
        ));
        assert!(matches!(
            LinuxLedger::create_new(directory.path()),
            Err(PersistentFreshnessLedgerErrorV1::LedgerAlreadyExists)
        ));
    }

    #[test]
    fn crash_recovery_applies_or_finalizes_every_durable_intent() {
        for (write_state, expected) in [
            (false, PersistentFreshnessRecoveryV1::AppliedPendingIntent),
            (true, PersistentFreshnessRecoveryV1::FinalizedPendingIntent),
        ] {
            let directory = TestDirectory::new();
            let (mut ledger, _) = LinuxLedger::create_new(directory.path()).unwrap();
            {
                let mut transaction = ledger.try_begin_exclusive().unwrap();
                stage_intent(&mut transaction, identity(4), write_state, true);
            }
            drop(ledger);

            let (mut reopened, recovery) = LinuxLedger::open_existing(directory.path()).unwrap();
            assert_eq!(recovery, expected);
            assert_eq!(
                reopened.try_begin_exclusive().unwrap().state().generation(),
                1
            );
            assert_eq!(
                reopened.try_begin_exclusive().unwrap().consume(identity(4)),
                Err(PersistentFreshnessLedgerErrorV1::Replay {
                    field: PersistentFreshnessIdentityFieldV1::Challenge,
                })
            );
        }
    }

    #[test]
    fn unpublished_intent_is_validated_then_discarded() {
        let directory = TestDirectory::new();
        let (mut ledger, _) = LinuxLedger::create_new(directory.path()).unwrap();
        {
            let mut transaction = ledger.try_begin_exclusive().unwrap();
            stage_intent(&mut transaction, identity(7), false, false);
        }
        drop(ledger);

        let (mut reopened, recovery) = LinuxLedger::open_existing(directory.path()).unwrap();
        assert_eq!(
            recovery,
            PersistentFreshnessRecoveryV1::DiscardedUncommittedIntent
        );
        assert_eq!(
            reopened.try_begin_exclusive().unwrap().state().generation(),
            0
        );
        reopened
            .try_begin_exclusive()
            .unwrap()
            .consume(identity(7))
            .unwrap();
    }

    #[test]
    fn symlinks_nonregular_files_hardlinks_and_broad_permissions_fail_closed() {
        let target = TestDirectory::new();
        let link_parent = TestDirectory::new();
        let link = link_parent.file("linked-ledger");
        symlink(target.path(), &link).unwrap();
        assert!(LinuxLedger::open_existing(&link).is_err());

        let directory = TestDirectory::new();
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o770)).unwrap();
        assert!(matches!(
            LinuxLedger::open_existing(directory.path()),
            Err(PersistentFreshnessLedgerErrorV1::InsecureDirectoryPermissions)
        ));

        let directory = TestDirectory::new();
        let (ledger, _) = LinuxLedger::create_new(directory.path()).unwrap();
        drop(ledger);
        fs::remove_file(directory.file(STATE_NAME)).unwrap();
        symlink("/etc/passwd", directory.file(STATE_NAME)).unwrap();
        assert!(LinuxLedger::open_existing(directory.path()).is_err());

        let directory = TestDirectory::new();
        let (ledger, _) = LinuxLedger::create_new(directory.path()).unwrap();
        drop(ledger);
        fs::remove_file(directory.file(STATE_NAME)).unwrap();
        fs::create_dir(directory.file(STATE_NAME)).unwrap();
        assert!(matches!(
            LinuxLedger::open_existing(directory.path()),
            Err(PersistentFreshnessLedgerErrorV1::FileNotRegular {
                file: PersistentFreshnessLedgerFileV1::State,
            })
        ));

        let directory = TestDirectory::new();
        let (ledger, _) = LinuxLedger::create_new(directory.path()).unwrap();
        drop(ledger);
        fs::hard_link(directory.file(STATE_NAME), directory.file("state-alias")).unwrap();
        assert!(matches!(
            LinuxLedger::open_existing(directory.path()),
            Err(PersistentFreshnessLedgerErrorV1::FileHasMultipleLinks {
                file: PersistentFreshnessLedgerFileV1::State,
                links: 2,
            })
        ));

        let directory = TestDirectory::new();
        let (ledger, _) = LinuxLedger::create_new(directory.path()).unwrap();
        drop(ledger);
        fs::hard_link(directory.file(LOCK_NAME), directory.file("lock-alias")).unwrap();
        assert!(matches!(
            LinuxLedger::open_existing(directory.path()),
            Err(PersistentFreshnessLedgerErrorV1::FileHasMultipleLinks {
                file: PersistentFreshnessLedgerFileV1::Lock,
                links: 2,
            })
        ));

        let directory = TestDirectory::new();
        let (ledger, _) = LinuxLedger::create_new(directory.path()).unwrap();
        drop(ledger);
        fs::set_permissions(
            directory.file(STATE_NAME),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        assert!(matches!(
            LinuxLedger::open_existing(directory.path()),
            Err(PersistentFreshnessLedgerErrorV1::FilePermissionsTooBroad {
                file: PersistentFreshnessLedgerFileV1::State,
            })
        ));
    }

    #[test]
    fn retained_lock_descriptor_detects_name_substitution() {
        let directory = TestDirectory::new();
        let (mut ledger, _) = LinuxLedger::create_new(directory.path()).unwrap();
        fs::rename(directory.file(LOCK_NAME), directory.file("old-lock")).unwrap();
        fs::write(directory.file(LOCK_NAME), []).unwrap();
        fs::set_permissions(directory.file(LOCK_NAME), fs::Permissions::from_mode(0o600)).unwrap();
        assert!(matches!(
            ledger.try_begin_exclusive(),
            Err(PersistentFreshnessLedgerErrorV1::LockFileSubstituted)
        ));
    }

    #[test]
    fn oversized_malformed_trailing_and_ambiguous_recovery_files_are_rejected() {
        let directory = TestDirectory::new();
        let (ledger, _) = LinuxLedger::create_new(directory.path()).unwrap();
        drop(ledger);
        fs::write(
            directory.file(STATE_NAME),
            vec![0; MAX_PERSISTENT_FRESHNESS_STATE_BYTES_V1 + 1],
        )
        .unwrap();
        assert!(matches!(
            LinuxLedger::open_existing(directory.path()),
            Err(PersistentFreshnessLedgerErrorV1::FileTooLarge {
                file: PersistentFreshnessLedgerFileV1::State,
                ..
            })
        ));

        for bytes in [vec![0; 64], {
            let mut bytes = FreshnessStateV1::empty(digest(0xf0)).encode().unwrap();
            bytes.push(0);
            bytes
        }] {
            let directory = TestDirectory::new();
            let (ledger, _) = LinuxLedger::create_new(directory.path()).unwrap();
            drop(ledger);
            fs::write(directory.file(STATE_NAME), bytes).unwrap();
            assert!(matches!(
                LinuxLedger::open_existing(directory.path()),
                Err(PersistentFreshnessLedgerErrorV1::Record {
                    file: PersistentFreshnessLedgerFileV1::State,
                    ..
                })
            ));
        }

        let directory = TestDirectory::new();
        let (ledger, _) = LinuxLedger::create_new(directory.path()).unwrap();
        drop(ledger);
        fs::write(directory.file(INTENT_NAME), vec![0; 64]).unwrap();
        fs::set_permissions(
            directory.file(INTENT_NAME),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        assert!(matches!(
            LinuxLedger::open_existing(directory.path()),
            Err(PersistentFreshnessLedgerErrorV1::Record {
                file: PersistentFreshnessLedgerFileV1::Intent,
                ..
            })
        ));

        let directory = TestDirectory::new();
        let (ledger, _) = LinuxLedger::create_new(directory.path()).unwrap();
        drop(ledger);
        fs::write(
            directory.file(STATE_TEMPORARY_NAME),
            FreshnessStateV1::empty(digest(0xf0)).encode().unwrap(),
        )
        .unwrap();
        fs::set_permissions(
            directory.file(STATE_TEMPORARY_NAME),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        assert!(matches!(
            LinuxLedger::open_existing(directory.path()),
            Err(PersistentFreshnessLedgerErrorV1::UnexpectedRecoveryFile {
                file: PersistentFreshnessLedgerFileV1::StateTemporary,
            })
        ));
    }

    #[test]
    fn independently_replayed_transcript_and_result_are_rejected() {
        let directory = TestDirectory::new();
        let (mut ledger, _) = LinuxLedger::create_new(directory.path()).unwrap();
        ledger
            .try_begin_exclusive()
            .unwrap()
            .consume(identity(10))
            .unwrap();
        let replayed_transcript = PersistentFreshnessIdentityV1 {
            challenge: digest(20),
            transcript: digest(11),
            result: digest(22),
        };
        assert_eq!(
            ledger
                .try_begin_exclusive()
                .unwrap()
                .consume(replayed_transcript),
            Err(PersistentFreshnessLedgerErrorV1::Replay {
                field: PersistentFreshnessIdentityFieldV1::Transcript,
            })
        );
        let replayed_result = PersistentFreshnessIdentityV1 {
            challenge: digest(23),
            transcript: digest(24),
            result: digest(12),
        };
        assert_eq!(
            ledger
                .try_begin_exclusive()
                .unwrap()
                .consume(replayed_result),
            Err(PersistentFreshnessLedgerErrorV1::Replay {
                field: PersistentFreshnessIdentityFieldV1::Result,
            })
        );
    }

    #[test]
    #[ignore]
    fn subprocess_consume_same_identity() {
        let path = std::env::var_os("FE2O3_LEDGER_TEST_DIRECTORY").unwrap();
        let (mut ledger, _) = match LinuxLedger::open_existing(Path::new(&path)) {
            Ok(value) => value,
            Err(PersistentFreshnessLedgerErrorV1::LockBusy) => std::process::exit(22),
            Err(error) => panic!("unexpected child open error: {error}"),
        };
        let outcome = ledger
            .try_begin_exclusive()
            .and_then(|mut transaction| transaction.consume(identity(30)));
        match outcome {
            Ok(_) => std::process::exit(20),
            Err(PersistentFreshnessLedgerErrorV1::Replay { .. }) => std::process::exit(21),
            Err(PersistentFreshnessLedgerErrorV1::LockBusy) => std::process::exit(22),
            Err(error) => panic!("unexpected child consume error: {error}"),
        }
    }

    #[test]
    fn two_processes_cannot_consume_the_same_identity() {
        let directory = TestDirectory::new();
        let (ledger, _) = LinuxLedger::create_new(directory.path()).unwrap();
        drop(ledger);
        let executable = std::env::current_exe().unwrap();
        let spawn = || {
            Command::new(&executable)
                .args([
                    "--ignored",
                    "--exact",
                    "persistent_freshness::linux::tests::subprocess_consume_same_identity",
                    "--nocapture",
                ])
                .env("FE2O3_LEDGER_TEST_DIRECTORY", directory.path())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap()
        };
        let mut first = spawn();
        let mut second = spawn();
        let first = first.wait().unwrap().code().unwrap();
        let second = second.wait().unwrap().code().unwrap();
        assert_eq!(
            [first, second].iter().filter(|code| **code == 20).count(),
            1
        );
        assert!(
            [first, second]
                .iter()
                .filter(|code| **code == 21 || **code == 22)
                .count()
                == 1
        );

        let (mut ledger, _) = LinuxLedger::open_existing(directory.path()).unwrap();
        assert_eq!(
            ledger.try_begin_exclusive().unwrap().state().generation(),
            1
        );
    }

    #[test]
    #[ignore]
    fn subprocess_hold_lock() {
        let path = std::env::var_os("FE2O3_LEDGER_TEST_DIRECTORY").unwrap();
        let ready = std::env::var_os("FE2O3_LEDGER_TEST_READY").unwrap();
        let (mut ledger, _) = LinuxLedger::open_existing(Path::new(&path)).unwrap();
        let _transaction = ledger.try_begin_exclusive().unwrap();
        fs::write(ready, b"ready").unwrap();
        thread::sleep(Duration::from_secs(2));
    }

    #[test]
    fn cross_process_lock_contention_fails_closed() {
        let directory = TestDirectory::new();
        let ready = directory.file("ready");
        let (ledger, _) = LinuxLedger::create_new(directory.path()).unwrap();
        drop(ledger);
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "persistent_freshness::linux::tests::subprocess_hold_lock",
                "--nocapture",
            ])
            .env("FE2O3_LEDGER_TEST_DIRECTORY", directory.path())
            .env("FE2O3_LEDGER_TEST_READY", &ready)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !ready.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(ready.exists());
        assert!(matches!(
            LinuxLedger::open_existing(directory.path()),
            Err(PersistentFreshnessLedgerErrorV1::LockBusy)
        ));
        assert!(child.wait().unwrap().success());
        LinuxLedger::open_existing(directory.path()).unwrap();
    }
}
