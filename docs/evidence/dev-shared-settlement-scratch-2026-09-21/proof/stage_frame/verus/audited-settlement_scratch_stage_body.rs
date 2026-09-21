            let mut $head = $initial;
            let mut $index = 0usize;
            while $index < $count
                $($annotations)*
            {
                let slot = $head.expect("validated complete retained chain");
                settlement_scratch_access_v1($journal);
                let member = $journal.members[slot].expect("validated retained member");
                settlement_scratch_access_v1($journal);
                $journal.scratch[$index] = Some(BeginMemberPlanV1 {
                    member_slot: slot,
                    allocation: member.allocation,
                    prior_lineage: member.prior_lineage,
                    attempt_epoch: member.attempt_epoch,
                });
                $head = member.next;
                $index += 1;
            }
            $journal.reserved_count = 0;
