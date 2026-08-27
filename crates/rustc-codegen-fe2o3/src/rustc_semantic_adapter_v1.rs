//! Neutral rustc-derived identity primitives for canonical semantic MIR.

use fe2o3_kernel_ir::DebugSourceMapFileV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiIdentityV1, SemanticBlockIdentityV1, SemanticConstGenericArgumentsIdentityV1,
    SemanticFunctionIdentityV1, SemanticGenericTypeArgumentsIdentityV1,
    SemanticItemDefinitionIdentityV1, SemanticLayoutIdentityV1, SemanticLocalIdentityV1,
    SemanticMonomorphizationIdentityV1, SemanticSourceFileIdentityV1, SemanticSourceOriginV1,
    SemanticSourceProvenanceV1, SemanticTargetDataLayoutV1, SemanticTypeIdentityV1,
};
use rustc_data_structures::fingerprint::Fingerprint;
use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_middle::ty::layout::TyAndLayout;
use rustc_middle::ty::{FnSig, GenericArgKind, Instance, Ty, TyCtxt};
use rustc_span::Span;
use rustc_target::callconv::FnAbi;
use sha2::{Digest as _, Sha256};

use crate::semantic_layout_bridge::{
    MAX_SEMANTIC_LAYOUT_TARGET_TEXT_BYTES_V1, SemanticLayoutTargetV1,
};

const TARGET_LAYOUT_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/rustc-target-layout/v1";
const TARGET_LAYOUT_TRANSCRIPT_FIELDS_V1: usize = 6;
/// Maximum exact target-layout digest preimage admitted by the semantic layout bridge.
pub(crate) const MAX_CANONICAL_TARGET_LAYOUT_TRANSCRIPT_BYTES_V1: usize =
    TARGET_LAYOUT_TRANSCRIPT_FIELDS_V1 * size_of::<u64>()
        + TARGET_LAYOUT_DOMAIN_V1.len()
        + 4 * MAX_SEMANTIC_LAYOUT_TARGET_TEXT_BYTES_V1
        + size_of::<u16>();
const FUNCTION_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/function/v1";
const ITEM_DEFINITION_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/item-definition/v1";
const MONOMORPHIZATION_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/monomorphization/v1";
const TYPE_ARGUMENTS_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/type-arguments/v1";
const CONST_ARGUMENTS_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/const-arguments/v1";
const MIR_BODY_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/rustc-mir-body/v1";
const TYPE_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/rustc-type/v1";
const LOCAL_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/rustc-local/v1";
const BLOCK_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/rustc-basic-block/v1";
const SOURCE_FILE_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/rustc-source-file/v1";
const EXPANSION_CHAIN_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/rustc-expansion-chain/v1";
const TYPE_LAYOUT_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/rustc-type-layout/v1";
const SEMANTIC_LAYOUT_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/semantic-layout/v1";
const SEMANTIC_FN_ABI_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/rustc-fn-abi/v1";
const SEMANTIC_FN_ABI_LAYOUT_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/rustc-fn-abi-layout/v1";
const RUSTC_FN_ABI_PREFLIGHT_DOMAIN_V1: &[u8] = b"fe2o3/semantic-mir/rustc-fn-abi-preflight/v1";
const RUSTC_FN_SIGNATURE_PREFLIGHT_DOMAIN_V1: &[u8] =
    b"fe2o3/semantic-mir/rustc-fn-signature-preflight/v1";

