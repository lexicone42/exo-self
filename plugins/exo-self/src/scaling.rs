use crate::config::Config;

/// Scale a default limit based on context window size.
/// Only scales if the config value matches the default (user hasn't overridden).
fn scale_factor(cfg: &Config) -> f64 {
    if cfg.estimated_max_chars > 800_000 {
        (cfg.estimated_max_chars as f64 / 800_000.0).min(4.0)
    } else {
        1.0
    }
}

/// Scaled max journal chars (default 1500, scales up to 6000 for 1M+ contexts)
pub fn journal_chars(cfg: &Config) -> usize {
    if cfg.max_journal_chars == 1500 {
        (1500.0 * scale_factor(cfg)) as usize
    } else {
        cfg.max_journal_chars
    }
}

/// Scaled max journal entries (default 2, scales up to 8)
pub fn journal_entries(cfg: &Config) -> usize {
    if cfg.max_journal_entries == 2 {
        (2.0 * scale_factor(cfg)).max(2.0) as usize
    } else {
        cfg.max_journal_entries
    }
}

/// Scaled max interests items (default 5, scales up to 20)
pub fn interests_items(cfg: &Config) -> usize {
    if cfg.max_interests_items == 5 {
        (5.0 * scale_factor(cfg)).max(5.0) as usize
    } else {
        cfg.max_interests_items
    }
}

/// Scaled max lessons display (default 5, scales up to 15)
pub fn lessons_display(cfg: &Config) -> usize {
    (5.0 * scale_factor(cfg)).min(15.0).max(5.0) as usize
}

/// Scaled max sparks display (default 5, scales up to 20)
pub fn sparks_display(cfg: &Config) -> usize {
    if cfg.max_sparks_display == 5 {
        (5.0 * scale_factor(cfg)).max(5.0) as usize
    } else {
        cfg.max_sparks_display
    }
}
