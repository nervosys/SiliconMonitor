//! The contract an AI agent relies on when driving simon.
//!
//! `simon describe` hands an agent a schema; `simon get` and `simon snapshot` hand
//! it values. Those are only useful together if they agree — a schema naming ids
//! the resolver never produces, or a resolver emitting ids the schema never
//! declared, sends an agent looking for things that do not exist.
//!
//! These tests drive the built binary rather than the library, because the argv
//! surface is the part an agent actually touches. A library-level test would pass
//! while `simon get` was broken.

use std::process::Command;

fn simon() -> Command {
    Command::new(env!("CARGO_BIN_EXE_simon"))
}

/// Whether this platform has readers that produce live hardware values.
///
/// The agent surface reads through `Simon::snapshot`, which requires every reader
/// to succeed. macOS gained CPU, memory and uptime readers in 3.1.0, but GPU,
/// power and temperature are still unimplemented there, so a snapshot never
/// populates and the agent surface has no values to report. Tests that assert
/// something *about a reading* have nothing to assert where no reading is
/// produced, and gating them on this says so once, by name, instead of scattering
/// `cfg(target_os)` through the file as if each site were its own special case.
/// The macOS readers that do exist are asserted in `tests/macos_readers.rs`.
///
/// This gates assertions about *values*. It deliberately does not gate the
/// contract itself: the schema, the id vocabulary, exit codes for unknown ids, and
/// the `describe`/`get`/`snapshot` agreement are all checked on every platform,
/// because they are properties of simon rather than of the hardware beneath it.
fn platform_has_hardware_readers() -> bool {
    cfg!(any(target_os = "linux", target_os = "windows"))
}

fn run(args: &[&str]) -> (String, String, i32) {
    let out = simon()
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to run `simon {}`: {e}", args.join(" ")));
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.code().unwrap_or(-1),
    )
}

fn json(args: &[&str]) -> serde_json::Value {
    let (stdout, stderr, _) = run(args);
    serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "`simon {}` did not emit valid JSON: {e}\nstdout:\n{stdout}\nstderr:\n{stderr}",
            args.join(" ")
        )
    })
}

/// The schema is what an agent fetches before it knows anything. It must be
/// machine-readable and carry its version.
#[test]
fn describe_emits_a_versioned_machine_readable_schema() {
    let doc = json(&["describe", "--format", "json"]);
    assert!(
        doc["version"].is_string(),
        "schema carries no version, so an agent cannot tell which contract it holds"
    );
    let entities = doc["entities"]
        .as_object()
        .expect("entities must be an object keyed by id");
    assert!(!entities.is_empty(), "schema declares no entities");

    for (id, entity) in entities {
        assert_eq!(
            entity["id"].as_str(),
            Some(id.as_str()),
            "key and id disagree for {id}"
        );
        // Provenance is the field that stops a constant being read as a sample; an
        // entity without it is unusable to a careful consumer.
        assert!(
            entity["provenance"].is_string(),
            "{id} declares no provenance"
        );
        assert!(
            entity["description"]
                .as_str()
                .is_some_and(|d| !d.is_empty()),
            "{id} has no description"
        );
    }
}

/// Every value the resolver produces must be declared in the schema, allowing for
/// template expansion (`gpu.{n}.name` covering `gpu.0.name`).
#[test]
fn every_resolved_reading_is_declared_in_the_schema() {
    use simonlib::ontology::Ontology;

    let ontology = Ontology::build();
    let snapshot = json(&["snapshot", "--format", "json"]);
    let readings = snapshot["readings"]
        .as_array()
        .expect("snapshot must carry a readings array");
    assert!(!readings.is_empty(), "snapshot produced nothing");

    for r in readings {
        let id = r["id"].as_str().expect("reading without an id");
        let declared = ontology.get(id).is_some() || {
            let parts: Vec<&str> = id.split('.').collect();
            ontology.entities.values().any(|e| {
                let t: Vec<&str> = e.id.split('.').collect();
                t.len() == parts.len()
                    && t.iter()
                        .zip(&parts)
                        .all(|(t, c)| t.starts_with('{') || t == c)
            })
        };
        assert!(
            declared,
            "{id} was resolved but the schema an agent fetched never declared it"
        );
    }
}

