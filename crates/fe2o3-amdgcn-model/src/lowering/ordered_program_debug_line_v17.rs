//! Opt-in whole ordered-region line attribution, not source authentication.
//! No variable, macro-frame, fine-instruction or allocator metadata is invented.

use super::*;
use fe2o3_kernel_ir::{
    AssemblySourceIdentity, DebugSourceMapFileV1, DebugSourceMapKirSiteV1, DebugSourceMapSiteV1,
    DebugSourceMapSpanV1, VerifiedCanonicalKernelIrIdentityV17, VerifiedCanonicalKernelIrModuleV17,
};

/// Narrow opt-in output ceiling; the legacy general emitter cap is unchanged.
pub const MAX_ORDERED_PROGRAM_DEBUG_LINE_LLVM_BYTES_V17: usize = 4 * 1024 * 1024;

/// Typed refusals before LLVM emission. Caller-provided identities are joins,
/// not authentication: only a live compiler source projection supplies custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderedProgramDebugLineErrorV17 {
    ResourceLimit,
    CanonicalIdentityMismatch,
    SourceIdentityMismatch,
    SiteMismatch,
    MissingSite,
    AmbiguousSite,
    AmbiguousSpan,
    MissingFile,
    AmbiguousFile,
    SpanOutsideFile,
    UnsupportedColumn,
    InvalidSpan,
    InvalidFile,
}

impl fmt::Display for OrderedProgramDebugLineErrorV17 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ordered-program debug line refused: {self:?}")
    }
}
impl std::error::Error for OrderedProgramDebugLineErrorV17 {}

/// A borrowed diagnostic selection joined to this exact immutable V17 owner.
/// The source map must come from the same compiler session for a source-backed
/// claim. Constructing this value from arbitrary public map values confers none.
/// No allocation, source-file I/O, owner cloning or executable admission occurs.
#[derive(Clone, Copy)]
pub struct OrderedProgramDebugLineV17<'a> {
    owner: &'a VerifiedCanonicalKernelIrModuleV17,
    file: &'a DebugSourceMapFileV1,
    span: DebugSourceMapSpanV1,
    operation: &'a Operation,
}

impl<'a> OrderedProgramDebugLineV17<'a> {
    pub fn try_new(
        owner: &'a VerifiedCanonicalKernelIrModuleV17,
        expected_kir: &VerifiedCanonicalKernelIrIdentityV17,
        expected_source: AssemblySourceIdentity,
        selected: DebugSourceMapKirSiteV1,
        sites: &[DebugSourceMapSiteV1],
        files: &'a [DebugSourceMapFileV1],
    ) -> Result<Self, OrderedProgramDebugLineErrorV17> {
        use OrderedProgramDebugLineErrorV17 as E;
        if sites.len() > 4096
            || files.len() > 16
            || sites
                .iter()
                .try_fold(0usize, |n, s| n.checked_add(s.spans().len()))
                .is_none_or(|n| n > 8192)
        {
            return Err(E::ResourceLimit);
        }
        if owner.identity() != expected_kir {
            return Err(E::CanonicalIdentityMismatch);
        }
        let index = |n: u64| usize::try_from(n).map_err(|_| E::SiteMismatch);
        let function = owner
            .module()
            .functions
            .get(index(selected.function_ordinal())?)
            .ok_or(E::SiteMismatch)?;
        let operation = function
            .body
            .as_ref()
            .and_then(|b| b.blocks.get(index(selected.block_ordinal()).ok()?))
            .and_then(|b| b.operations.get(index(selected.operation_ordinal()).ok()?))
            .ok_or(E::SiteMismatch)?;
        let OperationKind::Gfx942OrderedProgram(region) = &operation.kind else {
            return Err(E::SiteMismatch);
        };
        if !expected_source.is_complete() || region.source() != expected_source {
            return Err(E::SourceIdentityMismatch);
        }
        let mut selected_sites = sites.iter().filter(|s| s.site() == selected);
        let site = selected_sites.next().ok_or(E::MissingSite)?;
        if selected_sites.next().is_some() {
            return Err(E::AmbiguousSite);
        }
        let [span] = site.spans() else {
            return Err(E::AmbiguousSpan);
        };
        // Public map values can also arrive through serde, which does not call
        // their validating constructors. Recheck the selected primitive fields.
        if span.file_identity() == [0; 32] || span.line() == 0 || span.column() == 0 {
            return Err(E::InvalidSpan);
        }
        let mut selected_files = files
            .iter()
            .filter(|f| f.identity() == span.file_identity());
        let file = selected_files.next().ok_or(E::MissingFile)?;
        if selected_files.next().is_some() {
            return Err(E::AmbiguousFile);
        }
        if file.identity() == [0; 32]
            || file.byte_len() == 0
            || file.display_path().is_empty()
            || file.display_path().len() > 4096
            || file.display_path().contains('\0')
        {
            return Err(E::InvalidFile);
        }
        if span.byte_start() >= span.byte_end() || span.byte_end() > file.byte_len() {
            return Err(E::SpanOutsideFile);
        }
        // LLVM DILocation stores the source column in sixteen bits.
        if span.column() > u16::MAX.into() {
            return Err(E::UnsupportedColumn);
        }
        Ok(Self {
            owner,
            file,
            span: *span,
            operation,
        })
    }