macro_rules! stable_fingerprint {
    ($tcx:expr, $value:expr) => {{
        let fingerprint: Fingerprint = $tcx.with_stable_hashing_context(|mut context| {
            let mut hasher = StableHasher::new();
            ($value).hash_stable(&mut context, &mut hasher);
            hasher.finish()
        });
        fingerprint.to_le_bytes()
    }};
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct CanonicalFunctionIdentitiesV1 {
    function: SemanticFunctionIdentityV1,
    item_definition: SemanticItemDefinitionIdentityV1,
    monomorphization: SemanticMonomorphizationIdentityV1,
    generic_type_arguments: SemanticGenericTypeArgumentsIdentityV1,
    const_generic_arguments: SemanticConstGenericArgumentsIdentityV1,
}

pub(crate) struct SemanticIdentityDigestV1 {
    digest: Sha256,
    canonical_transcript: Option<Vec<u8>>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CanonicalSourceProvenanceV1 {
    provenance: SemanticSourceProvenanceV1,
    expansion_chain_sha256: [u8; 32],
    expansion_depth: usize,
}

impl CanonicalSourceProvenanceV1 {
    pub(crate) const fn provenance(self) -> SemanticSourceProvenanceV1 {
        self.provenance
    }

    pub(crate) const fn expansion_chain_sha256(self) -> [u8; 32] {
        self.expansion_chain_sha256
    }

    pub(crate) const fn expansion_depth(self) -> usize {
        self.expansion_depth
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CanonicalSourceErrorV1 {
    DummySpan,
    CrossFileSpan,
    InvalidPosition,
    CoordinateOverflow,
    InvalidDebugSourceFile,
    ExpansionDepthExceeded { actual: usize, maximum: usize },
}

impl std::fmt::Display for CanonicalSourceErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DummySpan => formatter.write_str("dummy source span"),
            Self::CrossFileSpan => formatter.write_str("source span crosses stable source files"),
            Self::InvalidPosition => formatter.write_str("source span has invalid coordinates"),
            Self::CoordinateOverflow => {
                formatter.write_str("source coordinate exceeds canonical width")
            }
            Self::InvalidDebugSourceFile => {
                formatter.write_str("source file exceeds debug-map identity or path bounds")
            }
            Self::ExpansionDepthExceeded { actual, maximum } => write!(
                formatter,
                "macro expansion depth {actual} exceeds reviewed maximum {maximum}",
            ),
        }
    }
}

impl SemanticIdentityDigestV1 {
    pub(crate) fn new(domain: &[u8]) -> Self {
        let mut digest = Sha256::new();
        append_field(&mut digest, domain);
        Self {
            digest,
            canonical_transcript: None,
        }
    }

    pub(crate) fn new_with_canonical_transcript(domain: &[u8]) -> Self {
        let mut digest = Sha256::new();
        let mut canonical_transcript = Vec::with_capacity(8 + domain.len());
        append_field(&mut digest, domain);
        append_transcript_field(&mut canonical_transcript, domain);
        Self {
            digest,
            canonical_transcript: Some(canonical_transcript),
        }
    }

    pub(crate) fn field(&mut self, field: &[u8]) {
        append_field(&mut self.digest, field);
        if let Some(transcript) = &mut self.canonical_transcript {
            append_transcript_field(transcript, field);
        }
    }

    pub(crate) fn finish(self) -> [u8; 32] {
        self.digest.finalize().into()
    }

    pub(crate) fn finish_with_canonical_transcript(self) -> ([u8; 32], Box<[u8]>) {
        let transcript = self
            .canonical_transcript
            .expect("canonical transcript capture was explicitly requested");
        (self.digest.finalize().into(), transcript.into_boxed_slice())
    }
}

impl CanonicalFunctionIdentitiesV1 {
    pub(crate) const fn function(self) -> SemanticFunctionIdentityV1 {
        self.function
    }

    pub(crate) const fn item_definition(self) -> SemanticItemDefinitionIdentityV1 {
        self.item_definition
    }

    pub(crate) const fn monomorphization(self) -> SemanticMonomorphizationIdentityV1 {
        self.monomorphization
    }

    pub(crate) const fn generic_type_arguments(self) -> SemanticGenericTypeArgumentsIdentityV1 {
        self.generic_type_arguments
    }

    pub(crate) const fn const_generic_arguments(self) -> SemanticConstGenericArgumentsIdentityV1 {
        self.const_generic_arguments
    }
}

/// Derives every canonical function identity axis from one live rustc instance.
///
/// Diagnostic symbols and paths are deliberately absent from these preimages.
pub(crate) fn canonical_function_identities_v1(
    tcx: TyCtxt<'_>,
    instance: Instance<'_>,
) -> CanonicalFunctionIdentitiesV1 {
    let instance_fingerprint = stable_fingerprint!(tcx, instance);
    let definition_fingerprint = tcx.def_path_hash(instance.def_id()).0.to_le_bytes();
    let mut type_arguments = SemanticIdentityDigestV1::new(TYPE_ARGUMENTS_DOMAIN_V1);
    let mut const_arguments = SemanticIdentityDigestV1::new(CONST_ARGUMENTS_DOMAIN_V1);
    for argument in instance.args {
        match argument.kind() {
            GenericArgKind::Type(ty) => {
                type_arguments.field(&stable_fingerprint!(tcx, ty));
            }
            GenericArgKind::Const(value) => {
                const_arguments.field(&stable_fingerprint!(tcx, value));
            }
            GenericArgKind::Lifetime(_) => {}
        }
    }
    CanonicalFunctionIdentitiesV1 {
        function: SemanticFunctionIdentityV1::from_sha256(domain_digest(
            FUNCTION_DOMAIN_V1,
            &[&instance_fingerprint],
        )),
        item_definition: SemanticItemDefinitionIdentityV1::from_sha256(domain_digest(
            ITEM_DEFINITION_DOMAIN_V1,
            &[&definition_fingerprint],
        )),
        monomorphization: SemanticMonomorphizationIdentityV1::from_sha256(domain_digest(
            MONOMORPHIZATION_DOMAIN_V1,
            &[&instance_fingerprint],
        )),
        generic_type_arguments: SemanticGenericTypeArgumentsIdentityV1::from_sha256(
            type_arguments.finish(),
        ),
        const_generic_arguments: SemanticConstGenericArgumentsIdentityV1::from_sha256(
            const_arguments.finish(),
        ),
    }
}

/// Binds the exact monomorphized MIR observed in the authenticated session.
/// This is a preflight identity, not a canonical semantic-MIR identity.
pub(crate) fn rustc_mir_body_sha256_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> [u8; 32] {
    let body = tcx.instance_mir(instance.def);
    domain_digest(
        MIR_BODY_DOMAIN_V1,
        &[
            &stable_fingerprint!(tcx, instance),
            &stable_fingerprint!(tcx, body),
        ],
    )
}

/// Identifies one normalized rustc type encountered during raw-MIR preflight.
/// Full semantic type/layout construction remains a later importer pass.
pub(crate) fn rustc_type_identity_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> SemanticTypeIdentityV1 {
    SemanticTypeIdentityV1::from_sha256(domain_digest(
        TYPE_DOMAIN_V1,
        &[&stable_fingerprint!(tcx, ty)],
    ))
}

/// Binds one target-resolved rustc layout producer before schema conversion.
/// This is a preflight identity, not a canonical semantic layout identity.
pub(crate) fn rustc_type_layout_sha256_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    layout: TyAndLayout<'tcx>,
) -> [u8; 32] {
    domain_digest(
        TYPE_LAYOUT_DOMAIN_V1,
        &[
            &stable_fingerprint!(tcx, layout.ty),
            &stable_fingerprint!(tcx, layout.layout),
        ],
    )
}