/// The invariant the resolver exists to hold: a value that could not be read is
/// absent and explained, never substituted with a plausible number.
#[test]
fn unreadable_values_are_absent_and_explained_never_defaulted() {
    let snapshot = json(&["snapshot", "--format", "json"]);
    let readings = snapshot["readings"].as_array().unwrap();

    for r in readings {
        let id = r["id"].as_str().unwrap();
        let provenance = r["provenance"].as_str().unwrap();
        if provenance == "unavailable" {
            assert!(
                r.get("value").is_none() || r["value"].is_null(),
                "{id} is unavailable yet carries a value — this is exactly the \
                 substitution that makes a failed read indistinguishable from a \
                 real one"
            );
            assert!(
                r["note"].as_str().is_some_and(|n| !n.trim().is_empty()),
                "{id} is unavailable with no reason, leaving an agent unable to \
                 tell an absent device from an unimplemented reader"
            );
        } else {
            assert!(
                r.get("value").is_some() && !r["value"].is_null(),
                "{id} claims provenance {provenance} but carries no value"
            );
        }
    }
}

/// `get` must distinguish "no such id" from "known id, nothing to report". An agent
/// that cannot tell these apart will retry a typo forever, or give up on a device
/// that is merely idle.
#[test]
fn get_distinguishes_unknown_ids_from_unavailable_values() {
    // `memory.total` is produced wherever a memory reader exists.
    if platform_has_hardware_readers() {
        let (_, _, code) = run(&["get", "memory.total"]);
        assert_eq!(code, 0, "reading memory.total should succeed");
    }

    let (_, stderr, code) = run(&["get", "definitely.not.an.entity"]);
    assert_eq!(code, 1, "an unknown id must exit 1");
    assert!(
        stderr.contains("Unknown entity"),
        "an unknown id should say so on stderr, got: {stderr}"
    );

    // A template is a schema construct, not a question with an answer.
    let (_, _, code) = run(&["get", "gpu.{n}.name"]);
    assert_eq!(code, 1, "a template id is not answerable and must exit 1");
}

/// Live readings must satisfy the ranges the schema declares. This is the
/// plausibility tier, enforced through the surface an agent actually reads.
#[test]
fn live_readings_satisfy_the_declared_ranges() {
    let (_, stderr, code) = run(&["snapshot", "--validate"]);
    assert_eq!(
        code, 0,
        "snapshot reported physically impossible readings:\n{stderr}"
    );

    let doc = json(&["snapshot", "--validate", "--format", "json"]);
    let impossible = doc["impossible"].as_array().unwrap();
    assert!(
        impossible.is_empty(),
        "impossible readings present: {impossible:#?}"
    );
}

/// An agent needs to discover operations, not only values. The catalogue is
/// generated from the parser, so this also checks the parser still exposes the
/// three commands the agentic surface is built on.
#[test]
fn the_command_catalogue_exposes_the_agentic_surface() {
    let catalog = json(&["describe", "--commands", "--format", "json"]);
    let names: Vec<&str> = catalog["subcommands"]
        .as_array()
        .expect("catalogue must list subcommands")
        .iter()
        .filter_map(|c| c["name"].as_str())
        .collect();

    for required in ["describe", "get", "snapshot"] {
        assert!(
            names.contains(&required),
            "`{required}` is missing from the command catalogue, so an agent \
             reading the catalogue cannot discover it; found {names:?}"
        );
    }

    // Arguments have to be described, or an agent has to guess at argv.
    let describe_cmd = catalog["subcommands"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "describe")
        .unwrap();
    let args = describe_cmd["args"].as_array().unwrap();
    assert!(
        args.iter().any(|a| a["long"] == "format"),
        "describe does not advertise --format"
    );
}

