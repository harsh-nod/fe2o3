use vstd::prelude::*;

verus! {

pub struct SemanticProgramV1 {
    pub body: Seq<int>,
    pub abi: Seq<int>,
    pub effects: Seq<int>,
}

pub open spec fn mutated_binding_v1(program: SemanticProgramV1) -> SemanticProgramV1 {
    SemanticProgramV1 {
        body: program.body,
        abi: program.abi,
        effects: program.effects.push(1),
    }
}

pub proof fn mutated_target_binding_does_not_preserve_effects_v1(program: SemanticProgramV1)
    ensures
        mutated_binding_v1(program).effects =~= program.effects,
{
}

} // verus!
