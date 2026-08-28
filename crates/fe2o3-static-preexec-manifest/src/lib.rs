#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

use std::error::Error;
use std::fmt;

/// Fixed descriptor carrying the sealed manifest into the static launcher.
pub const PREEXEC_MANIFEST_FD: i32 = 198;
/// Fixed descriptor carrying the sealed target executable into the launcher.
pub const PREEXEC_EXECUTABLE_FD: i32 = 199;
/// First fixed descriptor carrying a source object into the launcher.
pub const PREEXEC_SOURCE_FD_BASE: i32 = 200;
/// Maximum number of source-to-destination descriptor bindings in V1.
pub const PREEXEC_MAX_DESCRIPTORS: usize = 16;
/// Largest destination descriptor admitted by the static launcher.
pub const PREEXEC_MAX_DESTINATION_FD: i32 = 127;
/// V1 wire-format version.
pub const PREEXEC_MANIFEST_VERSION: u32 = 1;
/// V1 wire-format magic, including its terminal zero byte.
pub const PREEXEC_MANIFEST_MAGIC: [u8; 8] = *b"FE2PXM1\0";

/// Encoded size of one object identity.
pub const PREEXEC_OBJECT_IDENTITY_BYTES_V1: usize = 32;
/// Encoded size of one descriptor binding.
pub const PREEXEC_DESCRIPTOR_BYTES_V1: usize = 40;
/// Exact encoded size of a V1 manifest.
pub const PREEXEC_MANIFEST_BYTES_V1: usize = 704;

/// Byte offset of the magic field in a V1 manifest.
pub const MAGIC_OFFSET_V1: usize = 0;
/// Byte offset of the version field in a V1 manifest.
pub const VERSION_OFFSET_V1: usize = 8;
/// Byte offset of the descriptor-count field in a V1 manifest.
pub const DESCRIPTOR_COUNT_OFFSET_V1: usize = 12;
/// Byte offset of the parent-PID field in a V1 manifest.
pub const PARENT_PID_OFFSET_V1: usize = 16;
/// Byte offset of the manifest-level reserved field in a V1 manifest.
pub const MANIFEST_RESERVED_OFFSET_V1: usize = 20;
/// Byte offset of the parent-start-time field in a V1 manifest.
pub const PARENT_START_TIME_OFFSET_V1: usize = 24;
/// Byte offset of the executable object identity in a V1 manifest.
pub const EXECUTABLE_OFFSET_V1: usize = 32;
/// Byte offset of the first descriptor binding in a V1 manifest.
pub const DESCRIPTORS_OFFSET_V1: usize = 64;

const SOURCE_FD_OFFSET: usize = 0;
const DESTINATION_FD_OFFSET: usize = 4;
const DESCRIPTOR_OBJECT_OFFSET: usize = 8;

const OBJECT_DEVICE_OFFSET: usize = 0;
const OBJECT_INODE_OFFSET: usize = 8;
const OBJECT_SIZE_OFFSET: usize = 16;
const OBJECT_MODE_OFFSET: usize = 24;
const OBJECT_CLASS_OFFSET: usize = 28;

const STDIN_FILENO: i32 = 0;
const STDOUT_FILENO: i32 = 1;
const STDERR_FILENO: i32 = 2;
const MIN_PARENT_PID: i32 = 2;
const MIN_DESCRIPTOR_COUNT: usize = 3;

