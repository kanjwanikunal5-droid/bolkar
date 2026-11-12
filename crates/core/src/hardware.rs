//! Picks a starting speech model for a machine. This is only the first guess shown during
//! onboarding; the benchmark's measured real-time factor replaces it.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gpu {
    pub name: String,
    /// A dedicated graphics card (not the chip built into the CPU).
    pub discrete: bool,
    pub vulkan: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HardwareInfo {
    pub os: String,
    pub cpu: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
    pub ram_gb: f64,
    /// Free space on the drive the app stores models on.
    pub free_disk_gb: f64,
    pub gpus: Vec<Gpu>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WhisperModel {
    Base,
    Small,
    Medium,
    LargeV3Turbo,
}

impl WhisperModel {
    /// Approximate download size of the quantised model file, in MB.
    pub fn download_mb(self) -> u32 {
        match self {
            WhisperModel::Base => 60,
            WhisperModel::Small => 190,
            WhisperModel::Medium => 540,
            WhisperModel::LargeV3Turbo => 580,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Vulkan,
    Cpu,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recommendation {
    pub whisper_model: WhisperModel,
    pub backend: Backend,
    pub reason: String,
    pub warnings: Vec<String>,
}

/// Disk space below which the app warns before recording or downloading.
pub const LOW_DISK_GB: f64 = 0.5;

pub fn recommend(hw: &HardwareInfo) -> Recommendation {
    let mut warnings = Vec::new();
    let discrete = hw.gpus.iter().find(|g| g.discrete && g.vulkan);

    let (mut whisper_model, backend, mut reason) =
        if let Some(gpu) = discrete.filter(|_| hw.ram_gb >= 7.5) {
            (
                WhisperModel::LargeV3Turbo,
                Backend::Vulkan,
                format!(
                    "{} supports Vulkan, so the most accurate model can run on the graphics card.",
                    gpu.name
                ),
            )
        } else if hw.ram_gb >= 15.0 && hw.logical_cores >= 8 {
            (
            WhisperModel::Small,
            Backend::Cpu,
            "No dedicated graphics card: a mid-size model keeps dictation quick on the processor."
                .to_string(),
        )
        } else if hw.ram_gb >= 7.5 {
            (
                WhisperModel::Small,
                Backend::Cpu,
                "8 GB class machine: a mid-size model on the processor.".to_string(),
            )
        } else {
            (
                WhisperModel::Base,
                Backend::Cpu,
                "Limited memory: the smallest model is the safe choice.".to_string(),
            )
        };

    let needed_gb = f64::from(whisper_model.download_mb()) / 1024.0 + LOW_DISK_GB;
    if hw.free_disk_gb < needed_gb {
        warnings.push(format!(
            "Only {:.1} GB free: not enough for the suggested model ({} MB). Suggesting the smallest model instead.",
            hw.free_disk_gb,
            whisper_model.download_mb()
        ));
        whisper_model = WhisperModel::Base;
        reason = "Low disk space: the smallest model is the safe choice.".to_string();
    }
    if hw.free_disk_gb < LOW_DISK_GB {
        warnings.push("Less than 500 MB of disk space is free.".to_string());
    }
    if backend == Backend::Cpu && hw.gpus.iter().any(|g| g.vulkan) {
        warnings.push(
            "Only an integrated graphics chip was found; it is not used by default.".to_string(),
        );
    }

    Recommendation {
        whisper_model,
        backend,
        reason,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine(ram_gb: f64, logical: usize, free_disk_gb: f64, gpus: Vec<Gpu>) -> HardwareInfo {
        HardwareInfo {
            os: "test".into(),
            cpu: "test".into(),
            physical_cores: logical / 2,
            logical_cores: logical,
            ram_gb,
            free_disk_gb,
            gpus,
        }
    }

    fn gpu(name: &str, discrete: bool) -> Gpu {
        Gpu {
            name: name.into(),
            discrete,
            vulkan: true,
        }
    }

    #[test]
    fn owner_laptop_gets_turbo_on_vulkan() {
        let hw = machine(
            15.2,
            12,
            244.0,
            vec![
                gpu("NVIDIA GeForce RTX 4050 Laptop GPU", true),
                gpu("AMD Radeon 740M Graphics", false),
            ],
        );
        let r = recommend(&hw);
        assert_eq!(
            (r.whisper_model, r.backend),
            (WhisperModel::LargeV3Turbo, Backend::Vulkan)
        );
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn integrated_graphics_only_uses_cpu() {
        let r = recommend(&machine(16.0, 8, 100.0, vec![gpu("Intel Iris Xe", false)]));
        assert_eq!(
            (r.whisper_model, r.backend),
            (WhisperModel::Small, Backend::Cpu)
        );
        assert_eq!(r.warnings.len(), 1);
    }

    #[test]
    fn small_machine_gets_base() {
        let r = recommend(&machine(4.0, 4, 50.0, vec![]));
        assert_eq!(
            (r.whisper_model, r.backend),
            (WhisperModel::Base, Backend::Cpu)
        );
    }

    #[test]
    fn low_disk_downgrades_and_warns() {
        let r = recommend(&machine(16.0, 12, 0.3, vec![gpu("RTX", true)]));
        assert_eq!(r.whisper_model, WhisperModel::Base);
        assert_eq!(r.warnings.len(), 2);
    }
}
