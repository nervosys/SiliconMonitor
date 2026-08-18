// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2024 NervoSys

//! Tests derived from the capability ontology rather than written beside it.
//!
//! # Why derive instead of list
//!
//! A hand-written list of "things that should be declared" is a second copy of
//! the thing it checks, and the two drift. This project has watched that happen
//! more than once: a `rust-version` nothing built against, a README claiming
//! platforms it had never run on, a metric registry described as populated when
//! it was empty.
//!
//! So these tests iterate [`capability::catalogue`] and the code it describes,
//! and assert the two agree in **both** directions. Adding an apply handler
//! without declaring it fails. Declaring a capability for a handler that does
//! not exist fails. Emitting a detection rule that no schema names fails.
//!
//! That is what makes the ontology agentic-first rather than decorative: an
//! agent can read the catalogue and rely on it, because nothing can quietly
//! stop being true.

use simonlib::ontology::capability::{self, Platform, Support, Surface};
use std::collections::BTreeSet;

fn ids_with_prefix(prefix: &str) -> BTreeSet<String> {
    capability::catalogue()
        .into_iter()
        .filter(|c| c.id.starts_with(prefix))
        .map(|c| c.id[prefix.len()..].to_string())
        .collect()
}

/// Every registered write handler is declared, and every declaration is real.
///
/// Both directions on purpose. One direction alone lets the catalogue fall
/// behind the code or run ahead of it, and both failures read the same to an
/// agent: a promise that does not match the machine.
#[test]
fn declared_settings_and_registered_handlers_are_the_same_set() {
    let registered: BTreeSet<String> = simonlib::profile::apply::builtin_handlers()
        .iter()
        .map(|h| h.setting_id().to_string())
        .collect();
    let declared = ids_with_prefix("setting.");

    let undeclared: Vec<&String> = registered.difference(&declared).collect();
    assert!(
        undeclared.is_empty(),
        "these settings have write handlers and no capability entry: {undeclared:?}. \
         An agent reading the catalogue would not know simon can write them."
    );

    let phantom: Vec<&String> = declared.difference(&registered).collect();
    assert!(
        phantom.is_empty(),
        "these settings are declared and have no handler on this platform: \
         {phantom:?}. A capability that cannot be exercised is a promise the \
         binary cannot keep."
    );
}

/// Every rule a detector can emit is declared in `ids::RULES`.
///
/// Scans the source rather than the runtime, because most rules only fire
/// against a machine in a particular state and a test that waits for an
/// intrusion is not a test.
#[test]
fn every_detection_rule_the_source_emits_is_declared() {
    let declared: BTreeSet<&str> = simonlib::ids::RULES.iter().map(|(r, _)| *r).collect();

    let mut emitted = BTreeSet::new();
    for file in ["src/ids/file.rs", "src/ids/network.rs"] {
        let src = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("reading {file}: {e}"));
        // `Finding::new(` is immediately followed by the rule literal.
        for (idx, _) in src.match_indices("Finding::new(") {
            let rest = &src[idx..];
            let Some(open) = rest.find('"') else { continue };
            let after = &rest[open + 1..];
            let Some(close) = after.find('"') else {
                continue;
            };
            let rule = &after[..close];
            // Guard against picking up a doc line or a format string.
            if rule.contains(' ') || rule.is_empty() {
                continue;
            }
            emitted.insert(rule.to_string());
        }
    }

    assert!(
        !emitted.is_empty(),
        "the scan found no rules at all, so it is checking nothing — the shape of \
         `Finding::new` must have changed"
    );

    let undeclared: Vec<&String> = emitted
        .iter()
        .filter(|r| !declared.contains(r.as_str()))
        .collect();
    assert!(
        undeclared.is_empty(),
        "these rules are emitted and not declared in ids::RULES: {undeclared:?}. \
         A finding with no schema entry is one an agent cannot interpret."
    );

    let unemitted: Vec<&&str> = declared.iter().filter(|r| !emitted.contains(**r)).collect();
    assert!(
        unemitted.is_empty(),
        "these rules are declared and never emitted: {unemitted:?}. Either a \
         detector was removed or the schema describes something that cannot \
         happen."
    );
}

