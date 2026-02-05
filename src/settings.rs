//! Game settings and preferences
//!
//! Persisted separately from game saves in LocalStorage.

use serde::{Deserialize, Serialize};

/// Quality preset levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum QualityPreset {
    Low,
    #[default]
    Medium,
    High,
}

impl QualityPreset {
    pub fn as_str(&self) -> &'static str {
        match self {
            QualityPreset::Low => "Low",
            QualityPreset::Medium => "Medium",
            QualityPreset::High => "High",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "low" => Some(QualityPreset::Low),
            "medium" | "med" => Some(QualityPreset::Medium),
            "high" => Some(QualityPreset::High),
            _ => None,
        }
    }

    /// Maximum particles for this preset
    pub fn max_particles(&self) -> usize {
        match self {
            QualityPreset::Low => 100,
            QualityPreset::Medium => 500,
            QualityPreset::High => 2000,
        }
    }

    /// Trail length multiplier (1.0 = full)
    pub fn trail_quality(&self) -> f32 {
        match self {
            QualityPreset::Low => 0.25,
            QualityPreset::Medium => 0.6,
            QualityPreset::High => 1.0,
        }
    }

    /// Whether to render starfield parallax
    pub fn starfield_enabled(&self) -> bool {
        match self {
            QualityPreset::Low => false,
            QualityPreset::Medium => true,
            QualityPreset::High => true,
        }
    }

    /// Whether to render nebula background
    pub fn nebula_enabled(&self) -> bool {
        match self {
            QualityPreset::Low => false,
            QualityPreset::Medium => false,
            QualityPreset::High => true,
        }
    }
}

/// Game settings/preferences
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Graphics quality preset
    pub quality: QualityPreset,

    // === Visual Effects ===
    /// Screen shake on explosions/impacts
    pub screen_shake: bool,
    /// Ball trails
    pub trails: bool,
    /// Particle effects (explosions, sparks, etc.)
    pub particles: bool,
    /// Wave flash effect
    pub wave_flash: bool,
    /// Power-up visual effects (orbiting particles, sparkles)
    pub powerup_effects: bool,

    // === HUD ===
    /// Show FPS counter
    pub show_fps: bool,

    // === Audio (prep for later) ===
    /// Master volume (0.0 - 1.0)
    pub master_volume: f32,
    /// Sound effects volume (0.0 - 1.0)
    pub sfx_volume: f32,
    /// Music volume (0.0 - 1.0)
    pub music_volume: f32,
    /// Mute when window loses focus
    pub mute_on_blur: bool,

    // === Accessibility ===
    /// Reduced motion (minimize shake, flashes)
    pub reduced_motion: bool,
    /// High contrast mode
    pub high_contrast: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            quality: QualityPreset::Medium,

            // Visual effects - all on by default
            screen_shake: true,
            trails: true,
            particles: true,
            wave_flash: true,
            powerup_effects: true,

            // HUD
            show_fps: true,

            // Audio
            master_volume: 0.8,
            sfx_volume: 1.0,
            music_volume: 0.7,
            mute_on_blur: true,

            // Accessibility
            reduced_motion: false,
            high_contrast: false,
        }
    }
}

impl Settings {
    /// Create settings from a quality preset (applies preset defaults)
    pub fn from_preset(preset: QualityPreset) -> Self {
        let mut settings = Self::default();
        settings.quality = preset;
        settings
    }

    /// Apply a quality preset (updates quality-dependent settings)
    pub fn apply_preset(&mut self, preset: QualityPreset) {
        self.quality = preset;

        // Low preset disables some effects for performance
        if preset == QualityPreset::Low {
            self.powerup_effects = false;
            self.wave_flash = false;
        }
    }

    /// Effective screen shake (respects reduced_motion)
    pub fn effective_screen_shake(&self) -> bool {
        self.screen_shake && !self.reduced_motion
    }

    /// Effective wave flash (respects reduced_motion)
    pub fn effective_wave_flash(&self) -> bool {
        self.wave_flash && !self.reduced_motion
    }

    /// Effective particle count cap
    pub fn max_particles(&self) -> usize {
        if !self.particles {
            0
        } else {
            self.quality.max_particles()
        }
    }

    /// LocalStorage key
    const STORAGE_KEY: &'static str = "roto_pong_settings";

    /// Load settings from LocalStorage (WASM only)
    #[cfg(target_arch = "wasm32")]
    pub fn load() -> Self {
        let storage = web_sys::window()
            .and_then(|w| w.local_storage().ok())
            .flatten();

        if let Some(storage) = storage {
            if let Ok(Some(json)) = storage.get_item(Self::STORAGE_KEY) {
                if let Ok(settings) = serde_json::from_str(&json) {
                    log::info!("Loaded settings from LocalStorage");
                    return settings;
                }
            }
        }

        log::info!("Using default settings");
        Self::default()
    }

    /// Save settings to LocalStorage (WASM only)
    #[cfg(target_arch = "wasm32")]
    pub fn save(&self) {
        let storage = web_sys::window()
            .and_then(|w| w.local_storage().ok())
            .flatten();

        if let Some(storage) = storage {
            if let Ok(json) = serde_json::to_string(self) {
                let _ = storage.set_item(Self::STORAGE_KEY, &json);
                log::info!("Settings saved");
            }
        }
    }

    /// Native stubs
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load() -> Self {
        Self::default()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&self) {
        // No-op for native
    }
}
