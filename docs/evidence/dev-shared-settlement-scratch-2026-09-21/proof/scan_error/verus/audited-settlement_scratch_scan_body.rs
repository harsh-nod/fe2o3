            let mut $index = 0usize;
            while $index < $count
                $($annotations)*
            {
                settlement_scratch_access_v1($journal);
                if $journal.scratch[$index].is_some() {
                    return Err(ReadErrorV1::InvalidReference);
                }
                $index += 1;
            }
            Ok(())
