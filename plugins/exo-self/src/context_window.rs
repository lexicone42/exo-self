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

/// Rough chars-per-token ratio for English/code. Matches the codebase's own implied
/// assumption: the historical defaults were 800K chars for a 200K-token window and
/// 4M chars for a 1M-token window — both 4 chars/token.
const CHARS_PER_TOKEN: u64 = 4;

/// Token data younger than this is treated as current (source "tokens").
const FRESH_TOKEN_SECS: f64 = 120.0;
/// Token data between FRESH and this is stale-but-usable (source "tokens_stale").
/// A real percentage minutes old beats a filesize guess that, on a compacted session,
/// over-reports by an order of magnitude (see #16). Beyond this it's treated as
/// abandoned and we fall through to the filesize estimate.
const USABLE_TOKEN_SECS: f64 = 600.0;

/// Extract a usage ratio (0.0–1.0) from token data, preferring the explicit
/// `used_percentage` and falling back to the integer `usage_pct`.
fn token_ratio(ctx: &ContextWindow) -> Option<f64> {
    if let Some(pct) = ctx.used_percentage {
        Some(pct / 100.0)
    } else if ctx.usage_pct > 0 {
        Some(ctx.usage_pct as f64 / 100.0)
    } else {
        None
    }
}

/// Get context usage ratio, preferring token-accurate data from statusline.
/// Returns (ratio, source) where source is one of:
///   * "tokens"          — fresh (<2min) token-accurate percentage from statusline (best)
///   * "tokens_stale"    — token percentage 2–10min old; still beats filesize (#16)
///   * "filesize_window" — transcript filesize ÷ (real context window × chars/token)
///   * "filesize"        — transcript filesize ÷ static config estimate (last resort)
///   * "none"            — no signal available
pub fn get_usage_ratio(
    paths: &ExoPaths,
    transcript_path: &str,
    estimated_max_chars: u64,
) -> (f64, &'static str) {
    // Read the statusline-written JSON once; it serves two purposes below — the
    // token percentage AND the (staleness-independent) real context window size.
    let ctx = std::fs::read_to_string(&paths.context_window)
        .ok()
        .and_then(|data| serde_json::from_str::<ContextWindow>(&data).ok());

    // Priority 1: token-accurate percentage from statusline.
    //
    // We prefer a real percentage even when it's minutes stale, because the filesize
    // fallback over-reports badly on compacted sessions (#16): the transcript file is
    // append-only across compactions, so its size reflects cumulative history, not live
    // context. A stale token reading errs in the safe direction (usage only grows, so an
    // old reading under-reports → nudges fire slightly late) where filesize errs in the
    // annoying direction (over-reports → false "context full" alarms).
    if let Some(ref ctx) = ctx
        && let Some(ratio) = token_ratio(ctx)
    {
        let age = now() - ctx.updated_at;
        if age < FRESH_TOKEN_SECS {
            return (ratio, "tokens");
        }
        if age < USABLE_TOKEN_SECS {
            return (ratio, "tokens_stale");
        }
    }

    // Priority 2: transcript file size against a denominator.
    //
    // The context window size is stable within a session, so we trust it even from a
    // STALE JSON — only the token *counts* go stale, not the window size. Deriving the
    // denominator from the real window (rather than the static config estimate) is what
    // keeps 1M-context models from saturating the ratio at ~20% of real usage.
    if !transcript_path.is_empty()
        && let Ok(meta) = std::fs::metadata(transcript_path)
    {
        let window_chars = ctx
            .as_ref()
            .map(|c| c.context_window_size)
            .filter(|&size| size > 0)
            .map(|size| size.saturating_mul(CHARS_PER_TOKEN));

        let (max_chars, source) = match window_chars {
            Some(chars) => (chars, "filesize_window"),
            None => (estimated_max_chars, "filesize"),
        };

        if max_chars > 0 {
            let ratio = (meta.len() as f64 / max_chars as f64).min(1.0);
            return (ratio, source);
        }
    }

    (0.0, "none")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build an ExoPaths whose `context_window` points at `cw_path` and everything
    /// else at throwaway temp paths. Only `context_window` is read here.
    fn paths_with_context_window(cw_path: &std::path::Path) -> ExoPaths {
        let root = cw_path.parent().unwrap().to_path_buf();
        ExoPaths {
            journal: root.join("journal.md"),
            interests: root.join("interests.md"),
            config: root.join("config.json"),
            meta: root.join("meta.json"),
            sessions_dir: root.join("sessions"),
            handoffs_dir: root.join("handoffs"),
            per_project_dir: root.join("per-project"),
            shared_state: root.join("shared-state.json"),
            context_window: cw_path.to_path_buf(),
            synthesis: root.join("synthesis.md"),
            sigils_dir: root.join("sigils"),
            traces_dir: root.join("traces"),
            root,
        }
    }

    /// Write a transcript file of exactly `size` bytes; return its path.
    fn write_transcript(dir: &std::path::Path, size: usize) -> std::path::PathBuf {
        let path = dir.join("transcript.jsonl");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(&vec![b'x'; size]).unwrap();
        path
    }

    #[test]
    fn fresh_token_percentage_wins() {
        let dir = tempfile::tempdir().unwrap();
        let cw = dir.path().join(".context-window.json");
        let json = format!(
            r#"{{"used_percentage": 42.0, "context_window_size": 1000000, "updated_at": {}}}"#,
            now()
        );
        std::fs::write(&cw, json).unwrap();
        let paths = paths_with_context_window(&cw);
        let transcript = write_transcript(dir.path(), 1_000_000);

        let (ratio, source) = get_usage_ratio(&paths, transcript.to_str().unwrap(), 800_000);
        assert_eq!(source, "tokens");
        assert!((ratio - 0.42).abs() < 1e-9);
    }

    #[test]
    fn stale_json_still_supplies_window_size() {
        // Token data is abandoned (>10min old), but the window size is still valid. The
        // filesize ratio must use the real 1M window (×4 chars) — NOT the 800K config.
        let dir = tempfile::tempdir().unwrap();
        let cw = dir.path().join(".context-window.json");
        let json = format!(
            r#"{{"used_percentage": 99.0, "context_window_size": 1000000, "updated_at": {}}}"#,
            now() - 1000.0
        );
        std::fs::write(&cw, json).unwrap();
        let paths = paths_with_context_window(&cw);
        // 800K-char transcript: 20% of the real 4M-char window, NOT ~100% of stale config.
        let transcript = write_transcript(dir.path(), 800_000);

        let (ratio, source) = get_usage_ratio(&paths, transcript.to_str().unwrap(), 800_000);
        assert_eq!(source, "filesize_window");
        assert!((ratio - 0.20).abs() < 1e-6, "expected ~0.20, got {ratio}");
    }

    #[test]
    fn stale_but_usable_token_beats_filesize() {
        // #16: token data 2–10min old must be preferred over a filesize guess that would
        // saturate on a large (compacted) transcript. 300s old, 30% real, 5MB transcript.
        let dir = tempfile::tempdir().unwrap();
        let cw = dir.path().join(".context-window.json");
        let json = format!(
            r#"{{"used_percentage": 30.0, "context_window_size": 1000000, "updated_at": {}}}"#,
            now() - 300.0
        );
        std::fs::write(&cw, json).unwrap();
        let paths = paths_with_context_window(&cw);
        let transcript = write_transcript(dir.path(), 5_000_000); // would be >100% via filesize

        let (ratio, source) = get_usage_ratio(&paths, transcript.to_str().unwrap(), 4_000_000);
        assert_eq!(source, "tokens_stale");
        assert!((ratio - 0.30).abs() < 1e-9, "expected 0.30, got {ratio}");
    }

    #[test]
    fn no_json_falls_back_to_static_estimate() {
        let dir = tempfile::tempdir().unwrap();
        let cw = dir.path().join(".context-window.json"); // not created
        let paths = paths_with_context_window(&cw);
        let transcript = write_transcript(dir.path(), 2_000_000);

        let (ratio, source) = get_usage_ratio(&paths, transcript.to_str().unwrap(), 4_000_000);
        assert_eq!(source, "filesize");
        assert!((ratio - 0.5).abs() < 1e-6, "expected 0.5, got {ratio}");
    }

    #[test]
    fn filesize_ratio_caps_at_one() {
        let dir = tempfile::tempdir().unwrap();
        let cw = dir.path().join(".context-window.json"); // not created
        let paths = paths_with_context_window(&cw);
        let transcript = write_transcript(dir.path(), 10_000_000);

        let (ratio, _) = get_usage_ratio(&paths, transcript.to_str().unwrap(), 4_000_000);
        assert_eq!(ratio, 1.0);
    }

    #[test]
    fn no_transcript_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cw = dir.path().join(".context-window.json"); // not created
        let paths = paths_with_context_window(&cw);

        let (ratio, source) = get_usage_ratio(&paths, "", 4_000_000);
        assert_eq!(source, "none");
        assert_eq!(ratio, 0.0);
    }

    #[test]
    fn usage_pct_fallback_when_no_used_percentage() {
        // Fresh JSON without used_percentage but with usage_pct > 0.
        let dir = tempfile::tempdir().unwrap();
        let cw = dir.path().join(".context-window.json");
        let json = format!(
            r#"{{"usage_pct": 55, "context_window_size": 1000000, "updated_at": {}}}"#,
            now()
        );
        std::fs::write(&cw, json).unwrap();
        let paths = paths_with_context_window(&cw);
        let transcript = write_transcript(dir.path(), 100);

        let (ratio, source) = get_usage_ratio(&paths, transcript.to_str().unwrap(), 800_000);
        assert_eq!(source, "tokens");
        assert!((ratio - 0.55).abs() < 1e-9);
    }
}
