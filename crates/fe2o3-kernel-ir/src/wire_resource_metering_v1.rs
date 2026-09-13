enum WriterModeV1<'input> {
    Materialize,
    Count,
    Compare {
        expected: &'input [u8],
        matches: bool,
    },
}

struct Writer<'budget> {
    bytes: Vec<u8>,
    length: usize,
    peak_auxiliary_bytes: usize,
    mode: WriterModeV1<'budget>,
    version: u16,
    budget: Option<&'budget mut CanonicalKernelIrWorkBudgetV1>,
}

impl<'budget> Writer<'budget> {
    fn new(version: u16, budget: Option<&'budget mut CanonicalKernelIrWorkBudgetV1>) -> Self {
        Self {
            bytes: Vec::new(),
            length: 0,
            peak_auxiliary_bytes: 0,
            mode: WriterModeV1::Materialize,
            version,
            budget,
        }
    }

    fn counter(version: u16, budget: &'budget mut CanonicalKernelIrWorkBudgetV1) -> Self {
        Self {
            bytes: Vec::new(),
            length: 0,
            peak_auxiliary_bytes: 0,
            mode: WriterModeV1::Count,
            version,
            budget: Some(budget),
        }
    }

    fn comparing(
        version: u16,
        expected: &'budget [u8],
        budget: Option<&'budget mut CanonicalKernelIrWorkBudgetV1>,
    ) -> Self {
        Self {
            bytes: Vec::new(),
            length: 0,
            peak_auxiliary_bytes: 0,
            mode: WriterModeV1::Compare {
                expected,
                matches: true,
            },
            version,
            budget,
        }
    }

    fn counts_only(&self) -> bool {
        matches!(self.mode, WriterModeV1::Count)
    }

    fn charge_work(&mut self, work: usize) -> Result<(), KernelIrEncodeError> {
        if let Some(budget) = self.budget.as_deref_mut() {
            budget
                .charge_work(work)
                .map_err(KernelIrEncodeError::WorkLimit)?;
        }
        Ok(())
    }

    fn with_exact_capacity(
        version: u16,
        exact_length: usize,
        budget: &'budget mut CanonicalKernelIrWorkBudgetV1,
    ) -> Result<Self, KernelIrEncodeError> {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(exact_length)
            .map_err(|_| KernelIrEncodeError::Allocation)?;
        Ok(Self {
            bytes,
            length: 0,
            peak_auxiliary_bytes: 0,
            mode: WriterModeV1::Materialize,
            version,
            budget: Some(budget),
        })
    }

    const fn length(&self) -> usize {
        self.length
    }

    fn finish_module(mut self) -> Result<Vec<u8>, KernelIrEncodeError> {
        if !matches!(self.mode, WriterModeV1::Materialize) {
            return Err(KernelIrEncodeError::NonCanonical {
                field: "encoded bytes require a materializing writer",
            });
        }
        self.finish_length_field()?;
        Ok(self.bytes)
    }

    fn finish_comparison(mut self) -> Result<bool, KernelIrEncodeError> {
        if !matches!(self.mode, WriterModeV1::Compare { .. }) {
            return Err(KernelIrEncodeError::NonCanonical {
                field: "encoding comparison requires a comparing writer",
            });
        }
        self.finish_length_field()?;
        self.charge_work(1)?;
        let WriterModeV1::Compare { expected, matches } = self.mode else {
            return Err(KernelIrEncodeError::NonCanonical {
                field: "encoding comparison requires a comparing writer",
            });
        };
        Ok(matches && expected.len() == self.length)
    }

    fn finish_length_field(&mut self) -> Result<(), KernelIrEncodeError> {
        let length = u32::try_from(self.length).map_err(|_| KernelIrEncodeError::Overflow {
            field: "module length",
        })?;
        if let Some(budget) = self.budget.as_deref_mut() {
            budget
                .charge_work(std::mem::size_of::<u32>())
                .map_err(KernelIrEncodeError::WorkLimit)?;
        }
        match self.mode {
            WriterModeV1::Materialize => {
                self.bytes[12..16].copy_from_slice(&length.to_le_bytes());
            }
            WriterModeV1::Compare { .. } => self.compare_chunk(12, &length.to_le_bytes())?,
            WriterModeV1::Count => {}
        }
        Ok(())
    }

