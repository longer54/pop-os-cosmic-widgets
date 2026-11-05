//! Minimalne tłumaczenia PL/EN — język z LC_ALL, LC_MESSAGES, LANG (pierwsza niepusta).
//! Minimal PL/EN i18n — language from LC_ALL, LC_MESSAGES, LANG (first non-empty).

use std::sync::OnceLock;

/// Czy locale (np. `pl_PL.UTF-8`) oznacza polski.
pub fn is_polish(locale: &str) -> bool {
    let l = locale.trim().to_ascii_lowercase();
    l == "pl" || l.starts_with("pl_") || l.starts_with("pl.") || l.starts_with("pl@")
}

fn detect() -> bool {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(var) {
            if !v.trim().is_empty() {
                return is_polish(&v);
            }
        }
    }
    false
}

pub fn polish() -> bool {
    static PL: OnceLock<bool> = OnceLock::new();
    *PL.get_or_init(detect)
}

/// Wybiera tekst w języku systemu.
pub fn t(pl: &'static str, en: &'static str) -> &'static str {
    if polish() { pl } else { en }
}

#[cfg(test)]
mod tests {
    use super::is_polish;

    #[test]
    fn rozpoznaje_jezyk() {
        assert!(is_polish("pl_PL.UTF-8"));
        assert!(is_polish("pl"));
        assert!(is_polish("PL_pl"));
        assert!(!is_polish("en_US.UTF-8"));
        assert!(!is_polish("C"));
        assert!(!is_polish(""));
        assert!(!is_polish("plx_XX"));
    }
}
