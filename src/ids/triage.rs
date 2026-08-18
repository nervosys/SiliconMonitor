// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2024 NervoSys

//! Ranking and explaining findings — including when a model does it.
//!
//! # The rule
//!
//! **A model may rank and explain findings. It may not create one, alter its
//! evidence, or raise its confidence.**
//!
//! This is the same rule [`crate::tuning`] applies to settings — a model may
//! classify the workload, it may not choose the power limit — and it is here for
//! a sharper version of the same reason. A fabricated power limit damages one
//! machine. A fabricated security finding is indistinguishable from a real one to
//! everyone downstream: it will be triaged, escalated, and acted on, and the
//! only way to discover it was invented is to go back to a host that may no
//! longer exist in that state.
//!
//! The rule is enforced structurally rather than by instruction.
//! [`apply_model_triage`] takes the findings the detectors produced and a
//! model's proposed ordering, and returns findings drawn **only** from the
//! original set, matched by index. A model that names a finding that does not
//! exist changes nothing; a model that returns a different severity changes
//! nothing. The worst it can do is order them badly, and
//! [`Triage::unranked`] records anything it failed to mention so a bad ordering
//! cannot hide a finding either.
//!
//! # Why deterministic ranking exists too
//!
//! [`rank`] sorts without a model at all: severity, then confidence, then rule.
//! It is the default, and the model is an optional second opinion over the same
//! list. A tool whose triage depends on a network call is a tool that stops
//! working when the network does, which for an intrusion detector is precisely
//! when it is most likely to be needed.

use super::{Confidence, Finding, Severity};
use serde::{Deserialize, Serialize};

/// Findings in the order they should be looked at, plus what the ordering left
/// out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Triage {
    /// Highest priority first.
    pub ranked: Vec<Finding>,
    /// Findings the ranker did not place.
    ///
    /// Always empty for [`rank`], which places everything. Populated when a
    /// model's ordering omitted findings — appended to the end rather than
    /// dropped, because an ordering that loses a finding is worse than no
    /// ordering.
    pub unranked: Vec<Finding>,
    /// How the order was arrived at, so a reader can weigh it.
    pub method: Method,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// Sorted by severity, then confidence, then rule name. Reproducible.
    Deterministic,
    /// A model proposed the order over the deterministic list. Named so a
    /// consumer can discount it.
    Model { backend: String },
}

/// Sort findings by severity, then confidence, then rule.
///
/// Ties break on rule name so the output is stable across runs — an unstable
/// ordering makes two scans look different when nothing changed, which trains
/// readers to ignore the diff.
pub fn rank(mut findings: Vec<Finding>) -> Triage {
    findings.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then(b.confidence.cmp(&a.confidence))
            .then(a.rule.cmp(&b.rule))
            .then(a.title.cmp(&b.title))
    });
    Triage {
        ranked: findings,
        unranked: Vec::new(),
        method: Method::Deterministic,
    }
}

/// What a model is allowed to return: an ordering, expressed as indices into the
/// findings it was given.
///
/// Indices rather than reconstructed findings, and this is the whole enforcement
/// mechanism. A model that returns objects can return an object that was never
/// observed; a model that returns positions cannot say anything about a machine
/// at all. It can only reorder what the detectors found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelOrdering {
    /// Indices into the input slice, most important first.
    pub order: Vec<usize>,
    /// The model's explanation. Advisory prose, carried for a reader; it is not
    /// evidence and is never merged into any [`Finding`].
    #[serde(default)]
    pub rationale: Option<String>,
}

/// Apply a model's proposed ordering to findings the detectors produced.
///
/// Every guarantee here is structural:
///
/// - Output findings are cloned from `findings`; nothing else can appear.
/// - Out-of-range indices are ignored rather than erroring, because a malformed
///   ordering should degrade to a worse order, not to no report.
/// - Repeated indices are used once.
/// - Anything the ordering omitted lands in [`Triage::unranked`], so a model
///   cannot bury a finding by leaving it out.
pub fn apply_model_triage(
    findings: &[Finding],
    ordering: &ModelOrdering,
    backend: impl Into<String>,
) -> Triage {
    let mut used = vec![false; findings.len()];
    let mut ranked = Vec::with_capacity(findings.len());

    for &i in &ordering.order {
        let Some(slot) = used.get_mut(i) else {
            continue; // out of range: the model referred to something absent
        };
        if *slot {
            continue; // repeated
        }
        *slot = true;
        ranked.push(findings[i].clone());
    }

    let unranked: Vec<Finding> = findings
        .iter()
        .enumerate()
        .filter(|(i, _)| !used[*i])
        .map(|(_, f)| f.clone())
        .collect();

    Triage {
        ranked,
        unranked,
        method: Method::Model {
            backend: backend.into(),
        },
    }
}

