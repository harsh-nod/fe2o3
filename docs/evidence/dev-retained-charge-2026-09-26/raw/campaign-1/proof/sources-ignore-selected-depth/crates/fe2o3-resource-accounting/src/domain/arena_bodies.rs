// Production execution with ghost-only hooks in the pinned Verus instantiation.
macro_rules! resource_domain_node_body_v1 {
    ($nodes:ident, $key:ident) => {{
        if $key.slot >= $nodes.len() { return None; }
        match &$nodes[$key.slot] {
            Some(node) => if node.key.slot == $key.slot && node.key.generation == $key.generation { Some(node) } else { None },
            None => None,
        }
    }};
}

macro_rules! resource_domain_path_extract_body_v1 {
    ($syntax:ident, $nodes:ident, $profile:ident, $leaf:ident, $path:ident,
     $next:ident, $depth:ident, [$($annotations:tt)*], [$($staged:tt)*], [$($advance:tt)*], [$($finish:tt)*]) => {
        $syntax!({
            if $profile != MAX_RESOURCE_DOMAIN_DEPTH_V1 && $profile != MAX_RESOURCE_CLASS_DOMAIN_DEPTH_V1 {
                return None;
            }
            let mut $path = [ROOT; MAX_RESOURCE_CLASS_DOMAIN_DEPTH_V1];
            let mut $next = Some($leaf);
            let mut $depth = 0usize;
            while $next.is_some()
                $($annotations)*
            {
                let key = match $next { Some(key) => key, None => return None };
                if $depth == $path.len() { return None; }
                $($staged)*
                $path[$depth] = key;
                $depth += 1;
                $next = match domain_node_v1($nodes, key) {
                    Some(node) => node.parent,
                    None => return None,
                };
                $($advance)*
            }
            if $path[$depth - 1].slot != ROOT.slot || $path[$depth - 1].generation != ROOT.generation { return None; }
            $($finish)*
            Some(($path, $depth))
        })
    };
}

macro_rules! resource_domain_facts_body_v1 {
    ($syntax:ident, $nodes:ident, $path:ident, $depth:ident, $facts:ident,
     $index:ident, [$($start:tt)*], [$($annotations:tt)*], [$($advance:tt)*]) => {
        $syntax!({
            if $depth == 0 || $depth > MAX_RESOURCE_CLASS_DOMAIN_DEPTH_V1 { return None; }
            let empty = R75ResourceDomainFactsV1 {
                used: ResourceVectorV1::ZERO,
                capacity: ResourceVectorV1::ZERO,
                counts: [0; 3],
                record_limit: 0,
            };
            let mut $facts = [empty; R75_RESOURCE_DOMAIN_LEVELS_V1];
            let mut $index = 0usize;
            $($start)*
            while $index < $depth
                $($annotations)*
            {
                let node = match domain_node_v1($nodes, $path[$index]) {
                    Some(node) => node,
                    None => return None,
                };
                $facts[$index] = R75ResourceDomainFactsV1 {
                    used: node.used,
                    capacity: node.capacity,
                    counts: node.counts,
                    record_limit: node.record_limit,
                };
                $index += 1;
                $($advance)*
            }
            Some($facts)
        })
    };
}
