use super::*;
use std::fmt::Write;
const CHILD: &str = "FE2O3_PRIVATE_CELL_OWNED_TEST_CHILD";
const BEGIN: &str = "PRIVATE_CELL_OWNED_BEGIN\n";
const END: &str = "PRIVATE_CELL_OWNED_END";

#[test]
#[ignore = "fresh-process helper for genuine owning private-cell bytes and lineage"]
fn private_cell_owned_child() {
    assert_eq!(std::env::var(CHILD).as_deref(), Ok("1"));
    let mut text = String::new();
    for mutation in [false, true] {
        let mut module = fixture(ScalarType::U32, Some(2));
        if !mutation && let Kind::Load { access, .. } = &mut ops(&mut module)[6].kind {
            access.volatile = true;
        }
        with_input(module, |input, budget| {
            let owned = prepare_owned_private_cell_promotion_v1(input, budget).unwrap();
            budget.reserve_storage(owned.retained_storage()).unwrap();
            replay(&owned, input, budget);
            writeln!(
                text,
                "mutation={mutation} selected={:?} origins={:?}",
                owned.selected_allocations(),
                owned.origins()
            )
            .unwrap();
            for byte in owned.output().canonical().canonical_bytes() {
                write!(text, "{byte:02x}").unwrap();
            }
            text.push('\n');
            release(owned, budget);
        });
    }
    assert!(text.len() < 64 * 1024);
    println!("{BEGIN}{text}{END}");
}

#[test]
fn actual_owned_bytes_and_complete_lineage_repeat_in_two_fresh_processes() {
    let exe = std::env::current_exe().unwrap();
    let module = module_path!().split_once("::").unwrap().1;
    let name = format!("{module}::private_cell_owned_child");
    let mut previous = None;
    for _ in 0..2 {
        let result = std::process::Command::new(&exe)
            .args(["--exact", &name, "--ignored", "--nocapture"])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let stdout = std::str::from_utf8(&result.stdout).unwrap();
        assert_eq!(stdout.matches(BEGIN).count(), 1);
        assert_eq!(stdout.matches(END).count(), 1);
        let transcript = stdout
            .split_once(BEGIN)
            .unwrap()
            .1
            .split_once(END)
            .unwrap()
            .0;
        assert!(transcript.len() < 64 * 1024);
        assert!(transcript.contains("mutation=false"));
        assert!(transcript.contains("mutation=true"));
        if let Some(previous) = &previous {
            assert_eq!(transcript, previous);
        } else {
            previous = Some(transcript.to_owned());
        }
    }
}
