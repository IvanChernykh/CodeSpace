use crate::auth::{authenticate, validate_session};

pub fn handle_login(username: &str, password: &str) -> bool {
    authenticate(username, password)
        .map(|session| validate_session(&session))
        .unwrap_or(false)
}
