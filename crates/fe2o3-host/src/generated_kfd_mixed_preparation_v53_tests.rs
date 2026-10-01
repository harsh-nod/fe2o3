use super::*;
use crate::generated_kfd_arguments::mixed_preparation_v53;
use crate::{CompilerGeneratedKernelProfileV1, MixedWorkerV53PreparationError};
use fe2o3_artifacts::PointerWidth;
use fe2o3_device::KernelMarkerV1;

struct MixedKernel;
fn marker() {}
unsafe impl KernelMarkerV1 for MixedKernel {
    type Function = fn();
    type Registration = ();
    const LOGICAL_NAME: &'static str = "mixed";
    const EXPORT_NAME: &'static str = "mixed";
    const FUNCTION: Self::Function = marker;
    const REGISTRATION: &'static Self::Registration = &();
}
unsafe impl CompilerGeneratedKernelExpectationV1 for MixedKernel {
    const PROFILE: CompilerGeneratedKernelProfileV1 =
        CompilerGeneratedKernelProfileV1::new([0x43; 32]);
    const KERNEL_BINDING_ID_V1: [u8; 32] = [0x42; 32];
}

struct Arguments<'a, const WRONG_LAYOUT: bool> {
    input: &'a [i32],
    output: &'a mut [i32],
    count: u32,
}

// This test adapter uses the same exact generated layout and typed bindings as
// the existing mixed ABI fixture. No receiver or executable authority is forged.
unsafe impl<'a, const WRONG_LAYOUT: bool> CompilerGeneratedKfdArguments<'a, MixedKernel>
    for Arguments<'a, WRONG_LAYOUT>
{
    fn generated_argument_layout()
    -> std::result::Result<CompilerGeneratedArgumentLayoutV1, GeneratedArgumentLayoutError> {
        let plan = plan();
        CompilerGeneratedArgumentLayoutV1::new(
            if WRONG_LAYOUT { 48 } else { 40 },
            8,
            PointerWidth::Bits64,
            (0..3).map(|i| plan.argument(i).unwrap().clone()).collect(),
        )
    }

    fn bind_kfd_arguments(
        self,
        plan: &GeneratedArgumentPackingPlanV1,
    ) -> std::result::Result<GeneratedKfdArgumentBinding<'a>, GeneratedKfdArgumentError> {
        Ok(GeneratedKfdArgumentBinding::from_compiler_generated_parts(
            vec![
                plan.scalar(2, self.count)
                    .map_err(GeneratedKfdArgumentError::Pack)?,
            ],
            vec![
                GeneratedKfdReadSlice::new(self.input).bind_argument(plan, 0)?,
                GeneratedKfdReadWriteSlice::new(self.output).bind_argument(plan, 1)?,
            ],
        ))
    }
}

fn run<const WRONG_LAYOUT: bool>(
    work_limit: usize,
    storage_limit: usize,
    wrong_descriptor: bool,
) -> (
    std::result::Result<(), MixedWorkerV53PreparationError>,
    usize,
    usize,
    usize,
) {
    let descriptor = descriptor();
    let table = decode_device_descriptor_table_v3(&descriptor, &mut free).unwrap();
    let bytes = contract(&table, false, |subjects, _, _| {
        if wrong_descriptor {
            subjects.descriptor_identity[0] ^= 1;
        }
    });
    let contract = decode_mixed_contract_v26(&bytes, &mut free).unwrap();
    let input = [1, 2, 3];
    let mut output = [0; 3];
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    let result = mixed_preparation_v53::prepare::<MixedKernel, _>(
        &table,
        &contract,
        Arguments::<WRONG_LAYOUT> {
            input: &input,
            output: &mut output,
            count: 9,
        },
        geometry(),
        0,
        1000,
        &mut budget,
    );
    let result = result.map(|(runtime, completion)| {
        assert!(matches!(runtime.invocation_binding(),
            fe2o3_runtime::Gfx942RuntimeInvocationBindingV1::ConditionalMixedV26 { contract_identity, .. }
                if contract_identity == *contract.identity()));
        assert_eq!(completion.buffers.len(), 2);
        assert!(budget.storage() > 0, "complete mixed premises remain retained");
        drop((runtime, completion));
    });
    (
        result,
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
    )
}

#[test]
fn mixed_v53_generated_preparation_reaches_real_packer_and_preserves_v26_family() {
    run::<false>(usize::MAX, usize::MAX, false).0.unwrap();
}

#[test]
fn mixed_v53_generated_preparation_refuses_candidate_layout_and_contract_substitution() {
    run::<false>(usize::MAX, usize::MAX, false).0.unwrap();
    assert!(matches!(
        run::<true>(usize::MAX, usize::MAX, false).0,
        Err(MixedWorkerV53PreparationError::Arguments(_))
    ));
    assert!(matches!(
        run::<false>(usize::MAX, usize::MAX, true).0,
        Err(MixedWorkerV53PreparationError::Arguments(_))
    ));
}

#[test]
fn mixed_v53_generated_preparation_has_exact_and_one_short_complete_resources() {
    let measured = run::<false>(usize::MAX, usize::MAX, false);
    measured.0.unwrap();
    let exact = run::<false>(measured.1, measured.3, false);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    for is_work in [true, false] {
        let denied = run::<false>(
            measured.1 - usize::from(is_work),
            measured.3 - usize::from(!is_work),
            false,
        );
        let error = denied
            .0
            .expect_err("one-short complete preparation must refuse");
        let mut source: &(dyn std::error::Error + 'static) = &error;
        loop {
            if let Some(resource) = source.downcast_ref::<Resource>() {
                match (resource, is_work) {
                    (Resource::Work(error), true) => {
                        assert_eq!(error.actual(), measured.1);
                        assert_eq!(error.limit(), measured.1 - 1);
                    }
                    (Resource::Storage(error), false) => {
                        assert_eq!(error.actual(), measured.3);
                        assert_eq!(error.limit(), measured.3 - 1);
                    }
                    _ => panic!("unexpected resource: {resource:?}"),
                }
                break;
            }
            source = source
                .source()
                .unwrap_or_else(|| panic!("missing resource chain: {error}"));
        }
    }
}
