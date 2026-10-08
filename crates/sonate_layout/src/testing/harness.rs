//! The fixture runner: compares an engine's layout with Chrome's snapshots and keeps the output
//! short enough to act on.
//!
//! - One test per fixture file; a fixture expands into one case per `@matrix` combination.
//! - Only the first new failure prints a tree, cut off below the first wrong node.
//! - `known_failures.txt` lists cases that fail today. Listed failures don't fail the run;
//!   unlisted failures and listed cases that pass now do. `FIXTURES_UPDATE=prune` removes
//!   entries that pass, `FIXTURES_UPDATE=accept` also records new failures.
//! - Every failing case is drawn into an HTML report (Chrome vs. engine).

use super::snapshot::{self, Rect4, Snapshot, SnapshotCase};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Chrome lays out in 1/64 px units and drops remainders, so allow a little slack.
pub const DEFAULT_TOLERANCE: f64 = 1.0 / 16.0;

/// Lays out one case.
pub type Engine = dyn Fn(&Fixture, &SnapshotCase) -> Result<CaseOutput, String> + Send + Sync;

/// What an engine produced for one case, in snapshot order.
pub struct CaseOutput {
    /// Each node's rect relative to its parent.
    pub rects: Vec<Rect4>,
    /// Each node's computed values of the fixture's `@check` properties, formatted like
    /// `getComputedStyle`. `None` skips the value checks.
    pub values: Option<Vec<BTreeMap<String, String>>>,
}

pub struct Suite {
    /// Shown in the summary, e.g. `sonate_layout`.
    pub name: String,
    pub fixtures_dir: PathBuf,
    pub base_css: PathBuf,
    pub known_failures: PathBuf,
    pub report: PathBuf,
}

impl Suite {
    /// The fixtures of the `sonate_layout` crate with a suite-specific known-failures file.
    pub fn with_layout_fixtures(name: &str, known_failures: PathBuf) -> Self {
        let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let target_dir = std::env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| crate_dir.ancestors().nth(2).unwrap_or(crate_dir).join("target"));
        Self {
            name: name.to_owned(),
            fixtures_dir: crate_dir.join("tests/fixtures"),
            base_css: crate_dir.join("tools/bless/base.css"),
            known_failures,
            report: target_dir.join(format!("{name}-fixtures-report.html")),
        }
    }
}

pub struct Fixture {
    /// Path below the fixtures directory without extension, e.g. `nested/app_shell`.
    pub name: String,
    pub path: PathBuf,
    pub html: String,
    pub snapshot: Result<Snapshot, String>,
}

/// Finds all `*.html` fixtures and loads their snapshots.
pub fn discover(suite: &Suite) -> Vec<Fixture> {
    let base_css = std::fs::read_to_string(&suite.base_css).unwrap_or_default();
    let mut paths = Vec::new();
    collect_html(&suite.fixtures_dir, &mut paths);
    paths.sort();

    paths
        .into_iter()
        .map(|path| {
            let name = path
                .strip_prefix(&suite.fixtures_dir)
                .unwrap_or(&path)
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/");
            let html = std::fs::read_to_string(&path).unwrap_or_default();
            let snapshot = load_snapshot(&path, &html, &base_css, &name);
            Fixture {
                name,
                path,
                html,
                snapshot,
            }
        })
        .collect()
}

fn collect_html(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_html(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "html") {
            out.push(path);
        }
    }
}