    pub fn span(&self) -> DebugSourceMapSpanV1 {
        self.span
    }

    pub(super) fn matches_owner(&self, owner: &VerifiedCanonicalKernelIrModuleV17) -> bool {
        std::ptr::eq(self.owner, owner)
    }

    pub(super) fn matches_operation(&self, operation: &Operation) -> bool {
        std::ptr::eq(self.operation, operation)
    }

    pub(super) fn emit_metadata(&self, output: &mut dyn fmt::Write, symbol: &str) {
        // This path admits exactly one kernel, no anchor/BF16 context; !0 is
        // the unchanged workgroup node. All other metadata IDs are source-owned.
        writeln!(output, "!llvm.dbg.cu = !{{!1}}").unwrap();
        writeln!(output, "!llvm.module.flags = !{{!7, !8}}").unwrap();
        writeln!(output, "!1 = distinct !DICompileUnit(language: DW_LANG_Rust, file: !2, producer: \"fe2o3 ordered-region line diagnostic v17\", isOptimized: false, runtimeVersion: 0, emissionKind: LineTablesOnly)").unwrap();
        write!(output, "!2 = !DIFile(filename: ").unwrap();
        emit_metadata_string(output, self.file.display_path());
        writeln!(output, ", directory: \"\")").unwrap();
        writeln!(output, "!3 = !DISubroutineType(types: !4)").unwrap();
        writeln!(output, "!4 = !{{}}").unwrap();
        write!(output, "!5 = distinct !DISubprogram(name: ").unwrap();
        emit_metadata_string(output, symbol);
        write!(output, ", linkageName: ").unwrap();
        emit_metadata_string(output, symbol);
        // Function declaration position is unknown. Do not relabel the region's
        // call-site line as the function declaration; only !6 has that location.
        writeln!(output, ", scope: !2, file: !2, line: 0, type: !3, scopeLine: 0, spFlags: DISPFlagDefinition, unit: !1)").unwrap();
        writeln!(
            output,
            "!6 = !DILocation(line: {}, column: {}, scope: !5)",
            self.span.line(),
            self.span.column()
        )
        .unwrap();
        writeln!(output, "!7 = !{{i32 2, !\"Debug Info Version\", i32 3}}").unwrap();
        writeln!(output, "!8 = !{{i32 2, !\"Dwarf Version\", i32 4}}").unwrap();
    }
}

// Stream directly into the existing bounded LLVM writer. The display path is
// at most 4096 input bytes; each byte expands to at most three, with no scratch
// String. This semantic file identity is NOT a content checksum.
fn emit_metadata_string(output: &mut dyn fmt::Write, text: &str) {
    write!(output, "\"").unwrap();
    for byte in text.bytes() {
        match byte {
            0x20..=0x21 | 0x23..=0x5b | 0x5d..=0x7e => write!(output, "{}", char::from(byte)),
            _ => write!(output, "\\{byte:02X}"),
        }
        .unwrap();
    }
    write!(output, "\"").unwrap();
}

/// Explicit opt-in. None delegates to the original entry with byte-identical
/// output. Some emits one location for the indivisible ordered asm call only.
/// Other instructions remain unlocated. This is not a production compiler flag,
/// portable source-map transport, proof, final artifact or execution authority.
pub fn lower_canonical_v17_compiler_module_with_debug_line_to_gfx942_xnack_minus_llvm_ir(
    owner: &VerifiedCanonicalKernelIrModuleV17,
    line: Option<&OrderedProgramDebugLineV17<'_>>,
) -> Result<String, LoweringErrors> {
    let Some(line) = line else {
        return super::lower_canonical_v17_compiler_module_to_gfx942_xnack_minus_llvm_ir(owner);
    };
    if !line.matches_owner(owner) {
        return Err(LoweringErrors::one(
            LoweringLocation::module(owner.module()),
            LoweringDiagnosticCode::SemanticAnchorIdentityMismatch,
            "ordered-program debug line requires its exact retained V17 owner",
        ));
    }
    lower_compiler_module_with_debug_line_context(
        owner.module(),
        LoweringTarget::Gfx942XnackMinusV1,
        None,
        None,
        true,
        Some(OrderedModuleOwner::ProgramV17(owner)),
        None,
        Some(*line),
    )
}
