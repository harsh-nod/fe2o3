//! Closed target-selection codecs sharing one bounded association algorithm.

macro_rules! mixed_target_selection_family {
    ($magic_name:ident, $max_bytes:ident, $scratch:ident, $row:ident, $input:ident, $error:ident, $view:ident, $length:ident, $encode:ident, $magic:literal, $version:literal) => {
        use crate::TargetLineageIdentityV3;
        use fe2o3_amd_target::ProductionAmdTargetProfileV1;
        use fe2o3_kernel_descriptor::{MAX_KERNELS, MAX_NAME_BYTES};
        use std::{convert::Infallible, mem::size_of};

        /// Distinct selection framing; legacy V2/V3 target readers cannot admit it.
        pub const $magic_name: [u8; 8] = *$magic;
        const HEADER: usize = 16 + 8 + 3 * 40 + 32 + 4;
        /// Complete maximum wire extent, independently bounded before reading.
        pub const $max_bytes: usize = HEADER + MAX_KERNELS * (2 + MAX_NAME_BYTES + 12);

        /// One exact root in canonical V18 kernel order, not descriptor sort order.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $row<'a> {
            /// Stable kernel identifier.
            pub kernel: &'a str,
            /// Exact nonzero workgroup dimensions.
            pub workgroup: [u32; 3],
        }
        const EMPTY: $row<'static> = $row {
            kernel: "",
            workgroup: [0; 3],
        };

        /// Explicit association inputs. Identities and this codec grant no authority.
        #[derive(Clone, Copy, Debug)]
        pub struct $input<'a> {
            /// Exact original protected invocation coordinates.
            pub invocation: TargetLineageIdentityV3,
            /// Exact outer semantic-MIR receipt coordinates.
            pub semantic_mir: TargetLineageIdentityV3,
            /// One exact final V18 graph, unchanged by target selection.
            pub kernel_ir: TargetLineageIdentityV3,
            /// SHA256 of the complete family-specific descriptor and mandatory contracts.
            pub descriptor_sha256: [u8; 32],
            /// Exact existing AMD target profile, including CPU, features and triple.
            pub profile: ProductionAmdTargetProfileV1,
            /// Complete ordered root census, including singleton modules.
            pub workgroups: &'a [$row<'a>],
        }

        /// Strict schema, caller-accounting, or exact-association refusal.
        #[derive(Debug, Eq, PartialEq)]
        pub enum $error<E = Infallible> {
            /// The caller's original work refusal.
            Charge(E),
            /// Header, version, policy, profile or reserved field differs.
            Schema,
            /// Truncated, oversized, trailing or wrong output extent.
            Length,
            /// Invalid identity, root census, text or dimensions.
            Binding,
            /// Fixed decoder and validation scratch was not prepaid.
            Storage,
        }
        type Error<E> = $error<E>;

        /// Borrowed strict selection content. No owned graph or proof is reconstructed.
        pub struct $view<'a> {
            bytes: &'a [u8],
            invocation: TargetLineageIdentityV3,
            semantic: TargetLineageIdentityV3,
            graph: TargetLineageIdentityV3,
            descriptor: [u8; 32],
            profile: ProductionAmdTargetProfileV1,
            rows: [$row<'a>; MAX_KERNELS],
            count: usize,
        }

        /// All fixed reader/encoder, census-sort, identity and returned-view scratch.
        /// Immutable input/output backing remains caller-owned and separately prepaid.
        pub const $scratch: usize = 2 * size_of::<$view<'static>>()
            + size_of::<[usize; MAX_KERNELS]>()
            + 2 * size_of::<$input<'static>>()
            + 2 * size_of::<Reader<'static>>()
            + size_of::<[TargetLineageIdentityV3; 3]>()
            + size_of::<[u8; 40]>()
            + size_of::<[u8; 32]>()
            + size_of::<[u8; 12]>()
            + 16 * size_of::<usize>();

        fn charge<E>(
            pay: &mut impl FnMut(usize) -> Result<(), E>,
            n: usize,
        ) -> Result<(), Error<E>> {
            pay(n).map_err(Error::Charge)
        }
        fn extent<E>(
            input: $input<'_>,
            scratch: usize,
            pay: &mut impl FnMut(usize) -> Result<(), E>,
        ) -> Result<usize, Error<E>> {
            if scratch < $scratch {
                return Err(Error::Storage);
            }
            charge(pay, 33)?;
            if input.descriptor_sha256 == [0; 32]
                || !(1..=MAX_KERNELS).contains(&input.workgroups.len())
            {
                return Err(Error::Binding);
            }
            charge(pay, MAX_KERNELS)?;
            let mut order = [0usize; MAX_KERNELS];
            let mut length = HEADER;
            for (index, row) in input.workgroups.iter().enumerate() {
                charge(pay, row.kernel.len() + 5)?;
                if row.kernel.is_empty()
                    || row.kernel.len() > MAX_NAME_BYTES
                    || !row.kernel.is_ascii()
                    || row
                        .kernel
                        .bytes()
                        .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
                    || row.workgroup.contains(&0)
                {
                    return Err(Error::Binding);
                }
                length += 2 + row.kernel.len() + 12;
                order[index] = index;
            }
            let count = input.workgroups.len();
            let depth = usize::BITS as usize - count.leading_zeros() as usize;
            charge(pay, count * (depth + 1) * (MAX_NAME_BYTES + 1))?;
            order[..count].sort_unstable_by_key(|&index| input.workgroups[index].kernel);
            if order[..count]
                .windows(2)
                .any(|p| input.workgroups[p[0]].kernel == input.workgroups[p[1]].kernel)
            {
                return Err(Error::Binding);
            }
            Ok(length)
        }

        /// Determines the exact output extent with metered complete-census validation.
        pub fn $length<E>(
            input: $input<'_>,
            scratch: usize,
            mut pay: impl FnMut(usize) -> Result<(), E>,
        ) -> Result<usize, Error<E>> {
            extent(input, scratch, &mut pay)
        }

        /// Encodes into caller-prepaid storage. No heap allocation or proof fallback.
        pub fn $encode<E>(
            input: $input<'_>,
            bytes: &mut [u8],
            scratch: usize,
            mut pay: impl FnMut(usize) -> Result<(), E>,
        ) -> Result<(), Error<E>> {
            let length = extent(input, scratch, &mut pay)?;
            if bytes.len() != length {
                return Err(Error::Length);
            }
            charge(&mut pay, length)?;
            let mut offset = 0;
            let mut put = |value: &[u8]| {
                bytes[offset..offset + value.len()].copy_from_slice(value);
                offset += value.len();
            };
            put(&$magic_name);
            put(&($version as u16).to_le_bytes());
            put(&1u16.to_le_bytes()); // Association-only selection, never refinement.
            put(&(length as u32).to_le_bytes());
            put(&18u16.to_le_bytes());
            let profile: u16 = match input.profile {
                ProductionAmdTargetProfileV1::Gfx942 => 942,
                ProductionAmdTargetProfileV1::Gfx950 => 950,
            };
            put(&profile.to_le_bytes());
            put(&6u16.to_le_bytes());
            put(&64u16.to_le_bytes());
            for identity in [input.invocation, input.semantic_mir, input.kernel_ir] {
                put(&identity.encode());
            }
            put(&input.descriptor_sha256);
            put(&(input.workgroups.len() as u32).to_le_bytes());
            for row in input.workgroups {
                put(&(row.kernel.len() as u16).to_le_bytes());
                put(row.kernel.as_bytes());
                for dimension in row.workgroup {
                    put(&dimension.to_le_bytes());
                }
            }
            Ok(())
        }

        struct Reader<'a> {
            bytes: &'a [u8],
            offset: usize,
        }
        impl<'a> Reader<'a> {
            fn take<E>(&mut self, n: usize) -> Result<&'a [u8], Error<E>> {
                let end = self.offset.checked_add(n).ok_or(Error::Length)?;
                let value = self.bytes.get(self.offset..end).ok_or(Error::Length)?;
                self.offset = end;
                Ok(value)
            }
            fn u16<E>(&mut self) -> Result<u16, Error<E>> {
                Ok(u16::from_le_bytes(
                    self.take(2)?.try_into().map_err(|_| Error::Length)?,
                ))
            }
            fn u32<E>(&mut self) -> Result<u32, Error<E>> {
                Ok(u32::from_le_bytes(
                    self.take(4)?.try_into().map_err(|_| Error::Length)?,
                ))
            }
            fn identity<E>(&mut self) -> Result<TargetLineageIdentityV3, Error<E>> {
                TargetLineageIdentityV3::decode("mixed target subject", self.take(40)?)
                    .map_err(|_| Error::Binding)
            }
        }

        impl<'a> $view<'a> {
            /// Strictly decodes complete schema and unique ordered root census.
            pub fn read<E>(
                bytes: &'a [u8],
                scratch: usize,
                mut pay: impl FnMut(usize) -> Result<(), E>,
            ) -> Result<Self, Error<E>> {
                if scratch < $scratch {
                    return Err(Error::Storage);
                }
                if !(HEADER..=$max_bytes).contains(&bytes.len()) {
                    return Err(Error::Length);
                }
                charge(&mut pay, bytes.len() + MAX_KERNELS)?;
                let mut r = Reader { bytes, offset: 0 };
                if r.take(8)? != $magic_name || r.u16()? != $version || r.u16()? != 1 {
                    return Err(Error::Schema);
                }
                if r.u32()? as usize != bytes.len() {
                    return Err(Error::Length);
                }
                if r.u16()? != 18 {
                    return Err(Error::Schema);
                }
                let profile = match r.u16()? {
                    942 => ProductionAmdTargetProfileV1::Gfx942,
                    950 => ProductionAmdTargetProfileV1::Gfx950,
                    _ => return Err(Error::Schema),
                };
                if r.u16()? != 6 || r.u16()? != 64 {
                    return Err(Error::Schema);
                }
                let invocation = r.identity()?;
                let semantic = r.identity()?;
                let graph = r.identity()?;
                let descriptor = r.take(32)?.try_into().map_err(|_| Error::Length)?;
                let count = r.u32()? as usize;
                if !(1..=MAX_KERNELS).contains(&count) {
                    return Err(Error::Binding);
                }
                let mut rows = [EMPTY; MAX_KERNELS];
                for row in &mut rows[..count] {
                    let length = r.u16()? as usize;
                    if length > MAX_NAME_BYTES {
                        return Err(Error::Binding);
                    }
                    row.kernel =
                        std::str::from_utf8(r.take(length)?).map_err(|_| Error::Binding)?;
                    row.workgroup = [r.u32()?, r.u32()?, r.u32()?];
                }
                if r.offset != bytes.len() {
                    return Err(Error::Length);
                }
                let value = Self {
                    bytes,
                    invocation,
                    semantic,
                    graph,
                    descriptor,
                    profile,
                    rows,
                    count,
                };
                extent(value.inputs(), scratch, &mut pay)?;
                Ok(value)
            }
            /// Exact inert content coordinates and ordered roots, without ownership promotion.
            pub fn inputs(&self) -> $input<'_> {
                $input {
                    invocation: self.invocation,
                    semantic_mir: self.semantic,
                    kernel_ir: self.graph,
                    descriptor_sha256: self.descriptor,
                    profile: self.profile,
                    workgroups: &self.rows[..self.count],
                }
            }
            /// Original immutable canonical bytes.
            pub fn canonical_bytes(&self) -> &'a [u8] {
                self.bytes
            }
            /// Selection association is not a semantic or native refinement theorem.
            pub const fn establishes_refinement_proof(&self) -> bool {
                false
            }
            /// This content decoder grants no publication, load, or launch authority.
            pub const fn grants_runtime_authority(&self) -> bool {
                false
            }
        }
    };
}
pub(crate) use mixed_target_selection_family;
