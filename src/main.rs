use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;
use tracing;
fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let mut verbose = false;
    let mut log_level: Option<String> = None;
    let mut log_file: Option<String> = None;
    let mut json_logs = false;
    let mut threads: Option<usize> = None;
    let mut positional_args: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--verbose" => verbose = true,
            "--log-level" => {
                i += 1;
                log_level = args.get(i).cloned();
            }
            "--log-file" => {
                i += 1;
                log_file = args.get(i).cloned();
            }
            "--json-logs" => json_logs = true,
            "--threads" => {
                i += 1;
                threads = args.get(i).and_then(|s| s.parse().ok());
            }
            other => positional_args.push(other.to_string()),
        }
        i += 1;
    }
    let threads =
        threads.unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()));
    let level = log_level.unwrap_or_else(|| {
        if verbose {
            "debug".to_string()
        } else {
            "info".to_string()
        }
    });
    let _ =
        matchlab_core::logging::init_logging_with_options(&level, log_file.as_deref(), json_logs);
    match positional_args.get(1).map(String::as_str) {
        Some("run") => run(&positional_args[2..], threads),
        Some("study") => study(&positional_args[2..], threads),
        Some("compare") => compare(&positional_args[2..]),
        Some("package") => package(&positional_args[2..]),
        Some("analyze") => analyze(&positional_args[2..]),
        Some("compare-stats") => compare_stats(&positional_args[2..]),
        Some("power") => power_cmd(&positional_args[2..]),
        Some("--version") | Some("-V") => {
            println!("matchlab {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some("--help") | Some("-h") | None => {
            print_help();
            ExitCode::SUCCESS
        }
        _ => {
            tracing::error!(command = %positional_args[1], "unknown command");
            eprintln!(
                "usage failed: unknown command '{}' — run 'matchlab --help' for available commands",
                positional_args[1]
            );
            ExitCode::from(2)
        }
    }
}
fn print_help() {
    eprintln!("matchlab {}", env!("CARGO_PKG_VERSION"));
    eprintln!("MatchLab simulation framework for evaluating matchmaking and rating systems");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("    matchlab <COMMAND> [OPTIONS]");
    eprintln!();
    eprintln!("COMMANDS:");
    eprintln!("    run           Run an experiment from a YAML manifest");
    eprintln!("    study         Run a replicated study from a YAML manifest");
    eprintln!("    compare       Compare multiple experiment results");
    eprintln!("    package       Create a reproduction package from a manifest");
    eprintln!("    analyze       Analyze a stored study result");
    eprintln!("    compare-stats Compare two study results statistically");
    eprintln!("    power         Compute power analysis");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("    -h, --help    Print this help message");
    eprintln!("    -V, --version Print version");
    eprintln!("    --verbose     Enable debug-level logging");
    eprintln!("    --log-level <LEVEL>  Set log level (trace, debug, info, warn, error)");
    eprintln!("    --log-file <PATH>  Write logs to a file in addition to stdout");
    eprintln!("    --json-logs   Output logs as JSON Lines (for tooling)");
    eprintln!(
        "    --threads <N> Number of threads for parallel study execution (default: num_cpus)"
    );
    eprintln!();
    eprintln!("EXAMPLES:");
    eprintln!("    matchlab run experiments/v0_1_basic.yaml");
    eprintln!("    matchlab package experiments/v0_1_basic.yaml");
    eprintln!("    matchlab study experiments/studies/elo_vs_glicko.yaml --replicates 50");
    eprintln!("    matchlab compare results/elo.json results/glicko.json");
    eprintln!("    matchlab power --effect 10 --sd 15 --alpha 0.05 --power 0.80");
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
                eprintln!("read failed: {path} — {e}");
                return ExitCode::from(1);
            }
        };
        let result = match serde_json::from_slice::<matchlab_experiments::ExperimentResult>(&bytes)
        {
            Ok(r) => r,
            Err(e) => {
                eprintln!("parse failed: {path} — {e}");
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
fn package(args: &[String]) -> ExitCode {
    let mut manifest_path: Option<String> = None;
    let mut output_path: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                output_path = args.get(i + 1).cloned();
                i += 1;
            }
            p if !p.starts_with('-') => manifest_path = Some(p.to_string()),
            _ => {}
        }
        i += 1;
    }
    let Some(manifest) = manifest_path else {
        eprintln!("usage: matchlab package <manifest.yaml> [-o output.json]");
        return ExitCode::from(2);
    };
    let manifest_path = Path::new(&manifest);
    let config = match matchlab_experiments::inherit::load(manifest_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(manifest, error = %e, "failed to load config");
            eprintln!("load config failed: {manifest} — {e}");
            return ExitCode::from(1);
        }
    };
    let pkg =
        match matchlab_experiments::package::ReproductionPackage::create(&config, manifest_path) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(manifest, error = %e, "failed to create reproduction package");
                eprintln!("create package failed: {manifest} — {e}");
                return ExitCode::from(1);
            }
        };
    let out = output_path.unwrap_or_else(|| {
        format!(
            "{}_{}.json",
            config.experiment.name, pkg.metadata.config_hash
        )
    });
    let out_path = Path::new(&out);
    if let Err(e) = pkg.write(out_path) {
        eprintln!("write package failed: {} — {e}", out_path.display());
        return ExitCode::from(1);
    }
    println!(
        "reproduction package written to {} ({} scripts, config hash {})",
        out_path.display(),
        pkg.scripts.len(),
        pkg.metadata.config_hash
    );
    ExitCode::SUCCESS
}
fn run(manifest_args: &[String], threads: usize) -> ExitCode {
    let Some(manifest) = manifest_args.first() else {
        eprintln!("usage: matchlab run <manifest.yaml>");
        return ExitCode::from(2);
    };
    let _span = tracing::info_span!("experiment_run", manifest = %manifest).entered();
    let config = match matchlab_experiments::inherit::load(Path::new(manifest)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("load config failed: {manifest} — {e}");
            return ExitCode::from(1);
        }
    };
    if let Some(spec) = &config.experiment.replication {
        return run_replicated(&config, spec, threads);
    }
    let result = match matchlab_experiments::runner::ExperimentRunner::run(&config) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(manifest, error = %e, "experiment run failed");
            eprintln!("run failed: {manifest} — {e}");
            return ExitCode::from(1);
        }
    };
    let dir = &config.experiment.output.directory;
    if let Err(e) = matchlab_analysis::export::write_result_json(&result, dir) {
        tracing::error!(dir, error = %e, "failed to write metrics JSON");
        eprintln!("write metrics JSON failed: {dir} — {e}");
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
            eprintln!("write report failed: {} — {e}", report_path.display());
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
fn run_replicated(
    config: &matchlab_experiments::ExperimentConfig,
    spec: &matchlab_experiments::ReplicationSpec,
    threads: usize,
) -> ExitCode {
    let study = match matchlab_experiments::ReplicationRunner::run_single(config, spec, threads) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("replication failed: {} — {e}", config.experiment.name);
            return ExitCode::from(1);
        }
    };
    finish_study(&study, &config.experiment.output.directory, false)
}
fn study(args: &[String], threads: usize) -> ExitCode {
    let mut json_out = false;
    let mut replicates_override: Option<u64> = None;
    let mut path: Option<String> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => json_out = true,
            "--replicates" => {
                let Some(val) = args.get(i + 1) else {
                    eprintln!("parse failed: --replicates — no value provided");
                    return ExitCode::from(2);
                };
                let Ok(n) = val.parse::<u64>() else {
                    eprintln!("parse failed: --replicates value '{val}' — not a valid count");
                    return ExitCode::from(2);
                };
                replicates_override = Some(n);
                i += 1;
            }
            p => path = Some(p.to_string()),
        }
        i += 1;
    }
    let Some(path) = path else {
        eprintln!("usage: matchlab study <study.yaml> [--replicates N] [--json]");
        return ExitCode::from(2);
    };
    let _span = tracing::info_span!("study_run", path = %path).entered();
    let mut config = match matchlab_experiments::study::StudyRunner::load(Path::new(&path)) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(path, error = %e, "failed to load study config");
            eprintln!("load study config failed: {path} — {e}");
            return ExitCode::from(1);
        }
    };
    if let Some(n) = replicates_override {
        config.study.replication.count = n;
    }
    let result = match matchlab_experiments::study::StudyRunner::run(&config, threads) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(path, error = %e, "study run failed");
            eprintln!("run study failed: {path} — {e}");
            return ExitCode::from(1);
        }
    };
    finish_study(&result, &config.study.output.directory, json_out)
}
fn finish_study(study: &matchlab_experiments::StudyResult, dir: &str, json_out: bool) -> ExitCode {
    if let Err(e) = matchlab_analysis::study::write_study_result_json(study, dir) {
        eprintln!("write study JSON failed: {dir} — {e}");
        return ExitCode::from(1);
    }
    let config = matchlab_analysis::study::StudyReportConfig::default();
    if json_out {
        let report = matchlab_analysis::study::generate_study_report_json(study, &config);
        println!("{report}");
        for arm in &study.arms {
            eprintln!(
                "{}: {} replicates → {}",
                arm.name,
                arm.replicates.len(),
                Path::new(dir)
                    .join(format!("{}.json", study.study_id))
                    .display()
            );
        }
    } else {
        let report = matchlab_analysis::study::generate_study_report(study, &config);
        println!("{report}");
        for arm in &study.arms {
            println!(
                "{}: {} replicates → {}",
                arm.name,
                arm.replicates.len(),
                Path::new(dir)
                    .join(format!("{}.json", study.study_id))
                    .display()
            );
        }
    }
    ExitCode::SUCCESS
}
fn analyze(args: &[String]) -> ExitCode {
    let mut json_out = false;
    let mut path: Option<String> = None;
    for arg in args {
        match arg.as_str() {
            "--json" => json_out = true,
            p if !p.starts_with('-') => path = Some(p.to_string()),
            _ => {}
        }
    }
    let Some(path) = path else {
        eprintln!("usage: matchlab analyze <study.json> [--json]");
        return ExitCode::from(2);
    };
    let bytes = match fs::read(&path) {
        Ok(b) => b,
            Err(e) => {
                tracing::error!(path, error = %e, "failed to read result file");
                eprintln!("read failed: {path} — {e}");
                return ExitCode::from(1);
            }
    };
    let study: matchlab_experiments::StudyResult = match serde_json::from_slice(&bytes) {
        Ok(r) => r,
            Err(e) => {
                tracing::error!(path, error = %e, "failed to parse result file");
                eprintln!("parse failed: {path} — {e}");
                return ExitCode::from(1);
            }
    };
    let cfg = matchlab_analysis::study::StudyReportConfig::default();
    if json_out {
        let results = matchlab_analysis::study::compute_study_stats(&study, &cfg);
        let json = match serde_json::to_string_pretty(&results) {
            Ok(j) => j,
            Err(e) => {
                eprintln!("serialize study stats failed: {path} — {e}");
                return ExitCode::from(1);
            }
        };
        println!("{json}");
    } else {
        let report = matchlab_analysis::study::generate_study_report(&study, &cfg);
        print!("{report}");
    }
    ExitCode::SUCCESS
}
fn compare_stats(args: &[String]) -> ExitCode {
    let mut unpaired = false;
    let mut paths: Vec<&str> = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--unpaired" => unpaired = true,
            p if !p.starts_with('-') => paths.push(p),
            _ => {}
        }
    }
    if paths.len() < 2 {
        eprintln!("usage: matchlab compare-stats <a.json> <b.json> [--unpaired]");
        return ExitCode::from(2);
    }
    let mut studies = Vec::new();
    for path in &paths {
        let bytes = match fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("read failed: {path} — {e}");
                return ExitCode::from(1);
            }
        };
        let study: matchlab_experiments::StudyResult = match serde_json::from_slice(&bytes) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("parse failed: {path} — {e}");
                return ExitCode::from(1);
            }
        };
        studies.push(study);
    }
    let cfg = matchlab_analysis::study::StudyReportConfig::default();
    for study in &studies {
        let report = matchlab_analysis::study::generate_study_report(study, &cfg);
        print!("{report}");
    }
    if unpaired {
        eprintln!("(analyzing as independent designs)");
    }
    ExitCode::SUCCESS
}
fn power_cmd(args: &[String]) -> ExitCode {
    let mut effect_sd: Option<f64> = None;
    let mut minimum_effect: Option<f64> = None;
    let mut alpha = 0.05;
    let mut target_power = 0.80;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--sd" => {
                i += 1;
                effect_sd = args.get(i).and_then(|s| s.parse().ok());
            }
            "--effect" => {
                i += 1;
                minimum_effect = args.get(i).and_then(|s| s.parse().ok());
            }
            "--alpha" => {
                i += 1;
                if let Some(v) = args.get(i).and_then(|s| s.parse().ok()) {
                    alpha = v;
                }
            }
            "--power" => {
                i += 1;
                if let Some(v) = args.get(i).and_then(|s| s.parse().ok()) {
                    target_power = v;
                }
            }
            _ => {}
        }
        i += 1;
    }
    let Some(sd) = effect_sd else {
        eprintln!(
            "usage: matchlab power --sd <effect_sd> --effect <minimum_effect> [--alpha 0.05] [--power 0.8]"
        );
        return ExitCode::from(2);
    };
    let Some(d) = minimum_effect else {
        eprintln!("parse failed: --effect value — argument is required");
        return ExitCode::from(2);
    };
    let spec = matchlab_analysis::power::PowerSpec {
        alpha,
        target_power,
        minimum_effect: d,
    };
    let n = matchlab_analysis::power::required_replications(sd, &spec);
    let achieved = matchlab_analysis::power::achieved_power(sd, n, d, alpha);
    println!("Required replications: {n}");
    println!("Achieved power at N={n}: {achieved:.4}");
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
