//! Exact mixed-contract facade over durable Worker replay and publication.
//! Artifact publication preserves conditional premises; it is not proof or launch admission.

use crate::mixed_worker_publication_family::mixed_worker_publication_family;

mixed_worker_publication_family!(
    PreparedFinalizedNominalWorkerHsacoV89,
    PreparedMixedWorkerPublicationV89,
    RecoveredMixedWorkerPublicationV89,
    PublishedMixedWorkerHsacoV89,
    prepare_mixed_worker_compact_finalizer_replay_v89,
    prepare_mixed_worker_publication_v89,
    persist_prepared_mixed_worker_publication_v89,
    recover_mixed_worker_publication_v89,
    publish_recovered_mixed_worker_hsaco_v89,
    into_mixed_v89,
    MixedV89,
    r#"```compile_fail
use fe2o3_hsaco_finalize::RecoveredMixedWorkerPublicationV89;
fn duplicate(value: RecoveredMixedWorkerPublicationV89) { let _ = value.clone(); }
```"#
);
