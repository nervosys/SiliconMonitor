//! Reproduce the two conditions the GUI's AI tab gates on, outside the GUI.
//!
//! `draw_ai_assistant_tab` shows "AI backend not connected" whenever
//! `agent.is_some() && silicon_monitor.is_some()` is false, so this reports each
//! half separately — the message names the backend but either half can cause it.

fn main() {
    let t0 = std::time::Instant::now();
    match simonlib::agent::AgentConfig::auto_detect() {
        Ok(config) => {
            println!(
                "auto_detect: ok in {:?} -> backend {:?}",
                t0.elapsed(),
                config.backend.as_ref().map(|b| b.backend_type.clone())
            );
            match simonlib::agent::Agent::new(config) {
                Ok(_) => println!("Agent::new: ok"),
                Err(e) => println!("Agent::new: ERR {e}"),
            }
        }
        Err(e) => println!("auto_detect: ERR after {:?}: {e}", t0.elapsed()),
    }

    let t1 = std::time::Instant::now();
    match simonlib::SiliconMonitor::new() {
        Ok(m) => println!(
            "SiliconMonitor::new: ok in {:?}, {} GPU(s)",
            t1.elapsed(),
            m.gpus().len()
        ),
        Err(e) => println!("SiliconMonitor::new: ERR after {:?}: {e}", t1.elapsed()),
    }
}