    fn module_length_placeholder(&mut self) -> Result<(), KernelIrEncodeError> {
        if matches!(self.mode, WriterModeV1::Compare { .. }) {
            // The encoder patches only this fixed header field at finish.
            // Compare its computed value there, after every schema check.
            self.length = self.admit_bytes(4)?;
            Ok(())
        } else {
            self.u32(0)
        }
    }

    fn compare_chunk(&mut self, offset: usize, value: &[u8]) -> Result<(), KernelIrEncodeError> {
        // Byte comparisons were admitted by the write or length-field patch.
        // The checked slice query also executes for empty chunks.
        self.charge_work(1)?;
        let WriterModeV1::Compare { expected, matches } = &mut self.mode else {
            return Err(KernelIrEncodeError::NonCanonical {
                field: "encoding comparison requires a comparing writer",
            });
        };
        let end = offset
            .checked_add(value.len())
            .ok_or(KernelIrEncodeError::Overflow {
                field: "module length",
            })?;
        // Keep walking after mismatches so a later encoding error retains
        // precedence over the final canonical-byte mismatch.
        *matches &= expected
            .get(offset..end)
            .is_some_and(|bytes| bytes == value);
        Ok(())
    }

    fn bytes(&mut self, value: &[u8]) -> Result<(), KernelIrEncodeError> {
        if self.counts_only() {
            return self.count_bytes(value.len());
        }
        let next = self.admit_bytes(value.len())?;
        match self.mode {
            WriterModeV1::Materialize => self.bytes.extend_from_slice(value),
            WriterModeV1::Compare { .. } => self.compare_chunk(self.length, value)?,
            WriterModeV1::Count => {}
        }
        self.length = next;
        Ok(())
    }

    fn count_bytes(&mut self, length: usize) -> Result<(), KernelIrEncodeError> {
        if !self.counts_only() {
            return Err(KernelIrEncodeError::NonCanonical {
                field: "byte extent requires a counting writer",
            });
        }
        let next = self.checked_extent(length)?;
        // One schema token observes a field width and advances its checked
        // extent. Even empty spans cost a token; no payload bytes are read.
        self.charge_work(1)?;
        self.length = next;
        Ok(())
    }

    fn admit_bytes(&mut self, length: usize) -> Result<usize, KernelIrEncodeError> {
        let next = self.checked_extent(length)?;
        self.charge_work(length)?;
        Ok(next)
    }

    fn checked_extent(&self, length: usize) -> Result<usize, KernelIrEncodeError> {
        let next = self
            .length
            .checked_add(length)
            .ok_or(KernelIrEncodeError::Overflow {
                field: "module length",
            })?;
        if next > MAX_MODULE_BYTES_V1 {
            return Err(KernelIrEncodeError::TooLarge {
                max: MAX_MODULE_BYTES_V1,
            });
        }
        Ok(next)
    }

    fn u8(&mut self, value: u8) -> Result<(), KernelIrEncodeError> {
        if self.counts_only() {
            return self.count_bytes(1);
        }
        self.bytes(&[value])
    }

    fn u16(&mut self, value: u16) -> Result<(), KernelIrEncodeError> {
        if self.counts_only() {
            return self.count_bytes(2);
        }
        self.bytes(&value.to_le_bytes())
    }

    fn u32(&mut self, value: u32) -> Result<(), KernelIrEncodeError> {
        if self.counts_only() {
            return self.count_bytes(4);
        }
        self.bytes(&value.to_le_bytes())
    }

    fn u64(&mut self, value: u64) -> Result<(), KernelIrEncodeError> {
        if self.counts_only() {
            return self.count_bytes(8);
        }
        self.bytes(&value.to_le_bytes())
    }

    fn count(
        &mut self,
        field: &'static str,
        value: usize,
        max: usize,
    ) -> Result<(), KernelIrEncodeError> {
        check_limit(field, value, max)?;
        self.u32(u32::try_from(value).map_err(|_| KernelIrEncodeError::Overflow { field })?)
    }

    fn text(&mut self, field: &'static str, value: &str) -> Result<(), KernelIrEncodeError> {
        check_limit(field, value.len(), MAX_TEXT_BYTES_V1)?;
        self.u32(u32::try_from(value.len()).map_err(|_| KernelIrEncodeError::Overflow { field })?)?;
        if self.counts_only() {
            self.count_bytes(value.len())
        } else {
            self.bytes(value.as_bytes())
        }
    }
}

