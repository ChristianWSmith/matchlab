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

    if let Err(e) = matchlab_analysis::export::write_result_json(&result, dir) {
        eprintln!("cannot write metrics JSON: {e}");
        return ExitCode::from(1);
    }

    if config.experiment.output.report {
        let report = matchlab_analysis::report::generate_report(&result);
        let report_path = Path::new(dir).join(format!("{}.md", result.name));
        if let Err(e) = fs::write(&report_path, report) {
            eprintln!("cannot write {}: {e}", report_path.display());
            return ExitCode::from(1);
        }
        println!(
            "{}: {} matches in {:.1}s → report: {}",
            result.name,
            result.matches_completed,
            result.simulated_time_secs,
            report_path.display()
        );
    } else {
        println!(
            "{}: {} matches in {:.1}s → {}",
            result.name,
            result.matches_completed,
            result.simulated_time_secs,
            Path::new(dir)
                .join(format!("{}.json", result.name))
                .display()
        );
    }
    ExitCode::SUCCESS
}
