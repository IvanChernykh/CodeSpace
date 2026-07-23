pub mod auth;
pub mod service;

pub fn bootstrap() -> bool {
    service::handle_login("demo", "demo")
}
