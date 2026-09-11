use crate::acquisition::{expected_improvement, parego_scalarize, parego_weights};
use crate::config::{
    BoConfig, Direction, ObjectiveSpec, OptConfig, OptimizationResult, ParameterSpec, TrialResult,
};
use crate::gp::GaussianProcess;
use crate::multiobj::ParetoFront;
use crate::sampler::{latin_hypercube_sample, random_sample};
use matchlab_experiments::config::ExperimentConfig;
use matchlab_experiments::inherit;
use matchlab_experiments::runner::ExperimentRunner;
use ndarray::{Array1, Array2};
use rand::Rng;
use rand::SeedableRng;
use std::collections::BTreeMap;
use std::path::Path;
use tracing;

pub fn optimize(config: &OptConfig) -> Result<OptimizationResult, String> {
    let base_path = Path::new(&config.base);
    let base_config =
        inherit::load(base_path).map_err(|e| format!("load base config {}: {e}", config.base))?;

    let param_order: Vec<String> = config.search_space.parameters.keys().cloned().collect();
    let param_indices = classify_parameters(&config.search_space);

    let initial_n = config.bo.initial_points.min(config.budget);
    let initial_points = match config.bo.initial_design.as_str() {
        "latin_hypercube" => latin_hypercube_sample(&config.search_space, initial_n, config.seed),
        _ => random_sample(&config.search_space, initial_n, config.seed),
    };

    let mut all_objectives: Vec<Vec<f64>> = Vec::new();
    let mut all_params: Vec<BTreeMap<String, f64>> = Vec::new();
    let mut all_trial_results: Vec<TrialResult> = Vec::new();
    let mut pareto = ParetoFront::new();
    let directions: Vec<bool> = config
        .objectives
        .iter()
        .map(|o| o.direction == Direction::Maximize)
        .collect();

    tracing::info!(
        budget = config.budget,
        initial = initial_n,
        "starting Bayesian optimization"
    );

    for (i, point) in initial_points.iter().enumerate() {
        let trial_idx = i as u64;
        tracing::info!(trial = trial_idx + 1, "evaluating initial point");
        match evaluate_point(
            &base_config,
            point,
            &param_order,
            &config.objectives,
            config.seed + trial_idx,
        ) {
            Ok((obj_vals, trial_result)) => {
                all_objectives.push(obj_vals);
                all_params.push(point.clone());
                all_trial_results.push(trial_result);
                pareto.update(&all_objectives, &directions);
                tracing::info!(
                    trial = trial_idx + 1,
                    best = best_objective_value(&all_objectives, &config.objectives),
                    "initial point evaluated"
                );
            }
            Err(e) => {
                tracing::warn!(trial = trial_idx + 1, error = %e, "initial point evaluation failed, skipping");
            }
        }
    }

    for t in initial_n..config.budget {
        let trial_idx = t;
        tracing::info!(
            trial = trial_idx + 1,
            total = config.budget,
            "optimization step"
        );

        let next_point = if all_params.len() < 2 {
            let mut points = random_sample(&config.search_space, 1, config.seed + trial_idx * 1000);
            points.pop().unwrap()
        } else {
            suggest_next_point(
                &all_params,
                &all_objectives,
                &config.search_space,
                &param_indices,
                &config.objectives,
                &config.bo,
                config.seed + trial_idx * 1000,
            )?
        };

        match evaluate_point(
            &base_config,
            &next_point,
            &param_order,
            &config.objectives,
            config.seed + trial_idx,
        ) {
            Ok((obj_vals, trial_result)) => {
                all_objectives.push(obj_vals);
                all_params.push(next_point);
                all_trial_results.push(trial_result);
                pareto.update(&all_objectives, &directions);
                tracing::info!(
                    trial = trial_idx + 1,
                    best = best_objective_value(&all_objectives, &config.objectives),
                    "trial completed"
                );
            }
            Err(e) => {
                tracing::warn!(trial = trial_idx + 1, error = %e, "evaluation failed, retrying with random point");
                let mut points =
                    random_sample(&config.search_space, 1, config.seed + trial_idx * 777);
                if let Some(rand_point) = points.pop() {
                    if let Ok((obj_vals, trial_result)) = evaluate_point(
                        &base_config,
                        &rand_point,
                        &param_order,
                        &config.objectives,
                        config.seed + trial_idx + 50000,
                    ) {
                        all_objectives.push(obj_vals);
                        all_params.push(rand_point);
                        all_trial_results.push(trial_result);
                        pareto.update(&all_objectives, &directions);
                    }
                }
            }
        }
    }

    let best_idx = find_best_trial_index(&all_objectives, &config.objectives);
    let config_hash = matchlab_experiments::seed::hash_config(&base_config);
    let git_commit = matchlab_experiments::seed::git_commit_hash();
    let timestamp = iso8601_utc();

    Ok(OptimizationResult {
        name: config.name.clone(),
        seed: config.seed,
        budget: config.budget,
        search_space: config.search_space.clone(),
        objectives: config.objectives.clone(),
        trials: all_trial_results,
        best_index: best_idx,
        pareto_indices: pareto.indices,
        config_hash,
        git_commit,
        timestamp,
    })
}

