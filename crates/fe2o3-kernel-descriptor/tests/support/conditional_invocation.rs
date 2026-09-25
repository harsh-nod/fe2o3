//! Caller-authored transport fixtures, never source or proof authority.
use fe2o3_kernel_descriptor::*;
use sha2::{Digest, Sha256};

pub fn free(_: usize) -> Result<(), ()> {
    Ok(())
}

pub struct Fixture {
    pub subjects: ConditionalSubjectsV1,
    pub theorem: ConditionalTheoremV1,
    pub roots: Vec<[u64; 4]>,
    pub arguments: Vec<ConditionalArgumentBindingV1>,
    pub output: ConditionalOutputV1,
    pub reads: Vec<ConditionalReadOccurrenceV1>,
    pub premises: Vec<ConditionalRuntimePremiseV1>,
}
impl Fixture {
    pub fn new(inputs: usize, reads: usize) -> Self {
        assert!(inputs <= reads && (inputs > 0 || reads == 0));
        let subjects = ConditionalSubjectsV1 {
            kernel_id: [13; 32],
            exact_graph_identity: [2; 32],
            aggregate_statement_identity: [3; 32],
            source_semantic_identity: [4; 32],
            reference_kind: ConditionalReferenceKindV1::Mir,
            safe_reference_identity: [5; 32],
            safe_reference_source_hash: [0; 32],
            safe_reference_mir_hash: [6; 32],
            kernel_subject_identity: [7; 32],
            kernel_mir_hash: [8; 32],
        };
        let theorem = ConditionalTheoremV1 {
            statement_identity: [0; 32],
            generated_source_identity: [9; 32],
            execution_identity: [10; 32],
            receipt_identity: [11; 32],
            staging_receipt_identity: [12; 32],
            staging_obligation_identity: [14; 32],
            staging_signer_identity: [15; 32],
            staging_execution_identity: [16; 32],
        };
        let arguments = (0..=inputs)
            .map(|i| ConditionalArgumentBindingV1 {
                canonical_parameter: 10 + 3 * i as u32,
                source_argument: i as u32,
                adjusted_argument: 100 + i as u32,
                semantic_local: 1000 + i as u32,
                semantic_type: 7,
                generated_field: i as u16,
                role: if i == inputs {
                    ConditionalArgumentRoleV1::Output
                } else {
                    ConditionalArgumentRoleV1::Input
                },
                source_type_identity: [21; 32],
                device_layout_identity: [22; 32],
            })
            .collect();
        let output = ConditionalOutputV1 {
            argument: inputs as u16,
            canonical_store: ConditionalCanonicalLocationV1 {
                block: 42,
                operation: 10000,
            },
            ranked_store: ConditionalRankedLocationV1 {
                block: 0,
                operation: 10000,
            },
            ranked_effect: ConditionalRankedLocationV1 {
                block: 0,
                operation: 10001,
            },
            element_bytes: 4,
            alignment: 4,
            address_domain: ConditionalAddressDomainV1::GuardedOutput,
        };
        let reads = (0..reads)
            .map(|i| ConditionalReadOccurrenceV1 {
                argument: (i % inputs) as u16,
                canonical: ConditionalCanonicalLocationV1 {
                    block: 42,
                    operation: i as u64,
                },
                slice: 1 + i as u32 * 4,
                pointer: 2 + i as u32 * 4,
                index: 3 + i as u32 * 4,
                value: 4 + i as u32 * 4,
                ranked: ConditionalRankedLocationV1 {
                    block: 0,
                    operation: i as u32,
                },
                ranked_view: ConditionalRankedValueV1::Argument((i % inputs) as u32),
                ranked_index: ConditionalRankedValueV1::Local(99),
                access_domain: ConditionalAddressDomainV1::GuardedOutput,
                address_domain: if i % 2 == 0 {
                    ConditionalAddressDomainV1::GlobalLaunch
                } else {
                    ConditionalAddressDomainV1::GuardedOutput
                },
                element_bytes: 4,
                alignment: 4,
            })
            .collect();
        let mut fixture = Self {
            subjects,
            theorem,
            roots: vec![[11, 22, 33, 44]],
            arguments,
            output,
            reads,
            premises: vec![],
        };
        fixture.refresh_theorem();
        fixture.refresh_premises();
        fixture
    }
    pub fn refresh_theorem(&mut self) {
        // Independent reference spelling/order from conditional_ranked_formulas_v1.
        let mut hash = Sha256::new();
        hash.update(b"FE2O3/CONDITIONAL-MEMORY-TRANSITION/V1/LE/SHARED-IEEE\0");
        for d in [
            self.subjects.aggregate_statement_identity,
            self.theorem.generated_source_identity,
            self.theorem.staging_receipt_identity,
            self.theorem.staging_obligation_identity,
            self.theorem.staging_signer_identity,
            self.theorem.staging_execution_identity,
        ] {
            hash.update(d);
        }
        self.theorem.statement_identity = hash.finalize().into();
    }
    pub fn refresh_premises(&mut self) {
        use ConditionalRuntimePremiseV1 as P;
        let parameter = self.arguments[self.output.argument as usize].canonical_parameter;
        self.premises = vec![
            P::D1Launch,
            P::OutputWithinGlobalX { parameter },
            P::WritableOutput { parameter },
            P::RepresentableAddress {
                parameter,
                domain: self.output.address_domain,
                element_bytes: self.output.element_bytes,
                alignment: self.output.alignment,
            },
        ];
        for r in &self.reads {
            let input = self.arguments[r.argument as usize].canonical_parameter;
            self.premises.extend([
                P::ReadableInput {
                    parameter: input,
                    domain: r.access_domain,
                },
                P::SeparateInputOutput {
                    input,
                    output: parameter,
                },
                P::RepresentableAddress {
                    parameter: input,
                    domain: r.address_domain,
                    element_bytes: r.element_bytes,
                    alignment: r.alignment,
                },
            ]);
        }
    }
    pub fn input(&self) -> ConditionalInvocationContractInputV1<'_> {
        ConditionalInvocationContractInputV1 {
            numerical_domain: ConditionalNumericalDomainV1::LittleEndianSharedIeeeV1,
            subjects: self.subjects,
            theorem: self.theorem,
            typed_roots: &self.roots,
            arguments: &self.arguments,
            output: self.output,
            reads: &self.reads,
            premises: &self.premises,
        }
    }
    pub fn wire(&self) -> Vec<u8> {
        let mut bytes =
            vec![
                0;
                encoded_conditional_invocation_contract_v1_len(&self.input(), &mut free).unwrap()
            ];
        encode_conditional_invocation_contract_v1(&self.input(), &mut bytes, &mut free).unwrap();
        bytes
    }
}
