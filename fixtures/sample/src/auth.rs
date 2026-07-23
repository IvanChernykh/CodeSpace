/// Authenticate a user without logging credentials.
pub fn authenticate(username: &str, password: &str) -> Result<String, String> {
    if username.is_empty() || password.is_empty() {
        return Err("missing credentials".to_string());
    }
    Ok(format!("session:{username}"))
}

pub fn validate_session(session: &str) -> bool {
    session.starts_with("session:")
}
