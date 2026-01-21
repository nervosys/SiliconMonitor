use simon::ProcessMonitor;

fn main() {
    let mut monitor = ProcessMonitor::new().expect("Failed to create monitor");
    let processes = monitor.enumerate_processes().expect("Failed to enumerate");
    println!("Found {} processes", processes.len());
    
    // Show first 10 with new fields
    for p in processes.iter().take(10) {
        println!("PID: {}, Name: {}, Threads: {}, I/O: {}R/{}W, Handles: {}, Parent: {:?}",
            p.pid, p.name, p.thread_count, p.io_read_bytes, p.io_write_bytes, p.handle_count, p.parent_pid);
    }
}
