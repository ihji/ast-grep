use anyhow::{bail, ensure, Context, Result};
use ast_grep_language::SupportLang;
use ast_grep_semantic::{
  analyze_source,
  engine::reports::{CodeFlow, Finding, FindingKind, FindingTrace},
};
use std::{
  collections::HashMap,
  fs,
  path::{Path, PathBuf},
};

#[derive(Debug, Default)]
struct DirectiveGroup {
  finding: Option<PendingFinding>,
  creates: Vec<CreateExpectation>,
  assumes: Vec<AssumeExpectation>,
}

impl DirectiveGroup {
  fn is_empty(&self) -> bool {
    self.finding.is_none() && self.creates.is_empty() && self.assumes.is_empty()
  }
}

#[derive(Debug)]
struct PendingFinding {
  kind: FindingKind,
  line: usize,
  directive_line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum DirectiveId {
  Named(String),
  Default,
}

impl DirectiveId {
  fn new(id: Option<&str>) -> Self {
    match id.and_then(|value| {
      let trimmed = value.trim();
      if trimmed.is_empty() {
        None
      } else {
        Some(trimmed.to_string())
      }
    }) {
      Some(value) => DirectiveId::Named(value),
      None => DirectiveId::Default,
    }
  }

  fn describe(&self) -> String {
    match self {
      DirectiveId::Named(value) => format!("with id {}", value),
      DirectiveId::Default => "without id".to_string(),
    }
  }
}

struct DirectiveMeta<'a> {
  value: &'a str,
  id: Option<&'a str>,
}

#[derive(Debug)]
struct ExpectedFinding {
  kind: FindingKind,
  line: usize,
  id: Option<String>,
  creates: Vec<CreateExpectation>,
  assumes: Vec<AssumeExpectation>,
}

#[derive(Debug, Clone)]
struct CreateExpectation {
  line: usize,
  note: String,
}

#[derive(Debug, Clone)]
struct AssumeExpectation {
  line: usize,
  expected_true: bool,
  raw_value: String,
}

#[test]
fn semantic_e2e_fixtures() -> Result<()> {
  let fixtures_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("e2e_tests");
  ensure!(
    fixtures_root.exists(),
    "Fixture directory {} does not exist",
    fixtures_root.display()
  );

  let mut fixtures = Vec::new();
  collect_fixtures(&fixtures_root, &mut fixtures)?;
  fixtures.sort();

  ensure!(
    !fixtures.is_empty(),
    "No fixtures found under {}",
    fixtures_root.display()
  );

  for path in fixtures {
    run_fixture(&path)?;
  }
  Ok(())
}

fn collect_fixtures(dir: &Path, acc: &mut Vec<PathBuf>) -> Result<()> {
  for entry in fs::read_dir(dir).with_context(|| format!("Reading {}", dir.display()))? {
    let entry = entry?;
    let path = entry.path();
    if path.is_dir() {
      collect_fixtures(&path, acc)?;
    } else if lang_from_extension(&path).is_some() {
      acc.push(path);
    }
  }
  Ok(())
}

fn run_fixture(path: &Path) -> Result<()> {
  let lang = lang_from_extension(path)
    .with_context(|| format!("Unsupported fixture extension: {}", path.display()))?;
  let source = fs::read_to_string(path)
    .with_context(|| format!("Failed to read fixture {}", path.display()))?;
  let lines: Vec<String> = source.lines().map(str::to_string).collect();
  let expectations = parse_expectations(&lines)
    .with_context(|| format!("Parsing expectations for {}", path.display()))?;

  ensure!(
    !expectations.is_empty(),
    "Fixture {} must declare at least one FIND directive",
    path.display()
  );

  let reports = analyze_source(source, lang, Some(path.display().to_string()))
    .with_context(|| format!("Analyzing {}", path.display()))?;

  validate_findings(path, &expectations, &reports.all)?;
  Ok(())
}

