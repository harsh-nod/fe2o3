//! Retained original read/operand/scalar-operand operations and concrete observer.
//! No rvalue/install producer, source/SSA admission or ordinary activation.
use super::*;
#[derive(Clone, Copy)]
enum ReadOutput {
    Retained,
    Selected,
}

impl<'s, 'a, 'w> RetainedAliasSessionV1<'s, 'a, 'w> {
    pub(super) fn read_place(&mut self, place: &'a SemanticPlaceV1, consume: bool) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            if self.owner.removed.is_some()
                || self.owner.drain.is_some()
                || self.owner.current.is_some()
                || !self.owner.output.is_empty()
                || !self.owner.selected.is_empty()
                || !self.owner.fields.is_empty()
            {
                return Err(Error::ReplayMismatch);
            }
            self.owner.place = Some(place);
            self.owner.path = None;

            let local = place.local().index();
            let Some(path) = self.path(place)? else {
                self.poison_holder(local)?;
                self.source_write(local)?;
                return Ok(());
            };
            if consume && !path.dereference {
                self.source_write(local)?;
            }
            self.tree()?;
            self.owner.drain = self.owner.data.holders.remove(&local).map(Vec::into_iter);
            if self.owner.drain.is_some() {
                loop {
                    self.owner.current = self
                        .owner
                        .drain
                        .as_mut()
                        .ok_or(Error::ReplayMismatch)?
                        .next();
                    if self.owner.current.is_none() {
                        break;
                    }
                    if self.matches(Slot::Current, path.fields)? {
                        let index = self.alias(Slot::Current)?.candidate;
                        if !self.alias(Slot::Current)?.live {
                            self.owner.data.candidates[index].valid = false;
                        } else if path.dereference {
                            let length = self.alias(Slot::Current)?.fields.len();
                            let candidate = &mut self.owner.data.candidates[index];
                            if length != path.fields.len()
                                || consume
                                || candidate.reference != path.selected
                                || candidate.pointee != place.ty()
                            {
                                candidate.valid = false;
                            } else {
                                candidate.read = true;
                                if let Some(site) = self.owner.site {
                                    let source = self.owner.source.ok_or(Error::ReplayMismatch)?;
                                    self.owner.observer.record(
                                        source.function,
                                        source.types,
                                        candidate.site,
                                        candidate.source,
                                        (site.block, site.statement),
                                        place,
                                        self.budget,
                                        self.owned,
                                        &mut self.owner.failure,
                                    )?;
                                }
                            }
                        } else {
                            let length =
                                self.alias(Slot::Current)?.fields.len() - path.fields.len();
                            if self.reserve(1 + length)? {
                                alias_work(self.budget, &mut self.owner.failure, length)?;
                                self.add_live(index)?;
                                self.read_field_capacity(length)?;
                                self.owner.fields.extend_from_slice(
                                    &self
                                        .owner
                                        .current
                                        .as_ref()
                                        .ok_or(Error::ReplayMismatch)?
                                        .fields[path.fields.len()..],
                                );
                                self.read_alias_capacity(ReadOutput::Selected)?;
                                let fields = std::mem::take(&mut self.owner.fields);
                                self.owner.selected.push(Alias {
                                    candidate: index,
                                    fields,
                                    live: true,
                                });
                                if consume {
                                    self.deactivate(Slot::Current)?;
                                }
                            }
                        }
                    }
                    self.read_alias_capacity(ReadOutput::Retained)?;
                    let alias = self.owner.current.take().ok_or(Error::ReplayMismatch)?;
                    self.owner.output.push(alias);
                }
                self.owner.drain = None;
            }
            if !self.owner.output.is_empty() {
                self.tree()?;
                self.reserve_storage(size_of::<(u32, Vec<Alias>)>())
                    .map_err(|_| Error::ResourceOverflow)?;
                let retained = std::mem::take(&mut self.owner.output);
                self.owner.data.holders.insert(local, retained);
            }
            Ok(())
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    pub(super) fn operand(&mut self, operand: &'a SemanticOperandV1) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            if !self.owner.selected.is_empty() || !self.owner.fields.is_empty() {
                return Err(Error::ReplayMismatch);
            }
            self.owner.operand = Some(operand);

            alias_work(self.budget, &mut self.owner.failure, 1)?;
            match operand {
                SemanticOperandV1::Copy(place) => self.read_place(place, false),
                SemanticOperandV1::Move(place) => self.read_place(place, true),
                SemanticOperandV1::Constant(_) => Ok(()),
            }
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    pub(super) fn scalar_operand(&mut self, operand: &'a SemanticOperandV1) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            if self.owner.drain.is_some() || self.owner.current.is_some() {
                return Err(Error::ReplayMismatch);
            }
            self.operand(operand)?;
            self.owner.drain = Some(std::mem::take(&mut self.owner.selected).into_iter());
            self.discard_pending(true)
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    fn read_alias_capacity(&mut self, output: ReadOutput) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            let (length, old) = match output {
                ReadOutput::Retained => (self.owner.output.len(), self.owner.output.capacity()),
                ReadOutput::Selected => (self.owner.selected.len(), self.owner.selected.capacity()),
            };
            if length < old {
                return Ok(());
            }
            let needed = length
                .checked_add(1)
                .ok_or(Resource::Arithmetic)
                .map_err(|error| {
                    self.failed(error);
                    Error::ResourceOverflow
                })?;
            let growth = needed
                .checked_sub(old)
                .and_then(|count| count.checked_mul(size_of::<Alias>()))
                .ok_or(Resource::Arithmetic)
                .map_err(|error| {
                    self.failed(error);
                    Error::ResourceOverflow
                })?;
            self.reserve_storage(growth)
                .map_err(|_| Error::ResourceOverflow)?;
            match output {
                ReadOutput::Retained => self.owner.output.try_reserve_exact(1),
                ReadOutput::Selected => self.owner.selected.try_reserve_exact(1),
            }
            .map_err(|_| {
                self.failed(Resource::Allocation);
                Error::ResourceOverflow
            })?;
            let actual = match output {
                ReadOutput::Retained => self.owner.output.capacity(),
                ReadOutput::Selected => self.owner.selected.capacity(),
            };
            let excess = actual
                .checked_sub(needed)
                .ok_or(Resource::Accounting)
                .and_then(|count| {
                    count
                        .checked_mul(size_of::<Alias>())
                        .ok_or(Resource::Arithmetic)
                })
                .map_err(|error| {
                    self.failed(error);
                    Error::ResourceOverflow
                })?;
            self.reserve_storage(excess)
                .map_err(|_| Error::ResourceOverflow)?;
            Ok(())
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }
    fn read_field_capacity(&mut self, length: usize) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            if !self.owner.fields.is_empty() {
                return Err(Error::ReplayMismatch);
            }
            let old = self.owner.fields.capacity();
            if old >= length {
                return Ok(());
            }
            let growth = (length - old)
                .checked_mul(size_of::<u32>())
                .ok_or(Resource::Arithmetic)
                .map_err(|error| {
                    self.failed(error);
                    Error::ResourceOverflow
                })?;
            self.reserve_storage(growth)
                .map_err(|_| Error::ResourceOverflow)?;
            self.owner.fields.try_reserve_exact(length).map_err(|_| {
                self.failed(Resource::Allocation);
                Error::ResourceOverflow
            })?;
            let excess = self
                .owner
                .fields
                .capacity()
                .checked_sub(length)
                .ok_or(Resource::Accounting)
                .and_then(|count| {
                    count
                        .checked_mul(size_of::<u32>())
                        .ok_or(Resource::Arithmetic)
                })
                .map_err(|error| {
                    self.failed(error);
                    Error::ResourceOverflow
                })?;
            self.reserve_storage(excess)
                .map_err(|_| Error::ResourceOverflow)?;
            Ok(())
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }
}

