use crate::paths::ExoPaths;
use crate::state::now;
use serde::Deserialize;

/// Data written by statusline to .context-window.json
#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub struct ContextWindow {
    pub used_percentage: Option<f64>,
    pub remaining_percentage: Option<f64>,
    pub context_window_size: u64,
    pub used_tokens: u64,
    pub free_tokens: u64,
    pub usage_pct: u64,
    pub session_id: String,
    pub updated_at: f64,
}

/// Get context usage ratio, preferring token-accurate data from statusline.
/// Returns (ratio, source) where source is "tokens", "filesize", or "none".
pub fn get_usage_ratio(
    paths: &ExoPaths,
    transcript_path: &str,
    estimated_max_chars: u64,
) -> (f64, &'static str) {
    // Priority 1: statusline-written .context-window.json
    if let Ok(data) = std::fs::read_to_string(&paths.context_window)
        && let Ok(ctx) = serde_json::from_str::<ContextWindow>(&data)
            && now() - ctx.updated_at < 120.0 {
                if let Some(pct) = ctx.used_percentage {
                    return (pct / 100.0, "tokens");
                }
                if ctx.usage_pct > 0 {
                    return (ctx.usage_pct as f64 / 100.0, "tokens");
                }
            }

    // Priority 2: transcript file size (rough approximation)
    if !transcript_path.is_empty() && estimated_max_chars > 0
        && let Ok(meta) = std::fs::metadata(transcript_path) {
            let size = meta.len();
            let ratio = (size as f64 / estimated_max_chars as f64).min(1.0);
            return (ratio, "filesize");
        }

    (0.0, "none")
}
