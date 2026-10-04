use crate::digest::{Sha256Digest, sha256_bytes};
use crate::error::{HostLinkError, HostLinkErrorCodeV1, ResultContext};
use crate::model::{ElfClassV1, ElfEndianV1, ElfProfileV1, HostArtifactKindV1};
use crate::{
    MAX_HOST_LINK_INPUT_BYTES_V1, MAX_HOST_LINK_OUTPUT_BYTES_V1, MAX_HOST_LINK_PLAN_BYTES_V1,
};
use std::fs::File;

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use object::{
        Endianness, Object, ObjectSection, ObjectSegment, ObjectSymbol, SectionFlags, elf,
        read::archive::ArchiveFile,
        read::elf::{ElfFile64, FileHeader, ProgramHeader, SectionHeader},
    };
    use rustix::fs::{MemfdFlags, SealFlags};
    use sha2::{Digest, Sha256};
    use std::collections::{BTreeMap, BTreeSet};
    use std::io::Write;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::time::Instant;

    const REQUIRED_SEALS: SealFlags = SealFlags::WRITE
        .union(SealFlags::GROW)
        .union(SealFlags::SHRINK)
        .union(SealFlags::SEAL);
    const COPY_CHUNK_BYTES: usize = 64 * 1024;
    const MAX_ARCHIVE_MEMBERS_V1: usize = 8192;
    const MAX_ARCHIVE_MEMBER_NAME_BYTES_V1: usize = 1024 * 1024;
    const TMPFS_MAGIC: u64 = 0x0102_1994;
    // LLVM's address-significance table is intentionally absent from object 0.39's
    // public ELF constants. It is an inert, excluded section emitted by pinned rustc.
    const SHT_LLVM_ADDRSIG_V1: u32 = 0x6fff_4c03;

    const SHF_WRITE_V1: u64 = elf::SHF_WRITE as u64;
    const SHF_ALLOC_V1: u64 = elf::SHF_ALLOC as u64;
    const SHF_EXECINSTR_V1: u64 = elf::SHF_EXECINSTR as u64;
    const SHF_MERGE_V1: u64 = elf::SHF_MERGE as u64;
    const SHF_STRINGS_V1: u64 = elf::SHF_STRINGS as u64;
    const SHF_INFO_LINK_V1: u64 = elf::SHF_INFO_LINK as u64;
    const SHF_LINK_ORDER_V1: u64 = elf::SHF_LINK_ORDER as u64;
    const SHF_GROUP_V1: u64 = elf::SHF_GROUP as u64;
    const SHF_TLS_V1: u64 = elf::SHF_TLS as u64;
    const SHF_GNU_RETAIN_V1: u64 = elf::SHF_GNU_RETAIN as u64;
    const SHF_EXCLUDE_V1: u64 = elf::SHF_EXCLUDE as u64;

    pub(crate) struct ArtifactInspectionV1 {
        pub elf_profile: Option<ElfProfileV1>,
        pub archive_members: u64,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct FileSnapshot {
        device: u64,
        inode: u64,
        mode: u32,
        size: u64,
        modified_seconds: i64,
        modified_nanoseconds: i64,
        changed_seconds: i64,
        changed_nanoseconds: i64,
        link_count: u64,
    }

    impl FileSnapshot {
        fn capture(file: &File, context: &str) -> Result<Self, HostLinkError> {
            let metadata = file.metadata().context(HostLinkErrorCodeV1::Io, || {
                format!("inspect {context} descriptor")
            })?;
            if !metadata.file_type().is_file() {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::NotRegular,
                    format!("{context} descriptor is not a regular file"),
                ));
            }
            Ok(Self {
                device: metadata.dev(),
                inode: metadata.ino(),
                mode: metadata.mode(),
                size: metadata.len(),
                modified_seconds: metadata.mtime(),
                modified_nanoseconds: metadata.mtime_nsec(),
                changed_seconds: metadata.ctime(),
                changed_nanoseconds: metadata.ctime_nsec(),
                link_count: metadata.nlink(),
            })
        }
    }

    pub(crate) struct CapturedFile {
        pub file: File,
        pub sha256: Sha256Digest,
        pub size: u64,
        pub mode: u32,
        pub bytes: Vec<u8>,
    }

    pub(crate) struct CapturedOutputFileV1 {
        pub file: File,
        pub sha256: Sha256Digest,
        pub size: u64,
        pub mode: u32,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct ReceivedMemfdIdentity {
        device: u64,
        inode: u64,
        size: u64,
        owner_uid: u32,
    }

    enum IncrementalOutputCopyPhaseV1 {
        CopySender,
        VerifyReceiver,
    }

    pub(crate) struct IncrementalOutputCopyV1 {
        source: File,
        source_identity: ReceivedMemfdIdentity,
        receiver: File,
        size: u64,
        offset: u64,
        source_digest: Sha256,
        source_sha256: Option<Sha256Digest>,
        receiver_digest: Sha256,
        phase: IncrementalOutputCopyPhaseV1,
    }

    pub(crate) enum IncrementalOutputCopyProgressV1 {
        Pending(Box<IncrementalOutputCopyV1>),
        Complete(CapturedOutputFileV1),
    }

    #[derive(Clone, Copy)]
    struct OutputSectionV1 {
        name: u32,
        section_type: u32,
        flags: u64,
        address: u64,
        offset: u64,
        size: u64,
        link: usize,
        information: usize,
        alignment: u64,
        entry_size: u64,
    }

    enum IncrementalOutputInspectionPhaseV1 {
        ProgramHeaders,
        SectionHeaders,
        CrossValidateSections,
        ValidateSectionContents,
    }

    #[derive(Clone, Copy)]
    struct LoadSegmentV1 {
        file_offset: u64,
        file_end: u64,
        virtual_address: u64,
        memory_end: u64,
        flags: u32,
    }

    #[derive(Default)]
    struct IntervalSetV1 {
        by_start: BTreeMap<u64, u64>,
    }

    impl IntervalSetV1 {
        fn insert(
            &mut self,
            start: u64,
            end: u64,
            code: HostLinkErrorCodeV1,
            name: &str,
        ) -> Result<(), HostLinkError> {
            if start == end {
                return Ok(());
            }
            if self
                .by_start
                .range(..=start)
                .next_back()
                .is_some_and(|(_, previous_end)| *previous_end > start)
                || self
                    .by_start
                    .range(start..)
                    .next()
                    .is_some_and(|(next_start, _)| *next_start < end)
            {
                return Err(HostLinkError::new(
                    code,
                    format!("ELF {name} ranges overlap"),
                ));
            }
            self.by_start.insert(start, end);
            Ok(())
        }
    }

    pub(crate) struct IncrementalStaticOutputInspectionV1 {
        output: CapturedOutputFileV1,
        entry: u64,
        program_offset: u64,
        program_count: usize,
        section_offset: u64,
        section_count: usize,
        section_name_index: usize,
        index: usize,
        phase: IncrementalOutputInspectionPhaseV1,
        executable_loads: Vec<(u64, u64, u64)>,
        load_file_ranges: IntervalSetV1,
        load_memory_ranges: IntervalSetV1,
        loads_by_address: BTreeMap<u64, LoadSegmentV1>,
        section_file_ranges: IntervalSetV1,
        sections: Vec<OutputSectionV1>,
        content_offset: u64,
        has_writable_executable_segment: bool,
        has_executable_stack: bool,
    }

    pub(crate) enum IncrementalOutputInspectionProgressV1 {
        Pending(IncrementalStaticOutputInspectionV1),
        Complete(CapturedOutputFileV1, ElfProfileV1),
    }

    impl IncrementalOutputCopyV1 {
        pub(crate) fn new(
            source: File,
            name: &str,
            limit: u64,
            expected_size: u64,
        ) -> Result<Self, HostLinkError> {
            let source_identity = validate_received_memfd_identity(&source, name)?;
            if source_identity.size == 0 || source_identity.size > limit {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactTooLarge,
                    format!("sender-owned {name} size is outside its admitted bound"),
                ));
            }
            if source_identity.size != expected_size {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::DigestMismatch,
                    format!("sender-owned {name} size does not match the result record"),
                ));
            }
            let descriptor = rustix::fs::memfd_create(
                "fe2o3-host-link-admitted-output-v1",
                MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
            )
            .context(HostLinkErrorCodeV1::Io, || {
                format!("create receiver-owned snapshot for {name}")
            })?;
            Ok(Self {
                source,
                source_identity,
                receiver: File::from(descriptor),
                size: expected_size,
                offset: 0,
                source_digest: Sha256::new(),
                source_sha256: None,
                receiver_digest: Sha256::new(),
                phase: IncrementalOutputCopyPhaseV1::CopySender,
            })
        }

        pub(crate) fn advance(
            mut self,
            maximum_bytes: u64,
            absolute_deadline: Instant,
            quantum_deadline: Instant,
        ) -> Result<IncrementalOutputCopyProgressV1, HostLinkError> {
            if maximum_bytes == 0 {
                return Ok(IncrementalOutputCopyProgressV1::Pending(Box::new(self)));
            }
            let mut remaining_budget = maximum_bytes;
            let mut buffer = [0_u8; COPY_CHUNK_BYTES];
            while remaining_budget != 0 {
                ensure_copy_deadline(absolute_deadline)?;
                if Instant::now() >= quantum_deadline {
                    return Ok(IncrementalOutputCopyProgressV1::Pending(Box::new(self)));
                }
                let remaining_file = self.size - self.offset;
                if remaining_file == 0 {
                    match self.phase {
                        IncrementalOutputCopyPhaseV1::CopySender => {
                            self.finish_sender_copy()?;
                            self.phase = IncrementalOutputCopyPhaseV1::VerifyReceiver;
                            self.offset = 0;
                            continue;
                        }
                        IncrementalOutputCopyPhaseV1::VerifyReceiver => {
                            return self.finish_receiver_verification();
                        }
                    }
                }
                let count = usize::try_from(
                    remaining_file
                        .min(remaining_budget)
                        .min(COPY_CHUNK_BYTES as u64),
                )
                .expect("bounded output copy chunk fits usize");
                let source = match self.phase {
                    IncrementalOutputCopyPhaseV1::CopySender => &self.source,
                    IncrementalOutputCopyPhaseV1::VerifyReceiver => &self.receiver,
                };
                let read = rustix::io::pread(source, &mut buffer[..count], self.offset)
                    .context(HostLinkErrorCodeV1::Io, || {
                        "incrementally read sealed host-link output".to_owned()
                    })?;
                if read == 0 {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::OutputTruncated,
                        "sealed host-link output ended before its bound length",
                    ));
                }
                match self.phase {
                    IncrementalOutputCopyPhaseV1::CopySender => {
                        pwrite_all(&self.receiver, &buffer[..read], self.offset)?;
                        self.source_digest.update(&buffer[..read]);
                    }
                    IncrementalOutputCopyPhaseV1::VerifyReceiver => {
                        self.receiver_digest.update(&buffer[..read]);
                    }
                }
                self.offset = self.offset.checked_add(read as u64).ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::ArtifactTooLarge,
                        "incremental host-link output offset overflowed",
                    )
                })?;
                remaining_budget -= read as u64;
                ensure_copy_deadline(absolute_deadline)?;
                if Instant::now() >= quantum_deadline {
                    return Ok(IncrementalOutputCopyProgressV1::Pending(Box::new(self)));
                }
            }
            Ok(IncrementalOutputCopyProgressV1::Pending(Box::new(self)))
        }

        fn finish_sender_copy(&mut self) -> Result<(), HostLinkError> {
            ensure_no_extra_bytes(&self.source, self.size, "sender-owned host-link output")?;
            if validate_received_memfd_identity(&self.source, "host-link result output")?
                != self.source_identity
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::DescriptorChanged,
                    "sender-owned host-link output identity changed during receiver copy",
                ));
            }
            let source_digest = std::mem::take(&mut self.source_digest).finalize();
            self.source_sha256 = Some(Sha256Digest::from_bytes(source_digest.into()));
            self.receiver
                .set_permissions(std::fs::Permissions::from_mode(0o555))
                .context(HostLinkErrorCodeV1::Io, || {
                    "canonicalize receiver-owned host-link output mode".to_owned()
                })?;
            rustix::fs::fcntl_add_seals(&self.receiver, REQUIRED_SEALS)
                .context(HostLinkErrorCodeV1::Io, || {
                    "seal receiver-owned host-link output".to_owned()
                })?;
            verify_exact_seals(&self.receiver, "receiver-owned host-link output")
        }

        fn finish_receiver_verification(
            self,
        ) -> Result<IncrementalOutputCopyProgressV1, HostLinkError> {
            ensure_no_extra_bytes(&self.receiver, self.size, "receiver-owned host-link output")?;
            if validate_received_memfd_identity(&self.source, "host-link result output")?
                != self.source_identity
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::DescriptorChanged,
                    "sender-owned host-link output identity changed after receiver copy",
                ));
            }
            let source_sha256 = self
                .source_sha256
                .expect("sender digest is finalized before receiver verification");
            let receiver_sha256 = Sha256Digest::from_bytes(self.receiver_digest.finalize().into());
            if receiver_sha256 != source_sha256 {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::DescriptorChanged,
                    "receiver-owned host-link output differs from sender-owned sealed bytes",
                ));
            }
            verify_sealed_artifact_identity(
                &self.receiver,
                self.size,
                0o555,
                "receiver-owned host-link output",
            )?;
            Ok(IncrementalOutputCopyProgressV1::Complete(
                CapturedOutputFileV1 {
                    file: self.receiver,
                    sha256: source_sha256,
                    size: self.size,
                    mode: 0o555,
                },
            ))
        }

        #[cfg(test)]
        pub(crate) const fn bytes_processed(&self) -> u64 {
            self.offset
        }
    }

    impl IncrementalStaticOutputInspectionV1 {
        pub(crate) fn new(output: CapturedOutputFileV1) -> Result<Self, HostLinkError> {
            let mut header = [0_u8; 64];
            pread_exact_at(&output.file, &mut header, 0, "static output ELF header")?;
            if header[..4] != *b"\x7fELF"
                || header[4] != ElfClassV1::Elf64 as u8
                || header[5] != ElfEndianV1::Little as u8
                || header[6] != 1
                || u16_at(&header, 16) != elf::ET_EXEC
                || u16_at(&header, 18) != elf::EM_X86_64
                || u32_at(&header, 20) != 1
                || u32_at(&header, 48) != 0
                || u16_at(&header, 52) != 64
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "admitted output is not canonical x86_64 ELF64 little-endian ET_EXEC",
                ));
            }
            let entry = u64_at(&header, 24);
            let program_offset = u64_at(&header, 32);
            let section_offset = u64_at(&header, 40);
            let program_entry_size = u16_at(&header, 54);
            let program_count = u16_at(&header, 56) as usize;
            let section_entry_size = u16_at(&header, 58);
            let section_count = u16_at(&header, 60) as usize;
            let section_name_index = u16_at(&header, 62) as usize;
            if program_count == usize::from(elf::PN_XNUM)
                || section_name_index == usize::from(elf::SHN_XINDEX)
                || program_count == 0
                || program_count > crate::MAX_HOST_LINK_ELF_PROGRAM_HEADERS_V1
                || section_count > crate::MAX_HOST_LINK_ELF_SECTIONS_V1
                || program_entry_size != 56
                || !program_offset.is_multiple_of(8)
                || (section_count == 0 && (section_offset != 0 || section_name_index != 0))
                || (section_count != 0
                    && (section_entry_size != 64
                        || section_name_index >= section_count
                        || !section_offset.is_multiple_of(8)))
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output uses malformed or unsupported extended ELF table counts",
                ));
            }
            let program_end = checked_table_end(
                program_offset,
                program_count,
                56,
                output.size,
                "program-header",
            )?;
            let section_end = checked_table_end(
                section_offset,
                section_count,
                64,
                output.size,
                "section-header",
            )?;
            if program_offset < 64
                || (section_count != 0 && section_offset < 64)
                || ranges_overlap(program_offset, program_end, section_offset, section_end)
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output ELF header tables overlap or cover the ELF header",
                ));
            }
            let mut section_file_ranges = IntervalSetV1::default();
            section_file_ranges.insert(0, 64, HostLinkErrorCodeV1::ElfPolicy, "file")?;
            section_file_ranges.insert(
                program_offset,
                program_end,
                HostLinkErrorCodeV1::ElfPolicy,
                "file",
            )?;
            if section_count != 0 {
                section_file_ranges.insert(
                    section_offset,
                    section_end,
                    HostLinkErrorCodeV1::ElfPolicy,
                    "file",
                )?;
            }
            Ok(Self {
                output,
                entry,
                program_offset,
                program_count,
                section_offset,
                section_count,
                section_name_index,
                index: 0,
                phase: IncrementalOutputInspectionPhaseV1::ProgramHeaders,
                executable_loads: Vec::new(),
                load_file_ranges: IntervalSetV1::default(),
                load_memory_ranges: IntervalSetV1::default(),
                loads_by_address: BTreeMap::new(),
                section_file_ranges,
                sections: Vec::with_capacity(section_count),
                content_offset: 0,
                has_writable_executable_segment: false,
                has_executable_stack: false,
            })
        }

        pub(crate) fn advance(
            mut self,
            maximum_operations: usize,
            absolute_deadline: Instant,
            quantum_deadline: Instant,
        ) -> Result<IncrementalOutputInspectionProgressV1, HostLinkError> {
            let mut operations = 0;
            while operations < maximum_operations {
                ensure_copy_deadline(absolute_deadline)?;
                if Instant::now() >= quantum_deadline {
                    return Ok(IncrementalOutputInspectionProgressV1::Pending(self));
                }
                match self.phase {
                    IncrementalOutputInspectionPhaseV1::ProgramHeaders => {
                        if self.index == self.program_count {
                            self.finish_program_headers()?;
                            self.phase = IncrementalOutputInspectionPhaseV1::SectionHeaders;
                            self.index = 0;
                            continue;
                        }
                        self.inspect_program_header()?;
                    }
                    IncrementalOutputInspectionPhaseV1::SectionHeaders => {
                        if self.index == self.section_count {
                            self.finish_section_headers()?;
                            self.phase = IncrementalOutputInspectionPhaseV1::CrossValidateSections;
                            self.index = 0;
                            continue;
                        }
                        self.inspect_section_header()?;
                    }
                    IncrementalOutputInspectionPhaseV1::CrossValidateSections => {
                        if self.index == self.section_count {
                            self.phase =
                                IncrementalOutputInspectionPhaseV1::ValidateSectionContents;
                            self.index = 0;
                            self.content_offset = 0;
                            continue;
                        }
                        self.cross_validate_section()?;
                    }
                    IncrementalOutputInspectionPhaseV1::ValidateSectionContents => {
                        if self.index == self.section_count {
                            let profile = ElfProfileV1 {
                                class: ElfClassV1::Elf64,
                                endian: ElfEndianV1::Little,
                                elf_type: elf::ET_EXEC,
                                machine: elf::EM_X86_64,
                                interpreter: None,
                                soname: None,
                                needed: Vec::new(),
                                has_writable_executable_segment: self
                                    .has_writable_executable_segment,
                                has_executable_stack: self.has_executable_stack,
                            };
                            return Ok(IncrementalOutputInspectionProgressV1::Complete(
                                self.output,
                                profile,
                            ));
                        }
                        if !self.validate_section_content_entry()? {
                            operations += 1;
                            ensure_copy_deadline(absolute_deadline)?;
                            continue;
                        }
                    }
                }
                self.index += 1;
                self.content_offset = 0;
                operations += 1;
                ensure_copy_deadline(absolute_deadline)?;
                if Instant::now() >= quantum_deadline {
                    return Ok(IncrementalOutputInspectionProgressV1::Pending(self));
                }
            }
            Ok(IncrementalOutputInspectionProgressV1::Pending(self))
        }

        fn inspect_program_header(&mut self) -> Result<(), HostLinkError> {
            let offset = indexed_offset(self.program_offset, self.index, 56)?;
            let mut header = [0_u8; 56];
            pread_exact_at(
                &self.output.file,
                &mut header,
                offset,
                "static output program header",
            )?;
            let segment_type = u32_at(&header, 0);
            let flags = u32_at(&header, 4);
            let file_offset = u64_at(&header, 8);
            let virtual_address = u64_at(&header, 16);
            let file_size = u64_at(&header, 32);
            let memory_size = u64_at(&header, 40);
            let alignment = u64_at(&header, 48);
            if flags & !(elf::PF_R | elf::PF_W | elf::PF_X) != 0
                || file_size > memory_size
                || (alignment > 1
                    && (!alignment.is_power_of_two()
                        || file_offset % alignment != virtual_address % alignment))
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output segment size or alignment is malformed",
                ));
            }
            let file_end = file_offset.checked_add(file_size).ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output segment file range overflows",
                )
            })?;
            let memory_end = virtual_address.checked_add(memory_size).ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output segment memory range overflows",
                )
            })?;
            if file_end > self.output.size {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output segment extends beyond the sealed file",
                ));
            }
            if matches!(segment_type, elf::PT_INTERP | elf::PT_DYNAMIC) {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output contains a loader or dynamic segment",
                ));
            }
            if !matches!(
                segment_type,
                elf::PT_NULL
                    | elf::PT_LOAD
                    | elf::PT_PHDR
                    | elf::PT_NOTE
                    | elf::PT_TLS
                    | elf::PT_GNU_EH_FRAME
                    | elf::PT_GNU_STACK
                    | elf::PT_GNU_RELRO
            ) {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output contains an unsupported program-header type",
                ));
            }
            if segment_type == elf::PT_GNU_STACK && flags & elf::PF_X != 0 {
                self.has_executable_stack = true;
            }
            if segment_type == elf::PT_LOAD {
                if flags & elf::PF_W != 0 && flags & elf::PF_X != 0 {
                    self.has_writable_executable_segment = true;
                }
                self.load_file_ranges.insert(
                    file_offset,
                    file_end,
                    HostLinkErrorCodeV1::ElfPolicy,
                    "file",
                )?;
                self.load_memory_ranges.insert(
                    virtual_address,
                    memory_end,
                    HostLinkErrorCodeV1::ElfPolicy,
                    "memory",
                )?;
                if memory_size != 0
                    && self
                        .loads_by_address
                        .insert(
                            virtual_address,
                            LoadSegmentV1 {
                                file_offset,
                                file_end,
                                virtual_address,
                                memory_end,
                                flags,
                            },
                        )
                        .is_some()
                {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::ElfPolicy,
                        "static output has duplicate PT_LOAD virtual addresses",
                    ));
                }
                if flags & elf::PF_X != 0 && file_size != 0 {
                    self.executable_loads
                        .push((virtual_address, file_size, memory_size));
                }
            }
            Ok(())
        }

        fn finish_program_headers(&self) -> Result<(), HostLinkError> {
            if self.entry == 0
                || !self
                    .executable_loads
                    .iter()
                    .any(|(virtual_address, file_size, memory_size)| {
                        self.entry
                            .checked_sub(*virtual_address)
                            .is_some_and(|delta| delta < *file_size && delta < *memory_size)
                    })
                || self.has_writable_executable_segment
                || self.has_executable_stack
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output entry/load/WX/stack policy is not satisfied",
                ));
            }
            Ok(())
        }

        fn inspect_section_header(&mut self) -> Result<(), HostLinkError> {
            let offset = indexed_offset(self.section_offset, self.index, 64)?;
            let mut header = [0_u8; 64];
            pread_exact_at(
                &self.output.file,
                &mut header,
                offset,
                "static output section header",
            )?;
            let section = OutputSectionV1 {
                name: u32_at(&header, 0),
                section_type: u32_at(&header, 4),
                flags: u64_at(&header, 8),
                address: u64_at(&header, 16),
                offset: u64_at(&header, 24),
                size: u64_at(&header, 32),
                link: u32_at(&header, 40) as usize,
                information: u32_at(&header, 44) as usize,
                alignment: u64_at(&header, 48),
                entry_size: u64_at(&header, 56),
            };
            if self.index == 0
                && (section.name != 0
                    || section.section_type != elf::SHT_NULL
                    || section.flags != 0
                    || section.address != 0
                    || section.offset != 0
                    || section.size != 0
                    || section.link != 0
                    || section.information != 0
                    || section.alignment != 0
                    || section.entry_size != 0)
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output section zero is not the exact null section",
                ));
            }
            if self.index != 0 && section.section_type == elf::SHT_NULL {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output contains a nonzero null section",
                ));
            }
            if section.flags & u64::from(elf::SHF_COMPRESSED) != 0
                || section.section_type == elf::SHT_CREL
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output uses unsupported compressed or CREL section encoding",
                ));
            }
            if !is_supported_output_section_type(section.section_type) {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output contains an unsupported section type",
                ));
            }
            if !section_flags_are_valid(section.section_type, section.flags, true) {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output section flags are unsupported or incoherent",
                ));
            }
            if section.alignment > 1 && !section.alignment.is_power_of_two() {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output section alignment is malformed",
                ));
            }
            if section.size != 0
                && section.section_type != elf::SHT_NOBITS
                && section.alignment > 1
                && !section.offset.is_multiple_of(section.alignment)
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output section offset violates sh_addralign",
                ));
            }
            if section.flags & u64::from(elf::SHF_ALLOC) != 0
                && section.alignment > 1
                && section.address % section.alignment != section.offset % section.alignment
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output allocated section address/offset alignment is incongruent",
                ));
            }
            if section.entry_size != 0 && !section.size.is_multiple_of(section.entry_size) {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output section size is not a multiple of its entry size",
                ));
            }
            if section.link != 0 && section.link >= self.section_count
                || (section.flags & u64::from(elf::SHF_INFO_LINK) != 0
                    || matches!(
                        section.section_type,
                        elf::SHT_REL | elf::SHT_RELA | elf::SHT_CREL
                    ))
                    && section.information >= self.section_count
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output section references an invalid linked section",
                ));
            }
            if section.section_type == elf::SHT_NOBITS {
                if section.offset > self.output.size {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::ElfPolicy,
                        "static output NOBITS section offset is outside the file",
                    ));
                }
            } else {
                let end = section.offset.checked_add(section.size).ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::ElfPolicy,
                        "static output section file range overflows",
                    )
                })?;
                if end > self.output.size {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::ElfPolicy,
                        "static output section extends beyond the sealed file",
                    ));
                }
                self.section_file_ranges.insert(
                    section.offset,
                    end,
                    HostLinkErrorCodeV1::ElfPolicy,
                    "section file",
                )?;
            }
            self.validate_allocated_section_mapping(section)?;
            self.sections.push(section);
            Ok(())
        }

        fn validate_allocated_section_mapping(
            &self,
            section: OutputSectionV1,
        ) -> Result<(), HostLinkError> {
            if section.flags & u64::from(elf::SHF_ALLOC) == 0 || section.size == 0 {
                return Ok(());
            }
            let address_end = section.address.checked_add(section.size).ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output allocated section address range overflows",
                )
            })?;
            let Some((_, load)) = self.loads_by_address.range(..=section.address).next_back()
            else {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output allocated section is outside every PT_LOAD",
                ));
            };
            if address_end > load.memory_end
                || section.address < load.virtual_address
                || load.flags & elf::PF_R == 0
                || section.flags & u64::from(elf::SHF_WRITE) != 0 && load.flags & elf::PF_W == 0
                || section.flags & u64::from(elf::SHF_EXECINSTR) != 0 && load.flags & elf::PF_X == 0
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output allocated section has no compatible PT_LOAD mapping",
                ));
            }
            if section.section_type != elf::SHT_NOBITS {
                let section_end = section.offset.checked_add(section.size).ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::ElfPolicy,
                        "static output allocated section file range overflows",
                    )
                })?;
                if section_end > load.file_end
                    || section.offset < load.file_offset
                    || section.offset - load.file_offset != section.address - load.virtual_address
                {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::ElfPolicy,
                        "static output allocated section file/address mapping is incoherent",
                    ));
                }
            }
            Ok(())
        }

        fn finish_section_headers(&self) -> Result<(), HostLinkError> {
            if self.section_count == 0 {
                return Ok(());
            }
            let names = self.sections[self.section_name_index];
            if names.section_type != elf::SHT_STRTAB
                || names.size == 0
                || names.offset >= self.output.size
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output section-name table is malformed",
                ));
            }
            let mut terminators = [0_u8; 2];
            pread_exact_at(
                &self.output.file,
                &mut terminators[..1],
                names.offset,
                "section-name table first byte",
            )?;
            pread_exact_at(
                &self.output.file,
                &mut terminators[1..],
                names.offset + names.size - 1,
                "section-name table last byte",
            )?;
            if terminators != [0, 0] {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output section-name table is not NUL bounded",
                ));
            }
            Ok(())
        }

        fn cross_validate_section(&self) -> Result<(), HostLinkError> {
            let section = self.sections[self.index];
            if section.name != 0
                && (self.section_name_index == 0
                    || u64::from(section.name) >= self.sections[self.section_name_index].size)
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output section name offset is outside its string table",
                ));
            }
            let linked_type = self
                .sections
                .get(section.link)
                .map(|linked| linked.section_type);
            let valid = match section.section_type {
                elf::SHT_NULL => self.index == 0,
                elf::SHT_SYMTAB => {
                    section.link != 0
                        && linked_type == Some(elf::SHT_STRTAB)
                        && section.entry_size == 24
                        && section.size / 24 <= crate::MAX_HOST_LINK_ELF_TABLE_ENTRIES_V1
                        && section.information != 0
                        && section.information <= usize::try_from(section.size / 24).unwrap_or(0)
                }
                elf::SHT_INIT_ARRAY | elf::SHT_FINI_ARRAY | elf::SHT_PREINIT_ARRAY => {
                    section.link == 0
                        && section.information == 0
                        && matches!(section.entry_size, 0 | 8)
                }
                elf::SHT_PROGBITS => {
                    section.link == 0
                        && section.information == 0
                        && merge_entry_size_is_valid(
                            section.flags,
                            section.size,
                            section.entry_size,
                        )
                }
                elf::SHT_STRTAB | elf::SHT_NOBITS | elf::SHT_NOTE => {
                    section.link == 0 && section.information == 0 && section.entry_size == 0
                }
                _ => false,
            };
            if !valid {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output contains a malformed or dynamic linked section table",
                ));
            }
            if section.section_type == elf::SHT_STRTAB {
                if section.size == 0 {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::ElfPolicy,
                        "static output contains an empty string table",
                    ));
                }
                let mut endpoints = [0_u8; 2];
                pread_exact_at(
                    &self.output.file,
                    &mut endpoints[..1],
                    section.offset,
                    "static output string-table first byte",
                )?;
                pread_exact_at(
                    &self.output.file,
                    &mut endpoints[1..],
                    section.offset + section.size - 1,
                    "static output string-table last byte",
                )?;
                if endpoints != [0, 0] {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::ElfPolicy,
                        "static output string table is not NUL bounded",
                    ));
                }
            }
            if section.section_type == elf::SHT_PROGBITS
                && section.flags & SHF_STRINGS_V1 != 0
                && section.size != 0
            {
                let width = usize::try_from(section.entry_size).map_err(|_| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::ElfPolicy,
                        "static output merge-string width does not fit memory",
                    )
                })?;
                let mut terminator = [1_u8; 4];
                pread_exact_at(
                    &self.output.file,
                    &mut terminator[..width],
                    section.offset + section.size - section.entry_size,
                    "static output merge-string terminator",
                )?;
                if terminator[..width] != [0_u8; 4][..width] {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::ElfPolicy,
                        "static output merge-string section is not element-width NUL terminated",
                    ));
                }
            }
            Ok(())
        }

        fn validate_section_content_entry(&mut self) -> Result<bool, HostLinkError> {
            let section = self.sections[self.index];
            match section.section_type {
                elf::SHT_NOTE => self.validate_note_entry(section),
                elf::SHT_SYMTAB => self.validate_symbol_entry(section),
                _ => Ok(true),
            }
        }

        fn validate_note_entry(&mut self, section: OutputSectionV1) -> Result<bool, HostLinkError> {
            if self.content_offset == section.size {
                return Ok(true);
            }
            if section.size - self.content_offset < 12 {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output note section has a truncated note header",
                ));
            }
            let mut header = [0_u8; 12];
            pread_exact_at(
                &self.output.file,
                &mut header,
                section.offset + self.content_offset,
                "static output note header",
            )?;
            let name_size = u64::from(u32_at(&header, 0));
            let description_size = u64::from(u32_at(&header, 4));
            let record_size = 12_u64
                .checked_add(align_up_4(name_size)?)
                .and_then(|size| size.checked_add(align_up_4(description_size).ok()?))
                .ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::ElfPolicy,
                        "static output note record size overflows",
                    )
                })?;
            if self.content_offset / 12 >= crate::MAX_HOST_LINK_ELF_TABLE_ENTRIES_V1 {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output note count exceeds its traversal bound",
                ));
            }
            self.content_offset = self
                .content_offset
                .checked_add(record_size)
                .filter(|end| *end <= section.size)
                .ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::ElfPolicy,
                        "static output note record extends beyond its section",
                    )
                })?;
            Ok(self.content_offset == section.size)
        }

        fn validate_symbol_entry(
            &mut self,
            section: OutputSectionV1,
        ) -> Result<bool, HostLinkError> {
            if self.content_offset == section.size {
                return Ok(true);
            }
            let symbol_index = self.content_offset / 24;
            let mut symbol = [0_u8; 24];
            pread_exact_at(
                &self.output.file,
                &mut symbol,
                section.offset + self.content_offset,
                "static output symbol entry",
            )?;
            if symbol_index == 0 && symbol != [0; 24] {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output symbol zero is not the exact null symbol",
                ));
            }
            let binding = symbol[4] >> 4;
            let symbol_type = symbol[4] & 0xf;
            let visibility = symbol[5];
            let name = u64::from(u32_at(&symbol, 0));
            let section_index = u16_at(&symbol, 6);
            let string_table = self.sections[section.link];
            if name >= string_table.size
                || !matches!(binding, 0..=2 | 10)
                || !matches!(symbol_type, 0..=6 | 10)
                || visibility & !3 != 0
                || symbol_index < section.information as u64 && binding != 0
                || symbol_index >= section.information as u64 && binding == 0
                || section_index == elf::SHN_XINDEX
                || section_index < elf::SHN_LORESERVE
                    && usize::from(section_index) >= self.section_count
                || section_index >= elf::SHN_LORESERVE
                    && !matches!(section_index, elf::SHN_ABS | elf::SHN_COMMON)
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "static output symbol name, binding partition, or section index is invalid",
                ));
            }
            if section_index != elf::SHN_UNDEF && section_index < elf::SHN_LORESERVE {
                let target = self.sections[usize::from(section_index)];
                let value = u64_at(&symbol, 8);
                let size = u64_at(&symbol, 16);
                let valid_extent = if target.flags & u64::from(elf::SHF_ALLOC) != 0 {
                    value >= target.address
                        && value
                            .checked_add(size)
                            .is_some_and(|end| end <= target.address.saturating_add(target.size))
                } else {
                    value
                        .checked_add(size)
                        .is_some_and(|end| end <= target.size)
                };
                if !valid_extent {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::ElfPolicy,
                        "static output symbol extent is outside its target section",
                    ));
                }
            }
            self.content_offset += 24;
            Ok(self.content_offset == section.size)
        }
    }

    fn is_supported_output_section_type(section_type: u32) -> bool {
        matches!(
            section_type,
            elf::SHT_NULL
                | elf::SHT_PROGBITS
                | elf::SHT_SYMTAB
                | elf::SHT_STRTAB
                | elf::SHT_NOTE
                | elf::SHT_NOBITS
                | elf::SHT_INIT_ARRAY
                | elf::SHT_FINI_ARRAY
                | elf::SHT_PREINIT_ARRAY
        )
    }

    fn section_flags_are_valid(section_type: u32, flags: u64, executable: bool) -> bool {
        let allowed = if executable {
            match section_type {
                elf::SHT_NULL | elf::SHT_SYMTAB | elf::SHT_STRTAB => 0,
                elf::SHT_PROGBITS => {
                    SHF_WRITE_V1
                        | SHF_ALLOC_V1
                        | SHF_EXECINSTR_V1
                        | SHF_MERGE_V1
                        | SHF_STRINGS_V1
                        | SHF_TLS_V1
                        | SHF_GNU_RETAIN_V1
                }
                elf::SHT_NOTE => SHF_ALLOC_V1,
                elf::SHT_NOBITS
                | elf::SHT_INIT_ARRAY
                | elf::SHT_FINI_ARRAY
                | elf::SHT_PREINIT_ARRAY => {
                    SHF_WRITE_V1 | SHF_ALLOC_V1 | SHF_TLS_V1 | SHF_GNU_RETAIN_V1
                }
                _ => return false,
            }
        } else {
            match section_type {
                elf::SHT_NULL
                | elf::SHT_SYMTAB
                | elf::SHT_STRTAB
                | elf::SHT_GROUP
                | elf::SHT_SYMTAB_SHNDX => 0,
                elf::SHT_PROGBITS => {
                    SHF_WRITE_V1
                        | SHF_ALLOC_V1
                        | SHF_EXECINSTR_V1
                        | SHF_MERGE_V1
                        | SHF_STRINGS_V1
                        | SHF_LINK_ORDER_V1
                        | SHF_GROUP_V1
                        | SHF_TLS_V1
                        | SHF_GNU_RETAIN_V1
                        | SHF_EXCLUDE_V1
                }
                elf::SHT_RELA | elf::SHT_REL => SHF_INFO_LINK_V1 | SHF_GROUP_V1,
                elf::SHT_NOTE => SHF_ALLOC_V1,
                elf::SHT_NOBITS
                | elf::SHT_INIT_ARRAY
                | elf::SHT_FINI_ARRAY
                | elf::SHT_PREINIT_ARRAY => {
                    SHF_WRITE_V1 | SHF_ALLOC_V1 | SHF_GROUP_V1 | SHF_TLS_V1 | SHF_GNU_RETAIN_V1
                }
                elf::SHT_X86_64_UNWIND => SHF_ALLOC_V1 | SHF_LINK_ORDER_V1 | SHF_GROUP_V1,
                SHT_LLVM_ADDRSIG_V1 => SHF_EXCLUDE_V1,
                _ => return false,
            }
        };
        if flags & !allowed != 0
            || flags & SHF_STRINGS_V1 != 0 && flags & SHF_MERGE_V1 == 0
            || flags & SHF_TLS_V1 != 0 && flags & SHF_ALLOC_V1 == 0
            || flags & (SHF_WRITE_V1 | SHF_EXECINSTR_V1) != 0 && flags & SHF_ALLOC_V1 == 0
        {
            return false;
        }
        section_type != SHT_LLVM_ADDRSIG_V1 || flags == SHF_EXCLUDE_V1
    }

    fn merge_entry_size_is_valid(flags: u64, size: u64, entry_size: u64) -> bool {
        let merge = flags & SHF_MERGE_V1 != 0;
        let strings = flags & SHF_STRINGS_V1 != 0;
        if !merge {
            return entry_size == 0 && !strings;
        }
        entry_size != 0
            && size.is_multiple_of(entry_size)
            && (!strings || matches!(entry_size, 1 | 2 | 4))
    }

    fn align_up_4(value: u64) -> Result<u64, HostLinkError> {
        value.checked_add(3).map(|value| value & !3).ok_or_else(|| {
            HostLinkError::new(
                HostLinkErrorCodeV1::ElfPolicy,
                "ELF note field alignment overflows",
            )
        })
    }

    fn checked_table_end(
        offset: u64,
        count: usize,
        entry_size: u64,
        file_size: u64,
        name: &str,
    ) -> Result<u64, HostLinkError> {
        let length = (count as u64).checked_mul(entry_size).ok_or_else(|| {
            HostLinkError::new(
                HostLinkErrorCodeV1::ElfPolicy,
                format!("static output {name} table length overflows"),
            )
        })?;
        let end = offset.checked_add(length).ok_or_else(|| {
            HostLinkError::new(
                HostLinkErrorCodeV1::ElfPolicy,
                format!("static output {name} table range overflows"),
            )
        })?;
        if end > file_size {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ElfPolicy,
                format!("static output {name} table extends beyond the sealed file"),
            ));
        }
        Ok(end)
    }

    fn indexed_offset(base: u64, index: usize, entry_size: u64) -> Result<u64, HostLinkError> {
        base.checked_add((index as u64).checked_mul(entry_size).ok_or_else(|| {
            HostLinkError::new(
                HostLinkErrorCodeV1::ElfPolicy,
                "static output table index overflows",
            )
        })?)
        .ok_or_else(|| {
            HostLinkError::new(
                HostLinkErrorCodeV1::ElfPolicy,
                "static output table offset overflows",
            )
        })
    }

    fn ranges_overlap(
        first_start: u64,
        first_end: u64,
        second_start: u64,
        second_end: u64,
    ) -> bool {
        first_start != first_end
            && second_start != second_end
            && first_start < second_end
            && second_start < first_end
    }

    fn pread_exact_at(
        file: &File,
        mut bytes: &mut [u8],
        mut offset: u64,
        name: &str,
    ) -> Result<(), HostLinkError> {
        while !bytes.is_empty() {
            let count = rustix::io::pread(file, &mut *bytes, offset)
                .context(HostLinkErrorCodeV1::Io, || {
                    format!("read {name} without changing its shared offset")
                })?;
            if count == 0 {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::OutputTruncated,
                    format!("{name} is truncated"),
                ));
            }
            offset += count as u64;
            bytes = &mut bytes[count..];
        }
        Ok(())
    }

    fn u16_at(bytes: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes(
            bytes[offset..offset + 2]
                .try_into()
                .expect("fixed ELF field"),
        )
    }

    fn u32_at(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .expect("fixed ELF field"),
        )
    }

    fn u64_at(bytes: &[u8], offset: usize) -> u64 {
        u64::from_le_bytes(
            bytes[offset..offset + 8]
                .try_into()
                .expect("fixed ELF field"),
        )
    }

    fn ensure_copy_deadline(deadline: Instant) -> Result<(), HostLinkError> {
        if Instant::now() >= deadline {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::WorkerTimeout,
                "incremental host-link output work reached the fixed wall deadline",
            ));
        }
        Ok(())
    }

    fn ensure_no_extra_bytes(file: &File, size: u64, name: &str) -> Result<(), HostLinkError> {
        let mut extra = [0_u8; 1];
        if rustix::io::pread(file, &mut extra, size).context(HostLinkErrorCodeV1::Io, || {
            format!("bound {name} without changing its shared offset")
        })? != 0
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::OutputChanged,
                format!("{name} grew beyond its bound length"),
            ));
        }
        Ok(())
    }

    fn pwrite_all(file: &File, mut bytes: &[u8], mut offset: u64) -> Result<(), HostLinkError> {
        while !bytes.is_empty() {
            let written = rustix::io::pwrite(file, bytes, offset)
                .context(HostLinkErrorCodeV1::Io, || {
                    "incrementally write receiver-owned host-link output".to_owned()
                })?;
            if written == 0 {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::Io,
                    "receiver-owned host-link output write made no progress",
                ));
            }
            offset += written as u64;
            bytes = &bytes[written..];
        }
        Ok(())
    }

    fn validate_received_memfd_identity(
        file: &File,
        name: &str,
    ) -> Result<ReceivedMemfdIdentity, HostLinkError> {
        let metadata = file.metadata().context(HostLinkErrorCodeV1::Io, || {
            format!("inspect sender-owned {name}")
        })?;
        if !metadata.file_type().is_file() {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::NotRegular,
                format!("sender-owned {name} is not a regular file"),
            ));
        }
        if metadata.nlink() != 0 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                format!("sender-owned {name} is linked into a filesystem namespace"),
            ));
        }
        let effective_uid = rustix::process::geteuid().as_raw();
        if metadata.uid() != effective_uid {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                format!("sender-owned {name} has the wrong owner uid"),
            ));
        }
        let filesystem = rustix::fs::fstatfs(file).context(HostLinkErrorCodeV1::Io, || {
            format!("inspect sender-owned {name} filesystem identity")
        })?;
        if filesystem.f_type as u64 != TMPFS_MAGIC {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                format!("sender-owned {name} is not an anonymous shmem/memfd inode"),
            ));
        }
        verify_exact_seals(file, name)?;
        Ok(ReceivedMemfdIdentity {
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.len(),
            owner_uid: metadata.uid(),
        })
    }

    pub(crate) fn capture_to_sealed_memfd(
        source: File,
        name: &str,
        limit: u64,
    ) -> Result<CapturedFile, HostLinkError> {
        let before = FileSnapshot::capture(&source, name)?;
        if before.size == 0 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                format!("{name} is empty"),
            ));
        }
        if before.size > limit {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactTooLarge,
                format!("{name} exceeds the {limit}-byte bound"),
            ));
        }

        let descriptor = rustix::fs::memfd_create(
            "fe2o3-host-link-input-v1",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )
        .context(HostLinkErrorCodeV1::Io, || {
            format!("create sealed snapshot for {name}")
        })?;
        let mut snapshot = File::from(descriptor);
        let capacity = usize::try_from(before.size).map_err(|_| {
            HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactTooLarge,
                format!("{name} cannot fit in this process address space"),
            )
        })?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(capacity).map_err(|_| {
            HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactTooLarge,
                format!("cannot reserve memory for the bounded {name} snapshot"),
            )
        })?;
        let mut digest = Sha256::new();
        let mut buffer = [0_u8; COPY_CHUNK_BYTES];
        let mut copied = 0_u64;
        loop {
            let count = rustix::io::pread(&source, &mut buffer, copied)
                .context(HostLinkErrorCodeV1::Io, || {
                    format!("read {name} without changing its shared offset")
                })?;
            if count == 0 {
                break;
            }
            copied = copied.checked_add(count as u64).ok_or_else(|| {
                HostLinkError::new(HostLinkErrorCodeV1::ArtifactTooLarge, "copy size overflow")
            })?;
            if copied > before.size {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::DescriptorChanged,
                    format!("{name} grew while being copied"),
                ));
            }
            digest.update(&buffer[..count]);
            snapshot
                .write_all(&buffer[..count])
                .context(HostLinkErrorCodeV1::Io, || {
                    format!("write sealed snapshot for {name}")
                })?;
            bytes.extend_from_slice(&buffer[..count]);
        }
        if copied != before.size {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::DescriptorChanged,
                format!("{name} was truncated while being copied"),
            ));
        }
        let after = FileSnapshot::capture(&source, name)?;
        if before != after {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::DescriptorChanged,
                format!("{name} metadata changed while being copied"),
            ));
        }
        snapshot
            .set_permissions(std::fs::Permissions::from_mode(before.mode & 0o7777))
            .context(HostLinkErrorCodeV1::Io, || {
                format!("preserve admitted mode on snapshot for {name}")
            })?;
        rustix::fs::fcntl_add_seals(&snapshot, REQUIRED_SEALS)
            .context(HostLinkErrorCodeV1::Io, || {
                format!("seal snapshot for {name}")
            })?;
        verify_exact_seals(&snapshot, name)?;
        Ok(CapturedFile {
            file: snapshot,
            sha256: Sha256Digest::from_bytes(digest.finalize().into()),
            size: copied,
            mode: before.mode & 0o7777,
            bytes,
        })
    }

    pub(crate) fn read_sealed_file(
        file: File,
        name: &str,
        limit: u64,
    ) -> Result<CapturedFile, HostLinkError> {
        verify_exact_seals(&file, name)?;
        let snapshot = FileSnapshot::capture(&file, name)?;
        if snapshot.size == 0 || snapshot.size > limit {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactTooLarge,
                format!("sealed {name} size is outside the admitted bound"),
            ));
        }
        let capacity = usize::try_from(snapshot.size).map_err(|_| {
            HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactTooLarge,
                format!("sealed {name} cannot fit in this process address space"),
            )
        })?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(capacity).map_err(|_| {
            HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactTooLarge,
                format!("cannot reserve memory for sealed {name}"),
            )
        })?;
        bytes.resize(capacity, 0);
        let mut offset = 0_u64;
        while offset < snapshot.size {
            let start = usize::try_from(offset).map_err(|_| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactTooLarge,
                    format!("sealed {name} offset does not fit usize"),
                )
            })?;
            let count = rustix::io::pread(&file, &mut bytes[start..], offset)
                .context(HostLinkErrorCodeV1::Io, || {
                    format!("read sealed {name} without changing its shared offset")
                })?;
            if count == 0 {
                break;
            }
            offset = offset.checked_add(count as u64).ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactTooLarge,
                    format!("sealed {name} read offset overflowed"),
                )
            })?;
        }
        let mut extra = [0_u8; 1];
        let extra_count = rustix::io::pread(&file, &mut extra, snapshot.size)
            .context(HostLinkErrorCodeV1::Io, || {
                format!("bound sealed {name} without changing its shared offset")
            })?;
        if offset != snapshot.size || extra_count != 0 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::DescriptorChanged,
                format!("sealed {name} length changed"),
            ));
        }
        let sha256 = sha256_bytes(&bytes);
        Ok(CapturedFile {
            file,
            sha256,
            size: snapshot.size,
            mode: snapshot.mode & 0o7777,
            bytes,
        })
    }

    #[cfg(test)]
    #[cfg(test)]
    pub(crate) fn copy_received_sealed_file(
        source: File,
        name: &str,
        limit: u64,
    ) -> Result<CapturedFile, HostLinkError> {
        let sender_identity = validate_received_memfd_identity(&source, name)?;
        let before = read_sealed_file(
            source
                .try_clone()
                .context(HostLinkErrorCodeV1::Io, || format!("clone received {name}"))?,
            name,
            limit,
        )?;
        let descriptor = rustix::fs::memfd_create(
            "fe2o3-host-link-admitted-output-v1",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )
        .context(HostLinkErrorCodeV1::Io, || {
            format!("create receiver-owned snapshot for {name}")
        })?;
        let mut receiver = File::from(descriptor);
        receiver
            .write_all(&before.bytes)
            .context(HostLinkErrorCodeV1::Io, || {
                format!("copy {name} into receiver-owned snapshot")
            })?;
        receiver
            .set_permissions(std::fs::Permissions::from_mode(0o555))
            .context(HostLinkErrorCodeV1::Io, || {
                format!("canonicalize receiver-owned {name} mode")
            })?;
        rustix::fs::fcntl_add_seals(&receiver, REQUIRED_SEALS)
            .context(HostLinkErrorCodeV1::Io, || {
                format!("seal receiver-owned snapshot for {name}")
            })?;
        verify_exact_seals(&receiver, name)?;

        let after = read_sealed_file(source, name, limit)?;
        let after_identity = validate_received_memfd_identity(&after.file, name)?;
        if sender_identity != after_identity {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::DescriptorChanged,
                format!("sender-owned {name} identity changed while the receiver copied it"),
            ));
        }
        if before.sha256 != after.sha256 || before.size != after.size || before.bytes != after.bytes
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::DescriptorChanged,
                format!("sender-owned {name} changed while the receiver copied it"),
            ));
        }
        verify_sealed_artifact(&receiver, before.sha256, before.size, 0o555, name)?;
        Ok(CapturedFile {
            file: receiver,
            sha256: before.sha256,
            size: before.size,
            mode: 0o555,
            bytes: before.bytes,
        })
    }

    pub(crate) fn sealed_file_from_bytes(bytes: &[u8], name: &str) -> Result<File, HostLinkError> {
        if bytes.is_empty() || bytes.len() > MAX_HOST_LINK_PLAN_BYTES_V1 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::PlanTooLarge,
                "canonical plan size is outside its bound",
            ));
        }
        let descriptor = rustix::fs::memfd_create(
            "fe2o3-host-link-plan-v1",
            MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
        )
        .context(HostLinkErrorCodeV1::Io, || {
            format!("create sealed {name} descriptor")
        })?;
        let mut file = File::from(descriptor);
        file.write_all(bytes)
            .context(HostLinkErrorCodeV1::Io, || format!("write {name}"))?;
        rustix::fs::fcntl_add_seals(&file, REQUIRED_SEALS)
            .context(HostLinkErrorCodeV1::Io, || format!("seal {name}"))?;
        verify_exact_seals(&file, name)?;
        Ok(file)
    }

    pub(crate) fn verify_sealed_artifact(
        file: &File,
        expected_sha256: Sha256Digest,
        expected_size: u64,
        expected_mode: u32,
        name: &str,
    ) -> Result<(), HostLinkError> {
        verify_exact_seals(file, name)?;
        let captured = read_sealed_file(
            file.try_clone()
                .context(HostLinkErrorCodeV1::Io, || format!("clone sealed {name}"))?,
            name,
            MAX_HOST_LINK_OUTPUT_BYTES_V1,
        )?;
        if captured.sha256 != expected_sha256
            || captured.size != expected_size
            || captured.mode != expected_mode
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::DigestMismatch,
                format!("sealed {name} does not match its plan identity"),
            ));
        }
        Ok(())
    }

    pub(crate) fn verify_sealed_artifact_identity(
        file: &File,
        expected_size: u64,
        expected_mode: u32,
        name: &str,
    ) -> Result<(), HostLinkError> {
        verify_exact_seals(file, name)?;
        let snapshot = FileSnapshot::capture(file, name)?;
        if snapshot.size != expected_size || snapshot.mode & 0o7777 != expected_mode {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::DescriptorChanged,
                format!("sealed {name} identity changed after its bytes were authenticated"),
            ));
        }
        Ok(())
    }

    fn verify_exact_seals(file: &File, name: &str) -> Result<(), HostLinkError> {
        let seals = rustix::fs::fcntl_get_seals(file)
            .context(HostLinkErrorCodeV1::DescriptorUnsealed, || {
                format!("inspect seals on {name}")
            })?;
        if seals != REQUIRED_SEALS {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::DescriptorUnsealed,
                format!("{name} lacks the exact write/grow/shrink/seal set"),
            ));
        }
        Ok(())
    }

    pub(crate) fn inspect_artifact(
        kind: HostArtifactKindV1,
        bytes: &[u8],
    ) -> Result<ArtifactInspectionV1, HostLinkError> {
        match kind {
            HostArtifactKindV1::RegularArchive | HostArtifactKindV1::Rlib => {
                let archive_members = inspect_archive(bytes, kind)?;
                Ok(ArtifactInspectionV1 {
                    elf_profile: None,
                    archive_members,
                })
            }
            HostArtifactKindV1::LinkerScript | HostArtifactKindV1::ResponseFile => {
                if bytes.contains(&0) || !bytes.is_ascii() {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::ArtifactKind,
                        "link control file must contain bounded non-NUL ASCII",
                    ));
                }
                Ok(ArtifactInspectionV1 {
                    elf_profile: None,
                    archive_members: 0,
                })
            }
            HostArtifactKindV1::Plugin => Err(HostLinkError::new(
                HostLinkErrorCodeV1::Plugin,
                "linker plugins are outside HostLinkClosureV1",
            )),
            HostArtifactKindV1::LtoCache => Err(HostLinkError::new(
                HostLinkErrorCodeV1::Lto,
                "LTO inputs and caches are outside HostLinkClosureV1",
            )),
            HostArtifactKindV1::StaticWrapper
            | HostArtifactKindV1::StaticHostLld
            | HostArtifactKindV1::Crt
            | HostArtifactKindV1::Object
            | HostArtifactKindV1::Dso
            | HostArtifactKindV1::BuildScriptNative => {
                if matches!(
                    kind,
                    HostArtifactKindV1::Crt
                        | HostArtifactKindV1::Object
                        | HostArtifactKindV1::BuildScriptNative
                ) {
                    inspect_relocatable_elf(bytes)?;
                }
                let profile = inspect_elf(bytes)?;
                match kind {
                    HostArtifactKindV1::StaticWrapper | HostArtifactKindV1::StaticHostLld => {
                        if profile.machine != elf::EM_X86_64
                            || profile.elf_type != elf::ET_EXEC
                            || profile.interpreter.is_some()
                            || !profile.needed.is_empty()
                            || profile.has_writable_executable_segment
                            || profile.has_executable_stack
                        {
                            return Err(HostLinkError::new(
                                HostLinkErrorCodeV1::ElfPolicy,
                                "host-link tool does not satisfy the static executable profile",
                            ));
                        }
                    }
                    HostArtifactKindV1::Crt
                    | HostArtifactKindV1::Object
                    | HostArtifactKindV1::BuildScriptNative => {}
                    HostArtifactKindV1::Dso => {
                        if profile.machine != elf::EM_X86_64 || profile.elf_type != elf::ET_DYN {
                            return Err(HostLinkError::new(
                                HostLinkErrorCodeV1::ArtifactKind,
                                "DSO input is not ET_DYN",
                            ));
                        }
                    }
                    _ => unreachable!(),
                }
                Ok(ArtifactInspectionV1 {
                    elf_profile: Some(profile),
                    archive_members: 0,
                })
            }
        }
    }

    fn inspect_archive(bytes: &[u8], kind: HostArtifactKindV1) -> Result<u64, HostLinkError> {
        let raw_member_count = validate_regular_archive_container(bytes)?;
        let archive = ArchiveFile::parse(bytes).map_err(|_| {
            HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                "structured archive parser rejected the input",
            )
        })?;
        if archive.is_thin() {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ThinArchive,
                "thin archives are outside HostLinkClosureV1",
            ));
        }
        let mut member_count = 0_usize;
        let mut name_bytes = 0_usize;
        let mut aggregate_bytes = 0_u64;
        let mut saw_linkable_object = false;
        let mut saw_rust_metadata = false;
        for member in archive.members() {
            let member = member.map_err(|_| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactKind,
                    "structured archive parser rejected a member",
                )
            })?;
            let name = member.name();
            member_count = member_count
                .checked_add(1)
                .filter(|count| *count <= MAX_ARCHIVE_MEMBERS_V1)
                .ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::FieldTooLarge,
                        "archive member count exceeds its traversal bound",
                    )
                })?;
            name_bytes = name_bytes
                .checked_add(name.len())
                .filter(|length| *length <= MAX_ARCHIVE_MEMBER_NAME_BYTES_V1)
                .ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::FieldTooLarge,
                        "archive member names exceed their aggregate traversal bound",
                    )
                })?;
            aggregate_bytes = aggregate_bytes
                .checked_add(member.size())
                .filter(|length| *length <= MAX_HOST_LINK_INPUT_BYTES_V1)
                .ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::ArtifactTooLarge,
                        "archive members exceed their aggregate traversal bound",
                    )
                })?;
            if name.is_empty()
                || name.len() > 1024
                || matches!(name, b"." | b"..")
                || !name.iter().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'$' | b'+')
                })
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::InvalidPath,
                    "archive member name is external, nested, or path-shaped",
                ));
            }
            if member.is_thin() {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ThinArchive,
                    "archive contains an external thin member",
                ));
            }
            if name == b"lib.rmeta" {
                if kind != HostArtifactKindV1::Rlib || saw_rust_metadata {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::ArtifactKind,
                        "lib.rmeta is allowed exactly once and only in an rlib",
                    ));
                }
                saw_rust_metadata = true;
            } else {
                saw_linkable_object = true;
            }
        }
        for member in archive.members() {
            let member = member.map_err(|_| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactKind,
                    "structured archive parser rejected a member during content validation",
                )
            })?;
            let data = member.data(bytes).map_err(|_| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactKind,
                    "archive member data is malformed or external",
                )
            })?;
            inspect_archive_member(data)?;
        }
        if !saw_linkable_object {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                "archive contains no linkable ELF object member",
            ));
        }
        if member_count != raw_member_count {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                "archive structural parser and member iterator disagree",
            ));
        }
        Ok(member_count as u64)
    }

    enum ArchiveMemberNameV1 {
        Simple(Vec<u8>),
        GnuLong(usize),
    }

    struct ArchiveMemberRecordV1 {
        name: ArchiveMemberNameV1,
    }

    fn validate_regular_archive_container(bytes: &[u8]) -> Result<usize, HostLinkError> {
        if bytes.starts_with(b"!<thin>\n") {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ThinArchive,
                "thin archives are outside HostLinkClosureV1",
            ));
        }
        if !bytes.starts_with(b"!<arch>\n") {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                "archive magic is not the regular archive encoding",
            ));
        }
        let mut offset = 8_usize;
        let mut members = Vec::new();
        let mut regular_offsets = std::collections::BTreeSet::new();
        let mut gnu_symbols = None;
        let mut gnu_names = None;
        while offset < bytes.len() {
            let header_end = offset
                .checked_add(60)
                .filter(|end| *end <= bytes.len())
                .ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::ArtifactKind,
                        "archive header is truncated",
                    )
                })?;
            let header = &bytes[offset..header_end];
            if &header[58..60] != b"`\n" {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactKind,
                    "archive member header trailer is malformed",
                ));
            }
            validate_archive_optional_number(&header[16..28], "modification time", 10)?;
            validate_archive_optional_number(&header[28..34], "owner ID", 10)?;
            validate_archive_optional_number(&header[34..40], "group ID", 10)?;
            validate_archive_optional_number(&header[40..48], "mode", 8)?;
            let size = parse_archive_decimal(&header[48..58], "member size")?;
            let data_start = header_end;
            let data_end = data_start
                .checked_add(size)
                .filter(|end| *end <= bytes.len())
                .ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::ArtifactKind,
                        "archive member body is truncated",
                    )
                })?;
            let raw_name = trim_archive_spaces(&header[..16]);
            if raw_name.is_empty() {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactKind,
                    "archive member name is empty",
                ));
            }
            match raw_name {
                b"/" => {
                    if gnu_symbols.replace(&bytes[data_start..data_end]).is_some() {
                        return Err(HostLinkError::new(
                            HostLinkErrorCodeV1::DuplicateRecord,
                            "archive contains duplicate GNU symbol tables",
                        ));
                    }
                }
                b"//" => {
                    if gnu_names.replace(&bytes[data_start..data_end]).is_some() {
                        return Err(HostLinkError::new(
                            HostLinkErrorCodeV1::DuplicateRecord,
                            "archive contains duplicate GNU long-name tables",
                        ));
                    }
                }
                b"/SYM64/" => {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::ArtifactKind,
                        "GNU 64-bit archive symbol tables are outside V1",
                    ));
                }
                name if name.starts_with(b"#1/")
                    || name.starts_with(b"__.SYMDEF")
                    || name.starts_with(b"__.LLVM_SYM") =>
                {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::ArtifactKind,
                        "BSD archive name and symbol-table encodings are outside V1",
                    ));
                }
                name if name[0] == b'/' => {
                    let long_offset = parse_archive_decimal(&name[1..], "GNU long-name offset")?;
                    push_archive_member(
                        &mut members,
                        &mut regular_offsets,
                        offset,
                        ArchiveMemberNameV1::GnuLong(long_offset),
                    )?;
                }
                name => {
                    let Some(simple) = name.strip_suffix(b"/") else {
                        return Err(HostLinkError::new(
                            HostLinkErrorCodeV1::ArtifactKind,
                            "archive simple member name lacks its canonical terminator",
                        ));
                    };
                    push_archive_member(
                        &mut members,
                        &mut regular_offsets,
                        offset,
                        ArchiveMemberNameV1::Simple(simple.to_vec()),
                    )?;
                }
            }
            offset = data_end;
            if size % 2 != 0 {
                if bytes.get(offset) != Some(&b'\n') {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::ArtifactKind,
                        "archive member padding is missing or noncanonical",
                    ));
                }
                offset += 1;
            }
        }
        let long_names = parse_gnu_long_names(gnu_names.unwrap_or_default())?;
        for member in &members {
            let name = match &member.name {
                ArchiveMemberNameV1::Simple(name) => name.as_slice(),
                ArchiveMemberNameV1::GnuLong(offset) => {
                    long_names.get(offset).map(Vec::as_slice).ok_or_else(|| {
                        HostLinkError::new(
                            HostLinkErrorCodeV1::ArtifactKind,
                            "GNU archive long-name offset is not an entry boundary",
                        )
                    })?
                }
            };
            validate_archive_member_name(name)?;
        }
        if let Some(symbols) = gnu_symbols {
            validate_gnu_archive_symbols(symbols, &regular_offsets)?;
        }
        Ok(members.len())
    }

    fn push_archive_member(
        members: &mut Vec<ArchiveMemberRecordV1>,
        offsets: &mut std::collections::BTreeSet<usize>,
        header_offset: usize,
        name: ArchiveMemberNameV1,
    ) -> Result<(), HostLinkError> {
        if members.len() == MAX_ARCHIVE_MEMBERS_V1 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "archive member count exceeds its traversal bound",
            ));
        }
        offsets.insert(header_offset);
        members.push(ArchiveMemberRecordV1 { name });
        Ok(())
    }

    fn parse_gnu_long_names(bytes: &[u8]) -> Result<BTreeMap<usize, Vec<u8>>, HostLinkError> {
        let mut names = BTreeMap::new();
        let mut offset = 0_usize;
        let mut aggregate = 0_usize;
        while offset < bytes.len() {
            if bytes[offset..] == [b'\n'] && bytes.len().is_multiple_of(2) {
                break;
            }
            let relative_end = bytes[offset..]
                .windows(2)
                .position(|window| window == b"/\n")
                .ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::ArtifactKind,
                        "GNU archive long-name table has an unterminated entry",
                    )
                })?;
            let end = offset + relative_end;
            let name = &bytes[offset..end];
            aggregate = aggregate
                .checked_add(name.len())
                .filter(|total| *total <= MAX_ARCHIVE_MEMBER_NAME_BYTES_V1)
                .ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::FieldTooLarge,
                        "GNU archive long names exceed their aggregate bound",
                    )
                })?;
            validate_archive_member_name(name)?;
            names.insert(offset, name.to_vec());
            offset = end + 2;
        }
        Ok(names)
    }

    fn validate_gnu_archive_symbols(
        bytes: &[u8],
        member_offsets: &std::collections::BTreeSet<usize>,
    ) -> Result<(), HostLinkError> {
        if bytes.len() < 4 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                "GNU archive symbol table is truncated",
            ));
        }
        let count = u32::from_be_bytes(bytes[..4].try_into().expect("four bytes")) as usize;
        if count as u64 > crate::MAX_HOST_LINK_ELF_TABLE_ENTRIES_V1 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "GNU archive symbol count exceeds its traversal bound",
            ));
        }
        let offsets_end = 4_usize
            .checked_add(count.checked_mul(4).ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactKind,
                    "GNU archive symbol-offset table overflows",
                )
            })?)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactKind,
                    "GNU archive symbol-offset table is truncated",
                )
            })?;
        for index in 0..count {
            let start = 4 + index * 4;
            let member = u32::from_be_bytes(
                bytes[start..start + 4]
                    .try_into()
                    .expect("bounded archive offset"),
            ) as usize;
            if !member_offsets.contains(&member) {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactKind,
                    "GNU archive symbol references a non-member header offset",
                ));
            }
        }
        let mut names = &bytes[offsets_end..];
        for _ in 0..count {
            let end = names.iter().position(|byte| *byte == 0).ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactKind,
                    "GNU archive symbol name is unterminated",
                )
            })?;
            if end == 0 || !names[..end].iter().all(|byte| byte.is_ascii_graphic()) {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactKind,
                    "GNU archive symbol name is empty or contains control bytes",
                ));
            }
            names = &names[end + 1..];
        }
        let canonical_internal_padding =
            names == [0] && bytes.len().is_multiple_of(2) || count == 0 && names == [0, 0, 0, 0];
        if !names.is_empty() && !canonical_internal_padding {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                "GNU archive symbol table has trailing bytes",
            ));
        }
        Ok(())
    }

    fn parse_archive_decimal(bytes: &[u8], name: &str) -> Result<usize, HostLinkError> {
        parse_archive_radix(bytes, name, 10)
    }

    fn validate_archive_optional_number(
        bytes: &[u8],
        name: &str,
        radix: u8,
    ) -> Result<(), HostLinkError> {
        if trim_archive_spaces(bytes).is_empty() {
            return Ok(());
        }
        parse_archive_radix(bytes, name, radix).map(|_| ())
    }

    fn parse_archive_radix(bytes: &[u8], name: &str, radix: u8) -> Result<usize, HostLinkError> {
        let value = trim_archive_spaces(bytes);
        if value.is_empty()
            || !value
                .iter()
                .all(|byte| byte.is_ascii_digit() && *byte - b'0' < radix)
            || value.len() > 1 && value[0] == b'0'
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                format!("archive {name} is not canonical decimal"),
            ));
        }
        value.iter().try_fold(0_usize, |result, byte| {
            result
                .checked_mul(usize::from(radix))
                .and_then(|result| result.checked_add(usize::from(*byte - b'0')))
                .ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::ArtifactKind,
                        format!("archive {name} overflows"),
                    )
                })
        })
    }

    fn trim_archive_spaces(mut bytes: &[u8]) -> &[u8] {
        while bytes.last() == Some(&b' ') {
            bytes = &bytes[..bytes.len() - 1];
        }
        bytes
    }

    fn validate_archive_member_name(name: &[u8]) -> Result<(), HostLinkError> {
        if name.is_empty()
            || name.len() > 1024
            || matches!(name, b"." | b"..")
            || !name.iter().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'$' | b'+')
            })
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidPath,
                "archive member name is external, nested, or path-shaped",
            ));
        }
        Ok(())
    }

    fn inspect_archive_member(data: &[u8]) -> Result<(), HostLinkError> {
        if data.starts_with(b"!<arch>\n") || data.starts_with(b"!<thin>\n") {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                "nested archives are outside HostLinkClosureV1",
            ));
        }
        if data.starts_with(b"BC\xc0\xde") || data.starts_with(b"\xde\xc0\x17\x0b") {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::Lto,
                "raw LLVM bitcode archive members are outside HostLinkClosureV1",
            ));
        }
        inspect_relocatable_elf(data)
    }

    fn inspect_relocatable_elf(data: &[u8]) -> Result<(), HostLinkError> {
        if data.starts_with(b"BC\xc0\xde") || data.starts_with(b"\xde\xc0\x17\x0b") {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::Lto,
                "raw LLVM bitcode inputs are outside HostLinkClosureV1",
            ));
        }
        let file = object::File::parse(data).map_err(|_| {
            HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                "input is not a structurally valid ELF object",
            )
        })?;
        if file.format() != object::BinaryFormat::Elf
            || !file.is_64()
            || !file.is_little_endian()
            || file.architecture() != object::Architecture::X86_64
            || file.kind() != object::ObjectKind::Relocatable
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                "input is not x86_64 ELF64 little-endian ET_REL",
            ));
        }
        let elf_file = ElfFile64::<Endianness>::parse(data).map_err(|_| {
            HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                "structured ELF parser rejected the relocatable input",
            )
        })?;
        let endian = elf_file.endian();
        if elf_file.elf_header().e_machine(endian) != elf::EM_X86_64
            || elf_file.elf_header().e_type(endian) != elf::ET_REL
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                "input ELF header is not x86_64 ET_REL",
            ));
        }
        for section in elf_file.elf_section_table().iter() {
            let section_name = elf_file
                .elf_section_table()
                .section_name(endian, section)
                .map_err(|_| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::ArtifactKind,
                        "relocatable input has a malformed section name",
                    )
                })?;
            let flags = section.sh_flags(endian);
            let inert_llvmbc = section_name == b".llvmbc"
                && flags & SHF_EXCLUDE_V1 != 0
                && flags & (SHF_ALLOC_V1 | SHF_WRITE_V1 | SHF_EXECINSTR_V1) == 0;
            if section_name == b".llvmbc" && !inert_llvmbc
                || section_name != b".llvmbc"
                    && (section_name == b".llvm.lto"
                        || section_name == b".llvm_bc"
                        || section_name.starts_with(b".gnu.lto_"))
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::Lto,
                    "active or unsupported embedded bitcode is outside HostLinkClosureV1",
                ));
            }
        }
        validate_elf64_structure(&elf_file, data, false, HostLinkErrorCodeV1::ArtifactKind)?;
        force_object_structure(&file, HostLinkErrorCodeV1::ArtifactKind)?;
        for section in elf_file.elf_section_table().iter() {
            let section_type = section.sh_type(endian);
            let flags = section.sh_flags(endian);
            if section_type == elf::SHT_LLVM_DEPENDENT_LIBRARIES {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::Lto,
                    "LLVM dependent-library sections are outside HostLinkClosureV1",
                ));
            }
            let section_name = elf_file
                .elf_section_table()
                .section_name(endian, section)
                .map_err(|_| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::ArtifactKind,
                        "relocatable input has a malformed section name",
                    )
                })?;
            if section_name == b".deplibs" {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::Lto,
                    "LLVM dependent-library sections are outside HostLinkClosureV1",
                ));
            }
            let required_excluded = section_type == SHT_LLVM_ADDRSIG_V1
                && section_name == b".llvm_addrsig"
                || section_type == elf::SHT_PROGBITS
                    && matches!(
                        section_name,
                        b".llvmbc" | b".llvmcmd" | b".rmeta" | b".rmeta-link"
                    );
            if flags & SHF_EXCLUDE_V1 != 0 && !required_excluded {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactKind,
                    "excluded relocatable section is not a required inert Rust/LLVM section",
                ));
            }
        }
        for section in file.sections() {
            let section_name = section.name_bytes().map_err(|_| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactKind,
                    "archive member has a malformed section name",
                )
            })?;
            let is_excluded_llvmbc = section_name == b".llvmbc"
                && matches!(
                    section.flags(),
                    SectionFlags::Elf { sh_flags }
                        if sh_flags & u64::from(elf::SHF_EXCLUDE) != 0
                            && sh_flags
                                & u64::from(elf::SHF_ALLOC | elf::SHF_WRITE | elf::SHF_EXECINSTR)
                                == 0
                );
            if section_name == b".llvmbc" && !is_excluded_llvmbc {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::Lto,
                    "non-excluded .llvmbc archive section is outside HostLinkClosureV1",
                ));
            }
            if section_name != b".llvmbc"
                && (section_name == b".llvm.lto"
                    || section_name == b".llvm_bc"
                    || section_name.starts_with(b".gnu.lto_"))
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::Lto,
                    "LTO archive section is outside HostLinkClosureV1",
                ));
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn inspect_static_output_elf(bytes: &[u8]) -> Result<ElfProfileV1, HostLinkError> {
        let profile = inspect_elf(bytes)?;
        let object = object::File::parse(bytes).map_err(|_| {
            HostLinkError::new(
                HostLinkErrorCodeV1::ElfPolicy,
                "object parser rejected the static output ELF",
            )
        })?;
        let file = ElfFile64::<Endianness>::parse(bytes).map_err(|_| {
            HostLinkError::new(
                HostLinkErrorCodeV1::ElfPolicy,
                "structured ELF parser rejected the static output",
            )
        })?;
        let endian = file.endian();
        validate_elf64_structure(&file, bytes, true, HostLinkErrorCodeV1::ElfPolicy)?;
        force_object_structure(&object, HostLinkErrorCodeV1::ElfPolicy)?;
        let has_dynamic_segment = file
            .elf_program_headers()
            .iter()
            .any(|segment| segment.p_type(endian) == elf::PT_DYNAMIC);
        if object.format() != object::BinaryFormat::Elf
            || !object.is_64()
            || !object.is_little_endian()
            || object.architecture() != object::Architecture::X86_64
            || object.kind() != object::ObjectKind::Executable
            || profile.machine != elf::EM_X86_64
            || profile.elf_type != elf::ET_EXEC
            || profile.interpreter.is_some()
            || profile.soname.is_some()
            || !profile.needed.is_empty()
            || has_dynamic_segment
            || profile.has_writable_executable_segment
            || profile.has_executable_stack
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ElfPolicy,
                "output is not a static x86_64 ET_EXEC without loader, dynamic, WX, or executable-stack state",
            ));
        }
        Ok(profile)
    }

    fn force_object_structure(
        object: &object::File<'_>,
        code: HostLinkErrorCodeV1,
    ) -> Result<(), HostLinkError> {
        for segment in object.segments() {
            segment.data().map_err(|_| {
                HostLinkError::new(code, "ELF segment extent is outside the captured file")
            })?;
        }
        for section in object.sections() {
            section.name_bytes().map_err(|_| {
                HostLinkError::new(code, "ELF section name is outside its string table")
            })?;
            section.data().map_err(|_| {
                HostLinkError::new(code, "ELF section extent is outside the captured file")
            })?;
            for _ in section.relocations() {}
        }
        for symbol in object.symbols() {
            symbol.name_bytes().map_err(|_| {
                HostLinkError::new(code, "ELF symbol name is outside its string table")
            })?;
            if let Some(index) = symbol.section_index() {
                object.section_by_index(index).map_err(|_| {
                    HostLinkError::new(code, "ELF symbol references an invalid section")
                })?;
            }
        }
        Ok(())
    }

    #[derive(Clone, Copy)]
    struct RawSectionV1 {
        name: u32,
        section_type: u32,
        flags: u64,
        address: u64,
        offset: u64,
        size: u64,
        link: usize,
        information: usize,
        alignment: u64,
        entry_size: u64,
    }

    fn validate_elf64_structure(
        file: &ElfFile64<'_, Endianness>,
        bytes: &[u8],
        require_executable_load: bool,
        code: HostLinkErrorCodeV1,
    ) -> Result<(), HostLinkError> {
        let endian = file.endian();
        let header = file.elf_header();
        let mut executable_loads = Vec::new();
        let mut load_file_ranges = IntervalSetV1::default();
        let mut load_memory_ranges = IntervalSetV1::default();

        if file.elf_program_headers().len() > crate::MAX_HOST_LINK_ELF_PROGRAM_HEADERS_V1 {
            return Err(HostLinkError::new(
                code,
                "ELF program-header count exceeds the V1 bound",
            ));
        }

        for segment in file.elf_program_headers() {
            segment.data(endian, bytes).map_err(|_| {
                HostLinkError::new(code, "ELF segment extent is outside the captured file")
            })?;
            let offset = segment.p_offset(endian);
            let virtual_address = segment.p_vaddr(endian);
            let file_size = segment.p_filesz(endian);
            let memory_size = segment.p_memsz(endian);
            let alignment = segment.p_align(endian);
            if file_size > memory_size {
                return Err(HostLinkError::new(
                    code,
                    "ELF segment file size exceeds its memory size",
                ));
            }
            if alignment > 1
                && (!alignment.is_power_of_two()
                    || offset % alignment != virtual_address % alignment)
            {
                return Err(HostLinkError::new(
                    code,
                    "ELF segment alignment or offset/address congruence is invalid",
                ));
            }
            let file_end = offset
                .checked_add(file_size)
                .ok_or_else(|| HostLinkError::new(code, "ELF segment file range overflows"))?;
            let memory_end = virtual_address
                .checked_add(memory_size)
                .ok_or_else(|| HostLinkError::new(code, "ELF segment memory range overflows"))?;
            if file_end > bytes.len() as u64 {
                return Err(HostLinkError::new(
                    code,
                    "ELF segment extends beyond the captured file",
                ));
            }
            if segment.p_type(endian) == elf::PT_LOAD {
                load_file_ranges.insert(offset, file_end, code, "PT_LOAD file")?;
                load_memory_ranges.insert(virtual_address, memory_end, code, "PT_LOAD memory")?;
                if segment.p_flags(endian) & elf::PF_X != 0 && file_size != 0 {
                    executable_loads.push((virtual_address, file_size, memory_size));
                }
            }
        }
        validate_bounded_elf64_sections(bytes, require_executable_load, code)?;

        if require_executable_load {
            let entry = header.e_entry(endian);
            if entry == 0 || executable_loads.is_empty() {
                return Err(HostLinkError::new(
                    code,
                    "static output lacks a nonzero entry in a file-backed executable PT_LOAD",
                ));
            }
            if !executable_loads
                .iter()
                .any(|(virtual_address, file_size, memory_size)| {
                    let Some(delta) = entry.checked_sub(*virtual_address) else {
                        return false;
                    };
                    delta < *file_size && delta < *memory_size
                })
            {
                return Err(HostLinkError::new(
                    code,
                    "static output entry is outside every executable PT_LOAD mapping",
                ));
            }
        }
        Ok(())
    }

    fn validate_bounded_elf64_sections(
        bytes: &[u8],
        executable: bool,
        code: HostLinkErrorCodeV1,
    ) -> Result<(), HostLinkError> {
        if bytes.len() < 64
            || bytes.get(..4) != Some(b"\x7fELF")
            || bytes[4] != 2
            || bytes[5] != 1
            || bytes[6] != 1
            || u16_at(bytes, 52) != 64
        {
            return Err(HostLinkError::new(code, "ELF64 header is malformed"));
        }
        let program_offset = u64_at(bytes, 32);
        let section_offset = u64_at(bytes, 40);
        let program_entry_size = u16_at(bytes, 54);
        let program_count = usize::from(u16_at(bytes, 56));
        let section_entry_size = u16_at(bytes, 58);
        let section_count = usize::from(u16_at(bytes, 60));
        let section_names = usize::from(u16_at(bytes, 62));
        if u16_at(bytes, 18) != elf::EM_X86_64
            || u32_at(bytes, 20) != 1
            || u32_at(bytes, 48) != 0
            || !executable && (u16_at(bytes, 16) != elf::ET_REL || u64_at(bytes, 24) != 0)
            || executable && u16_at(bytes, 16) != elf::ET_EXEC
            || program_count == usize::from(elf::PN_XNUM)
            || section_names == usize::from(elf::SHN_XINDEX)
            || program_count > crate::MAX_HOST_LINK_ELF_PROGRAM_HEADERS_V1
            || section_count > crate::MAX_HOST_LINK_ELF_SECTIONS_V1
            || program_count != 0 && program_entry_size != 56
            || program_count == 0 && program_offset != 0
            || !executable && program_count != 0
            || program_count == 0 && !matches!(program_entry_size, 0 | 56)
            || section_count == 0 && (section_offset != 0 || section_names != 0)
            || section_count != 0 && (section_entry_size != 64 || section_names >= section_count)
            || program_count != 0 && !program_offset.is_multiple_of(8)
            || section_count != 0 && !section_offset.is_multiple_of(8)
        {
            return Err(HostLinkError::new(
                code,
                "ELF table count or entry-size encoding is unsupported",
            ));
        }
        if section_count == 0 {
            return Ok(());
        }
        let program_end = raw_table_end(program_offset, program_count, 56, bytes.len(), code)?;
        let section_end = raw_table_end(section_offset, section_count, 64, bytes.len(), code)?;
        if section_offset < 64
            || program_count != 0 && program_offset < 64
            || ranges_overlap(program_offset, program_end, section_offset, section_end)
        {
            return Err(HostLinkError::new(
                code,
                "ELF header tables overlap or cover the ELF header",
            ));
        }
        let mut file_ranges = IntervalSetV1::default();
        file_ranges.insert(0, 64, code, "file")?;
        if program_count != 0 {
            file_ranges.insert(program_offset, program_end, code, "file")?;
        }
        file_ranges.insert(section_offset, section_end, code, "file")?;
        let mut sections = Vec::with_capacity(section_count);
        for index in 0..section_count {
            let offset = indexed_raw_offset(section_offset, index, 64, code)?;
            let header = bounded_bytes(bytes, offset, 64, code, "ELF section header")?;
            let section = RawSectionV1 {
                name: u32_at(header, 0),
                section_type: u32_at(header, 4),
                flags: u64_at(header, 8),
                address: u64_at(header, 16),
                offset: u64_at(header, 24),
                size: u64_at(header, 32),
                link: u32_at(header, 40) as usize,
                information: u32_at(header, 44) as usize,
                alignment: u64_at(header, 48),
                entry_size: u64_at(header, 56),
            };
            validate_raw_section_header(
                section,
                index,
                section_count,
                executable,
                bytes.len() as u64,
                &mut file_ranges,
                code,
            )?;
            sections.push(section);
        }
        validate_section_names(bytes, &sections, section_names, code)?;
        validate_linked_section_tables(bytes, &sections, executable, code)
    }

    fn validate_raw_section_header(
        section: RawSectionV1,
        index: usize,
        section_count: usize,
        executable: bool,
        file_size: u64,
        file_ranges: &mut IntervalSetV1,
        code: HostLinkErrorCodeV1,
    ) -> Result<(), HostLinkError> {
        if index == 0
            && (section.name != 0
                || section.section_type != elf::SHT_NULL
                || section.flags != 0
                || section.address != 0
                || section.offset != 0
                || section.size != 0
                || section.link != 0
                || section.information != 0
                || section.alignment != 0
                || section.entry_size != 0)
        {
            return Err(HostLinkError::new(
                code,
                "ELF section zero is not exactly null",
            ));
        }
        if index != 0 && section.section_type == elf::SHT_NULL {
            return Err(HostLinkError::new(code, "ELF has a nonzero null section"));
        }
        if section.flags & u64::from(elf::SHF_COMPRESSED) != 0
            || section.section_type == elf::SHT_CREL
        {
            return Err(HostLinkError::new(
                code,
                "compressed and CREL sections are outside the V1 ELF subset",
            ));
        }
        if !executable && section.section_type == elf::SHT_LLVM_DEPENDENT_LIBRARIES {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::Lto,
                "LLVM dependent-library sections are outside HostLinkClosureV1",
            ));
        }
        let supported = if executable {
            is_supported_output_section_type(section.section_type)
        } else {
            is_supported_relocatable_section_type(section.section_type)
        };
        if !supported {
            return Err(HostLinkError::new(
                code,
                "ELF section type is outside the V1 subset",
            ));
        }
        if !section_flags_are_valid(section.section_type, section.flags, executable) {
            return Err(HostLinkError::new(
                code,
                "ELF section flags are unsupported or incoherent",
            ));
        }
        if !executable && section.address != 0 {
            return Err(HostLinkError::new(
                code,
                "ET_REL section has a nonzero address",
            ));
        }
        if section.alignment > 1 && !section.alignment.is_power_of_two()
            || section.entry_size != 0 && !section.size.is_multiple_of(section.entry_size)
            || section.size != 0
                && section.section_type != elf::SHT_NOBITS
                && section.alignment > 1
                && !section.offset.is_multiple_of(section.alignment)
        {
            return Err(HostLinkError::new(
                code,
                "ELF section alignment, extent, or entry size is malformed",
            ));
        }
        if section.link != 0 && section.link >= section_count
            || matches!(section.section_type, elf::SHT_REL | elf::SHT_RELA)
                && (section.information == 0 || section.information >= section_count)
        {
            return Err(HostLinkError::new(
                code,
                "ELF section link or information index is invalid",
            ));
        }
        if section.section_type == elf::SHT_NOBITS {
            if section.offset > file_size {
                return Err(HostLinkError::new(
                    code,
                    "ELF NOBITS offset is outside the file",
                ));
            }
        } else {
            let end = section
                .offset
                .checked_add(section.size)
                .ok_or_else(|| HostLinkError::new(code, "ELF section file range overflows"))?;
            if end > file_size {
                return Err(HostLinkError::new(
                    code,
                    "ELF section extends beyond the file",
                ));
            }
            file_ranges.insert(section.offset, end, code, "section file")?;
        }
        Ok(())
    }

    fn is_supported_relocatable_section_type(section_type: u32) -> bool {
        matches!(
            section_type,
            elf::SHT_NULL
                | elf::SHT_PROGBITS
                | elf::SHT_SYMTAB
                | elf::SHT_STRTAB
                | elf::SHT_RELA
                | elf::SHT_REL
                | elf::SHT_NOTE
                | elf::SHT_NOBITS
                | elf::SHT_INIT_ARRAY
                | elf::SHT_FINI_ARRAY
                | elf::SHT_PREINIT_ARRAY
                | elf::SHT_GROUP
                | elf::SHT_SYMTAB_SHNDX
                | elf::SHT_X86_64_UNWIND
                | SHT_LLVM_ADDRSIG_V1
        )
    }

    fn validate_section_names(
        bytes: &[u8],
        sections: &[RawSectionV1],
        names_index: usize,
        code: HostLinkErrorCodeV1,
    ) -> Result<(), HostLinkError> {
        let names = sections[names_index];
        if names.section_type != elf::SHT_STRTAB {
            return Err(HostLinkError::new(
                code,
                "ELF section-name table is not STRTAB",
            ));
        }
        let strings = section_bytes(bytes, names, code)?;
        validate_string_table(strings, code, "section-name")?;
        if sections
            .iter()
            .any(|section| section.name as usize >= strings.len())
        {
            return Err(HostLinkError::new(
                code,
                "ELF section name is outside the section-name table",
            ));
        }
        Ok(())
    }

    fn validate_linked_section_tables(
        bytes: &[u8],
        sections: &[RawSectionV1],
        executable: bool,
        code: HostLinkErrorCodeV1,
    ) -> Result<(), HostLinkError> {
        let mut extended_indexes = BTreeMap::new();
        for (index, section) in sections.iter().copied().enumerate() {
            if section.section_type == elf::SHT_SYMTAB_SHNDX
                && (section.entry_size != 4
                    || section.link == 0
                    || sections[section.link].section_type != elf::SHT_SYMTAB
                    || extended_indexes.insert(section.link, index).is_some())
            {
                return Err(HostLinkError::new(
                    code,
                    "ELF SYMTAB_SHNDX linkage is malformed",
                ));
            }
        }
        let mut grouped_members = BTreeSet::new();
        for (index, section) in sections.iter().copied().enumerate() {
            match section.section_type {
                elf::SHT_NULL => {}
                elf::SHT_PROGBITS => {
                    validate_progbits_section(bytes, sections, index, section, code)?;
                }
                elf::SHT_STRTAB => {
                    if section.link != 0 || section.information != 0 || section.entry_size != 0 {
                        return Err(HostLinkError::new(code, "ELF STRTAB linkage is malformed"));
                    }
                    validate_string_table(section_bytes(bytes, section, code)?, code, "string")?;
                }
                elf::SHT_SYMTAB => validate_symbol_table(
                    bytes,
                    sections,
                    index,
                    extended_indexes.get(&index).copied(),
                    executable,
                    code,
                )?,
                elf::SHT_REL | elf::SHT_RELA => {
                    validate_relocation_table(bytes, sections, section, code)?;
                }
                elf::SHT_GROUP => {
                    for member in validate_group_section(bytes, sections, index, section, code)? {
                        if !grouped_members.insert(member) {
                            return Err(HostLinkError::new(
                                code,
                                "ELF section belongs to more than one GROUP",
                            ));
                        }
                    }
                }
                elf::SHT_NOTE => {
                    if section.link != 0 || section.information != 0 {
                        return Err(HostLinkError::new(code, "ELF NOTE linkage is malformed"));
                    }
                    validate_note_section(bytes, section, code)?;
                }
                elf::SHT_SYMTAB_SHNDX => {
                    let symbols = sections[section.link];
                    if section.information != 0 || section.size / 4 != symbols.size / 24 {
                        return Err(HostLinkError::new(
                            code,
                            "ELF SYMTAB_SHNDX cardinality does not match its symbol table",
                        ));
                    }
                    for entry in section_bytes(bytes, section, code)?.chunks_exact(4) {
                        let index = u32_at(entry, 0) as usize;
                        if index != 0 && index >= sections.len() {
                            return Err(HostLinkError::new(
                                code,
                                "ELF SYMTAB_SHNDX entry is outside the section table",
                            ));
                        }
                    }
                }
                elf::SHT_INIT_ARRAY | elf::SHT_FINI_ARRAY | elf::SHT_PREINIT_ARRAY => {
                    if section.link != 0
                        || section.information != 0
                        || !matches!(section.entry_size, 0 | 8)
                    {
                        return Err(HostLinkError::new(
                            code,
                            "ELF array linkage or entry size is invalid",
                        ));
                    }
                }
                elf::SHT_NOBITS | elf::SHT_X86_64_UNWIND => {
                    let link_order = section.flags & SHF_LINK_ORDER_V1 != 0;
                    if section.information != 0
                        || section.entry_size != 0
                        || link_order && (section.link == 0 || section.link == index)
                        || !link_order && section.link != 0
                    {
                        return Err(HostLinkError::new(
                            code,
                            "ELF NOBITS/unwind linkage is malformed",
                        ));
                    }
                }
                SHT_LLVM_ADDRSIG_V1 => {
                    if section.link == 0
                        || section.link == index
                        || sections[section.link].section_type != elf::SHT_SYMTAB
                        || section.information != 0
                        || section.entry_size != 0
                    {
                        return Err(HostLinkError::new(
                            code,
                            "LLVM address-significance linkage is malformed",
                        ));
                    }
                }
                _ => unreachable!("section type was admitted by the finite V1 grammar"),
            }
        }
        for (index, section) in sections.iter().enumerate() {
            if (section.flags & SHF_GROUP_V1 != 0) != grouped_members.contains(&index) {
                return Err(HostLinkError::new(
                    code,
                    "ELF SHF_GROUP membership does not match GROUP tables",
                ));
            }
        }
        Ok(())
    }

    fn validate_progbits_section(
        bytes: &[u8],
        sections: &[RawSectionV1],
        index: usize,
        section: RawSectionV1,
        code: HostLinkErrorCodeV1,
    ) -> Result<(), HostLinkError> {
        let link_order = section.flags & SHF_LINK_ORDER_V1 != 0;
        if section.information != 0
            || link_order && (section.link == 0 || section.link == index)
            || !link_order && section.link != 0
            || !merge_entry_size_is_valid(section.flags, section.size, section.entry_size)
        {
            return Err(HostLinkError::new(
                code,
                "ELF PROGBITS linkage or merge encoding is malformed",
            ));
        }
        if link_order && sections[section.link].section_type == elf::SHT_NULL {
            return Err(HostLinkError::new(
                code,
                "ELF PROGBITS link-order target is null",
            ));
        }
        validate_merge_string_terminator(bytes, section, code)
    }

    fn validate_merge_string_terminator(
        bytes: &[u8],
        section: RawSectionV1,
        code: HostLinkErrorCodeV1,
    ) -> Result<(), HostLinkError> {
        if section.flags & SHF_STRINGS_V1 == 0 || section.size == 0 {
            return Ok(());
        }
        let width = usize::try_from(section.entry_size)
            .map_err(|_| HostLinkError::new(code, "ELF merge-string width does not fit memory"))?;
        let data = section_bytes(bytes, section, code)?;
        if data[data.len() - width..].iter().any(|byte| *byte != 0) {
            return Err(HostLinkError::new(
                code,
                "ELF merge-string section is not element-width NUL terminated",
            ));
        }
        Ok(())
    }

    fn validate_symbol_table(
        bytes: &[u8],
        sections: &[RawSectionV1],
        symbol_index: usize,
        extended_index: Option<usize>,
        executable: bool,
        code: HostLinkErrorCodeV1,
    ) -> Result<(), HostLinkError> {
        let section = sections[symbol_index];
        if section.entry_size != 24
            || section.link == 0
            || sections[section.link].section_type != elf::SHT_STRTAB
            || section.size / 24 > crate::MAX_HOST_LINK_ELF_TABLE_ENTRIES_V1
        {
            return Err(HostLinkError::new(
                code,
                "ELF symbol-table encoding is malformed",
            ));
        }
        let count = section.size / 24;
        if section.information == 0 || section.information as u64 > count {
            return Err(HostLinkError::new(
                code,
                "ELF symbol local/global partition is malformed",
            ));
        }
        let symbols = section_bytes(bytes, section, code)?;
        let strings = section_bytes(bytes, sections[section.link], code)?;
        let extended = extended_index
            .map(|index| section_bytes(bytes, sections[index], code))
            .transpose()?;
        for entry_index in 0..count {
            let offset = usize::try_from(entry_index * 24).map_err(|_| {
                HostLinkError::new(code, "ELF symbol-table offset does not fit memory")
            })?;
            let symbol = &symbols[offset..offset + 24];
            if entry_index == 0 && symbol != [0; 24] {
                return Err(HostLinkError::new(
                    code,
                    "ELF symbol zero is not exactly null",
                ));
            }
            let name = u32_at(symbol, 0) as usize;
            let binding = symbol[4] >> 4;
            let symbol_type = symbol[4] & 0xf;
            let visibility = symbol[5];
            let raw_section = u16_at(symbol, 6);
            if name >= strings.len()
                || !strings[name..].contains(&0)
                || !matches!(binding, 0..=2 | 10)
                || !matches!(symbol_type, 0..=6 | 10)
                || visibility & !3 != 0
                || entry_index < section.information as u64 && binding != 0
                || entry_index >= section.information as u64 && binding == 0
            {
                return Err(HostLinkError::new(
                    code,
                    "ELF symbol name, info, visibility, or binding partition is invalid",
                ));
            }
            let effective_section = if raw_section == elf::SHN_XINDEX {
                let indexes = extended.ok_or_else(|| {
                    HostLinkError::new(code, "ELF symbol uses SHN_XINDEX without SYMTAB_SHNDX")
                })?;
                u32_at(indexes, entry_index as usize * 4) as usize
            } else {
                usize::from(raw_section)
            };
            if raw_section == elf::SHN_XINDEX
                && (effective_section == 0 || effective_section >= sections.len())
                || effective_section < usize::from(elf::SHN_LORESERVE)
                    && effective_section >= sections.len()
                || effective_section >= usize::from(elf::SHN_LORESERVE)
                    && !matches!(
                        raw_section,
                        elf::SHN_XINDEX | elf::SHN_ABS | elf::SHN_COMMON
                    )
            {
                return Err(HostLinkError::new(
                    code,
                    "ELF symbol references an unsupported section index",
                ));
            }
            if !executable && effective_section != 0 && effective_section < sections.len() {
                let value = u64_at(symbol, 8);
                let size = u64_at(symbol, 16);
                if value
                    .checked_add(size)
                    .is_none_or(|end| end > sections[effective_section].size)
                {
                    return Err(HostLinkError::new(
                        code,
                        "ET_REL symbol extent is outside its target section",
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_relocation_table(
        bytes: &[u8],
        sections: &[RawSectionV1],
        section: RawSectionV1,
        code: HostLinkErrorCodeV1,
    ) -> Result<(), HostLinkError> {
        let expected_size = if section.section_type == elf::SHT_REL {
            16
        } else {
            24
        };
        if section.entry_size != expected_size
            || section.link == 0
            || sections[section.link].section_type != elf::SHT_SYMTAB
            || section.information == 0
            || section.information >= sections.len()
        {
            return Err(HostLinkError::new(
                code,
                "ELF relocation linkage is malformed",
            ));
        }
        let count = section.size / expected_size;
        if count > crate::MAX_HOST_LINK_ELF_TABLE_ENTRIES_V1 {
            return Err(HostLinkError::new(
                code,
                "ELF relocation count exceeds its traversal bound",
            ));
        }
        let symbol_count = sections[section.link].size / 24;
        let target = sections[section.information];
        if !matches!(
            target.section_type,
            elf::SHT_PROGBITS
                | elf::SHT_NOBITS
                | elf::SHT_INIT_ARRAY
                | elf::SHT_FINI_ARRAY
                | elf::SHT_PREINIT_ARRAY
                | elf::SHT_NOTE
                | elf::SHT_X86_64_UNWIND
        ) {
            return Err(HostLinkError::new(
                code,
                format!(
                    "ELF relocation target type 0x{:x} is not a linkable data/code section",
                    target.section_type
                ),
            ));
        }
        let relocations = section_bytes(bytes, section, code)?;
        for index in 0..count {
            let offset = usize::try_from(index * expected_size).map_err(|_| {
                HostLinkError::new(code, "ELF relocation offset does not fit memory")
            })?;
            let relocation_offset = u64_at(relocations, offset);
            let information = u64_at(relocations, offset + 8);
            let relocation_type = information as u32;
            if relocation_offset >= target.size
                || information >> 32 >= symbol_count
                || !is_pinned_llvm_x86_64_relocation(relocation_type)
            {
                return Err(HostLinkError::new(
                    code,
                    "ELF relocation offset, symbol, target, or type is unsupported",
                ));
            }
        }
        Ok(())
    }

    fn is_pinned_llvm_x86_64_relocation(kind: u32) -> bool {
        // This is the complete set in pinned LLVM's ELFRelocs/x86_64.def. Named
        // object::elf constants make additions an explicit cross-lane protocol change.
        matches!(
            kind,
            elf::R_X86_64_NONE
                | elf::R_X86_64_64
                | elf::R_X86_64_PC32
                | elf::R_X86_64_GOT32
                | elf::R_X86_64_PLT32
                | elf::R_X86_64_COPY
                | elf::R_X86_64_GLOB_DAT
                | elf::R_X86_64_JUMP_SLOT
                | elf::R_X86_64_RELATIVE
                | elf::R_X86_64_GOTPCREL
                | elf::R_X86_64_32
                | elf::R_X86_64_32S
                | elf::R_X86_64_16
                | elf::R_X86_64_PC16
                | elf::R_X86_64_8
                | elf::R_X86_64_PC8
                | elf::R_X86_64_DTPMOD64
                | elf::R_X86_64_DTPOFF64
                | elf::R_X86_64_TPOFF64
                | elf::R_X86_64_TLSGD
                | elf::R_X86_64_TLSLD
                | elf::R_X86_64_DTPOFF32
                | elf::R_X86_64_GOTTPOFF
                | elf::R_X86_64_TPOFF32
                | elf::R_X86_64_PC64
                | elf::R_X86_64_GOTOFF64
                | elf::R_X86_64_GOTPC32
                | elf::R_X86_64_GOT64
                | elf::R_X86_64_GOTPCREL64
                | elf::R_X86_64_GOTPC64
                | elf::R_X86_64_GOTPLT64
                | elf::R_X86_64_PLTOFF64
                | elf::R_X86_64_SIZE32
                | elf::R_X86_64_SIZE64
                | elf::R_X86_64_GOTPC32_TLSDESC
                | elf::R_X86_64_TLSDESC_CALL
                | elf::R_X86_64_TLSDESC
                | elf::R_X86_64_IRELATIVE
                | elf::R_X86_64_GOTPCRELX
                | elf::R_X86_64_REX_GOTPCRELX
                | elf::R_X86_64_CODE_4_GOTPCRELX
                | elf::R_X86_64_CODE_4_GOTTPOFF
                | elf::R_X86_64_CODE_4_GOTPC32_TLSDESC
                | elf::R_X86_64_CODE_6_GOTTPOFF
        )
    }

    fn validate_group_section(
        bytes: &[u8],
        sections: &[RawSectionV1],
        group_index: usize,
        section: RawSectionV1,
        code: HostLinkErrorCodeV1,
    ) -> Result<BTreeSet<usize>, HostLinkError> {
        if section.entry_size != 4
            || section.size < 8
            || section.link == 0
            || section.link == group_index
            || sections[section.link].section_type != elf::SHT_SYMTAB
            || section.information == 0
            || section.information as u64 >= sections[section.link].size / 24
            || section.size / 4 > crate::MAX_HOST_LINK_ELF_TABLE_ENTRIES_V1
        {
            return Err(HostLinkError::new(code, "ELF GROUP linkage is malformed"));
        }
        let data = section_bytes(bytes, section, code)?;
        if u32_at(data, 0) & !1 != 0 {
            return Err(HostLinkError::new(code, "ELF GROUP flags are unsupported"));
        }
        let mut members = std::collections::BTreeSet::new();
        for offset in (4..data.len()).step_by(4) {
            let member = u32_at(data, offset) as usize;
            if member == 0
                || member >= sections.len()
                || member == group_index
                || sections[member].section_type == elf::SHT_GROUP
                || sections[member].flags & u64::from(elf::SHF_GROUP) == 0
                || !members.insert(member)
            {
                return Err(HostLinkError::new(
                    code,
                    "ELF GROUP member index is invalid",
                ));
            }
        }
        Ok(members)
    }

    fn validate_note_section(
        bytes: &[u8],
        section: RawSectionV1,
        code: HostLinkErrorCodeV1,
    ) -> Result<(), HostLinkError> {
        if section.entry_size != 0 {
            return Err(HostLinkError::new(code, "ELF NOTE entry size must be zero"));
        }
        let data = section_bytes(bytes, section, code)?;
        let mut offset = 0_u64;
        let mut count = 0_u64;
        while offset < section.size {
            count = count
                .checked_add(1)
                .ok_or_else(|| HostLinkError::new(code, "ELF note count overflowed"))?;
            if count > crate::MAX_HOST_LINK_ELF_TABLE_ENTRIES_V1 || section.size - offset < 12 {
                return Err(HostLinkError::new(
                    code,
                    "ELF NOTE traversal bound is exceeded",
                ));
            }
            let start = usize::try_from(offset)
                .map_err(|_| HostLinkError::new(code, "ELF NOTE offset does not fit memory"))?;
            let name_size = u64::from(u32_at(data, start));
            let description_size = u64::from(u32_at(data, start + 4));
            let record_size = 12_u64
                .checked_add(align_up_4_with_code(name_size, code)?)
                .and_then(|size| {
                    size.checked_add(align_up_4_with_code(description_size, code).ok()?)
                })
                .ok_or_else(|| HostLinkError::new(code, "ELF NOTE record size overflows"))?;
            offset = offset
                .checked_add(record_size)
                .filter(|end| *end <= section.size)
                .ok_or_else(|| HostLinkError::new(code, "ELF NOTE record is truncated"))?;
        }
        Ok(())
    }

    fn validate_string_table(
        bytes: &[u8],
        code: HostLinkErrorCodeV1,
        name: &str,
    ) -> Result<(), HostLinkError> {
        if bytes.is_empty() || bytes.first() != Some(&0) || bytes.last() != Some(&0) {
            return Err(HostLinkError::new(
                code,
                format!("ELF {name} string table is not NUL bounded"),
            ));
        }
        Ok(())
    }

    fn section_bytes(
        bytes: &[u8],
        section: RawSectionV1,
        code: HostLinkErrorCodeV1,
    ) -> Result<&[u8], HostLinkError> {
        bounded_bytes(bytes, section.offset, section.size, code, "ELF section")
    }

    fn bounded_bytes<'a>(
        bytes: &'a [u8],
        offset: u64,
        size: u64,
        code: HostLinkErrorCodeV1,
        name: &str,
    ) -> Result<&'a [u8], HostLinkError> {
        let start = usize::try_from(offset)
            .map_err(|_| HostLinkError::new(code, format!("{name} offset does not fit memory")))?;
        let length = usize::try_from(size)
            .map_err(|_| HostLinkError::new(code, format!("{name} size does not fit memory")))?;
        let end = start
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| HostLinkError::new(code, format!("{name} extends beyond the file")))?;
        Ok(&bytes[start..end])
    }

    fn raw_table_end(
        offset: u64,
        count: usize,
        entry_size: u64,
        file_size: usize,
        code: HostLinkErrorCodeV1,
    ) -> Result<u64, HostLinkError> {
        offset
            .checked_add(
                (count as u64)
                    .checked_mul(entry_size)
                    .ok_or_else(|| HostLinkError::new(code, "ELF table length overflows"))?,
            )
            .filter(|end| *end <= file_size as u64)
            .ok_or_else(|| HostLinkError::new(code, "ELF table extends beyond the file"))
    }

    fn indexed_raw_offset(
        base: u64,
        index: usize,
        entry_size: u64,
        code: HostLinkErrorCodeV1,
    ) -> Result<u64, HostLinkError> {
        base.checked_add(
            (index as u64)
                .checked_mul(entry_size)
                .ok_or_else(|| HostLinkError::new(code, "ELF table index overflows"))?,
        )
        .ok_or_else(|| HostLinkError::new(code, "ELF table offset overflows"))
    }

    fn align_up_4_with_code(value: u64, code: HostLinkErrorCodeV1) -> Result<u64, HostLinkError> {
        value
            .checked_add(3)
            .map(|value| value & !3)
            .ok_or_else(|| HostLinkError::new(code, "ELF note field alignment overflows"))
    }

    pub(crate) fn inspect_elf(bytes: &[u8]) -> Result<ElfProfileV1, HostLinkError> {
        if bytes.len() < 64
            || bytes.get(..4) != Some(b"\x7fELF")
            || bytes[4] != ElfClassV1::Elf64 as u8
            || bytes[5] != ElfEndianV1::Little as u8
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ElfPolicy,
                "artifact is not a bounded ELF64 little-endian object",
            ));
        }
        let file = ElfFile64::<Endianness>::parse(bytes).map_err(|_| {
            HostLinkError::new(
                HostLinkErrorCodeV1::ElfPolicy,
                "structured ELF parser rejected the artifact",
            )
        })?;
        let endian = file.endian();
        if endian != Endianness::Little {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ElfPolicy,
                "ELF endianness is not little-endian",
            ));
        }
        let header = file.elf_header();
        let mut interpreter = None;
        let mut has_wx = false;
        let mut has_executable_stack = false;
        for segment in file.elf_program_headers() {
            let segment_type = segment.p_type(endian);
            let flags = segment.p_flags(endian);
            if segment_type == elf::PT_LOAD && flags & elf::PF_W != 0 && flags & elf::PF_X != 0 {
                has_wx = true;
            }
            if segment_type == elf::PT_GNU_STACK && flags & elf::PF_X != 0 {
                has_executable_stack = true;
            }
            let segment_interpreter = segment.interpreter(endian, bytes).map_err(|_| {
                HostLinkError::new(HostLinkErrorCodeV1::ElfPolicy, "invalid PT_INTERP segment")
            })?;
            if segment_interpreter
                .is_some_and(|value| interpreter.replace(value.to_vec()).is_some())
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "multiple PT_INTERP segments are not admitted",
                ));
            }
        }
        let dynamic = file.elf_dynamic_table().map_err(|_| {
            HostLinkError::new(HostLinkErrorCodeV1::ElfPolicy, "invalid ELF dynamic table")
        })?;
        let mut soname = None;
        let mut needed = Vec::new();
        for entry in dynamic.iter() {
            if entry.tag == elf::DT_NEEDED {
                needed.push(
                    dynamic
                        .string(entry)
                        .map_err(|_| {
                            HostLinkError::new(
                                HostLinkErrorCodeV1::ElfPolicy,
                                "invalid DT_NEEDED string",
                            )
                        })?
                        .to_vec(),
                );
            }
            if entry.tag == elf::DT_SONAME {
                let value = dynamic.string(entry).map_err(|_| {
                    HostLinkError::new(HostLinkErrorCodeV1::ElfPolicy, "invalid DT_SONAME string")
                })?;
                if soname.replace(value.to_vec()).is_some() {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::ElfPolicy,
                        "ELF contains multiple DT_SONAME entries",
                    ));
                }
            }
            if matches!(entry.tag, elf::DT_RPATH | elf::DT_RUNPATH) {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ElfPolicy,
                    "RPATH and RUNPATH are outside the V1 output policy",
                ));
            }
        }
        needed.sort();
        if needed.windows(2).any(|window| window[0] == window[1]) {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ElfPolicy,
                "ELF contains duplicate DT_NEEDED entries",
            ));
        }
        Ok(ElfProfileV1 {
            class: ElfClassV1::Elf64,
            endian: ElfEndianV1::Little,
            elf_type: header.e_type(endian),
            machine: header.e_machine(endian),
            interpreter,
            soname,
            needed,
            has_writable_executable_segment: has_wx,
            has_executable_stack,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::io::{Seek, SeekFrom};

        fn static_output() -> Vec<u8> {
            let mut bytes = vec![0_u8; 121];
            let file_size = bytes.len() as u64;
            bytes[..4].copy_from_slice(b"\x7fELF");
            bytes[4] = 2;
            bytes[5] = 1;
            bytes[6] = 1;
            bytes[16..18].copy_from_slice(&elf::ET_EXEC.to_le_bytes());
            bytes[18..20].copy_from_slice(&elf::EM_X86_64.to_le_bytes());
            bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
            bytes[24..32].copy_from_slice(&0x400078_u64.to_le_bytes());
            bytes[32..40].copy_from_slice(&64_u64.to_le_bytes());
            bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
            bytes[54..56].copy_from_slice(&56_u16.to_le_bytes());
            bytes[56..58].copy_from_slice(&1_u16.to_le_bytes());
            bytes[58..60].copy_from_slice(&64_u16.to_le_bytes());
            bytes[64..68].copy_from_slice(&elf::PT_LOAD.to_le_bytes());
            bytes[68..72].copy_from_slice(&(elf::PF_R | elf::PF_X).to_le_bytes());
            bytes[72..80].copy_from_slice(&0_u64.to_le_bytes());
            bytes[80..88].copy_from_slice(&0x400000_u64.to_le_bytes());
            bytes[88..96].copy_from_slice(&0x400000_u64.to_le_bytes());
            bytes[96..104].copy_from_slice(&file_size.to_le_bytes());
            bytes[104..112].copy_from_slice(&file_size.to_le_bytes());
            bytes[112..120].copy_from_slice(&0x1000_u64.to_le_bytes());
            bytes[120] = 0xc3;
            bytes
        }

        fn sealed_static_output_with_size(size: u64) -> File {
            assert!(size >= 121);
            let descriptor = rustix::fs::memfd_create(
                "fe2o3-incremental-output-test",
                MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
            )
            .unwrap();
            let file = File::from(descriptor);
            rustix::fs::ftruncate(&file, size).unwrap();
            pwrite_all(&file, &static_output(), 0).unwrap();
            file.set_permissions(std::fs::Permissions::from_mode(0o444))
                .unwrap();
            rustix::fs::fcntl_add_seals(&file, REQUIRED_SEALS).unwrap();
            file
        }

        fn captured_output(bytes: &[u8]) -> CapturedOutputFileV1 {
            let descriptor = rustix::fs::memfd_create(
                "fe2o3-elf-inspection-test",
                MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
            )
            .unwrap();
            let file = File::from(descriptor);
            pwrite_all(&file, bytes, 0).unwrap();
            file.set_permissions(std::fs::Permissions::from_mode(0o555))
                .unwrap();
            rustix::fs::fcntl_add_seals(&file, REQUIRED_SEALS).unwrap();
            CapturedOutputFileV1 {
                file,
                sha256: sha256_bytes(bytes),
                size: bytes.len() as u64,
                mode: 0o555,
            }
        }

        fn relocatable_with_out_of_bounds_text() -> Vec<u8> {
            let mut bytes = vec![0_u8; 263];
            bytes[..4].copy_from_slice(b"\x7fELF");
            bytes[4] = 2;
            bytes[5] = 1;
            bytes[6] = 1;
            bytes[16..18].copy_from_slice(&elf::ET_REL.to_le_bytes());
            bytes[18..20].copy_from_slice(&elf::EM_X86_64.to_le_bytes());
            bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
            bytes[40..48].copy_from_slice(&64_u64.to_le_bytes());
            bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
            bytes[54..56].copy_from_slice(&56_u16.to_le_bytes());
            bytes[58..60].copy_from_slice(&64_u16.to_le_bytes());
            bytes[60..62].copy_from_slice(&3_u16.to_le_bytes());
            bytes[62..64].copy_from_slice(&1_u16.to_le_bytes());

            let string_section = 64 + 64;
            bytes[string_section + 4..string_section + 8]
                .copy_from_slice(&elf::SHT_STRTAB.to_le_bytes());
            bytes[string_section + 24..string_section + 32].copy_from_slice(&256_u64.to_le_bytes());
            bytes[string_section + 32..string_section + 40].copy_from_slice(&7_u64.to_le_bytes());
            bytes[string_section + 48..string_section + 56].copy_from_slice(&1_u64.to_le_bytes());

            let text_section = 64 + 128;
            bytes[text_section..text_section + 4].copy_from_slice(&1_u32.to_le_bytes());
            bytes[text_section + 4..text_section + 8]
                .copy_from_slice(&elf::SHT_PROGBITS.to_le_bytes());
            bytes[text_section + 24..text_section + 32].copy_from_slice(&4096_u64.to_le_bytes());
            bytes[text_section + 32..text_section + 40].copy_from_slice(&16_u64.to_le_bytes());
            bytes[text_section + 48..text_section + 56].copy_from_slice(&16_u64.to_le_bytes());
            bytes[256..].copy_from_slice(b"\0.text\0");
            bytes
        }

        fn relocatable_with_symbols_and_relocation() -> Vec<u8> {
            const SECTION_TABLE: usize = 256;
            const SECTION_COUNT: usize = 6;
            const SHSTRTAB: &[u8] = b"\0.shstrtab\0.text\0.strtab\0.symtab\0.rela.text\0";
            let mut bytes = vec![0_u8; SECTION_TABLE + SECTION_COUNT * 64];
            bytes[..4].copy_from_slice(b"\x7fELF");
            bytes[4] = 2;
            bytes[5] = 1;
            bytes[6] = 1;
            bytes[16..18].copy_from_slice(&elf::ET_REL.to_le_bytes());
            bytes[18..20].copy_from_slice(&elf::EM_X86_64.to_le_bytes());
            bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
            bytes[40..48].copy_from_slice(&(SECTION_TABLE as u64).to_le_bytes());
            bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
            bytes[54..56].copy_from_slice(&56_u16.to_le_bytes());
            bytes[58..60].copy_from_slice(&64_u16.to_le_bytes());
            bytes[60..62].copy_from_slice(&(SECTION_COUNT as u16).to_le_bytes());
            bytes[62..64].copy_from_slice(&1_u16.to_le_bytes());
            bytes[64..64 + SHSTRTAB.len()].copy_from_slice(SHSTRTAB);
            bytes[128] = 0xc3;
            bytes[129..134].copy_from_slice(b"\0sym\0");
            bytes[136 + 24..136 + 28].copy_from_slice(&1_u32.to_le_bytes());
            bytes[136 + 24 + 4] = 0x12;
            bytes[136 + 24 + 6..136 + 24 + 8].copy_from_slice(&2_u16.to_le_bytes());
            bytes[136 + 24 + 16..136 + 24 + 24].copy_from_slice(&1_u64.to_le_bytes());
            bytes[184 + 8..184 + 16].copy_from_slice(&((1_u64 << 32) | 2).to_le_bytes());

            let section = |index: usize| SECTION_TABLE + index * 64;
            let shstrtab = section(1);
            bytes[shstrtab..shstrtab + 4].copy_from_slice(&1_u32.to_le_bytes());
            bytes[shstrtab + 4..shstrtab + 8].copy_from_slice(&elf::SHT_STRTAB.to_le_bytes());
            bytes[shstrtab + 24..shstrtab + 32].copy_from_slice(&64_u64.to_le_bytes());
            bytes[shstrtab + 32..shstrtab + 40]
                .copy_from_slice(&(SHSTRTAB.len() as u64).to_le_bytes());
            bytes[shstrtab + 48..shstrtab + 56].copy_from_slice(&1_u64.to_le_bytes());

            let text = section(2);
            bytes[text..text + 4].copy_from_slice(&11_u32.to_le_bytes());
            bytes[text + 4..text + 8].copy_from_slice(&elf::SHT_PROGBITS.to_le_bytes());
            bytes[text + 8..text + 16]
                .copy_from_slice(&u64::from(elf::SHF_ALLOC | elf::SHF_EXECINSTR).to_le_bytes());
            bytes[text + 24..text + 32].copy_from_slice(&128_u64.to_le_bytes());
            bytes[text + 32..text + 40].copy_from_slice(&1_u64.to_le_bytes());
            bytes[text + 48..text + 56].copy_from_slice(&1_u64.to_le_bytes());

            let strtab = section(3);
            bytes[strtab..strtab + 4].copy_from_slice(&17_u32.to_le_bytes());
            bytes[strtab + 4..strtab + 8].copy_from_slice(&elf::SHT_STRTAB.to_le_bytes());
            bytes[strtab + 24..strtab + 32].copy_from_slice(&129_u64.to_le_bytes());
            bytes[strtab + 32..strtab + 40].copy_from_slice(&5_u64.to_le_bytes());
            bytes[strtab + 48..strtab + 56].copy_from_slice(&1_u64.to_le_bytes());

            let symtab = section(4);
            bytes[symtab..symtab + 4].copy_from_slice(&25_u32.to_le_bytes());
            bytes[symtab + 4..symtab + 8].copy_from_slice(&elf::SHT_SYMTAB.to_le_bytes());
            bytes[symtab + 24..symtab + 32].copy_from_slice(&136_u64.to_le_bytes());
            bytes[symtab + 32..symtab + 40].copy_from_slice(&48_u64.to_le_bytes());
            bytes[symtab + 40..symtab + 44].copy_from_slice(&3_u32.to_le_bytes());
            bytes[symtab + 44..symtab + 48].copy_from_slice(&1_u32.to_le_bytes());
            bytes[symtab + 48..symtab + 56].copy_from_slice(&8_u64.to_le_bytes());
            bytes[symtab + 56..symtab + 64].copy_from_slice(&24_u64.to_le_bytes());

            let rela = section(5);
            bytes[rela..rela + 4].copy_from_slice(&33_u32.to_le_bytes());
            bytes[rela + 4..rela + 8].copy_from_slice(&elf::SHT_RELA.to_le_bytes());
            bytes[rela + 24..rela + 32].copy_from_slice(&184_u64.to_le_bytes());
            bytes[rela + 32..rela + 40].copy_from_slice(&24_u64.to_le_bytes());
            bytes[rela + 40..rela + 44].copy_from_slice(&4_u32.to_le_bytes());
            bytes[rela + 44..rela + 48].copy_from_slice(&2_u32.to_le_bytes());
            bytes[rela + 48..rela + 56].copy_from_slice(&8_u64.to_le_bytes());
            bytes[rela + 56..rela + 64].copy_from_slice(&24_u64.to_le_bytes());
            bytes
        }

        fn relocatable_with_group() -> Vec<u8> {
            const SECTION_TABLE: usize = 256;
            const GROUP_INDEX: usize = 6;
            const OLD_NAMES_LENGTH: usize =
                b"\0.shstrtab\0.text\0.strtab\0.symtab\0.rela.text\0".len();
            let mut bytes = relocatable_with_symbols_and_relocation();
            bytes.resize(SECTION_TABLE + (GROUP_INDEX + 1) * 64, 0);
            bytes[60..62].copy_from_slice(&((GROUP_INDEX + 1) as u16).to_le_bytes());
            bytes[64 + OLD_NAMES_LENGTH..64 + OLD_NAMES_LENGTH + 7].copy_from_slice(b".group\0");
            let shstrtab = SECTION_TABLE + 64;
            bytes[shstrtab + 32..shstrtab + 40]
                .copy_from_slice(&((OLD_NAMES_LENGTH + 7) as u64).to_le_bytes());
            let text = SECTION_TABLE + 2 * 64;
            bytes[text + 8..text + 16].copy_from_slice(
                &u64::from(elf::SHF_ALLOC | elf::SHF_EXECINSTR | elf::SHF_GROUP).to_le_bytes(),
            );
            bytes[208..212].copy_from_slice(&1_u32.to_le_bytes());
            bytes[212..216].copy_from_slice(&2_u32.to_le_bytes());
            let group = SECTION_TABLE + GROUP_INDEX * 64;
            bytes[group..group + 4].copy_from_slice(&(OLD_NAMES_LENGTH as u32).to_le_bytes());
            bytes[group + 4..group + 8].copy_from_slice(&elf::SHT_GROUP.to_le_bytes());
            bytes[group + 24..group + 32].copy_from_slice(&208_u64.to_le_bytes());
            bytes[group + 32..group + 40].copy_from_slice(&8_u64.to_le_bytes());
            bytes[group + 40..group + 44].copy_from_slice(&4_u32.to_le_bytes());
            bytes[group + 44..group + 48].copy_from_slice(&1_u32.to_le_bytes());
            bytes[group + 48..group + 56].copy_from_slice(&4_u64.to_le_bytes());
            bytes[group + 56..group + 64].copy_from_slice(&4_u64.to_le_bytes());
            bytes
        }

        fn archive(member: &[u8]) -> Vec<u8> {
            let mut bytes = b"!<arch>\n".to_vec();
            append_archive_record(&mut bytes, b"member.o/", member);
            bytes
        }

        fn append_archive_record(bytes: &mut Vec<u8>, name: &[u8], data: &[u8]) {
            assert!(name.len() <= 16);
            let mut header = [b' '; 60];
            header[..name.len()].copy_from_slice(name);
            header[16..28].copy_from_slice(b"0           ");
            header[28..34].copy_from_slice(b"0     ");
            header[34..40].copy_from_slice(b"0     ");
            header[40..48].copy_from_slice(b"100644  ");
            let size = format!("{:<10}", data.len());
            header[48..58].copy_from_slice(size.as_bytes());
            header[58..].copy_from_slice(b"`\n");
            bytes.extend_from_slice(&header);
            bytes.extend_from_slice(data);
            if !data.len().is_multiple_of(2) {
                bytes.push(b'\n');
            }
        }

        fn archive_with_gnu_symbols(member: &[u8], symbols: Vec<u8>) -> Vec<u8> {
            let mut bytes = b"!<arch>\n".to_vec();
            append_archive_record(&mut bytes, b"/", &symbols);
            append_archive_record(&mut bytes, b"member.o/", member);
            bytes
        }

        fn output_with_program_header(machine: u16, segment_type: u32, flags: u32) -> Vec<u8> {
            let mut bytes = vec![0_u8; 64 + 56];
            bytes[..4].copy_from_slice(b"\x7fELF");
            bytes[4] = 2;
            bytes[5] = 1;
            bytes[6] = 1;
            bytes[16..18].copy_from_slice(&elf::ET_EXEC.to_le_bytes());
            bytes[18..20].copy_from_slice(&machine.to_le_bytes());
            bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
            bytes[32..40].copy_from_slice(&64_u64.to_le_bytes());
            bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
            bytes[54..56].copy_from_slice(&56_u16.to_le_bytes());
            bytes[56..58].copy_from_slice(&1_u16.to_le_bytes());
            bytes[58..60].copy_from_slice(&64_u16.to_le_bytes());
            bytes[64..68].copy_from_slice(&segment_type.to_le_bytes());
            bytes[68..72].copy_from_slice(&flags.to_le_bytes());
            bytes
        }

        fn static_output_with_sections() -> Vec<u8> {
            const BASE: u64 = 0x400000;
            const TEXT: usize = 0x100;
            const SHSTRTAB: usize = 0x108;
            const STRTAB: usize = 0x138;
            const SYMTAB: usize = 0x140;
            const NOTE: usize = 0x170;
            const SECTION_TABLE: usize = 0x180;
            const SECTION_COUNT: usize = 6;
            const NAMES: &[u8] = b"\0.shstrtab\0.text\0.strtab\0.symtab\0.note.test\0";
            let mut bytes = vec![0_u8; SECTION_TABLE + SECTION_COUNT * 64];
            let file_size = bytes.len() as u64;
            bytes[..4].copy_from_slice(b"\x7fELF");
            bytes[4] = 2;
            bytes[5] = 1;
            bytes[6] = 1;
            bytes[16..18].copy_from_slice(&elf::ET_EXEC.to_le_bytes());
            bytes[18..20].copy_from_slice(&elf::EM_X86_64.to_le_bytes());
            bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
            bytes[24..32].copy_from_slice(&(BASE + TEXT as u64).to_le_bytes());
            bytes[32..40].copy_from_slice(&64_u64.to_le_bytes());
            bytes[40..48].copy_from_slice(&(SECTION_TABLE as u64).to_le_bytes());
            bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
            bytes[54..56].copy_from_slice(&56_u16.to_le_bytes());
            bytes[56..58].copy_from_slice(&1_u16.to_le_bytes());
            bytes[58..60].copy_from_slice(&64_u16.to_le_bytes());
            bytes[60..62].copy_from_slice(&(SECTION_COUNT as u16).to_le_bytes());
            bytes[62..64].copy_from_slice(&1_u16.to_le_bytes());
            bytes[64..68].copy_from_slice(&elf::PT_LOAD.to_le_bytes());
            bytes[68..72].copy_from_slice(&(elf::PF_R | elf::PF_X).to_le_bytes());
            bytes[80..88].copy_from_slice(&BASE.to_le_bytes());
            bytes[88..96].copy_from_slice(&BASE.to_le_bytes());
            bytes[96..104].copy_from_slice(&file_size.to_le_bytes());
            bytes[104..112].copy_from_slice(&file_size.to_le_bytes());
            bytes[112..120].copy_from_slice(&0x1000_u64.to_le_bytes());
            bytes[TEXT] = 0xc3;
            bytes[SHSTRTAB..SHSTRTAB + NAMES.len()].copy_from_slice(NAMES);
            bytes[STRTAB..STRTAB + 6].copy_from_slice(b"\0main\0");
            bytes[SYMTAB + 24..SYMTAB + 28].copy_from_slice(&1_u32.to_le_bytes());
            bytes[SYMTAB + 28] = 0x12;
            bytes[SYMTAB + 30..SYMTAB + 32].copy_from_slice(&2_u16.to_le_bytes());
            bytes[SYMTAB + 32..SYMTAB + 40].copy_from_slice(&(BASE + TEXT as u64).to_le_bytes());
            bytes[SYMTAB + 40..SYMTAB + 48].copy_from_slice(&1_u64.to_le_bytes());
            bytes[NOTE..NOTE + 4].copy_from_slice(&4_u32.to_le_bytes());
            bytes[NOTE + 8..NOTE + 12].copy_from_slice(&1_u32.to_le_bytes());
            bytes[NOTE + 12..NOTE + 16].copy_from_slice(b"GNU\0");

            let section = |index: usize| SECTION_TABLE + index * 64;
            let shstrtab = section(1);
            bytes[shstrtab..shstrtab + 4].copy_from_slice(&1_u32.to_le_bytes());
            bytes[shstrtab + 4..shstrtab + 8].copy_from_slice(&elf::SHT_STRTAB.to_le_bytes());
            bytes[shstrtab + 24..shstrtab + 32].copy_from_slice(&(SHSTRTAB as u64).to_le_bytes());
            bytes[shstrtab + 32..shstrtab + 40]
                .copy_from_slice(&(NAMES.len() as u64).to_le_bytes());
            bytes[shstrtab + 48..shstrtab + 56].copy_from_slice(&1_u64.to_le_bytes());

            let text = section(2);
            bytes[text..text + 4].copy_from_slice(&11_u32.to_le_bytes());
            bytes[text + 4..text + 8].copy_from_slice(&elf::SHT_PROGBITS.to_le_bytes());
            bytes[text + 8..text + 16]
                .copy_from_slice(&u64::from(elf::SHF_ALLOC | elf::SHF_EXECINSTR).to_le_bytes());
            bytes[text + 16..text + 24].copy_from_slice(&(BASE + TEXT as u64).to_le_bytes());
            bytes[text + 24..text + 32].copy_from_slice(&(TEXT as u64).to_le_bytes());
            bytes[text + 32..text + 40].copy_from_slice(&1_u64.to_le_bytes());
            bytes[text + 48..text + 56].copy_from_slice(&16_u64.to_le_bytes());

            let strtab = section(3);
            bytes[strtab..strtab + 4].copy_from_slice(&17_u32.to_le_bytes());
            bytes[strtab + 4..strtab + 8].copy_from_slice(&elf::SHT_STRTAB.to_le_bytes());
            bytes[strtab + 24..strtab + 32].copy_from_slice(&(STRTAB as u64).to_le_bytes());
            bytes[strtab + 32..strtab + 40].copy_from_slice(&6_u64.to_le_bytes());
            bytes[strtab + 48..strtab + 56].copy_from_slice(&1_u64.to_le_bytes());

            let symtab = section(4);
            bytes[symtab..symtab + 4].copy_from_slice(&25_u32.to_le_bytes());
            bytes[symtab + 4..symtab + 8].copy_from_slice(&elf::SHT_SYMTAB.to_le_bytes());
            bytes[symtab + 24..symtab + 32].copy_from_slice(&(SYMTAB as u64).to_le_bytes());
            bytes[symtab + 32..symtab + 40].copy_from_slice(&48_u64.to_le_bytes());
            bytes[symtab + 40..symtab + 44].copy_from_slice(&3_u32.to_le_bytes());
            bytes[symtab + 44..symtab + 48].copy_from_slice(&1_u32.to_le_bytes());
            bytes[symtab + 48..symtab + 56].copy_from_slice(&8_u64.to_le_bytes());
            bytes[symtab + 56..symtab + 64].copy_from_slice(&24_u64.to_le_bytes());

            let note = section(5);
            bytes[note..note + 4].copy_from_slice(&33_u32.to_le_bytes());
            bytes[note + 4..note + 8].copy_from_slice(&elf::SHT_NOTE.to_le_bytes());
            bytes[note + 24..note + 32].copy_from_slice(&(NOTE as u64).to_le_bytes());
            bytes[note + 32..note + 40].copy_from_slice(&16_u64.to_le_bytes());
            bytes[note + 48..note + 56].copy_from_slice(&4_u64.to_le_bytes());
            bytes
        }

        fn assert_incremental_output_rejected(bytes: &[u8]) {
            let captured = captured_output(bytes);
            let mut state = match IncrementalStaticOutputInspectionV1::new(captured) {
                Ok(state) => state,
                Err(error) => {
                    assert_eq!(error.code(), HostLinkErrorCodeV1::ElfPolicy);
                    return;
                }
            };
            loop {
                match state.advance(
                    1,
                    Instant::now() + std::time::Duration::from_secs(1),
                    Instant::now() + std::time::Duration::from_secs(1),
                ) {
                    Ok(IncrementalOutputInspectionProgressV1::Pending(next)) => state = next,
                    Ok(IncrementalOutputInspectionProgressV1::Complete(_, _)) => {
                        panic!("hostile descriptor ELF layout was admitted")
                    }
                    Err(error) => {
                        assert_eq!(error.code(), HostLinkErrorCodeV1::ElfPolicy);
                        return;
                    }
                }
            }
        }

        fn assert_incremental_output_admitted(bytes: &[u8]) {
            let captured = captured_output(bytes);
            let mut state = IncrementalStaticOutputInspectionV1::new(captured).unwrap();
            loop {
                match state
                    .advance(
                        1,
                        Instant::now() + std::time::Duration::from_secs(1),
                        Instant::now() + std::time::Duration::from_secs(1),
                    )
                    .unwrap()
                {
                    IncrementalOutputInspectionProgressV1::Pending(next) => state = next,
                    IncrementalOutputInspectionProgressV1::Complete(_, profile) => {
                        assert_eq!(profile.elf_type, elf::ET_EXEC);
                        return;
                    }
                }
            }
        }

        fn decode_hex_fixture(encoded: &str) -> Vec<u8> {
            let digits = encoded
                .bytes()
                .filter(|byte| !byte.is_ascii_whitespace())
                .collect::<Vec<_>>();
            assert!(digits.len().is_multiple_of(2));
            digits
                .chunks_exact(2)
                .map(|pair| {
                    let digit = |byte| match byte {
                        b'0'..=b'9' => byte - b'0',
                        b'a'..=b'f' => byte - b'a' + 10,
                        _ => panic!("fixture contains noncanonical hex"),
                    };
                    digit(pair[0]) << 4 | digit(pair[1])
                })
                .collect()
        }

        #[test]
        fn preserved_lld_and_rust_outputs_satisfy_the_exact_output_subset() {
            let fixtures = [
                (
                    include_str!("../tests/fixtures/minimal-static.hex"),
                    912,
                    "7ab5cb021cbe38abd72fffd64f20bcd71a981992df6dee0ddff8a5d4f0af7a5d",
                ),
                (
                    include_str!("../tests/fixtures/rust-static.hex"),
                    1000,
                    "a1fdb07712fd2acde1108bb3e9496dbe930243152b863cf648c20a6958c7fde6",
                ),
            ];
            for (encoded, expected_size, expected_sha256) in fixtures {
                let bytes = decode_hex_fixture(encoded);
                assert_eq!(bytes.len(), expected_size);
                assert_eq!(sha256_bytes(&bytes).to_hex(), expected_sha256);
                inspect_static_output_elf(&bytes).unwrap();
                assert_incremental_output_admitted(&bytes);
            }
        }

        #[test]
        fn merge_string_output_metadata_is_finite_and_content_checked() {
            const SECTION_TABLE: usize = 0x210;
            const COMMENT_DATA: usize = 0x129;
            const COMMENT_SIZE: usize = 0x79;
            let comment = SECTION_TABLE + 2 * 64;
            let valid = decode_hex_fixture(include_str!("../tests/fixtures/minimal-static.hex"));

            let mut strings_without_merge = valid.clone();
            strings_without_merge[comment + 8..comment + 16]
                .copy_from_slice(&SHF_STRINGS_V1.to_le_bytes());
            let mut entry_size_without_merge = valid.clone();
            entry_size_without_merge[comment + 8..comment + 16].fill(0);
            let mut unsupported_string_width = valid.clone();
            unsupported_string_width[comment + 32..comment + 40]
                .copy_from_slice(&120_u64.to_le_bytes());
            unsupported_string_width[comment + 56..comment + 64]
                .copy_from_slice(&8_u64.to_le_bytes());
            let mut missing_terminator = valid;
            missing_terminator[COMMENT_DATA + COMMENT_SIZE - 1] = b'x';

            for hostile in [
                strings_without_merge,
                entry_size_without_merge,
                unsupported_string_width,
                missing_terminator,
            ] {
                assert_incremental_output_rejected(&hostile);
            }
        }

        #[test]
        fn pinned_llvm_relocation_allowlist_has_no_reserved_or_newer_holes() {
            for kind in 0..=37 {
                assert!(is_pinned_llvm_x86_64_relocation(kind));
            }
            for kind in 41..=45 {
                assert!(is_pinned_llvm_x86_64_relocation(kind));
            }
            assert!(is_pinned_llvm_x86_64_relocation(50));
            for kind in [38, 39, 40, 46, 47, 48, 49, 51, u32::MAX] {
                assert!(!is_pinned_llvm_x86_64_relocation(kind));
            }
        }

        #[test]
        fn selected_static_tool_output_matrix_satisfies_the_exact_output_subset() {
            let Some(paths) = std::env::var_os("FE2O3_HOST_LINK_COMPAT_OUTPUTS") else {
                return;
            };
            let paths = std::env::split_paths(&paths).collect::<Vec<_>>();
            assert!(!paths.is_empty(), "static tool output matrix is empty");
            for path in paths {
                let bytes = std::fs::read(&path).unwrap_or_else(|error| {
                    panic!("read static tool output {}: {error}", path.display())
                });
                inspect_static_output_elf(&bytes)
                    .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
                assert_incremental_output_admitted(&bytes);
            }
        }

        #[test]
        fn actual_output_policy_rejects_wrong_machine_dynamic_wx_and_exec_stack() {
            for bytes in [
                output_with_program_header(elf::EM_AARCH64, elf::PT_LOAD, elf::PF_R),
                output_with_program_header(elf::EM_X86_64, elf::PT_DYNAMIC, elf::PF_R),
                output_with_program_header(
                    elf::EM_X86_64,
                    elf::PT_LOAD,
                    elf::PF_R | elf::PF_W | elf::PF_X,
                ),
                output_with_program_header(
                    elf::EM_X86_64,
                    elf::PT_GNU_STACK,
                    elf::PF_R | elf::PF_X,
                ),
            ] {
                assert_eq!(
                    inspect_static_output_elf(&bytes).unwrap_err().code(),
                    HostLinkErrorCodeV1::ElfPolicy
                );
            }
        }

        #[test]
        fn static_output_requires_bounded_loads_and_mapped_entry() {
            inspect_static_output_elf(&static_output()).unwrap();

            let mut entry_zero = static_output();
            entry_zero[24..32].fill(0);
            let mut segment_past_eof = static_output();
            segment_past_eof[72..80].copy_from_slice(&0x1000_u64.to_le_bytes());
            let mut file_larger_than_memory = static_output();
            file_larger_than_memory[104..112].copy_from_slice(&1_u64.to_le_bytes());
            let mut bad_alignment = static_output();
            bad_alignment[112..120].copy_from_slice(&3_u64.to_le_bytes());
            let mut no_executable_load = static_output();
            no_executable_load[68..72].copy_from_slice(&elf::PF_R.to_le_bytes());
            for hostile in [
                entry_zero,
                segment_past_eof,
                file_larger_than_memory,
                bad_alignment,
                no_executable_load,
            ] {
                assert_eq!(
                    inspect_static_output_elf(&hostile).unwrap_err().code(),
                    HostLinkErrorCodeV1::ElfPolicy
                );
            }
        }

        #[test]
        fn incremental_static_output_accepts_the_bounded_section_subset() {
            let captured = captured_output(&static_output_with_sections());
            let mut state = IncrementalStaticOutputInspectionV1::new(captured).unwrap();
            loop {
                match state
                    .advance(
                        1,
                        Instant::now() + std::time::Duration::from_secs(1),
                        Instant::now() + std::time::Duration::from_secs(1),
                    )
                    .unwrap()
                {
                    IncrementalOutputInspectionProgressV1::Pending(next) => state = next,
                    IncrementalOutputInspectionProgressV1::Complete(_, profile) => {
                        assert_eq!(profile.elf_type, elf::ET_EXEC);
                        break;
                    }
                }
            }
        }

        #[test]
        fn auditor_static_section_and_symbol_corpus_rejects() {
            const SECTION_TABLE: usize = 0x180;
            let section = |index: usize| SECTION_TABLE + index * 64;
            let valid = static_output_with_sections();

            let mut nonnull_zero = valid.clone();
            nonnull_zero[section(0) + 4..section(0) + 8]
                .copy_from_slice(&elf::SHT_PROGBITS.to_le_bytes());
            let mut misaligned_offset = valid.clone();
            misaligned_offset[section(2) + 24..section(2) + 32]
                .copy_from_slice(&0x101_u64.to_le_bytes());
            let mut overflowing_alloc = valid.clone();
            overflowing_alloc[section(2) + 16..section(2) + 24]
                .copy_from_slice(&u64::MAX.to_le_bytes());
            let mut unmapped_alloc = valid.clone();
            unmapped_alloc[section(2) + 16..section(2) + 24]
                .copy_from_slice(&0x900000_u64.to_le_bytes());
            let mut overlaps_section_headers = valid.clone();
            overlaps_section_headers[section(1) + 24..section(1) + 32]
                .copy_from_slice(&(SECTION_TABLE as u64).to_le_bytes());
            let mut compressed = valid.clone();
            compressed[section(2) + 8..section(2) + 16]
                .copy_from_slice(&u64::from(elf::SHF_COMPRESSED).to_le_bytes());
            let mut corrupt_note = valid.clone();
            corrupt_note[0x170..0x174].copy_from_slice(&u32::MAX.to_le_bytes());
            let mut bad_symbol_name = valid.clone();
            bad_symbol_name[0x158..0x15c].copy_from_slice(&6_u32.to_le_bytes());
            let mut bad_symbol_partition = valid.clone();
            bad_symbol_partition[0x15c] = 0x02;
            let mut unknown_section = valid.clone();
            unknown_section[section(5) + 4..section(5) + 8]
                .copy_from_slice(&0x6000_1234_u32.to_le_bytes());

            for hostile in [
                nonnull_zero,
                misaligned_offset,
                overflowing_alloc,
                unmapped_alloc,
                overlaps_section_headers,
                compressed,
                corrupt_note,
                bad_symbol_name,
                bad_symbol_partition,
                unknown_section,
            ] {
                assert_incremental_output_rejected(&hostile);
            }
        }

        #[test]
        fn static_output_header_counts_are_rejected_before_table_traversal() {
            let mut too_many_programs = static_output();
            too_many_programs[56..58].copy_from_slice(
                &(crate::MAX_HOST_LINK_ELF_PROGRAM_HEADERS_V1 as u16 + 1).to_le_bytes(),
            );
            let mut too_many_sections = static_output();
            too_many_sections[40..48].copy_from_slice(&120_u64.to_le_bytes());
            too_many_sections[60..62]
                .copy_from_slice(&(crate::MAX_HOST_LINK_ELF_SECTIONS_V1 as u16 + 1).to_le_bytes());
            assert_incremental_output_rejected(&too_many_programs);
            assert_incremental_output_rejected(&too_many_sections);
        }

        #[test]
        fn auditor_out_of_file_load_and_entry_zero_corpus_rejects() {
            let mut bytes =
                output_with_program_header(elf::EM_X86_64, elf::PT_LOAD, elf::PF_R | elf::PF_X);
            bytes[72..80].copy_from_slice(&0x1000_u64.to_le_bytes());
            bytes[96..104].copy_from_slice(&16_u64.to_le_bytes());
            bytes[104..112].copy_from_slice(&16_u64.to_le_bytes());
            assert_eq!(bytes.len(), 120);
            assert_eq!(
                inspect_static_output_elf(&bytes).unwrap_err().code(),
                HostLinkErrorCodeV1::ElfPolicy
            );
        }

        #[test]
        fn out_of_bounds_relocatable_section_rejects_direct_and_archived() {
            let malformed = relocatable_with_out_of_bounds_text();
            assert_eq!(
                inspect_artifact(HostArtifactKindV1::Object, &malformed)
                    .err()
                    .expect("out-of-bounds direct object must fail")
                    .code(),
                HostLinkErrorCodeV1::ArtifactKind
            );
            assert_eq!(
                inspect_artifact(HostArtifactKindV1::RegularArchive, &archive(&malformed))
                    .err()
                    .expect("out-of-bounds archive member must fail")
                    .code(),
                HostLinkErrorCodeV1::ArtifactKind
            );
        }

        #[test]
        fn malformed_symbol_and_relocation_tables_reject() {
            let valid = relocatable_with_symbols_and_relocation();
            inspect_artifact(HostArtifactKindV1::Object, &valid).unwrap();

            let mut bad_symbol_name = valid.clone();
            bad_symbol_name[160..164].copy_from_slice(&5_u32.to_le_bytes());
            let mut bad_symbol_section = valid.clone();
            bad_symbol_section[166..168].copy_from_slice(&6_u16.to_le_bytes());
            let mut bad_relocation_link = valid.clone();
            let rela = 256 + 5 * 64;
            bad_relocation_link[rela + 40..rela + 44].copy_from_slice(&3_u32.to_le_bytes());
            let mut bad_relocation_symbol = valid.clone();
            bad_relocation_symbol[192..200].copy_from_slice(&((2_u64 << 32) | 2).to_le_bytes());
            for hostile in [
                bad_symbol_name,
                bad_symbol_section,
                bad_relocation_link,
                bad_relocation_symbol,
            ] {
                assert_eq!(
                    inspect_artifact(HostArtifactKindV1::Object, &hostile)
                        .err()
                        .expect("malformed direct ET_REL table must fail")
                        .code(),
                    HostLinkErrorCodeV1::ArtifactKind
                );
                assert_eq!(
                    inspect_artifact(HostArtifactKindV1::RegularArchive, &archive(&hostile))
                        .err()
                        .expect("malformed archived ET_REL table must fail")
                        .code(),
                    HostLinkErrorCodeV1::ArtifactKind
                );
            }
        }

        #[test]
        fn auditor_relocatable_section_corpus_rejects_direct_archive_and_rlib() {
            const SECTION_TABLE: usize = 256;
            let section = |index: usize| SECTION_TABLE + index * 64;
            let valid = relocatable_with_symbols_and_relocation();

            let mut nonnull_zero = valid.clone();
            nonnull_zero[4 + SECTION_TABLE..8 + SECTION_TABLE]
                .copy_from_slice(&elf::SHT_PROGBITS.to_le_bytes());
            let mut compressed = valid.clone();
            compressed[section(2) + 8..section(2) + 16]
                .copy_from_slice(&u64::from(elf::SHF_COMPRESSED).to_le_bytes());
            let mut crel = valid.clone();
            crel[section(5) + 4..section(5) + 8].copy_from_slice(&elf::SHT_CREL.to_le_bytes());
            let mut overlapping = valid.clone();
            overlapping[section(2) + 24..section(2) + 32].copy_from_slice(&129_u64.to_le_bytes());
            let mut misaligned = valid.clone();
            misaligned[section(2) + 48..section(2) + 56].copy_from_slice(&256_u64.to_le_bytes());
            let mut bad_relocation_offset = valid.clone();
            bad_relocation_offset[184..192].copy_from_slice(&u64::MAX.to_le_bytes());
            let mut bad_information = valid.clone();
            bad_information[section(5) + 44..section(5) + 48]
                .copy_from_slice(&u32::MAX.to_le_bytes());
            let mut corrupt_note = valid.clone();
            corrupt_note[section(5) + 4..section(5) + 8]
                .copy_from_slice(&elf::SHT_NOTE.to_le_bytes());
            corrupt_note[section(5) + 40..section(5) + 48].fill(0);
            corrupt_note[section(5) + 56..section(5) + 64].fill(0);
            corrupt_note[184..188].copy_from_slice(&u32::MAX.to_le_bytes());
            let mut mismatched_shndx = valid.clone();
            mismatched_shndx.resize(SECTION_TABLE + 7 * 64, 0);
            mismatched_shndx[60..62].copy_from_slice(&7_u16.to_le_bytes());
            let shndx = section(6);
            mismatched_shndx[shndx + 4..shndx + 8]
                .copy_from_slice(&elf::SHT_SYMTAB_SHNDX.to_le_bytes());
            mismatched_shndx[shndx + 24..shndx + 32].copy_from_slice(&208_u64.to_le_bytes());
            mismatched_shndx[shndx + 32..shndx + 40].copy_from_slice(&4_u64.to_le_bytes());
            mismatched_shndx[shndx + 40..shndx + 44].copy_from_slice(&4_u32.to_le_bytes());
            mismatched_shndx[shndx + 48..shndx + 56].copy_from_slice(&4_u64.to_le_bytes());
            mismatched_shndx[shndx + 56..shndx + 64].copy_from_slice(&4_u64.to_le_bytes());

            for (case, hostile) in [
                nonnull_zero,
                compressed,
                crel,
                overlapping,
                misaligned,
                bad_relocation_offset,
                bad_information,
                corrupt_note,
                mismatched_shndx,
            ]
            .into_iter()
            .enumerate()
            {
                for kind in [
                    HostArtifactKindV1::Object,
                    HostArtifactKindV1::RegularArchive,
                    HostArtifactKindV1::Rlib,
                ] {
                    let input = if kind == HostArtifactKindV1::Object {
                        hostile.clone()
                    } else {
                        archive(&hostile)
                    };
                    assert_eq!(
                        inspect_artifact(kind, &input)
                            .err()
                            .unwrap_or_else(|| panic!(
                                "hostile ET_REL case {case} kind {kind:?} passed"
                            ))
                            .code(),
                        HostLinkErrorCodeV1::ArtifactKind
                    );
                }
            }
        }

        #[test]
        fn auditor_gnu_archive_index_corpus_rejects_before_member_parsing() {
            let member = relocatable_with_symbols_and_relocation();
            let mut excessive_count = Vec::from(u32::MAX.to_be_bytes());
            excessive_count.extend_from_slice(b"ignored");

            let mut bad_offset = Vec::from(1_u32.to_be_bytes());
            bad_offset.extend_from_slice(&1_u32.to_be_bytes());
            bad_offset.extend_from_slice(b"symbol\0");

            let symbol_body_size = 4 + 4 + b"symbol".len();
            let member_offset = 8 + 60 + symbol_body_size + (symbol_body_size % 2);
            let mut unterminated = Vec::from(1_u32.to_be_bytes());
            unterminated.extend_from_slice(&(member_offset as u32).to_be_bytes());
            unterminated.extend_from_slice(b"symbol");

            for symbols in [excessive_count, bad_offset, unterminated] {
                let error = inspect_artifact(
                    HostArtifactKindV1::RegularArchive,
                    &archive_with_gnu_symbols(&member, symbols),
                )
                .err()
                .expect("malformed GNU symbol index must fail");
                assert!(matches!(
                    error.code(),
                    HostLinkErrorCodeV1::ArtifactKind | HostLinkErrorCodeV1::FieldTooLarge
                ));
            }
        }

        #[test]
        fn canonical_gnu_archive_symbol_index_is_accepted() {
            let member = relocatable_with_symbols_and_relocation();
            let body_size = 4 + 4 + b"symbol\0".len();
            let member_offset = 8 + 60 + body_size + (body_size % 2);
            let mut symbols = Vec::from(1_u32.to_be_bytes());
            symbols.extend_from_slice(&(member_offset as u32).to_be_bytes());
            symbols.extend_from_slice(b"symbol\0");
            let mut internally_padded = symbols.clone();
            internally_padded.push(0);
            for symbols in [symbols, internally_padded] {
                inspect_artifact(
                    HostArtifactKindV1::RegularArchive,
                    &archive_with_gnu_symbols(&member, symbols),
                )
                .unwrap();
            }
        }

        #[test]
        fn canonical_gnu_archive_padding_and_plus_names_are_accepted() {
            let member = relocatable_with_symbols_and_relocation();
            let mut empty_index = b"!<arch>\n".to_vec();
            append_archive_record(&mut empty_index, b"/", &[0; 8]);
            append_archive_record(&mut empty_index, b"member+one.o/", &member);
            inspect_artifact(HostArtifactKindV1::RegularArchive, &empty_index).unwrap();

            let long_name = b"member+with-a-long-name.o";
            let mut long_names = b"!<arch>\n".to_vec();
            let mut names = long_name.to_vec();
            names.extend_from_slice(b"/\n\n");
            append_archive_record(&mut long_names, b"//", &names);
            append_archive_record(&mut long_names, b"/0", &member);
            inspect_artifact(HostArtifactKindV1::RegularArchive, &long_names).unwrap();
        }

        #[test]
        fn reject_by_default_relocatable_corpus_covers_every_container_kind() {
            const SECTION_TABLE: usize = 256;
            let section = |index: usize| SECTION_TABLE + index * 64;
            let valid = relocatable_with_symbols_and_relocation();

            let mut unknown_flags = valid.clone();
            unknown_flags[section(2) + 8..section(2) + 16]
                .copy_from_slice(&(1_u64 << 63).to_le_bytes());
            let mut self_link = valid.clone();
            self_link[section(2) + 40..section(2) + 44].copy_from_slice(&2_u32.to_le_bytes());
            let mut self_info = valid.clone();
            self_info[section(2) + 44..section(2) + 48].copy_from_slice(&2_u32.to_le_bytes());
            let mut unknown_relocation = valid.clone();
            unknown_relocation[192..200]
                .copy_from_slice(&((1_u64 << 32) | u64::from(u32::MAX)).to_le_bytes());
            let mut relocation_targets_symtab = valid;
            relocation_targets_symtab[section(5) + 44..section(5) + 48]
                .copy_from_slice(&4_u32.to_le_bytes());

            let mut zero_group_signature = relocatable_with_group();
            zero_group_signature[section(6) + 44..section(6) + 48].fill(0);
            let mut self_member_group = relocatable_with_group();
            self_member_group[212..216].copy_from_slice(&6_u32.to_le_bytes());

            for (case, hostile) in [
                unknown_flags,
                self_link,
                self_info,
                unknown_relocation,
                relocation_targets_symtab,
                zero_group_signature,
                self_member_group,
            ]
            .into_iter()
            .enumerate()
            {
                for kind in [
                    HostArtifactKindV1::Object,
                    HostArtifactKindV1::RegularArchive,
                    HostArtifactKindV1::Rlib,
                ] {
                    let input = if kind == HostArtifactKindV1::Object {
                        hostile.clone()
                    } else {
                        archive(&hostile)
                    };
                    assert_eq!(
                        inspect_artifact(kind, &input)
                            .err()
                            .unwrap_or_else(|| panic!(
                                "reject-by-default case {case} kind {kind:?} passed"
                            ))
                            .code(),
                        HostLinkErrorCodeV1::ArtifactKind
                    );
                }
            }
        }

        #[test]
        fn canonical_group_is_preserved_and_cross_validated() {
            let grouped = relocatable_with_group();
            inspect_artifact(HostArtifactKindV1::Object, &grouped).unwrap();
            inspect_artifact(HostArtifactKindV1::RegularArchive, &archive(&grouped)).unwrap();
            inspect_artifact(HostArtifactKindV1::Rlib, &archive(&grouped)).unwrap();
        }

        #[test]
        fn current_rustc_rlib_satisfies_the_closure_subset() {
            let test_binary = std::env::current_exe().unwrap();
            let deps = test_binary
                .parent()
                .expect("test binary has a deps directory");
            let rlib = std::fs::read_dir(deps)
                .unwrap()
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .find(|path| {
                    path.file_name().is_some_and(|name| {
                        let name = name.as_encoded_bytes();
                        name.starts_with(b"libfe2o3_host_link_closure-") && name.ends_with(b".rlib")
                    })
                })
                .expect("cargo test produced the crate rlib");
            let bytes = std::fs::read(rlib).unwrap();
            inspect_artifact(HostArtifactKindV1::Rlib, &bytes).unwrap();
        }

        #[test]
        fn selected_rustc_sysroot_rlib_matrix_satisfies_the_closure_subset() {
            let Some(sysroot) = std::env::var_os("FE2O3_HOST_LINK_COMPAT_SYSROOT") else {
                return;
            };
            let sysroot = std::path::PathBuf::from(sysroot);
            let library_directory = sysroot
                .join("lib")
                .join("rustlib")
                .join("x86_64-unknown-linux-gnu")
                .join("lib");
            let mut rlibs = std::fs::read_dir(&library_directory)
                .unwrap_or_else(|error| {
                    panic!("read sysroot {}: {error}", library_directory.display())
                })
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == "rlib")
                })
                .collect::<Vec<_>>();
            rlibs.sort();
            if let Some(expected) = std::env::var_os("FE2O3_HOST_LINK_EXPECTED_RLIB_COUNT") {
                let expected = expected
                    .to_str()
                    .and_then(|value| value.parse::<usize>().ok())
                    .expect("expected rlib count is canonical decimal");
                assert_eq!(rlibs.len(), expected, "selected sysroot rlib count drifted");
            }
            assert!(!rlibs.is_empty(), "selected sysroot has no rlibs");
            let failures = rlibs
                .iter()
                .filter_map(|path| {
                    let bytes = std::fs::read(path).unwrap_or_else(|error| {
                        panic!("read compatibility rlib {}: {error}", path.display())
                    });
                    inspect_artifact(HostArtifactKindV1::Rlib, &bytes)
                        .err()
                        .map(|error| format!("{}: {error}", path.display()))
                })
                .collect::<Vec<_>>();
            assert!(
                failures.is_empty(),
                "{}/{} selected sysroot rlibs failed:\n{}",
                failures.len(),
                rlibs.len(),
                failures.join("\n")
            );
        }

        #[test]
        fn selected_runtime_archive_matrix_satisfies_the_closure_subset() {
            let Some(paths) = std::env::var_os("FE2O3_HOST_LINK_COMPAT_ARCHIVES") else {
                return;
            };
            let paths = std::env::split_paths(&paths).collect::<Vec<_>>();
            assert!(!paths.is_empty(), "runtime archive matrix is empty");
            let failures = paths
                .iter()
                .filter_map(|path| {
                    let bytes = std::fs::read(path).unwrap_or_else(|error| {
                        panic!("read compatibility archive {}: {error}", path.display())
                    });
                    inspect_artifact(HostArtifactKindV1::RegularArchive, &bytes)
                        .err()
                        .map(|error| format!("{}: {error}", path.display()))
                })
                .collect::<Vec<_>>();
            assert!(
                failures.is_empty(),
                "{}/{} selected runtime archives failed:\n{}",
                failures.len(),
                paths.len(),
                failures.join("\n")
            );
        }

        #[test]
        fn unsupported_bsd_and_unterminated_gnu_long_names_reject() {
            let member = relocatable_with_symbols_and_relocation();
            let mut bsd = b"!<arch>\n".to_vec();
            append_archive_record(&mut bsd, b"#1/8", b"member.o");
            let mut long = b"!<arch>\n".to_vec();
            append_archive_record(&mut long, b"//", b"unterminated");
            append_archive_record(&mut long, b"/0", &member);
            for hostile in [bsd, long] {
                assert_eq!(
                    inspect_artifact(HostArtifactKindV1::RegularArchive, &hostile)
                        .err()
                        .expect("unsupported archive name encoding must fail")
                        .code(),
                    HostLinkErrorCodeV1::ArtifactKind
                );
            }
        }

        #[test]
        fn incremental_copy_has_a_fixed_large_output_byte_quantum() {
            for size in [128_u64 * 1024 * 1024, 512_u64 * 1024 * 1024] {
                let source = sealed_static_output_with_size(size);
                let copy =
                    IncrementalOutputCopyV1::new(source, "large test output", size, size).unwrap();
                let started = Instant::now();
                let progress = copy
                    .advance(
                        crate::HOST_LINK_ADMISSION_MAX_BYTES_PER_POLL_V1,
                        Instant::now() + std::time::Duration::from_secs(2),
                        Instant::now() + std::time::Duration::from_secs(2),
                    )
                    .unwrap();
                assert!(started.elapsed() < std::time::Duration::from_secs(1));
                let IncrementalOutputCopyProgressV1::Pending(copy) = progress else {
                    panic!("one bounded poll copied an entire {size}-byte output");
                };
                assert_eq!(
                    copy.bytes_processed(),
                    crate::HOST_LINK_ADMISSION_MAX_BYTES_PER_POLL_V1
                );
            }
        }

        #[test]
        fn incremental_large_output_copy_checks_deadline_before_work() {
            for size in [128_u64 * 1024 * 1024, 512_u64 * 1024 * 1024] {
                let source = sealed_static_output_with_size(size);
                let copy =
                    IncrementalOutputCopyV1::new(source, "late test output", size, size).unwrap();
                let started = Instant::now();
                let error = copy
                    .advance(
                        crate::HOST_LINK_ADMISSION_MAX_BYTES_PER_POLL_V1,
                        Instant::now(),
                        Instant::now(),
                    )
                    .err()
                    .expect("expired copy must fail terminally");
                assert_eq!(error.code(), HostLinkErrorCodeV1::WorkerTimeout);
                assert!(started.elapsed() < std::time::Duration::from_millis(250));
            }
        }

        #[test]
        fn cooperative_quantum_yields_without_poisoning_copy_or_inspection() {
            let size = 128_u64 * 1024 * 1024;
            let copy = IncrementalOutputCopyV1::new(
                sealed_static_output_with_size(size),
                "cooperative copy test",
                size,
                size,
            )
            .unwrap();
            let IncrementalOutputCopyProgressV1::Pending(copy) = copy
                .advance(
                    crate::HOST_LINK_ADMISSION_MAX_BYTES_PER_POLL_V1,
                    Instant::now() + std::time::Duration::from_secs(1),
                    Instant::now(),
                )
                .unwrap()
            else {
                panic!("expired cooperative quantum completed a large copy");
            };
            assert_eq!(copy.bytes_processed(), 0);

            let inspection =
                IncrementalStaticOutputInspectionV1::new(captured_output(&static_output()))
                    .unwrap();
            assert!(matches!(
                inspection
                    .advance(
                        crate::HOST_LINK_ADMISSION_MAX_OPERATIONS_PER_POLL_V1,
                        Instant::now() + std::time::Duration::from_secs(1),
                        Instant::now(),
                    )
                    .unwrap(),
                IncrementalOutputInspectionProgressV1::Pending(_)
            ));
        }

        #[test]
        fn incremental_receiver_copy_and_descriptor_elf_inspection_complete() {
            let bytes = static_output();
            let source = sealed_static_output_with_size(bytes.len() as u64);
            let mut progress = IncrementalOutputCopyProgressV1::Pending(Box::new(
                IncrementalOutputCopyV1::new(
                    source,
                    "incremental valid output",
                    bytes.len() as u64,
                    bytes.len() as u64,
                )
                .unwrap(),
            ));
            let captured = loop {
                progress = match progress {
                    IncrementalOutputCopyProgressV1::Pending(copy) => copy
                        .advance(
                            32,
                            Instant::now() + std::time::Duration::from_secs(1),
                            Instant::now() + std::time::Duration::from_secs(1),
                        )
                        .unwrap(),
                    IncrementalOutputCopyProgressV1::Complete(captured) => break captured,
                };
            };
            assert_eq!(captured.sha256, sha256_bytes(&bytes));
            let mut inspection = IncrementalOutputInspectionProgressV1::Pending(
                IncrementalStaticOutputInspectionV1::new(captured).unwrap(),
            );
            loop {
                inspection = match inspection {
                    IncrementalOutputInspectionProgressV1::Pending(state) => state
                        .advance(
                            1,
                            Instant::now() + std::time::Duration::from_secs(1),
                            Instant::now() + std::time::Duration::from_secs(1),
                        )
                        .unwrap(),
                    IncrementalOutputInspectionProgressV1::Complete(_, profile) => {
                        assert_eq!(profile.machine, elf::EM_X86_64);
                        assert_eq!(profile.elf_type, elf::ET_EXEC);
                        break;
                    }
                };
            }
        }

        #[test]
        fn incremental_descriptor_elf_inspection_rejects_hostile_layout_corpus() {
            let mut entry_zero = static_output();
            entry_zero[24..32].fill(0);
            let mut out_of_file_segment = static_output();
            out_of_file_segment[72..80].copy_from_slice(&4096_u64.to_le_bytes());
            let mut file_larger_than_memory = static_output();
            file_larger_than_memory[104..112].copy_from_slice(&1_u64.to_le_bytes());
            let mut bad_program_table = static_output();
            bad_program_table[32..40].copy_from_slice(&100_u64.to_le_bytes());
            let mut bad_section_table = static_output();
            bad_section_table[40..48].copy_from_slice(&120_u64.to_le_bytes());
            bad_section_table[60..62].copy_from_slice(&1_u16.to_le_bytes());
            let mut overlapping_loads = static_output();
            overlapping_loads.resize(177, 0);
            overlapping_loads[56..58].copy_from_slice(&2_u16.to_le_bytes());
            overlapping_loads.copy_within(64..120, 120);

            for hostile in [
                entry_zero,
                out_of_file_segment,
                file_larger_than_memory,
                bad_program_table,
                bad_section_table,
                overlapping_loads,
            ] {
                let captured = captured_output(&hostile);
                let mut state = match IncrementalStaticOutputInspectionV1::new(captured) {
                    Ok(state) => state,
                    Err(error) => {
                        assert_eq!(error.code(), HostLinkErrorCodeV1::ElfPolicy);
                        continue;
                    }
                };
                loop {
                    match state.advance(
                        1,
                        Instant::now() + std::time::Duration::from_secs(1),
                        Instant::now() + std::time::Duration::from_secs(1),
                    ) {
                        Ok(IncrementalOutputInspectionProgressV1::Pending(next)) => state = next,
                        Ok(IncrementalOutputInspectionProgressV1::Complete(_, _)) => {
                            panic!("hostile descriptor ELF layout was admitted")
                        }
                        Err(error) => {
                            assert_eq!(error.code(), HostLinkErrorCodeV1::ElfPolicy);
                            break;
                        }
                    }
                }
            }
        }

        #[test]
        fn receiver_copy_is_offset_independent_and_sender_mode_is_not_authority() {
            let descriptor = rustix::fs::memfd_create(
                "fe2o3-sender-owned-output-test",
                MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
            )
            .unwrap();
            let mut sender = File::from(descriptor);
            let bytes = b"receiver-owned immutable output";
            sender.write_all(bytes).unwrap();
            sender
                .set_permissions(std::fs::Permissions::from_mode(0o555))
                .unwrap();
            rustix::fs::fcntl_add_seals(&sender, REQUIRED_SEALS).unwrap();
            sender.seek(SeekFrom::Start(7)).unwrap();
            let received = sender.try_clone().unwrap();
            sender
                .set_permissions(std::fs::Permissions::from_mode(0o444))
                .unwrap();

            let admitted =
                copy_received_sealed_file(received, "sender-owned test output", 4096).unwrap();
            assert_eq!(sender.stream_position().unwrap(), 7);
            assert_eq!(sender.metadata().unwrap().mode() & 0o7777, 0o444);

            sender
                .set_permissions(std::fs::Permissions::from_mode(0o555))
                .unwrap();
            assert_eq!(sender.metadata().unwrap().mode() & 0o7777, 0o555);
            assert_eq!(admitted.file.metadata().unwrap().mode() & 0o7777, 0o555);
            assert_eq!(
                rustix::fs::fcntl_get_seals(&admitted.file).unwrap(),
                REQUIRED_SEALS
            );
            let mut copied = vec![0_u8; bytes.len()];
            assert_eq!(
                rustix::io::pread(&admitted.file, &mut copied, 0).unwrap(),
                copied.len()
            );
            assert_eq!(copied, bytes);
        }

        #[test]
        fn receiver_copy_rejects_linked_and_non_shmem_files() {
            let mut linked = tempfile::NamedTempFile::new().unwrap();
            linked.write_all(b"linked output impostor").unwrap();
            assert_eq!(linked.as_file().metadata().unwrap().nlink(), 1);
            assert_eq!(
                copy_received_sealed_file(
                    linked.reopen().unwrap(),
                    "linked output impostor",
                    4096,
                )
                .err()
                .expect("linked output must be rejected")
                .code(),
                HostLinkErrorCodeV1::ArtifactKind
            );

            let current_directory = std::env::current_dir().unwrap();
            let mut anonymous = tempfile::tempfile_in(current_directory).unwrap();
            anonymous.write_all(b"non-shmem output impostor").unwrap();
            assert_eq!(anonymous.metadata().unwrap().nlink(), 0);
            if rustix::fs::fstatfs(&anonymous).unwrap().f_type as u64 != TMPFS_MAGIC {
                assert_eq!(
                    copy_received_sealed_file(anonymous, "non-shmem output impostor", 4096,)
                        .err()
                        .expect("non-shmem output must be rejected")
                        .code(),
                    HostLinkErrorCodeV1::ArtifactKind
                );
            }
        }

        #[test]
        fn receiver_copy_rejects_linked_tmpfs_files_when_available() {
            let tmpfs = std::path::Path::new("/dev/shm");
            if !tmpfs.is_dir() {
                return;
            }
            let Ok(mut linked) = tempfile::NamedTempFile::new_in(tmpfs) else {
                return;
            };
            linked.write_all(b"linked tmpfs output impostor").unwrap();
            if rustix::fs::fstatfs(linked.as_file()).unwrap().f_type as u64 != TMPFS_MAGIC {
                return;
            }
            assert_eq!(linked.as_file().metadata().unwrap().nlink(), 1);
            if rustix::fs::fcntl_add_seals(linked.as_file(), REQUIRED_SEALS).is_ok() {
                assert_eq!(
                    rustix::fs::fcntl_get_seals(linked.as_file()).unwrap(),
                    REQUIRED_SEALS
                );
            }
            assert_eq!(
                copy_received_sealed_file(
                    linked.reopen().unwrap(),
                    "linked tmpfs output impostor",
                    4096,
                )
                .err()
                .expect("linked tmpfs output must be rejected")
                .code(),
                HostLinkErrorCodeV1::ArtifactKind
            );
        }
    }
}

