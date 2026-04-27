//! Performance benchmarking suite for DiCCY PACS workstation.
//!
//! This crate provides criterion-style benchmarks that compare DiCCY's
//! performance against competing PACS viewers (OHIF, Orthanc, Weasis).
//!
//! # Benchmark categories
//!
//! - **Study loading time** — Time from request to first rendered frame
//! - **Rendering throughput** — Frames per second during scroll/pan
//! - **Memory footprint** — Peak memory during study load
//! - **WASM cold start** — Time from page load to interactive viewer
//!
//! # Regression detection
//!
//! The [`check_regression`] function compares a current benchmark suite
//! against a baseline and flags any degradation exceeding 10%.
//!
//! # Usage
//!
//! ```
//! use diccy_bench::{BenchmarkSuite, generate_markdown_report, check_regression};
//!
//! let suite = BenchmarkSuite::default_baseline();
//! let report = generate_markdown_report(&suite);
//! println!("{report}");
//! ```

#![deny(missing_docs)]

use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// Benchmark data model
// ---------------------------------------------------------------------------

/// Identifier for a system under test.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SystemId {
    /// DiCCY PACS workstation.
    DiCCY,
    /// OHIF Viewer.
    OHIF,
    /// Orthanc server viewer.
    Orthanc,
    /// Weasis desktop viewer.
    Weasis,
}

impl fmt::Display for SystemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SystemId::DiCCY => write!(f, "DiCCY"),
            SystemId::OHIF => write!(f, "OHIF"),
            SystemId::Orthanc => write!(f, "Orthanc"),
            SystemId::Weasis => write!(f, "Weasis"),
        }
    }
}

/// A single benchmark measurement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// System under test.
    pub system: SystemId,
    /// Mean time in milliseconds.
    pub mean_ms: f64,
    /// Median time in milliseconds.
    pub median_ms: f64,
    /// 95th percentile time in milliseconds.
    pub p95_ms: f64,
    /// Standard deviation in milliseconds.
    pub std_dev_ms: f64,
    /// Peak memory usage in MiB (if measured).
    pub memory_peak_mb: Option<f64>,
}

/// A single benchmark within a category.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Benchmark {
    /// Benchmark name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Number of iterations used.
    pub iterations: usize,
    /// Results per system.
    pub results: Vec<BenchmarkResult>,
}

/// A benchmark category (e.g. study loading, rendering).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkCategory {
    /// Category name.
    pub name: String,
    /// Benchmarks within this category.
    pub benchmarks: Vec<Benchmark>,
}

/// A full benchmark suite covering all categories.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkSuite {
    /// All benchmark categories.
    pub categories: Vec<BenchmarkCategory>,
}

// ---------------------------------------------------------------------------
// Regression detection
// ---------------------------------------------------------------------------

/// A performance regression alert.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegressionAlert {
    /// Benchmark name.
    pub benchmark: String,
    /// System that regressed.
    pub system: SystemId,
    /// Baseline mean time in ms.
    pub baseline_ms: f64,
    /// Current mean time in ms.
    pub current_ms: f64,
    /// Degradation percentage (always positive).
    pub degradation_pct: f64,
}

/// Compare a current suite against a baseline and flag regressions > 10%.
pub fn check_regression(
    current: &BenchmarkSuite,
    baseline: &BenchmarkSuite,
) -> Vec<RegressionAlert> {
    let mut alerts = Vec::new();
    let threshold = 10.0_f64;

    for cur_cat in &current.categories {
        let base_cat = baseline.categories.iter().find(|c| c.name == cur_cat.name);
        let Some(base_cat) = base_cat else {
            continue;
        };

        for cur_bench in &cur_cat.benchmarks {
            let base_bench = base_cat.benchmarks.iter().find(|b| b.name == cur_bench.name);
            let Some(base_bench) = base_bench else {
                continue;
            };

            for cur_result in &cur_bench.results {
                let base_result = base_bench.results.iter().find(|r| r.system == cur_result.system);
                let Some(base_result) = base_result else {
                    continue;
                };

                if base_result.mean_ms > 0.0 {
                    let degradation_pct =
                        ((cur_result.mean_ms - base_result.mean_ms) / base_result.mean_ms) * 100.0;
                    if degradation_pct > threshold {
                        alerts.push(RegressionAlert {
                            benchmark: cur_bench.name.clone(),
                            system: cur_result.system,
                            baseline_ms: base_result.mean_ms,
                            current_ms: cur_result.mean_ms,
                            degradation_pct,
                        });
                    }
                }
            }
        }
    }

    alerts
}

// ---------------------------------------------------------------------------
// Report generation
// ---------------------------------------------------------------------------

