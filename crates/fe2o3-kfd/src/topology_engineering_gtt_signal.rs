//! Private observation contract for the dedicated two-rank dependency canary.
//! All CPU endpoints are covered because GTT placement is not reported.

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
struct CpuEndpoint {
    node_id: u32,
    name: String,
    properties: BTreeMap<String, u64>,
    device: u64,
    inode: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EngineeringGttSignalRoutesV1 {
    topology: TopologySnapshot,
    participants: [u32; 2],
    cpus: Vec<CpuEndpoint>,
    links: Vec<KfdTopologyLinkV1>,
}

fn admit_link_set(
    source: u32,
    cpus: &[u32],
    links: &[KfdTopologyLinkV1],
) -> Result<Vec<KfdTopologyLinkV1>, String> {
    if cpus.is_empty() || cpus.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("missing or ambiguous CPU endpoint roster".into());
    }
    cpus.iter()
        .map(|cpu| {
            let mut matches = links.iter().filter(|link| link.node_to == *cpu);
            let link = matches.next().ok_or("missing GPU-to-CPU signal route")?;
            if matches.next().is_some()
                || link.node_from != source
                || link.link_type != 2
                || link.flags != 1
            {
                return Err("ambiguous or unsupported GPU-to-CPU signal route".into());
            }
            Ok(*link)
        })
        .collect()
}

impl EngineeringGttSignalRoutesV1 {
    pub(crate) fn observe(
        retained: &HostTopologySnapshot,
        participants: [u32; 2],
    ) -> Result<Self, String> {
        Self::observe_topology(&retained.topology, participants)
    }

    pub(super) fn observe_topology(
        topology: &TopologySnapshot,
        participants: [u32; 2],
    ) -> Result<Self, String> {
        if participants[0] == participants[1] {
            return Err("signal route requires two distinct GPUs".into());
        }
        let root = &topology.provenance.root;
        let fresh = discover_topology_at_for_target(root, GfxTarget::Gfx950)
            .map_err(|error| format!("{error:?}"))?;
        if fresh != *topology {
            return Err("signal-route topology changed".into());
        }
        let nodes = root.join("nodes");
        let identity = ensure_directory(&nodes).map_err(|error| format!("{error:?}"))?;
        let mut cpus = Vec::new();
        let mut roster = BTreeSet::new();
        for (name, path) in
            read_directory(&nodes, MAX_TOPOLOGY_NODES).map_err(|error| format!("{error:?}"))?
        {
            let node_id = parse_node_id(&name).map_err(|error| format!("{error:?}"))?;
            if !roster.insert(node_id) {
                return Err("duplicate CPU/GPU node identity".into());
            }
            let before = ensure_directory(&path).map_err(|error| format!("{error:?}"))?;
            validate_node_entries(&path).map_err(|error| format!("{error:?}"))?;
            let gpu_id = read_scalar(&path.join("gpu_id")).map_err(|error| format!("{error:?}"))?;
            if gpu_id == 0 {
                let properties = parse_properties(&path.join("properties"))
                    .map_err(|error| format!("{error:?}"))?;
                if properties.get("cpu_cores_count").copied().unwrap_or(0) == 0
                    || properties.get("simd_count") != Some(&0)
                {
                    return Err("unclassified non-GPU topology endpoint".into());
                }
                cpus.push(CpuEndpoint {
                    node_id,
                    name: read_name(&path.join("name")).map_err(|error| format!("{error:?}"))?,
                    properties,
                    device: before.device,
                    inode: before.inode,
                });
            } else if !topology
                .gpu_nodes
                .iter()
                .any(|gpu| gpu.node_id == node_id && gpu.gpu_id == gpu_id)
            {
                return Err("unclassified GPU topology endpoint".into());
            }
            if ensure_directory(&path).map_err(|error| format!("{error:?}"))? != before {
                return Err("signal-route node replaced".into());
            }
        }
        if roster.len() != topology.observed_node_count
            || ensure_directory(&nodes).map_err(|error| format!("{error:?}"))? != identity
            || read_scalar(&root.join("generation_id")).map_err(|error| format!("{error:?}"))?
                != topology.provenance.generation
        {
            return Err("signal-route CPU roster changed".into());
        }
        cpus.sort_by_key(|cpu| cpu.node_id);
        let ids = cpus.iter().map(|cpu| cpu.node_id).collect::<Vec<_>>();
        let mut links = Vec::new();
        for gpu_id in participants {
            let gpu = topology
                .gpu_nodes
                .iter()
                .find(|gpu| gpu.gpu_id == u64::from(gpu_id))
                .ok_or("signal-route GPU absent")?;
            let all = gpu
                .io_links
                .iter()
                .chain(&gpu.p2p_links)
                .copied()
                .collect::<Vec<_>>();
            links.extend(admit_link_set(gpu.node_id, &ids, &all)?);
        }
        let after = discover_topology_at_for_target(root, GfxTarget::Gfx950)
            .map_err(|error| format!("{error:?}"))?;
        if after != *topology {
            return Err("signal-route topology changed after CPU read".into());
        }
        Ok(Self {
            topology: topology.clone(),
            participants,
            cpus,
            links,
        })
    }