/// An agent needs to know what it may change, not only what it may read. Every
/// entity advertising `writable_via` must name a setting the binary will actually
/// accept, checked through the two surfaces an agent can reach.
#[test]
fn the_write_surface_agrees_with_what_the_binary_accepts() {
    let doc = json(&["describe", "--writable", "--format", "json"]);
    let entities = doc["entities"].as_object().expect("entities object");

    // `profile writable` is the operator-facing list; `describe --writable` is the
    // agent-facing one. They are different code paths over the same registry and
    // must not disagree.
    let (listed, _, _) = run(&["profile", "writable"]);

    for (id, entity) in entities {
        let via = entity["writable_via"]
            .as_str()
            .unwrap_or_else(|| panic!("{id} appears under --writable but has no writable_via"));
        assert_eq!(
            entity["kind"].as_str(),
            Some("setting"),
            "{id} is writable but is not declared a setting"
        );
        assert!(
            listed.contains(via),
            "the schema advertises writing {via:?} via {id}, but `simon profile \
             writable` does not list it — an agent would attempt a write the \
             binary rejects.\nlisted:\n{listed}"
        );
    }
}

/// The TUI must be readable without a terminal, or it is the one surface an agent
/// cannot inspect at all.
#[test]
fn the_tui_renders_a_frame_headlessly() {
    let (stdout, _, code) = run(&["tui", "--frame", "--width", "160", "--height", "24"]);
    assert_eq!(code, 0, "rendering a frame should succeed");
    assert!(
        stdout.contains("Overview"),
        "the frame is missing the tab bar:\n{stdout}"
    );
    let rows = stdout.lines().count();
    assert!(
        rows >= 20,
        "expected roughly the requested 24 rows, got {rows}"
    );
}

/// A frame rendered before the collector publishes shows zeroed defaults that look
/// exactly like an idle machine. The command waits for real data; this asserts it,
/// because the failure is silent and an agent would read the zeros as fact.
#[test]
fn a_rendered_frame_carries_readings_not_zeroed_defaults() {
    let (stdout, stderr, _) = run(&["tui", "--frame", "--width", "160", "--height", "24"]);

    // The command says so on stderr if it gave up waiting. If it did, that is a
    // legitimate outcome on a slow machine and the warning is the contract.
    if stderr.contains("no snapshot arrived") {
        return;
    }

    // The header carries CPU and MEM. Both reading exactly zero is the signature of
    // the un-populated state rather than of an idle machine: memory is never 0%.
    let header = stdout.lines().next().unwrap_or_default();
    assert!(
        header.contains("MEM:"),
        "the header does not report memory at all:\n{header}"
    );
    // 0% is the signature of the un-populated state on a platform that *has* a
    // memory reader. Where there is none it is simply the truth, so this checks
    // the header exists everywhere and checks its value only where a reading is
    // produced.
    if platform_has_hardware_readers() {
        assert!(
            !header.contains("MEM:0%"),
            "memory reads 0%, which no running machine does — the frame was rendered \
             before the collector published and is showing defaults:\n{header}"
        );
    }
}

/// Tab selection has to work by name, since an agent reading the tab bar has names
/// rather than indices — and an unknown name must not silently render the default.
#[test]
fn tui_frame_selects_tabs_by_name_and_rejects_unknown_ones() {
    let (stdout, _, code) = run(&["tui", "--frame", "--tab", "Memory", "--width", "160"]);
    assert_eq!(code, 0, "selecting a known tab by name should succeed");
    assert!(stdout.contains("Overview"), "tab bar missing");

    let (_, stderr, code) = run(&["tui", "--frame", "--tab", "not-a-tab"]);
    assert_eq!(code, 1, "an unknown tab must exit 1");
    assert!(
        stderr.contains("Unknown tab") && stderr.contains("Overview"),
        "an unknown tab should list the available ones, got: {stderr}"
    );
}

