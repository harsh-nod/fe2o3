//! Explicit V4 structural preflight using the unchanged strict ModuleV2 child.
use crate::inert_semantic_module_handoff_v4::{
    ExpandedCompilerHandoffErrorV4, HEADER, INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V4,
    INERT_COMPILER_MODULE_PAIR_BINDING_DOMAIN_V4, INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DOMAIN_V4,
    MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V4, bad, ensure, expanded_handoff_length_v4,
    overflow, pay,
};
use crate::{
    COMPILER_DESCRIPTOR_SOURCE_DOMAIN_V3, CodeObjectVersion, CompilerModuleHandoffV2,
    CompilerModuleKindV1, InertFinalCompilerModuleCommitmentV3, MAX_COMPILER_FFI_ENVELOPE_BYTES_V1,
    MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1, MAX_FINAL_COMPILER_MODULE_COMMITMENT_BYTES_V3,
};
use fe2o3_compiler_lineage::{
    EXPANDED_CAPSULE_READ_STORAGE_V4, EXPANDED_OUTPUT_ASSOCIATION_READ_STORAGE_V1,
    EXPANDED_PUBLICATION_HASH_STORAGE_V4, ExpandedCapsuleRefV4, ExpandedContentIdentityV4,
    ExpandedOutputAssociationRefV1, ExpandedOutputAxisV1, ExpandedReceiptSlotV4,
    expanded_content_identity_v4,
};
use std::{mem::size_of, sync::Arc};

/// Separately admitted legacy ModuleV2 metadata domain, not internal telemetry.
/// The historical artifact-transaction V3 shared-range contract reserves three
/// bounded envelope/manifest forms plus 8 MiB for invocation, collection metadata
/// (including the manifest's cloned-name BTreeMap) and fixed owners. Retain that
/// ENTIRE allowance, conservatively even though V4 separately pays invocation.
/// Raw module bytes share the original allocation; no old maximum is increased.
pub const EXPANDED_MODULE_CHILD_STORAGE_V4: usize = 3 * MAX_COMPILER_FFI_ENVELOPE_BYTES_V1
    + 3 * MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1
    + 8 * 1024 * 1024
    + 2 * size_of::<CompilerModuleHandoffV2>();
/// Old compact-commitment decoder's input/output/re-encoding forms and typed owner.
pub const EXPANDED_COMMITMENT_CHILD_STORAGE_V4: usize = 3
    * MAX_FINAL_COMPILER_MODULE_COMMITMENT_BYTES_V3
    + size_of::<InertFinalCompilerModuleCommitmentV3>();
/// Complete additional view/reader/hash domains, retaining every borrowed child
/// until its last use. Input backing and the outer owner are paid separately.
pub const EXPANDED_HANDOFF_READ_STORAGE_V4: usize = 2 * size_of::<
    ExpandedCompilerModuleHandoffRefV4<'static>,
>() + size_of::<Reader<'static>>()
    + INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V4
    + EXPANDED_CAPSULE_READ_STORAGE_V4
    + EXPANDED_MODULE_CHILD_STORAGE_V4
    + EXPANDED_OUTPUT_ASSOCIATION_READ_STORAGE_V1
    + EXPANDED_COMMITMENT_CHILD_STORAGE_V4
    + EXPANDED_PUBLICATION_HASH_STORAGE_V4
    + 4 * size_of::<ExpandedContentIdentityV4>();

/// Immutable outer view. ModuleV2 shares the same canonical Vec; its legacy
/// FFI/manifest metadata stays within the separately prepaid child domain.
/// ```compile_fail
/// use fe2o3_compiler_ffi::{InertSemanticCompilerModuleHandoffV4, ExpandedCompilerModuleHandoffRefV4};
/// fn escape(x: InertSemanticCompilerModuleHandoffV4) -> ExpandedCompilerModuleHandoffRefV4<'static> {
///     x.view(usize::MAX, &mut |_| Ok::<(), ()>(())).unwrap()
/// }
/// ```
pub struct ExpandedCompilerModuleHandoffRefV4<'a> {
    bytes: &'a [u8],
    capsule: ExpandedCapsuleRefV4<'a>,
    module: CompilerModuleHandoffV2,
    identity: ExpandedContentIdentityV4,
    pair: ExpandedContentIdentityV4,
}
impl<'a> ExpandedCompilerModuleHandoffRefV4<'a> {
    /// Complete immutable outer bytes.
    pub const fn canonical_bytes(&self) -> &'a [u8] {
        self.bytes
    }
    /// Explicit V4 capsule, not a relabeled V3 capsule.
    pub const fn capsule(&self) -> &ExpandedCapsuleRefV4<'a> {
        &self.capsule
    }
    /// Unchanged structural LLVM/FFI/manifest transport, not descriptor V1 evidence.
    pub const fn module_handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.module
    }
    /// Full outer content identity, not a signed occurrence.
    pub const fn identity(&self) -> ExpandedContentIdentityV4 {
        self.identity
    }
    /// Complete pair V4 identity, binding both exact inner identities.
    pub const fn pair_identity(&self) -> ExpandedContentIdentityV4 {
        self.pair
    }
}

