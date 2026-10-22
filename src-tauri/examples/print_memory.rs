//! Prints what the RAM display reads for this very process next to what the OS says, to check the units on each
//! platform:  cargo run --example print_memory
//! Linux: compared with VmRSS in /proc/self/status. Other systems: just the reading.

fn main() {
    // Use some memory so the numbers aren't tiny.
    let block: Vec<u8> = vec![1; 200 * 1024 * 1024];
    std::hint::black_box(&block);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let usage = rt.block_on(resincraft_lib::core::memory::usage());
    let mb = |b: u64| b as f64 / 1048576.0;
    println!("ResinCraft reads: {:.1} MB ({} process(es))", mb(usage.launcher), usage.launcher_processes);
    println!("This PC: {:.1} MB used of {:.1} MB", mb(usage.system_used), mb(usage.system_total));
    #[cfg(target_os = "linux")]
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        if let Some(line) = status.lines().find(|l| l.starts_with("VmRSS:")) {
            println!("Linux says:       {}", line);
        }
    }
}