    pub(crate) fn refresh(&self, retained: &HostTopologySnapshot) -> Result<(), String> {
        if Self::observe(retained, self.participants)? != *self {
            return Err("signal-route evidence changed".into());
        }
        Ok(())
    }

    pub(crate) fn report(&self) -> serde_json::Value {
        serde_json::json!({
            "schema": "gfx950-gtt-all-cpu-endpoints-v1",
            "topology_generation": self.topology.provenance.generation,
            "participants": self.participants,
            "allocation_flags": 0x8600_0002_u32,
            "physical_numa_placement": "unreported; all observed CPU endpoints required",
            "cpu_endpoints": self.cpus.iter().map(|cpu| serde_json::json!({
                "node_id": cpu.node_id, "name": cpu.name, "properties": cpu.properties,
                "device": cpu.device, "inode": cpu.inode,
            })).collect::<Vec<_>>(),
            "routes": self.links.iter().map(|link| serde_json::json!({
                "set": format!("{:?}", link.set), "index": link.index,
                "node_from": link.node_from, "node_to": link.node_to,
                "type": link.link_type, "flags": link.flags,
                "observation": format!("{link:?}"),
                "bandwidth_is_authority": false,
            })).collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(cpu: u32, set: KfdTopologyLinkSetV1) -> KfdTopologyLinkV1 {
        KfdTopologyLinkV1 {
            set,
            index: 0,
            link_type: 2,
            version_major: 0,
            version_minor: 0,
            node_from: 2,
            node_to: cpu,
            weight: 0,
            min_latency: 0,
            max_latency: 0,
            min_bandwidth: 0,
            max_bandwidth: 0,
            recommended_transfer_size: 0,
            flags: 1,
            recommended_sdma_engine_id_mask: 0,
        }
    }

    #[test]
    fn all_cpu_endpoints_require_one_positive_directional_path() {
        let routes = [
            link(0, KfdTopologyLinkSetV1::Io),
            link(1, KfdTopologyLinkSetV1::P2p),
        ];
        assert_eq!(admit_link_set(2, &[0, 1], &routes).unwrap(), routes);
        assert!(admit_link_set(2, &[0, 1], &routes[..1]).is_err());
        assert!(admit_link_set(2, &[], &routes).is_err());
        assert!(admit_link_set(2, &[0, 0], &routes).is_err());
        let mut duplicate = routes.to_vec();
        duplicate.push(link(0, KfdTopologyLinkSetV1::P2p));
        assert!(admit_link_set(2, &[0, 1], &duplicate).is_err());
    }

    #[test]
    fn flags_type_and_direction_are_exact_not_compute_peer_policy() {
        for flags in [0, 2, 3, 5, 9, 13, 17, u32::MAX] {
            let mut route = link(0, KfdTopologyLinkSetV1::Io);
            route.flags = flags;
            assert!(admit_link_set(2, &[0], &[route]).is_err());
        }
        let mut route = link(0, KfdTopologyLinkSetV1::Io);
        route.link_type = 11;
        assert!(admit_link_set(2, &[0], &[route]).is_err());
        route.link_type = 2;
        route.node_from = 3;
        assert!(admit_link_set(2, &[0], &[route]).is_err());
    }
}
