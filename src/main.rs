fn main() {
    match codespace::cli::run(std::env::args()) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("cse: {error}");
            std::process::exit(2);
        }
    }
}
