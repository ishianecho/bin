#[cfg(unix)]
fn reset_sigpipe() {
    unsafe {
        signal(13, 0);
    }
}

#[cfg(unix)]
extern "C" {
    fn signal(signum: i32, handler: usize) -> usize;
}

#[cfg(not(unix))]
fn reset_sigpipe() {}

fn main() {
    reset_sigpipe();
    if let Err(message) = bin::xrayctl::cli::run() {
        eprintln!("Error: {message}");
        std::process::exit(1);
    }
}
