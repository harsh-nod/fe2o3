use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub struct TargetRequirementsV1 {
    pub gfx942: bool,
    pub gfx950: bool,
    pub wave64: bool,
    pub remaining_contract: nat,
}

#[derive(PartialEq, Eq)]
pub struct TargetCapabilitiesV1 {
    pub gfx942: bool,
    pub gfx950: bool,
    pub wave64: bool,
    pub remaining_contract: nat,
}

pub struct SemanticProgramV1 {
    pub body: Seq<int>,
    pub abi: Seq<int>,
    pub effects: Seq<int>,
    pub requirements: TargetRequirementsV1,
}

pub open spec fn bind_gfx942_wave64_v1(program: SemanticProgramV1) -> SemanticProgramV1 {
    SemanticProgramV1 {
        body: program.body,
        abi: program.abi,
        effects: program.effects,
        requirements: TargetRequirementsV1 {
            gfx942: true,
            gfx950: program.requirements.gfx950,
            wave64: true,
            remaining_contract: program.requirements.remaining_contract,
        },
    }
}

pub open spec fn bind_gfx950_wave64_v1(program: SemanticProgramV1) -> SemanticProgramV1 {
    SemanticProgramV1 {
        body: program.body,
        abi: program.abi,
        effects: program.effects,
        requirements: TargetRequirementsV1 {
            gfx942: program.requirements.gfx942,
            gfx950: true,
            wave64: true,
            remaining_contract: program.requirements.remaining_contract,
        },
    }
}

pub open spec fn target_admits_v1(
    target: TargetCapabilitiesV1,
    requirements: TargetRequirementsV1,
) -> bool {
    &&& (!requirements.gfx942 || target.gfx942)
    &&& (!requirements.gfx950 || target.gfx950)
    &&& (!requirements.wave64 || target.wave64)
    &&& requirements.remaining_contract == target.remaining_contract
}

pub open spec fn observable_trace_v1(
    target: TargetCapabilitiesV1,
    program: SemanticProgramV1,
) -> Option<Seq<int>> {
    if target_admits_v1(target, program.requirements) {
        Some(program.body)
    } else {
        None
    }
}

pub proof fn gfx942_binding_preserves_semantic_program_v1(program: SemanticProgramV1)
    ensures
        bind_gfx942_wave64_v1(program).body =~= program.body,
        bind_gfx942_wave64_v1(program).abi =~= program.abi,
        bind_gfx942_wave64_v1(program).effects =~= program.effects,
        bind_gfx942_wave64_v1(program).requirements.remaining_contract
            == program.requirements.remaining_contract,
{
}

pub proof fn gfx950_binding_preserves_semantic_program_v1(program: SemanticProgramV1)
    ensures
        bind_gfx950_wave64_v1(program).body =~= program.body,
        bind_gfx950_wave64_v1(program).abi =~= program.abi,
        bind_gfx950_wave64_v1(program).effects =~= program.effects,
        bind_gfx950_wave64_v1(program).requirements.remaining_contract
            == program.requirements.remaining_contract,
{
}

pub proof fn gfx942_binding_preserves_admitted_trace_v1(
    target: TargetCapabilitiesV1,
    program: SemanticProgramV1,
)
    requires
        target_admits_v1(target, bind_gfx942_wave64_v1(program).requirements),
    ensures
        target_admits_v1(target, program.requirements),
        observable_trace_v1(target, bind_gfx942_wave64_v1(program))
            == observable_trace_v1(target, program),
{
}

pub proof fn gfx950_binding_preserves_admitted_trace_v1(
    target: TargetCapabilitiesV1,
    program: SemanticProgramV1,
)
    requires
        target_admits_v1(target, bind_gfx950_wave64_v1(program).requirements),
    ensures
        target_admits_v1(target, program.requirements),
        observable_trace_v1(target, bind_gfx950_wave64_v1(program))
            == observable_trace_v1(target, program),
{
}

} // verus!
