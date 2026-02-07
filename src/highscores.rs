//! High score leaderboard system
//!
//! Persisted to LocalStorage, tracks top 10 scores.

use serde::{Deserialize, Serialize};

/// Maximum number of high scores to keep
pub const MAX_HIGH_SCORES: usize = 10;

/// A single high score entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighScoreEntry {
    /// Player's score
    pub score: u64,
    /// Wave reached
    pub wave: u32,
    /// Unix timestamp (ms) when achieved
    pub timestamp: f64,
}

/// High score leaderboard
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HighScores {
    pub entries: Vec<HighScoreEntry>,
}

impl HighScores {
    /// LocalStorage key (used only in wasm32)
    #[allow(dead_code)]
    const STORAGE_KEY: &'static str = "roto_pong_highscores";

    /// Create empty leaderboard
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Check if a score qualifies for the leaderboard
    pub fn qualifies(&self, score: u64) -> bool {
        if score == 0 {
            return false;
        }
        if self.entries.len() < MAX_HIGH_SCORES {
            return true;
        }
        // Check if score beats the lowest entry
        self.entries.last().map(|e| score > e.score).unwrap_or(true)
    }

    /// Get the rank a score would achieve (1-indexed, None if doesn't qualify)
    pub fn potential_rank(&self, score: u64) -> Option<usize> {
        if !self.qualifies(score) {
            return None;
        }
        let rank = self.entries.iter().position(|e| score > e.score);
        Some(rank.unwrap_or(self.entries.len()) + 1)
    }

    /// Add a new score to the leaderboard (if it qualifies)
    /// Returns the rank achieved (1-indexed) or None if didn't qualify
    pub fn add_score(&mut self, score: u64, wave: u32, timestamp: f64) -> Option<usize> {
        if !self.qualifies(score) {
            return None;
        }

        let entry = HighScoreEntry {
            score,
            wave,
            timestamp,
        };

        // Find insertion point (sorted descending by score)
        let pos = self.entries.iter().position(|e| score > e.score);
        let rank = match pos {
            Some(i) => {
                self.entries.insert(i, entry);
                i + 1
            }
            None => {
                self.entries.push(entry);
                self.entries.len()
            }
        };

        // Trim to max size
        self.entries.truncate(MAX_HIGH_SCORES);

        Some(rank)
    }

    /// Check if the leaderboard is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get the top score (if any)
    pub fn top_score(&self) -> Option<u64> {
        self.entries.first().map(|e| e.score)
    }

    /// Load high scores from LocalStorage (WASM only)
    #[cfg(target_arch = "wasm32")]
    pub fn load() -> Self {
        let storage = web_sys::window()
            .and_then(|w| w.local_storage().ok())
            .flatten();

        if let Some(storage) = storage {
            if let Ok(Some(json)) = storage.get_item(Self::STORAGE_KEY) {
                if let Ok(scores) = serde_json::from_str::<HighScores>(&json) {
                    log::info!("Loaded {} high scores", scores.entries.len());
                    return scores;
                }
            }
        }

        log::info!("No high scores found, starting fresh");
        Self::new()
    }

    /// Save high scores to LocalStorage (WASM only)
    #[cfg(target_arch = "wasm32")]
    pub fn save(&self) {
        let storage = web_sys::window()
            .and_then(|w| w.local_storage().ok())
            .flatten();

        if let Some(storage) = storage {
            if let Ok(json) = serde_json::to_string(self) {
                let _ = storage.set_item(Self::STORAGE_KEY, &json);
                log::info!("High scores saved ({} entries)", self.entries.len());
            }
        }
    }

    /// Native stubs
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load() -> Self {
        Self::new()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&self) {
        // No-op for native
    }
}

/// Format a timestamp as a relative date string
#[cfg(target_arch = "wasm32")]
pub fn format_date(timestamp: f64) -> String {
    let now = js_sys::Date::now();
    let diff_ms = now - timestamp;
    let diff_secs = diff_ms / 1000.0;
    let diff_mins = diff_secs / 60.0;
    let diff_hours = diff_mins / 60.0;
    let diff_days = diff_hours / 24.0;

    if diff_days >= 1.0 {
        let days = diff_days.floor() as i32;
        if days == 1 {
            "Yesterday".to_string()
        } else if days < 7 {
            format!("{} days ago", days)
        } else {
            // Format as date
            let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(timestamp));
            format!(
                "{}/{}/{}",
                date.get_month() + 1,
                date.get_date(),
                date.get_full_year() % 100
            )
        }
    } else if diff_hours >= 1.0 {
        let hours = diff_hours.floor() as i32;
        if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{} hours ago", hours)
        }
    } else if diff_mins >= 1.0 {
        let mins = diff_mins.floor() as i32;
        if mins == 1 {
            "1 min ago".to_string()
        } else {
            format!("{} mins ago", mins)
        }
    } else {
        "Just now".to_string()
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn format_date(_timestamp: f64) -> String {
    "N/A".to_string()
}