/// Identifies target-resolved layout facts independently of the source type.
///
/// Equal rustc layouts in the same authenticated target session receive the
/// same semantic layout identity. This is still a record identity, not
/// compiler or artifact authority.
pub(crate) fn rustc_semantic_layout_identity_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    target: SemanticTargetDataLayoutV1,
    layout: TyAndLayout<'tcx>,
) -> SemanticLayoutIdentityV1 {
    SemanticLayoutIdentityV1::from_sha256(domain_digest(
        SEMANTIC_LAYOUT_DOMAIN_V1,
        &[
            target.identity().as_bytes(),
            &stable_fingerprint!(tcx, layout.layout),
        ],
    ))
}

/// Identifies one complete, role-adjusted rustc `FnAbi` observation.
pub(crate) fn rustc_semantic_fn_abi_identity_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    function: SemanticFunctionIdentityV1,
    abi: &FnAbi<'tcx, Ty<'tcx>>,
) -> SemanticAbiIdentityV1 {
    SemanticAbiIdentityV1::from_sha256(domain_digest(
        SEMANTIC_FN_ABI_DOMAIN_V1,
        &[function.as_bytes(), &stable_fingerprint!(tcx, abi)],
    ))
}

/// Identifies the target-resolved physical ABI independently of function identity.
pub(crate) fn rustc_semantic_fn_abi_layout_identity_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    target: SemanticTargetDataLayoutV1,
    abi: &FnAbi<'tcx, Ty<'tcx>>,
) -> SemanticLayoutIdentityV1 {
    SemanticLayoutIdentityV1::from_sha256(domain_digest(
        SEMANTIC_FN_ABI_LAYOUT_DOMAIN_V1,
        &[target.identity().as_bytes(), &stable_fingerprint!(tcx, abi)],
    ))
}