fn load_snapshot(path: &Path, html: &str, base_css: &str, name: &str) -> Result<Snapshot, String> {
    let bless = format!("run `deno task bless {name}` in crates/sonate_layout/tools/bless");
    let json = std::fs::read_to_string(path.with_extension("snap.json"))
        .map_err(|_| format!("no snapshot; {bless}"))?;
    let snapshot = snapshot::parse(&json).map_err(|err| format!("broken snapshot: {err}; {bless}"))?;
    if snapshot.hash != snapshot::fixture_hash(html, base_css) {
        return Err(format!("the fixture or base.css changed since the snapshot was taken; {bless}"));
    }
    Ok(snapshot)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UpdateMode {
    None,
    Prune,
    Accept,
}

impl UpdateMode {
    pub fn from_env() -> Self {
        match std::env::var("FIXTURES_UPDATE").as_deref() {
            Ok("prune") => UpdateMode::Prune,
            Ok("accept") => UpdateMode::Accept,
            _ => UpdateMode::None,
        }
    }
}

struct CaseResult {
    key: String,
    failure: Option<CaseFailure>,
}

struct CaseFailure {
    /// One line naming the first wrong node.
    summary: String,
    /// The tree, cut off below wrong nodes.
    tree: String,
    /// HTML fragment for the report.
    report: String,
}

struct FixtureResult {
    name: String,
    error: Option<String>,
    cases: Vec<CaseResult>,
}

fn case_key(fixture: &str, params: &str) -> String {
    if params.is_empty() {
        fixture.to_owned()
    } else {
        format!("{fixture} [{params}]")
    }
}

fn belongs_to(key: &str, fixture: &str) -> bool {
    key == fixture || key.starts_with(&format!("{fixture} ["))
}

fn evaluate_fixture(fixture: &Fixture, engine: &Engine) -> FixtureResult {
    let snapshot = match &fixture.snapshot {
        Ok(snapshot) => snapshot,
        Err(err) => {
            return FixtureResult {
                name: fixture.name.clone(),
                error: Some(err.clone()),
                cases: Vec::new(),
            }
        }
    };
    let tolerance = snapshot.tolerance.unwrap_or(DEFAULT_TOLERANCE);

    let cases = snapshot
        .cases
        .iter()
        .map(|case| {
            let key = case_key(&fixture.name, &case.params);
            let failure = match engine(fixture, case) {
                Ok(actual) if actual.rects.len() == case.nodes.len() => {
                    compare(case, &actual, tolerance, &key)
                }
                Ok(actual) => Some(error_failure(
                    &key,
                    format!("engine returned {} rects for {} nodes", actual.rects.len(), case.nodes.len()),
                )),
                Err(err) => Some(error_failure(&key, err)),
            };
            CaseResult { key, failure }
        })
        .collect();

    FixtureResult {
        name: fixture.name.clone(),
        error: None,
        cases,
    }
}

fn error_failure(key: &str, message: String) -> CaseFailure {
    CaseFailure {
        summary: format!("error: {message}"),
        tree: format!("    error: {message}\n"),
        report: format!("<section><h3>{}</h3><p>error: {}</p></section>", escape(key), escape(&message)),
    }
}

/// Decides whether the fixture's test passes, given the known failures.
fn verdict(result: &FixtureResult, known: &BTreeSet<String>, mode: UpdateMode) -> Result<(), String> {
    if let Some(err) = &result.error {
        return Err(err.clone());
    }

    let failing = result.cases.iter().filter(|case| case.failure.is_some()).count();
    let new_failures: Vec<&CaseResult> = result
        .cases
        .iter()
        .filter(|case| case.failure.is_some() && !known.contains(&case.key))
        .collect();
    let now_passing: Vec<&str> = result
        .cases
        .iter()
        .filter(|case| case.failure.is_none() && known.contains(&case.key))
        .map(|case| case.key.as_str())
        .collect();
    let stale: Vec<&str> = known
        .iter()
        .filter(|key| belongs_to(key, &result.name))
        .filter(|key| !result.cases.iter().any(|case| &&case.key == key))
        .map(String::as_str)
        .collect();

    let mut message = String::new();
    if mode != UpdateMode::Accept && !new_failures.is_empty() {
        let _ = writeln!(
            message,
            "{failing} of {} cases fail ({} new, {} known)\n",
            result.cases.len(),
            new_failures.len(),
            failing - new_failures.len()
        );
        for (index, case) in new_failures.iter().enumerate() {
            let failure = case.failure.as_ref().unwrap();
            if index == 0 {
                let _ = writeln!(message, "NEW FAILURE {}\n{}", case.key, failure.tree);
            } else {
                let _ = writeln!(message, "NEW FAILURE {}: {}", case.key, failure.summary);
            }
        }
    }
    if mode == UpdateMode::None && !(now_passing.is_empty() && stale.is_empty()) {
        let _ = writeln!(
            message,
            "\nThese known_failures.txt entries are outdated; remove them or rerun with FIXTURES_UPDATE=prune:"
        );
        for key in now_passing.iter().chain(&stale) {
            let _ = writeln!(message, "  {key}");
        }
    }

    if message.is_empty() {
        Ok(())
    } else {
        Err(message)
    }
}

/// Compares the engine's rects and checked values with Chrome's and renders the differences.
fn compare(case: &SnapshotCase, output: &CaseOutput, tolerance: f64, key: &str) -> Option<CaseFailure> {
    let actual = &output.rects;
    let expected: Vec<Rect4> = case.nodes.iter().map(|node| node.rect).collect();
    let rect_wrong: Vec<bool> = expected
        .iter()
        .zip(actual)
        .map(|(expected, actual)| {
            expected
                .iter()
                .zip(actual)
                .any(|(expected, actual)| differs(*expected, *actual, tolerance))
        })
        .collect();
    let value_errors: Vec<Vec<String>> = case
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| {
            let Some(values) = &output.values else {
                return Vec::new();
            };
            node.checks
                .iter()
                .filter_map(|(property, expected)| {
                    let actual = values.get(index).and_then(|values| values.get(property));
                    (actual != Some(expected)).then(|| {
                        let actual = actual.map_or("<unsupported>", String::as_str);
                        format!("{property} {actual} (expected {expected})")
                    })
                })
                .collect()
        })
        .collect();
    let wrong: Vec<bool> = rect_wrong
        .iter()
        .zip(&value_errors)
        .map(|(rect_wrong, errors)| *rect_wrong || !errors.is_empty())
        .collect();
    if !wrong.contains(&true) {
        return None;
    }

    let children = case.children();
    let labels = labels(case, &children);
    let mut subtree_wrong = wrong.clone();
    for index in (0..case.nodes.len()).rev() {
        if children[index].iter().any(|&child| subtree_wrong[child]) {
            subtree_wrong[index] = true;
        }
    }

    let mut tree = String::new();
    let nodes = TreeNodes {
        case,
        children: &children,
        labels: &labels,
        expected: &expected,
        actual,
        rect_wrong: &rect_wrong,
        value_errors: &value_errors,
        subtree_wrong: &subtree_wrong,
    };
    render_tree(0, &nodes, &mut tree);

    // The first wrong node in tree order is usually the cause; its descendants follow from it.
    let first = (0..case.nodes.len()).find(|&index| wrong[index]).unwrap();
    let mut path = vec![labels[first].clone()];
    let mut depth = case.nodes[first].depth;
    for index in (0..first).rev() {
        if case.nodes[index].depth < depth {
            depth = case.nodes[index].depth;
            path.push(labels[index].clone());
        }
    }
    path.reverse();
    let mut problems: Vec<String> = ["x", "y", "width", "height"]
        .iter()
        .enumerate()
        .filter(|&(field, _)| differs(expected[first][field], actual[first][field], tolerance))
        .map(|(field, name)| {
            format!(
                "{name} {} (expected {})",
                number(actual[first][field]),
                number(expected[first][field])
            )
        })
        .collect();
    problems.extend(value_errors[first].iter().cloned());
    let summary = format!("first wrong node {}: {}", path.join(" > "), problems.join(", "));

    Some(CaseFailure {
        summary,
        tree,
        report: report_case(key, &children, &labels, &expected, actual, &wrong),
    })
}

