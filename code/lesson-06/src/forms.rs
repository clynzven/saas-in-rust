//! Small, explicit validation helpers. Each returns a message a person can act on.

pub fn email(raw: &str) -> Result<String, String> {
    let e = raw.trim().to_lowercase();
    let ok = e.len() <= 254
        && e.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
        })
        && !e.contains(char::is_whitespace);
    if ok {
        Ok(e)
    } else {
        Err("Please enter a valid email address.".into())
    }
}

pub fn password(raw: &str) -> Result<(), String> {
    match raw.chars().count() {
        n if n < 10 => Err("Use at least 10 characters for your password.".into()),
        n if n > 128 => Err("That password is too long (128 characters at most).".into()),
        _ => Ok(()),
    }
}

pub fn text(raw: &str, what: &str, max: usize) -> Result<String, String> {
    let t = raw.trim();
    if t.is_empty() {
        Err(format!("Please enter {what}."))
    } else if t.chars().count() > max {
        Err(format!("{what} can be {max} characters at most."))
    } else {
        Ok(t.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emails() {
        assert_eq!(email("  Ana@Example.COM ").unwrap(), "ana@example.com");
        for bad in [
            "",
            "ana",
            "ana@",
            "@x.com",
            "ana@localhost",
            "a na@x.com",
            "ana@x.",
        ] {
            assert!(email(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn passwords() {
        assert!(password("short").is_err());
        assert!(password("long enough!").is_ok());
        assert!(password(&"x".repeat(129)).is_err());
    }
}
