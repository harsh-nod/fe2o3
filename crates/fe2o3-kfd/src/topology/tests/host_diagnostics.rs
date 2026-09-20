use super::*;
use crate::currentness_diagnostic::{Disabled, Enabled, Mode};
use std::cell::RefCell;
use std::os::unix::fs::symlink;
use std::panic::{AssertUnwindSafe, catch_unwind};

type Event = (&'static str, PathBuf, usize);
type Hook = (usize, Box<dyn FnOnce()>);

#[derive(Default)]
struct Trace {
    events: Vec<Event>,
    hook: Option<Hook>,
}

thread_local! { static TRACE: RefCell<Option<Trace>> = const { RefCell::new(None) }; }

// This records selected Rust I/O boundaries, not kernel syscall counts.
pub(in crate::topology) fn io(operation: &'static str, path: &Path, bound: usize) {
    let hook = TRACE.with_borrow_mut(|active| {
        let trace = active.as_mut()?;
        trace.events.push((operation, path.to_path_buf(), bound));
        if trace
            .hook
            .as_ref()
            .is_some_and(|(at, _)| *at == trace.events.len() - 1)
        {
            trace.hook.take().map(|(_, hook)| hook)
        } else {
            None
        }
    });
    if let Some(hook) = hook {
        hook();
    }
}

struct TraceScope;

impl TraceScope {
    fn new(hook: Option<Hook>) -> Self {
        TRACE.with_borrow_mut(|active| {
            assert!(active.is_none());
            *active = Some(Trace {
                events: Vec::new(),
                hook,
            });
        });
        Self
    }

    fn events(&self) -> Vec<Event> {
        TRACE.with_borrow(|active| active.as_ref().unwrap().events.clone())
    }
}

impl Drop for TraceScope {
    fn drop(&mut self) {
        TRACE.with_borrow_mut(|active| *active = None);
    }
}

struct HostFixture {
    topology: Fixture,
    render: RenderFixture,
}

impl HostFixture {
    fn valid(gpus: u32) -> Self {
        let topology = Fixture::valid(gpus);
        let render = RenderFixture::valid();
        topology.replace_property(1, "location_id 4096\n", "location_id 1280\n");
        topology.replace_property(1, "unique_id 2001\n", "unique_id 4660\n");
        for node in 2..=gpus {
            let minor = 127 + node;
            let pci = render
                .devices_root
                .join(format!("pci0000:00/0000:{:02x}:00.0", 16 * node));
            let drm = pci.join(format!("drm/renderD{minor}"));
            fs::create_dir_all(&drm).unwrap();
            for field in [
                "vendor",
                "device",
                "revision",
                "current_compute_partition",
                "current_memory_partition",
            ] {
                fs::copy(render.pci_path.join(field), pci.join(field)).unwrap();
            }
            fs::write(pci.join("unique_id"), format!("{:x}\n", 2000 + node)).unwrap();
            fs::write(drm.join("dev"), format!("226:{minor}\n")).unwrap();
            symlink(&pci, drm.join("device")).unwrap();
            symlink(
                &drm,
                render.device_character_root.join(format!("226:{minor}")),
            )
            .unwrap();
        }
        Self { topology, render }
    }

    fn paths(&self) -> DiscoveryPaths<'_> {
        let mut paths = self.render.paths();
        paths.topology_root = &self.topology.root;
        paths
    }

    fn discover<M: Mode>(&self) -> Result<(HostTopologySnapshot, M::Topology), TopologyError> {
        discover_host_topology_with::<M>(&self.paths())
    }
}

fn observed<M: Mode>(fixture: &HostFixture) -> (Result<HostTopologySnapshot, String>, Vec<Event>) {
    let trace = TraceScope::new(None);
    let result = fixture
        .discover::<M>()
        .map(|(snapshot, _)| snapshot)
        .map_err(|e| format!("{e:?}"));
    (result, trace.events())
}