/// Stable validation failures for a V1 manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum StaticPreexecManifestErrorV1 {
    /// The input does not have the exact bounded V1 length.
    WrongLength {
        /// Required V1 byte length.
        expected: usize,
        /// Supplied byte length.
        actual: usize,
    },
    /// The wire magic is not the V1 magic.
    InvalidMagic,
    /// The wire version is not V1.
    UnsupportedVersion(u32),
    /// A manifest-level reserved byte is nonzero.
    NonzeroManifestReserved,
    /// The parent PID is not greater than one.
    InvalidParentPid(i32),
    /// The parent process start time is zero.
    ZeroParentStartTime,
    /// The descriptor count falls outside the inclusive range 3 through 16.
    InvalidDescriptorCount(usize),
    /// A descriptor index cannot be represented by the bounded V1 table.
    InvalidDescriptorIndex(usize),
    /// The executable object does not use ordinary `fstat` validation.
    InvalidExecutableObjectClass(u32),
    /// An active descriptor declares an unsupported object-validation class.
    InvalidDescriptorObjectClass {
        /// Index of the malformed active descriptor.
        index: usize,
        /// Unsupported encoded class.
        class: u32,
    },
    /// An inactive descriptor slot contains nonzero data.
    NonzeroInactiveDescriptor {
        /// Index of the malformed inactive descriptor.
        index: usize,
    },
    /// An active descriptor's source FD is not `200 + index`.
    SourceFdOutOfOrder {
        /// Descriptor-table index.
        index: usize,
        /// Required source FD for this index.
        expected: i32,
        /// Encoded source FD.
        actual: i32,
    },
    /// A destination FD falls outside the inclusive range 0 through 127.
    InvalidDestinationFd {
        /// Descriptor-table index.
        index: usize,
        /// Invalid destination FD.
        destination_fd: i32,
    },
    /// Two active descriptors target the same destination FD.
    DuplicateDestinationFd {
        /// Earlier descriptor-table index.
        first: usize,
        /// Later descriptor-table index.
        second: usize,
        /// Duplicated destination FD.
        destination_fd: i32,
    },
    /// One of descriptors 0, 1, or 2 is absent from the destination table.
    MissingStandardDescriptor(i32),
    /// A descriptor object aliases the executable by device and inode.
    ExecutableDescriptorAlias {
        /// Index of the aliasing descriptor.
        descriptor: usize,
    },
    /// Two descriptor objects alias each other by device and inode.
    DescriptorObjectAlias {
        /// Earlier descriptor-table index.
        first: usize,
        /// Later descriptor-table index.
        second: usize,
    },
    /// The executable object aliases the manifest object by device and inode.
    ExecutableManifestAlias,
    /// A descriptor object aliases the manifest object by device and inode.
    DescriptorManifestAlias {
        /// Index of the aliasing descriptor.
        descriptor: usize,
    },
}

impl fmt::Display for StaticPreexecManifestErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        use StaticPreexecManifestErrorV1 as ErrorV1;
        match self {
            ErrorV1::WrongLength { expected, actual } => {
                write!(
                    formatter,
                    "manifest length {actual} does not equal {expected}"
                )
            }
            ErrorV1::InvalidMagic => formatter.write_str("invalid V1 manifest magic"),
            ErrorV1::UnsupportedVersion(version) => {
                write!(formatter, "unsupported manifest version {version}")
            }
            ErrorV1::NonzeroManifestReserved => {
                formatter.write_str("manifest reserved field is nonzero")
            }
            ErrorV1::InvalidParentPid(pid) => write!(formatter, "invalid parent PID {pid}"),
            ErrorV1::ZeroParentStartTime => formatter.write_str("parent start time is zero"),
            ErrorV1::InvalidDescriptorCount(count) => {
                write!(formatter, "invalid descriptor count {count}")
            }
            ErrorV1::InvalidDescriptorIndex(index) => {
                write!(formatter, "invalid descriptor index {index}")
            }
            ErrorV1::InvalidExecutableObjectClass(class) => {
                write!(formatter, "invalid executable object class {class}")
            }
            ErrorV1::InvalidDescriptorObjectClass { index, class } => {
                write!(
                    formatter,
                    "descriptor {index} has unsupported object class {class}"
                )
            }
            ErrorV1::NonzeroInactiveDescriptor { index } => {
                write!(formatter, "inactive descriptor {index} is nonzero")
            }
            ErrorV1::SourceFdOutOfOrder {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "descriptor {index} source FD {actual} does not equal {expected}"
            ),
            ErrorV1::InvalidDestinationFd {
                index,
                destination_fd,
            } => write!(
                formatter,
                "descriptor {index} destination FD {destination_fd} is out of bounds"
            ),
            ErrorV1::DuplicateDestinationFd {
                first,
                second,
                destination_fd,
            } => write!(
                formatter,
                "descriptors {first} and {second} duplicate destination FD {destination_fd}"
            ),
            ErrorV1::MissingStandardDescriptor(fd) => {
                write!(formatter, "standard descriptor {fd} is missing")
            }
            ErrorV1::ExecutableDescriptorAlias { descriptor } => write!(
                formatter,
                "descriptor {descriptor} object aliases the executable"
            ),
            ErrorV1::DescriptorObjectAlias { first, second } => write!(
                formatter,
                "descriptor {second} object aliases descriptor {first}"
            ),
            ErrorV1::ExecutableManifestAlias => {
                formatter.write_str("executable object aliases the manifest object")
            }
            ErrorV1::DescriptorManifestAlias { descriptor } => write!(
                formatter,
                "descriptor {descriptor} object aliases the manifest object"
            ),
        }
    }
}

