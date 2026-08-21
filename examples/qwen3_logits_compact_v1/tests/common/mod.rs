#![allow(dead_code)]

use fe2o3_qwen3_logits_compact_v1::*;

pub const PLAN: LogitsPlanIdentityV1 = LogitsPlanIdentityV1([0x51; 32]);

pub fn candidate(role: Qwen3LogitsRoleV1, bucket: B3LogitsBucketV1) -> StructuralLogitsCandidateV1 {
    let profile = LogitsProfileDescriptorV1::canonical(role, bucket);
    admit_logits_candidate_v1(LogitsCandidateDescriptorV1::canonical(profile, PLAN)).unwrap()
}

pub fn binding(
    candidate: StructuralLogitsCandidateV1,
) -> (CompactBatchBindingV1, CompactBatchExpectationV1) {
    let profile = candidate.profile().descriptor();
    let requests: Vec<_> = (0..profile.sequences)
        .map(|index| CompactRequestIdentityV1 {
            slot: u32::try_from(index).unwrap(),
            generation: u32::try_from(index + 11).unwrap(),
        })
        .collect();
    (
        CompactBatchBindingV1 {
            plan_identity: candidate.plan_identity(),
            epoch: 73,
            speculative_k: profile.speculative_k,
            requests: requests.clone(),
        },
        CompactBatchExpectationV1 {
            epoch: 73,
            requests,
        },
    )
}

pub fn sentinel(candidate: StructuralLogitsCandidateV1) -> CompactCompletionRecordV1 {
    let profile = candidate.profile().descriptor();
    CompactCompletionRecordV1 {
        schema_version: 99,
        role: profile.role,
        bucket: profile.bucket,
        request: CompactRequestIdentityV1 {
            slot: 31,
            generation: u32::MAX,
        },
        epoch: u64::MAX,
        plan_identity: candidate.plan_identity(),
        candidate_identity: candidate.candidate_identity(),
        row: u32::MAX,
        local_token: u32::MAX,
        token_id: u32::MAX,
    }
}

pub struct FormulaProvider {
    pub rows: usize,
    pub vocabulary: usize,
    pub first_winner: usize,
    pub second_winner: usize,
    pub nonfinite: Option<(usize, usize)>,
}

impl FormulaProvider {
    pub fn value(&self, row: usize, token_id: usize) -> f32 {
        if self.nonfinite == Some((row, token_id)) {
            return f32::NAN;
        }
        if token_id == self.first_winner || token_id == self.second_winner {
            100.0 + row as f32
        } else {
            -((token_id % 101) as f32) - row as f32
        }
    }
}

impl LogitProviderV1 for FormulaProvider {
    fn rows(&self) -> usize {
        self.rows
    }

    fn vocabulary_size(&self) -> usize {
        self.vocabulary
    }

    fn logit(&self, row: usize, token_id: usize) -> Result<f32, LogitsReferenceErrorV1> {
        if row >= self.rows || token_id >= self.vocabulary {
            return Err(LogitsReferenceErrorV1::CoordinateOutOfRange);
        }
        Ok(self.value(row, token_id))
    }
}

#[derive(Clone, Copy)]
pub struct ProceduralSource {
    pub activation_elements: usize,
    pub weight_elements: usize,
    pub missing_activation: Option<usize>,
    pub missing_weight: Option<usize>,
    pub nonfinite_activation: Option<usize>,
    pub nonfinite_weight: Option<usize>,
    pub maximum_finite: bool,
}

impl ProceduralSource {
    fn finite(bits_index: usize, weight: bool) -> Bf16V1 {
        let value = if weight {
            match bits_index % 4 {
                0 => 0.5,
                1 => -0.25,
                2 => 0.125,
                _ => 0.0,
            }
        } else if bits_index.is_multiple_of(2) {
            1.0
        } else {
            -0.5
        };
        Bf16V1::from_f32_rne(value).unwrap()
    }
}

impl Bf16ProjectionSourceV1 for ProceduralSource {
    fn activation_elements(&self) -> usize {
        self.activation_elements
    }

    fn weight_elements(&self) -> usize {
        self.weight_elements
    }

    fn activation(&self, index: usize) -> Option<Bf16V1> {
        if self.missing_activation == Some(index) {
            None
        } else if self.nonfinite_activation == Some(index) {
            Some(Bf16V1::from_bits(0x7fc1))
        } else if self.maximum_finite {
            Some(Bf16V1::from_bits(0x7f7f))
        } else {
            Some(Self::finite(index, false))
        }
    }

    fn weight(&self, index: usize) -> Option<Bf16V1> {
        if self.missing_weight == Some(index) {
            None
        } else if self.nonfinite_weight == Some(index) {
            Some(Bf16V1::from_bits(0x7f80))
        } else if self.maximum_finite {
            Some(Bf16V1::from_bits(0x7f7f))
        } else {
            Some(Self::finite(index, true))
        }
    }
}

pub fn source(candidate: StructuralLogitsCandidateV1) -> ProceduralSource {
    ProceduralSource {
        activation_elements: usize::try_from(candidate.resources().activation_elements).unwrap(),
        weight_elements: usize::try_from(candidate.resources().weight_elements).unwrap(),
        missing_activation: None,
        missing_weight: None,
        nonfinite_activation: None,
        nonfinite_weight: None,
        maximum_finite: false,
    }
}
