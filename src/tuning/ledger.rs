// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2024 NervoSys

//! What the loop has already learned, so it stops relearning it.
//!
//! # The defect this exists to fix
//!
//! [`crate::tuning::verify`] closed half a loop. It measures whether a write
//! helped and undoes the ones that hurt — and then the next cycle proposes the
//! same setting again, because nothing consults the result.
//!
//! On a machine where a setting genuinely makes things worse, the unattended
//! server therefore: applies it, spends two sampling windows and a settle period
//! measuring it, concludes it regressed, reverts it — and repeats that on every
//! cycle, forever. Two writes and several seconds of measurement per interval,
//! producing a machine that flaps between two configurations and an audit log
//! that fills with the same pair of entries.
//!
//! Feedback that reaches the revert but never the decision is not a closed loop.
//! It is an expensive way to stand still.
//!
//! # What is remembered, and what is deliberately not
//!
//! Verdicts, per setting id. Not values, not timestamps of the machine's state,
//! not a model of the hardware — those are the resolver's job and it does them
//! better. The ledger answers one question: *has this been tried, and what
//! happened?*
//!
//! It is owned by the caller rather than held in a static. A hidden global would
//! make the loop's behaviour depend on invisible history, which is precisely
//! what makes an autonomous agent hard to reason about; passing it in means a
//! caller can inspect it, serialise it, or start clean.
//!
//! # Nothing here is permanent
//!
//! A regression on an idle Tuesday is not proof about a busy Friday, so
//! [`Ledger::clear`] and [`Ledger::forget`] exist and the entries carry their
//! attempt counts. What the ledger prevents is *relearning the same thing every
//! sixty seconds*. It is not a claim that the world stopped changing.

use super::verify::Verdict;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Why a setting is not being proposed again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Skip {
    /// It was applied, measured, and made things worse.
    Regressed,
    /// It was applied and measured, and nothing moved. Re-applying costs two
    /// sampling windows and a settle period to learn the same thing.
    Unchanged,
    /// It was applied, measured, and helped — so it is already in effect and
    /// there is nothing to do.
    AlreadyHelping,
}

impl Skip {
    pub fn reason(&self, setting_id: &str) -> String {
        match self {
            Skip::Regressed => format!(
                "{setting_id} was applied and measured as a regression on this machine, \
                 and was reverted; it will not be proposed again until the ledger is cleared"
            ),
            Skip::Unchanged => format!(
                "{setting_id} was applied and measured, and nothing moved beyond noise; \
                 re-measuring it every cycle costs time and learns the same thing"
            ),
            Skip::AlreadyHelping => format!(
                "{setting_id} was applied and measured as an improvement, so it is \
                 already in effect"
            ),
        }
    }
}

/// One setting's history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// How many times this setting has been applied and measured.
    pub attempts: u32,
    /// The most recent verdict.
    pub last: Verdict,
}

/// What the loop has tried.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Ledger {
    entries: BTreeMap<String, Entry>,
}

