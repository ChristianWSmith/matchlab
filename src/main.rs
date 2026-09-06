use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("run") => run(&args[2..]),
        Some("compare") => compare(&args[2..]),
        _ => {
            eprintln!("usage: matchlab run <manifest.yaml>");
            eprintln!("       matchlab compare <result.json>... [--json]");
            ExitCode::from(2)
        }
    }
}

fn compare(args: &[String]) -> ExitCode {
    let mut json_out = false;
    let mut paths: Vec<&str> = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--json" => json_out = true,
            p => paths.push(p),
        }
    }
    if paths.is_empty() {
        eprintln!("usage: matchlab compare <result.json>... [--json]");
        return ExitCode::from(2);
    }

    let mut results = Vec::new();
    for path in &paths {
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("cannot read {path}: {e}");
                return ExitCode::from(1);
            }
        };
        let result = match serde_json::from_slice::<matchlab_experiments::ExperimentResult>(&bytes)
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("cannot parse {path}: {e}");
                return ExitCode::from(1);
            }
        };
        results.push(result);
    }

    let format = if json_out {
        matchlab_analysis::report::ReportFormat::Json
    } else {
        matchlab_analysis::report::ReportFormat::Markdown
    };
    let config = matchlab_analysis::report::ReportConfig {
        include_plots: false,
        include_raw_data: false,
        format,
    };
    let report = matchlab_analysis::report::generate_comparison_report(&results, &config);
    println!("{report}");

    let comparator = matchlab_analysis::comparator::Comparator::new(results);
    let ranked = comparator.ranking();
    if !ranked.is_empty() {
        println!("# Utility ranking\n");
        for (i, (r, score)) in ranked.iter().enumerate() {
            println!("{}. {} ({:.4})", i + 1, r.name, score);
        }
    }
    ExitCode::SUCCESS
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

    let features = feature_summary(&config);
    let utility = result
        .utility_score
        .map(|s| format!(", utility {s:.4}"))
        .unwrap_or_default();

    if config.experiment.output.report {
        let report = matchlab_analysis::report::generate_report(&result);
        let report_path = Path::new(dir).join(format!("{}.md", result.name));
        if let Err(e) = fs::write(&report_path, report) {
            eprintln!("cannot write {}: {e}", report_path.display());
            return ExitCode::from(1);
        }
        println!(
            "{}: {} matches in {:.1}s{} → report: {}",
            result.name,
            result.matches_completed,
            result.simulated_time_secs,
            utility,
            report_path.display()
        );
    } else {
        println!(
            "{}: {} matches in {:.1}s{} → {}",
            result.name,
            result.matches_completed,
            result.simulated_time_secs,
            utility,
            Path::new(dir)
                .join(format!("{}.json", result.name))
                .display()
        );
    }
    if !features.is_empty() {
        println!("features: {features}");
    }
    ExitCode::SUCCESS
}

fn feature_summary(config: &matchlab_experiments::ExperimentConfig) -> String {
    let exp = &config.experiment;
    let mut parts: Vec<String> = Vec::new();
    if exp.detection.as_ref().map(|d| d.enabled).unwrap_or(false) {
        parts.push("detection".to_string());
    }
    if exp.ranking.is_some() {
        parts.push("ranking".to_string());
    }
    if exp
        .adversarial
        .as_ref()
        .map(|a| !a.agents.is_empty())
        .unwrap_or(false)
    {
        parts.push("adversarial".to_string());
    }
    if exp
        .satisfaction
        .as_ref()
        .map(|s| s.enabled)
        .unwrap_or(false)
    {
        parts.push("satisfaction".to_string());
    }
    if exp.game.script != "plugins/game/logistic.lua" {
        parts.push(format!("outcome:{}", exp.game.script));
    }
    if exp.matchmaking.script != "plugins/matchmaking/batch.lua" {
        parts.push(format!("matchmaker:{}", exp.matchmaking.script));
    }
    parts.join(", ")
}
