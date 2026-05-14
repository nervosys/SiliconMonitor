//! Profile snapshot diff — compare two [`ProfileSnapshot`]s and surface
//! added / removed / changed settings.
//!
//! Critical for tracking drift after driver updates, BIOS flashes, or BIOS
//! resets. Works on any platform because it operates purely on the
//! serializable snapshot type.

use super::{ProfileGroup, ProfileSnapshot, Setting, SettingValue, Subsystem};
use serde::{Deserialize, Serialize};

/// Result of comparing two snapshots.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileDiff {
    /// Settings that exist in `current` but not `baseline`.
    pub added: Vec<DiffEntry>,
    /// Settings that exist in `baseline` but not `current`.
    pub removed: Vec<DiffEntry>,
    /// Settings whose value changed between baseline and current.
    pub changed: Vec<DiffChange>,
    /// Devices/groups added in `current`.
    pub added_groups: Vec<DiffGroup>,
    /// Devices/groups removed in `current`.
    pub removed_groups: Vec<DiffGroup>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffEntry {
    pub subsystem: Subsystem,
    pub device: String,
    pub profile: String,
    pub setting_id: String,
    pub display_name: String,
    pub value: SettingValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffChange {
    pub subsystem: Subsystem,
    pub device: String,
    pub profile: String,
    pub setting_id: String,
    pub display_name: String,
    pub before: SettingValue,
    pub after: SettingValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffGroup {
    pub subsystem: Subsystem,
    pub device: String,
    pub profile: String,
    pub setting_count: usize,
}

impl ProfileDiff {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.changed.is_empty()
            && self.added_groups.is_empty()
            && self.removed_groups.is_empty()
    }

    pub fn total_changes(&self) -> usize {
        self.added.len()
            + self.removed.len()
            + self.changed.len()
            + self.added_groups.len()
            + self.removed_groups.len()
    }
}

/// Compare two snapshots. The comparison key for a setting is
/// `(subsystem, device, profile, setting_id)` — stable across runs because
/// `device`/`profile`/`id` are all derived from hardware identifiers, not
/// timestamps.
pub fn diff_snapshots(baseline: &ProfileSnapshot, current: &ProfileSnapshot) -> ProfileDiff {
    let mut diff = ProfileDiff::default();

    let base_groups = flatten_groups(baseline);
    let cur_groups = flatten_groups(current);

    // Detect added/removed groups by (subsystem, device, profile) key.
    let base_keys: std::collections::BTreeSet<_> = base_groups.keys().cloned().collect();
    let cur_keys: std::collections::BTreeSet<_> = cur_groups.keys().cloned().collect();

    for key in cur_keys.difference(&base_keys) {
        let group = cur_groups[key];
        diff.added_groups.push(DiffGroup {
            subsystem: key.0,
            device: key.1.clone(),
            profile: key.2.clone(),
            setting_count: group.settings.len(),
        });
        for s in &group.settings {
            diff.added.push(DiffEntry {
                subsystem: key.0,
                device: key.1.clone(),
                profile: key.2.clone(),
                setting_id: s.id.clone(),
                display_name: s.display_name.clone(),
                value: s.value.clone(),
            });
        }
    }
    for key in base_keys.difference(&cur_keys) {
        let group = base_groups[key];
        diff.removed_groups.push(DiffGroup {
            subsystem: key.0,
            device: key.1.clone(),
            profile: key.2.clone(),
            setting_count: group.settings.len(),
        });
        for s in &group.settings {
            diff.removed.push(DiffEntry {
                subsystem: key.0,
                device: key.1.clone(),
                profile: key.2.clone(),
                setting_id: s.id.clone(),
                display_name: s.display_name.clone(),
                value: s.value.clone(),
            });
        }
    }

    // For groups present in both, diff individual settings.
    for key in base_keys.intersection(&cur_keys) {
        let base = base_groups[key];
        let cur = cur_groups[key];
        let base_settings: std::collections::HashMap<&str, &Setting> =
            base.settings.iter().map(|s| (s.id.as_str(), s)).collect();
        let cur_settings: std::collections::HashMap<&str, &Setting> =
            cur.settings.iter().map(|s| (s.id.as_str(), s)).collect();

        for (id, s) in &cur_settings {
            match base_settings.get(id) {
                None => diff.added.push(DiffEntry {
                    subsystem: key.0,
                    device: key.1.clone(),
                    profile: key.2.clone(),
                    setting_id: s.id.clone(),
                    display_name: s.display_name.clone(),
                    value: s.value.clone(),
                }),
                Some(prev) if prev.value != s.value => diff.changed.push(DiffChange {
                    subsystem: key.0,
                    device: key.1.clone(),
                    profile: key.2.clone(),
                    setting_id: s.id.clone(),
                    display_name: s.display_name.clone(),
                    before: prev.value.clone(),
                    after: s.value.clone(),
                }),
                _ => {}
            }
        }
        for (id, s) in &base_settings {
            if !cur_settings.contains_key(id) {
                diff.removed.push(DiffEntry {
                    subsystem: key.0,
                    device: key.1.clone(),
                    profile: key.2.clone(),
                    setting_id: s.id.clone(),
                    display_name: s.display_name.clone(),
                    value: s.value.clone(),
                });
            }
        }
    }

    diff
}

type GroupKey = (Subsystem, String, String);

fn flatten_groups(snap: &ProfileSnapshot) -> std::collections::BTreeMap<GroupKey, &ProfileGroup> {
    let mut out = std::collections::BTreeMap::new();
    for (sub, groups) in &snap.providers {
        for group in groups {
            out.insert(
                (*sub, group.device.clone(), group.display_name.clone()),
                group,
            );
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::super::{Setting, SettingValue};
    use super::*;
    use std::collections::BTreeMap;

    fn make_snapshot(values: &[(&str, &str, &str, &str, SettingValue)]) -> ProfileSnapshot {
        let mut providers: BTreeMap<Subsystem, Vec<ProfileGroup>> = BTreeMap::new();
        for (sub, device, profile, id, value) in values {
            let sub = Subsystem::parse(sub).unwrap();
            let entry = providers.entry(sub).or_default();
            let group = entry.iter_mut().find(|g| g.device == *device && g.display_name == *profile);
            let group = match group {
                Some(g) => g,
                None => {
                    entry.push(ProfileGroup::new(sub, *device, *profile, "test"));
                    entry.last_mut().unwrap()
                }
            };
            group.push(Setting::info(*id, *id, value.clone()));
        }
        ProfileSnapshot {
            timestamp: 0,
            providers,
            errors: BTreeMap::new(),
        }
    }

    #[test]
    fn detects_added_removed_changed() {
        let baseline = make_snapshot(&[
            ("gpu", "GPU0", "Global", "ecc", SettingValue::Bool(false)),
            ("gpu", "GPU0", "Global", "power", SettingValue::Uint(350)),
            ("gpu", "GPU0", "Global", "doomed", SettingValue::Bool(true)),
        ]);
        let current = make_snapshot(&[
            ("gpu", "GPU0", "Global", "ecc", SettingValue::Bool(false)), // unchanged
            ("gpu", "GPU0", "Global", "power", SettingValue::Uint(450)), // changed
            ("gpu", "GPU0", "Global", "new_key", SettingValue::Int(42)), // added
            // doomed removed
        ]);
        let diff = diff_snapshots(&baseline, &current);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.changed.len(), 1);
        assert_eq!(diff.changed[0].setting_id, "power");
    }

    #[test]
    fn detects_added_removed_group() {
        let baseline = make_snapshot(&[
            ("gpu", "GPU0", "Global", "a", SettingValue::Bool(true)),
        ]);
        let current = make_snapshot(&[
            ("gpu", "GPU0", "Global", "a", SettingValue::Bool(true)),
            ("gpu", "GPU1", "Global", "x", SettingValue::Uint(1)),
        ]);
        let diff = diff_snapshots(&baseline, &current);
        assert_eq!(diff.added_groups.len(), 1);
        assert_eq!(diff.added_groups[0].device, "GPU1");
    }

    #[test]
    fn empty_diff_for_identical_snapshots() {
        let snap = make_snapshot(&[
            ("gpu", "GPU0", "Global", "a", SettingValue::Bool(true)),
            ("cpu", "CPU0", "Active", "pl1", SettingValue::Float(125.0)),
        ]);
        let diff = diff_snapshots(&snap, &snap);
        assert!(diff.is_empty());
        assert_eq!(diff.total_changes(), 0);
    }
}