/// Every ontology domain has a reading capability.
///
/// The reading ontology says what a value means; this says whether simon can
/// produce it here. A domain present in one and absent from the other leaves an
/// agent able to interpret a reading it cannot obtain, or able to request one it
/// cannot interpret.
#[test]
fn every_ontology_domain_declares_its_reading_support() {
    let declared = ids_with_prefix("reading.");
    for domain in simonlib::ontology::Domain::ALL {
        assert!(
            declared.contains(domain.as_str()),
            "the ontology declares the {} domain and the capability catalogue \
             says nothing about whether simon can read it",
            domain.as_str()
        );
    }
    for d in &declared {
        assert!(
            simonlib::ontology::Domain::ALL
                .iter()
                .any(|dom| dom.as_str() == d),
            "reading.{d} is declared and is not an ontology domain"
        );
    }
}

/// A capability that claims to work here should actually be reachable here.
///
/// Checked for the surfaces that can be asked cheaply. This is the test that
/// would have caught the macOS resolver having no reader wired in while the
/// documentation implied one.
#[test]
fn readings_claimed_usable_here_actually_resolve() {
    let snapshot = simonlib::ontology::resolve::snapshot();
    let here = Platform::current().expect("a named platform");

    for c in capability::catalogue() {
        if c.surface != Surface::Reading {
            continue;
        }
        let Some(support) = c.support.get(&here) else {
            continue;
        };
        if !support.is_usable() {
            continue;
        }
        let domain = c.id.trim_start_matches("reading.");
        let prefix = format!("{domain}.");

        let observed = snapshot
            .iter()
            .filter(|r| r.id.starts_with(&prefix))
            .filter(|r| r.is_observation())
            .count();

        assert!(
            observed > 0,
            "{} claims to be usable on {} and produced no observation at all in a \
             live snapshot. Either the reader is broken or the claim is wrong; \
             both are worth failing over.",
            c.id,
            here.as_str()
        );
    }
}

/// Anything the catalogue says is unusable here must not be silently producing
/// confident readings anyway.
///
/// The inverse of the test above, and the one that catches a stale pessimistic
/// claim — a capability marked unimplemented long after somebody implemented it.
#[test]
fn readings_claimed_unimplemented_here_really_produce_nothing() {
    let snapshot = simonlib::ontology::resolve::snapshot();
    let here = Platform::current().expect("a named platform");

    for c in capability::catalogue() {
        if c.surface != Surface::Reading {
            continue;
        }
        let Some(Support::Unimplemented { .. }) = c.support.get(&here) else {
            continue;
        };
        let domain = c.id.trim_start_matches("reading.");
        let prefix = format!("{domain}.");

        let observed: Vec<&str> = snapshot
            .iter()
            .filter(|r| r.id.starts_with(&prefix))
            .filter(|r| r.is_observation())
            .map(|r| r.id.as_str())
            .take(5)
            .collect();

        assert!(
            observed.is_empty(),
            "{} is declared unimplemented on {} and yet these resolved as real \
             observations: {observed:?}. The catalogue is out of date, which is \
             worse than it being pessimistic — an agent planned around an absence \
             that is not there.",
            c.id,
            here.as_str()
        );
    }
}

