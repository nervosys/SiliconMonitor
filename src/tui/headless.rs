//! Headless rendering harness for the TUI.
//!
//! The GUI counterpart in `crate::gui::headless` exists because screenshots could
//! not distinguish "this text was never drawn" from "this text was drawn in an
//! unreadable colour". The TUI has the same problem in a different form: it renders
//! into an alternate screen under raw mode, so the only way to see what it produced
//! was to look at a terminal — which no test can do, and which cannot be diffed.
//!
//! `ratatui::backend::TestBackend` renders into an in-memory buffer instead. Reading
//! that buffer is the terminal equivalent of walking the GUI's painted galleys, and
//! it is what lets an agent — or a test — assert on what a human would see without
//! a terminal being involved at all.

use ratatui::backend::TestBackend;
use ratatui::Terminal;

use super::app::App;

/// Default geometry for a headless frame.
///
/// Wide enough that the tab bar does not wrap, which would split words across cells
/// and make substring assertions fail for reasons unrelated to what is being tested.
pub const DEFAULT_WIDTH: u16 = 200;
pub const DEFAULT_HEIGHT: u16 = 50;

/// Render one frame of `app` and return the visible text, one entry per row.
///
/// Trailing blanks are trimmed per row: a buffer is a fixed grid, so every row is
/// padded to full width and un-trimmed output would be mostly spaces.
pub fn render_rows(app: &App, width: u16, height: u16) -> Vec<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("TestBackend cannot fail to initialise");
    terminal
        .draw(|f| super::ui::draw(f, app))
        .expect("drawing into an in-memory buffer cannot fail");

    let buffer = terminal.backend().buffer();
    let mut rows = Vec::with_capacity(height as usize);
    for y in 0..height {
        let mut row = String::with_capacity(width as usize);
        for x in 0..width {
            row.push_str(buffer[(x, y)].symbol());
        }
        rows.push(row.trim_end().to_string());
    }
    rows
}

/// One frame as a single string, for substring assertions.
pub fn render_frame(app: &App) -> String {
    render_rows(app, DEFAULT_WIDTH, DEFAULT_HEIGHT).join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Building a real `App` enumerates hardware. Slow, but a mock would test the
    /// mock.
    fn app() -> App {
        App::new_fast().expect("the TUI app must be constructible headlessly")
    }

    #[test]
    fn a_frame_renders_and_contains_the_tab_bar() {
        let app = app();
        let frame = render_frame(&app);
        assert!(
            !frame.trim().is_empty(),
            "the TUI rendered an entirely blank frame"
        );
        // The tab bar is the one element present on every tab.
        assert!(
            frame.contains("Overview"),
            "the tab bar is missing from the frame:\n{frame}"
        );
    }

    /// The harness must see real content, not just chrome — otherwise it would pass
    /// against a TUI that drew borders around nothing.
    #[test]
    fn the_overview_tab_renders_actual_readings() {
        let mut app = app();
        app.selected_tab = 0;
        let frame = render_frame(&app);
        assert!(
            frame.contains("CPU") || frame.contains("Memory"),
            "the Overview tab shows no subsystem at all:\n{frame}"
        );
    }

    /// Every tab must render without panicking and produce something.
    ///
    /// A tab that panics takes the whole TUI down, and one that renders blank is
    /// indistinguishable to a user from one that is broken — the failure that
    /// started this whole thread in the GUI.
    #[test]
    fn every_tab_renders_without_panicking() {
        let mut app = app();
        let tab_count = app.tabs.len();
        assert!(tab_count > 1, "expected several tabs, found {tab_count}");

        for index in 0..tab_count {
            app.selected_tab = index;
            let frame = render_frame(&app);
            assert!(
                !frame.trim().is_empty(),
                "tab {index} ({}) rendered a blank frame",
                app.tabs[index]
            );
            // The tab bar survives on every tab, so its absence means the layout
            // collapsed rather than the content simply being empty.
            assert!(
                frame.contains("Overview"),
                "tab {index} ({}) lost the tab bar, so the layout collapsed:\n{frame}",
                app.tabs[index]
            );
        }
    }

    /// The TUI's own vocabulary must match the ontology, asserted against rendered
    /// output rather than against the source list — a tab renamed at the point of
    /// drawing would pass a check that only read `app.tabs`.
    #[test]
    fn rendered_tab_labels_use_the_ontology_spelling() {
        use crate::ontology::labels;

        let app = app();
        let frame = render_frame(&app);

        for tab in &app.tabs {
            let lowered = tab.to_ascii_lowercase();
            if labels::is_known_domain(&lowered) {
                let canonical = labels::domain_label(&lowered);
                assert!(
                    frame.contains(&canonical),
                    "the TUI renders the domain {lowered:?} but the frame does not \
                     contain its ontology spelling {canonical:?}:\n{frame}"
                );
            }
        }
    }

    /// A frame is a fixed grid, so geometry must be honoured exactly — an agent
    /// diffing two frames needs them to be the same shape.
    #[test]
    fn frame_geometry_matches_the_requested_size() {
        let app = app();
        let rows = render_rows(&app, 120, 30);
        assert_eq!(rows.len(), 30, "wrong number of rows");
        // Rows are right-trimmed, so assert the untrimmed width instead by checking
        // no row exceeds the requested width.
        for (i, row) in rows.iter().enumerate() {
            assert!(
                row.chars().count() <= 120,
                "row {i} is {} cells wide, exceeding the requested 120",
                row.chars().count()
            );
        }
    }
}