/// The GUI must be readable without a window, or it is the surface an agent cannot
/// see at all — and the one where "rendered but invisible" already happened once.
///
/// Since 4.0.0 the GUI is a Dewey application and `--frame` emits the ontology
/// tree rather than painted text. That is a stronger contract than the one this
/// test used to check: it asks whether a *named* node is present, not whether
/// some glyphs appeared. The old form was satisfied by a spinner, which is
/// exactly how four broken tabs passed it for six releases.
#[test]
fn the_gui_renders_a_tab_headlessly() {
    let (stdout, stderr, code) = run(&["gui", "--frame", "--tab", "profiles"]);
    assert_eq!(
        code, 0,
        "rendering a GUI tab should succeed.\nstderr: {stderr}"
    );
    let tree: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("--frame emitted invalid JSON: {e}\n{stdout}"));
    assert!(
        tree_contains_agent_id(&tree, "profiles_heading"),
        "the Profiles tab produced no profiles_heading node:\n{stdout}"
    );

    // An unknown tab must list the alternatives rather than silently rendering the
    // default, which would give an agent a frame for a tab it did not ask for.
    let (_, stderr, code) = run(&["gui", "--frame", "--tab", "not-a-tab"]);
    assert_eq!(code, 1, "an unknown GUI tab must exit 1");
    assert!(
        stderr.contains("unknown tab") && stderr.contains("Overview"),
        "an unknown tab should name the available ones, got: {stderr}"
    );
}

/// Walk the exported ontology tree looking for a node with this agent id.
fn tree_contains_agent_id(value: &serde_json::Value, id: &str) -> bool {
    match value {
        serde_json::Value::Object(map) => {
            if map
                .get("agent_id")
                .and_then(|v| v.as_str())
                .is_some_and(|v| v == id)
            {
                return true;
            }
            map.values().any(|v| tree_contains_agent_id(v, id))
        }
        serde_json::Value::Array(items) => items.iter().any(|v| tree_contains_agent_id(v, id)),
        _ => false,
    }
}

/// Every GUI tab must render its own named content.
///
/// The pre-4.0.0 version of this test counted "substantive lines" of painted
/// text, which was a proxy for the thing actually wanted and a poor one — a
/// spinner counted, and the threshold had to be tuned twice to avoid failing
/// tabs that were working. Asking for the tab's heading node by name is the
/// property that was meant all along.
#[test]
fn every_gui_tab_renders_its_own_content() {
    for (tab, expected) in [
        ("overview", "overview_heading"),
        ("cpu", "cpu_heading"),
        ("accelerators", "accelerators_heading"),
        ("processes", "processes_heading"),
        ("memory", "memory_heading"),
        ("network", "network_heading"),
        ("disk", "disk_heading"),
        ("system", "system_heading"),
        ("peripherals", "peripherals_heading"),
        ("profiles", "profiles_heading"),
        ("connections", "connections_heading"),
        ("network tools", "networktools_heading"),
        ("ai assistant", "ai_heading"),
    ] {
        let (stdout, stderr, code) = run(&["gui", "--frame", "--tab", tab]);
        assert_eq!(code, 0, "tab {tab} failed to render.\nstderr: {stderr}");
        let tree: serde_json::Value = serde_json::from_str(&stdout)
            .unwrap_or_else(|e| panic!("{tab}: --frame emitted invalid JSON: {e}"));

        // Tabs whose content is legitimately empty on some machines report that
        // through their own node rather than sharing the heading's, so an empty
        // result is still a positive answer and never looks like a stuck load.
        let empty_id = format!("{}_empty", expected.trim_end_matches("_heading"));
        assert!(
            tree_contains_agent_id(&tree, expected) || tree_contains_agent_id(&tree, &empty_id),
            "the {tab} tab produced neither {expected} nor {empty_id}:\n{stdout}"
        );
    }
}

