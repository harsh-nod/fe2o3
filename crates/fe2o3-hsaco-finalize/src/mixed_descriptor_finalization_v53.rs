//! Inert conditional V53 artifact integrity, retaining every mandatory contract.
//! This does not join owned source proof, machine evidence or Worker custody.

use crate::mixed_descriptor_finalization_family::mixed_descriptor_finalization_family;

mixed_descriptor_finalization_family!(
    ".fe2o3.kd.v53",
    COMPILER_MIXED_DESCRIPTOR_SECTION_V53,
    CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V53,
    MIXED_DESCRIPTOR_READER_STORAGE_V53,
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V53,
    MixedDescriptorTableV53,
    MixedDescriptorErrorV53,
    decode_mixed_descriptor_v53,
    NominalDescriptorInspectionV53,
    r#"Exact V53 wire, mandatory contracts and independently inspected physical bindings.
Matching public identities do not authenticate source, proof or compiler origin.

```compile_fail
use fe2o3_hsaco_finalize::NominalDescriptorInspectionV53;
use fe2o3_kernel_descriptor::DeviceDescriptorTableV4;
fn downgrade<'a>(view: &'a NominalDescriptorInspectionV53<'a>) -> &'a DeviceDescriptorTableV4<'a> {
    view.descriptor_table()
}
```"#,
    FinalizedNominalHsacoV53,
    r#"Move-only structural artifact with exact mandatory-conditional V53 bytes.
The byte copy and parsed metadata are a separate bounded allocation domain.
This owner is not Worker finalization, proof, admission or launch authority.

```compile_fail
use fe2o3_hsaco_finalize::FinalizedNominalHsacoV53;
fn clone(value: FinalizedNominalHsacoV53) { let _ = value.clone(); }
```
```compile_fail
use fe2o3_hsaco_finalize::FinalizedNominalHsacoV53;
fn mutate(value: FinalizedNominalHsacoV53) { value.as_bytes()[0] = 1; }
```
```compile_fail
use fe2o3_hsaco_finalize::{FinalizedNominalHsacoV4, FinalizedNominalHsacoV53};
fn downgrade(value: FinalizedNominalHsacoV53) -> FinalizedNominalHsacoV4 { value.into() }
```"#,
    NominalFinalizationErrorV53,
    inspect_unfinalized_nominal_hsaco_v53,
    inspect_finalized_nominal_hsaco_v53,
    finalize_unfinalized_nominal_hsaco_v53,
    derive_unfinalized_nominal_hsaco_v53
);