fn check_limit(field: &'static str, actual: usize, max: usize) -> Result<(), KernelIrEncodeError> {
    if actual > max {
        Err(KernelIrEncodeError::LimitExceeded { field, actual, max })
    } else {
        Ok(())
    }
}

struct Reader<'input, 'budget> {
    bytes: &'input [u8],
    offset: usize,
    version: u16,
    budget: Option<DecodeBudgetV12<'budget>>,
}

impl<'input, 'budget> Reader<'input, 'budget> {
    fn new(bytes: &'input [u8], budget: Option<DecodeBudgetV12<'budget>>) -> Self {
        Self {
            bytes,
            offset: 0,
            version: 0,
            budget,
        }
    }

    fn take(&mut self, length: usize) -> Result<&'input [u8], KernelIrDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(KernelIrDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(KernelIrDecodeError::Truncated)?;
        if let Some(budget) = self.budget.as_mut() {
            budget
                .charge_work(length)
                .map_err(KernelIrDecodeError::WorkLimit)?;
        }
        self.offset = end;
        Ok(value)
    }

    fn charge_work(&mut self, amount: usize) -> Result<(), KernelIrDecodeError> {
        if let Some(budget) = self.budget.as_mut() {
            budget
                .charge_work(amount)
                .map_err(KernelIrDecodeError::WorkLimit)?;
        }
        Ok(())
    }

    fn work_limit(&self) -> usize {
        self.budget
            .as_ref()
            .map_or(usize::MAX, |budget| budget.limit())
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], KernelIrDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| KernelIrDecodeError::Truncated)
    }

    fn u8(&mut self) -> Result<u8, KernelIrDecodeError> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, KernelIrDecodeError> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, KernelIrDecodeError> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, KernelIrDecodeError> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn reserved_u32(&mut self, field: &'static str) -> Result<(), KernelIrDecodeError> {
        if self.u32()? != 0 {
            Err(KernelIrDecodeError::ReservedNonZero { field })
        } else {
            Ok(())
        }
    }

    fn count(&mut self, field: &'static str, max: usize) -> Result<usize, KernelIrDecodeError> {
        let count = self.u32()? as usize;
        if count > max {
            Err(KernelIrDecodeError::LimitExceeded {
                field,
                actual: count,
                max,
            })
        } else {
            Ok(count)
        }
    }

    fn text(&mut self, field: &'static str) -> Result<String, KernelIrDecodeError> {
        let length = self.count(field, MAX_TEXT_BYTES_V1)?;
        let bytes = self.take(length)?;
        if let Some(budget) = self.budget.as_mut() {
            let validation_and_copy = length.checked_mul(2).ok_or_else(|| {
                KernelIrDecodeError::WorkLimit(CanonicalKernelIrWorkLimitV1::new(
                    usize::MAX,
                    budget.limit(),
                ))
            })?;
            budget
                .charge_work(validation_and_copy)
                .map_err(KernelIrDecodeError::WorkLimit)?;
        }
        let value =
            str::from_utf8(bytes).map_err(|_| KernelIrDecodeError::InvalidUtf8 { field })?;
        if !self.tracks_allocations() {
            return Ok(value.to_owned());
        }
        self.reserve_payload(length)?;
        let mut owned = String::new();
        owned.try_reserve_exact(length).map_err(|_| {
            KernelIrDecodeError::Resource(
                crate::CanonicalKernelIrVerificationResourceErrorV1::Allocation,
            )
        })?;
        if owned.capacity() != length {
            return Err(KernelIrDecodeError::Resource(
                crate::CanonicalKernelIrVerificationResourceErrorV1::Allocation,
            ));
        }
        owned.push_str(value);
        Ok(owned)
    }

    fn option(&mut self, field: &'static str) -> Result<bool, KernelIrDecodeError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(KernelIrDecodeError::UnknownTag { kind: field, tag }),
        }
    }

    fn into_work_budget(self) -> Option<DecodeBudgetV12<'budget>> {
        self.budget
    }

    fn boolean(&mut self, field: &'static str) -> Result<bool, KernelIrDecodeError> {
        self.option(field)
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