#[test]
fn timing_preserves_fresh_complete_host_snapshots_and_ordered_io() {
    for gpus in 1..=3 {
        for optional in [0, 1, 2] {
            let fixture = HostFixture::valid(gpus);
            if optional == 0 {
                fs::remove_file(fixture.render.module_root.join("version")).unwrap();
                fs::remove_file(fixture.render.module_root.join("srcversion")).unwrap();
            } else if optional == 2 {
                let parameters = fixture.render.module_root.join("parameters");
                fs::create_dir(&parameters).unwrap();
                for name in ["mes", "sched_policy", "cwsr_enable"] {
                    fs::write(parameters.join(name), "1\n").unwrap();
                }
            }
            let (ordinary, expected) = observed::<Disabled>(&fixture);
            let ordinary = ordinary.unwrap();
            let sentinels: Vec<_> = expected
                .iter()
                .filter_map(|(op, path, _)| {
                    if *op == "bounded read" && *path == fixture.topology.root.join("generation_id")
                    {
                        Some("generation")
                    } else if *op == "bounded read" && *path == fixture.render.boot_id {
                        Some("boot")
                    } else if *op == "bounded read" && *path == fixture.render.os_release {
                        Some("release")
                    } else if *op == "optional inspect"
                        && *path == fixture.render.module_root.join("version")
                    {
                        Some("module")
                    } else if *op == "inspect" && *path == fixture.render.device_character_root {
                        Some("character root")
                    } else if *op == "canonicalize" && *path == fixture.render.devices_root {
                        Some("devices root")
                    } else if *op == "inspect"
                        && path.parent() == Some(&fixture.render.device_character_root)
                    {
                        Some("render link")
                    } else {
                        None
                    }
                })
                .collect();
            let pinned: Vec<_> = [
                "generation",
                "generation",
                "boot",
                "release",
                "module",
                "character root",
                "devices root",
            ]
            .into_iter()
            .chain(std::iter::repeat_n("render link", gpus as usize))
            .chain(["generation", "boot", "release", "module"])
            .collect();
            assert_eq!(sentinels, pinned);
            let scope = TraceScope::new(None);
            let (profiled, detail) = fixture.discover::<Enabled>().unwrap();
            assert_eq!(ordinary, profiled);
            assert_eq!(scope.events(), expected);
            assert!(detail.is_complete());
            drop(scope);
            let path = fixture.topology.node(1).join("io_links/0/properties");
            let contents = fs::read_to_string(&path).unwrap();
            fs::write(path, contents.replace("weight 15\n", "weight 16\n")).unwrap();
            let (updated, _) = observed::<Disabled>(&fixture);
            assert_ne!(updated.as_ref().unwrap(), &ordinary);
            assert_eq!(
                observed::<Disabled>(&fixture),
                observed::<Enabled>(&fixture)
            );
        }
    }
}

#[test]
fn timing_preserves_static_topology_identity_and_render_failures() {
    for case in 0..17 {
        let fixture = HostFixture::valid(2);
        let link = fixture.topology.node(1).join("p2p_links/0/properties");
        match case {
            0 => fs::write(&link, "type 11\n").unwrap(),
            1 => fs::write(&link, vec![b'x'; MAX_PROPERTY_BYTES + 1]).unwrap(),
            2 => fs::write(&link, [0xff]).unwrap(),
            3 => {
                fs::remove_file(&link).unwrap();
            }
            4 => {
                fs::remove_file(&link).unwrap();
                symlink(fixture.topology.root.join("generation_id"), &link).unwrap();
            }
            5 => {
                fs::remove_file(&link).unwrap();
                fs::create_dir(&link).unwrap();
            }
            6 => {
                fs::remove_file(&link).unwrap();
                rustix::fs::mkfifoat(
                    rustix::fs::CWD,
                    &link,
                    rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
                )
                .unwrap();
            }
            7 => fs::write(&fixture.render.boot_id, "bad\n").unwrap(),
            8 => fs::write(&fixture.render.os_release, "bad release\n").unwrap(),
            9 => fs::write(fixture.render.module_root.join("srcversion"), "bad\n").unwrap(),
            10 => fs::remove_file(fixture.render.device_character_root.join("226:128")).unwrap(),
            11 => fs::write(fixture.render.pci_path.join("unique_id"), "1235\n").unwrap(),
            12 => fs::write(fixture.render.pci_path.join("device"), "0x0001\n").unwrap(),
            13 => fs::write(
                fixture.render.pci_path.join("current_compute_partition"),
                "bad\n",
            )
            .unwrap(),
            14 => fixture
                .topology
                .replace_property(1, "location_id 1280\n", "location_id 1536\n"),
            15 => fs::write(
                fixture.render.pci_path.join("current_memory_partition"),
                "bad\n",
            )
            .unwrap(),
            16 => fs::write(
                fixture.render.pci_path.join("drm/renderD128/dev"),
                "226:129\n",
            )
            .unwrap(),
            _ => unreachable!(),
        }
        let ordinary = observed::<Disabled>(&fixture);
        assert!(ordinary.0.is_err(), "case {case}");
        assert_eq!(ordinary, observed::<Enabled>(&fixture), "case {case}");
    }
}

