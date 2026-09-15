use matchlab_metrics::MetricResult;
use plotters::prelude::*;
use std::collections::BTreeMap;
use std::path::Path;

pub fn generate_plots(
    metrics: &BTreeMap<String, MetricResult>,
    directory: &str,
    experiment_name: &str,
) -> Result<(), String> {
    std::fs::create_dir_all(directory)
        .map_err(|e| format!("create plot directory: {e}"))?;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        generate_plots_inner(metrics, directory, experiment_name)
    }));

    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("plot rendering unavailable (font backend missing)".to_string()),
    }
}

fn generate_plots_inner(
    metrics: &BTreeMap<String, MetricResult>,
    directory: &str,
    experiment_name: &str,
) -> Result<(), String> {

    let summary_metrics: Vec<(&str, f64)> = metrics
        .iter()
        .filter_map(|(k, v)| match v {
            MetricResult::Summary { mean, .. } => Some((k.as_str(), *mean)),
            _ => None,
        })
        .collect();

    if !summary_metrics.is_empty() {
        if let Err(e) = draw_summary_bar_chart(&summary_metrics, directory, experiment_name) {
            return Err(format!("summary bar chart: {e}"));
        }
    }

    for (name, result) in metrics {
        if let MetricResult::TimeSeries { bucket_means } = result {
            if let Err(e) = draw_time_series(bucket_means, directory, &format!("{experiment_name}_{name}")) {
                return Err(format!("time series {name}: {e}"));
            }
        }
        if let MetricResult::Histogram { buckets } = result {
            if let Err(e) = draw_histogram(buckets, directory, &format!("{experiment_name}_{name}")) {
                return Err(format!("histogram {name}: {e}"));
            }
        }
    }

    Ok(())
}

fn draw_summary_bar_chart(
    metrics: &[(&str, f64)],
    directory: &str,
    experiment_name: &str,
) -> Result<(), String> {
    if metrics.is_empty() {
        return Ok(());
    }
    let path = Path::new(directory).join(format!("{experiment_name}_metrics.png"));
    let root = BitMapBackend::new(&path, (800, 500)).into_drawing_area();
    root.fill(&WHITE)
        .map_err(|e| format!("draw background: {e}"))?;

    let n = metrics.len();
    let means: Vec<f64> = metrics.iter().map(|(_, m)| *m).collect();
    let max_val = means.iter().cloned().fold(f64::MIN, f64::max);
    let min_val = means.iter().cloned().fold(f64::MAX, f64::min);
    let y_range = if (max_val - min_val).abs() < 1e-12 {
        0.0..max_val * 1.2
    } else {
        (min_val - (max_val - min_val) * 0.1)..(max_val + (max_val - min_val) * 0.1)
    };

    let mut chart = ChartBuilder::on(&root)
        .caption(
            format!("Metric Summary — {experiment_name}"),
            ("sans-serif", 20).into_font(),
        )
        .x_label_area_size(35)
        .y_label_area_size(40)
        .build_cartesian_2d(0usize..n, y_range)
        .map_err(|e| format!("build chart: {e}"))?;

    chart
        .configure_mesh()
        .x_labels(n)
        .x_label_formatter(&|x| {
            metrics.get(*x).map(|(name, _)| (*name).to_string()).unwrap_or_default()
        })
        .label_style(("sans-serif", 12))
        .draw()
        .map_err(|e| format!("draw mesh: {e}"))?;

    chart
        .draw_series((0..n).zip(means.iter()).map(|(i, m)| {
            Rectangle::new(
                [(i, 0.0), (i + 1, *m)],
                Palette99::pick(i).mix(0.7).filled(),
            )
        }))
        .map_err(|e| format!("draw bars: {e}"))?;

    root.present()
        .map_err(|e| format!("save plot: {e}"))?;
    Ok(())
}

fn draw_time_series(
    bucket_means: &[f64],
    directory: &str,
    name: &str,
) -> Result<(), String> {
    let path = Path::new(directory).join(format!("{name}.png"));
    let root = BitMapBackend::new(&path, (800, 400)).into_drawing_area();
    root.fill(&WHITE)
        .map_err(|e| format!("draw background: {e}"))?;

    if bucket_means.is_empty() {
        return Ok(());
    }

    let max_val = bucket_means.iter().cloned().fold(f64::MIN, f64::max);
    let min_val = bucket_means.iter().cloned().fold(f64::MAX, f64::min);
    let y_range = if (max_val - min_val).abs() < 1e-12 {
        0.0..max_val * 1.2
    } else {
        (min_val - (max_val - min_val) * 0.1)..(max_val + (max_val - min_val) * 0.1)
    };

    let n = bucket_means.len();
    let data: Vec<(usize, f64)> = bucket_means.iter().enumerate().map(|(i, &v)| (i, v)).collect();

    let mut chart = ChartBuilder::on(&root)
        .caption(
            format!("{name}"),
            ("sans-serif", 18).into_font(),
        )
        .x_label_area_size(35)
        .y_label_area_size(40)
        .build_cartesian_2d(0usize..n, y_range)
        .map_err(|e| format!("build chart: {e}"))?;

    chart
        .configure_mesh()
        .x_labels(10)
        .x_label_formatter(&|x| format!("{}", x + 1))
        .label_style(("sans-serif", 12))
        .draw()
        .map_err(|e| format!("draw mesh: {e}"))?;

    chart
        .draw_series(LineSeries::new(data, &BLUE))
        .map_err(|e| format!("draw series: {e}"))?;

    root.present()
        .map_err(|e| format!("save plot: {e}"))?;
    Ok(())
}

fn draw_histogram(
    buckets: &[(f64, u64)],
    directory: &str,
    name: &str,
) -> Result<(), String> {
    let path = Path::new(directory).join(format!("{name}.png"));
    let root = BitMapBackend::new(&path, (800, 400)).into_drawing_area();
    root.fill(&WHITE)
        .map_err(|e| format!("draw background: {e}"))?;

    if buckets.is_empty() {
        return Ok(());
    }

    let max_count = buckets.iter().map(|(_, c)| *c).max().unwrap_or(1) as f64;
    let bar_count = buckets.len();
    let y_range = 0.0..(max_count * 1.1);

    let mut chart = ChartBuilder::on(&root)
        .caption(
            format!("{name}"),
            ("sans-serif", 18).into_font(),
        )
        .x_label_area_size(35)
        .y_label_area_size(40)
        .build_cartesian_2d(0usize..bar_count, y_range)
        .map_err(|e| format!("build chart: {e}"))?;

    chart
        .configure_mesh()
        .x_labels(bar_count.min(10))
        .x_label_formatter(&|x| {
            buckets.get(*x).map(|(edge, _)| format!("{edge:.1}")).unwrap_or_default()
        })
        .label_style(("sans-serif", 10))
        .draw()
        .map_err(|e| format!("draw mesh: {e}"))?;

    chart
        .draw_series((0..bar_count).zip(buckets.iter()).map(|(i, (_, count))| {
            Rectangle::new(
                [(i, 0.0), (i + 1, *count as f64)],
                Palette99::pick(i).mix(0.6).filled(),
            )
        }))
        .map_err(|e| format!("draw bars: {e}"))?;

    root.present()
        .map_err(|e| format!("save plot: {e}"))?;
    Ok(())
}
