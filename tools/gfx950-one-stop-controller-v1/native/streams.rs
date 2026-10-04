//! Two bounded readers. Caps are charged before appending/queueing bytes.
use fe2o3_private_one_stop_protocol::{MAX_BYTES, MAX_LINE, MAX_RECORDS, Refusal};
use std::{
    io::{BufRead, BufReader, Read},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
};
#[derive(Clone, Copy)]
pub(super) enum Stream {
    Out,
    Err,
}
pub(super) enum Item {
    Line(Stream, Vec<u8>),
    Eof(Stream),
    Failed,
}
#[derive(Default)]
pub(super) struct ReadBudget {
    bytes: AtomicUsize,
    records: AtomicUsize,
}
fn charge(counter: &AtomicUsize, n: usize, cap: usize) -> Result<(), Refusal> {
    counter
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
            v.checked_add(n).filter(|v| *v <= cap)
        })
        .map(|_| ())
        .map_err(|_| Refusal::Bound)
}
pub(super) fn line(
    reader: &mut impl BufRead,
    budget: &ReadBudget,
) -> Result<Option<Vec<u8>>, Refusal> {
    let mut out = Vec::new();
    out.try_reserve_exact(MAX_LINE)
        .map_err(|_| Refusal::Bound)?;
    loop {
        let block = reader.fill_buf().map_err(|_| Refusal::Incomplete)?;
        if block.is_empty() {
            if out.is_empty() {
                return Ok(None);
            }
            return Err(Refusal::Bound);
        }
        let n = block
            .iter()
            .position(|b| *b == b'\n')
            .map_or(block.len(), |x| x + 1);
        if out.len().checked_add(n).is_none_or(|v| v > MAX_LINE) {
            return Err(Refusal::Bound);
        }
        charge(&budget.bytes, n, MAX_BYTES)?;
        let final_line = block[n - 1] == b'\n';
        out.extend_from_slice(&block[..n]);
        reader.consume(n);
        if final_line {
            charge(&budget.records, 1, MAX_RECORDS)?;
            return Ok(Some(out));
        }
    }
}
pub(super) fn start(
    input: impl Read + Send + 'static,
    which: Stream,
    tx: mpsc::SyncSender<Item>,
    budget: Arc<ReadBudget>,
) -> Result<JoinHandle<()>, Refusal> {
    thread::Builder::new()
        .name("one-stop-mi-reader".into())
        .spawn(move || {
            let mut input = BufReader::with_capacity(8192, input);
            loop {
                let item = match line(&mut input, &budget) {
                    Ok(Some(x)) => Item::Line(which, x),
                    Ok(None) => Item::Eof(which),
                    Err(_) => Item::Failed,
                };
                let terminal = !matches!(item, Item::Line(..));
                if tx.send(item).is_err() || terminal {
                    break;
                }
            }
        })
        .map_err(|_| Refusal::Incomplete)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    #[test]
    fn exact_line_cap_and_unterminated_refusal() {
        let mut x = vec![b'a'; MAX_LINE];
        x[MAX_LINE - 1] = b'\n';
        assert_eq!(
            line(&mut Cursor::new(&x), &ReadBudget::default())
                .unwrap()
                .unwrap(),
            x
        );
        x.push(b'\n');
        x[MAX_LINE - 1] = b'a';
        assert!(line(&mut Cursor::new(x), &ReadBudget::default()).is_err());
        assert!(line(&mut Cursor::new(b"partial"), &ReadBudget::default()).is_err());
    }
    #[test]
    fn cumulative_bytes_charged_before_copy() {
        let b = ReadBudget::default();
        b.bytes.store(MAX_BYTES - 2, Ordering::Release);
        line(&mut Cursor::new(b"x\n"), &b).unwrap();
        assert!(line(&mut Cursor::new(b"\n"), &b).is_err());
        assert_eq!(b.bytes.load(Ordering::Acquire), MAX_BYTES);
    }
    #[test]
    fn two_readers_share_record_and_byte_census() {
        let b = ReadBudget::default();
        b.records.store(MAX_RECORDS - 1, Ordering::Release);
        line(&mut Cursor::new(b"a\n"), &b).unwrap();
        assert!(line(&mut Cursor::new(b"b\n"), &b).is_err());
        assert_eq!(b.records.load(Ordering::Acquire), MAX_RECORDS);
    }
    #[test]
    fn arithmetic_overflow_does_not_reset_prior_charge() {
        let c = AtomicUsize::new(3);
        assert!(charge(&c, usize::MAX, usize::MAX).is_err());
        assert_eq!(c.load(Ordering::Acquire), 3);
    }
}