fn closing_change<M: Mode>(
    fixture: &HostFixture,
    index: usize,
    path: &Path,
    replacement: &'static str,
) -> (String, Vec<Event>) {
    let path = path.to_path_buf();
    let scope = TraceScope::new(Some((
        index,
        Box::new(move || fs::write(path, replacement).unwrap()),
    )));
    let error = match fixture.discover::<M>() {
        Err(error) => error,
        Ok(_) => panic!("closing change was accepted"),
    };
    (format!("{error:?}"), scope.events())
}

#[test]
fn closing_identity_changes_fail_identically_in_each_mode() {
    let fixture = HostFixture::valid(2);
    let (_, expected) = observed::<Disabled>(&fixture);
    for (path, operation, replacement) in [
        (
            fixture.topology.root.join("generation_id"),
            "inspect",
            "8\n",
        ),
        (
            fixture.render.boot_id.clone(),
            "inspect",
            "417d0f9a-4f05-4ab0-8922-3ebfd7354c8b\n",
        ),
        (
            fixture.render.os_release.clone(),
            "inspect",
            "6.8.0-125-generic\n",
        ),
        (
            fixture.render.module_root.join("version"),
            "optional inspect",
            "6.16.14\n",
        ),
    ] {
        let index = expected
            .iter()
            .rposition(|(op, p, _)| *op == operation && p == &path)
            .unwrap();
        let original = fs::read(&path).unwrap();
        let ordinary = closing_change::<Disabled>(&fixture, index, &path, replacement);
        fs::write(&path, &original).unwrap();
        let profiled = closing_change::<Enabled>(&fixture, index, &path, replacement);
        fs::write(&path, &original).unwrap();
        assert_eq!(ordinary, profiled);
        let expected_error = if path == fixture.topology.root.join("generation_id") {
            TopologyError::TopologyChanged {
                before: 7,
                after: 8,
            }
        } else if operation == "optional inspect" {
            TopologyError::ChangedDuringRead(fixture.render.module_root.clone())
        } else {
            TopologyError::ChangedDuringRead(path)
        };
        assert_eq!(ordinary.0, format!("{expected_error:?}"));
    }
}

fn panic_at<M: Mode>(fixture: &HostFixture, index: usize) -> Vec<Event> {
    let payload = Box::new(0xcafe_u64);
    let identity = &*payload as *const u64;
    let scope = TraceScope::new(Some((
        index,
        Box::new(move || std::panic::panic_any(payload)),
    )));
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _ = fixture.discover::<M>();
    }));
    let payload = result.unwrap_err().downcast::<Box<u64>>().unwrap();
    assert!(std::ptr::eq(&**payload, identity));
    scope.events()
}

#[test]
fn every_traced_io_panic_preserves_payload_and_order_in_both_modes() {
    let fixture = HostFixture::valid(2);
    let (_, expected) = observed::<Disabled>(&fixture);
    for index in 0..expected.len() {
        assert_eq!(panic_at::<Disabled>(&fixture, index), expected[..=index]);
        assert_eq!(panic_at::<Enabled>(&fixture, index), expected[..=index]);
    }
    assert_eq!(
        observed::<Disabled>(&fixture),
        observed::<Enabled>(&fixture)
    );
}