impl Error for StaticPreexecManifestErrorV1 {}

/// Kernel validation applied to one source-object snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum StaticPreexecObjectClassV1 {
    /// Validate the exact `fstat` snapshot and require a unique device/inode key.
    Fstat = 0,
    /// Validate the `fstat` snapshot and require a live Linux process pidfd.
    ProcessPidfd = 1,
}

impl StaticPreexecObjectClassV1 {
    const fn decode(encoded: u32) -> Option<Self> {
        match encoded {
            0 => Some(Self::Fstat),
            1 => Some(Self::ProcessPidfd),
            _ => None,
        }
    }
}

/// Immutable `st_dev`, `st_ino`, size, mode, and validation-class snapshot encoded by V1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticPreexecObjectIdentityV1 {
    device: u64,
    inode: u64,
    size: u64,
    mode: u32,
    class: StaticPreexecObjectClassV1,
}

impl StaticPreexecObjectIdentityV1 {
    /// Constructs an object identity from an already validated file snapshot.
    pub const fn new(device: u64, inode: u64, size: u64, mode: u32) -> Self {
        Self {
            device,
            inode,
            size,
            mode,
            class: StaticPreexecObjectClassV1::Fstat,
        }
    }

    /// Constructs an identity for a live process pidfd whose exact target is checked later.
    pub const fn new_process_pidfd(device: u64, inode: u64, size: u64, mode: u32) -> Self {
        Self {
            device,
            inode,
            size,
            mode,
            class: StaticPreexecObjectClassV1::ProcessPidfd,
        }
    }

    /// Returns the encoded device number.
    pub const fn device(&self) -> u64 {
        self.device
    }

    /// Returns the encoded inode number.
    pub const fn inode(&self) -> u64 {
        self.inode
    }

    /// Returns the encoded object size.
    pub const fn size(&self) -> u64 {
        self.size
    }

    /// Returns the encoded file mode.
    pub const fn mode(&self) -> u32 {
        self.mode
    }

    /// Returns the required kernel validation class.
    pub const fn class(&self) -> StaticPreexecObjectClassV1 {
        self.class
    }

    const fn has_same_key(&self, other: &Self) -> bool {
        self.device == other.device && self.inode == other.inode
    }

    const fn may_share_key_with(&self, other: &Self) -> bool {
        matches!(self.class, StaticPreexecObjectClassV1::ProcessPidfd)
            && matches!(other.class, StaticPreexecObjectClassV1::ProcessPidfd)
    }
}

/// One ordered V1 source-to-destination descriptor binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticPreexecDescriptorV1 {
    source_fd: i32,
    destination_fd: i32,
    object: StaticPreexecObjectIdentityV1,
}

impl StaticPreexecDescriptorV1 {
    /// Constructs the binding for `index`, deriving its required source FD.
    pub fn for_index(
        index: usize,
        destination_fd: i32,
        object: StaticPreexecObjectIdentityV1,
    ) -> Result<Self, StaticPreexecManifestErrorV1> {
        if index >= PREEXEC_MAX_DESCRIPTORS {
            return Err(StaticPreexecManifestErrorV1::InvalidDescriptorIndex(index));
        }
        if !(0..=PREEXEC_MAX_DESTINATION_FD).contains(&destination_fd) {
            return Err(StaticPreexecManifestErrorV1::InvalidDestinationFd {
                index,
                destination_fd,
            });
        }
        Ok(Self {
            source_fd: PREEXEC_SOURCE_FD_BASE + index as i32,
            destination_fd,
            object,
        })
    }

