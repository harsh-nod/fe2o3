            let mut $head = $initial;
            if $count > $journal.allocation_capacity || ($count == 0) != $head.is_none() {
                return Err(ReadErrorV1::InvalidState);
            }
            let mut $previous = None;
            let mut $index = 0usize;
            while $index < $count
                $($annotations)*
            {
                let member = match shared_retained_member_v1($journal, $writer, $head, $previous) {
                    Ok(member) => member,
                    Err(error) => return Err(error),
                };
                $previous = Some(member.allocation.key);
                $head = member.next;
                $index += 1;
            }
            if false {
                return Err(ReadErrorV1::InvalidState);
            }
            Ok(())