/// The tuning verification claim is checked against the code, not remembered.
#[test]
fn the_tuning_verify_capability_matches_the_metric_registry() {
    let cap = capability::catalogue()
        .into_iter()
        .find(|c| c.id == "tuning.verify")
        .expect("tuning.verify is declared");

    let any_metric = [
        "active_scheme_guid",
        "scaling_governor",
        "persistence_mode",
        "perf_level",
        "gt_max_freq_mhz",
    ]
    .iter()
    .any(|id| simonlib::tuning::verify::metric_for(id).is_some());

    let claims_partial = cap
        .support
        .values()
        .all(|s| matches!(s, Support::Partial { .. }));

    if any_metric {
        assert!(
            !claims_partial,
            "a metric is now registered, so tuning.verify can reach a verdict and \
             the catalogue still says it cannot"
        );
    } else {
        assert!(
            claims_partial,
            "no metric is registered, so every verified apply reports \
             unverifiable, and the catalogue must say so on every platform"
        );
    }
}

/// The catalogue and the README must not contradict each other.
///
/// They are two renderings of the same claims for two audiences, and the prose
/// one is the one that rots.
#[test]
fn the_readme_and_the_catalogue_agree_about_macos_gpu() {
    let readme = std::fs::read_to_string("README.md").expect("README.md");
    let says_no_gpu = readme.contains("macOS has no GPU, power or temperature readers")
        || readme.contains("macOS has no GPU, power or temperature reader");

    let cap = capability::catalogue()
        .into_iter()
        .find(|c| c.id == "reading.gpu")
        .expect("reading.gpu is declared");
    let declared_missing = matches!(
        cap.support.get(&Platform::MacOS),
        Some(Support::Unimplemented { .. })
    );

    assert_eq!(
        says_no_gpu, declared_missing,
        "the README and the capability catalogue disagree about whether macOS \
         has a GPU reader. Two renderings of one claim that differ means at least \
         one is lying to somebody."
    );
}

/// Every interface module simon ships is declared.
///
/// The first version of the catalogue declared three interfaces and simon had
/// eight: the gui, tui, MCP server, HTTP server and daemon were all missing, and
/// nothing noticed because the catalogue was only ever checked against itself.
/// An agent asking "how can I talk to this" would have been told about a third
/// of the answers.
#[test]
fn every_interface_module_is_declared() {
    // Module path to capability id. Listed rather than derived from the
    // directory because not every module is an interface, and a heuristic over
    // file names would either miss one or invent one.
    const INTERFACES: &[(&str, &str)] = &[
        ("src/tui", "interface.tui"),
        ("src/gui", "interface.gui"),
        ("src/ai_api/mcp_server.rs", "interface.mcp"),
        ("src/http_server.rs", "interface.http"),
        ("src/daemon.rs", "interface.daemon"),
        ("src/agent", "interface.agent"),
    ];

    let declared: BTreeSet<String> = capability::catalogue()
        .into_iter()
        .filter(|c| c.surface == Surface::Interface)
        .map(|c| c.id)
        .collect();

    for (path, id) in INTERFACES {
        assert!(
            std::path::Path::new(path).exists(),
            "{path} is listed as an interface and does not exist; either it was              removed and {id} should go too, or the path is wrong"
        );
        assert!(
            declared.contains(*id),
            "{path} ships and {id} is not in the capability catalogue. An agent              reading the catalogue would not know this way of talking to simon              exists."
        );
    }
}

/// Surfaces are covered rather than declared and unused.
#[test]
fn every_surface_has_at_least_one_capability() {
    let cat = capability::catalogue();
    for surface in Surface::ALL {
        assert!(
            cat.iter().any(|c| c.surface == *surface),
            "the {} surface is declared in the type and described by nothing",
            surface.as_str()
        );
    }
}

/// What an agent would ask first: what is not worth trying here.
#[test]
fn unusable_here_is_answerable_and_explains_itself() {
    for c in capability::unusable_here() {
        let support = c.here().expect("filtered on this platform");
        let reason = match support {
            Support::Unimplemented { reason } | Support::Unverified { reason } => reason.clone(),
            other => panic!("{} is listed unusable while declaring {other:?}", c.id),
        };
        assert!(
            !reason.trim().is_empty(),
            "{} is unusable here and does not say why",
            c.id
        );
    }
}
