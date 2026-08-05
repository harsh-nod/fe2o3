use core::fmt;

/// MessagePack resource limit exceeded before value-tree decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessagePackLimit {
    Depth,
    Nodes,
    CollectionItems,
    TotalCollectionItems,
    StringBytes,
    BlobBytes,
}

/// Why HSACO inspection failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InspectionError {
    InputTooLarge,
    InvalidElf(&'static str),
    UnsupportedElfClass,
    UnsupportedEndianness,
    UnsupportedMachine,
    UnsupportedOsAbi,
    UnsupportedCodeObjectVersion,
    TooManySections,
    TooManySegments,
    TooManyNotes,
    MetadataNoteTooLarge,
    MissingMetadataNote,
    DuplicateMetadataNote,
    MalformedMessagePack,
    TrailingMessagePack,
    MessagePackLimit(MessagePackLimit),
    NonStringMapKey,
    InvalidUtf8String,
    DuplicateMapKey,
    MissingField(&'static str),
    InvalidFieldType(&'static str),
    InvalidFieldValue(&'static str),
    UnsupportedMetadataVersion,
    MetadataVersionMismatch,
    InvalidTargetPrefix,
    InvalidTargetId,
    NonCanonicalTargetId,
    TargetFlagsMismatch,
    UnknownRootField,
    UnknownKernelField,
    UnsupportedFieldForCodeObjectVersion(&'static str),
    ConflictingFieldAliases(&'static str),
    TooManyKernels,
    DuplicateKernelName,
    DuplicateKernelSymbol,
    TooManyArguments,
    DuplicateArgumentName,
    UnknownArgumentField,
    UnsupportedValueKindForCodeObjectVersion,
    UnknownValueKind,
    UnknownAddressSpace,
    UnknownAccess,
    InvalidArgumentRange,
    ArgumentsOutOfOrder,
    OverlappingArguments,
    ExplicitArgumentAfterHidden,
    ExplicitQualifierOnHiddenArgument,
    InvalidImplicitArgumentSpan,
    InvalidHiddenArgumentLayout,
}

impl fmt::Display for InspectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputTooLarge => formatter.write_str("HSACO exceeds the input size limit"),
            Self::InvalidElf(reason) => write!(formatter, "invalid ELF: {reason}"),
            Self::UnsupportedElfClass => formatter.write_str("HSACO must be ELF64"),
            Self::UnsupportedEndianness => {
                formatter.write_str("HSACO must use little-endian ELF encoding")
            }
            Self::UnsupportedMachine => formatter.write_str("ELF machine must be EM_AMDGPU"),
            Self::UnsupportedOsAbi => formatter.write_str("ELF OS ABI must be AMDGPU HSA"),
            Self::UnsupportedCodeObjectVersion => {
                formatter.write_str("AMDGPU HSA code object version is unsupported")
            }
            Self::TooManySections => formatter.write_str("ELF has too many sections"),
            Self::TooManySegments => formatter.write_str("ELF has too many program segments"),
            Self::TooManyNotes => formatter.write_str("ELF has too many notes"),
            Self::MetadataNoteTooLarge => formatter.write_str("AMDGPU metadata note is too large"),
            Self::MissingMetadataNote => formatter.write_str("AMDGPU metadata note is missing"),
            Self::DuplicateMetadataNote => {
                formatter.write_str("multiple AMDGPU metadata notes were found")
            }
            Self::MalformedMessagePack => formatter.write_str("metadata MessagePack is malformed"),
            Self::TrailingMessagePack => {
                formatter.write_str("metadata has trailing MessagePack bytes")
            }
            Self::MessagePackLimit(limit) => {
                write!(formatter, "MessagePack limit exceeded: {limit}")
            }
            Self::NonStringMapKey => formatter.write_str("metadata map key is not a UTF-8 string"),
            Self::InvalidUtf8String => {
                formatter.write_str("metadata contains an invalid UTF-8 string")
            }
            Self::DuplicateMapKey => formatter.write_str("metadata map contains a duplicate key"),
            Self::MissingField(field) => {
                write!(formatter, "missing required metadata field {field}")
            }
            Self::InvalidFieldType(field) => {
                write!(formatter, "invalid type for metadata field {field}")
            }
            Self::InvalidFieldValue(field) => {
                write!(formatter, "invalid value for metadata field {field}")
            }
            Self::UnsupportedMetadataVersion => {
                formatter.write_str("AMDHSA metadata version is unsupported")
            }
            Self::MetadataVersionMismatch => {
                formatter.write_str("metadata version does not match the code object version")
            }
            Self::InvalidTargetPrefix => {
                formatter.write_str("AMDHSA target has an invalid triple prefix")
            }
            Self::InvalidTargetId => formatter.write_str("AMDHSA target ID is invalid"),
            Self::NonCanonicalTargetId => {
                formatter.write_str("AMDHSA target ID is not canonically spelled")
            }
            Self::TargetFlagsMismatch => {
                formatter.write_str("AMDHSA metadata target does not match ELF flags")
            }
            Self::UnknownRootField => formatter.write_str("metadata root has an unknown field"),
            Self::UnknownKernelField => formatter.write_str("kernel metadata has an unknown field"),
            Self::UnsupportedFieldForCodeObjectVersion(field) => {
                write!(
                    formatter,
                    "metadata field {field} is not supported by this code object version"
                )
            }
            Self::ConflictingFieldAliases(field) => {
                write!(formatter, "metadata field {field} uses conflicting aliases")
            }
            Self::TooManyKernels => formatter.write_str("metadata has too many kernels"),
            Self::DuplicateKernelName => {
                formatter.write_str("metadata has a duplicate kernel name")
            }
            Self::DuplicateKernelSymbol => {
                formatter.write_str("metadata has a duplicate kernel symbol")
            }
            Self::TooManyArguments => formatter.write_str("kernel has too many physical arguments"),
            Self::DuplicateArgumentName => {
                formatter.write_str("kernel has a duplicate argument name")
            }
            Self::UnknownArgumentField => {
                formatter.write_str("kernel argument has an unknown metadata field")
            }
            Self::UnsupportedValueKindForCodeObjectVersion => formatter.write_str(
                "kernel argument value kind is not supported by this code object version",
            ),
            Self::UnknownValueKind => {
                formatter.write_str("kernel argument has an unknown value kind")
            }
            Self::UnknownAddressSpace => {
                formatter.write_str("kernel argument has an unknown address space")
            }
            Self::UnknownAccess => {
                formatter.write_str("kernel argument has an unknown access qualifier")
            }
            Self::InvalidArgumentRange => formatter.write_str("kernel argument range is invalid"),
            Self::ArgumentsOutOfOrder => {
                formatter.write_str("kernel arguments are not in offset order")
            }
            Self::OverlappingArguments => formatter.write_str("kernel argument ranges overlap"),
            Self::ExplicitArgumentAfterHidden => {
                formatter.write_str("explicit argument occurs after a hidden argument")
            }
            Self::ExplicitQualifierOnHiddenArgument => {
                formatter.write_str("hidden argument contains an explicit-only qualifier")
            }
            Self::InvalidImplicitArgumentSpan => formatter
                .write_str("kernel arguments do not bind the declared implicit argument span"),
            Self::InvalidHiddenArgumentLayout => {
                formatter.write_str("hidden arguments do not match the code object ABI layout")
            }
        }
    }
}

impl fmt::Display for MessagePackLimit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Depth => "depth",
            Self::Nodes => "node count",
            Self::CollectionItems => "collection length",
            Self::TotalCollectionItems => "total collection entries",
            Self::StringBytes => "string length",
            Self::BlobBytes => "binary length",
        })
    }
}