/// Whether two lengths differ by more than the tolerance; NaN always differs.
#[allow(clippy::neg_cmp_op_on_partial_ord)]
fn differs(expected: f64, actual: f64, tolerance: f64) -> bool {
    !((expected - actual).abs() <= tolerance)
}

/// `#id`, `.class` or `div`, with a sibling index when the label is not unique.
fn labels(case: &SnapshotCase, children: &[Vec<usize>]) -> Vec<String> {
    let mut labels: Vec<String> = case.nodes.iter().map(|node| node.label.clone()).collect();
    for siblings in children {
        for &child in siblings {
            let label = &case.nodes[child].label;
            if label.starts_with('#') {
                continue;
            }
            let same: Vec<usize> = siblings
                .iter()
                .copied()
                .filter(|&other| &case.nodes[other].label == label)
                .collect();
            if same.len() > 1 {
                let position = same.iter().position(|&other| other == child).unwrap();
                labels[child] = format!("{label}[{position}]");
            }
        }
    }
    for (label, node) in labels.iter_mut().zip(&case.nodes) {
        if let Some(text) = &node.text {
            let short: String = text.chars().take(12).collect();
            let ellipsis = if short.len() < text.len() { "…" } else { "" };
            let _ = write!(label, " \"{short}{ellipsis}\"");
        }
    }
    labels
}