    /// Returns the ordered source FD, always `200 + descriptor index`.
    pub const fn source_fd(&self) -> i32 {
        self.source_fd
    }

    /// Returns the destination FD installed before target execution.
    pub const fn destination_fd(&self) -> i32 {
        self.destination_fd
    }

    /// Returns the required source-object identity.
    pub const fn object(&self) -> &StaticPreexecObjectIdentityV1 {
        &self.object
    }
}

/// A fully validated typed V1 static pre-exec launcher manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticPreexecManifestV1 {
    parent_pid: i32,
    parent_start_time: u64,
    executable: StaticPreexecObjectIdentityV1,
    descriptors: Vec<StaticPreexecDescriptorV1>,
}

impl StaticPreexecManifestV1 {
    /// Constructs and validates a typed V1 manifest.
    pub fn new(
        parent_pid: i32,
        parent_start_time: u64,
        executable: StaticPreexecObjectIdentityV1,
        descriptors: Vec<StaticPreexecDescriptorV1>,
    ) -> Result<Self, StaticPreexecManifestErrorV1> {
        let manifest = Self {
            parent_pid,
            parent_start_time,
            executable,
            descriptors,
        };
        manifest.validate()?;
        Ok(manifest)
    }

    /// Decodes and strictly validates one exact 704-byte little-endian record.
    pub fn decode(bytes: &[u8]) -> Result<Self, StaticPreexecManifestErrorV1> {
        if bytes.len() != PREEXEC_MANIFEST_BYTES_V1 {
            return Err(StaticPreexecManifestErrorV1::WrongLength {
                expected: PREEXEC_MANIFEST_BYTES_V1,
                actual: bytes.len(),
            });
        }
        if bytes[MAGIC_OFFSET_V1..VERSION_OFFSET_V1] != PREEXEC_MANIFEST_MAGIC {
            return Err(StaticPreexecManifestErrorV1::InvalidMagic);
        }
        let version = read_u32(bytes, VERSION_OFFSET_V1);
        if version != PREEXEC_MANIFEST_VERSION {
            return Err(StaticPreexecManifestErrorV1::UnsupportedVersion(version));
        }
        if read_u32(bytes, MANIFEST_RESERVED_OFFSET_V1) != 0 {
            return Err(StaticPreexecManifestErrorV1::NonzeroManifestReserved);
        }

        let descriptor_count = read_u32(bytes, DESCRIPTOR_COUNT_OFFSET_V1) as usize;
        if !(MIN_DESCRIPTOR_COUNT..=PREEXEC_MAX_DESCRIPTORS).contains(&descriptor_count) {
            return Err(StaticPreexecManifestErrorV1::InvalidDescriptorCount(
                descriptor_count,
            ));
        }
        let parent_pid = read_i32(bytes, PARENT_PID_OFFSET_V1);
        let parent_start_time = read_u64(bytes, PARENT_START_TIME_OFFSET_V1);
        let executable_class = read_u32(bytes, EXECUTABLE_OFFSET_V1 + OBJECT_CLASS_OFFSET);
        if executable_class != StaticPreexecObjectClassV1::Fstat as u32 {
            return Err(StaticPreexecManifestErrorV1::InvalidExecutableObjectClass(
                executable_class,
            ));
        }
        let executable = decode_object(
            bytes,
            EXECUTABLE_OFFSET_V1,
            StaticPreexecObjectClassV1::Fstat,
        );

        let mut descriptors = Vec::with_capacity(descriptor_count);
        for index in 0..descriptor_count {
            let offset = descriptor_offset(index);
            let encoded_class = read_u32(
                bytes,
                offset + DESCRIPTOR_OBJECT_OFFSET + OBJECT_CLASS_OFFSET,
            );
            let class = StaticPreexecObjectClassV1::decode(encoded_class).ok_or(
                StaticPreexecManifestErrorV1::InvalidDescriptorObjectClass {
                    index,
                    class: encoded_class,
                },
            )?;
            descriptors.push(StaticPreexecDescriptorV1 {
                source_fd: read_i32(bytes, offset + SOURCE_FD_OFFSET),
                destination_fd: read_i32(bytes, offset + DESTINATION_FD_OFFSET),
                object: decode_object(bytes, offset + DESCRIPTOR_OBJECT_OFFSET, class),
            });
        }
        for index in descriptor_count..PREEXEC_MAX_DESCRIPTORS {
            let offset = descriptor_offset(index);
            if bytes[offset..offset + PREEXEC_DESCRIPTOR_BYTES_V1]
                .iter()
                .any(|byte| *byte != 0)
            {
                return Err(StaticPreexecManifestErrorV1::NonzeroInactiveDescriptor { index });
            }
        }

        Self::new(parent_pid, parent_start_time, executable, descriptors)
    }