impl core::error::Error for InspectionError {}

/// Why explicit metadata-to-ELF kernel binding failed.
///
/// Binding errors never grant module-loading or dispatch authority. They only
/// explain why the descriptive ELF evidence could not be tied to the metadata
/// in the same byte slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelBindingError {
    Inspection(InspectionError),
    InvalidSymbolTable(&'static str),
    TooManySymbols,
    MissingDescriptorSymbol,
    AmbiguousDescriptorSymbol,
    InvalidDescriptorSymbol(&'static str),
    MissingEntrySymbol,
    AmbiguousEntrySymbol,
    InvalidEntrySymbol(&'static str),
    InvalidLoadMapping(&'static str),
    InvalidKernelDescriptor(&'static str),
    MetadataMismatch(&'static str),
}

impl From<InspectionError> for KernelBindingError {
    fn from(error: InspectionError) -> Self {
        Self::Inspection(error)
    }
}

impl fmt::Display for KernelBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inspection(error) => write!(formatter, "HSACO inspection failed: {error}"),
            Self::InvalidSymbolTable(reason) => {
                write!(formatter, "invalid ELF symbol table: {reason}")
            }
            Self::TooManySymbols => formatter.write_str("ELF has too many static symbols"),
            Self::MissingDescriptorSymbol => {
                formatter.write_str("metadata kernel descriptor symbol is missing")
            }
            Self::AmbiguousDescriptorSymbol => {
                formatter.write_str("metadata kernel descriptor symbol is ambiguous")
            }
            Self::InvalidDescriptorSymbol(reason) => {
                write!(formatter, "invalid kernel descriptor symbol: {reason}")
            }
            Self::MissingEntrySymbol => {
                formatter.write_str("metadata kernel entry symbol is missing")
            }
            Self::AmbiguousEntrySymbol => {
                formatter.write_str("metadata kernel entry symbol is ambiguous")
            }
            Self::InvalidEntrySymbol(reason) => {
                write!(formatter, "invalid kernel entry symbol: {reason}")
            }
            Self::InvalidLoadMapping(reason) => {
                write!(formatter, "invalid ELF load mapping: {reason}")
            }
            Self::InvalidKernelDescriptor(reason) => {
                write!(formatter, "invalid AMDHSA kernel descriptor: {reason}")
            }
            Self::MetadataMismatch(field) => {
                write!(
                    formatter,
                    "kernel descriptor disagrees with metadata field {field}"
                )
            }
        }
    }
}

impl core::error::Error for KernelBindingError {}
