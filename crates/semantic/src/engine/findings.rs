use crate::engine::trace::{Pos, Trace};
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
}

#[derive(Debug)]
pub struct NullDereferenceFinding {
  message: String,
  location: Pos,
  trace: Trace,
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
}

impl NullDereferenceFinding {
  pub fn new(message: String, trace: Trace) -> Self {
    Self {
      message,
      location: trace.current_pos().unwrap_or(Pos {
        file: "unknown".to_string(),
        line: 0,
      }),
      trace,
    }
  }
}