/// Preflight commitment used to prove that later construction consumed the
/// exact rustc ABI retained by the canonical producer plan.
pub(crate) fn rustc_fn_abi_sha256_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    abi: &FnAbi<'tcx, Ty<'tcx>>,
) -> [u8; 32] {
    domain_digest(
        RUSTC_FN_ABI_PREFLIGHT_DOMAIN_V1,
        &[&stable_fingerprint!(tcx, abi)],
    )
}

pub(crate) fn rustc_fn_signature_sha256_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    signature: FnSig<'tcx>,
) -> [u8; 32] {
    domain_digest(
        RUSTC_FN_SIGNATURE_PREFLIGHT_DOMAIN_V1,
        &[&stable_fingerprint!(tcx, signature)],
    )
}

pub(crate) fn rustc_local_identity_v1(
    function: SemanticFunctionIdentityV1,
    mir_body_sha256: [u8; 32],
    rustc_local: u32,
) -> SemanticLocalIdentityV1 {
    SemanticLocalIdentityV1::from_sha256(domain_digest(
        LOCAL_DOMAIN_V1,
        &[
            function.as_bytes(),
            &mir_body_sha256,
            &rustc_local.to_le_bytes(),
        ],
    ))
}

pub(crate) fn rustc_block_identity_v1(
    function: SemanticFunctionIdentityV1,
    mir_body_sha256: [u8; 32],
    rustc_block: u32,
) -> SemanticBlockIdentityV1 {
    SemanticBlockIdentityV1::from_sha256(domain_digest(
        BLOCK_DOMAIN_V1,
        &[
            function.as_bytes(),
            &mir_body_sha256,
            &rustc_block.to_le_bytes(),
        ],
    ))
}

/// Captures the exact rustc span and its resolved source call site while
/// binding the complete, bounded macro expansion chain into a stable digest.
pub(crate) fn canonical_source_provenance_v1(
    tcx: TyCtxt<'_>,
    span: Span,
    maximum_expansion_depth: usize,
) -> Result<CanonicalSourceProvenanceV1, CanonicalSourceErrorV1> {
    let expansion = canonical_source_origin_v1(tcx, span)?;
    let call_site = canonical_source_origin_v1(tcx, span.source_callsite())?;
    canonical_source_provenance_from_origins_v1(
        tcx,
        span,
        maximum_expansion_depth,
        expansion,
        call_site,
    )
}

/// Captures semantic provenance and exact live-rustc file metadata together.
///
/// Display paths are remapped diagnostic labels. Source bytes remain under
/// rustc `SourceMap` custody and are used only to derive the exact byte length.
pub(crate) fn canonical_source_provenance_and_debug_files_v1(
    tcx: TyCtxt<'_>,
    span: Span,
    maximum_expansion_depth: usize,
) -> Result<(CanonicalSourceProvenanceV1, Option<DebugSourceMapFileV1>), CanonicalSourceErrorV1> {
    let provenance = canonical_source_provenance_v1(tcx, span, maximum_expansion_depth)?;
    let (_, debug_file) = canonical_source_origin_and_debug_file_v1(tcx, span.source_callsite())?;
    Ok((provenance, Some(debug_file)))
}

