//! Inert conditional V89 artifact integrity, retaining every mandatory contract.
//! This does not join owned source proof, machine evidence or Worker custody.

use crate::mixed_descriptor_finalization_family::mixed_descriptor_finalization_family;

mixed_descriptor_finalization_family!(
    ".fe2o3.kd.v89",
    COMPILER_MIXED_DESCRIPTOR_SECTION_V89,
    CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V89,
    MIXED_DESCRIPTOR_READER_STORAGE_V89,
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V89,
    MixedDescriptorTableV89,
    MixedDescriptorErrorV89,
    decode_mixed_descriptor_v89,
    NominalDescriptorInspectionV89,
    r#"Exact V89 wire, mandatory contracts and independently inspected physical bindings.
Matching public identities do not authenticate source, proof or compiler origin.

```compile_fail
use fe2o3_hsaco_finalize::NominalDescriptorInspectionV89;
use fe2o3_kernel_descriptor::MixedDescriptorTableV53;
fn downgrade<'a>(view: &'a NominalDescriptorInspectionV89<'a>) -> &'a MixedDescriptorTableV53<'a> {
    view.descriptor_table()
}
```"#,
    FinalizedNominalHsacoV89,
    r#"Move-only structural artifact with exact mandatory-conditional V89 bytes.
The byte copy and parsed metadata are a separate bounded allocation domain.
This owner is not Worker finalization, proof, admission or launch authority.

```compile_fail
use fe2o3_hsaco_finalize::FinalizedNominalHsacoV89;
fn clone(value: FinalizedNominalHsacoV89) { let _ = value.clone(); }
```
```compile_fail
use fe2o3_hsaco_finalize::FinalizedNominalHsacoV89;
fn mutate(value: FinalizedNominalHsacoV89) { value.as_bytes()[0] = 1; }
```
```compile_fail
use fe2o3_hsaco_finalize::{FinalizedNominalHsacoV53, FinalizedNominalHsacoV89};
fn downgrade(value: FinalizedNominalHsacoV89) -> FinalizedNominalHsacoV53 { value.into() }
```"#,
    NominalFinalizationErrorV89,
    inspect_unfinalized_nominal_hsaco_v89,
    inspect_finalized_nominal_hsaco_v89,
    finalize_unfinalized_nominal_hsaco_v89,
    derive_unfinalized_nominal_hsaco_v89
);