/// Everything `render_tree` needs about the case.
struct TreeNodes<'a> {
    case: &'a SnapshotCase,
    children: &'a [Vec<usize>],
    labels: &'a [String],
    expected: &'a [Rect4],
    actual: &'a [Rect4],
    rect_wrong: &'a [bool],
    value_errors: &'a [Vec<String>],
    subtree_wrong: &'a [bool],
}

fn render_tree(index: usize, nodes: &TreeNodes, out: &mut String) {
    let indent = "  ".repeat(nodes.case.nodes[index].depth + 2);
    let _ = write!(out, "{indent}{:<24} {}", nodes.labels[index], rect(nodes.actual[index]));
    if nodes.rect_wrong[index] {
        let _ = write!(out, "   ✗ expected {}", rect(nodes.expected[index]));
    }
    if !nodes.value_errors[index].is_empty() {
        let _ = write!(out, "   ✗ {}", nodes.value_errors[index].join(", "));
    }
    if nodes.rect_wrong[index] {
        let hidden = count_descendants(index, nodes.children);
        if hidden > 0 {
            let _ = write!(out, "  (+{hidden} descendants hidden)");
        }
        out.push('\n');
        return;
    }
    out.push('\n');

    let mut matching = 0;
    for &child in &nodes.children[index] {
        if nodes.subtree_wrong[child] {
            render_tree(child, nodes, out);
        } else {
            matching += 1;
        }
    }
    if matching > 0 {
        let _ = writeln!(out, "{indent}  … {matching} matching");
    }
}

fn count_descendants(index: usize, children: &[Vec<usize>]) -> usize {
    children[index]
        .iter()
        .map(|&child| 1 + count_descendants(child, children))
        .sum()
}

fn number(value: f64) -> String {
    if (value - value.round()).abs() < 1e-6 {
        format!("{}", value.round() as i64)
    } else {
        let text = format!("{value:.3}");
        text.trim_end_matches('0').to_owned()
    }
}

fn rect(rect: Rect4) -> String {
    format!(
        "{},{} {}x{}",
        number(rect[0]),
        number(rect[1]),
        number(rect[2]),
        number(rect[3])
    )
}