fn canonical_source_provenance_from_origins_v1(
    tcx: TyCtxt<'_>,
    span: Span,
    maximum_expansion_depth: usize,
    expansion: SemanticSourceOriginV1,
    call_site: SemanticSourceOriginV1,
) -> Result<CanonicalSourceProvenanceV1, CanonicalSourceErrorV1> {
    let mut expansion_chain = SemanticIdentityDigestV1::new(EXPANSION_CHAIN_DOMAIN_V1);
    expansion_chain.field(&stable_fingerprint!(tcx, span));
    expansion_chain.field(&stable_fingerprint!(tcx, span.ctxt()));
    let mut cursor = span;
    let mut expansion_depth = 0_usize;
    while let Some(parent) = cursor.parent_callsite() {
        expansion_depth = expansion_depth.checked_add(1).ok_or(
            CanonicalSourceErrorV1::ExpansionDepthExceeded {
                actual: usize::MAX,
                maximum: maximum_expansion_depth,
            },
        )?;
        if expansion_depth > maximum_expansion_depth {
            return Err(CanonicalSourceErrorV1::ExpansionDepthExceeded {
                actual: expansion_depth,
                maximum: maximum_expansion_depth,
            });
        }
        let data = cursor.ctxt().outer_expn_data();
        expansion_chain.field(&stable_fingerprint!(tcx, data));
        expansion_chain.field(&stable_fingerprint!(tcx, data.call_site));
        expansion_chain.field(&stable_fingerprint!(tcx, data.def_site));
        cursor = parent;
    }

    Ok(CanonicalSourceProvenanceV1 {
        provenance: SemanticSourceProvenanceV1::new(Some(expansion), Some(call_site)),
        expansion_chain_sha256: expansion_chain.finish(),
        expansion_depth,
    })
}

fn canonical_source_origin_v1(
    tcx: TyCtxt<'_>,
    span: Span,
) -> Result<SemanticSourceOriginV1, CanonicalSourceErrorV1> {
    if span.is_dummy() {
        return Err(CanonicalSourceErrorV1::DummySpan);
    }
    let source_map = tcx.sess.source_map();
    let start = source_map.lookup_char_pos(span.lo());
    let end = source_map.lookup_char_pos(span.hi());
    if start.file.stable_id != end.file.stable_id {
        return Err(CanonicalSourceErrorV1::CrossFileSpan);
    }
    if start.line == 0 || end.line == 0 || (start.line, start.col.0) > (end.line, end.col.0) {
        return Err(CanonicalSourceErrorV1::InvalidPosition);
    }
    let byte_start = span
        .lo()
        .0
        .checked_sub(start.file.start_pos.0)
        .ok_or(CanonicalSourceErrorV1::InvalidPosition)?;
    let byte_end = span
        .hi()
        .0
        .checked_sub(start.file.start_pos.0)
        .ok_or(CanonicalSourceErrorV1::InvalidPosition)?;
    let line_start =
        u32::try_from(start.line).map_err(|_| CanonicalSourceErrorV1::CoordinateOverflow)?;
    let line_end =
        u32::try_from(end.line).map_err(|_| CanonicalSourceErrorV1::CoordinateOverflow)?;
    let column_start = u32::try_from(
        start
            .col
            .0
            .checked_add(1)
            .ok_or(CanonicalSourceErrorV1::CoordinateOverflow)?,
    )
    .map_err(|_| CanonicalSourceErrorV1::CoordinateOverflow)?;
    let column_end = u32::try_from(
        end.col
            .0
            .checked_add(1)
            .ok_or(CanonicalSourceErrorV1::CoordinateOverflow)?,
    )
    .map_err(|_| CanonicalSourceErrorV1::CoordinateOverflow)?;
    let file = SemanticSourceFileIdentityV1::from_sha256(domain_digest(
        SOURCE_FILE_DOMAIN_V1,
        &[&stable_fingerprint!(tcx, start.file.stable_id)],
    ));
    SemanticSourceOriginV1::new(
        file,
        u64::from(byte_start),
        u64::from(byte_end),
        line_start,
        column_start,
        line_end,
        column_end,
    )
    .map_err(|_| CanonicalSourceErrorV1::InvalidPosition)
}

