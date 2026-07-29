//! Deviation report — every [`Setting`] with a declared `default` whose
//! current `value` differs from that default.
//!
//! Catches the "I changed something I forgot about" case after BIOS updates,
//! driver reinstalls, or NVPI tinkering. Pure inspection — no writes.

use super::{ProfileSnapshot, SettingValue, Subsystem};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deviation {
    pub subsystem: Subsystem,
    pub device: String,
    pub profile: String,
    pub setting_id: String,
    pub display_name: String,
    pub current: SettingValue,
    pub default: SettingValue,
    /// Risk classification copied from the underlying setting.
    pub risk: super::SettingRisk,
}

/// Walk a snapshot and collect every setting whose `value` differs from its
/// declared `default`. Settings without a default are skipped.
pub fn deviations_from_default(snapshot: &ProfileSnapshot) -> Vec<Deviation> {
    let mut out = Vec::new();
    for (sub, groups) in &snapshot.providers {
        for group in groups {
            for s in &group.settings {
                let Some(default) = s.default.clone() else {
                    continue;
                };
                if default == s.value {
                    continue;
                }
                out.push(Deviation {
                    subsystem: *sub,
                    device: group.device.clone(),
                    profile: group.display_name.clone(),
                    setting_id: s.id.clone(),
                    display_name: s.display_name.clone(),
                    current: s.value.clone(),
                    default,
                    risk: s.risk,
                });
            }
        }
    }
    // Stable ordering: by risk (Dangerous first), then subsystem, device, id.
    out.sort_by(|a, b| {
        risk_rank(a.risk)
            .cmp(&risk_rank(b.risk))
            .then_with(|| a.subsystem.cmp(&b.subsystem))
            .then_with(|| a.device.cmp(&b.device))
            .then_with(|| a.setting_id.cmp(&b.setting_id))
    });
    out
}

fn risk_rank(r: super::SettingRisk) -> u8 {
    match r {
        super::SettingRisk::Dangerous => 0,
        super::SettingRisk::Moderate => 1,
        super::SettingRisk::Safe => 2,
        super::SettingRisk::Informational => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{ProfileGroup, Setting, SettingRisk};
    use super::*;
    use std::collections::BTreeMap;

    fn snap_with(settings: Vec<Setting>) -> ProfileSnapshot {
        let mut group = ProfileGroup::new(Subsystem::Gpu, "GPU0", "Global", "test");
        group.settings = settings;
        let mut providers = BTreeMap::new();
        providers.insert(Subsystem::Gpu, vec![group]);
        ProfileSnapshot {
            timestamp: 0,
            providers,
            errors: BTreeMap::new(),
        }
    }

    #[test]
    fn no_default_no_deviation() {
        let snap = snap_with(vec![Setting::info("a", "A", SettingValue::Bool(true))]);
        assert!(deviations_from_default(&snap).is_empty());
    }

    #[test]
    fn matching_default_no_deviation() {
        let mut s = Setting::info("a", "A", SettingValue::Uint(450));
        s.default = Some(SettingValue::Uint(450));
        let snap = snap_with(vec![s]);
        assert!(deviations_from_default(&snap).is_empty());
    }

    #[test]
    fn mismatched_default_reports_deviation() {
        let mut s = Setting::info("power_limit", "Power Limit", SettingValue::Uint(350_000));
        s.default = Some(SettingValue::Uint(450_000));
        s.risk = SettingRisk::Moderate;
        let snap = snap_with(vec![s]);
        let devs = deviations_from_default(&snap);
        assert_eq!(devs.len(), 1);
        assert_eq!(devs[0].setting_id, "power_limit");
        assert!(matches!(devs[0].current, SettingValue::Uint(350_000)));
        assert!(matches!(devs[0].default, SettingValue::Uint(450_000)));
    }

    #[test]
    fn sorted_by_risk() {
        let mut s1 = Setting::info("safe_one", "S1", SettingValue::Bool(false));
        s1.default = Some(SettingValue::Bool(true));
        s1.risk = SettingRisk::Safe;
        let mut s2 = Setting::info("dangerous_one", "S2", SettingValue::Bool(false));
        s2.default = Some(SettingValue::Bool(true));
        s2.risk = SettingRisk::Dangerous;
        let snap = snap_with(vec![s1, s2]);
        let devs = deviations_from_default(&snap);
        assert_eq!(devs.len(), 2);
        assert_eq!(devs[0].setting_id, "dangerous_one"); // dangerous first
    }
}
