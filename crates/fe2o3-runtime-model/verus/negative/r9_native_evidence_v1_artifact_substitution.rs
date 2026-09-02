use vstd::prelude::*;

verus! {

pub open spec fn mutated_loaded_artifact_v1(artifact: nat) -> nat {
    artifact + 1
}

pub proof fn mutated_machine_evidence_retains_artifact_v1(artifact: nat)
    requires artifact > 0,
    ensures mutated_loaded_artifact_v1(artifact) == artifact,
{
}

} // verus!