    /// Encodes this validated value into the exact C-compatible V1 record.
    pub fn encode(&self) -> [u8; PREEXEC_MANIFEST_BYTES_V1] {
        let mut bytes = [0_u8; PREEXEC_MANIFEST_BYTES_V1];
        bytes[MAGIC_OFFSET_V1..VERSION_OFFSET_V1].copy_from_slice(&PREEXEC_MANIFEST_MAGIC);
        write_u32(&mut bytes, VERSION_OFFSET_V1, PREEXEC_MANIFEST_VERSION);
        write_u32(
            &mut bytes,
            DESCRIPTOR_COUNT_OFFSET_V1,
            self.descriptors.len() as u32,
        );
        write_i32(&mut bytes, PARENT_PID_OFFSET_V1, self.parent_pid);
        write_u64(
            &mut bytes,
            PARENT_START_TIME_OFFSET_V1,
            self.parent_start_time,
        );
        encode_object(&mut bytes, EXECUTABLE_OFFSET_V1, &self.executable);
        for (index, descriptor) in self.descriptors.iter().enumerate() {
            let offset = descriptor_offset(index);
            write_i32(&mut bytes, offset + SOURCE_FD_OFFSET, descriptor.source_fd);
            write_i32(
                &mut bytes,
                offset + DESTINATION_FD_OFFSET,
                descriptor.destination_fd,
            );
            encode_object(
                &mut bytes,
                offset + DESCRIPTOR_OBJECT_OFFSET,
                &descriptor.object,
            );
        }
        bytes
    }

    /// Returns the exact parent PID bound by this record.
    pub const fn parent_pid(&self) -> i32 {
        self.parent_pid
    }

    /// Returns the exact `/proc/<pid>/stat` start-time field bound by this record.
    pub const fn parent_start_time(&self) -> u64 {
        self.parent_start_time
    }

    /// Returns the required sealed executable identity.
    pub const fn executable(&self) -> &StaticPreexecObjectIdentityV1 {
        &self.executable
    }

    /// Returns the ordered active descriptor table.
    pub fn descriptors(&self) -> &[StaticPreexecDescriptorV1] {
        &self.descriptors
    }

