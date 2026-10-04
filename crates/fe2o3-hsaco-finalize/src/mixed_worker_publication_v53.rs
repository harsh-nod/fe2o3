//! Exact mixed-contract facade over durable Worker replay and publication.
//! Artifact publication preserves conditional premises; it is not proof or launch admission.

use crate::mixed_worker_publication_family::mixed_worker_publication_family;

mixed_worker_publication_family!(
    PreparedFinalizedNominalWorkerHsacoV53,
    PreparedMixedWorkerPublicationV53,
    RecoveredMixedWorkerPublicationV53,
    PublishedMixedWorkerHsacoV53,
    prepare_mixed_worker_compact_finalizer_replay_v53,
    prepare_mixed_worker_publication_v53,
    persist_prepared_mixed_worker_publication_v53,
    recover_mixed_worker_publication_v53,
    publish_recovered_mixed_worker_hsaco_v53,
    into_mixed_v53,
    MixedV53,
    r#"```compile_fail
use fe2o3_hsaco_finalize::RecoveredMixedWorkerPublicationV53;
fn duplicate(value: RecoveredMixedWorkerPublicationV53) { let _ = value.clone(); }
```"#
);
