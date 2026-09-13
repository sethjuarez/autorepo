use anyhow::{Result, bail};

pub fn format_marker(pack_id: &str, resource_id: &str) -> Result<String> {
    Ok(format!(
        "<!-- {} -->",
        format_marker_token(pack_id, resource_id)?
    ))
}

pub fn format_marker_token(pack_id: &str, resource_id: &str) -> Result<String> {
    validate_marker_part("pack", pack_id)?;
    validate_marker_part("id", resource_id)?;
    Ok(format!("autorepo:pack={pack_id};id={resource_id}"))
}

fn validate_marker_part(name: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        bail!("{name} marker value must contain only ASCII letters, numbers, '.', '_' or '-'");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{format_marker, format_marker_token};

    #[test]
    fn formats_marker() {
        let marker = format_marker("generic-starter", "issue.improve-readme").unwrap();
        assert_eq!(
            marker,
            "<!-- autorepo:pack=generic-starter;id=issue.improve-readme -->"
        );
    }

    #[test]
    fn formats_marker_token() {
        let marker = format_marker_token("generic-starter", "file.ci").unwrap();
        assert_eq!(marker, "autorepo:pack=generic-starter;id=file.ci");
    }

    #[test]
    fn rejects_unsafe_marker_parts() {
        assert!(format_marker("generic starter", "issue.one").is_err());
        assert!(format_marker("generic-starter", "issue;one").is_err());
    }
}
