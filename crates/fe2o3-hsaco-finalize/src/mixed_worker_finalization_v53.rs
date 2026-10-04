//! Structural descriptor V53 continuation of the existing strict Worker V3 transaction.
//! This retains conditional contracts without granting source, proof or launch authority.

use crate::mixed_worker_finalization_family::mixed_worker_finalization_family;

mixed_worker_finalization_family!(
    FinalizedNominalHsacoV53,
    NominalFinalizationErrorV53,
    finalize_unfinalized_nominal_hsaco_v53,
    inspect_unfinalized_nominal_hsaco_v53,
    PreparedFinalizedNominalWorkerHsacoV53,
    r#"Move-only V53 artifact and original strict transaction, with no authority upgrade.
There is no conversion to a V1/V3 owner or authenticated conditional proof.

```compile_fail
use fe2o3_hsaco_finalize::PreparedFinalizedNominalWorkerHsacoV53;
fn duplicate(value: PreparedFinalizedNominalWorkerHsacoV53) { let _ = value.clone(); }
```
```compile_fail
use fe2o3_hsaco_finalize::{PreparedFinalizedNominalWorkerHsacoV4, PreparedFinalizedNominalWorkerHsacoV53};
fn downgrade(value: PreparedFinalizedNominalWorkerHsacoV53) -> PreparedFinalizedNominalWorkerHsacoV4 { value.into() }
```"#,
    NominalWorkerFinalizationErrorV53,
    finalize_protected_worker_nominal_hsaco_v53,
    calculate_nominal_worker_finalized_identity_v53
);
