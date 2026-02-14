use std::panic;

mod cli;
mod commands;
mod config;
mod context_window;
mod hook_io;
mod markdown;
mod meta;
mod paths;
mod project;
mod scaling;
mod state;

fn main() {
    // Hooks must NEVER crash — catch panics and exit clean
    let result = panic::catch_unwind(|| {
        let cmd = cli::parse();
        cli::dispatch(cmd);
    });

    if result.is_err() {
        // Print empty JSON so Claude Code doesn't see an error
        println!("{{}}");
    }
}
