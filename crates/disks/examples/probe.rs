//! `cargo run -p echofiles-disks --example probe` — lists what udisks reports.

fn main() {
    match ef_disks::all_volumes() {
        Ok(vols) => {
            for v in vols {
                let gb = v.size as f64 / 1e9;
                let mounted = if v.is_mounted() { v.mount_points.join(", ") } else { "not mounted".into() };
                println!("{:<20} {:<11} {:>7.1} GB  {:<38} {:<12} {}", v.device, v.fs_type, gb, v.uuid, v.display_name(), mounted);
            }
        }
        Err(e) => eprintln!("{e}"),
    }
    println!("\nLinux drives:");
    match ef_disks::linux_volumes() {
        Ok(vols) => {
            for v in vols {
                println!(
                    "  {} ({}, partition name {:?}, label {:?}){}",
                    v.display_name(),
                    v.device,
                    v.part_name,
                    v.label,
                    if v.locked { " — locked" } else { "" }
                );
            }
        }
        Err(e) => eprintln!("{e}"),
    }
}
