use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

const EVENT_NAME: &str = "performance_operation_v1";
const WARM_COMPLETED_MINIMUM: usize = 30;
const COLD_MINIMUM: usize = 5;
const SUPPORTED_ENVIRONMENTS: [&str; 2] = ["home-win11", "work-win11"];
const TERMINAL_OUTCOMES: [&str; 4] = ["Completed", "NoChange", "Unsupported", "RolledBackOrFailed"];

pub fn run(args: &[String]) {
    let Some(input) = arg_value(args, "--input") else {
        eprintln!("usage: stepler-cli performance-snapshot --input <performance.jsonl> --output <snapshot.json>");
        std::process::exit(2);
    };
    let Some(output) = arg_value(args, "--output") else {
        eprintln!("performance-snapshot error: missing --output");
        std::process::exit(2);
    };

    let input_path = PathBuf::from(input);
    let output_path = PathBuf::from(output);
    if let Err(error) = ensure_distinct_paths(&input_path, &output_path) {
        eprintln!("performance-snapshot error: {error}");
        std::process::exit(2);
    }

    let source = match std::fs::read_to_string(&input_path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("performance-snapshot input error: {error}");
            std::process::exit(1);
        }
    };
    let snapshot = match build_snapshot(&source) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("performance-snapshot error: {error}");
            std::process::exit(1);
        }
    };
    let serialized = match serde_json::to_string_pretty(&snapshot) {
        Ok(serialized) => format!("{serialized}\n"),
        Err(error) => {
            eprintln!("performance-snapshot serialization error: {error}");
            std::process::exit(1);
        }
    };
    if let Err(error) = std::fs::write(&output_path, serialized) {
        eprintln!("performance-snapshot output error: {error}");
        std::process::exit(1);
    }

    let group_count = snapshot
        .get("groups")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    println!(
        "performance snapshot written: {} ({} groups)",
        output_path.display(),
        group_count
    );
}

fn arg_value<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].as_str())
}

fn ensure_distinct_paths(input: &Path, output: &Path) -> Result<(), SnapshotError> {
    if input == output {
        return Err(SnapshotError::OutputOverwritesInput);
    }

    let input = std::fs::canonicalize(input).map_err(|error| SnapshotError::InputPath {
        path: input.display().to_string(),
        error: error.to_string(),
    })?;
    let output = if output.exists() {
        std::fs::canonicalize(output).map_err(|error| SnapshotError::OutputPath {
            path: output.display().to_string(),
            error: error.to_string(),
        })?
    } else {
        let parent = output
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let parent = std::fs::canonicalize(parent).map_err(|error| SnapshotError::OutputPath {
            path: parent.display().to_string(),
            error: error.to_string(),
        })?;
        parent.join(output.file_name().unwrap_or_default())
    };
    if input == output {
        return Err(SnapshotError::OutputOverwritesInput);
    }
    Ok(())
}

#[derive(Debug)]
enum SnapshotError {
    EmptyDataset,
    InputPath { path: String, error: String },
    InvalidJson { line: usize, error: String },
    InvalidEvent { line: usize, error: String },
    MultipleBuilds(Vec<String>),
    OutputOverwritesInput,
    OutputPath { path: String, error: String },
}