fn parse_expectations(lines: &[String]) -> Result<Vec<ExpectedFinding>> {
  let mut expectations = Vec::new();
  let mut directives_by_id: HashMap<DirectiveId, DirectiveGroup> = HashMap::new();
  let mut find_order = Vec::new();

  for (idx, line) in lines.iter().enumerate() {
    if let Some((directive, meta)) = extract_directive(line) {
      let code_line_idx = next_code_line_index(lines, idx)
        .with_context(|| format!("Directive on line {} missing target statement", idx + 1))?;
      let code_line = code_line_idx + 1; // convert to 1-based
      let key = DirectiveId::new(meta.id);
      let group = directives_by_id.entry(key.clone()).or_default();

      match directive {
        DirectiveKind::Create => {
          group.creates.push(CreateExpectation {
            line: code_line,
            note: meta.value.to_string(),
          });
        }
        DirectiveKind::Assume => {
          let expected_true = parse_bool(meta.value).with_context(|| {
            format!("Invalid ASSUME value '{}' on line {}", meta.value, idx + 1)
          })?;
          group.assumes.push(AssumeExpectation {
            line: code_line,
            expected_true,
            raw_value: meta.value.to_string(),
          });
        }
        DirectiveKind::Find => {
          if let Some(existing) = group.finding.as_ref() {
            bail!(
              "Duplicate FIND directive {} on lines {} and {}",
              key.describe(),
              existing.directive_line,
              idx + 1
            );
          }
          let kind = parse_finding_kind(meta.value)
            .with_context(|| format!("Unknown FIND kind '{}' on line {}", meta.value, idx + 1))?;
          group.finding = Some(PendingFinding {
            kind,
            line: code_line,
            directive_line: idx + 1,
          });
          find_order.push(key);
        }
      }
    }
  }

  for id in find_order {
    let group = directives_by_id
      .remove(&id)
      .with_context(|| format!("Internal error resolving directives {}", id.describe()))?;
    let DirectiveGroup {
      finding,
      creates,
      assumes,
    } = group;
    let finding = finding.with_context(|| {
      format!(
        "Directive group {} is missing a FIND directive",
        id.describe()
      )
    })?;
    expectations.push(ExpectedFinding {
      kind: finding.kind,
      line: finding.line,
      id: match &id {
        DirectiveId::Named(value) => Some(value.clone()),
        DirectiveId::Default => None,
      },
      creates,
      assumes,
    });
  }

  ensure!(
    directives_by_id.values().all(|pending| pending.is_empty()),
    "Dangling directives without FIND target"
  );
  Ok(expectations)
}

fn validate_findings(
  path: &Path,
  expected: &[ExpectedFinding],
  findings: &[Finding],
) -> Result<()> {
  ensure!(
    findings.len() == expected.len(),
    "{} expected {} findings, but analyzer reported {}",
    path.display(),
    expected.len(),
    findings.len()
  );

  let mut used = vec![false; findings.len()];
  for exp in expected {
    let index = findings
      .iter()
      .enumerate()
      .find_map(|(idx, finding)| {
        if used[idx] {
          return None;
        }
        let location = finding.location();
        (finding.kind() == &exp.kind && location.line == exp.line).then_some(idx)
      })
      .with_context(|| {
        let id_hint = exp
          .id
          .as_ref()
          .map(|id| format!(" with id {}", id))
          .unwrap_or_default();
        format!(
          "{} missing finding {:?}{} at line {}",
          path.display(),
          exp.kind,
          id_hint,
          exp.line
        )
      })?;
    used[index] = true;
    verify_directives(path, &findings[index], exp)?;
  }
  Ok(())
}

fn verify_directives(path: &Path, finding: &Finding, expected: &ExpectedFinding) -> Result<()> {
  let flow = match finding.trace() {
    FindingTrace::Flow(flow) => flow,
    other => bail!(
      "{} expected code flow data but found {:?}",
      path.display(),
      other
    ),
  };

  for create in &expected.creates {
    ensure!(
      flow_has_creation(flow, create.line, &create.note),
      "{} expected creation event containing '{}' at line {}",
      path.display(),
      create.note,
      create.line
    );
  }

  for assume in &expected.assumes {
    ensure!(
      flow_branch_matches(flow, assume.line, assume.expected_true),
      "{} expected branch at line {} to be taken {}, but no matching event found",
      path.display(),
      assume.line,
      assume.raw_value
    );
  }
  Ok(())
}

