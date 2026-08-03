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
    // A value present on every supported platform.
    let (_, _, code) = run(&["get", "memory.total"]);
    assert_eq!(code, 0, "reading memory.total should succeed");

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