/// The GUI script surface speaks Dewey's agent protocol.
///
/// Before 4.0.0 this was a vocabulary simon had invented — `goto`, `assert`,
/// `capture`. It is now one JSON agent request per line, so anything that can
/// drive a Dewey application can drive simon's GUI, and the protocol is
/// documented by Dewey rather than by this crate.
#[test]
fn the_gui_can_be_inspected_by_a_script() {
    use std::io::Write;
    use std::process::Stdio;

    fn run_script(script: &str) -> (String, String, i32) {
        let mut child = simon()
            .args(["gui", "--script", "-"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn simon gui --script");
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(script.as_bytes())
            .expect("write script");
        let out = child.wait_with_output().expect("wait");
        (
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
            out.status.code().unwrap_or(-1),
        )
    }

    let (stdout, stderr, code) = run_script("{\"type\":\"get_tree\"}\n");
    assert_eq!(
        code, 0,
        "a GetTree request should succeed.\nstderr: {stderr}"
    );
    assert!(
        stdout.contains("agent_id"),
        "the response should carry the ui tree:\n{stdout}"
    );

    // Blank lines and comments are skipped rather than erroring, so a script can
    // be commented without special-casing at the call site.
    let (_, _, code) = run_script("\n# a comment\n{\"type\":\"get_tree\"}\n");
    assert_eq!(code, 0, "comments and blank lines must be skipped");

    // Malformed input names the offending line, since a script that fails on
    // line 40 of 60 is useless if the error does not say which line.
    let (_, stderr, code) = run_script("{\"type\":\"get_tree\"}\nnot-json\n");
    assert_eq!(code, 1, "an unparseable request must exit 1");
    assert!(
        stderr.contains("line 2"),
        "the error should name the failing line, got: {stderr}"
    );
}

/// Driving the TUI is the other half of operability: an agent must be able to
/// navigate and assert, not only observe a frame it did not choose.
#[test]
fn the_tui_can_be_driven_by_a_script() {
    use std::io::Write;
    use std::process::Stdio;

    fn run_script(script: &str) -> (String, String, i32) {
        let mut child = simon()
            .args(["tui", "--script", "-", "--width", "120", "--height", "12"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn simon tui --script");
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(script.as_bytes())
            .expect("write script");
        let out = child.wait_with_output().expect("wait");
        (
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
            out.status.code().unwrap_or(-1),
        )
    }

    // Navigate by key, then assert on what that produced. `5` selects the fifth tab
    // in the interactive TUI, so a passing assertion here is evidence the shared
    // key handler behaves the same headlessly.
    let (stdout, stderr, code) = run_script("goto CPU\nkey 5\nassert Memory\ncapture\n");
    assert_eq!(code, 0, "script should pass.\nstderr: {stderr}");
    assert!(
        stdout.contains("Memory"),
        "the captured frame should show the Memory tab:\n{stdout}"
    );

    // A failed assertion must be reported and exit non-zero, or an agent would read
    // silence as success.
    let (_, stderr, code) = run_script("assert absolutely-not-on-this-screen\n");
    assert_eq!(code, 1, "a failed assertion must exit 1");
    assert!(
        stderr.contains("absolutely-not-on-this-screen"),
        "the failure should name what was missing, got: {stderr}"
    );

    // A malformed script is a different failure from a failed assertion, and gets a
    // different exit code so a caller can tell "my script is wrong" from "the TUI
    // is wrong".
    let (_, stderr, code) = run_script("frobnicate\n");
    assert_eq!(code, 2, "a malformed script must exit 2, not 1");
    assert!(stderr.contains("frobnicate"), "got: {stderr}");
}

/// The schema must be a fact about simon, not about the machine it ran on —
/// otherwise an agent cannot fetch it ahead of time or cache it across hosts.
#[test]
fn the_schema_is_stable_across_invocations() {
    let a = run(&["describe", "--format", "json"]).0;
    let b = run(&["describe", "--format", "json"]).0;
    assert_eq!(a, b, "describe output differs between runs");
}

/// Labels shown on screen must map back to ids, so an agent can turn what a user
/// reports seeing into something it can query.
#[test]
fn on_screen_labels_map_back_to_queryable_ids() {
    use simonlib::ontology::labels;

    let ids = labels::ids_for_label("Total");
    assert!(
        ids.iter().any(|id| id == "memory.total"),
        "the label a surface renders for memory.total does not map back to it"
    );

    // Domain spelling is shared, so the three surfaces cannot drift apart on
    // whether it is "Cpu", "cpu" or "CPU".
    assert_eq!(labels::domain_label("cpu"), "CPU");
    assert_eq!(labels::domain_label("gpu"), "GPU");
    assert!(labels::is_known_domain("memory"));
    assert!(!labels::is_known_domain("Overview"));
}