    /// Rejects an external manifest-file identity that aliases any carried object.
    ///
    /// The manifest file's identity is not self-referentially encoded. A caller
    /// with its `fstat` snapshot must invoke this check to mirror the launcher's
    /// manifest/executable/source alias rules.
    pub fn validate_manifest_object(
        &self,
        manifest_object: &StaticPreexecObjectIdentityV1,
    ) -> Result<(), StaticPreexecManifestErrorV1> {
        if self.executable.has_same_key(manifest_object) {
            return Err(StaticPreexecManifestErrorV1::ExecutableManifestAlias);
        }
        if let Some(descriptor) = self
            .descriptors
            .iter()
            .position(|entry| entry.object.has_same_key(manifest_object))
        {
            return Err(StaticPreexecManifestErrorV1::DescriptorManifestAlias { descriptor });
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), StaticPreexecManifestErrorV1> {
        if self.executable.class != StaticPreexecObjectClassV1::Fstat {
            return Err(StaticPreexecManifestErrorV1::InvalidExecutableObjectClass(
                self.executable.class as u32,
            ));
        }
        if self.parent_pid < MIN_PARENT_PID {
            return Err(StaticPreexecManifestErrorV1::InvalidParentPid(
                self.parent_pid,
            ));
        }
        if self.parent_start_time == 0 {
            return Err(StaticPreexecManifestErrorV1::ZeroParentStartTime);
        }
        if !(MIN_DESCRIPTOR_COUNT..=PREEXEC_MAX_DESCRIPTORS).contains(&self.descriptors.len()) {
            return Err(StaticPreexecManifestErrorV1::InvalidDescriptorCount(
                self.descriptors.len(),
            ));
        }

        let mut destinations = [None; PREEXEC_MAX_DESTINATION_FD as usize + 1];
        for (index, descriptor) in self.descriptors.iter().enumerate() {
            let expected = PREEXEC_SOURCE_FD_BASE + index as i32;
            if descriptor.source_fd != expected {
                return Err(StaticPreexecManifestErrorV1::SourceFdOutOfOrder {
                    index,
                    expected,
                    actual: descriptor.source_fd,
                });
            }
            if !(0..=PREEXEC_MAX_DESTINATION_FD).contains(&descriptor.destination_fd) {
                return Err(StaticPreexecManifestErrorV1::InvalidDestinationFd {
                    index,
                    destination_fd: descriptor.destination_fd,
                });
            }
            let destination = descriptor.destination_fd as usize;
            if let Some(first) = destinations[destination] {
                return Err(StaticPreexecManifestErrorV1::DuplicateDestinationFd {
                    first,
                    second: index,
                    destination_fd: descriptor.destination_fd,
                });
            }
            destinations[destination] = Some(index);
            if descriptor.object.has_same_key(&self.executable) {
                return Err(StaticPreexecManifestErrorV1::ExecutableDescriptorAlias {
                    descriptor: index,
                });
            }
            if let Some(first) = self.descriptors[..index].iter().position(|previous| {
                descriptor.object.has_same_key(&previous.object)
                    && !descriptor.object.may_share_key_with(&previous.object)
            }) {
                return Err(StaticPreexecManifestErrorV1::DescriptorObjectAlias {
                    first,
                    second: index,
                });
            }
        }
        for standard_fd in [STDIN_FILENO, STDOUT_FILENO, STDERR_FILENO] {
            if destinations[standard_fd as usize].is_none() {
                return Err(StaticPreexecManifestErrorV1::MissingStandardDescriptor(
                    standard_fd,
                ));
            }
        }
        Ok(())
    }
}

const fn descriptor_offset(index: usize) -> usize {
    DESCRIPTORS_OFFSET_V1 + index * PREEXEC_DESCRIPTOR_BYTES_V1
}

fn decode_object(
    bytes: &[u8],
    offset: usize,
    class: StaticPreexecObjectClassV1,
) -> StaticPreexecObjectIdentityV1 {
    StaticPreexecObjectIdentityV1 {
        device: read_u64(bytes, offset + OBJECT_DEVICE_OFFSET),
        inode: read_u64(bytes, offset + OBJECT_INODE_OFFSET),
        size: read_u64(bytes, offset + OBJECT_SIZE_OFFSET),
        mode: read_u32(bytes, offset + OBJECT_MODE_OFFSET),
        class,
    }
}

fn encode_object(bytes: &mut [u8], offset: usize, object: &StaticPreexecObjectIdentityV1) {
    write_u64(bytes, offset + OBJECT_DEVICE_OFFSET, object.device);
    write_u64(bytes, offset + OBJECT_INODE_OFFSET, object.inode);
    write_u64(bytes, offset + OBJECT_SIZE_OFFSET, object.size);
    write_u32(bytes, offset + OBJECT_MODE_OFFSET, object.mode);
    write_u32(bytes, offset + OBJECT_CLASS_OFFSET, object.class as u32);
}

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(
        bytes[offset..offset + size_of::<i32>()]
            .try_into()
            .expect("fixed-size field is in the validated manifest bounds"),
    )
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        bytes[offset..offset + size_of::<u32>()]
            .try_into()
            .expect("fixed-size field is in the validated manifest bounds"),
    )
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + size_of::<u64>()]
            .try_into()
            .expect("fixed-size field is in the validated manifest bounds"),
    )
}

fn write_i32(bytes: &mut [u8], offset: usize, value: i32) {
    bytes[offset..offset + size_of::<i32>()].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + size_of::<u32>()].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + size_of::<u64>()].copy_from_slice(&value.to_le_bytes());
}
