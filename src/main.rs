use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("run") => run(&args[2..]),
        _ => {
            eprintln!("usage: matchlab run <manifest.yaml>");
            ExitCode::from(2)
        }
    }
}

fn run(_manifest_args: &[String]) -> ExitCode {
    // The v0.1 runner (experiment config parsing + simulation) lands in
    // tickets 10-11. This is the CLI skeleton for ticket 01.
    eprintln!("matchlab run: not yet implemented (ticket 10)");
    ExitCode::from(1)
}
