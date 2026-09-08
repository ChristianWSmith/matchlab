//! Visualization API: first-class visualization abstraction
//! with plot types focused on research questions.
use crate::dataframe::ResearchDataFrame;
/// An axis specification for a plot.
#[derive(Debug, Clone)]
pub struct AxisSpec {
    pub label: String,
    pub scale: AxisScale,
}
#[derive(Debug, Clone, Default)]
pub enum AxisScale {
    #[default]
    Linear,
    Logarithmic,
}
/// Data source for a plot.
#[derive(Debug, Clone)]
pub enum PlotData {
    Table(ResearchDataFrame),
}
/// A mark (geometric element) in a plot.
#[derive(Debug, Clone)]
pub enum Mark {
    Points {
        x: String,
        y: String,
    },
    Lines {
        x: String,
        y: String,
    },
    Bars {
        x: String,
        y: String,
    },
    ErrorBars {
        x: String,
        y_lo: String,
        y_hi: String,
    },
}
/// A plot specification.
#[derive(Debug, Clone)]
pub struct PlotSpec {
    pub title: String,
    pub x_axis: AxisSpec,
    pub y_axis: AxisSpec,
    pub data: PlotData,
    pub marks: Vec<Mark>,
}
/// Output from rendering a plot.
#[derive(Debug, Clone)]
pub enum PlotOutput {
    Text(String),
    Json(String),
}
/// A trait for rendering plots.
pub trait PlotRenderer: Send + Sync {
    fn render(&self, plot: &PlotSpec) -> PlotOutput;
}
/// Text renderer: produces ASCII/text representation.
pub struct TextRenderer;
impl PlotRenderer for TextRenderer {
    fn render(&self, plot: &PlotSpec) -> PlotOutput {
        let mut out = format!("=== {} ===\n", plot.title);
        out += &format!("X: {}\n", plot.x_axis.label);
        out += &format!("Y: {}\n", plot.y_axis.label);
        out += &format!("Marks: {}\n", plot.marks.len());
        PlotOutput::Text(out)
    }
}
/// JSON renderer: produces JSON representation.
pub struct JsonRenderer;
impl PlotRenderer for JsonRenderer {
    fn render(&self, plot: &PlotSpec) -> PlotOutput {
        let json = serde_json::json!({
            "title": plot.title,
            "x_axis": plot.x_axis.label,
            "y_axis": plot.y_axis.label,
            "marks": plot.marks.len(),
        });
        PlotOutput::Json(json.to_string())
    }
}
/// Convenience function: create a distribution plot.
pub fn plot_distribution(data: &ResearchDataFrame, column: &str) -> PlotSpec {
    PlotSpec {
        title: format!("Distribution of {column}"),
        x_axis: AxisSpec {
            label: column.to_string(),
            scale: AxisScale::Linear,
        },
        y_axis: AxisSpec {
            label: "count".to_string(),
            scale: AxisScale::Linear,
        },
        data: PlotData::Table(data.clone()),
        marks: vec![Mark::Bars {
            x: column.to_string(),
            y: "count".to_string(),
        }],
    }
}
/// Convenience function: create a comparison plot.
pub fn plot_comparison(data: &ResearchDataFrame, x_column: &str, y_column: &str) -> PlotSpec {
    PlotSpec {
        title: format!("{y_column} by {x_column}"),
        x_axis: AxisSpec {
            label: x_column.to_string(),
            scale: AxisScale::Linear,
        },
        y_axis: AxisSpec {
            label: y_column.to_string(),
            scale: AxisScale::Linear,
        },
        data: PlotData::Table(data.clone()),
        marks: vec![Mark::Points {
            x: x_column.to_string(),
            y: y_column.to_string(),
        }],
    }
}
/// Convenience function: create a time-series plot.
pub fn plot_timeseries(
    data: &ResearchDataFrame,
    time_column: &str,
    value_column: &str,
) -> PlotSpec {
    PlotSpec {
        title: format!("{value_column} over {time_column}"),
        x_axis: AxisSpec {
            label: time_column.to_string(),
            scale: AxisScale::Linear,
        },
        y_axis: AxisSpec {
            label: value_column.to_string(),
            scale: AxisScale::Linear,
        },
        data: PlotData::Table(data.clone()),
        marks: vec![Mark::Lines {
            x: time_column.to_string(),
            y: value_column.to_string(),
        }],
    }
}
/// Convenience function: create a Pareto/tradeoff plot.
pub fn plot_tradeoff(data: &ResearchDataFrame, x_column: &str, y_column: &str) -> PlotSpec {
    PlotSpec {
        title: format!("Tradeoff: {x_column} vs {y_column}"),
        x_axis: AxisSpec {
            label: x_column.to_string(),
            scale: AxisScale::Linear,
        },
        y_axis: AxisSpec {
            label: y_column.to_string(),
            scale: AxisScale::Linear,
        },
        data: PlotData::Table(data.clone()),
        marks: vec![Mark::Points {
            x: x_column.to_string(),
            y: y_column.to_string(),
        }],
    }
}
/// Convenience function: create an interaction plot.
pub fn plot_interaction(
    data: &ResearchDataFrame,
    factor_a: &str,
    factor_b: &str,
    response: &str,
) -> PlotSpec {
    PlotSpec {
        title: format!("Interaction: {factor_a} × {factor_b}"),
        x_axis: AxisSpec {
            label: factor_a.to_string(),
            scale: AxisScale::Linear,
        },
        y_axis: AxisSpec {
            label: response.to_string(),
            scale: AxisScale::Linear,
        },
        data: PlotData::Table(data.clone()),
        marks: vec![Mark::Lines {
            x: factor_a.to_string(),
            y: response.to_string(),
        }],
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataframe::{Column, DataType, DataValue, ResearchDataFrame};
    fn sample_df() -> ResearchDataFrame {
        let mut df = ResearchDataFrame::new(vec![
            Column {
                name: "x".to_string(),
                dtype: DataType::Numeric,
            },
            Column {
                name: "y".to_string(),
                dtype: DataType::Numeric,
            },
        ]);
        df.add_row(vec![DataValue::Float(1.0), DataValue::Float(2.0)]);
        df.add_row(vec![DataValue::Float(3.0), DataValue::Float(4.0)]);
        df
    }
    #[test]
    fn text_renderer_works() {
        let plot = plot_distribution(&sample_df(), "x");
        let renderer = TextRenderer;
        let output = renderer.render(&plot);
        assert!(matches!(output, PlotOutput::Text(_)));
    }
    #[test]
    fn json_renderer_works() {
        let plot = plot_comparison(&sample_df(), "x", "y");
        let renderer = JsonRenderer;
        let output = renderer.render(&plot);
        assert!(matches!(output, PlotOutput::Json(_)));
    }
    #[test]
    fn plot_distribution_creates_valid_spec() {
        let plot = plot_distribution(&sample_df(), "x");
        assert!(plot.title.contains("x"));
        assert_eq!(plot.marks.len(), 1);
    }
    #[test]
    fn plot_comparison_creates_valid_spec() {
        let plot = plot_comparison(&sample_df(), "x", "y");
        assert!(plot.title.contains("y"));
        assert!(plot.title.contains("x"));
    }
    #[test]
    fn plot_tradeoff_creates_valid_spec() {
        let plot = plot_tradeoff(&sample_df(), "queue_time", "quality");
        assert!(plot.title.contains("Tradeoff"));
    }
}
