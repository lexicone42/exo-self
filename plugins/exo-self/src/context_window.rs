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
    /// Serving model, as reported by Claude Code to the statusline. Authoritative —
    /// unlike a model's self-report, which can be stale (see the 2026-07-27 note: a
    /// system prompt claimed Fable 5 while the transcript showed claude-opus-5 serving).
    pub model: String,
}

/// The serving model for a session, from the statusline-written JSON.
/// Returns None when unavailable or when the record belongs to another session.
pub fn serving_model(paths: &ExoPaths, session_id: &str) -> Option<String> {
    let read = |p: &std::path::Path| {
        std::fs::read_to_string(p)
            .ok()
            .and_then(|d| serde_json::from_str::<ContextWindow>(&d).ok())
    };
    let ctx = if session_id.is_empty() {
        read(&paths.context_window)
    } else {
        read(&paths.context_window_file(session_id))
            .or_else(|| read(&paths.context_window).filter(|c| c.session_id == session_id))
    }?;
    (!ctx.model.is_empty()).then_some(ctx.model)
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
///
/// Session identity (#19): with concurrent sessions, the legacy shared
/// .context-window.json holds whichever session wrote last — trusting it blindly
/// makes one session read a neighbor's usage on the *best* code path and prematurely
/// wind down. So: prefer the per-session file (its path IS the validation); accept
/// the legacy file only when its embedded session_id matches ours (or when we have
/// no session_id to validate with, preserving old single-session behavior). A
/// mismatched record contributes nothing — not even its window size, since the
/// neighbor may be running a different window (200K vs 1M).
pub fn get_usage_ratio(
    paths: &ExoPaths,
    session_id: &str,
    transcript_path: &str,
    estimated_max_chars: u64,
) -> (f64, &'static str) {
    // Prefer the per-session file; fall back to the legacy shared file only if its
    // session_id checks out. The surviving `ctx` also supplies the window size for
    // the filesize denominator below — validated records only.
    let read = |p: &std::path::Path| {
        std::fs::read_to_string(p)
            .ok()
            .and_then(|data| serde_json::from_str::<ContextWindow>(&data).ok())
    };
    let per_session = if session_id.is_empty() {
        None
    } else {
        read(&paths.context_window_file(session_id))
    };
    let ctx = per_session.or_else(|| {
        read(&paths.context_window).filter(|c| session_id.is_empty() || c.session_id == session_id)
    });

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
            let raw = meta.len() as f64 / max_chars as f64;
            // Saturation guard (#20): a filesize estimate that pegs at/over 1.0 is
            // evidence the ESTIMATE is unusable, not evidence the context is full.
            // Transcripts are append-only across compactions, so a long compacted
            // session (observed: 960MB) saturates any filesize ratio immediately.
            // Returning 1.0 here previously fired a false "context full" alarm the
            // agent acted on. Report no signal instead of a confident wrong one.
            if raw >= 1.0 {
                return (0.0, "none");
            }
            return (raw, source);
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

    /// Write a context-window JSON with an embedded session_id.
    fn write_ctx(path: &std::path::Path, session_id: &str, pct: f64, window: u64) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let json = format!(
            r#"{{"used_percentage": {pct}, "context_window_size": {window}, "session_id": "{session_id}", "updated_at": {}}}"#,
            now()
        );
        std::fs::write(path, json).unwrap();
    }

    #[test]
    fn per_session_file_preferred_over_shared() {
        // #19: shared file belongs to a concurrent session at 90%; our per-session
        // file says 20%. Must read our own.
        let dir = tempfile::tempdir().unwrap();
        let cw = dir.path().join(".context-window.json");
        let paths = paths_with_context_window(&cw);
        write_ctx(&cw, "other-session", 90.0, 1_000_000);
        write_ctx(&paths.context_window_file("mine"), "mine", 20.0, 1_000_000);
        let transcript = write_transcript(dir.path(), 100);

        let (ratio, source) =
            get_usage_ratio(&paths, "mine", transcript.to_str().unwrap(), 4_000_000);
        assert_eq!(source, "tokens");
        assert!(
            (ratio - 0.20).abs() < 1e-9,
            "read a neighbor's usage: {ratio}"
        );
    }

    #[test]
    fn shared_file_rejected_on_session_mismatch() {
        // #19: no per-session file; shared file belongs to another session at 99%.
        // Must NOT adopt it (not even its window size) — fall through to filesize.
        let dir = tempfile::tempdir().unwrap();
        let cw = dir.path().join(".context-window.json");
        let paths = paths_with_context_window(&cw);
        write_ctx(&cw, "other-session", 99.0, 1_000_000);
        let transcript = write_transcript(dir.path(), 400_000); // 10% of 4M config

        let (ratio, source) =
            get_usage_ratio(&paths, "mine", transcript.to_str().unwrap(), 4_000_000);
        assert_eq!(source, "filesize", "adopted a mismatched session's data");
        assert!(
            (ratio - 0.10).abs() < 1e-6,
            "expected filesize 0.10, got {ratio}"
        );
    }

    #[test]
    fn shared_file_accepted_on_session_match() {
        // Mixed-version deployment: no per-session file, shared file is ours → trust it.
        let dir = tempfile::tempdir().unwrap();
        let cw = dir.path().join(".context-window.json");
        let paths = paths_with_context_window(&cw);
        write_ctx(&cw, "mine", 30.0, 1_000_000);
        let transcript = write_transcript(dir.path(), 100);

        let (ratio, source) =
            get_usage_ratio(&paths, "mine", transcript.to_str().unwrap(), 4_000_000);
        assert_eq!(source, "tokens");
        assert!((ratio - 0.30).abs() < 1e-9);
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

        let (ratio, source) = get_usage_ratio(&paths, "", transcript.to_str().unwrap(), 800_000);
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

        let (ratio, source) = get_usage_ratio(&paths, "", transcript.to_str().unwrap(), 800_000);
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

        let (ratio, source) = get_usage_ratio(&paths, "", transcript.to_str().unwrap(), 4_000_000);
        assert_eq!(source, "tokens_stale");
        assert!((ratio - 0.30).abs() < 1e-9, "expected 0.30, got {ratio}");
    }

    #[test]
    fn no_json_falls_back_to_static_estimate() {
        let dir = tempfile::tempdir().unwrap();
        let cw = dir.path().join(".context-window.json"); // not created
        let paths = paths_with_context_window(&cw);
        let transcript = write_transcript(dir.path(), 2_000_000);

        let (ratio, source) = get_usage_ratio(&paths, "", transcript.to_str().unwrap(), 4_000_000);
        assert_eq!(source, "filesize");
        assert!((ratio - 0.5).abs() < 1e-6, "expected 0.5, got {ratio}");
    }

    #[test]
    fn saturated_filesize_reports_no_signal_not_full() {
        // #20: a compacted session's append-only transcript (observed: 960MB) saturates
        // any filesize ratio. Previously returned 1.0 → false "context full" alarm the
        // agent acted on. Must report "none" so no threshold fires.
        let dir = tempfile::tempdir().unwrap();
        let cw = dir.path().join(".context-window.json"); // absent
        let paths = paths_with_context_window(&cw);
        let transcript = write_transcript(dir.path(), 8_000_000); // 2x the 4M budget

        let (ratio, source) = get_usage_ratio(&paths, "", transcript.to_str().unwrap(), 4_000_000);
        assert_eq!(source, "none", "saturated estimate must not claim fullness");
        assert_eq!(ratio, 0.0);
    }

    #[test]
    fn unsaturated_filesize_still_reports_normally() {
        // Guard must not suppress legitimate sub-1.0 filesize estimates.
        let dir = tempfile::tempdir().unwrap();
        let cw = dir.path().join(".context-window.json"); // absent
        let paths = paths_with_context_window(&cw);
        let transcript = write_transcript(dir.path(), 2_000_000); // 50% of 4M

        let (ratio, source) = get_usage_ratio(&paths, "", transcript.to_str().unwrap(), 4_000_000);
        assert_eq!(source, "filesize");
        assert!((ratio - 0.5).abs() < 1e-6);
    }

    #[test]
    fn filesize_ratio_never_reports_saturated_as_full() {
        // Superseded by #20: this previously asserted the ratio CAPS at 1.0. Capping is
        // exactly what produced false "context full" alarms on compacted sessions, so a
        // saturated filesize estimate is now reported as no-signal instead.
        let dir = tempfile::tempdir().unwrap();
        let cw = dir.path().join(".context-window.json"); // not created
        let paths = paths_with_context_window(&cw);
        let transcript = write_transcript(dir.path(), 10_000_000);

        let (ratio, source) = get_usage_ratio(&paths, "", transcript.to_str().unwrap(), 4_000_000);
        assert_eq!(source, "none");
        assert_eq!(ratio, 0.0);
    }

    #[test]
    fn no_transcript_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cw = dir.path().join(".context-window.json"); // not created
        let paths = paths_with_context_window(&cw);

        let (ratio, source) = get_usage_ratio(&paths, "", "", 4_000_000);
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

        let (ratio, source) = get_usage_ratio(&paths, "", transcript.to_str().unwrap(), 800_000);
        assert_eq!(source, "tokens");
        assert!((ratio - 0.55).abs() < 1e-9);
    }
}
