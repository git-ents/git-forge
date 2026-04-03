//! Input validation helpers for MCP tool parameters.

pub(crate) fn validate_oid(value: &str) -> Result<(), String> {
    if value.len() != 40 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!(
            "invalid OID: expected 40 hex characters, got {value:?}"
        ));
    }
    Ok(())
}

pub(crate) fn validate_uuid(value: &str) -> Result<(), String> {
    if value.len() != 36
        || !value.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
    {
        return Err(format!(
            "invalid UUID: expected 36-char UUID, got {value:?}"
        ));
    }
    Ok(())
}