impl Ledger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record what a verified apply concluded.
    ///
    /// [`Verdict::Unverifiable`] is recorded but never causes a skip. "We could
    /// not tell" is not a result, and treating it as one would silently stop the
    /// loop from ever trying a setting whose metric happens to be unreadable —
    /// which, given the metric registry is empty, is currently every setting.
    pub fn record(&mut self, setting_id: &str, verdict: Verdict) {
        let entry = self.entries.entry(setting_id.to_string()).or_insert(Entry {
            attempts: 0,
            last: verdict.clone(),
        });
        entry.attempts = entry.attempts.saturating_add(1);
        entry.last = verdict;
    }

    /// Whether this setting should be left alone, and why.
    pub fn skip(&self, setting_id: &str) -> Option<Skip> {
        match &self.entries.get(setting_id)?.last {
            Verdict::Regressed { .. } => Some(Skip::Regressed),
            Verdict::Unchanged { .. } => Some(Skip::Unchanged),
            Verdict::Improved { .. } => Some(Skip::AlreadyHelping),
            // Deliberately not a skip. See `record`.
            Verdict::Unverifiable { .. } => None,
        }
    }

    pub fn get(&self, setting_id: &str) -> Option<&Entry> {
        self.entries.get(setting_id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Drop one setting's history, so it will be tried again.
    pub fn forget(&mut self, setting_id: &str) -> Option<Entry> {
        self.entries.remove(setting_id)
    }

    /// Drop everything. A verdict is evidence about the machine as it was, and
    /// a machine's workload changes.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Every setting the ledger has an opinion about, in id order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Entry)> {
        self.entries.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regressed() -> Verdict {
        Verdict::Regressed {
            metric: "m".into(),
            before: 10.0,
            after: 20.0,
            delta: 10.0,
        }
    }

    fn improved() -> Verdict {
        Verdict::Improved {
            metric: "m".into(),
            before: 20.0,
            after: 10.0,
            delta: -10.0,
        }
    }

    fn unchanged() -> Verdict {
        Verdict::Unchanged {
            metric: "m".into(),
            before: 10.0,
            after: 10.0,
            delta: 0.0,
            threshold: 1.0,
        }
    }

    /// The whole point: a measured regression is not retried.
    #[test]
    fn a_regression_is_not_proposed_again() {
        let mut l = Ledger::new();
        assert_eq!(l.skip("governor"), None, "an untried setting is fair game");
        l.record("governor", regressed());
        assert_eq!(l.skip("governor"), Some(Skip::Regressed));
    }

    #[test]
    fn an_improvement_is_already_in_effect_and_needs_no_rewrite() {
        let mut l = Ledger::new();
        l.record("governor", improved());
        assert_eq!(l.skip("governor"), Some(Skip::AlreadyHelping));
    }

    #[test]
    fn a_null_result_is_not_remeasured_every_cycle() {
        let mut l = Ledger::new();
        l.record("governor", unchanged());
        assert_eq!(l.skip("governor"), Some(Skip::Unchanged));
    }

    /// The exception that keeps the loop from freezing.
    ///
    /// Every setting is currently unverifiable, because no metric survived
    /// scrutiny. If "could not tell" counted as a result, the first cycle would
    /// record it for everything and the loop would never act again — a tuner
    /// stopped permanently by the absence of a measurement it never had.
    #[test]
    fn unverifiable_never_stops_a_setting_being_tried() {
        let mut l = Ledger::new();
        l.record(
            "governor",
            Verdict::Unverifiable {
                reason: "no metric is declared".into(),
            },
        );
        assert_eq!(
            l.skip("governor"),
            None,
            "an absent measurement is not evidence that the setting is bad"
        );
        assert_eq!(
            l.get("governor").map(|e| e.attempts),
            Some(1),
            "it is still recorded, so a reader can see it was tried"
        );
    }

    #[test]
    fn the_latest_verdict_wins_and_attempts_accumulate() {
        let mut l = Ledger::new();
        l.record("governor", regressed());
        l.record("governor", improved());
        assert_eq!(l.skip("governor"), Some(Skip::AlreadyHelping));
        assert_eq!(l.get("governor").unwrap().attempts, 2);
    }

    #[test]
    fn forgetting_puts_a_setting_back_in_play() {
        let mut l = Ledger::new();
        l.record("governor", regressed());
        assert!(l.skip("governor").is_some());
        l.forget("governor");
        assert_eq!(
            l.skip("governor"),
            None,
            "a regression measured once is evidence about that moment, not a life sentence"
        );
    }

    #[test]
    fn a_ledger_survives_serialisation() {
        let mut l = Ledger::new();
        l.record("governor", regressed());
        let json = serde_json::to_string(&l).unwrap();
        let back: Ledger = serde_json::from_str(&json).unwrap();
        assert_eq!(back, l);
        assert_eq!(back.skip("governor"), Some(Skip::Regressed));
    }

    /// The reasons are what a person reads in the cycle output, so they have to
    /// name the setting and say what happened rather than "skipped".
    #[test]
    fn skip_reasons_name_the_setting_and_the_evidence() {
        for skip in [Skip::Regressed, Skip::Unchanged, Skip::AlreadyHelping] {
            let r = skip.reason("scaling_governor");
            assert!(r.contains("scaling_governor"), "{r}");
            assert!(
                r.contains("measured"),
                "a skip must say it was measured, not just that it was skipped: {r}"
            );
        }
    }
}