fn load_known_failures(path: &Path) -> BTreeSet<String> {
    // Shows known failures like new ones, for debugging.
    if std::env::var_os("FIXTURES_IGNORE_KNOWN").is_some() {
        return BTreeSet::new();
    }
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

fn write_known_failures(path: &Path, entries: &BTreeSet<String>) {
    let mut text = String::from(
        "# Fixture cases that fail today. Maintained by the fixture runner:\n\
         #   FIXTURES_UPDATE=prune  removes entries that pass now\n\
         #   FIXTURES_UPDATE=accept also adds new failures\n",
    );
    for entry in entries {
        text.push_str(entry);
        text.push('\n');
    }
    if let Err(err) = std::fs::write(path, text) {
        eprintln!("could not write {}: {err}", path.display());
    }
}

/// Updates the known failures, prints the summary and writes the report.
fn finish(suite: &Suite, known: &BTreeSet<String>, mode: UpdateMode, results: &[FixtureResult]) -> String {
    if mode != UpdateMode::None {
        let mut entries = known.clone();
        for result in results.iter().filter(|result| result.error.is_none()) {
            entries.retain(|key| !belongs_to(key, &result.name));
            for case in &result.cases {
                let keep = match mode {
                    UpdateMode::Accept => case.failure.is_some(),
                    _ => case.failure.is_some() && known.contains(&case.key),
                };
                if keep {
                    entries.insert(case.key.clone());
                }
            }
        }
        if &entries != known {
            write_known_failures(&suite.known_failures, &entries);
            eprintln!("updated {}", suite.known_failures.display());
        }
    }

    // Per-category counts: (cases, passing, known failures, new failures).
    let mut categories: BTreeMap<String, (usize, usize, usize, usize)> = BTreeMap::new();
    let mut fragments = (String::new(), String::new());
    for result in results {
        let category = result.name.split('/').next().unwrap_or("").to_owned();
        let counts = categories.entry(category).or_default();
        for case in &result.cases {
            counts.0 += 1;
            match &case.failure {
                None => counts.1 += 1,
                Some(failure) if known.contains(&case.key) => {
                    counts.2 += 1;
                    fragments.1.push_str(&failure.report);
                }
                Some(failure) => {
                    counts.3 += 1;
                    fragments.0.push_str(&failure.report);
                }
            }
        }
    }

    let total = categories.values().fold((0, 0, 0, 0), |acc, counts| {
        (acc.0 + counts.0, acc.1 + counts.1, acc.2 + counts.2, acc.3 + counts.3)
    });
    let mut summary = format!(
        "\n{} fixtures: {} cases, {} pass, {} known failures, {} new failures\n",
        suite.name, total.0, total.1, total.2, total.3
    );
    for (category, (cases, passing, known_failing, new_failing)) in &categories {
        let _ = write!(summary, "  {category:<12} {passing:>4}/{cases:<4}");
        if *known_failing > 0 {
            let _ = write!(summary, "  {known_failing} known");
        }
        if *new_failing > 0 {
            let _ = write!(summary, "  {new_failing} NEW");
        }
        summary.push('\n');
    }

    if total.2 + total.3 > 0 {
        write_report(&suite.report, &suite.name, &fragments.0, &fragments.1);
        let _ = writeln!(summary, "  report: {}", suite.report.display());
    }
    summary
}

/// Runs all fixtures as libtest-mimic tests. Use from a `harness = false` test target.
pub fn run_main(suite: Suite, engine: Arc<Engine>) -> std::process::ExitCode {
    let args = libtest_mimic::Arguments::from_args();
    let known = Arc::new(load_known_failures(&suite.known_failures));
    let mode = UpdateMode::from_env();
    let results = Arc::new(Mutex::new(Vec::new()));

    let trials = discover(&suite)
        .into_iter()
        .map(|fixture| {
            let (known, results, engine) = (known.clone(), results.clone(), engine.clone());
            libtest_mimic::Trial::test(fixture.name.clone(), move || {
                let result = evaluate_fixture(&fixture, &*engine);
                let verdict = verdict(&result, &known, mode);
                results.lock().unwrap().push(result);
                verdict.map_err(Into::into)
            })
        })
        .collect();

    let conclusion = libtest_mimic::run(&args, trials);
    let results = results.lock().unwrap();
    if !results.is_empty() {
        println!("{}", finish(&suite, &known, mode, &results));
    }
    if conclusion.has_failed() {
        std::process::ExitCode::from(101)
    } else {
        std::process::ExitCode::SUCCESS
    }
}

/// Runs all fixtures whose name contains `filter` inside a single test, e.g. from a crate's
/// unit tests. Returns the summary, or all failure messages.
pub fn run_all(suite: &Suite, engine: &Engine, filter: Option<&str>) -> Result<String, String> {
    let known = load_known_failures(&suite.known_failures);
    let mode = UpdateMode::from_env();
    let mut failures = String::new();
    let mut results = Vec::new();

    for fixture in discover(suite) {
        if filter.is_some_and(|filter| !fixture.name.contains(filter)) {
            continue;
        }
        let result = evaluate_fixture(&fixture, engine);
        if let Err(message) = verdict(&result, &known, mode) {
            let _ = writeln!(failures, "---- {} ----\n{message}", fixture.name);
        }
        results.push(result);
    }

    let summary = finish(suite, &known, mode, &results);
    if failures.is_empty() {
        Ok(summary)
    } else {
        Err(format!("{failures}{summary}"))
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Draws Chrome's boxes next to the engine's boxes.
fn report_case(
    key: &str,
    children: &[Vec<usize>],
    labels: &[String],
    expected: &[Rect4],
    actual: &[Rect4],
    wrong: &[bool],
) -> String {
    let absolute = |rects: &[Rect4]| {
        let mut out = rects.to_vec();
        for index in 0..rects.len() {
            for &child in &children[index] {
                out[child][0] += out[index][0];
                out[child][1] += out[index][1];
            }
        }
        out
    };
    let (expected_abs, actual_abs) = (absolute(expected), absolute(actual));
    let width = expected_abs
        .iter()
        .chain(&actual_abs)
        .map(|rect| rect[0] + rect[2])
        .fold(1.0, f64::max);
    let height = expected_abs
        .iter()
        .chain(&actual_abs)
        .map(|rect| rect[1] + rect[3])
        .fold(1.0, f64::max);
    let scale = (360.0 / width).min(1.0);

    let panel = |title: &str, rects: &[Rect4], other: &[Rect4]| {
        let mut html = format!(
            "<figure><figcaption>{title}</figcaption><div class=canvas style=\"width:{:.1}px;height:{:.1}px\"><div style=\"transform:scale({scale});transform-origin:0 0\">",
            width * scale,
            height * scale
        );
        for (index, rect) in rects.iter().enumerate() {
            let class = if wrong[index] { "box wrong" } else { "box" };
            let _ = write!(
                html,
                "<div class=\"{class}\" style=\"left:{}px;top:{}px;width:{}px;height:{}px\" title=\"{} — {} (other: {})\"></div>",
                rect[0],
                rect[1],
                rect[2],
                rect[3],
                escape(&labels[index]),
                self::rect(*rect),
                self::rect(other[index]),
            );
        }
        html.push_str("</div></div></figure>");
        html
    };
    format!(
        "<section><h3>{}</h3><div class=pair>{}{}</div></section>",
        escape(key),
        panel("Chrome", &expected_abs, &actual_abs),
        panel("engine", &actual_abs, &expected_abs),
    )
}

fn write_report(path: &Path, suite: &str, new_failures: &str, known_failures: &str) {
    let html = format!(
        r#"<!doctype html><meta charset="utf-8"><title>{suite} fixture failures</title>
<style>
body {{ font: 13px system-ui, sans-serif; margin: 16px; background: #fff; color: #222; }}
h3 {{ font: 600 13px ui-monospace, monospace; margin: 16px 0 6px; }}
.pair {{ display: flex; gap: 24px; flex-wrap: wrap; }}
figure {{ margin: 0; }}
figcaption {{ color: #666; margin-bottom: 4px; }}
.canvas {{ position: relative; overflow: hidden; outline: 1px solid #ddd; }}
.box {{ position: absolute; box-sizing: border-box; border: 1px solid #4a7bd0; background: rgba(74,123,208,.06); }}
.box.wrong {{ border: 2px solid #d03b3b; background: rgba(208,59,59,.12); }}
.box:hover {{ background: rgba(255,200,0,.35); }}
details {{ margin-top: 24px; }}
</style>
<h1>{suite}: failing fixture cases</h1>
<p>Red boxes differ from Chrome. Hover a box for its rect.</p>
<h2>New failures</h2>{new_failures}
<details><summary>Known failures</summary>{known_failures}</details>
"#
    );
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(err) = std::fs::write(path, html) {
        eprintln!("could not write {}: {err}", path.display());
    }
}