fn canonical_source_origin_and_debug_file_v1(
    tcx: TyCtxt<'_>,
    span: Span,
) -> Result<(SemanticSourceOriginV1, DebugSourceMapFileV1), CanonicalSourceErrorV1> {
    if span.is_dummy() {
        return Err(CanonicalSourceErrorV1::DummySpan);
    }
    let source_map = tcx.sess.source_map();
    let start = source_map.lookup_char_pos(span.lo());
    let end = source_map.lookup_char_pos(span.hi());
    if start.file.stable_id != end.file.stable_id {
        return Err(CanonicalSourceErrorV1::CrossFileSpan);
    }
    if start.line == 0 || end.line == 0 || (start.line, start.col.0) > (end.line, end.col.0) {
        return Err(CanonicalSourceErrorV1::InvalidPosition);
    }
    let byte_start = span
        .lo()
        .0
        .checked_sub(start.file.start_pos.0)
        .ok_or(CanonicalSourceErrorV1::InvalidPosition)?;
    let byte_end = span
        .hi()
        .0
        .checked_sub(start.file.start_pos.0)
        .ok_or(CanonicalSourceErrorV1::InvalidPosition)?;
    let line_start =
        u32::try_from(start.line).map_err(|_| CanonicalSourceErrorV1::CoordinateOverflow)?;
    let line_end =
        u32::try_from(end.line).map_err(|_| CanonicalSourceErrorV1::CoordinateOverflow)?;
    let column_start = start
        .col
        .0
        .checked_add(1)
        .ok_or(CanonicalSourceErrorV1::CoordinateOverflow)?;
    let column_end = end
        .col
        .0
        .checked_add(1)
        .ok_or(CanonicalSourceErrorV1::CoordinateOverflow)?;
    let column_start =
        u32::try_from(column_start).map_err(|_| CanonicalSourceErrorV1::CoordinateOverflow)?;
    let column_end =
        u32::try_from(column_end).map_err(|_| CanonicalSourceErrorV1::CoordinateOverflow)?;
    let file = SemanticSourceFileIdentityV1::from_sha256(domain_digest(
        SOURCE_FILE_DOMAIN_V1,
        &[&stable_fingerprint!(tcx, start.file.stable_id)],
    ));
    let origin = SemanticSourceOriginV1::new(
        file,
        u64::from(byte_start),
        u64::from(byte_end),
        line_start,
        column_start,
        line_end,
        column_end,
    )
    .map_err(|_| CanonicalSourceErrorV1::InvalidPosition)?;
    let byte_len = u64::from(
        start
            .file
            .end_position()
            .0
            .checked_sub(start.file.start_pos.0)
            .ok_or(CanonicalSourceErrorV1::InvalidPosition)?,
    );
    let display_path = start
        .file
        .name
        .prefer_remapped_unconditionally()
        .to_string_lossy()
        .into_owned();
    let debug_file = DebugSourceMapFileV1::new(*file.as_bytes(), byte_len, display_path)
        .map_err(|_| CanonicalSourceErrorV1::InvalidDebugSourceFile)?;
    Ok((origin, debug_file))
}

/// Returns the exact bounded, domain-framed preimage of the target-layout identity.
///
/// Fields are encoded in identity order as an eight-byte little-endian length
/// followed by exact bytes: domain, LLVM target, data layout, pointer width,
/// active CPU, and normalized active features.
pub(crate) fn canonical_target_layout_transcript_v1(target: &SemanticLayoutTargetV1) -> Box<[u8]> {
    let pointer_width = target.default_pointer_width_bits().to_le_bytes();
    let cpu = target.active_cpu().unwrap_or_default();
    let features = target.active_features().unwrap_or_default();
    let fields = [
        TARGET_LAYOUT_DOMAIN_V1,
        target.llvm_target().as_bytes(),
        target.data_layout().as_bytes(),
        &pointer_width,
        cpu.as_bytes(),
        features.as_bytes(),
    ];
    let exact_length = fields
        .iter()
        .map(|field| size_of::<u64>() + field.len())
        .sum();
    debug_assert!(exact_length <= MAX_CANONICAL_TARGET_LAYOUT_TRANSCRIPT_BYTES_V1);

    let mut transcript = Vec::with_capacity(exact_length);
    for field in fields {
        append_transcript_field(&mut transcript, field);
    }
    debug_assert_eq!(transcript.len(), exact_length);
    transcript.into_boxed_slice()
}

