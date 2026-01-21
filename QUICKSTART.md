# Silicon Monitor - Quick Start Guide# Quick Start Guide - simon



Welcome to Silicon Monitor! This guide will help you get started with comprehensive hardware monitoring in Rust.Get up and running with simon in under 5 minutes!



## Table of Contents## Prerequisites



1. [Installation](#installation)- Rust 1.70 or later ([install here](https://rustup.rs/))

2. [Basic Usage](#basic-usage)- NVIDIA hardware (Jetson device or desktop GPU)

3. [GPU Monitoring](#gpu-monitoring)

4. [Process Monitoring](#process-monitoring)## 1. Installation

5. [Network Monitoring](#network-monitoring)

6. [Terminal UI](#terminal-ui)### Option A: Use as a Library

7. [Examples](#examples)

8. [Troubleshooting](#troubleshooting)Add to your `Cargo.toml`:



## Installation```toml

[dependencies]

### Prerequisitessimon = { version = "0.1", features = ["nvml"] }

```

**Linux:**

```bash### Option B: Install CLI Tool

# Ubuntu/Debian

sudo apt install build-essential pkg-config libdrm-dev```bash

cargo install simon --features cli

# For NVIDIA support, ensure CUDA toolkit or driver is installed```

# (provides libnvidia-ml.so)

```### Option C: Build from Source



**Windows:**```bash

```bashgit clone https://github.com/nervosys/SiliconMonitor

# Install Visual Studio Build Toolscd SiliconMonitor

# For NVIDIA support, install CUDA toolkit or NVIDIA drivercargo build --release --features full

``````



### Build from Source## 2. First Program



```bashCreate `main.rs`:

# Clone the repository

git clone https://github.com/nervosys/SiliconMonitor```rust

cd SiliconMonitoruse simon::simon;



# Build with all featuresfn main() -> Result<(), Box<dyn std::error::Error>> {

cargo build --release --features full    // Create stats instance

    let mut stats = simon::new()?;

# Or build for specific GPU vendors    

cargo build --release --features nvidia      # NVIDIA only    // Get snapshot

cargo build --release --features amd         # AMD only    let snapshot = stats.snapshot()?;

cargo build --release --features intel       # Intel only    

cargo build --release --features nvidia,amd  # NVIDIA + AMD    // Print GPU info

```    println!("=== GPU Information ===");

    for (name, gpu) in &snapshot.gpus {

## Basic Usage        println!("{}: {:.1}% @ {} MHz", 

            name, 

### Add to Your Project            gpu.status.load, 

            gpu.frequency.current);

Add Silicon Monitor to your `Cargo.toml`:    }

    

```toml    // Print CPU info

[dependencies]    println!("\n=== CPU Information ===");

simon = { path = "../simon", features = ["full"] }    println!("Usage: {:.1}%", 100.0 - snapshot.cpu.total.idle);

    

# Or for specific vendors:    // Print memory info

# simon = { path = "../simon", features = ["nvidia"] }    println!("\n=== Memory Information ===");

```    println!("RAM: {:.1}%", snapshot.memory.ram_usage_percent());

    

### Import and Initialize    Ok(())

}

```rust```

use simon::{GpuCollection, ProcessMonitor, NetworkMonitor};

Run it:

fn main() -> Result<(), Box<dyn std::error::Error>> {

    // Auto-detect all available GPUs```bash

    let gpus = GpuCollection::auto_detect()?;cargo run

    ```

    // Create process monitor with GPU attribution

    let mut proc_monitor = ProcessMonitor::with_gpus(gpus)?;## 3. Using the CLI

    

    // Create network monitor```bash

    let mut net_monitor = NetworkMonitor::new()?;# Interactive monitoring (like htop)

    simon

    Ok(())

}# Show GPU information

```simon gpu



## GPU Monitoring# Show CPU information

simon cpu

### Detect All GPUs

# Show all stats as JSON

```rustsimon --format json all

use simon::gpu::{GpuCollection, Device};```



let gpus = GpuCollection::auto_detect()?;## 4. Common Tasks



println!("Found {} GPUs", gpus.device_count());### Monitor GPU Continuously



// Snapshot all GPUs at once```rust

for (idx, info) in gpus.snapshot_all()?.iter().enumerate() {use simon::simon;

    println!("GPU {}: {}", idx, info.static_info.name);use std::thread;

    println!("  Vendor: {:?}", info.static_info.vendor);use std::time::Duration;

    println!("  Memory: {} / {} MB",

        info.dynamic_info.memory.used / 1024 / 1024,fn main() -> Result<(), Box<dyn std::error::Error>> {

        info.static_info.memory_total / 1024 / 1024);    let mut stats = simon::with_interval(0.5)?;

    println!("  Utilization: {}%", info.dynamic_info.utilization.graphics);    

    println!("  Temperature: {}°C", info.dynamic_info.temperature.gpu);    loop {

    println!("  Power: {:.1}W", info.dynamic_info.power.current / 1000.0);        let snapshot = stats.snapshot()?;

}        

```        if let Some((name, gpu)) = snapshot.gpus.iter().next() {

            println!("\rGPU {}: {:.1}% @ {} MHz", 

### Query Specific GPU Properties                name, gpu.status.load, gpu.frequency.current);

        }

```rust        

use simon::gpu::{GpuCollection, Device};        thread::sleep(stats.interval());

    }

let gpus = GpuCollection::auto_detect()?;}

```

for device in gpus.gpus() {

    // Basic info### Export to JSON

    let name = device.name()?;

    let vendor = device.vendor();```rust

    use simon::simon;

    // Temperature

    if let Ok(temp) = device.temperature() {fn main() -> Result<(), Box<dyn std::error::Error>> {

        println!("{}: GPU={}°C, Memory={}°C",    let mut stats = simon::new()?;

            name, temp.gpu, temp.memory.unwrap_or(0));    let snapshot = stats.snapshot()?;

    }    

        // Serialize to JSON

    // Power    let json = serde_json::to_string_pretty(&snapshot)?;

    if let Ok(power) = device.power() {    println!("{}", json);

        println!("  Power: {:.1}W / {:.1}W (limit)",    

            power.current / 1000.0,    // Or save to file

            power.limit.unwrap_or(0) / 1000.0);    std::fs::write("stats.json", json)?;

    }    

        Ok(())

    // Clocks}

    if let Ok(clocks) = device.clocks() {```

        println!("  Clocks: GPU={}MHz, Memory={}MHz",

            clocks.graphics.unwrap_or(0),### Control GPU (Jetson Only)

            clocks.memory.unwrap_or(0));

    }```rust

    use simon::core::gpu::GpuStats;

    // Processes

    if let Ok(processes) = device.processes() {fn main() -> Result<(), Box<dyn std::error::Error>> {

        println!("  {} processes using this GPU", processes.len());    let mut gpu_stats = GpuStats::new();

    }    

}    // Enable 3D scaling (requires root on Jetson)

```    gpu_stats.set_3d_scaling("gpu", true)?;

    

## Process Monitoring    println!("3D scaling enabled!");

    

### List All Processes    Ok(())

}

```rust```

use simon::{ProcessMonitor, GpuCollection};

Run with:

let gpus = GpuCollection::auto_detect()?;```bash

let mut monitor = ProcessMonitor::with_gpus(gpus)?;sudo cargo run

```

let processes = monitor.processes()?;

println!("Total processes: {}", processes.len());## 5. Platform-Specific Notes



for proc in processes.iter().take(10) {### On Jetson

    println!("{} (PID {}): CPU={:.1}%, Memory={} MB",

        proc.name,```bash

        proc.pid,# Build

        proc.cpu_percent,cargo build --release

        proc.memory_bytes / 1024 / 1024);

}# Run (no root needed for monitoring)

```./target/release/simon



### Find Top GPU Consumers# Run with control features (needs root)

sudo ./target/release/simon

```rust```

use simon::{ProcessMonitor, GpuCollection};

### On Linux Desktop

let gpus = GpuCollection::auto_detect()?;

let mut monitor = ProcessMonitor::with_gpus(gpus)?;```bash

# Ensure NVML is available

// Get processes sorted by GPU memory usagewhich nvidia-smi

let top_gpu = monitor.processes_by_gpu_memory()?;

# Build with NVML support

println!("Top 10 GPU consumers:");cargo build --release --features nvml

for proc in top_gpu.iter().take(10) {

    println!("{} (PID {}): {} MB GPU memory on {} GPUs",# Run

        proc.name,./target/release/simon

        proc.pid,```

        proc.total_gpu_memory_bytes / 1024 / 1024,

        proc.gpu_indices.len());### On Windows

    

    // Show per-GPU breakdown```powershell

    for (gpu_idx, &mem) in &proc.gpu_memory_per_device {# Build

        println!("  GPU {}: {} MB", gpu_idx, mem / 1024 / 1024);cargo build --release --features nvml

    }

}# Run

```.\target\release\simon.exe

```

## Network Monitoring

## 6. Troubleshooting

### List All Interfaces

### "NVML not found"

```rust

use simon::NetworkMonitor;```bash

# Install CUDA toolkit

let mut monitor = NetworkMonitor::new()?;sudo apt-get install nvidia-cuda-toolkit

let interfaces = monitor.interfaces()?;

# Or build without NVML

for iface in interfaces {cargo build --no-default-features

    println!("{}: {} / {}",```

        iface.name,

        if iface.is_up { "UP" } else { "DOWN" },### "Permission denied" (Jetson)

        if iface.is_running { "RUNNING" } else { "STOPPED" }

    );For GPU control operations, you need root:

    println!("  RX: {} MB ({} packets)",

        iface.rx_mb(),```bash

        iface.rx_packets);# Run with sudo

    println!("  TX: {} MB ({} packets)",sudo -E cargo run

        iface.tx_mb(),

        iface.tx_packets);# Or set capabilities

    sudo setcap cap_sys_admin+ep ./target/release/your-app

    if let Some(speed) = iface.speed_mbps {```

        println!("  Speed: {} Mbps", speed);

    }### "Failed to read /proc/stat"

}

```On some systems, you may need to enable procfs access. This is usually not an issue on standard Linux systems.



### Monitor Bandwidth Rates## 7. Next Steps



```rust- Read the [full README](README-RUST.md)

use simon::NetworkMonitor;- Check out [examples](examples/)

use std::{thread, time::Duration};- Read the [migration guide](MIGRATION.md) if coming from Python

- View [API documentation](https://docs.rs/simon)

let mut monitor = NetworkMonitor::new()?;

## 8. Examples

// Establish baseline

let _ = monitor.interfaces()?;### Example 1: GPU Temperature Monitoring



loop {```rust

    thread::sleep(Duration::from_secs(1));use simon::simon;

    

    let interfaces = monitor.interfaces()?;fn main() -> Result<(), Box<dyn std::error::Error>> {

        let mut stats = simon::new()?;

    println!("\n=== Network Bandwidth ===");    let snapshot = stats.snapshot()?;

    for iface in interfaces {    

        if iface.is_active() {    for (name, gpu) in &snapshot.gpus {

            let (rx_rate, tx_rate) = monitor.bandwidth_rate(&iface.name, &iface);        if let Some(temp) = gpu.status.temperature {

            println!("{}: ↓{:.2} MB/s ↑{:.2} MB/s",            println!("GPU {}: {:.1}°C", name, temp);

                iface.name,            

                rx_rate / 1_000_000.0,            if temp > 80.0 {

                tx_rate / 1_000_000.0);                println!("⚠️  Warning: High temperature!");

        }            }

    }        }

}    }

```    

    Ok(())

## Terminal UI}

```

Silicon Monitor includes a beautiful terminal user interface (TUI) for real-time monitoring.

### Example 2: Power Monitoring

### Running the TUI

```rust

```bashuse simon::simon;

# Build and run

cargo run --release --features cli --example tuifn main() -> Result<(), Box<dyn std::error::Error>> {

    let mut stats = simon::new()?;

# Or after building    let snapshot = stats.snapshot()?;

./target/release/examples/tui    

```    println!("Total Power: {:.2}W", snapshot.power.total_watts());

    

### TUI Controls    println!("\nPower Rails:");

    for (name, rail) in &snapshot.power.rails {

- **Tab Navigation**: `←`/`→` arrow keys or `1`-`5` number keys        if rail.online {

- **Quit**: `Q` or `Esc`            let watts = rail.power as f64 / 1000.0;

- **Refresh**: Automatic (1-second interval)            println!("  {}: {:.2}W", name, watts);

        }

### TUI Features    }

    

- **Overview Tab** (1): System summary with hostname, uptime, CPU, memory    Ok(())

- **CPU Tab** (2): Per-core utilization and frequency (placeholder)}

- **GPU Tab** (3): Live GPU metrics for all detected GPUs```

  - Utilization graphs (60-second history)

  - Memory usage gauges### Example 3: System Health Check

  - Temperature, power, clocks

  - Multi-vendor support (NVIDIA/AMD/Intel)```rust

- **Memory Tab** (4): RAM and swap usage (placeholder)use simon::simon;

- **System Tab** (5): System information (placeholder)

fn main() -> Result<(), Box<dyn std::error::Error>> {

## Examples    let mut stats = simon::new()?;

    let snapshot = stats.snapshot()?;

The `examples/` directory contains comprehensive demonstrations. Run any example with:    

    println!("=== System Health Check ===\n");

```bash    

# GPU monitoring    // Check GPU

cargo run --release --features nvidia --example gpu_monitor    for (name, gpu) in &snapshot.gpus {

cargo run --release --features full --example all_gpus        print!("GPU {}: ", name);

        if gpu.status.load > 90.0 {

# Process monitoring            println!("🔴 Overloaded ({:.1}%)", gpu.status.load);

cargo run --release --features nvidia --example process_monitor        } else if gpu.status.load > 70.0 {

            println!("🟡 High load ({:.1}%)", gpu.status.load);

# Network monitoring        } else {

cargo run --release --example network_monitor            println!("🟢 OK ({:.1}%)", gpu.status.load);

        }

# TUI    }

cargo run --release --features cli --example tui    

```    // Check CPU

    let cpu_usage = 100.0 - snapshot.cpu.total.idle;

## Troubleshooting    print!("CPU: ");

    if cpu_usage > 90.0 {

### NVIDIA: "Failed to initialize NVML"        println!("🔴 Overloaded ({:.1}%)", cpu_usage);

    } else if cpu_usage > 70.0 {

```bash        println!("🟡 High load ({:.1}%)", cpu_usage);

# Linux: Install NVIDIA driver or CUDA toolkit    } else {

sudo apt install nvidia-driver-535  # Ubuntu/Debian        println!("🟢 OK ({:.1}%)", cpu_usage);

    }

# Check if libnvidia-ml.so is available    

ldconfig -p | grep nvidia-ml    // Check Memory

```    let ram_usage = snapshot.memory.ram_usage_percent();

    print!("Memory: ");

### AMD: No GPUs detected    if ram_usage > 90.0 {

        println!("🔴 Critical ({:.1}%)", ram_usage);

```bash    } else if ram_usage > 70.0 {

# Ensure amdgpu driver is loaded        println!("🟡 High ({:.1}%)", ram_usage);

lsmod | grep amdgpu    } else {

        println!("🟢 OK ({:.1}%)", ram_usage);

# Check for AMD devices    }

ls -la /sys/class/drm/card*/device/vendor    

# Should show "0x1002" for AMD    // Check Temperature

```    if let Some(max_temp) = snapshot.temperature.max_temp() {

        print!("Temperature: ");

### Permissions: Access denied        if max_temp > 80.0 {

            println!("🔴 Hot ({:.1}°C)", max_temp);

```bash        } else if max_temp > 60.0 {

# Run with sudo for full access            println!("🟡 Warm ({:.1}°C)", max_temp);

sudo ./target/release/examples/gpu_monitor        } else {

            println!("🟢 OK ({:.1}°C)", max_temp);

# Or add user to video group        }

sudo usermod -a -G video $USER    }

```    

    Ok(())

## Next Steps}

```

- Read the [README](README.md) for comprehensive feature list

- Check [PROJECT_SUMMARY.md](PROJECT_SUMMARY.md) for implementation details## Need Help?

- Explore the [examples/](examples/) directory

- Generate API docs: `cargo doc --features full --no-deps --open`- 📖 [Full Documentation](README-RUST.md)

- 🐛 [Report Issues](https://github.com/nervosys/SiliconMonitor/issues)

---- 💬 [Discussions](https://github.com/nervosys/SiliconMonitor/discussions)



**Happy Monitoring!** 🔬---


**Happy Monitoring! 🚀**

