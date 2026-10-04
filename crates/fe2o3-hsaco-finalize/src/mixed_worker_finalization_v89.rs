//! Structural descriptor V89 continuation of the existing strict Worker V3 transaction.
//! This retains conditional contracts without granting source, proof or launch authority.

use crate::mixed_worker_finalization_family::mixed_worker_finalization_family;

mixed_worker_finalization_family!(
    FinalizedNominalHsacoV89,
    NominalFinalizationErrorV89,
    finalize_unfinalized_nominal_hsaco_v89,
    inspect_unfinalized_nominal_hsaco_v89,
    PreparedFinalizedNominalWorkerHsacoV89,
    r#"Move-only V89 artifact and original strict transaction, with no authority upgrade.
There is no conversion to a V1/V3 owner or authenticated conditional proof.

```compile_fail
use fe2o3_hsaco_finalize::PreparedFinalizedNominalWorkerHsacoV89;
fn duplicate(value: PreparedFinalizedNominalWorkerHsacoV89) { let _ = value.clone(); }
```
```compile_fail
use fe2o3_hsaco_finalize::{PreparedFinalizedNominalWorkerHsacoV53, PreparedFinalizedNominalWorkerHsacoV89};
fn downgrade(value: PreparedFinalizedNominalWorkerHsacoV89) -> PreparedFinalizedNominalWorkerHsacoV53 { value.into() }
```"#,
    NominalWorkerFinalizationErrorV89,
    finalize_protected_worker_nominal_hsaco_v89,
    calculate_nominal_worker_finalized_identity_v89
);
