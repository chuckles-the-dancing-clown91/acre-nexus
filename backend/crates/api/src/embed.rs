//! Website embeds: which sites may show a workspace's widgets. Pure, so the
//! rules are tested without a database.

pub const MAX_ORIGINS: usize = 20;

/// Normalise a list of allowed sites (one per line, or comma separated) to
/// `https://host[:port]` lines. Accepts a bare host and adds `https://`.
/// `http://` is allowed only for localhost. Paths and wildcards are refused so
/// a site is named exactly. `None` for an empty list.
pub fn normalize_origins(raw: &str) -> Result<Option<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for item in raw.split(['\n', ',']) {
        let t = item.trim().trim_end_matches('/').to_lowercase();
        if t.is_empty() {
            continue;
        }
        if t.contains('*') {
            return Err("wildcards are not allowed: name each site".into());
        }
        let (scheme, rest) = match t.split_once("://") {
            Some((s, r)) => (s.to_string(), r.to_string()),
            None => ("https".to_string(), t.clone()),
        };
        if rest.is_empty()
            || rest.contains('/')
            || rest.contains('?')
            || rest.contains('#')
            || rest.contains('@')
        {
            return Err(format!(
                "{item}: use just the site, like https://example.com"
            ));
        }
        let host = rest.split(':').next().unwrap_or("");
        let local = host == "localhost" || host == "127.0.0.1";
        if scheme == "http" && !local {
            return Err(format!("{item}: use https"));
        }
        if scheme != "https" && scheme != "http" {
            return Err(format!("{item}: use https"));
        }
        if !host.contains('.') && !local {
            return Err(format!("{item}: that does not look like a website"));
        }
        if !host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            return Err(format!("{item}: that does not look like a website"));
        }
        let o = format!("{scheme}://{rest}");
        if !out.contains(&o) {
            out.push(o);
        }
    }
    if out.len() > MAX_ORIGINS {
        return Err(format!("at most {MAX_ORIGINS} sites"));
    }
    Ok((!out.is_empty()).then(|| out.join("\n")))
}

/// The list as a vector.
pub fn origins(stored: Option<&str>) -> Vec<String> {
    stored
        .unwrap_or("")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_dedupes() {
        let r = normalize_origins(
            "Example.com\nhttps://example.com/\nhttps://www.example.com, http://localhost:3000",
        )
        .unwrap();
        assert_eq!(
            r.as_deref(),
            Some("https://example.com\nhttps://www.example.com\nhttp://localhost:3000")
        );
        assert_eq!(normalize_origins("  \n ").unwrap(), None);
    }

    #[test]
    fn refuses_wildcards_paths_and_plain_http() {
        for bad in [
            "*.example.com",
            "https://example.com/page",
            "http://example.com",
            "ftp://example.com",
            "intranet",
            "https://exa mple.com",
            "https://a@example.com",
        ] {
            assert!(normalize_origins(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn caps_the_list() {
        let many: String = (0..21)
            .map(|i| format!("https://s{i}.example.com\n"))
            .collect();
        assert!(normalize_origins(&many).is_err());
    }

    #[test]
    fn reads_back_as_a_list() {
        assert_eq!(
            origins(Some("https://a.co\nhttps://b.co")),
            vec!["https://a.co", "https://b.co"]
        );
        assert!(origins(None).is_empty());
    }
}