#[cfg(target_os = "linux")]
pub(crate) use linux::*;

#[cfg(not(target_os = "linux"))]
mod unsupported {
    use super::*;

    fn unsupported<T>() -> Result<T, HostLinkError> {
        Err(HostLinkError::new(
            HostLinkErrorCodeV1::UnsupportedPlatform,
            "HostLinkClosureV1 requires Linux descriptor sealing and openat2",
        ))
    }

    pub(crate) struct CapturedFile {
        pub file: File,
        pub sha256: Sha256Digest,
        pub size: u64,
        pub mode: u32,
        pub bytes: Vec<u8>,
    }

    pub(crate) struct CapturedOutputFileV1 {
        pub file: File,
        pub sha256: Sha256Digest,
        pub size: u64,
        pub mode: u32,
    }

    pub(crate) struct IncrementalOutputCopyV1;
    pub(crate) enum IncrementalOutputCopyProgressV1 {
        Pending(Box<IncrementalOutputCopyV1>),
        Complete(CapturedOutputFileV1),
    }
    pub(crate) struct IncrementalStaticOutputInspectionV1;
    pub(crate) enum IncrementalOutputInspectionProgressV1 {
        Pending(IncrementalStaticOutputInspectionV1),
        Complete(CapturedOutputFileV1, ElfProfileV1),
    }