fn iso8601_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 153 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

fn classify_parameters(space: &crate::config::SearchSpace) -> ParamIndices {
    let mut cont = Vec::new();
    let mut cat = Vec::new();
    let mut cat_n_levels = Vec::new();

    for (i, (_name, spec)) in space.parameters.iter().enumerate() {
        match spec {
            ParameterSpec::Float { .. } => cont.push(i),
            ParameterSpec::Categorical { values } => {
                cat.push(i);
                cat_n_levels.push(values.len());
            }
        }
    }
    ParamIndices {
        cont,
        cat,
        cat_n_levels,
    }
}

struct ParamIndices {
    cont: Vec<usize>,
    cat: Vec<usize>,
    cat_n_levels: Vec<usize>,
}

fn suggest_next_point(
    all_params: &[BTreeMap<String, f64>],
    all_objectives: &[Vec<f64>],
    space: &crate::config::SearchSpace,
    indices: &ParamIndices,
    objectives: &[ObjectiveSpec],
    bo: &BoConfig,
    seed: u64,
) -> Result<BTreeMap<String, f64>, String> {
    let param_order: Vec<String> = space.parameters.keys().cloned().collect();
    let n_obj = objectives.len();

    let x_train = build_training_matrix(all_params, &param_order, &indices.cont, &indices.cat);
    let directions: Vec<bool> = objectives
        .iter()
        .map(|o| o.direction == Direction::Maximize)
        .collect();

    if n_obj == 1 {
        let y_train = build_single_objective(all_objectives, 0, directions[0]);
        let params_gp = GaussianProcess::optimize_hyperparameters(
            &x_train,
            &y_train,
            &indices.cont,
            &indices.cat,
            &indices.cat_n_levels,
            seed,
        );
        let gp = GaussianProcess::fit(&x_train, &y_train, &params_gp, &indices.cont)?;
        let best_y = y_train.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let xi = bo.xi.unwrap_or(0.01);

        let candidates = random_candidates(space, 1000, seed);
        let x_cand = build_training_matrix(&candidates, &param_order, &indices.cont, &indices.cat);
        let ei = expected_improvement(&gp, &x_cand, best_y, xi);
        let best_cand = ei
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);

        Ok(candidates[best_cand].clone())
    } else {
        let weights = parego_weights(n_obj, seed);

        let mut best_scalarized = f64::NEG_INFINITY;
        for obj in all_objectives {
            let s = parego_scalarize(obj, &weights, &directions);
            if s > best_scalarized {
                best_scalarized = s;
            }
        }

        let y_train = build_scalarized_objective(all_objectives, &weights, &directions);
        let params_gp = GaussianProcess::optimize_hyperparameters(
            &x_train,
            &y_train,
            &indices.cont,
            &indices.cat,
            &indices.cat_n_levels,
            seed,
        );
        let gp = GaussianProcess::fit(&x_train, &y_train, &params_gp, &indices.cont)?;
        let xi = bo.xi.unwrap_or(0.01);

        let candidates = random_candidates(space, 1000, seed);
        let x_cand = build_training_matrix(&candidates, &param_order, &indices.cont, &indices.cat);
        let ei = expected_improvement(&gp, &x_cand, best_scalarized, xi);
        let best_cand = ei
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);

        Ok(candidates[best_cand].clone())
    }
}

fn build_training_matrix(
    points: &[BTreeMap<String, f64>],
    param_order: &[String],
    _cont_indices: &[usize],
    _cat_indices: &[usize],
) -> Array2<f64> {
    let n = points.len();
    let d = param_order.len();
    let mut x = Array2::<f64>::zeros((n, d));

    for (i, point) in points.iter().enumerate() {
        for (j, name) in param_order.iter().enumerate() {
            x[[i, j]] = point[name];
        }
    }
    x
}

fn build_single_objective(all_objectives: &[Vec<f64>], idx: usize, maximize: bool) -> Array1<f64> {
    let vals: Vec<f64> = all_objectives
        .iter()
        .map(|obj| if maximize { obj[idx] } else { -obj[idx] })
        .collect();
    Array1::from_vec(vals)
}

fn build_scalarized_objective(
    all_objectives: &[Vec<f64>],
    weights: &[f64],
    directions: &[bool],
) -> Array1<f64> {
    let vals: Vec<f64> = all_objectives
        .iter()
        .map(|obj| parego_scalarize(obj, weights, directions))
        .collect();
    Array1::from_vec(vals)
}

