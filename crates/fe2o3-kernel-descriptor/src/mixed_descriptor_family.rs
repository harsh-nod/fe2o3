//! One closed descriptor algorithm with distinct concrete wire families.
//!
//! The shared nominal-subject domain is deliberately unchanged. Contract decoders,
//! contract identity domains, outer headers and public table types stay distinct.

macro_rules! mixed_descriptor_family {
    (
        $magic:literal, $version:literal, $header_error:literal,
        $section:ident, $section_name:literal, $digest_offset:ident,
        $reader_storage:ident, $table:ident, $error:ident,
        $contract:ident, $contract_error:ident, $max_contract:ident, $codec_storage:ident,
        $decode_contract:ident, $decode:ident, $encoded_len:ident, $encode:ident
    ) => {
        use crate::mixed_conditional_v26::mixed_descriptor_subject_v26;
        use crate::nominal_v3::{Output, pay};
        use crate::wire_common::Reader;
        use crate::*;
        use std::{error::Error, fmt, mem::size_of};

        const MAGIC: &[u8; 8] = $magic;
        const HEADER: usize = 8 + 2 + 2 + 4 + 4;

        /// Distinct mixed descriptor section. V3/V5 readers cannot downgrade this wire.
        pub const $section: &str = $section_name;
        /// The only byte field normalized by artifact digest finalization.
        pub const $digest_offset: usize = HEADER + CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V3;
        /// Complete borrowed reader state, nested readers and contract hashing scratch.
        pub const $reader_storage: usize = size_of::<$table<'static>>()
            + size_of::<Reader<'static>>()
            + DESCRIPTOR_READER_SCRATCH_STORAGE_V3
            + $codec_storage
            + size_of::<RootCensus>()
            + size_of::<[u8; 32]>()
            + 8 * size_of::<usize>();

        /// Strict format, association or caller resource failure.
        #[derive(Debug)]
        pub enum $error<E> {
            /// The embedded canonical nominal table was refused.
            Nominal(DescriptorWireErrorV3<E>),
            /// A mandatory mixed contract was refused.
            Contract($contract_error<E>),
            /// A complete roster or content binding differs.
            Binding(&'static str),
            /// Caller supplied the wrong output extent.
            OutputLength {
                /// Exact required extent.
                expected: usize,
                /// Actual supplied extent.
                actual: usize,
            },
        }
        impl<E> From<DescriptorWireErrorV3<E>> for $error<E> {
            fn from(error: DescriptorWireErrorV3<E>) -> Self {
                Self::Nominal(error)
            }
        }
        impl<E> From<$contract_error<E>> for $error<E> {
            fn from(error: $contract_error<E>) -> Self {
                Self::Contract(error)
            }
        }
        impl<E: fmt::Display> fmt::Display for $error<E> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Nominal(error) => error.fmt(f),
                    Self::Contract(error) => error.fmt(f),
                    Self::Binding(rule) => write!(f, "mixed descriptor: {rule}"),
                    Self::OutputLength { expected, actual } => {
                        write!(f, "mixed descriptor extent {actual}, expected {expected}")
                    }
                }
            }
        }
        impl<E: Error + 'static> Error for $error<E> {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                match self {
                    Self::Nominal(error) => Some(error),
                    Self::Contract(error) => Some(error),
                    _ => None,
                }
            }
        }
        type Result<T, E> = std::result::Result<T, $error<E>>;

        /// Inert complete table. It grants no proof, publication or launch authority.
        /// The nested V3 wire is retained exactly because the contract commits to that subject;
        /// it is never reinterpreted as an unconditional launch descriptor.
        pub struct $table<'a> {
            bytes: &'a [u8],
            nominal: DeviceDescriptorTableV3<'a>,
            contracts: [(usize, usize); MAX_KERNELS],
        }

        struct RootCensus {
            roots: [bool; MAX_KERNELS],
            functions: [u32; MAX_KERNELS],
            graphs: Option<[[u8; 32]; 3]>,
        }
        impl RootCensus {
            fn new<E>(
                charge: &mut impl FnMut(usize) -> std::result::Result<(), E>,
            ) -> Result<Self, E> {
                pay(charge, 2 * MAX_KERNELS)?;
                Ok(Self {
                    roots: [false; MAX_KERNELS],
                    functions: [0; MAX_KERNELS],
                    graphs: None,
                })
            }
            fn record<E>(
                &mut self,
                ordinal: usize,
                count: usize,
                contract: &$contract<'_>,
                charge: &mut impl FnMut(usize) -> std::result::Result<(), E>,
            ) -> Result<(), E> {
                pay(charge, 96 + 4)?;
                let subject = contract.subjects();
                let root = subject.original_root as usize;
                let graphs = [
                    subject.source_semantic_identity,
                    subject.original_graph_identity,
                    subject.output_graph_identity,
                ];
                if root >= count
                    || self.roots[root]
                    || self.graphs.is_some_and(|stored| stored != graphs)
                {
                    return Err($error::Binding("complete same-source root census"));
                }
                self.roots[root] = true;
                self.functions[ordinal] = subject.output_function;
                self.graphs = Some(graphs);
                Ok(())
            }
            fn finish<E>(
                &mut self,
                count: usize,
                charge: &mut impl FnMut(usize) -> std::result::Result<(), E>,
            ) -> Result<(), E> {
                let depth = usize::BITS as usize - count.leading_zeros() as usize;
                pay(charge, count * (depth + 1) * 4)?;
                self.functions[..count].sort_unstable();
                if self.functions[..count]
                    .windows(2)
                    .any(|pair| pair[0] == pair[1])
                {
                    return Err($error::Binding("unique actual root functions"));
                }
                Ok(())
            }
        }

        fn join<E>(
            table: &DeviceDescriptorTableV3<'_>,
            ordinal: usize,
            expected_descriptor: &[u8; 32],
            contract: &$contract<'_>,
            charge: &mut impl FnMut(usize) -> std::result::Result<(), E>,
        ) -> Result<(), E> {
            let kernel = table.kernel(ordinal, charge)?;
            let subject = contract.subjects();
            pay(charge, 96 + 9)?;
            if subject.descriptor_identity != *expected_descriptor
                || &subject.kernel_id != kernel.kernel_id().as_bytes()
                || subject.source_rank != kernel.launch().rank()
                || subject.source_argument_count as usize != kernel.argument_count()
                || subject.generated_field_count as usize != kernel.component_count()
                || subject.explicit_argument_bytes != kernel.abi_layout().explicit_argument_size()
                || subject.kernarg_alignment != kernel.abi_layout().kernarg_segment_alignment()
            {
                return Err($error::Binding("exact nominal ABI and mixed subject"));
            }
            Ok(())
        }

        // The nominal hash is shared, not the contract decoder or its identity.
        fn descriptor_subject<E>(
            nominal: &DeviceDescriptorTableV3<'_>,
            charge: &mut impl FnMut(usize) -> std::result::Result<(), E>,
        ) -> Result<[u8; 32], E> {
            mixed_descriptor_subject_v26(nominal, charge).map_err(|error| {
                $error::Contract(match error {
                    crate::mixed_conditional_v26::MixedContractErrorV26::Resource(error) => {
                        $contract_error::Resource(error)
                    }
                    crate::mixed_conditional_v26::MixedContractErrorV26::Invalid(rule) => {
                        $contract_error::Invalid(rule)
                    }
                })
            })
        }

        /// Decode every contract and its exact nominal ABI association, with no fallback.
        /// Callers prepay the family-specific reader storage before this query.
        pub fn $decode<'a, E>(
            bytes: &'a [u8],
            charge: &mut impl FnMut(usize) -> std::result::Result<(), E>,
        ) -> Result<$table<'a>, E> {
            if bytes.len() > MAX_DESCRIPTOR_TABLE_BYTES {
                return Err($error::Binding("complete byte limit"));
            }
            let mut reader = Reader::at(bytes, 0);
            if reader.fixed::<8, E>(charge)? != *MAGIC || reader.u16(charge)? != $version {
                return Err($error::Binding($header_error));
            }
            let count = reader.count("mixed contracts", MAX_KERNELS, charge)?;
            if reader.u32(charge)? as usize != bytes.len() {
                return Err($error::Binding("complete wire extent"));
            }
            let nominal_length = reader.u32(charge)? as usize;
            let nominal =
                decode_device_descriptor_table_v3(reader.take(nominal_length, charge)?, charge)?;
            if count != nominal.kernel_count() || count == 0 {
                return Err($error::Binding("complete kernel contract roster"));
            }
            let descriptor = descriptor_subject(&nominal, charge)?;
            pay(charge, MAX_KERNELS * 2)?;
            let mut contracts = [(0, 0); MAX_KERNELS];
            let mut census = RootCensus::new(charge)?;
            for (ordinal, row) in contracts[..count].iter_mut().enumerate() {
                let length = reader.u32(charge)? as usize;
                if length > $max_contract {
                    return Err($error::Binding("contract byte limit"));
                }
                let offset = reader.position;
                let contract = $decode_contract(reader.take(length, charge)?, charge)?;
                join(&nominal, ordinal, &descriptor, &contract, charge)?;
                census.record(ordinal, count, &contract, charge)?;
                *row = (offset, length);
            }
            census.finish(count, charge)?;
            if reader.position != bytes.len() {
                return Err($error::Binding("trailing mixed bytes"));
            }
            Ok($table {
                bytes,
                nominal,
                contracts,
            })
        }

        fn preflight<E>(
            nominal_bytes: &[u8],
            contracts: &[$contract<'_>],
            charge: &mut impl FnMut(usize) -> std::result::Result<(), E>,
        ) -> Result<usize, E> {
            let nominal = decode_device_descriptor_table_v3(nominal_bytes, charge)?;
            if contracts.len() != nominal.kernel_count() || contracts.is_empty() {
                return Err($error::Binding("complete kernel contract roster"));
            }
            let descriptor = descriptor_subject(&nominal, charge)?;
            let mut census = RootCensus::new(charge)?;
            let mut total = HEADER
                .checked_add(nominal_bytes.len())
                .ok_or($error::Binding("table extent"))?;
            for (ordinal, stored) in contracts.iter().enumerate() {
                let contract = $decode_contract(stored.canonical_bytes(), charge)?;
                join(&nominal, ordinal, &descriptor, &contract, charge)?;
                census.record(ordinal, contracts.len(), &contract, charge)?;
                total = total
                    .checked_add(4)
                    .and_then(|n| n.checked_add(contract.canonical_bytes().len()))
                    .filter(|n| *n <= MAX_DESCRIPTOR_TABLE_BYTES)
                    .ok_or($error::Binding("complete byte limit"))?;
            }
            census.finish(contracts.len(), charge)?;
            Ok(total)
        }

        /// Exact output length after independently checking every nested subject.
        pub fn $encoded_len<E>(
            nominal: &[u8],
            contracts: &[$contract<'_>],
            charge: &mut impl FnMut(usize) -> std::result::Result<(), E>,
        ) -> Result<usize, E> {
            preflight(nominal, contracts, charge)
        }

        /// Encode only after all checks and work debits succeed; errors leave output intact.
        pub fn $encode<E>(
            nominal: &[u8],
            contracts: &[$contract<'_>],
            output: &mut [u8],
            charge: &mut impl FnMut(usize) -> std::result::Result<(), E>,
        ) -> Result<(), E> {
            let expected = preflight(nominal, contracts, charge)?;
            if output.len() != expected {
                return Err($error::OutputLength {
                    expected,
                    actual: output.len(),
                });
            }
            pay(charge, expected * 2 + 1)?;
            let mut writer = Output {
                bytes: output,
                position: 0,
            };
            writer.bytes(MAGIC);
            writer.u16($version);
            writer.u16(contracts.len() as u16);
            writer.u32(expected as u32);
            writer.u32(nominal.len() as u32);
            writer.bytes(nominal);
            for contract in contracts {
                writer.u32(contract.canonical_bytes().len() as u32);
                writer.bytes(contract.canonical_bytes());
            }
            Ok(())
        }

        impl<'a> $table<'a> {
            /// Complete versioned bytes, including all mandatory contracts.
            pub fn canonical_bytes(&self) -> &'a [u8] {
                self.bytes
            }
            /// Exact nested nominal bytes for ABI packing replay only. Consumers must
            /// retain this versioned owner and the mandatory same-ordinal mixed contracts;
            /// decoding these bytes alone does not establish an unconditional kernel.
            pub fn nominal_canonical_bytes(&self) -> &'a [u8] {
                self.nominal.canonical_bytes()
            }
            /// Number of exact kernel/contract pairs.
            pub fn kernel_count(&self) -> usize {
                self.nominal.kernel_count()
            }
            /// Exact target of the complete table.
            pub fn device_target(&self) -> DeviceTargetV1 {
                self.nominal.device_target()
            }
            /// Exact final code-object digest, zero only before finalization.
            pub fn canonical_code_object_digest(&self) -> CanonicalCodeObjectDigest {
                self.nominal.canonical_code_object_digest()
            }
            /// Exact physical code-object version; this query does not erase contracts.
            pub fn code_object_version(&self) -> CodeObjectVersion {
                self.nominal.code_object_version()
            }
            /// Exact target requirement for the same-ordinal mandatory-contract kernel.
            pub fn requirement<E>(
                &self,
                ordinal: usize,
                charge: &mut impl FnMut(usize) -> std::result::Result<(), E>,
            ) -> Result<KernelTargetRequirementsV2, E> {
                Ok(self.nominal.requirement(ordinal, charge)?)
            }
            /// Original nominal type row for physical ABI cross-checking only.
            pub fn source_type<E>(
                &self,
                identity: RustTypeIdentity,
                charge: &mut impl FnMut(usize) -> std::result::Result<(), E>,
            ) -> Result<SourceTypeRecordV3, E> {
                Ok(self.nominal.source_type(identity, charge)?)
            }
            /// Exact nominal kernel view. It must be consumed with its mandatory contract.
            pub fn kernel<E>(
                &self,
                ordinal: usize,
                charge: &mut impl FnMut(usize) -> std::result::Result<(), E>,
            ) -> Result<KernelDescriptorRefV3<'_, 'a>, E> {
                Ok(self.nominal.kernel(ordinal, charge)?)
            }
            /// Re-decodes the mandatory same-ordinal mixed contract.
            pub fn contract<E>(
                &self,
                ordinal: usize,
                charge: &mut impl FnMut(usize) -> std::result::Result<(), E>,
            ) -> Result<$contract<'a>, E> {
                pay(charge, 2)?;
                let &(offset, length) = self
                    .contracts
                    .get(ordinal)
                    .filter(|_| ordinal < self.kernel_count())
                    .ok_or($error::Binding("kernel ordinal"))?;
                Ok($decode_contract(
                    &self.bytes[offset..offset + length],
                    charge,
                )?)
            }
            /// Structural content alone never authorizes publication or launch.
            pub const fn grants_authority(&self) -> bool {
                false
            }
        }
    };
}

pub(crate) use mixed_descriptor_family;