impl Display for SnapshotError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyDataset => write!(
                formatter,
                "no labeled {EVENT_NAME} events found; set STEPLER_PERF_ENV to home-win11 or work-win11"
            ),
            Self::InputPath { path, error } => {
                write!(formatter, "cannot resolve input path {path}: {error}")
            }
            Self::InvalidJson { line, error } => {
                write!(formatter, "invalid JSON at line {line}: {error}")
            }
            Self::InvalidEvent { line, error } => {
                write!(formatter, "invalid {EVENT_NAME} event at line {line}: {error}")
            }
            Self::MultipleBuilds(builds) => write!(
                formatter,
                "input contains multiple build_version values ({}) - create one snapshot per build",
                builds.join(", ")
            ),
            Self::OutputOverwritesInput => {
                write!(formatter, "--output must be different from --input")
            }
            Self::OutputPath { path, error } => {
                write!(formatter, "cannot resolve output path {path}: {error}")
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct BaselineSeriesKey {
    build_version: String,
    environment_label: String,
    surface_kind: String,
    surface_confidence: u64,
    context_method: String,
    replacement_method: String,
    profile: String,
    algorithm_branch: String,
    trigger: String,
    selection_state: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct BaselineGroupKey {
    series: BaselineSeriesKey,
    cold_warm: String,
}

#[derive(Debug)]
struct Record {
    key: BaselineGroupKey,
    outcome: String,
    duration_ms: u64,
    retry_count: u64,
    timings: Vec<Timing>,
}

#[derive(Debug)]
struct Timing {
    phase: String,
    elapsed_ms: u64,
}

#[derive(Default)]
struct SampleCounts {
    warm_n: usize,
    warm_completed_n: usize,
    cold_n: usize,
    destructive_outcome_n: usize,
}

impl SampleCounts {
    fn add(&mut self, record: &Record) {
        if record.outcome == "RolledBackOrFailed" {
            self.destructive_outcome_n += 1;
        }
        if record.key.cold_warm == "warm" {
            self.warm_n += 1;
            if record.outcome == "Completed" {
                self.warm_completed_n += 1;
            }
        } else {
            self.cold_n += 1;
        }
    }
}

#[derive(Default)]
struct Aggregate {
    durations_ms: Vec<u64>,
    failed: usize,
    retried: usize,
    outcomes: BTreeMap<String, usize>,
    phases_ms: BTreeMap<String, u64>,
}

impl Aggregate {
    fn add(&mut self, record: Record) {
        self.durations_ms.push(record.duration_ms);
        if record.outcome == "RolledBackOrFailed" {
            self.failed += 1;
        }
        if record.retry_count > 0 {
            self.retried += 1;
        }
        *self.outcomes.entry(record.outcome).or_default() += 1;
        for timing in record.timings {
            *self.phases_ms.entry(timing.phase).or_default() += timing.elapsed_ms;
        }
    }

    fn to_json(&self) -> Value {
        let n = self.durations_ms.len();
        let total_duration_ms = self.durations_ms.iter().sum::<u64>();
        let phases = self
            .phases_ms
            .iter()
            .map(|(phase, total_ms)| {
                json!({
                    "phase": phase,
                    "total_ms": total_ms,
                    "share_of_operation_duration": if total_duration_ms == 0 {
                        0.0
                    } else {
                        *total_ms as f64 / total_duration_ms as f64
                    }
                })
            })
            .collect::<Vec<_>>();
        let bottleneck_phase = self
            .phases_ms
            .iter()
            .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
            .map(|(phase, _)| phase.clone());

        json!({
            "n": n,
            "p50_ms": percentile(&self.durations_ms, 50),
            "p90_ms": percentile(&self.durations_ms, 90),
            "p95_ms": percentile(&self.durations_ms, 95),
            "max_ms": self.durations_ms.iter().copied().max().unwrap_or_default(),
            "failure_rate": rate(self.failed, n),
            "retry_rate": rate(self.retried, n),
            "outcome_counts": self.outcomes,
            "phase_contribution": phases,
            "bottleneck_phase": bottleneck_phase,
        })
    }
}

fn build_snapshot(source: &str) -> Result<Value, SnapshotError> {
    let mut records = Vec::new();
    let mut ignored_non_performance = 0usize;
    let mut ignored_unlabeled = 0usize;
    let mut builds = BTreeSet::new();

    for (line_index, line) in source.lines().enumerate() {
        let line_number = line_index + 1;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value =
            serde_json::from_str(line).map_err(|error| SnapshotError::InvalidJson {
                line: line_number,
                error: error.to_string(),
            })?;
        if value.get("event").and_then(Value::as_str) != Some(EVENT_NAME) {
            ignored_non_performance += 1;
            continue;
        }
        if value.get("environment_label").and_then(Value::as_str) == Some("unlabeled") {
            ignored_unlabeled += 1;
            continue;
        }
        let record = parse_record(&value, line_number)?;
        builds.insert(record.key.series.build_version.clone());
        records.push(record);
    }

    if records.is_empty() {
        return Err(SnapshotError::EmptyDataset);
    }
    if builds.len() > 1 {
        return Err(SnapshotError::MultipleBuilds(builds.into_iter().collect()));
    }

    let build_version = records[0].key.series.build_version.clone();
    let environment_labels = records
        .iter()
        .map(|record| record.key.series.environment_label.clone())
        .collect::<BTreeSet<_>>();
    let mut groups = BTreeMap::<BaselineGroupKey, Aggregate>::new();
    let mut samples = BTreeMap::<BaselineSeriesKey, SampleCounts>::new();
    for record in records {
        samples
            .entry(record.key.series.clone())
            .or_default()
            .add(&record);
        groups.entry(record.key.clone()).or_default().add(record);
    }

    let groups = groups
        .iter()
        .map(|(key, aggregate)| {
            let mut group = baseline_series_key_json(&key.series);
            group["cold_warm"] = Value::String(key.cold_warm.clone());
            if let Value::Object(fields) = &mut group {
                if let Value::Object(metrics) = aggregate.to_json() {
                    fields.extend(metrics);
                }
            }
            group
        })
        .collect::<Vec<_>>();
    let sample_assessments = samples
        .iter()
        .map(|(key, counts)| {
            let mut missing = Vec::new();
            if counts.warm_completed_n < WARM_COMPLETED_MINIMUM {
                missing.push("warm_completed>=30");
            }
            if counts.cold_n < COLD_MINIMUM {
                missing.push("cold>=5");
            }
            let status = if !missing.is_empty() {
                "insufficient_sample"
            } else if counts.destructive_outcome_n > 0 {
                "blocked_by_destructive_outcomes"
            } else {
                "sufficient"
            };
            let mut assessment = baseline_series_key_json(key);
            if let Value::Object(fields) = &mut assessment {
                fields.insert("warm_n".to_owned(), json!(counts.warm_n));
                fields.insert(
                    "warm_completed_n".to_owned(),
                    json!(counts.warm_completed_n),
                );
                fields.insert("cold_n".to_owned(), json!(counts.cold_n));
                fields.insert(
                    "destructive_outcome_n".to_owned(),
                    json!(counts.destructive_outcome_n),
                );
                fields.insert("status".to_owned(), Value::String(status.to_owned()));
                fields.insert("missing".to_owned(), json!(missing));
            }
            assessment
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "schema": "performance_snapshot_v1",
        "build_version": build_version,
        "environment_labels": environment_labels,
        "source_event": EVENT_NAME,
        "sample_rule": {
            "warm_completed_minimum": WARM_COMPLETED_MINIMUM,
            "cold_minimum": COLD_MINIMUM,
            "scope": "build/environment/surface/confidence/method/profile/branch/trigger/selection",
            "n_includes_all_terminal_outcomes": true,
            "sufficient_requires_completed_warm": true,
            "sufficient_requires_no_destructive_outcomes": true
        },
        "ignored_lines": {
            "non_performance_events": ignored_non_performance,
            "unlabeled_environment": ignored_unlabeled
        },
        "sample_assessments": sample_assessments,
        "groups": groups
    }))
}

fn parse_record(value: &Value, line: usize) -> Result<Record, SnapshotError> {
    let field = |name| required_string(value, name, line);
    let cold_warm = field("cold_warm")?;
    if cold_warm != "cold" && cold_warm != "warm" {
        return Err(invalid_event(
            line,
            format!("cold_warm must be cold or warm, got {cold_warm:?}"),
        ));
    }
    let build_version = field("build_version")?;
    if build_version == "unknown" {
        return Err(invalid_event(
            line,
            "build_version is unknown; use a release BUILD_INFO or STEPLER_BUILD_VERSION",
        ));
    }
    let environment_label = field("environment_label")?;
    if !SUPPORTED_ENVIRONMENTS.contains(&environment_label.as_str()) {
        return Err(invalid_event(
            line,
            format!(
                "environment_label must be home-win11 or work-win11, got {environment_label:?}"
            ),
        ));
    }
    let surface_confidence = required_u64(value, "surface_confidence", line)?;
    if surface_confidence > 100 {
        return Err(invalid_event(
            line,
            format!("surface_confidence must be between 0 and 100, got {surface_confidence}"),
        ));
    }
    let outcome = field("outcome")?;
    if !TERMINAL_OUTCOMES.contains(&outcome.as_str()) {
        return Err(invalid_event(
            line,
            format!("unsupported terminal outcome {outcome:?}"),
        ));
    }
    let series = BaselineSeriesKey {
        build_version,
        environment_label,
        surface_kind: field("surface_kind")?,
        surface_confidence,
        context_method: field("context_method")?,
        replacement_method: field("replacement_method")?,
        profile: field("profile")?,
        algorithm_branch: field("algorithm_branch")?,
        trigger: field("trigger")?,
        selection_state: field("selection_state")?,
    };
    let timings = value
        .get("timings_ms")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_event(line, "timings_ms must be an array"))?
        .iter()
        .map(|timing| {
            if timing.get("state").is_some() {
                return Err(invalid_event(
                    line,
                    "legacy timings_ms[].state is unsupported; collect a fresh log with timings_ms[].phase",
                ));
            }
            let phase = match timing.get("phase").and_then(Value::as_str) {
                Some(phase) => phase,
                None => {
                    return Err(invalid_event(line, "timings_ms.phase must be a string"));
                }
            };
            if phase.trim().is_empty() {
                return Err(invalid_event(line, "timings_ms.phase must not be empty"));
            }
            let elapsed_ms = timing
                .get("elapsed_ms")
                .and_then(Value::as_u64)
                .ok_or_else(|| invalid_event(line, "timings_ms.elapsed_ms must be an integer"))?;
            Ok(Timing {
                phase: phase.to_owned(),
                elapsed_ms,
            })
        })
        .collect::<Result<Vec<_>, SnapshotError>>()?;

    Ok(Record {
        key: BaselineGroupKey { series, cold_warm },
        outcome,
        duration_ms: required_u64(value, "duration_ms", line)?,
        retry_count: required_u64(value, "retry_count", line)?,
        timings,
    })
}