fn next_code_line_index(lines: &[String], mut idx: usize) -> Option<usize> {
  while idx + 1 < lines.len() {
    idx += 1;
    let trimmed = lines[idx].trim();
    if trimmed.is_empty()
      || trimmed.starts_with("//")
      || trimmed.starts_with('#')
      || trimmed.starts_with("/*")
      || trimmed.starts_with('*')
      || trimmed.starts_with("*/")
    {
      continue;
    }
    return Some(idx);
  }
  None
}

fn flow_has_creation(flow: &CodeFlow, line: usize, note: &str) -> bool {
  flow
    .thread_flows()
    .iter()
    .flat_map(|thread| thread.locations())
    .filter(|loc| loc.location().line == line)
    .filter_map(|loc| loc.message())
    .any(|msg| {
      let lower = msg.to_ascii_lowercase();
      if !lower.contains("creation") && !lower.contains("null created") {
        return false;
      }
      note.is_empty() || contains_case_insensitive(msg, note)
    })
}

fn flow_branch_matches(flow: &CodeFlow, line: usize, expected_true: bool) -> bool {
  flow
    .thread_flows()
    .iter()
    .flat_map(|thread| thread.locations())
    .filter(|loc| loc.location().line == line)
    .filter_map(|loc| loc.message())
    .any(|msg| match parse_branch_taken(msg) {
      Some(actual) => actual == expected_true,
      None => false,
    })
}

fn parse_branch_taken(message: &str) -> Option<bool> {
  if !message.contains("Branch at") || !message.contains("was taken") {
    return None;
  }
  if message.contains(" was taken True") {
    Some(true)
  } else if message.contains(" was taken False") {
    Some(false)
  } else {
    None
  }
}

fn extract_directive(line: &str) -> Option<(DirectiveKind, DirectiveMeta<'_>)> {
  let comment_index = line.find("//")?;
  let content = line[comment_index + 2..].trim_start();

  const DIRECTIVES: &[(DirectiveKind, &str)] = &[
    (DirectiveKind::Assume, "ASSUME"),
    (DirectiveKind::Create, "CREATE"),
    (DirectiveKind::Find, "FIND"),
  ];

  for (kind, name) in DIRECTIVES {
    if let Some(rest) = content.strip_prefix(name) {
      let rest = rest.trim_start();
      if let Some(after_bracket) = rest.strip_prefix('[') {
        let end_idx = after_bracket.find(']')?;
        let value = after_bracket[..end_idx].trim();
        let remaining = after_bracket[end_idx + 1..].trim_start();
        let id = remaining
          .strip_prefix(':')
          .map(str::trim)
          .filter(|id| !id.is_empty());
        return Some((*kind, DirectiveMeta { value, id }));
      }
      if let Some(after_colon) = rest.strip_prefix(':') {
        let value = after_colon.trim();
        if value.is_empty() {
          return None;
        }
        return Some((*kind, DirectiveMeta { value, id: None }));
      }
    }
  }
  None
}

fn parse_bool(value: &str) -> Result<bool> {
  match value {
    v if v.eq_ignore_ascii_case("true") => Ok(true),
    v if v.eq_ignore_ascii_case("false") => Ok(false),
    _ => bail!("expected boolean value but found '{}'", value),
  }
}

fn parse_finding_kind(value: &str) -> Result<FindingKind> {
  match value {
    "NULL_DEREFERENCE" => Ok(FindingKind::NullDereference),
    _ => bail!("unsupported finding kind '{}'", value),
  }
}

fn lang_from_extension(path: &Path) -> Option<SupportLang> {
  let ext = path.extension()?.to_str()?;
  match ext {
    "go" => Some(SupportLang::Go),
    _ => None,
  }
}

fn contains_case_insensitive(haystack: &str, needle: &str) -> bool {
  haystack
    .to_ascii_lowercase()
    .contains(&needle.to_ascii_lowercase())
}

#[derive(Clone, Copy)]
enum DirectiveKind {
  Create,
  Assume,
  Find,
}
