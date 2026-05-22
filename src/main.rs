fn main() {
    if let Err(error) = lgtm::run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