/// The prompt a model is given. Exposed so it can be read and tested rather
/// than living inline at a call site.
///
/// It asks for positions and nothing else. A model that ignores the instruction
/// and returns prose simply fails to parse, and the caller falls back to
/// [`rank`] — the failure mode is a worse ordering, never a wrong finding.
pub const TRIAGE_PROMPT: &str = "\
You are ordering security findings that have already been detected by other means. \
Return only a JSON object of the form {\"order\":[<indices>],\"rationale\":\"...\"} \
where each index refers to a finding in the list you were given, most important \
first. You cannot add findings, change their severity or confidence, or alter \
their evidence — anything you write other than the ordering and the rationale is \
discarded. Judge importance by what an incident responder should look at first.";

/// Summary counts, for a caller deciding whether to wake anyone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Counts {
    pub critical: usize,
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub info: usize,
    /// Of the above, how many rest on a heuristic rather than a direct
    /// observation. Reported separately so "12 findings" cannot be read as
    /// "12 facts".
    pub possible_only: usize,
}

pub fn counts(findings: &[Finding]) -> Counts {
    let mut c = Counts::default();
    for f in findings {
        match f.severity {
            Severity::Critical => c.critical += 1,
            Severity::High => c.high += 1,
            Severity::Medium => c.medium += 1,
            Severity::Low => c.low += 1,
            Severity::Info => c.info += 1,
        }
        if f.confidence == Confidence::Possible {
            c.possible_only += 1;
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{Evidence, Subject};

    fn finding(rule: &str, sev: Severity, conf: Confidence) -> Finding {
        Finding::new(
            rule,
            format!("title for {rule}"),
            sev,
            conf,
            Subject::File {
                path: format!("/{rule}"),
            },
            vec![Evidence::observed("test", rule)],
        )
        .expect("evidence supplied")
    }

    #[test]
    fn ranking_puts_the_worst_first_and_is_stable() {
        let input = vec![
            finding("b.low", Severity::Low, Confidence::Certain),
            finding("a.critical", Severity::Critical, Confidence::Possible),
            finding("c.high", Severity::High, Confidence::Certain),
        ];
        let t = rank(input.clone());
        let rules: Vec<&str> = t.ranked.iter().map(|f| f.rule.as_str()).collect();
        assert_eq!(rules, vec!["a.critical", "c.high", "b.low"]);
        assert!(t.unranked.is_empty());

        // Same input in a different order must give the same output.
        let mut shuffled = input;
        shuffled.reverse();
        let again = rank(shuffled);
        assert_eq!(
            again
                .ranked
                .iter()
                .map(|f| f.rule.clone())
                .collect::<Vec<_>>(),
            t.ranked.iter().map(|f| f.rule.clone()).collect::<Vec<_>>(),
            "an unstable order makes two identical scans look different, which \
             trains a reader to ignore the diff"
        );
    }

    #[test]
    fn confidence_breaks_a_severity_tie() {
        let t = rank(vec![
            finding("z.guess", Severity::High, Confidence::Possible),
            finding("a.observed", Severity::High, Confidence::Certain),
        ]);
        assert_eq!(t.ranked[0].rule, "a.observed");
    }

    /// The central guarantee: a model cannot invent a finding.
    #[test]
    fn a_model_cannot_introduce_a_finding_that_was_not_detected() {
        let detected = vec![finding(
            "file.modified",
            Severity::High,
            Confidence::Certain,
        )];
        // The model names an index that does not exist, twice, plus a valid one.
        let ordering = ModelOrdering {
            order: vec![99, 0, 42],
            rationale: Some("I also found a rootkit".into()),
        };
        let t = apply_model_triage(&detected, &ordering, "test-backend");

        assert_eq!(t.ranked.len(), 1, "only detected findings may appear");
        assert_eq!(t.ranked[0].rule, "file.modified");
        assert!(
            t.unranked.is_empty(),
            "the one real finding was placed, so nothing is left over"
        );
        // The prose is carried nowhere near a Finding.
        assert!(!format!("{:?}", t.ranked).contains("rootkit"));
    }

    /// A model cannot change what a finding says, only where it sits.
    #[test]
    fn model_triage_preserves_severity_confidence_and_evidence() {
        let detected = vec![
            finding("a", Severity::Low, Confidence::Possible),
            finding("b", Severity::Critical, Confidence::Certain),
        ];
        let ordering = ModelOrdering {
            order: vec![0, 1],
            rationale: None,
        };
        let t = apply_model_triage(&detected, &ordering, "test-backend");
        assert_eq!(t.ranked[0].severity, Severity::Low);
        assert_eq!(t.ranked[0].confidence, Confidence::Possible);
        assert_eq!(t.ranked[0].evidence, detected[0].evidence);
        assert_eq!(t.ranked[1].severity, Severity::Critical);
    }

    /// A bad ordering must not be able to hide something.
    #[test]
    fn a_finding_the_model_omitted_is_kept_rather_than_dropped() {
        let detected = vec![
            finding("a", Severity::Low, Confidence::Certain),
            finding("b.critical", Severity::Critical, Confidence::Certain),
        ];
        // The model lists only the harmless one.
        let ordering = ModelOrdering {
            order: vec![0],
            rationale: None,
        };
        let t = apply_model_triage(&detected, &ordering, "test-backend");
        assert_eq!(t.ranked.len(), 1);
        assert_eq!(
            t.unranked.len(),
            1,
            "an ordering that omits a finding must not remove it from the report"
        );
        assert_eq!(t.unranked[0].rule, "b.critical");
    }

    #[test]
    fn a_repeated_index_is_used_once() {
        let detected = vec![finding("a", Severity::Low, Confidence::Certain)];
        let ordering = ModelOrdering {
            order: vec![0, 0, 0],
            rationale: None,
        };
        let t = apply_model_triage(&detected, &ordering, "test-backend");
        assert_eq!(t.ranked.len(), 1);
    }

    #[test]
    fn an_empty_ordering_leaves_everything_unranked_rather_than_losing_it() {
        let detected = vec![finding("a", Severity::High, Confidence::Certain)];
        let t = apply_model_triage(
            &detected,
            &ModelOrdering {
                order: vec![],
                rationale: None,
            },
            "test-backend",
        );
        assert!(t.ranked.is_empty());
        assert_eq!(t.unranked.len(), 1);
    }

    #[test]
    fn the_method_records_which_ranker_was_used() {
        let detected = vec![finding("a", Severity::Low, Confidence::Certain)];
        assert_eq!(rank(detected.clone()).method, Method::Deterministic);
        let t = apply_model_triage(
            &detected,
            &ModelOrdering {
                order: vec![0],
                rationale: None,
            },
            "ollama/llama3",
        );
        assert_eq!(
            t.method,
            Method::Model {
                backend: "ollama/llama3".into()
            }
        );
    }

    /// Heuristic findings are counted separately so a total cannot be read as a
    /// count of facts.
    #[test]
    fn counts_separate_heuristics_from_observations() {
        let c = counts(&[
            finding("a", Severity::High, Confidence::Possible),
            finding("b", Severity::High, Confidence::Certain),
            finding("c", Severity::Critical, Confidence::Possible),
        ]);
        assert_eq!(c.high, 2);
        assert_eq!(c.critical, 1);
        assert_eq!(
            c.possible_only, 2,
            "'3 findings' must not be readable as '3 facts'"
        );
    }

    #[test]
    fn the_prompt_states_the_limit_it_relies_on() {
        assert!(TRIAGE_PROMPT.contains("cannot add findings"));
        assert!(TRIAGE_PROMPT.contains("discarded"));
    }
}