pub(super) fn frame() -> std::result::Result<usize, Resource> {
    let rows = [
        size_of::<(
            Vec<Alias>,
            Vec<u32>,
            Option<&'static SemanticOperandV1>,
            Option<SemanticTransparentBorrowSiteV1>,
            ReadOutput,
            &ReadOutput,
        )>(),
        size_of::<(
            &'static SemanticOperandV1,
            &'static SemanticPlaceV1,
            &'static [SemanticProjectionV1],
            Path<'static>,
            Option<Path<'static>>,
            Source<'static>,
            Result<Source<'static>>,
            Option<Source<'static>>,
        )>(),
        size_of::<(
            Alias,
            Option<Alias>,
            Result<Alias>,
            &Alias,
            &mut Alias,
            Result<&Alias>,
            &mut Candidate,
            &mut Vec<Alias>,
            &Vec<Alias>,
            &mut Vec<u32>,
            &Vec<u32>,
            &[u32],
        )>(),
        size_of::<(
            std::vec::IntoIter<Alias>,
            Option<std::vec::IntoIter<Alias>>,
            &mut std::vec::IntoIter<Alias>,
            Option<&mut std::vec::IntoIter<Alias>>,
            Result<&mut std::vec::IntoIter<Alias>>,
            Option<Vec<Alias>>,
            Vec<Alias>,
        )>(),
        size_of::<(
            usize,
            usize,
            usize,
            usize,
            usize,
            u32,
            bool,
            bool,
            (usize, usize),
            SemanticTransparentBorrowSiteV1,
            Option<SemanticTransparentBorrowSiteV1>,
            (u32, u32),
        )>(),
        size_of::<(
            Result<()>,
            Result<bool>,
            Error,
            Resource,
            std::result::Result<(), Resource>,
            std::result::Result<usize, Resource>,
            Option<usize>,
            Result<usize>,
            std::result::Result<(), std::collections::TryReserveError>,
            std::collections::TryReserveError,
        )>(),
        size_of::<(
            &Budget<'static>,
            &mut Budget<'static>,
            &usize,
            &mut usize,
            &mut Option<Resource>,
            &mut Resource,
            &mut RetainedAliasSessionV1<'static, 'static, 'static>,
            &mut RetainedSharedObserverV1<'static>,
            (u32, Vec<Alias>),
        )>(),
        size_of::<(
            [usize; 8],
            std::array::IntoIter<usize, 8>,
            usize,
            usize,
            std::result::Result<usize, Resource>,
            Resource,
        )>(),
    ];
    rows.into_iter().try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })
}

#[cfg(test)]
#[path = "adapter_shared_primitive_read_retained_v1_tests.rs"]
mod tests;
