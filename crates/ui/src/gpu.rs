//! GPU selection before wgpu starts (build plan §2.7).
//!
//! Measured on the dev laptop (Vega iGPU + GTX 1660Ti): Iced's default high-performance
//! adapter puts the first frame at ~275 ms; the low-power adapter at ~145 ms; also keeping
//! the Vulkan loader from loading NVIDIA's driver at ~67 ms.

use std::path::Path;

/// Set `WGPU_POWER_PREF`, `WGPU_BACKEND` and, on hybrid laptops, `VK_LOADER_DRIVERS_SELECT`
/// unless the user already set them.
///
/// # Safety
/// Mutates the environment: call while the process is still single-threaded.
pub unsafe fn configure() {
    let unset = |k: &str| std::env::var_os(k).is_none();
    let wants_high = std::env::var("WGPU_POWER_PREF").is_ok_and(|v| v.eq_ignore_ascii_case("high"));
    unsafe {
        if unset("WGPU_POWER_PREF") {
            std::env::set_var("WGPU_POWER_PREF", "low");
        }
        if unset("WGPU_BACKEND") {
            std::env::set_var("WGPU_BACKEND", "vulkan");
        }
        if unset("VK_LOADER_DRIVERS_SELECT") && !wants_high {
            if let Some(glob) = display_gpu_driver_glob() {
                std::env::set_var("VK_LOADER_DRIVERS_SELECT", glob);
            }
        }
    }
}

/// On a machine with GPUs from more than one vendor, the Vulkan driver glob for the GPU
/// that drives the display (`boot_vga`) — the integrated one on hybrid laptops.
fn display_gpu_driver_glob() -> Option<&'static str> {
    let mut vendors = Vec::new();
    let mut display = None;
    for dev in std::fs::read_dir("/sys/bus/pci/devices").ok()?.flatten() {
        let p = dev.path();
        let class = read(&p.join("class"));
        if !class.starts_with("0x0300") && !class.starts_with("0x0302") {
            continue;
        }
        let vendor = read(&p.join("vendor"));
        if read(&p.join("boot_vga")) == "1" {
            display = Some(vendor.clone());
        }
        vendors.push(vendor);
    }
    vendors.sort();
    vendors.dedup();
    if vendors.len() < 2 {
        return None;
    }
    let (glob, manifest) = match display?.as_str() {
        "0x1002" => ("*radeon*", "radeon_icd"),
        "0x8086" => ("*intel*", "intel_icd"),
        _ => return None,
    };
    // Only restrict the loader if that driver is actually installed.
    let installed = ["/usr/share/vulkan/icd.d", "/etc/vulkan/icd.d"].iter().any(|d| {
        std::fs::read_dir(d)
            .map(|es| es.flatten().any(|e| e.file_name().to_string_lossy().starts_with(manifest)))
            .unwrap_or(false)
    });
    installed.then_some(glob)
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).map(|s| s.trim().to_string()).unwrap_or_default()
}
