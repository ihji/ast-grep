use crate::engine::{
  context::SessionCtx,
  history::ValueHistory,
  trace::{Pos, Trace, TraceEvent, TraceIdx},
};
use std::fmt::{self, Debug};

#[derive(Debug)]
pub struct Reports {
  pub all: Vec<Finding>,
}

impl Reports {
  pub fn new() -> Self {
    Self { all: vec![] }
  }
  pub fn finalize(&mut self, ctx: &SessionCtx) {
    for finding in self.all.iter_mut() {
      finding.with_code_flow(ctx);
    }
  }
  pub fn clear(&mut self) {
    self.all.clear();
  }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FindingKind {
  NullDereference,
}

#[derive(Debug, Clone)]
pub struct CodeFlow {
  thread_flows: Vec<ThreadFlow>,
}

impl CodeFlow {
  pub fn new() -> Self {
    Self {
      thread_flows: vec![],
    }
  }

  pub fn add_thread_flow(mut self, thread_flow: ThreadFlow) -> Self {
    self.thread_flows.push(thread_flow);
    self
  }

  pub fn thread_flows(&self) -> &[ThreadFlow] {
    &self.thread_flows
  }
}

impl fmt::Display for CodeFlow {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let thread_count = self.thread_flows.len();
    if thread_count == 0 {
      writeln!(f, "Code Flow: no recorded threads")?;
      return Ok(());
    }

    writeln!(
      f,
      "Code Flow ({} thread{})",
      thread_count,
      if thread_count == 1 { "" } else { "s" }
    )?;

    for (thread_idx, thread) in self.thread_flows.iter().enumerate() {
      let step_count = thread.locations.len();
      writeln!(
        f,
        "  Thread #{} ({} step{})",
        thread_idx + 1,
        step_count,
        if step_count == 1 { "" } else { "s" }
      )?;

      if step_count == 0 {
        writeln!(f, "    No recorded steps.")?;
      } else {
        let step_count_digits = step_count.to_string().len();
        let step_width = if step_count_digits < 2 {
          2
        } else {
          step_count_digits
        };

        for (step_idx, location) in thread.locations.iter().enumerate() {
          let nesting_indent = "  ".repeat(location.nesting_level());
          let indent = format!("    {}", nesting_indent);
          let step_label = format!("{:>width$}.", step_idx + 1, width = step_width);

          writeln!(f, "{}{} {}", indent, step_label, location.location())?;

          let message_offset = " ".repeat(step_label.len() + 1);
          let base_message_indent = format!("{}{}", indent, message_offset);
          let bullet_indent = format!("{}- ", base_message_indent);
          let continuation_indent = format!("{}  ", base_message_indent);
          let raw_message = location.message().unwrap_or("").trim();

          if raw_message.is_empty() {
            writeln!(f, "{}No additional details.", bullet_indent)?;
          } else {
            let mut lines = raw_message.lines();
            if let Some(first_line) = lines.next() {
              writeln!(f, "{}{}", bullet_indent, first_line)?;
              for line in lines {
                writeln!(f, "{}{}", continuation_indent, line)?;
              }
            }
          }
        }
      }

      if thread_idx + 1 < thread_count {
        f.write_str("\n")?;
      }
    }

    Ok(())
  }
}

#[derive(Debug, Clone)]
pub struct ThreadFlow {
  locations: Vec<ThreadFlowLocation>,
}

impl ThreadFlow {
  pub fn new() -> Self {
    Self { locations: vec![] }
  }

  pub fn add_location(mut self, location: ThreadFlowLocation) -> Self {
    self.locations.push(location);
    self
  }

  pub fn locations(&self) -> &[ThreadFlowLocation] {
    &self.locations
  }
}

#[derive(Debug, Clone)]
pub struct ThreadFlowLocation {
  location: Pos,
  message: Option<String>,
  nesting_level: usize,
}

impl ThreadFlowLocation {
  pub fn new(location: Pos) -> Self {
    Self {
      location,
      message: None,
      nesting_level: 0,
    }
  }

  pub fn with_message(mut self, message: String) -> Self {
    self.message = Some(message);
    self
  }

  pub fn with_nesting_level(mut self, nesting_level: usize) -> Self {
    self.nesting_level = nesting_level;
    self
  }

  pub fn location(&self) -> &Pos {
    &self.location
  }

  pub fn message(&self) -> Option<&str> {
    self.message.as_deref()
  }

  pub fn nesting_level(&self) -> usize {
    self.nesting_level
  }
}

#[derive(Debug, Clone)]
pub enum FindingTrace {
  Flow(CodeFlow),
  TraceWithHistory(Trace, Option<ValueHistory>),
}

impl fmt::Display for FindingTrace {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      FindingTrace::Flow(code_flow) => write!(f, "{}", code_flow),
      FindingTrace::TraceWithHistory(trace, history) => {
        writeln!(f, "Trace: {:?}", trace)?;
        if let Some(h) = history {
          write!(f, "History: {:?}", h)?;
        }
        Ok(())
      }
    }
  }
}

#[derive(Debug, Clone)]
pub struct Finding {
  kind: FindingKind,
  location: Pos,
  trace: FindingTrace,
}

impl Finding {
  pub fn kind(&self) -> &FindingKind {
    &self.kind
  }

  pub fn location(&self) -> Pos {
    self.location.clone()
  }

  pub fn trace(&self) -> &FindingTrace {
    &self.trace
  }

  pub fn new(kind: FindingKind, trace: Trace, history: Option<ValueHistory>) -> Self {
    Self {
      kind,
      location: trace.current_pos().unwrap_or(Pos {
        file: "unknown".to_string(),
        line: 0,
      }),
      trace: FindingTrace::TraceWithHistory(trace, history),
    }
  }

  fn trace_idx_to_thread_flow(
    &self,
    ctx: &SessionCtx,
    tail: Option<TraceIdx>,
    history: Option<&ValueHistory>,
  ) -> ThreadFlow {
    let mut thread_flow = ThreadFlow::new();
    for ev in ctx
      .trace_arena
      .collect_forward_with_history(tail, history)
      .into_iter()
    {
      let location = ev.at().cloned().unwrap_or(Pos {
        file: "unknown".to_string(),
        line: 0,
      });
      let message = Some(format!("{}", ev));
      thread_flow = thread_flow.add_location(
        ThreadFlowLocation::new(location)
          .with_message(message.unwrap_or_default())
          .with_nesting_level(0),
      );
      if let TraceEvent::Invocation {
        ctx: invocation_ctx,
        ..
      } = ev
      {
        let nesting_level = thread_flow
          .locations()
          .last()
          .map_or(0, |loc| loc.nesting_level() + 1);
        let nested_flow = self.trace_idx_to_thread_flow(ctx, invocation_ctx.tail(), history);
        for loc in nested_flow.locations().iter() {
          thread_flow = thread_flow.add_location(
            ThreadFlowLocation::new(loc.location().clone())
              .with_message(loc.message().unwrap_or_default().to_string())
              .with_nesting_level(nesting_level),
          );
        }
      }
    }
    thread_flow
  }

  pub fn with_code_flow(&mut self, ctx: &SessionCtx) {
    if let FindingTrace::TraceWithHistory(trace, history) = &self.trace {
      let mut code_flow = CodeFlow::new();
      let thread_flow = self.trace_idx_to_thread_flow(ctx, trace.tail(), history.as_ref());
      code_flow = code_flow.add_thread_flow(thread_flow);
      self.trace = FindingTrace::Flow(code_flow);
    }
  }
}
