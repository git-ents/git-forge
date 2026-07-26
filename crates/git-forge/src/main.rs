//! `git-forge`: A Git subcommand for store, anchor, and query.

use clap::Parser;

#[derive(Parser)]
#[command(name = "git-forge", about = "Forge software on Git", version)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
    println!("Hello, git-forge!");
}
