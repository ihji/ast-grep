use crate::engine::{
  history::ValueHistory,
  trace::{Pos, Trace},
};
use std::fmt::Debug;

#[derive(Debug)]
pub struct Findings {
  pub all: Vec<Box<dyn Finding>>,
}

impl Findings {
  pub fn new() -> Self {
    Self { all: vec![] }
  }
}

pub trait Finding: Debug {
  fn message(&self) -> &str;
  fn location(&self) -> Pos;
  fn trace(&self) -> &Trace;
  fn history(&self) -> Option<&ValueHistory>;
}

#[derive(Debug)]
pub struct NullDereferenceFinding {
  message: String,
  location: Pos,
  trace: Trace,
  history: Option<ValueHistory>,
}

impl Finding for NullDereferenceFinding {
  fn message(&self) -> &str {
    &self.message
  }

  fn location(&self) -> Pos {
    self.location.clone()
  }

  fn trace(&self) -> &Trace {
    &self.trace
  }

  fn history(&self) -> Option<&ValueHistory> {
    self.history.as_ref()
  }
}

impl NullDereferenceFinding {
  pub fn new(message: String, trace: Trace, history: Option<ValueHistory>) -> Self {
    Self {
      message,
      location: trace.current_pos().unwrap_or(Pos {
        file: "unknown".to_string(),
        line: 0,
      }),
      trace,
      history,
    }
  }
}
