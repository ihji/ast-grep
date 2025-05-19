use std::fmt::Display;

use crate::engine::context::Context;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
  current: Option<Pos>,
  ctx: Ctx,
}

impl Display for Trace {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}", self.ctx)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ctx {
  cid: usize,
  callsite: Option<Pos>,
  events: Vec<TraceEvent>,
}

impl Display for Ctx {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    writeln!(
      f,
      "[{}] Call at {}",
      self.cid,
      if let Some(pos) = &self.callsite {
        format!("{}:{}", pos.file, pos.line)
      } else {
        "unknown location".to_string()
      }
    )?;
    for event in &self.events {
      writeln!(f, "  {}", event)?;
    }
    Ok(())
  }
}

impl Ctx {
  fn new() -> Self {
    Self {
      cid: 0,
      callsite: None,
      events: vec![],
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pos {
  pub file: String,
  pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceEvent {
  BranchTaken { taken: bool, next: Pos, at: Pos },
  Assumed { condition: String, at: Pos },
}

impl Display for TraceEvent {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      TraceEvent::BranchTaken { taken, next, at } => {
        write!(
          f,
          "Branch at {}:{} was taken {} to {}",
          at.file,
          at.line,
          if *taken { "True" } else { "False" },
          if next.line == 0 {
            "unknown location".to_string()
          } else {
            format!("{}:{}", next.file, next.line)
          }
        )
      }
      TraceEvent::Assumed { condition, at } => {
        write!(f, "Assumed '{}' at {}:{}", condition, at.file, at.line)
      }
    }
  }
}

impl Trace {
  pub fn new() -> Self {
    Self {
      current: None,
      ctx: Ctx::new(),
    }
  }

  pub fn current_pos(&self) -> Option<Pos> {
    self.current.clone()
  }

  pub fn update_pos(&mut self, context: &Context, tag: &Option<usize>) {
    let line = tag.and_then(|t| context.get_line(t)).unwrap_or(0);
    self.current = Some(Pos {
      file: context.file_path.clone(),
      line,
    });
  }

  pub fn add_branch(
    &mut self,
    context: &Context,
    next_tag: &Option<usize>,
    tag: &Option<usize>,
    taken: bool,
  ) {
    let next_line = next_tag.and_then(|t| context.get_line(t)).unwrap_or(0);
    let line = tag.and_then(|t| context.get_line(t)).unwrap_or(0);
    let event = TraceEvent::BranchTaken {
      taken,
      next: Pos {
        file: context.file_path.clone(),
        line: next_line,
      },
      at: Pos {
        file: context.file_path.clone(),
        line,
      },
    };
    self.ctx.events.push(event);
  }

  pub fn add_assume(&mut self, context: &Context, tag: &Option<usize>, condition: String) {
    let line = tag.and_then(|t| context.get_line(t)).unwrap_or(0);
    let event = TraceEvent::Assumed {
      condition,
      at: Pos {
        file: context.file_path.clone(),
        line,
      },
    };
    self.ctx.events.push(event);
  }
}
