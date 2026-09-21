// Shared executable scratch scan/staging. Loop annotations carry no runtime code.
macro_rules! settlement_scratch_scan_body {
    ($syntax:ident, $journal:ident, $count:ident, $index:ident, [$($annotations:tt)*]) => {
        $syntax!({
            let mut $index = 0usize;
            while $index < $count
                $($annotations)*
            {
                settlement_scratch_access_v1($journal);
                if $journal.scratch[$index].is_some() {
                    return Err(ReadErrorV1::InvalidState);
                }
                $index += 1;
            }
            Ok(())
        })
    };
}

macro_rules! settlement_scratch_stage_body {
    ($syntax:ident, $journal:ident, $initial:ident, $count:ident, $head:ident,
     $index:ident, [$($annotations:tt)*]) => {
        $syntax!({
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
                    attempt_epoch: 0,
                });
                $head = member.next;
                $index += 1;
            }
        })
    };
}
