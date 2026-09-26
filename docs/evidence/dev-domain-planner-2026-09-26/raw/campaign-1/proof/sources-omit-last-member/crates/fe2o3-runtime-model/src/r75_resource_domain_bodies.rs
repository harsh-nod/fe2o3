// The same bodies are compiled by Rust and Verus. Hooks are proof-only.
macro_rules! resource_domain_records_body_v1 {
    ($facts:ident, $count:ident, $owner:ident) => {{
        let occupied = match $facts.counts[0].checked_add($facts.counts[1]) {
            Some(value) => value,
            None => return Err(R75ResourceDomainErrorV1::Invariant),
        };
        let occupied = match occupied.checked_add($facts.counts[2]) {
            Some(value) => value,
            None => return Err(R75ResourceDomainErrorV1::Invariant),
        };
        let free = match $facts.record_limit.checked_sub(occupied) {
            Some(value) => value,
            None => return Err(R75ResourceDomainErrorV1::Invariant),
        };
        if $count == 0 || $count > 65_536 {
            return Err(R75ResourceDomainErrorV1::InvalidMemberCount);
        }
        if $count > free { return Err(R75ResourceDomainErrorV1::RecordCapacity); }
        if $owner == 0 { return Err(R75ResourceDomainErrorV1::GenerationExhausted); }
        let next_owner = match $owner.checked_add($count as u64) {
            Some(value) => value,
            None => return Err(R75ResourceDomainErrorV1::GenerationExhausted),
        };
        Ok(($facts.counts[0] + $count, next_owner))
    }};
}

macro_rules! resource_domain_leaf_body_v1 {
    ($syntax:ident, $used:ident, $charges:ident, $capacity:ident,
     $next:ident, $member:ident, [$($annotations:tt)*], [$($reject:tt)*], [$($advance:tt)*]) => {
        $syntax!({
            let mut $next = $used;
            let mut $member = 0usize;
            while $member + 1 < $charges.len()
                $($annotations)*
            {
                $next = match r67_resource_reserve_v1($next, $charges[$member], $capacity) {
                    Some(value) => value,
                    None => { $($reject)* return Err(R75ResourceDomainErrorV1::Capacity); },
                };
                $member += 1;
                $($advance)*
            }
            Ok($next)
        })
    };
}

macro_rules! resource_domain_path_body_v1 {
    ($syntax:ident, $facts:ident, $depth:ident, $profile:ident, $charges:ident, $owner:ident,
     $plan:ident, $total:ident, $level:ident, $next:ident, [$($annotations:tt)*],
     [$($record_error:tt)*], [$($leaf_error:tt)*], [$($leaf_admitted:tt)*], [$($leaf_ready:tt)*],
     [$($parent_error:tt)*], [$($staged:tt)*], [$($advance:tt)*]) => {
        $syntax!({
            if ($profile != 3 && $profile != 4) || $depth == 0 || $depth > $profile {
                return Err(R75ResourceDomainErrorV1::Invariant);
            }
            if $owner == 0 { return Err(R75ResourceDomainErrorV1::Invariant); }
            let mut $plan = R75ResourceDomainPlanV1 {
                next_used: [R67ResourceVectorV1::ZERO; R75_RESOURCE_DOMAIN_LEVELS_V1],
                next_reserved: [0; R75_RESOURCE_DOMAIN_LEVELS_V1],
                next_owner: $owner,
            };
            let mut $total = R67ResourceVectorV1::ZERO;
            let mut $level = 0usize;
            while $level < $depth
                $($annotations)*
            {
                let fact = $facts[$level];
                let (reserved, next_owner) = match resource_domain_records_v1(fact, $charges.len(), $owner) {
                    Ok(value) => value,
                    Err(error) => { $($record_error)* return Err(error); },
                };
                let $next = if $level == 0 {
                    let next = match resource_domain_leaf_v1(fact.used, $charges, fact.capacity) {
                        Ok(value) => value,
                        Err(error) => { $($leaf_error)* return Err(error); },
                    };
                    $($leaf_admitted)*
                    if $depth > 1 {
                        $total = if $charges.len() == 1 { $charges[0] } else {
                            match r67_resource_release_v1(next, fact.used) {
                                Some(value) => value,
                                None => return Err(R75ResourceDomainErrorV1::Invariant),
                            }
                        };
                    }
                    $($leaf_ready)*
                    next
                } else {
                    match r67_resource_reserve_v1(fact.used, $total, fact.capacity) {
                        Some(value) => value,
                        None => { $($parent_error)* return Err(R75ResourceDomainErrorV1::Capacity); },
                    }
                };
                $($staged)*
                $plan.next_used[$level] = $next;
                $plan.next_reserved[$level] = reserved;
                $plan.next_owner = next_owner;
                $level += 1;
                $($advance)*
            }
            Ok($plan)
        })
    };
}