/// Derives the canonical target-layout identity from exact, already observed
/// rustc target facts. Authentication remains the importer's responsibility.
pub(crate) fn canonical_target_layout_v1(
    target: &SemanticLayoutTargetV1,
) -> SemanticTargetDataLayoutV1 {
    let transcript = canonical_target_layout_transcript_v1(target);
    SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(
        Sha256::digest(&transcript).into(),
    ))
}

pub(crate) fn domain_digest(domain: &[u8], fields: &[&[u8]]) -> [u8; 32] {
    let mut digest = SemanticIdentityDigestV1::new(domain);
    for field in fields {
        digest.field(field);
    }
    digest.finish()
}

fn append_field(digest: &mut Sha256, field: &[u8]) {
    digest.update((field.len() as u64).to_le_bytes());
    digest.update(field);
}

fn append_transcript_field(transcript: &mut Vec<u8>, field: &[u8]) {
    transcript.extend_from_slice(&(field.len() as u64).to_le_bytes());
    transcript.extend_from_slice(field);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_identity_transcript_is_the_exact_digest_preimage() {
        let mut captured = SemanticIdentityDigestV1::new_with_canonical_transcript(b"domain");
        captured.field(b"first");
        captured.field(b"");
        captured.field(b"third");
        let (identity, transcript) = captured.finish_with_canonical_transcript();

        assert_eq!(identity, <[u8; 32]>::from(Sha256::digest(&transcript)));

        let mut digest_only = SemanticIdentityDigestV1::new(b"domain");
        digest_only.field(b"first");
        digest_only.field(b"");
        digest_only.field(b"third");
        assert_eq!(digest_only.finish(), identity);
    }

    fn target(features: &str) -> SemanticLayoutTargetV1 {
        target_with_profile("amdgcn-amd-amdhsa", "e-p:64:64", 64, "gfx942", features)
    }

    fn target_with_profile(
        llvm_target: &str,
        data_layout: &str,
        pointer_width: u16,
        cpu: &str,
        features: &str,
    ) -> SemanticLayoutTargetV1 {
        SemanticLayoutTargetV1::new_with_codegen_profile(
            llvm_target,
            data_layout,
            pointer_width,
            cpu,
            "",
            features,
        )
        .unwrap()
    }

    fn transcript_identity(transcript: &[u8]) -> [u8; 32] {
        Sha256::digest(transcript).into()
    }

    fn framed_field_ranges(transcript: &[u8]) -> Vec<(std::ops::Range<usize>, usize)> {
        let mut fields = Vec::new();
        let mut cursor = 0;
        while cursor < transcript.len() {
            let length_offset = cursor;
            let length_end = cursor + size_of::<u64>();
            let field_length = u64::from_le_bytes(
                transcript[cursor..length_end]
                    .try_into()
                    .expect("framed field length is eight bytes"),
            ) as usize;
            cursor = length_end;
            let field_end = cursor + field_length;
            fields.push((cursor..field_end, length_offset));
            cursor = field_end;
        }
        assert_eq!(cursor, transcript.len());
        fields
    }

    #[test]
    fn framed_identity_preimages_reject_concatenation_and_domain_substitution() {
        assert_ne!(
            domain_digest(b"domain-a", &[b"ab", b"c"]),
            domain_digest(b"domain-a", &[b"a", b"bc"])
        );
        assert_ne!(
            domain_digest(b"domain-a", &[b"same"]),
            domain_digest(b"domain-b", &[b"same"])
        );
    }

    #[test]
    fn target_layout_identity_binds_every_codegen_profile_axis() {
        let exact = canonical_target_layout_v1(&target("-xnack,+wavefrontsize64"));
        let substituted = canonical_target_layout_v1(&target("+xnack,+wavefrontsize64"));
        assert_ne!(exact.identity(), substituted.identity());

        let reordered = canonical_target_layout_v1(&target("+wavefrontsize64,-xnack"));
        assert_eq!(exact.identity(), reordered.identity());
    }

    #[test]
    fn target_layout_transcript_is_exact_bounded_deterministic_identity_preimage() {
        let target = target("-xnack,+wavefrontsize64");
        let first = canonical_target_layout_transcript_v1(&target);
        let second = canonical_target_layout_transcript_v1(&target);

        assert_eq!(first, second);
        assert!(first.len() <= MAX_CANONICAL_TARGET_LAYOUT_TRANSCRIPT_BYTES_V1);
        assert_eq!(
            canonical_target_layout_v1(&target).identity().as_bytes(),
            &transcript_identity(&first),
        );
    }

    #[test]
    fn target_layout_transcript_rejects_every_component_and_framing_substitution() {
        let target = target("-xnack,+wavefrontsize64");
        let transcript = canonical_target_layout_transcript_v1(&target);
        let identity = transcript_identity(&transcript);
        let fields = framed_field_ranges(&transcript);
        assert_eq!(fields.len(), TARGET_LAYOUT_TRANSCRIPT_FIELDS_V1);

        for (field_index, (field_range, _)) in fields.iter().enumerate() {
            assert!(!field_range.is_empty());
            let mut substituted = transcript.to_vec();
            substituted[field_range.start] ^= 1;
            assert_ne!(
                transcript_identity(&substituted),
                identity,
                "field {field_index} substitution must change the identity",
            );
        }

        for (field_index, (_, length_offset)) in fields.iter().enumerate() {
            let mut substituted = transcript.to_vec();
            substituted[*length_offset] ^= 1;
            assert_ne!(
                transcript_identity(&substituted),
                identity,
                "field {field_index} framing substitution must change the identity",
            );
        }
    }

    #[test]
    fn target_layout_transcript_binds_each_target_component() {
        let exact = target_with_profile(
            "amdgcn-amd-amdhsa",
            "e-p:64:64",
            64,
            "gfx942",
            "-xnack,+wavefrontsize64",
        );
        let substitutions = [
            target_with_profile(
                "amdgcn-unknown-amdhsa",
                "e-p:64:64",
                64,
                "gfx942",
                "-xnack,+wavefrontsize64",
            ),
            target_with_profile(
                "amdgcn-amd-amdhsa",
                "e-p:64:64-i64:32",
                64,
                "gfx942",
                "-xnack,+wavefrontsize64",
            ),
            target_with_profile(
                "amdgcn-amd-amdhsa",
                "e-p:64:64",
                32,
                "gfx942",
                "-xnack,+wavefrontsize64",
            ),
            target_with_profile(
                "amdgcn-amd-amdhsa",
                "e-p:64:64",
                64,
                "gfx90a",
                "-xnack,+wavefrontsize64",
            ),
            target_with_profile(
                "amdgcn-amd-amdhsa",
                "e-p:64:64",
                64,
                "gfx942",
                "+xnack,+wavefrontsize64",
            ),
        ];
        let exact_transcript = canonical_target_layout_transcript_v1(&exact);

        for (component_index, substituted) in substitutions.iter().enumerate() {
            assert_ne!(
                canonical_target_layout_transcript_v1(substituted),
                exact_transcript,
                "target component {component_index} substitution must change the transcript",
            );
        }
    }

    #[test]
    fn local_and_block_domains_bind_function_and_rustc_index() {
        let function_a = SemanticFunctionIdentityV1::from_sha256([1; 32]);
        let function_b = SemanticFunctionIdentityV1::from_sha256([2; 32]);
        assert_ne!(
            rustc_local_identity_v1(function_a, [3; 32], 0),
            rustc_local_identity_v1(function_a, [3; 32], 1),
        );
        assert_ne!(
            rustc_local_identity_v1(function_a, [3; 32], 0),
            rustc_local_identity_v1(function_b, [3; 32], 0),
        );
        assert_ne!(
            rustc_local_identity_v1(function_a, [3; 32], 0),
            rustc_local_identity_v1(function_a, [4; 32], 0),
        );
        assert_ne!(
            rustc_local_identity_v1(function_a, [3; 32], 0).as_bytes(),
            rustc_block_identity_v1(function_a, [3; 32], 0).as_bytes(),
        );
    }
}
