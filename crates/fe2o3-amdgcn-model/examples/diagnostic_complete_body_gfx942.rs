//! Diagnostic mechanism generator. No source, canonical owner, proof or launch.
use fe2o3_amdgcn_model::{
    Gfx942CompleteBodyBlockV1 as Block, Gfx942CompleteBodyBoundaryV1 as Boundary,
    Gfx942CompleteBodyPlanV1 as Plan, Gfx942CompleteBodyResourcesV1 as Resources,
    Gfx942CompleteBodySymbolV1, render_gfx942_complete_body_llvm_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, Gfx942CompleteBodyLabelV1 as Label,
    Gfx942CompleteBodyTerminatorV1 as End, Gfx942OrderedProgramRegistersV1 as Registers,
    Gfx942ProgramDestinationV1 as Destination, Gfx942ProgramInstructionV1 as Instruction,
    Gfx942ProgramRoleV1 as Role,
};
use std::io::Write;
const OUT: Instruction = Instruction::Move {
    destination: Destination::Output,
    source: Role::Input0,
};
const SCRATCH: Instruction = Instruction::Move {
    destination: Destination::Scratch,
    source: Role::Input1,
};
fn block(label: u8, instructions: &[Instruction], terminator: End) -> Block<'_> {
    Block {
        label: Label(label),
        instructions,
        terminator,
    }
}
fn checked(blocks: &[Block<'_>], registers: Registers) -> Result<Plan, String> {
    Plan::check(
        Boundary::PROFILE,
        registers,
        Resources::required(registers),
        blocks,
        &mut Work::new(512),
    )
    .map_err(|error| error.to_string())
}
fn profile(name: &str) -> Result<Plan, String> {
    let registers = Registers::new(32, 33, [34, 35, 36]).map_err(|error| error.to_string())?;
    match name {
        "one" => checked(
            &[block(255, &[OUT], End::GuardedStoreOutputAndEnd)],
            registers,
        ),
        "output_diamond" => checked(
            &[
                block(
                    240,
                    &[],
                    End::BranchSelectorZero {
                        zero: Label(17),
                        nonzero: Label(2),
                    },
                ),
                block(17, &[OUT], End::Jump(Label(4))),
                block(
                    2,
                    &[Instruction::Move {
                        destination: Destination::Output,
                        source: Role::Input1,
                    }],
                    End::Jump(Label(4)),
                ),
                block(4, &[], End::GuardedStoreOutputAndEnd),
            ],
            registers,
        ),
        "scratch_diamond" => checked(
            &[
                block(
                    0,
                    &[],
                    End::BranchSelectorZero {
                        zero: Label(93),
                        nonzero: Label(7),
                    },
                ),
                block(93, &[SCRATCH], End::Jump(Label(250))),
                block(
                    7,
                    &[Instruction::Move {
                        destination: Destination::Scratch,
                        source: Role::Input0,
                    }],
                    End::Jump(Label(250)),
                ),
                block(
                    250,
                    &[Instruction::Move {
                        destination: Destination::Output,
                        source: Role::Scratch,
                    }],
                    End::GuardedStoreOutputAndEnd,
                ),
            ],
            registers,
        ),
        "two_terminals" => checked(
            &[
                block(
                    41,
                    &[],
                    End::BranchSelectorZero {
                        zero: Label(10),
                        nonzero: Label(3),
                    },
                ),
                block(10, &[OUT], End::GuardedStoreOutputAndEnd),
                block(3, &[OUT], End::GuardedStoreOutputAndEnd),
            ],
            registers,
        ),
        "maximum" => {
            let self_writes = [Instruction::Move {
                destination: Destination::Output,
                source: Role::Output,
            }; 14];
            checked(
                &[
                    block(200, &[OUT], End::Jump(Label(0))),
                    block(0, &[], End::Jump(Label(255))),
                    block(255, &self_writes, End::Jump(Label(18))),
                    block(18, &[], End::Jump(Label(1))),
                    block(1, &[], End::Jump(Label(99))),
                    block(99, &[], End::Jump(Label(44))),
                    block(44, &[], End::Jump(Label(2))),
                    block(2, &[OUT], End::GuardedStoreOutputAndEnd),
                ],
                Registers::new(60, 63, [8, 9, 10]).map_err(|error| error.to_string())?,
            )
        }
        _ => Err(
            "profile must be one, output_diamond, scratch_diamond, two_terminals or maximum".into(),
        ),
    }
}
fn run() -> Result<(), String> {
    let mut args = std::env::args();
    let _program = args.next();
    let name = args.next().ok_or("one closed profile is required")?;
    if args.next().is_some() || name.len() > 32 {
        return Err("one closed profile is required".into());
    }
    let plan = profile(&name)?;
    let emitted = render_gfx942_complete_body_llvm_v1(
        &plan,
        Gfx942CompleteBodySymbolV1::new("complete_body_fixture")
            .map_err(|error| error.to_string())?,
        &mut Work::new(65_536),
    )
    .map_err(|error| error.to_string())?;
    if emitted.llvm_ir().len() > 16 * 1024 {
        return Err("LLVM text cap".into());
    }
    let mut output = std::io::stdout().lock();
    output
        .write_all(emitted.llvm_ir().as_bytes())
        .map_err(|error| error.to_string())?;
    output.flush().map_err(|error| error.to_string())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("complete-body diagnostic mechanism refused: {error}");
        std::process::exit(1);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn five_literal_profiles_preserve_counts() {
        for (name, blocks, steps) in [
            ("one", 1, 1),
            ("output_diamond", 4, 2),
            ("scratch_diamond", 4, 3),
            ("two_terminals", 3, 2),
            ("maximum", 8, 16),
        ] {
            let plan = profile(name).unwrap();
            assert_eq!(plan.block_count(), blocks);
            assert_eq!(plan.instruction_count(), steps);
        }
        assert!(profile("custom").is_err());
    }
    #[test]
    fn undefined_output_has_no_renderable_plan() {
        let registers = Registers::new(32, 33, [34, 35, 36]).unwrap();
        assert!(
            checked(
                &[block(0, &[SCRATCH], End::GuardedStoreOutputAndEnd)],
                registers
            )
            .is_err()
        );
    }
}
