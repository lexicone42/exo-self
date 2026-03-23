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
mod traces;

fn main() {
    let cmd = cli::parse();

    if cmd.is_tool() {
        // Tool commands get real exit codes, no panic swallowing
        let code = cli::dispatch(cmd);
        std::process::exit(code as i32);
    } else {
        // Hook commands must NEVER crash — catch panics and exit clean
        let result = panic::catch_unwind(|| cli::dispatch(cmd));
        if result.is_err() {
            println!("{{}}");
        }
    }
}