/// Checks exact V4/nominal-V3/ModuleV2 content joins. Inputs are prepaid.
/// This establishes no source authentication, nominal Rust correspondence,
/// graph-to-native semantics, invocation currentness or publication authority.
pub fn preflight_expanded_compiler_handoff_v4<E>(
    capsule: &ExpandedCapsuleRefV4<'_>,
    module: &CompilerModuleHandoffV2,
    available: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(), ExpandedCompilerHandoffErrorV4<E>> {
    ensure(EXPANDED_HANDOFF_READ_STORAGE_V4, available)?;
    if capsule.target() != module.target()
        || module.code_object_version() != CodeObjectVersion::V6
        || module.kind() != CompilerModuleKindV1::LlvmTextIr
    {
        return Err(bad("expanded module target/kind/COV"));
    }
    let association = ExpandedOutputAssociationRefV1::read(
        capsule
            .receipt(ExpandedReceiptSlotV4::ProofBinding)
            .canonical_preimage(),
        EXPANDED_OUTPUT_ASSOCIATION_READ_STORAGE_V1,
        charge,
    )?;
    pay(1, charge)?;
    let commitment = InertFinalCompilerModuleCommitmentV3::decode(
        capsule
            .receipt(ExpandedReceiptSlotV4::FinalModuleCommitment)
            .canonical_preimage(),
    )
    .map_err(ExpandedCompilerHandoffErrorV4::Commitment)?;
    if !commitment.matches_handoff(module) {
        return Err(bad("final module commitment"));
    }
    let manifest = capsule
        .receipt(ExpandedReceiptSlotV4::ExportManifest)
        .canonical_preimage();
    pay(manifest.len(), charge)?;
    if manifest != module.symbol_manifest().canonical_bytes() {
        return Err(bad("complete final export manifest"));
    }
    let nominal = capsule
        .receipt(ExpandedReceiptSlotV4::NominalAbi)
        .canonical_preimage();
    let descriptor_id = expanded_content_identity_v4(
        COMPILER_DESCRIPTOR_SOURCE_DOMAIN_V3,
        nominal,
        EXPANDED_PUBLICATION_HASH_STORAGE_V4,
        charge,
    )?;
    let native_id = declared(
        *module.module_identity().sha256(),
        module.module_identity().byte_len(),
    )?;
    let symbol_id = declared(
        *module.symbol_manifest().identity().sha256(),
        module.symbol_manifest().identity().byte_len(),
    )?;
    let envelope_id = declared(
        module.envelope().identity().as_bytes(),
        module.envelope().canonical_bytes().len() as u64,
    )?;
    pay(4 * 40, charge)?;
    if association.axis(ExpandedOutputAxisV1::Descriptor) != descriptor_id
        || association.axis(ExpandedOutputAxisV1::NativeModule) != native_id
        || association.axis(ExpandedOutputAxisV1::SymbolManifest) != symbol_id
        || association.axis(ExpandedOutputAxisV1::FfiEnvelope) != envelope_id
    {
        return Err(bad("actual descriptor/native/manifest/envelope identities"));
    }
    Ok(())
}

pub(crate) fn read_shared<'a, E>(
    backing: &'a Arc<Vec<u8>>,
    available: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<ExpandedCompilerModuleHandoffRefV4<'a>, ExpandedCompilerHandoffErrorV4<E>> {
    ensure(EXPANDED_HANDOFF_READ_STORAGE_V4, available)?;
    let bytes = backing.as_slice();
    if bytes.len() > MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V4 {
        return Err(bad("outer aggregate"));
    }
    pay(HEADER, charge)?;
    let mut r = Reader::new(bytes);
    if r.take(8)? != b"F2O3IHV4" || r.u16()? != 4 || r.u16()? != 0 || r.u64()? != bytes.len() as u64
    {
        return Err(bad("outer V4 header"));
    }
    let capsule_len = usize::try_from(r.u64()?).map_err(|_| overflow())?;
    let module_len = usize::try_from(r.u64()?).map_err(|_| overflow())?;
    if r.u32()? as usize != INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V4
        || r.u32()? != 0
        || expanded_handoff_length_v4(capsule_len, module_len) != Some(bytes.len())
    {
        return Err(bad("outer canonical lengths"));
    }
    let capsule = ExpandedCapsuleRefV4::read(
        r.take(capsule_len)?,
        EXPANDED_CAPSULE_READ_STORAGE_V4,
        charge,
    )?;
    let module_start = r.at;
    r.take::<E>(module_len)?;
    // The payload range stays in the original Vec. Metadata is a paid legacy child.
    pay(1, charge)?;
    let module = CompilerModuleHandoffV2::decode_shared_vec_range(
        Arc::clone(backing),
        module_start,
        module_len,
    )
    .map_err(ExpandedCompilerHandoffErrorV4::Module)?;
    let pair_bytes = r.take::<E>(INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V4)?;
    r.finish()?;
    pay(INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V4, charge)?;
    let mut pair = Reader::new(pair_bytes);
    if pair.take(8)? != b"F2O3PBV4"
        || pair.u16()? != 4
        || pair.u16()? != 0
        || pair.u32()? as usize != pair_bytes.len()
    {
        return Err(bad("pair V4 header"));
    }
    let capsule_id = pair.identity()?;
    let module_id = pair.identity()?;
    pair.finish()?;
    if capsule_id != capsule.identity()
        || module_id != declared(*module.identity().sha256(), module.identity().byte_len())?
    {
        return Err(bad("complete pair identities"));
    }
    preflight_expanded_compiler_handoff_v4(
        &capsule,
        &module,
        EXPANDED_HANDOFF_READ_STORAGE_V4,
        charge,
    )?;
    let pair = expanded_content_identity_v4(
        INERT_COMPILER_MODULE_PAIR_BINDING_DOMAIN_V4,
        pair_bytes,
        EXPANDED_PUBLICATION_HASH_STORAGE_V4,
        charge,
    )?;
    let identity = expanded_content_identity_v4(
        INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DOMAIN_V4,
        bytes,
        EXPANDED_PUBLICATION_HASH_STORAGE_V4,
        charge,
    )?;
    Ok(ExpandedCompilerModuleHandoffRefV4 {
        bytes,
        capsule,
        module,
        identity,
        pair,
    })
}
fn declared<E>(
    digest: [u8; 32],
    length: u64,
) -> Result<ExpandedContentIdentityV4, ExpandedCompilerHandoffErrorV4<E>> {
    ExpandedContentIdentityV4::from_declared(digest, length)
        .ok_or_else(|| bad("zero content identity"))
}
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }
    fn take<E>(&mut self, n: usize) -> Result<&'a [u8], ExpandedCompilerHandoffErrorV4<E>> {
        let end = self.at.checked_add(n).ok_or_else(overflow)?;
        let out = self
            .bytes
            .get(self.at..end)
            .ok_or_else(|| bad("truncated outer field"))?;
        self.at = end;
        Ok(out)
    }
    fn array<E, const N: usize>(&mut self) -> Result<[u8; N], ExpandedCompilerHandoffErrorV4<E>> {
        self.take(N)?
            .try_into()
            .map_err(|_| bad("outer fixed field"))
    }
    fn u16<E>(&mut self) -> Result<u16, ExpandedCompilerHandoffErrorV4<E>> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32<E>(&mut self) -> Result<u32, ExpandedCompilerHandoffErrorV4<E>> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64<E>(&mut self) -> Result<u64, ExpandedCompilerHandoffErrorV4<E>> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn identity<E>(
        &mut self,
    ) -> Result<ExpandedContentIdentityV4, ExpandedCompilerHandoffErrorV4<E>> {
        declared(self.array()?, self.u64()?)
    }
    fn finish<E>(self) -> Result<(), ExpandedCompilerHandoffErrorV4<E>> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(bad("outer trailing bytes"))
        }
    }
}
