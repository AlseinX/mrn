use std::include;

include!("lib.rs");
mod cli;

fn main() {
    cli::run();
}