fn required_string(value: &Value, field: &str, line: usize) -> Result<String, SnapshotError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| invalid_event(line, format!("{field} must be a non-empty string")))
}

fn required_u64(value: &Value, field: &str, line: usize) -> Result<u64, SnapshotError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_event(line, format!("{field} must be an integer")))
}

fn invalid_event(line: usize, error: impl Into<String>) -> SnapshotError {
    SnapshotError::InvalidEvent {
        line,
        error: error.into(),
    }
}

fn baseline_series_key_json(key: &BaselineSeriesKey) -> Value {
    json!({
        "build_version": key.build_version,
        "environment_label": key.environment_label,
        "surface_kind": key.surface_kind,
        "surface_confidence": key.surface_confidence,
        "context_method": key.context_method,
        "replacement_method": key.replacement_method,
        "profile": key.profile,
        "algorithm_branch": key.algorithm_branch,
        "trigger": key.trigger,
        "selection_state": key.selection_state,
    })
}

fn percentile(values: &[u64], percentile: u64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let rank = (percentile * sorted.len() as u64).div_ceil(100) as usize;
    sorted[rank.saturating_sub(1)]
}

fn rate(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(
        build_version: &str,
        environment_label: &str,
        cold_warm: &str,
        duration_ms: u64,
        outcome: &str,
        retry_count: u64,
        timings: &[(&str, u64)],
    ) -> String {
        json!({
            "event": EVENT_NAME,
            "build_version": build_version,
            "environment_label": environment_label,
            "surface_kind": "FastBrowserEditor",
            "surface_confidence": 100,
            "context_method": "web_keyboard_selection",
            "replacement_method": "web_keyboard_selection",
            "profile": "Fast",
            "algorithm_branch": "web-keyboard-line-selection",
            "trigger": "Pause",
            "selection_state": "none",
            "cold_warm": cold_warm,
            "outcome": outcome,
            "retry_count": retry_count,
            "duration_ms": duration_ms,
            "timings_ms": timings.iter().map(|(phase, elapsed_ms)| json!({
                "phase": phase,
                "elapsed_ms": elapsed_ms,
            })).collect::<Vec<_>>(),
            "secret_user_text": "must not be copied"
        })
        .to_string()
    }

    #[test]
    fn snapshot_separates_cold_warm_and_reports_metrics() {
        let source = [
            event(
                "1.0.test",
                "home-win11",
                "warm",
                100,
                "Completed",
                0,
                &[("capture", 10), ("verify", 90)],
            ),
            event(
                "1.0.test",
                "home-win11",
                "warm",
                200,
                "Completed",
                1,
                &[("capture", 20), ("verify", 180)],
            ),
            event(
                "1.0.test",
                "home-win11",
                "warm",
                300,
                "RolledBackOrFailed",
                0,
                &[("capture", 30), ("verify", 270)],
            ),
            event(
                "1.0.test",
                "home-win11",
                "cold",
                400,
                "Completed",
                0,
                &[("capture", 40), ("verify", 360)],
            ),
            event(
                "1.0.test",
                "work-win11",
                "warm",
                50,
                "Completed",
                0,
                &[("capture", 5), ("verify", 45)],
            ),
            "{\"event\":\"clipboard_guard\"}".to_owned(),
            event("1.0.test", "unlabeled", "warm", 999, "Completed", 0, &[]),
        ]
        .join("\n");

        let snapshot = build_snapshot(&source).expect("snapshot should build");
        assert_eq!(snapshot["build_version"], "1.0.test");
        assert_eq!(snapshot["groups"].as_array().unwrap().len(), 3);
        assert_eq!(snapshot["environment_labels"].as_array().unwrap().len(), 2);
        assert_eq!(snapshot["ignored_lines"]["non_performance_events"], 1);
        assert_eq!(snapshot["ignored_lines"]["unlabeled_environment"], 1);

        let warm_group = snapshot["groups"]
            .as_array()
            .unwrap()
            .iter()
            .find(|group| {
                group["environment_label"] == "home-win11" && group["cold_warm"] == "warm"
            })
            .unwrap();
        assert_eq!(warm_group["n"], 3);
        assert_eq!(warm_group["p50_ms"], 200);
        assert_eq!(warm_group["p90_ms"], 300);
        assert_eq!(warm_group["p95_ms"], 300);
        assert_eq!(warm_group["max_ms"], 300);
        assert_eq!(warm_group["failure_rate"], 1.0 / 3.0);
        assert_eq!(warm_group["retry_rate"], 1.0 / 3.0);
        assert_eq!(warm_group["bottleneck_phase"], "verify");
        assert_eq!(warm_group["phase_contribution"][1]["phase"], "verify");
        assert!(!serde_json::to_string(&snapshot)
            .unwrap()
            .contains("must not be copied"));

        let assessment = snapshot["sample_assessments"]
            .as_array()
            .unwrap()
            .iter()
            .find(|assessment| assessment["environment_label"] == "home-win11")
            .unwrap();
        assert_eq!(assessment["warm_n"], 3);
        assert_eq!(assessment["cold_n"], 1);
        assert_eq!(assessment["status"], "insufficient_sample");
        assert_eq!(assessment["missing"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn snapshot_rejects_multiple_builds() {
        let source = [
            event("1.0.a", "home-win11", "warm", 10, "Completed", 0, &[]),
            event("1.0.b", "home-win11", "warm", 10, "Completed", 0, &[]),
        ]
        .join("\n");

        let error = build_snapshot(&source).expect_err("multiple builds must be rejected");
        assert!(error.to_string().contains("multiple build_version"));
    }

    #[test]
    fn snapshot_rejects_legacy_state_timings_with_actionable_error() {
        let source = event(
            "1.0.test",
            "home-win11",
            "warm",
            10,
            "Completed",
            0,
            &[("capture", 10)],
        )
        .replace("\"phase\":\"capture\"", "\"state\":\"ContextCaptured\"");

        let error = build_snapshot(&source).expect_err("legacy timings must be rejected");
        assert!(error.to_string().contains("legacy timings_ms[].state"));
    }

    #[test]
    fn snapshot_rejects_unknown_outcome() {
        let source = event("1.0.test", "home-win11", "warm", 10, "TypoOutcome", 0, &[]);

        let error = build_snapshot(&source).expect_err("unknown outcomes must be rejected");
        assert!(error.to_string().contains("unsupported terminal outcome"));
    }

    #[test]
    fn sample_sufficiency_requires_completed_warm_events() {
        let insufficient = (0..30)
            .map(|_| {
                event(
                    "1.0.test",
                    "home-win11",
                    "warm",
                    10,
                    "RolledBackOrFailed",
                    0,
                    &[],
                )
            })
            .chain((0..5).map(|_| event("1.0.test", "home-win11", "cold", 10, "Completed", 0, &[])))
            .collect::<Vec<_>>()
            .join("\n");
        let insufficient_snapshot = build_snapshot(&insufficient).unwrap();
        let insufficient_assessment = insufficient_snapshot["sample_assessments"][0].clone();
        assert_eq!(insufficient_assessment["warm_n"], 30);
        assert_eq!(insufficient_assessment["warm_completed_n"], 0);
        assert_eq!(insufficient_assessment["cold_n"], 5);
        assert_eq!(insufficient_assessment["status"], "insufficient_sample");

        let sufficient = (0..30)
            .map(|_| event("1.0.test", "home-win11", "warm", 10, "Completed", 0, &[]))
            .chain((0..5).map(|_| event("1.0.test", "home-win11", "cold", 10, "Completed", 0, &[])))
            .collect::<Vec<_>>()
            .join("\n");
        let sufficient_snapshot = build_snapshot(&sufficient).unwrap();
        let sufficient_assessment = sufficient_snapshot["sample_assessments"][0].clone();
        assert_eq!(sufficient_assessment["warm_completed_n"], 30);
        assert_eq!(sufficient_assessment["status"], "sufficient");
    }

    #[test]
    fn destructive_outcome_blocks_an_otherwise_sufficient_assessment() {
        let source = (0..30)
            .map(|_| event("1.0.test", "home-win11", "warm", 10, "Completed", 0, &[]))
            .chain((0..5).map(|_| event("1.0.test", "home-win11", "cold", 10, "Completed", 0, &[])))
            .chain(std::iter::once(event(
                "1.0.test",
                "home-win11",
                "warm",
                10,
                "RolledBackOrFailed",
                0,
                &[],
            )))
            .collect::<Vec<_>>()
            .join("\n");

        let snapshot = build_snapshot(&source).unwrap();
        let assessment = snapshot["sample_assessments"][0].clone();
        assert_eq!(assessment["warm_completed_n"], 30);
        assert_eq!(assessment["destructive_outcome_n"], 1);
        assert_eq!(assessment["status"], "blocked_by_destructive_outcomes");
    }

    #[test]
    fn snapshot_keeps_surface_confidence_in_separate_groups() {
        let source = [
            event("1.0.test", "home-win11", "warm", 10, "Completed", 0, &[]),
            event("1.0.test", "home-win11", "warm", 10, "Completed", 0, &[])
                .replace("\"surface_confidence\":100", "\"surface_confidence\":80"),
        ]
        .join("\n");

        let snapshot = build_snapshot(&source).unwrap();
        assert_eq!(snapshot["groups"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn snapshot_rejects_invalid_environment_and_empty_phase() {
        let invalid_environment = event("1.0.test", "lab-win11", "warm", 10, "Completed", 0, &[]);
        let error =
            build_snapshot(&invalid_environment).expect_err("invalid environment must fail");
        assert!(error
            .to_string()
            .contains("environment_label must be home-win11 or work-win11"));

        let empty_phase = event(
            "1.0.test",
            "home-win11",
            "warm",
            10,
            "Completed",
            0,
            &[("capture", 10)],
        )
        .replace("\"phase\":\"capture\"", "\"phase\":\"\"");
        let error = build_snapshot(&empty_phase).expect_err("empty phase must fail");
        assert!(error
            .to_string()
            .contains("timings_ms.phase must not be empty"));
    }

    #[test]
    fn output_without_explicit_parent_is_a_valid_distinct_path() {
        let directory = std::env::temp_dir().join(format!(
            "stepler-performance-snapshot-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let input = directory.join("input.jsonl");
        std::fs::write(&input, "{}").unwrap();

        assert!(ensure_distinct_paths(&input, Path::new("snapshot.json")).is_ok());

        std::fs::remove_file(&input).unwrap();
        std::fs::remove_dir(&directory).unwrap();
    }

    #[test]
    fn percentile_uses_nearest_rank() {
        assert_eq!(percentile(&[300, 100, 200], 50), 200);
        assert_eq!(percentile(&[300, 100, 200], 90), 300);
        assert_eq!(percentile(&[], 95), 0);
    }
}
