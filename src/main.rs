use std::env;
use std::fs;
use std::path::Path;
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

fn run(manifest_args: &[String]) -> ExitCode {
    let Some(manifest) = manifest_args.first() else {
        eprintln!("usage: matchlab run <manifest.yaml>");
        return ExitCode::from(2);
    };

    let config = match matchlab_experiments::inherit::load(Path::new(manifest)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config error: {e}");
            return ExitCode::from(1);
        }
    };

    let result = match matchlab_experiments::runner::ExperimentRunner::run(&config) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("run error: {e}");
            return ExitCode::from(1);
        }
    };

    let dir = &config.experiment.output.directory;
    if let Err(e) = fs::create_dir_all(dir) {
        eprintln!("cannot create output directory {dir}: {e}");
        return ExitCode::from(1);
    }

    let out_path = Path::new(dir).join(format!("{}.json", result.name));
    let json = match serde_json::to_string_pretty(&result) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("serialize error: {e}");
            return ExitCode::from(1);
        }
    };

    if let Err(e) = fs::write(&out_path, json) {
        eprintln!("cannot write {}: {e}", out_path.display());
        return ExitCode::from(1);
    }

    println!(
        "{}: {} matches in {:.1}s → {}",
        result.name,
        result.matches_completed,
        result.simulated_time_secs,
        out_path.display()
    );
    ExitCode::SUCCESS
}
