fn main() {
    if let Err(error) = snap_rs::run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
