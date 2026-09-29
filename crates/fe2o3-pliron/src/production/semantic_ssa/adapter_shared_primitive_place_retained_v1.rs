//! Inert source-bound place invalidation within the retained alias session.
//! No observer, rvalue/install producer, actual SSA admission or route activation.
use super::*;
use fe2o3_mir_model::semantic_mir_v1 as model;

impl<'s, 'a, 'w> RetainedAliasSessionV1<'s, 'a, 'w> {
    pub(super) fn path(&mut self, place: &'a SemanticPlaceV1) -> Result<Option<Path<'a>>> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            self.owner.place = Some(place);
            self.owner.path = None;

            alias_work(self.budget, &mut self.owner.failure, 4)?;
            let source = self.owner.source.ok_or(Error::ReplayMismatch)?;
            let Some(local) = source.function.locals().get(place.local().index() as usize) else {
                return Ok(None);
            };
            let mut ty = local.ty();
            for (index, projection) in place.projections().iter().enumerate() {
                alias_work(self.budget, &mut self.owner.failure, 12)?;
                let Some(declaration) = source.types.get(ty.index() as usize) else {
                    return Ok(None);
                };
                match (projection.kind(), declaration.shape()) {
                    (
                        SemanticProjectionKindV1::Field(field),
                        SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields),
                    ) => {
                        let Some(&field_type) = fields.fields().get(field as usize) else {
                            return Ok(None);
                        };
                        if field_type != projection.result_type() {
                            return Ok(None);
                        }
                        ty = field_type;
                    }
                    (
                        SemanticProjectionKindV1::Dereference,
                        SemanticTypeShapeV1::Pointer(pointer),
                    ) if index + 1 == place.projections().len()
                        && pointer.kind() == SemanticPointerKindV1::Reference
                        && pointer.mutability() == SemanticMutabilityV1::Immutable
                        && pointer.metadata() == SemanticPointerMetadataV1::None
                        && pointer.pointee() == projection.result_type()
                        && primitive(source.types, pointer.pointee())
                        && place.ty() == pointer.pointee() =>
                    {
                        return Ok(Some(Path {
                            fields: &place.projections()[..index],
                            dereference: true,
                            selected: ty,
                        }));
                    }
                    _ => return Ok(None),
                }
            }
            Ok((ty == place.ty()).then_some(Path {
                fields: place.projections(),
                dereference: false,
                selected: ty,
            }))
        })();
        if let Ok(path) = &result {
            self.owner.path = *path;
        }
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    pub(super) fn matches(&mut self, slot: Slot, prefix: &[SemanticProjectionV1]) -> Result<bool> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            let fields = match slot {
                Slot::Current => self.owner.current.as_ref(),
                Slot::Held(index) => self
                    .owner
                    .removed
                    .as_ref()
                    .and_then(|held| held.aliases.get(index)),
            }
            .ok_or(Error::ReplayMismatch)?
            .fields
            .as_slice();

            alias_work(
                self.budget,
                &mut self.owner.failure,
                (1 + fields.len().min(prefix.len()))
                    .checked_mul(4)
                    .ok_or(Error::ResourceOverflow)?,
            )?;
            Ok(fields.len() >= prefix.len()
                && fields.iter().zip(prefix).all(|(&field, projection)| {
                    projection.kind() == SemanticProjectionKindV1::Field(field)
                }))
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    pub(super) fn deinitialize(&mut self, place: &'a SemanticPlaceV1) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            if self.owner.removed.is_some()
                || self.owner.drain.is_some()
                || self.owner.current.is_some()
                || !self.owner.output.is_empty()
            {
                return Err(Error::ReplayMismatch);
            }
            self.owner.place = Some(place);
            self.owner.path = None;

            let local = place.local().index();
            self.source_write(local)?;
            let Some(path) = self.path(place)?.filter(|path| !path.dereference) else {
                return self.poison_holder(local);
            };
            self.tree()?;
            self.owner.removed = self.owner.data.holders.remove(&local).map(|aliases| Held {
                local,
                aliases,
                cursor: 0,
            });
            if self.owner.removed.is_some() {
                while {
                    let held = self.owner.removed.as_ref().ok_or(Error::ReplayMismatch)?;
                    held.cursor < held.aliases.len()
                } {
                    let index = self
                        .owner
                        .removed
                        .as_ref()
                        .ok_or(Error::ReplayMismatch)?
                        .cursor;
                    if self.matches(Slot::Held(index), path.fields)? {
                        self.deactivate(Slot::Held(index))?;
                    }
                    self.owner
                        .removed
                        .as_mut()
                        .ok_or(Error::ReplayMismatch)?
                        .cursor += 1;
                }
                self.tree()?;
                self.reserve_storage(size_of::<(u32, Vec<Alias>)>())
                    .map_err(|_| Error::ResourceOverflow)?;
                let held = self.owner.removed.take().ok_or(Error::ReplayMismatch)?;
                self.owner.data.holders.insert(held.local, held.aliases);
            }
            Ok(())
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    pub(super) fn write_place(&mut self, place: &'a SemanticPlaceV1) -> Result<Option<Path<'a>>> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            if self.owner.removed.is_some()
                || self.owner.drain.is_some()
                || self.owner.current.is_some()
                || !self.owner.output.is_empty()
            {
                return Err(Error::ReplayMismatch);
            }
            self.owner.place = Some(place);
            self.owner.path = None;

            let local = place.local().index();
            self.source_write(local)?;
            let Some(path) = self.path(place)? else {
                self.poison_holder(local)?;
                return Ok(None);
            };
            if path.dereference {
                self.poison_holder(local)?;
                return Ok(None);
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
                        self.release_current()?;
                    } else {
                        self.output_capacity()?;
                        let alias = self.owner.current.take().ok_or(Error::ReplayMismatch)?;
                        self.owner.output.push(alias);
                    }
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
            Ok(Some(path))
        })();
        if result.is_err() {
            self.owner.phase = Phase::Terminal;
        }
        result
    }

    fn output_capacity(&mut self) -> Result<()> {
        if self.owner.phase != Phase::Active {
            return Err(Error::ReplayMismatch);
        }
        let result = (|| {
            if self.owner.output.len() < self.owner.output.capacity() {
                return Ok(());
            }
            let old = self.owner.output.capacity();
            let needed = self
                .owner
                .output
                .len()
                .checked_add(1)
                .ok_or(Resource::Arithmetic)
                .map_err(|error| {
                    self.failed(error);
                    Error::ResourceOverflow
                })?;
            let growth = needed
                .checked_sub(old)
                .and_then(|value| value.checked_mul(size_of::<Alias>()))
                .ok_or(Resource::Arithmetic)
                .map_err(|error| {
                    self.failed(error);
                    Error::ResourceOverflow
                })?;
            self.reserve_storage(growth)
                .map_err(|_| Error::ResourceOverflow)?;
            self.owner.output.try_reserve_exact(1).map_err(|_| {
                self.failed(Resource::Allocation);
                Error::ResourceOverflow
            })?;
            let excess = self
                .owner
                .output
                .capacity()
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
}

pub(super) fn frame() -> std::result::Result<usize, Resource> {
    // Typed reached-carrier envelope, not allocator internals or an RSS claim.
    let rows = [
        size_of::<(
            Vec<Alias>,
            &Vec<Alias>,
            &mut Vec<Alias>,
            Option<&'static SemanticPlaceV1>,
            Option<Path<'static>>,
            Path<'static>,
        )>(),
        size_of::<(
            Source<'static>,
            Option<Source<'static>>,
            Result<Source<'static>>,
            &'static SemanticPlaceV1,
            &'static [SemanticProjectionV1],
            &'static SemanticFunctionDeclV1,
            &'static [SemanticTypeDeclV1],
        )>(),
        size_of::<(
            model::SemanticLocalIdV1,
            model::SemanticTypeIdV1,
            u32,
            usize,
            bool,
            &[model::SemanticLocalDeclV1],
            &model::SemanticLocalDeclV1,
            Option<&model::SemanticLocalDeclV1>,
            &SemanticTypeDeclV1,
            Option<&SemanticTypeDeclV1>,
        )>(),
        size_of::<(
            model::SemanticProjectionV1,
            &model::SemanticProjectionV1,
            model::SemanticProjectionKindV1,
            &model::SemanticTypeShapeV1,
            &model::SemanticAggregateTypeV1,
            &model::SemanticPointerTypeV1,
            model::SemanticPointerKindV1,
            model::SemanticMutabilityV1,
            model::SemanticPointerMetadataV1,
            &[model::SemanticTypeIdV1],
            &model::SemanticTypeIdV1,
            Option<&model::SemanticTypeIdV1>,
        )>(),
        size_of::<(
            std::slice::Iter<'static, SemanticProjectionV1>,
            std::iter::Enumerate<std::slice::Iter<'static, SemanticProjectionV1>>,
            (usize, &'static SemanticProjectionV1),
            Option<(usize, &'static SemanticProjectionV1)>,
            std::slice::Iter<'static, u32>,
            std::iter::Zip<
                std::slice::Iter<'static, u32>,
                std::slice::Iter<'static, SemanticProjectionV1>,
            >,
            (&'static u32, &'static SemanticProjectionV1),
            &'static u32,
        )>(),
        size_of::<(
            Option<Path<'static>>,
            Result<Option<Path<'static>>>,
            &Result<Option<Path<'static>>>,
            &Option<Path<'static>>,
            &Path<'static>,
            Result<bool>,
            Result<()>,
            Option<usize>,
            Result<usize>,
            std::result::Result<usize, Resource>,
        )>(),
        size_of::<(
            Slot,
            Result<&Alias>,
            Result<&mut Alias>,
            &[u32],
            &Held,
            &mut Held,
            Option<&Held>,
            Option<&mut Held>,
            Result<&Held>,
            Result<&mut Held>,
            Result<Held>,
        )>(),
        size_of::<(
            std::vec::IntoIter<Alias>,
            Option<std::vec::IntoIter<Alias>>,
            &mut std::vec::IntoIter<Alias>,
            Option<&mut std::vec::IntoIter<Alias>>,
            Option<Alias>,
            Alias,
            Result<Alias>,
            Result<&mut std::vec::IntoIter<Alias>>,
            Option<Vec<Alias>>,
            Vec<Alias>,
            (u32, Vec<Alias>),
            &mut Vec<Alias>,
        )>(),
        size_of::<(
            usize,
            usize,
            usize,
            usize,
            usize,
            u32,
            bool,
            std::result::Result<(), std::collections::TryReserveError>,
            std::collections::TryReserveError,
            std::result::Result<(), Resource>,
            Resource,
            Error,
            &mut Option<Resource>,
            &mut RetainedAliasSessionV1<'static, 'static, 'static>,
        )>(),
        size_of::<(
            [usize; 10],
            std::array::IntoIter<usize, 10>,
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
#[path = "adapter_shared_primitive_place_retained_v1_tests.rs"]
mod tests;
