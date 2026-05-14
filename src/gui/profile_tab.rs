//! Profile inspector tab — NVIDIA Profile Inspector / XTU / Ryzen Master /
//! nvme-cli style read-only enumeration of vendor driver settings.

use egui::RichText;

use super::app::SiliconMonitorApp;
use super::theme::CyberColors;

impl SiliconMonitorApp {
    pub(super) fn draw_profiles_tab(&mut self, ui: &mut egui::Ui) {
        use crate::profile::{cache::CachedProfileInspector, SettingRisk, Subsystem};

        ui.horizontal(|ui| {
            ui.heading(RichText::new("🛠 Hardware Profile Inspector").color(CyberColors::CYAN));
            ui.label(
                RichText::new(
                    "  read-only — NVIDIA Profile Inspector / Intel XTU / AMD Ryzen Master / nvme-cli",
                )
                .color(CyberColors::TEXT_SECONDARY)
                .italics(),
            );
        });

        ui.horizontal(|ui| {
            let refresh_clicked = ui
                .button(RichText::new("🔄 Refresh").color(CyberColors::CYAN))
                .clicked();
            if refresh_clicked || self.profile_snapshot.is_none() {
                let mut inspector = CachedProfileInspector::new();
                if refresh_clicked {
                    inspector.invalidate(None);
                }
                self.profile_snapshot = Some(inspector.snapshot_all());
            }
            ui.separator();
            ui.label("Filter:");
            ui.text_edit_singleline(&mut self.profile_filter);
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "cache: {} hit / {} miss",
                    crate::profile::cache::CACHE_STATS.hits(),
                    crate::profile::cache::CACHE_STATS.misses()
                ))
                .small()
                .color(CyberColors::TEXT_SECONDARY),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Subsystems:");
            if ui
                .selectable_label(self.profile_subsystem_filter.is_none(), "All")
                .clicked()
            {
                self.profile_subsystem_filter = None;
            }
            for sub in Subsystem::ALL {
                let selected = self.profile_subsystem_filter == Some(*sub);
                if ui.selectable_label(selected, sub.as_str()).clicked() {
                    self.profile_subsystem_filter = if selected { None } else { Some(*sub) };
                }
            }
        });

        ui.separator();

        let Some(snapshot) = self.profile_snapshot.clone() else {
            ui.label("Loading profile snapshot…");
            return;
        };

        // ── Deviations panel ──────────────────────────────────────────────
        let deviations = crate::profile::deviation::deviations_from_default(&snapshot);
        let heading = if deviations.is_empty() {
            RichText::new(format!(
                "▾ Deviations from default · 0 — at stock"
            ))
            .color(egui::Color32::from_rgb(100, 220, 100))
        } else {
            RichText::new(format!(
                "▾ Deviations from default · {}",
                deviations.len()
            ))
            .color(egui::Color32::from_rgb(240, 200, 80))
            .strong()
        };
        egui::CollapsingHeader::new(heading)
            .default_open(!deviations.is_empty())
            .id_salt("profile_deviations_panel")
            .show(ui, |ui| {
                if deviations.is_empty() {
                    ui.label(
                        RichText::new("All settings with a declared default are at their default value.")
                            .color(CyberColors::TEXT_SECONDARY),
                    );
                } else {
                    egui::Grid::new("profile_deviations_grid")
                        .striped(true)
                        .num_columns(4)
                        .show(ui, |ui| {
                            ui.label(RichText::new("risk").strong());
                            ui.label(RichText::new("setting").strong());
                            ui.label(RichText::new("default").strong());
                            ui.label(RichText::new("current").strong());
                            ui.end_row();
                            for d in &deviations {
                                let risk_color = match d.risk {
                                    crate::profile::SettingRisk::Dangerous => {
                                        egui::Color32::from_rgb(255, 90, 90)
                                    }
                                    crate::profile::SettingRisk::Moderate => {
                                        egui::Color32::from_rgb(240, 200, 80)
                                    }
                                    crate::profile::SettingRisk::Safe => {
                                        egui::Color32::from_rgb(100, 220, 100)
                                    }
                                    crate::profile::SettingRisk::Informational => {
                                        CyberColors::TEXT_SECONDARY
                                    }
                                };
                                ui.label(
                                    RichText::new(format!("{:?}", d.risk))
                                        .color(risk_color)
                                        .small(),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "[{}] {} :: {}",
                                        d.subsystem.as_str(),
                                        d.device,
                                        d.display_name
                                    )),
                                );
                                ui.label(
                                    RichText::new(d.default.to_string())
                                        .color(CyberColors::TEXT_SECONDARY),
                                );
                                ui.label(
                                    RichText::new(d.current.to_string())
                                        .strong()
                                        .color(risk_color),
                                );
                                ui.end_row();
                            }
                        });
                }
            });

        // ── Audit log tail panel ─────────────────────────────────────────
        let audit_path = crate::profile::apply::audit_log_path();
        let audit_text = std::fs::read_to_string(&audit_path).unwrap_or_default();
        let audit_lines: Vec<&str> = audit_text.lines().collect();
        let tail_n = 12usize.min(audit_lines.len());
        let tail = if audit_lines.len() > tail_n {
            &audit_lines[audit_lines.len() - tail_n..]
        } else {
            &audit_lines[..]
        };
        let audit_heading = RichText::new(format!(
            "▾ Apply audit log · last {} entr{}",
            tail.len(),
            if tail.len() == 1 { "y" } else { "ies" }
        ))
        .color(CyberColors::CYAN);
        egui::CollapsingHeader::new(audit_heading)
            .default_open(false)
            .id_salt("profile_audit_panel")
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!("source: {}", audit_path.display()))
                        .color(CyberColors::TEXT_SECONDARY)
                        .italics()
                        .small(),
                );
                if tail.is_empty() {
                    ui.label(
                        RichText::new("(no entries — no apply attempts have been made)")
                            .color(CyberColors::TEXT_SECONDARY),
                    );
                } else {
                    egui::Grid::new("profile_audit_grid")
                        .striped(true)
                        .num_columns(4)
                        .show(ui, |ui| {
                            ui.label(RichText::new("when").strong());
                            ui.label(RichText::new("status").strong());
                            ui.label(RichText::new("setting").strong());
                            ui.label(RichText::new("requested").strong());
                            ui.end_row();
                            for line in tail {
                                if let Ok(o) = serde_json::from_str::<
                                    crate::profile::apply::ApplyOutcome,
                                >(line)
                                {
                                    let (status_str, status_color) = match o.status {
                                        crate::profile::apply::ApplyStatus::Applied => (
                                            "applied",
                                            egui::Color32::from_rgb(100, 220, 100),
                                        ),
                                        crate::profile::apply::ApplyStatus::Refused => (
                                            "refused",
                                            egui::Color32::from_rgb(255, 90, 90),
                                        ),
                                        crate::profile::apply::ApplyStatus::Failed => (
                                            "failed",
                                            egui::Color32::from_rgb(255, 90, 90),
                                        ),
                                        crate::profile::apply::ApplyStatus::NotWritable => (
                                            "not writable",
                                            egui::Color32::from_rgb(240, 200, 80),
                                        ),
                                        crate::profile::apply::ApplyStatus::NeedsConfirm => (
                                            "needs confirm",
                                            egui::Color32::from_rgb(240, 200, 80),
                                        ),
                                    };
                                    ui.label(
                                        RichText::new(o.timestamp.to_string())
                                            .color(CyberColors::TEXT_SECONDARY)
                                            .small(),
                                    );
                                    ui.label(RichText::new(status_str).color(status_color));
                                    ui.label(RichText::new(o.setting_id).monospace());
                                    ui.label(RichText::new(o.requested.to_string()).strong());
                                    ui.end_row();
                                }
                            }
                        });
                }
            });

        ui.separator();

        let filter_lc = self.profile_filter.to_ascii_lowercase();
        let subsystem_filter = self.profile_subsystem_filter;
        let mut total_groups = 0usize;
        let mut total_settings = 0usize;

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for (sub, groups) in &snapshot.providers {
                    if let Some(sel) = subsystem_filter {
                        if sel != *sub {
                            continue;
                        }
                    }
                    if groups.is_empty() {
                        continue;
                    }
                    let heading = RichText::new(format!("▾ {}", sub.as_str().to_uppercase()))
                        .color(CyberColors::CYAN)
                        .strong();
                    egui::CollapsingHeader::new(heading)
                        .default_open(true)
                        .id_salt(format!("profile_sub_{}", sub.as_str()))
                        .show(ui, |ui| {
                            for group in groups {
                                let visible_settings: Vec<_> = group
                                    .settings
                                    .iter()
                                    .filter(|s| {
                                        if filter_lc.is_empty() {
                                            return true;
                                        }
                                        let hay = format!(
                                            "{} {} {} {}",
                                            s.id,
                                            s.display_name,
                                            s.description.as_deref().unwrap_or(""),
                                            s.value
                                        )
                                        .to_ascii_lowercase();
                                        hay.contains(&filter_lc)
                                    })
                                    .collect();
                                if visible_settings.is_empty() && !filter_lc.is_empty() {
                                    continue;
                                }
                                total_groups += 1;
                                total_settings += visible_settings.len();
                                let group_heading = RichText::new(format!(
                                    "  ▸ {}  —  {}",
                                    group.device, group.display_name
                                ))
                                .strong();
                                egui::CollapsingHeader::new(group_heading)
                                    .default_open(false)
                                    .id_salt(format!(
                                        "profile_grp_{}_{}",
                                        sub.as_str(),
                                        group.device
                                    ))
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(format!("source: {}", group.source))
                                                .color(CyberColors::TEXT_SECONDARY)
                                                .italics(),
                                        );
                                        let grid_id = format!(
                                            "profile_grid_{}_{}",
                                            sub.as_str(),
                                            group.device
                                        );
                                        egui::Grid::new(grid_id)
                                            .striped(true)
                                            .num_columns(3)
                                            .show(ui, |ui| {
                                                for s in visible_settings {
                                                    let risk_color = match s.risk {
                                                        SettingRisk::Informational => {
                                                            CyberColors::TEXT_SECONDARY
                                                        }
                                                        SettingRisk::Safe => {
                                                            egui::Color32::from_rgb(100, 220, 100)
                                                        }
                                                        SettingRisk::Moderate => {
                                                            egui::Color32::from_rgb(240, 200, 80)
                                                        }
                                                        SettingRisk::Dangerous => {
                                                            egui::Color32::from_rgb(255, 90, 90)
                                                        }
                                                    };
                                                    ui.label(
                                                        RichText::new(&s.display_name)
                                                            .color(risk_color),
                                                    );
                                                    let unit = s
                                                        .unit
                                                        .as_deref()
                                                        .map(|u| format!(" {}", u))
                                                        .unwrap_or_default();
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "{}{}",
                                                            s.value, unit
                                                        ))
                                                        .strong(),
                                                    );
                                                    ui.label(
                                                        RichText::new(s.id.clone())
                                                            .color(CyberColors::TEXT_SECONDARY)
                                                            .italics()
                                                            .monospace(),
                                                    );
                                                    ui.end_row();
                                                }
                                            });
                                        for n in &group.notes {
                                            ui.label(
                                                RichText::new(format!("• {}", n))
                                                    .color(CyberColors::TEXT_SECONDARY)
                                                    .italics(),
                                            );
                                        }
                                    });
                            }
                        });
                }
                ui.separator();
                ui.label(
                    RichText::new(format!(
                        "Showing {} group(s) · {} setting(s)",
                        total_groups, total_settings
                    ))
                    .color(CyberColors::TEXT_SECONDARY),
                );
            });
    }
}
