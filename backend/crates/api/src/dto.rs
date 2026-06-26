//! Shared DTO helpers and formatting used across route modules.

/// Format integer cents as a USD string, e.g. `185000 -> "$1,850"`.
pub fn usd(cents: i64) -> String {
    let dollars = cents / 100;
    let mut s = String::new();
    let digits = dollars.abs().to_string();
    let bytes = digits.as_bytes();
    let len = bytes.len();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (len - i) % 3 == 0 {
            s.push(',');
        }
        s.push(*b as char);
    }
    if dollars < 0 {
        format!("-${s}")
    } else {
        format!("${s}")
    }
}

#[cfg(test)]
mod tests {
    use super::usd;
    #[test]
    fn formats_thousands() {
        assert_eq!(usd(185000), "$1,850");
        assert_eq!(usd(5060000), "$50,600");
        assert_eq!(usd(0), "$0");
    }
}
