        let (head, count, unknown) = match shared_retained_header_v1($journal, $writer, true) {
            Ok(header) => header,
            Err(error) => return Err(error),
        };
        let result = shared_retained_chain_v1($journal, $writer, head, count);
        if result.is_err() {
            return result;
        }
        if !unknown {
            retained_indexed_access_v1($journal);
            $journal.writers[$writer.slot] = Some(WriterEntryV1::Pending {
                key: $writer.key,
                head,
                count,
            });
        }
        result
