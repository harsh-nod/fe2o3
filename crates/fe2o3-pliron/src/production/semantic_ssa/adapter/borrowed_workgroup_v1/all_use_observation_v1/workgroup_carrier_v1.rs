//! Rejection triage over retained shared Borrows and already-built route maps.
use super::*;

const MAX_BORROWS: usize = 32;

fn spend(remaining: &mut usize, amount: usize) -> bool {
    if let Some(left) = remaining.checked_sub(amount) {
        *remaining = left;
        true
    } else {
        *remaining = 0;
        false
    }
}

pub(super) fn write(
    out: &mut impl Write,
    view: &SemanticExpandedRootV1,
    candidates: &[SemanticBorrowCandidateV1],
    routes: &math_capture_flow_v1::Routes<'_>,
    references: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    remaining: &mut usize,
) -> io::Result<()> {
    // Reserve part of the unchanged enclosing output buffer for the existing
    // component/invalidation records. This is a prefix cap, not another buffer.
    let mut prefix = Prefix {
        out,
        remaining: MAX_OUTPUT / 2,
        truncated: false,
    };
    let result = write_inner(&mut prefix, view, candidates, routes, references, remaining);
    if prefix.truncated {
        writeln!(
            prefix.out,
            "\nworkgroup-borrow-census-output_truncated=true scan_complete=false diagnostic_remaining={remaining}"
        )
    } else {
        result
    }
}

fn write_inner(
    out: &mut impl Write,
    view: &SemanticExpandedRootV1,
    candidates: &[SemanticBorrowCandidateV1],
    routes: &math_capture_flow_v1::Routes<'_>,
    references: &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    remaining: &mut usize,
) -> io::Result<()> {
    let same_body = routes.observation_body_matches(view.body());
    writeln!(
        out,
        "workgroup-borrow-census same_body={same_body} diagnostic_only=true"
    )?;
    if !same_body {
        return Ok(());
    }
    let mut records = 0;
    let mut complete = true;
    'blocks: for (block, body) in view.body().blocks().iter().enumerate() {
        if !spend(remaining, 1) {
            complete = false;
            break;
        }
        for (statement, node) in body.statements().iter().enumerate() {
            if !spend(remaining, 1) {
                complete = false;
                break 'blocks;
            }
            let SemanticStatementKindV1::Assign(a) = node.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            } = a.value().kind()
            else {
                continue;
            };
            if records == MAX_BORROWS {
                complete = false;
                break 'blocks;
            }
            // All candidate comparisons are charged before the scan, including
            // nodes not selected by the Workgroup owned-type trace filter.
            let lookup = 1 + (usize::BITS - references.len().leading_zeros()) as usize;
            let Some(cost) = candidates.len().checked_add(24 + lookup) else {
                complete = false;
                break 'blocks;
            };
            if !spend(remaining, cost) {
                complete = false;
                break 'blocks;
            }
            let site = Site {
                block: block as u32,
                statement: statement as u32,
            };
            let mut count = 0;
            let mut first = None;
            for (index, candidate) in candidates.iter().enumerate() {
                if candidate.site == site {
                    count += 1;
                    first = first.or(Some(index));
                }
            }
            records += 1;
            let origin = view.block_origins().get(block);
            let local_origin = view.local_origins().get(place.local().index() as usize);
            let source_ty = view
                .body()
                .locals()
                .get(place.local().index() as usize)
                .map(|l| l.ty());
            writeln!(
                out,
                "shared-borrow site=({block},{statement}) destination={} reference={} result={} source={} source_ty={source_ty:?} place_ty={} destination_projections={} place_projections={} prefix={:?} truncated={} candidate_count={count} first_candidate={first:?} direct_reference_owner={:?}",
                a.destination().local().index(),
                a.destination().ty().index(),
                a.value().result_type().index(),
                place.local().index(),
                place.ty().index(),
                a.destination().projections().len(),
                place.projections().len(),
                &place.projections()[..place.projections().len().min(4)],
                place.projections().len() > 4,
                references.get(&a.destination().ty()).map(|ty| ty.index())
            )?;
            writeln!(
                out,
                "shared-borrow-source block={:?} statement={:?} local={:?}",
                origin.map(|o| (
                    o.instance().index(),
                    o.function().index(),
                    o.block().index()
                )),
                origin.and_then(|o| o.statements().get(statement)),
                local_origin.map(|o| (
                    o.instance().index(),
                    o.function().index(),
                    o.local().index()
                ))
            )?;
            let types = [Some(a.destination().ty()), source_ty, Some(place.ty())];
            for (index, ty) in types.iter().copied().enumerate() {
                if types[..index].contains(&ty) {
                    continue;
                }
                if let Some(ty) = ty {
                    if !routes.write_type_observation(out, ty, remaining)? {
                        complete = false;
                        break 'blocks;
                    }
                }
            }
        }
    }
    writeln!(
        out,
        "workgroup-borrow-census-end records={records} scan_complete={complete} truncated={} diagnostic_remaining={remaining}",
        !complete
    )
}

struct Prefix<'a, W> {
    out: &'a mut W,
    remaining: usize,
    truncated: bool,
}

impl<W: Write> Write for Prefix<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.remaining {
            self.truncated = true;
            return Err(io::ErrorKind::WriteZero.into());
        }
        let written = self.out.write(bytes)?;
        self.remaining -= written;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.out.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn census_prefix_cannot_consume_the_reserved_component_output() {
        let mut out = Output {
            bytes: [0; MAX_OUTPUT],
            len: 0,
            truncated: false,
        };
        {
            let mut prefix = Prefix {
                out: &mut out,
                remaining: MAX_OUTPUT / 2,
                truncated: false,
            };
            prefix.write_all(&[b'x'; MAX_OUTPUT / 2]).unwrap();
            assert!(prefix.write_all(b"y").is_err());
            assert!(prefix.truncated);
            assert_eq!(prefix.remaining, 0);
        }
        assert_eq!(out.len, MAX_OUTPUT / 2);
        assert!(!out.truncated);
        out.write_all(b"existing-component-record").unwrap();
        assert!(!out.truncated);
    }

    #[test]
    fn census_debits_are_checked_and_output_errors_are_not_success() {
        let mut remaining = 3;
        assert!(spend(&mut remaining, 3));
        assert_eq!(remaining, 0);
        assert!(!spend(&mut remaining, 1));
        let mut remaining = 2;
        assert!(!spend(&mut remaining, 3));
        assert_eq!(remaining, 0);
        struct Fails;
        impl Write for Fails {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut sink = Fails;
        let mut prefix = Prefix {
            out: &mut sink,
            remaining: 16,
            truncated: false,
        };
        assert!(prefix.write_all(b"observed").is_err());
        assert!(!prefix.truncated);
        assert_eq!(prefix.remaining, 16);
    }
}