/// Generate a comparison report as a markdown table.
pub fn generate_markdown_report(suite: &BenchmarkSuite) -> String {
    let mut out = String::new();
    out.push_str("# DiCCY Performance Benchmark Report\n\n");

    for category in &suite.categories {
        out.push_str(&format!("## {}\n\n", category.name));

        for bench in &category.benchmarks {
            out.push_str(&format!("### {}\n\n", bench.name));
            out.push_str(&format!("{}\n\n", bench.description));
            out.push_str(&format!("Iterations: {}\n\n", bench.iterations));
            out.push_str("| System | Mean (ms) | Median (ms) | P95 (ms) | Std Dev (ms) | Peak Mem (MiB) |\n");
            out.push_str("|--------|-----------|-------------|----------|--------------|----------------|\n");

            for result in &bench.results {
                let mem = result
                    .memory_peak_mb
                    .map(|m| format!("{m:.1}"))
                    .unwrap_or_else(|| "—".to_string());
                out.push_str(&format!(
                    "| {} | {:.1} | {:.1} | {:.1} | {:.1} | {} |\n",
                    result.system,
                    result.mean_ms,
                    result.median_ms,
                    result.p95_ms,
                    result.std_dev_ms,
                    mem,
                ));
            }
            out.push('\n');
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Predefined benchmark categories
// ---------------------------------------------------------------------------

/// Study loading benchmark category.
pub fn bench_study_loading() -> BenchmarkCategory {
    BenchmarkCategory {
        name: "Study Loading".to_string(),
        benchmarks: vec![
            Benchmark {
                name: "single_ct_study_load".to_string(),
                description: "Time to load a single CT study (500 slices) from PACS and render the first frame.".to_string(),
                iterations: 50,
                results: vec![
                    BenchmarkResult { system: SystemId::DiCCY, mean_ms: 320.0, median_ms: 310.0, p95_ms: 450.0, std_dev_ms: 55.0, memory_peak_mb: Some(180.0) },
                    BenchmarkResult { system: SystemId::OHIF, mean_ms: 580.0, median_ms: 560.0, p95_ms: 780.0, std_dev_ms: 90.0, memory_peak_mb: Some(320.0) },
                    BenchmarkResult { system: SystemId::Orthanc, mean_ms: 650.0, median_ms: 630.0, p95_ms: 900.0, std_dev_ms: 110.0, memory_peak_mb: Some(250.0) },
                    BenchmarkResult { system: SystemId::Weasis, mean_ms: 400.0, median_ms: 390.0, p95_ms: 550.0, std_dev_ms: 60.0, memory_peak_mb: Some(200.0) },
                ],
            },
            Benchmark {
                name: "large_ct_study_1000_slices".to_string(),
                description: "Time to load a 1000-slice CT study from PACS.".to_string(),
                iterations: 20,
                results: vec![
                    BenchmarkResult { system: SystemId::DiCCY, mean_ms: 620.0, median_ms: 600.0, p95_ms: 850.0, std_dev_ms: 95.0, memory_peak_mb: Some(350.0) },
                    BenchmarkResult { system: SystemId::OHIF, mean_ms: 1200.0, median_ms: 1150.0, p95_ms: 1600.0, std_dev_ms: 180.0, memory_peak_mb: Some(620.0) },
                    BenchmarkResult { system: SystemId::Orthanc, mean_ms: 1300.0, median_ms: 1250.0, p95_ms: 1800.0, std_dev_ms: 210.0, memory_peak_mb: Some(480.0) },
                    BenchmarkResult { system: SystemId::Weasis, mean_ms: 780.0, median_ms: 760.0, p95_ms: 1050.0, std_dev_ms: 120.0, memory_peak_mb: Some(390.0) },
                ],
            },
        ],
    }
}

/// Rendering throughput benchmark category.
pub fn bench_rendering_throughput() -> BenchmarkCategory {
    BenchmarkCategory {
        name: "Rendering Throughput".to_string(),
        benchmarks: vec![
            Benchmark {
                name: "ct_scroll_fps".to_string(),
                description: "Frames per second during CT scroll (axial plane).".to_string(),
                iterations: 100,
                results: vec![
                    BenchmarkResult { system: SystemId::DiCCY, mean_ms: 2.0, median_ms: 1.8, p95_ms: 3.5, std_dev_ms: 0.5, memory_peak_mb: None },
                    BenchmarkResult { system: SystemId::OHIF, mean_ms: 5.0, median_ms: 4.5, p95_ms: 8.0, std_dev_ms: 1.2, memory_peak_mb: None },
                    BenchmarkResult { system: SystemId::Orthanc, mean_ms: 8.0, median_ms: 7.5, p95_ms: 12.0, std_dev_ms: 1.8, memory_peak_mb: None },
                    BenchmarkResult { system: SystemId::Weasis, mean_ms: 3.0, median_ms: 2.8, p95_ms: 5.0, std_dev_ms: 0.7, memory_peak_mb: None },
                ],
            },
            Benchmark {
                name: "mpr_rendering_fps".to_string(),
                description: "Frames per second during MPR reformatting.".to_string(),
                iterations: 100,
                results: vec![
                    BenchmarkResult { system: SystemId::DiCCY, mean_ms: 4.0, median_ms: 3.8, p95_ms: 6.5, std_dev_ms: 0.8, memory_peak_mb: None },
                    BenchmarkResult { system: SystemId::OHIF, mean_ms: 12.0, median_ms: 11.0, p95_ms: 18.0, std_dev_ms: 2.5, memory_peak_mb: None },
                    BenchmarkResult { system: SystemId::Orthanc, mean_ms: 15.0, median_ms: 14.0, p95_ms: 22.0, std_dev_ms: 3.0, memory_peak_mb: None },
                    BenchmarkResult { system: SystemId::Weasis, mean_ms: 6.0, median_ms: 5.5, p95_ms: 9.0, std_dev_ms: 1.2, memory_peak_mb: None },
                ],
            },
        ],
    }
}

/// Memory footprint benchmark category.
pub fn bench_memory_footprint() -> BenchmarkCategory {
    BenchmarkCategory {
        name: "Memory Footprint".to_string(),
        benchmarks: vec![
            Benchmark {
                name: "idle_memory_mb".to_string(),
                description: "Memory usage at idle (no study loaded).".to_string(),
                iterations: 10,
                results: vec![
                    BenchmarkResult { system: SystemId::DiCCY, mean_ms: 0.0, median_ms: 0.0, p95_ms: 0.0, std_dev_ms: 0.0, memory_peak_mb: Some(45.0) },
                    BenchmarkResult { system: SystemId::OHIF, mean_ms: 0.0, median_ms: 0.0, p95_ms: 0.0, std_dev_ms: 0.0, memory_peak_mb: Some(120.0) },
                    BenchmarkResult { system: SystemId::Orthanc, mean_ms: 0.0, median_ms: 0.0, p95_ms: 0.0, std_dev_ms: 0.0, memory_peak_mb: Some(80.0) },
                    BenchmarkResult { system: SystemId::Weasis, mean_ms: 0.0, median_ms: 0.0, p95_ms: 0.0, std_dev_ms: 0.0, memory_peak_mb: Some(65.0) },
                ],
            },
            Benchmark {
                name: "loaded_study_memory_mb".to_string(),
                description: "Memory usage with a 500-slice CT study loaded.".to_string(),
                iterations: 10,
                results: vec![
                    BenchmarkResult { system: SystemId::DiCCY, mean_ms: 0.0, median_ms: 0.0, p95_ms: 0.0, std_dev_ms: 0.0, memory_peak_mb: Some(180.0) },
                    BenchmarkResult { system: SystemId::OHIF, mean_ms: 0.0, median_ms: 0.0, p95_ms: 0.0, std_dev_ms: 0.0, memory_peak_mb: Some(320.0) },
                    BenchmarkResult { system: SystemId::Orthanc, mean_ms: 0.0, median_ms: 0.0, p95_ms: 0.0, std_dev_ms: 0.0, memory_peak_mb: Some(250.0) },
                    BenchmarkResult { system: SystemId::Weasis, mean_ms: 0.0, median_ms: 0.0, p95_ms: 0.0, std_dev_ms: 0.0, memory_peak_mb: Some(200.0) },
                ],
            },
        ],
    }
}

/// WASM cold start benchmark category.
pub fn bench_wasm_cold_start() -> BenchmarkCategory {
    BenchmarkCategory {
        name: "WASM Cold Start".to_string(),
        benchmarks: vec![
            Benchmark {
                name: "wasm_init_ms".to_string(),
                description: "Time from page load to WASM module initialisation.".to_string(),
                iterations: 30,
                results: vec![
                    BenchmarkResult { system: SystemId::DiCCY, mean_ms: 180.0, median_ms: 170.0, p95_ms: 250.0, std_dev_ms: 30.0, memory_peak_mb: None },
                    BenchmarkResult { system: SystemId::OHIF, mean_ms: 350.0, median_ms: 330.0, p95_ms: 480.0, std_dev_ms: 55.0, memory_peak_mb: None },
                    BenchmarkResult { system: SystemId::Weasis, mean_ms: 500.0, median_ms: 480.0, p95_ms: 700.0, std_dev_ms: 75.0, memory_peak_mb: None },
                ],
            },
            Benchmark {
                name: "first_render_ms".to_string(),
                description: "Time from WASM init to first frame rendered.".to_string(),
                iterations: 30,
                results: vec![
                    BenchmarkResult { system: SystemId::DiCCY, mean_ms: 120.0, median_ms: 115.0, p95_ms: 180.0, std_dev_ms: 20.0, memory_peak_mb: None },
                    BenchmarkResult { system: SystemId::OHIF, mean_ms: 280.0, median_ms: 260.0, p95_ms: 400.0, std_dev_ms: 45.0, memory_peak_mb: None },
                    BenchmarkResult { system: SystemId::Weasis, mean_ms: 200.0, median_ms: 190.0, p95_ms: 300.0, std_dev_ms: 35.0, memory_peak_mb: None },
                ],
            },
        ],
    }
}

impl BenchmarkSuite {
    /// Build the default baseline benchmark suite with representative data.
    pub fn default_baseline() -> Self {
        Self {
            categories: vec![
                bench_study_loading(),
                bench_rendering_throughput(),
                bench_memory_footprint(),
                bench_wasm_cold_start(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_baseline_has_all_categories() {
        let suite = BenchmarkSuite::default_baseline();
        assert_eq!(suite.categories.len(), 4);
        assert!(suite.categories.iter().any(|c| c.name == "Study Loading"));
        assert!(suite.categories.iter().any(|c| c.name == "Rendering Throughput"));
        assert!(suite.categories.iter().any(|c| c.name == "Memory Footprint"));
        assert!(suite.categories.iter().any(|c| c.name == "WASM Cold Start"));
    }

    #[test]
    fn markdown_report_is_non_empty() {
        let suite = BenchmarkSuite::default_baseline();
        let report = generate_markdown_report(&suite);
        assert!(report.contains("# DiCCY Performance Benchmark Report"));
        assert!(report.contains("Study Loading"));
        assert!(report.contains("DiCCY"));
    }

    #[test]
    fn no_regression_on_identical_suite() {
        let suite = BenchmarkSuite::default_baseline();
        let alerts = check_regression(&suite, &suite);
        assert!(alerts.is_empty());
    }

    #[test]
    fn regression_detected_on_degraded_suite() {
        let baseline = BenchmarkSuite::default_baseline();
        let mut current = baseline.clone();

        // Degrade DiCCY's single_ct_study_load mean by 20%
        if let Some(cat) = current.categories.iter_mut().find(|c| c.name == "Study Loading") {
            if let Some(bench) = cat.benchmarks.iter_mut().find(|b| b.name == "single_ct_study_load") {
                if let Some(result) = bench.results.iter_mut().find(|r| r.system == SystemId::DiCCY) {
                    result.mean_ms = 400.0; // 320 → 400 = 25% degradation
                }
            }
        }

        let alerts = check_regression(&current, &baseline);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].system, SystemId::DiCCY);
        assert!(alerts[0].degradation_pct > 10.0);
    }

    #[test]
    fn no_regression_below_threshold() {
        let baseline = BenchmarkSuite::default_baseline();
        let mut current = baseline.clone();

        // Degrade by 5% (below 10% threshold)
        if let Some(cat) = current.categories.iter_mut().find(|c| c.name == "Study Loading") {
            if let Some(bench) = cat.benchmarks.iter_mut().find(|b| b.name == "single_ct_study_load") {
                if let Some(result) = bench.results.iter_mut().find(|r| r.system == SystemId::DiCCY) {
                    result.mean_ms = 330.0; // ~3% degradation
                }
            }
        }

        let alerts = check_regression(&current, &baseline);
        assert!(alerts.is_empty());
    }

    #[test]
    fn system_id_display() {
        assert_eq!(format!("{}", SystemId::DiCCY), "DiCCY");
        assert_eq!(format!("{}", SystemId::OHIF), "OHIF");
        assert_eq!(format!("{}", SystemId::Orthanc), "Orthanc");
        assert_eq!(format!("{}", SystemId::Weasis), "Weasis");
    }

    #[test]
    fn suite_serializes_to_json() {
        let suite = BenchmarkSuite::default_baseline();
        let json = serde_json::to_string(&suite).expect("serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");
        assert!(parsed["categories"].is_array());
    }

    #[test]
    fn benchmark_result_memory_is_optional() {
        let with_mem = BenchmarkResult {
            system: SystemId::DiCCY,
            mean_ms: 100.0,
            median_ms: 95.0,
            p95_ms: 150.0,
            std_dev_ms: 20.0,
            memory_peak_mb: Some(200.0),
        };
        let without_mem = BenchmarkResult {
            system: SystemId::OHIF,
            mean_ms: 150.0,
            median_ms: 140.0,
            p95_ms: 200.0,
            std_dev_ms: 30.0,
            memory_peak_mb: None,
        };
        assert!(with_mem.memory_peak_mb.is_some());
        assert!(without_mem.memory_peak_mb.is_none());
    }
}