    impl IncrementalOutputCopyV1 {
        pub(crate) fn new(
            _source: File,
            _name: &str,
            _limit: u64,
            _expected_size: u64,
        ) -> Result<Self, HostLinkError> {
            unsupported()
        }

        pub(crate) fn advance(
            self,
            _maximum_bytes: u64,
            _deadline: std::time::Instant,
        ) -> Result<IncrementalOutputCopyProgressV1, HostLinkError> {
            unsupported()
        }
    }

    impl IncrementalStaticOutputInspectionV1 {
        pub(crate) fn new(_output: CapturedOutputFileV1) -> Result<Self, HostLinkError> {
            unsupported()
        }

        pub(crate) fn advance(
            self,
            _maximum_operations: usize,
            _deadline: std::time::Instant,
        ) -> Result<IncrementalOutputInspectionProgressV1, HostLinkError> {
            unsupported()
        }
    }

    pub(crate) struct ArtifactInspectionV1 {
        pub elf_profile: Option<ElfProfileV1>,
        pub archive_members: u64,
    }

    pub(crate) fn capture_to_sealed_memfd(
        _source: File,
        _name: &str,
        _limit: u64,
    ) -> Result<CapturedFile, HostLinkError> {
        unsupported()
    }
    pub(crate) fn verify_sealed_artifact_identity(
        _file: &File,
        _expected_size: u64,
        _expected_mode: u32,
        _name: &str,
    ) -> Result<(), HostLinkError> {
        unsupported()
    }
    pub(crate) fn read_sealed_file(
        _file: File,
        _name: &str,
        _limit: u64,
    ) -> Result<CapturedFile, HostLinkError> {
        unsupported()
    }
    #[cfg(test)]
    pub(crate) fn copy_received_sealed_file(
        _source: File,
        _name: &str,
        _limit: u64,
    ) -> Result<CapturedFile, HostLinkError> {
        unsupported()
    }
    pub(crate) fn sealed_file_from_bytes(
        _bytes: &[u8],
        _name: &str,
    ) -> Result<File, HostLinkError> {
        unsupported()
    }
    pub(crate) fn verify_sealed_artifact(
        _file: &File,
        _expected_sha256: Sha256Digest,
        _expected_size: u64,
        _expected_mode: u32,
        _name: &str,
    ) -> Result<(), HostLinkError> {
        unsupported()
    }
    pub(crate) fn inspect_artifact(
        _kind: HostArtifactKindV1,
        _bytes: &[u8],
    ) -> Result<ArtifactInspectionV1, HostLinkError> {
        unsupported()
    }
    pub(crate) fn inspect_elf(_bytes: &[u8]) -> Result<ElfProfileV1, HostLinkError> {
        unsupported()
    }
    pub(crate) fn inspect_static_output_elf(_bytes: &[u8]) -> Result<ElfProfileV1, HostLinkError> {
        unsupported()
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) use unsupported::*;
