//! Headless rendering harness for the GUI.
//!
//! egui draws into a `Context` that needs no window, so a test can run real widget
//! code and read back the text that was actually painted. That matters here because
//! the alternative — screenshotting a live window — proved unusable: on a
//! multi-monitor setup the window handle resolves to transient popups and the
//! captured rect is whatever happened to be on top.
//!
//! More importantly, a screenshot cannot answer the question that actually matters.
//! The Profiles tab rendered all 19 of its groups perfectly while every heading was
//! drawn in the panel colour; a human looking at the window and a naive capture both
//! reported an empty tab. Reading the painted galleys distinguishes "this text was
//! never emitted" from "this text was emitted and could not be seen", which are
//! different bugs with different fixes.

use egui::Context;

/// Every string painted by `body`, in paint order.
///
/// Walks the tessellated output rather than instrumenting the widgets, so it sees
/// what egui actually drew — including text emitted by widgets this module knows
/// nothing about.
pub fn painted_text(ctx: &Context, body: impl FnOnce(&mut egui::Ui)) -> Vec<String> {
    let mut out = Vec::new();
    // `Context::run` wants an `FnMut` because a frame can be re-entered; the body is
    // an `FnOnce`, so hand it over on the first call and leave nothing behind.
    let mut body = Some(body);
    let full_output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(body) = body.take() {
                body(ui);
            }
        });
    });

    for clipped in &full_output.shapes {
        collect_shape_text(&clipped.shape, &mut out);
    }
    out
}

/// Text painted by `body`, joined into one haystack for substring assertions.
pub fn painted_blob(ctx: &Context, body: impl FnOnce(&mut egui::Ui)) -> String {
    painted_text(ctx, body).join("\n")
}

fn collect_shape_text(shape: &egui::Shape, out: &mut Vec<String>) {
    match shape {
        egui::Shape::Text(text) => {
            let s = text.galley.text();
            if !s.trim().is_empty() {
                out.push(s.to_string());
            }
        }
        // Widgets compose, so text is routinely nested inside a Vec shape.
        egui::Shape::Vec(shapes) => {
            for s in shapes {
                collect_shape_text(s, out);
            }
        }
        _ => {}
    }
}

/// A context with the app's own theme applied, so tests exercise the real palette
/// rather than egui's defaults.
pub fn themed_context() -> Context {
    let ctx = Context::default();
    super::theme::apply_cyber_theme(&ctx);
    ctx
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::RichText;

    #[test]
    fn the_harness_sees_text_that_was_painted() {
        let ctx = themed_context();
        let blob = painted_blob(&ctx, |ui| {
            ui.label("a plain label");
            ui.label(RichText::new("a strong label").strong());
        });
        assert!(blob.contains("a plain label"), "got: {blob}");
        assert!(blob.contains("a strong label"), "got: {blob}");
    }

    #[test]
    fn the_harness_sees_text_nested_inside_widgets() {
        let ctx = themed_context();
        let blob = painted_blob(&ctx, |ui| {
            egui::CollapsingHeader::new("outer heading")
                .default_open(true)
                .show(ui, |ui| {
                    ui.label("nested body text");
                });
        });
        assert!(blob.contains("outer heading"), "got: {blob}");
        assert!(blob.contains("nested body text"), "got: {blob}");
    }

    /// The harness must not report text that was never drawn, or it would mask the
    /// very regressions it exists to catch.
    #[test]
    fn the_harness_does_not_invent_text() {
        let ctx = themed_context();
        let blob = painted_blob(&ctx, |ui| {
            ui.label("only this");
        });
        assert!(!blob.contains("not drawn"), "got: {blob}");
    }

    /// Reading painted text separates "never emitted" from "emitted but invisible" —
    /// the distinction a screenshot cannot make, and the one that made the Profiles
    /// tab look dead while it was rendering correctly.
    #[test]
    fn invisible_text_is_still_painted_text() {
        let ctx = themed_context();
        let panel_fill = ctx.style().visuals.panel_fill;
        let blob = painted_blob(&ctx, |ui| {
            // Deliberately drawn in the background colour, as the old theme did.
            ui.label(RichText::new("camouflaged").color(panel_fill));
        });
        assert!(
            blob.contains("camouflaged"),
            "the harness should see text regardless of its colour, so that a \
             contrast bug is diagnosable as distinct from a missing-widget bug"
        );
    }
}

#[cfg(test)]
mod ontology_binding_tests {
    use super::*;
    use crate::gui::widgets::{domain_section_title, SectionHeader};
    use crate::ontology::labels;

    /// The GUI must paint the ontology's spelling of a domain, not its own.
    ///
    /// Asserted through the harness rather than by calling the helper directly:
    /// a helper that returns the right string but is never reached would pass a
    /// unit test and leave the screen unchanged.
    #[test]
    fn section_headings_paint_the_ontology_domain_spelling() {
        let ctx = themed_context();
        let title = domain_section_title("gpu", "Utilization");
        let blob = painted_blob(&ctx, |ui| {
            ui.add(SectionHeader::new(&title));
        });
        assert!(
            blob.contains("GPU Utilization"),
            "expected the ontology spelling in painted output, got: {blob}"
        );
        // And the domain word is one an agent can actually query.
        assert!(labels::is_known_domain("gpu"));
    }

    /// Text painted by the GUI must map back to ids, so an agent handed a screenshot
    /// description by a user can turn it into a query.
    #[test]
    fn painted_labels_resolve_to_entity_ids() {
        let ctx = themed_context();
        let label = labels::short_label("memory.total");
        let blob = painted_blob(&ctx, |ui| {
            ui.label(&label);
        });
        assert!(blob.contains("Total"), "got: {blob}");

        let ids = labels::ids_for_label(&label);
        assert!(
            ids.iter().any(|id| id == "memory.total"),
            "the label the GUI painted does not map back to memory.total: {ids:?}"
        );
    }

    /// The regression that made Profiles look dead, caught at the level it occurred:
    /// strong text is emitted *and* distinguishable from the surface behind it.
    #[test]
    fn strong_headings_are_both_painted_and_legible() {
        let ctx = themed_context();
        let blob = painted_blob(&ctx, |ui| {
            ui.label(egui::RichText::new("Group Heading").strong());
        });
        assert!(
            blob.contains("Group Heading"),
            "strong text was not painted at all: {blob}"
        );

        let visuals = ctx.style().visuals.clone();
        let strong = visuals.strong_text_color();
        let panel = visuals.panel_fill;
        let distance = (strong.r() as i32 - panel.r() as i32).abs()
            + (strong.g() as i32 - panel.g() as i32).abs()
            + (strong.b() as i32 - panel.b() as i32).abs();
        assert!(
            distance > 60,
            "strong text {strong:?} is painted but indistinguishable from the panel \
             {panel:?} — the Profiles failure mode"
        );
    }
}
