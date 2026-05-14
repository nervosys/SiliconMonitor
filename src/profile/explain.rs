//! Look up a setting by id and return its full metadata.
//!
//! Different from [`super::ProfileSnapshot::search`]: that returns *all*
//! settings whose value/description matches a substring; this returns the
//! *first* exact-id hit, plus its complete metadata bundle (description,
//! default, range, choices, risk, source). Designed for AI agents to ask
//! "what is `pl1_w`?" and get a structured answer.

use super::{ProfileGroup, ProfileSnapshot, Setting, Subsystem};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingExplanation {
    pub subsystem: Subsystem,
    pub device: String,
    pub profile: String,
    pub setting: Setting,
    /// Other settings in the same group that look related (id prefix match
    /// — e.g. when looking up "pl1_w", also surface "pl1_enable", "pl2_w").
    pub related: Vec<Setting>,
}

/// Find a setting by exact `id` match (case-sensitive). Returns the first
/// match alphabetically by subsystem to make output deterministic.
pub fn explain(snapshot: &ProfileSnapshot, setting_id: &str) -> Option<SettingExplanation> {
    for (sub, groups) in &snapshot.providers {
        for group in groups {
            if let Some(setting) = group.settings.iter().find(|s| s.id == setting_id) {
                let related = related_settings(group, setting_id);
                return Some(SettingExplanation {
                    subsystem: *sub,
                    device: group.device.clone(),
                    profile: group.display_name.clone(),
                    setting: setting.clone(),
                    related,
                });
            }
        }
    }
    None
}

/// Fuzzy-find candidate setting ids when an exact match fails. Useful for
/// CLI / agent "did you mean…" prompts.
pub fn candidates(snapshot: &ProfileSnapshot, partial: &str) -> Vec<String> {
    let needle = partial.to_ascii_lowercase();
    let mut out: Vec<String> = snapshot
        .providers
        .values()
        .flat_map(|gs| gs.iter())
        .flat_map(|g| g.settings.iter())
        .filter(|s| s.id.to_ascii_lowercase().contains(&needle))
        .map(|s| s.id.clone())
        .collect();
    out.sort();
    out.dedup();
    out.truncate(20);
    out
}

fn related_settings(group: &ProfileGroup, target_id: &str) -> Vec<Setting> {
    // Sibling settings whose id shares a stem with the target. We split on
    // '.' (e.g. "feat.write_cache.enabled") and consider any setting sharing
    // the first one or two segments as "related".
    let segments: Vec<&str> = target_id.split('.').collect();
    let primary = segments.first().copied().unwrap_or("");
    let secondary = segments.get(1).copied();

    group
        .settings
        .iter()
        .filter(|s| s.id != target_id)
        .filter(|s| {
            let s_segs: Vec<&str> = s.id.split('.').collect();
            if s_segs.first().copied() != Some(primary) {
                return false;
            }
            match secondary {
                Some(sec) => s_segs.get(1).copied() == Some(sec),
                None => true,
            }
        })
        .take(8)
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::{ProfileGroup, Setting, SettingValue};
    use super::*;
    use std::collections::BTreeMap;

    fn snap_with(group: ProfileGroup) -> ProfileSnapshot {
        let mut providers = BTreeMap::new();
        let sub = group.subsystem;
        providers.insert(sub, vec![group]);
        ProfileSnapshot {
            timestamp: 0,
            providers,
            errors: BTreeMap::new(),
        }
    }

    #[test]
    fn finds_exact_id() {
        let mut g = ProfileGroup::new(Subsystem::Cpu, "CPU0", "Active", "test");
        let mut s = Setting::info("pl1_w", "PL1", SettingValue::Float(125.0));
        s.description = Some("Long-duration power limit.".into());
        g.push(s);
        let snap = snap_with(g);
        let exp = explain(&snap, "pl1_w").unwrap();
        assert_eq!(exp.setting.id, "pl1_w");
        assert!(exp.setting.description.unwrap().contains("Long"));
    }

    #[test]
    fn missing_id_returns_none() {
        let g = ProfileGroup::new(Subsystem::Cpu, "CPU0", "Active", "test");
        let snap = snap_with(g);
        assert!(explain(&snap, "nope").is_none());
    }

    #[test]
    fn related_shares_primary_segment() {
        let mut g = ProfileGroup::new(Subsystem::Nvme, "nvme0", "Features", "test");
        g.push(Setting::info("feat.write_cache.enabled", "WC", SettingValue::Bool(true)));
        g.push(Setting::info("feat.write_cache.raw", "WC raw", SettingValue::Uint(1)));
        g.push(Setting::info("feat.apst.enabled", "APST", SettingValue::Bool(true)));
        g.push(Setting::info("vbios_version", "vbios", SettingValue::Text("x".into())));
        let snap = snap_with(g);
        let exp = explain(&snap, "feat.write_cache.enabled").unwrap();
        // Should include feat.write_cache.raw (shares feat.write_cache) but
        // not feat.apst.enabled (different secondary) or vbios_version.
        assert!(exp.related.iter().any(|s| s.id == "feat.write_cache.raw"));
        assert!(!exp.related.iter().any(|s| s.id == "feat.apst.enabled"));
        assert!(!exp.related.iter().any(|s| s.id == "vbios_version"));
    }

    #[test]
    fn candidates_match_substring() {
        let mut g = ProfileGroup::new(Subsystem::Gpu, "GPU0", "Global", "test");
        g.push(Setting::info("power_limit_mw", "PL", SettingValue::Uint(450_000)));
        g.push(Setting::info("max_gfx_clock_mhz", "Clk", SettingValue::Uint(2100)));
        g.push(Setting::info("ecc_mode", "ECC", SettingValue::Bool(false)));
        let snap = snap_with(g);
        let cands = candidates(&snap, "power");
        assert_eq!(cands, vec!["power_limit_mw".to_string()]);
    }
}
