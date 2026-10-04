//! Shared metered nominal descriptor reader; versions supply their framing.
use crate::model;
use crate::nominal_v3::*;
use crate::wire_v3::count;
use crate::*;
type ResultV3<T, E> = Result<T, DescriptorWireErrorV3<E>>;

pub(crate) struct Reader<'a> {
    pub(crate) bytes: &'a [u8],
    pub(crate) position: usize,
}
impl<'a> Reader<'a> {
    pub(crate) fn at(bytes: &'a [u8], position: usize) -> Self {
        Self { bytes, position }
    }
    pub(crate) fn take<E>(
        &mut self,
        n: usize,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV3<&'a [u8], E> {
        pay(c, 3 * n + 1)?;
        let end = self.position.checked_add(n).ok_or(DecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.position..end)
            .ok_or(DecodeError::Truncated)?;
        self.position = end;
        Ok(value)
    }
    pub(crate) fn fixed<const N: usize, E>(
        &mut self,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV3<[u8; N], E> {
        Ok(self
            .take(N, c)?
            .try_into()
            .map_err(|_| DecodeError::Truncated)?)
    }
    pub(crate) fn u8<E>(&mut self, c: &mut impl FnMut(usize) -> Result<(), E>) -> ResultV3<u8, E> {
        Ok(self.fixed::<1, E>(c)?[0])
    }
    pub(crate) fn u16<E>(
        &mut self,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV3<u16, E> {
        Ok(u16::from_le_bytes(self.fixed(c)?))
    }
    pub(crate) fn u32<E>(
        &mut self,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV3<u32, E> {
        Ok(u32::from_le_bytes(self.fixed(c)?))
    }
    pub(crate) fn zero<E>(
        &mut self,
        field: &'static str,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV3<(), E> {
        if self.u16(c)? != 0 {
            return Err(DecodeError::NonzeroReserved { field }.into());
        }
        Ok(())
    }
    pub(crate) fn count<E>(
        &mut self,
        field: &'static str,
        max: usize,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV3<usize, E> {
        let n = usize::from(self.u16(c)?);
        count(n, field, max)?;
        Ok(n)
    }
    pub(crate) fn text<E>(
        &mut self,
        field: &'static str,
        name: bool,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV3<&'a str, E> {
        let n = self.count(field, if name { MAX_NAME_BYTES } else { MAX_TEXT_BYTES }, c)?;
        let bytes = self.take(n, c)?;
        pay(c, n + 1)?;
        let text = std::str::from_utf8(bytes).map_err(|_| DecodeError::InvalidText { field })?;
        if name {
            model::validate_name(text, field)?;
        } else {
            model::validate_text(text, field)?;
        }
        Ok(text)
    }
}
