# fe2o3-middle-end

This crate owns the frontend-neutral production middle end. It consumes
admitted semantic MIR and produces ranked PLIRON custody after the mandatory
generic checks. Frontends provide optional independently authenticated
reference effects through the narrow `AuthenticatedReferenceEffectsV1`
interface; no rustc-private type crosses this crate boundary.
