use crate::engine::{
  context::SessionCtx,
  history::ValueHistory,
  trace::{Pos, Trace, TraceEvent, TraceIdx},
};
use std::fmt::Debug;

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
}

#[derive(Debug, PartialEq, Eq)]
pub enum FindingKind {
  NullDereference,
}

#[derive(Debug)]
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

#[derive(Debug)]
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

#[derive(Debug)]
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

#[derive(Debug)]
pub enum FindingTrace {
  Flow(CodeFlow),
  TraceWithHistory(Trace, Option<ValueHistory>),
}

#[derive(Debug)]
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
