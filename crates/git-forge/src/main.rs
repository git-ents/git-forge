//! The `forge` CLI.

fn main() {
    let cli: git_forge::cli::Cli = figue::from_std_args().unwrap();
    if let Err(e) = git_forge::exe::Executor::discover().and_then(|exe| {
        println!(); // add a newline for better visual separation
        exe.run(&cli)
    }) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}
