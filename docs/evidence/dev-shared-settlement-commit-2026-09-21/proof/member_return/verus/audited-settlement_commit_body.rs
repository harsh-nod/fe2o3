            let mut $index = 0usize;
            while $index < $count
                $($annotations)*
            {
                settlement_commit_access_v1($journal);
                let plan = $journal.scratch[$index]
                    .take()
                    .expect("complete settlement plan");
                settlement_commit_access_v1($journal);
                {
                    let allocation = $journal.allocations[plan.allocation.slot]
                        .as_mut()
                        .expect("validated exact retained allocation");
                    if $success {
                        allocation.content_lineage = plan.attempt_epoch;
                    }
                    allocation.pending_member = None;
                }
                settlement_commit_access_v1($journal);
                $journal.members[plan.member_slot] = None;
                settlement_commit_access_v1($journal);
                let _ = plan.member_slot;
                $index += 1;
            }
            settlement_commit_access_v1($journal);
            $journal.writers[$writer.slot] = None;
            settlement_commit_access_v1($journal);
            $journal.free.push($writer.slot);
