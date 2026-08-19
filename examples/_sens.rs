fn main() {
    match simonlib::sensors::SensorMonitor::new() {
        Ok(m) => {
            let s = m.sensors();
            println!("sensors: {}", s.len());
            for x in s.iter().take(4) {
                println!("  {:?}", x.name);
            }
        }
        Err(e) => println!("sensors failed: {e}"),
    }
    match simonlib::rapl::RaplMonitor::new() {
        Ok(_) => println!("rapl monitor constructed"),
        Err(e) => println!("rapl failed: {e}"),
    }
}
