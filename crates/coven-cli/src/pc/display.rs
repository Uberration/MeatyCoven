use crate::pc::diagnostics::{DiskInfo, ProcessInfo, SystemSnapshot};

pub enum OutputFormat {
    Compact,
    Json,
    Verbose,
}

fn health_indicator(pct: f32, warn: f32, crit: f32) -> &'static str {
    if pct >= crit {
        "🔴"
    } else if pct >= warn {
        "🟡"
    } else {
        "🟢"
    }
}

pub fn print_status(snap: &SystemSnapshot, _format: &OutputFormat) {
    let cpu_ind = health_indicator(snap.cpu_usage_pct, 70.0, 90.0);
    let mem_pct = if snap.memory_total_mb > 0 {
        snap.memory_used_mb as f32 / snap.memory_total_mb as f32 * 100.0
    } else {
        0.0
    };
    let mem_ind = health_indicator(mem_pct, 70.0, 90.0);
    println!(
        "{cpu_ind} CPU {:.1}%  {mem_ind} RAM {}/{} MB  uptime {}",
        snap.cpu_usage_pct,
        snap.memory_used_mb,
        snap.memory_total_mb,
        format_uptime(snap.uptime_secs),
    );
}

pub fn print_full(snap: &SystemSnapshot, format: &OutputFormat) {
    match format {
        OutputFormat::Json => print_json(snap),
        OutputFormat::Compact | OutputFormat::Verbose => print_human(snap, format),
    }
}

fn print_human(snap: &SystemSnapshot, format: &OutputFormat) {
    println!("── System ──────────────────────────────────");
    let cpu_ind = health_indicator(snap.cpu_usage_pct, 70.0, 90.0);
    println!("  {cpu_ind} CPU      {:.1}%", snap.cpu_usage_pct);

    let mem_pct = if snap.memory_total_mb > 0 {
        snap.memory_used_mb as f32 / snap.memory_total_mb as f32 * 100.0
    } else {
        0.0
    };
    let mem_ind = health_indicator(mem_pct, 70.0, 90.0);
    println!(
        "  {mem_ind} Memory   {} / {} MB  ({:.0}%)",
        snap.memory_used_mb, snap.memory_total_mb, mem_pct,
    );

    if snap.swap_total_mb > 0 {
        let swap_pct = snap.swap_used_mb as f32 / snap.swap_total_mb as f32 * 100.0;
        let swap_ind = health_indicator(swap_pct, 50.0, 80.0);
        println!(
            "  {swap_ind} Swap     {} / {} MB  ({:.0}%)",
            snap.swap_used_mb, snap.swap_total_mb, swap_pct,
        );
    }
    println!("  ⏱  Uptime   {}", format_uptime(snap.uptime_secs));

    println!("\n── Disk ────────────────────────────────────");
    for disk in &snap.disks {
        print_disk(disk);
    }

    println!("\n── Top Processes (by CPU) ──────────────────");
    for proc in snap.processes.iter().take(10) {
        print_proc(proc, format);
    }
}

fn print_disk(d: &DiskInfo) {
    let ind = health_indicator(d.used_pct, 80.0, 95.0);
    println!(
        "  {ind} {}  {:.1} GB used / {:.1} GB total  ({:.0}%)",
        d.mount,
        d.total_gb - d.available_gb,
        d.total_gb,
        d.used_pct,
    );
}

fn print_proc(p: &ProcessInfo, format: &OutputFormat) {
    let base = format!(
        "  PID {:>6}  CPU {:>5.1}%  MEM {:>5} MB  {}",
        p.pid, p.cpu_pct, p.memory_mb, p.name,
    );
    if let (OutputFormat::Verbose, Some(argv)) = (format, &p.argv) {
        println!("{}  [{}]", base, argv.join(" "));
    } else {
        println!("{base}");
    }
}

pub fn print_top(snap: &SystemSnapshot, n: usize, format: &OutputFormat) {
    println!("── Top {} Processes (by CPU) ────────────────", n);
    for proc in snap.processes.iter().take(n) {
        print_proc(proc, format);
    }
}

pub fn print_disk_usage(snap: &SystemSnapshot) {
    println!("── Disk Usage ──────────────────────────────");
    for disk in &snap.disks {
        print_disk(disk);
    }
}

fn print_json(snap: &SystemSnapshot) {
    // Minimal JSON output — no external serde dependency needed for this simple shape
    println!("{{");
    println!("  \"cpu_usage_pct\": {:.2},", snap.cpu_usage_pct);
    println!("  \"memory_used_mb\": {},", snap.memory_used_mb);
    println!("  \"memory_total_mb\": {},", snap.memory_total_mb);
    println!("  \"swap_used_mb\": {},", snap.swap_used_mb);
    println!("  \"swap_total_mb\": {},", snap.swap_total_mb);
    println!("  \"uptime_secs\": {},", snap.uptime_secs);
    println!("  \"processes\": [");
    let procs: Vec<_> = snap.processes.iter().take(20).collect();
    for (i, p) in procs.iter().enumerate() {
        let comma = if i + 1 < procs.len() { "," } else { "" };
        // Redact argv in JSON output too (verbose-only)
        println!(
            "    {{\"pid\": {}, \"name\": {:?}, \"cpu_pct\": {:.2}, \"memory_mb\": {}}}{}",
            p.pid, p.name, p.cpu_pct, p.memory_mb, comma
        );
    }
    println!("  ],");
    println!("  \"disks\": [");
    for (i, d) in snap.disks.iter().enumerate() {
        let comma = if i + 1 < snap.disks.len() { "," } else { "" };
        println!(
            "    {{\"mount\": {:?}, \"total_gb\": {:.2}, \"available_gb\": {:.2}, \"used_pct\": {:.1}}}{}",
            d.mount, d.total_gb, d.available_gb, d.used_pct, comma
        );
    }
    println!("  ]");
    println!("}}");
}

fn format_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("{days}d {hours}h {mins}m")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else {
        format!("{mins}m")
    }
}
