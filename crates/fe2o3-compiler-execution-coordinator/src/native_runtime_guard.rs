//! Policy checks through original stopped-task custody, never an admission token.
use crate::{
    compiler_invocation_backing::CompilerInvocationBacking as Backing,
    native_runtime_descriptors as descriptors,
    native_runtime_inventory::{self as inventory, NativeCompilerExecutableInventory as Inventory},
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_protected_service_spawn::{
    native_spawn::{
        ProtectedServiceSpawnErrorV2 as SpawnError, RuntimeSyscallEntryV1 as Entry,
        RuntimeTaskObservationV1 as View,
    },
    trace_runtime::{self as policy, PolicyError, memory::MappingRequirement},
};
use rustix::{fs, io};
use std::{fmt, fs::File, mem::size_of, os::fd::AsFd};

#[path = "native_runtime_kernel_image.rs"]
mod kernel;
pub(crate) use kernel::NativeKernelImage;
#[path = "native_runtime_descriptor_guard.rs"]
pub(crate) mod descriptor;

#[derive(Debug)]
pub(crate) enum Error {
    Resource(Resource),
    Spawn(SpawnError),
    Inventory(inventory::Error),
    Descriptor(descriptors::Error),
    Policy(PolicyError),
    Io {
        operation: &'static str,
        source: io::Errno,
    },
    Invalid(&'static str),
}
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<SpawnError> for Error {
    fn from(e: SpawnError) -> Self {
        Self::Spawn(e)
    }
}
impl From<inventory::Error> for Error {
    fn from(e: inventory::Error) -> Self {
        Self::Inventory(e)
    }
}
impl From<descriptors::Error> for Error {
    fn from(e: descriptors::Error) -> Self {
        Self::Descriptor(e)
    }
}
impl From<PolicyError> for Error {
    fn from(e: PolicyError) -> Self {
        Self::Policy(e)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Spawn(e) => e.fmt(f),
            Self::Inventory(e) => e.fmt(f),
            Self::Descriptor(e) => e.fmt(f),
            Self::Policy(e) => e.fmt(f),
            Self::Io { operation, source } => write!(f, "{operation}: {source}"),
            Self::Invalid(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

pub(crate) const FRAME: usize = 4096 + 8 * size_of::<Error>() + 4 * size_of::<fs::Stat>();
const MAPS_STORAGE: usize = policy::MAX_MAP_BYTES + size_of::<(Vec<u8>, usize)>();
const LOCAL_SCRATCH: usize = FRAME + MAPS_STORAGE;
const MAP_PARSE_WORK: usize = 8
    + policy::MAX_MAP_BYTES * 64
    + policy::MAX_EXECUTABLE_MAPPINGS
        * fe2o3_build_authority::COMPILER_RUNTIME_MANIFEST_MAX_ENTRIES_V1
        * policy::elf::MAX_PROGRAM_HEADERS
        * 16;
pub(crate) const IMAGE_WORK: usize = 8
    + Inventory::RANGE_SCOPE_WORK
    + View::PERSONALITY_WORK
    + View::MAPS_WORK
    + MAP_PARSE_WORK
    + NativeKernelImage::VALIDATE_WORK;
pub(crate) const IMAGE_SCRATCH: usize = LOCAL_SCRATCH
    + Inventory::FRAME
    + View::PERSONALITY_SCRATCH
    + View::MAPS_SCRATCH
    + MAPS_STORAGE
    + NativeKernelImage::VALIDATE_SCRATCH;
pub(crate) const MEMORY_WORK: usize = 8
    + Inventory::RANGE_SCOPE_WORK
    + View::PERSONALITY_WORK
    + View::DESCRIPTOR_WORK
    + descriptors::WORK
    + View::MAPS_WORK
    + MAP_PARSE_WORK
    + 16 * 1088;
pub(crate) const MEMORY_SCRATCH: usize = LOCAL_SCRATCH
    + Inventory::FRAME
    + View::PERSONALITY_SCRATCH
    + View::MAPS_SCRATCH
    + MAPS_STORAGE
    + View::FRAME
    + size_of::<(File, usize)>()
    + descriptors::FRAME;

/// Caller holds the original complete stopped census and immutable compiler
/// backing/write exclusion. The kernel image must come from an actual held exec
/// in this same original account. Success classifies this observation only;
/// the caller still owns resume policy, source/output exclusion and completion.
pub(crate) fn validate_stopped_image(
    view: &View<'_, '_>,
    backing: &Backing,
    inventory: &Inventory,
    kernel: &NativeKernelImage,
    b: &mut Budget<'_>,
) -> Result<()> {
    b.with_prepaid_scope(
        kernel.retained_storage(),
        8,
        8 + MAP_PARSE_WORK,
        LOCAL_SCRATCH,
        |b| {
            inventory.with_ranges(backing, b, |inventory, b| {
                validate_personality(view, b)?;
                with_maps(view, b, |maps, b| {
                    let interval = kernel.validate(view, maps, b)?;
                    policy::validate_native_x86_executable_mapping_rows(
                        maps,
                        inventory.ranges(),
                        &[interval],
                    )?;
                    Ok(())
                })
            })
        },
    )
}

/// Validate only an actual selected memory entry. This does not execute it:
/// the original owner must step just that task, validate its kernel exit/result,
/// recheck every stopped sharer's image, then decide whether any task may resume.
pub(crate) fn validate_memory_entry(
    entry: Entry,
    view: &View<'_, '_>,
    backing: &Backing,
    inventory: &Inventory,
    b: &mut Budget<'_>,
) -> Result<()> {
    b.with_prepaid_scope(0, 8, 8 + MAP_PARSE_WORK + 16 * 1088, LOCAL_SCRATCH, |b| {
        inventory.with_ranges(backing, b, |inventory, b| {
            validate_personality(view, b)?;
            match policy::memory::mapping_requirement(entry.number, entry.arguments)? {
                MappingRequirement::NoAddedExecution => Ok(()),
                MappingRequirement::ExecutableFile {
                    descriptor,
                    offset,
                    length,
                } => with_descriptor(view, descriptor, b, |file, b| {
                    descriptors::inspect(file.as_fd(), b)?;
                    let flags = fs::fcntl_getfl(file).map_err(|source| Error::Io {
                        operation: "inspect executable mapping descriptor access",
                        source,
                    })?;
                    let stat = fs::fstat(file).map_err(|source| Error::Io {
                        operation: "inspect executable mapping object",
                        source,
                    })?;
                    if flags.contains(fs::OFlags::PATH)
                        || flags & fs::OFlags::ACCMODE != fs::OFlags::RDONLY
                        || fs::FileType::from_raw_mode(stat.st_mode) != fs::FileType::RegularFile
                        || !policy::executable_object_range_is_allowed(
                            stat.st_dev,
                            stat.st_ino,
                            offset,
                            length,
                            inventory.ranges(),
                        )?
                    {
                        return Err(Error::Invalid(
                            "executable mapping descriptor or complete file range is not retained",
                        ));
                    }
                    Ok(())
                }),
                MappingRequirement::NonExecutableRegion { start, length } => {
                    with_maps(view, b, |maps, _| {
                        policy::validate_nonexecutable_mapping_rows(maps, start, length)?;
                        Ok(())
                    })
                }
            }
        })
    })
}

fn validate_personality(view: &View<'_, '_>, b: &mut Budget<'_>) -> Result<()> {
    let (bytes, length) = view.read_personality(b)?;
    let text = std::str::from_utf8(&bytes[..length])
        .map_err(|_| Error::Invalid("noncanonical task personality bytes"))?;
    policy::validate_personality(text)?;
    Ok(())
}

fn with_maps<R>(
    view: &View<'_, '_>,
    b: &mut Budget<'_>,
    operation: impl FnOnce(&str, &mut Budget<'_>) -> Result<R>,
) -> Result<R> {
    let (maps, storage) = view.read_maps(b)?;
    let charge = storage.additional_storage();
    b.reserve_storage(charge)?;
    let result = match std::str::from_utf8(&maps) {
        Ok(maps) => operation(maps, b),
        Err(_) => Err(Error::Invalid("noncanonical stopped task mapping bytes")),
    };
    drop(maps);
    let released = b.release_storage(charge);
    match result {
        Ok(value) => {
            released?;
            Ok(value)
        }
        Err(error) => Err(error),
    }
}

fn with_descriptor<R>(
    view: &View<'_, '_>,
    descriptor: i32,
    b: &mut Budget<'_>,
    operation: impl FnOnce(&File, &mut Budget<'_>) -> Result<R>,
) -> Result<R> {
    let (file, storage) = view.duplicate_descriptor(descriptor, b)?;
    let charge = storage.additional_storage();
    b.reserve_storage(charge)?;
    let result = operation(&file, b);
    drop(file);
    let released = b.release_storage(charge);
    match result {
        Ok(value) => {
            released?;
            Ok(value)
        }
        Err(error) => Err(error),
    }
}