fn random_candidates(
    space: &crate::config::SearchSpace,
    n: usize,
    seed: u64,
) -> Vec<BTreeMap<String, f64>> {
    let param_order: Vec<String> = space.parameters.keys().cloned().collect();
    let mut rng = rand::rngs::SmallRng::seed_from_u64(seed);
    let mut candidates = Vec::with_capacity(n);

    for _ in 0..n {
        let mut point = BTreeMap::new();
        for name in &param_order {
            match space.parameters.get(name).unwrap() {
                ParameterSpec::Float { bounds, .. } => {
                    point.insert(name.clone(), rng.gen_range(bounds[0]..bounds[1]));
                }
                ParameterSpec::Categorical { values } => {
                    let idx = rng.gen_range(0..values.len());
                    point.insert(name.clone(), idx as f64);
                }
            }
        }
        candidates.push(point);
    }
    candidates
}

fn evaluate_point(
    base_config: &ExperimentConfig,
    point: &BTreeMap<String, f64>,
    _param_order: &[String],
    objectives: &[ObjectiveSpec],
    seed: u64,
) -> Result<(Vec<f64>, TrialResult), String> {
    let mut config = base_config.clone();
    config.experiment.seed = seed;

    for (name, &val) in point {
        apply_parameter(&mut config, name, val);
    }

    let result = ExperimentRunner::run(&config)?;

    let obj_vals: Vec<f64> = objectives
        .iter()
        .map(|obj| extract_metric_value(&result.metrics, &obj.metric))
        .collect();

    let params_yaml: BTreeMap<String, serde_yaml::Value> = point
        .iter()
        .map(|(k, &v)| {
            let yaml_val = serde_yaml::Value::Number(serde_yaml::Number::from(v));
            (k.clone(), yaml_val)
        })
        .collect();

    let trial = TrialResult {
        trial_index: 0,
        parameters: params_yaml,
        objectives: obj_vals.clone(),
        utility_score: result.utility_score,
        matches_completed: result.matches_completed,
        simulated_time_secs: result.simulated_time_secs,
    };

    Ok((obj_vals, trial))
}

fn apply_parameter(config: &mut ExperimentConfig, path: &str, val: f64) {
    let yaml_val = serde_yaml::Value::Number(serde_yaml::Number::from(val));
    matchlab_experiments::factorial::set_nested_value(config, path, yaml_val);
}

fn extract_metric_value(
    metrics: &BTreeMap<String, matchlab_metrics::MetricResult>,
    metric_name: &str,
) -> f64 {
    if let Some(result) = metrics.get(metric_name) {
        match result {
            matchlab_metrics::MetricResult::Scalar(v) => *v,
            matchlab_metrics::MetricResult::Summary { mean, .. } => *mean,
            matchlab_metrics::MetricResult::TimeSeries { bucket_means } => {
                bucket_means.iter().sum::<f64>() / bucket_means.len() as f64
            }
            matchlab_metrics::MetricResult::Distribution(v) => {
                v.iter().sum::<f64>() / v.len() as f64
            }
            _ => 0.0,
        }
    } else {
        0.0
    }
}

fn find_best_trial_index(all_objectives: &[Vec<f64>], objectives: &[ObjectiveSpec]) -> usize {
    let directions: Vec<bool> = objectives
        .iter()
        .map(|o| o.direction == Direction::Maximize)
        .collect();

    if objectives.len() == 1 {
        let idx = 0;
        let maximize = directions[0];
        all_objectives
            .iter()
            .enumerate()
            .max_by(|a, b| {
                let va = if maximize { a.1[idx] } else { -a.1[idx] };
                let vb = if maximize { b.1[idx] } else { -b.1[idx] };
                va.partial_cmp(&vb).unwrap()
            })
            .map(|(i, _)| i)
            .unwrap_or(0)
    } else {
        let weights = parego_weights(objectives.len(), 12345);
        let mut best_idx = 0;
        let mut best_val = f64::NEG_INFINITY;
        for (i, obj) in all_objectives.iter().enumerate() {
            let s = parego_scalarize(obj, &weights, &directions);
            if s > best_val {
                best_val = s;
                best_idx = i;
            }
        }
        best_idx
    }
}

fn best_objective_value(all_objectives: &[Vec<f64>], objectives: &[ObjectiveSpec]) -> f64 {
    let best_idx = find_best_trial_index(all_objectives, objectives);
    if let Some(obj) = all_objectives.get(best_idx) {
        obj.first().copied().unwrap_or(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_metric_value_summary() {
        let mut metrics = BTreeMap::new();
        metrics.insert(
            "match_quality".to_string(),
            matchlab_metrics::MetricResult::Summary {
                mean: 0.95,
                median: 0.96,
                p75: 0.98,
                p90: 0.99,
                p95: 0.995,
                p99: 1.0,
                stddev: 0.02,
            },
        );
        let val = extract_metric_value(&metrics, "match_quality");
        assert!((val - 0.95).abs() < 1e-10);
    }

    #[test]
    fn extract_metric_value_scalar() {
        let mut metrics = BTreeMap::new();
        metrics.insert(
            "stability".to_string(),
            matchlab_metrics::MetricResult::Scalar(42.0),
        );
        let val = extract_metric_value(&metrics, "stability");
        assert!((val - 42.0).abs() < 1e-10);
    }
}
