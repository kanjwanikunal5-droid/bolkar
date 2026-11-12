//! Reads this machine's hardware (read-only) and prints a starting model recommendation.
//!
//!   probe            human-readable
//!   probe --json     machine-readable

use std::{env, path::Path, process::Command};

use app_core::hardware::{recommend, Gpu, HardwareInfo};
use sysinfo::{Disks, System};

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

/// Parses `vulkaninfo --summary`. Returns an empty list when Vulkan tools are not installed;
/// that means "unknown", and the recommendation then falls back to the processor.
fn vulkan_gpus() -> Vec<Gpu> {
    let Ok(output) = Command::new("vulkaninfo").arg("--summary").output() else {
        return Vec::new();
    };
    parse_vulkan_summary(&String::from_utf8_lossy(&output.stdout))
}

fn parse_vulkan_summary(text: &str) -> Vec<Gpu> {
    let mut gpus = Vec::new();
    let mut device_type = String::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key.trim() {
            "deviceType" => device_type = value.trim().to_string(),
            "deviceName" => {
                // Software renderers (type CPU) are not real graphics cards.
                if !device_type.contains("CPU") {
                    gpus.push(Gpu {
                        name: value.trim().to_string(),
                        discrete: device_type.contains("DISCRETE"),
                        vulkan: true,
                    });
                }
                device_type.clear();
            }
            _ => {}
        }
    }
    gpus
}

fn free_disk_gb(dir: &Path) -> f64 {
    let disks = Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|d| dir.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len())
        .map_or(0.0, |d| d.available_space() as f64 / GIB)
}

fn main() {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cwd = env::current_dir().unwrap_or_default();
    let logical = sys.cpus().len();
    let hw = HardwareInfo {
        os: System::long_os_version().unwrap_or_else(|| "unknown".to_string()),
        cpu: sys
            .cpus()
            .first()
            .map_or_else(|| "unknown".to_string(), |c| c.brand().trim().to_string()),
        physical_cores: System::physical_core_count().unwrap_or(logical),
        logical_cores: logical,
        ram_gb: (sys.total_memory() as f64 / GIB * 10.0).round() / 10.0,
        free_disk_gb: (free_disk_gb(&cwd) * 10.0).round() / 10.0,
        gpus: vulkan_gpus(),
    };
    let rec = recommend(&hw);

    if env::args().any(|a| a == "--json") {
        let json = serde_json::json!({ "hardware": hw, "recommendation": rec });
        println!(
            "{}",
            serde_json::to_string_pretty(&json).unwrap_or_default()
        );
        return;
    }

    println!("OS:        {}", hw.os);
    println!(
        "Processor: {} ({} cores / {} threads)",
        hw.cpu, hw.physical_cores, hw.logical_cores
    );
    println!("Memory:    {:.1} GB", hw.ram_gb);
    println!("Free disk: {:.1} GB ({})", hw.free_disk_gb, cwd.display());
    if hw.gpus.is_empty() {
        println!("Graphics:  none found through Vulkan");
    }
    for g in &hw.gpus {
        println!(
            "Graphics:  {} ({}, Vulkan)",
            g.name,
            if g.discrete {
                "dedicated"
            } else {
                "integrated"
            }
        );
    }
    println!();
    println!(
        "Suggested model: {:?} on {:?}",
        rec.whisper_model, rec.backend
    );
    println!("Why: {}", rec.reason);
    for w in &rec.warnings {
        println!("Note: {w}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_vulkaninfo_summary() {
        let text = "GPU0:\n\tapiVersion = 1.4.329\n\tdeviceType = PHYSICAL_DEVICE_TYPE_INTEGRATED_GPU\n\tdeviceName = AMD Radeon 740M Graphics\nGPU1:\n\tdeviceType = PHYSICAL_DEVICE_TYPE_DISCRETE_GPU\n\tdeviceName = NVIDIA GeForce RTX 4050 Laptop GPU\nGPU2:\n\tdeviceType = PHYSICAL_DEVICE_TYPE_CPU\n\tdeviceName = llvmpipe\n";
        let gpus = parse_vulkan_summary(text);
        assert_eq!(gpus.len(), 2);
        assert!(!gpus[0].discrete);
        assert!(gpus[1].discrete);
        assert_eq!(gpus[1].name, "NVIDIA GeForce RTX 4050 Laptop GPU");
    }
}
