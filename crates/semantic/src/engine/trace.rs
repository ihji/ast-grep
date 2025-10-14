use std::{
  collections::HashMap,
  fmt::{Display, Formatter},
  sync::RwLock,
};

use crate::engine::{
  context::SessionCtx,
  history::{HistoryEvent, ValueHistory},
};

// --------------------------- //
//       Trace Arena Core      //
// --------------------------- //

pub type TraceIdx = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceNode {
  pub prev: Option<TraceIdx>,
  pub event: TraceEvent,
}

#[derive(Debug, Default)]
pub struct TraceArena {
  storage: RwLock<Vec<TraceNode>>,
}

impl TraceArena {
  pub fn new() -> Self {
    Self {
      storage: RwLock::new(Vec::new()),
    }
  }

  /// Append-only push.
  pub fn push(&self, node: TraceNode) -> TraceIdx {
    let mut w = self.storage.write().expect("trace arena poisoned");
    let idx = w.len() as TraceIdx;
    w.push(node);
    idx
  }

  pub fn get(&self, idx: TraceIdx) -> Option<TraceNode> {
    let r = self.storage.read().expect("trace arena poisoned");
    r.get(idx as usize).cloned()
  }

  pub fn collect_forward(&self, tail: Option<TraceIdx>) -> Vec<TraceEvent> {
    self.collect_forward_with_history(tail, None)
  }

  pub fn collect_forward_with_history(
    &self,
    tail: Option<TraceIdx>,
    history: Option<&ValueHistory>,
  ) -> Vec<TraceEvent> {
    let mut history_map: HashMap<TraceIdx, Vec<TraceEvent>> = HashMap::new();
    let mut none_idx_history: Vec<TraceEvent> = Vec::new();
    if let Some(history) = history {
      if let Some(creation) = &history.creation {
        if let Some(idx) = creation.trace_event_idx {
          history_map
            .entry(idx)
            .or_insert_with(Vec::new)
            .push(TraceEvent::History {
              event: creation.clone(),
            });
        } else {
          none_idx_history.push(TraceEvent::History {
            event: creation.clone(),
          });
        }
      }
      for event in &history.events {
        if let Some(idx) = event.trace_event_idx {
          history_map
            .entry(idx)
            .or_insert_with(Vec::new)
            .push(TraceEvent::History {
              event: event.clone(),
            });
        } else {
          none_idx_history.push(TraceEvent::History {
            event: event.clone(),
          });
        }
      }
    }
    let mut acc = Vec::new();
    let mut cur = tail;
    let r = self.storage.read().expect("trace arena poisoned");
    while let Some(i) = cur {
      if let Some(history_events) = history_map.get(&i) {
        let mut reversed_events = history_events.clone();
        reversed_events.reverse();
        acc.extend(reversed_events);
      }
      if let Some(node) = r.get(i as usize) {
        acc.push(node.event.clone());
        cur = node.prev;
      } else {
        break;
      }
    }
    let mut reversed_none_idx_history = none_idx_history.clone();
    reversed_none_idx_history.reverse();
    acc.extend(reversed_none_idx_history);
    acc.reverse();
    acc
  }
}

// --------------------------- //
//         Trace Types         //
// --------------------------- //

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
  current: Option<Pos>,
  tail: Option<TraceIdx>,
}

pub struct TraceFormatter<'a> {
  trace: &'a Trace,
  history: Option<&'a ValueHistory>,
  ctx: &'a SessionCtx<'a>,
}

impl<'a> Display for TraceFormatter<'a> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    if let Some(pos) = &self.trace.current {
      writeln!(f, "[trace @ {}]", pos)?;
    } else {
      writeln!(f, "[trace @ unknown location]")?;
    }
    for ev in self
      .ctx
      .trace_arena
      .collect_forward_with_history(self.trace.tail, self.history)
      .into_iter()
    {
      writeln!(f, "  {}", ev.fmt_with(&self.ctx.trace_arena))?;
    }
    Ok(())
  }
}

impl Trace {
  pub fn new() -> Self {
    Self {
      current: None,
      tail: None,
    }
  }

  pub fn tail(&self) -> Option<TraceIdx> {
    self.tail
  }

  pub fn current_pos(&self) -> Option<Pos> {
    self.current.clone()
  }

  pub fn update_pos(&mut self, context: &SessionCtx, tag: &Option<usize>) {
    let line = tag.and_then(|t| context.get_line(t)).unwrap_or(0);
    self.current = Some(Pos {
      file: context.file_path.clone(),
      line,
    });
  }

  pub fn events_forward(&self, arena: &TraceArena) -> Vec<TraceEvent> {
    arena.collect_forward(self.tail)
  }

  pub fn fmt_with<'a>(
    &'a self,
    ctx: &'a SessionCtx<'a>,
    history: Option<&'a ValueHistory>,
  ) -> TraceFormatter<'a> {
    TraceFormatter {
      trace: self,
      ctx,
      history,
    }
  }

  pub fn add_branch(
    &mut self,
    context: &SessionCtx,
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
    let node = TraceNode {
      prev: self.tail,
      event,
    };
    let idx = context.trace_arena.push(node);
    self.tail = Some(idx);
  }

  pub fn add_assume(&mut self, context: &SessionCtx, tag: &Option<usize>, condition: String) {
    let line = tag.and_then(|t| context.get_line(t)).unwrap_or(0);
    let event = TraceEvent::Assumed {
      condition,
      at: Pos {
        file: context.file_path.clone(),
        line,
      },
    };
    let node = TraceNode {
      prev: self.tail,
      event,
    };
    let idx = context.trace_arena.push(node);
    self.tail = Some(idx);
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pos {
  pub file: String,
  pub line: usize,
}

impl Default for Pos {
  fn default() -> Self {
    Self {
      file: "unknown".to_string(),
      line: 0,
    }
  }
}

impl Display for Pos {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{}:{}", self.file, self.line)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ctx {
  cid: usize,
  callsite: Option<Pos>,
  events: Vec<TraceEvent>,
}

impl Ctx {
  fn new() -> Self {
    Self {
      cid: 0,
      callsite: None,
      events: vec![],
    }
  }
  fn events(&self) -> &Vec<TraceEvent> {
    &self.events
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceEvent {
  BranchTaken { taken: bool, next: Pos, at: Pos },
  Assumed { condition: String, at: Pos },
  Invocation { ctx: Ctx, at: Pos },
  History { event: HistoryEvent },
}

impl TraceEvent {
  pub fn fmt_with<'a>(&'a self, arena: &'a TraceArena) -> TraceEventFormatter<'a> {
    TraceEventFormatter { event: self, arena }
  }
}

pub struct TraceEventFormatter<'a> {
  event: &'a TraceEvent,
  arena: &'a TraceArena,
}

impl<'a> Display for TraceEventFormatter<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self.event {
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
      TraceEvent::Invocation { ctx, at } => {
        write!(f, "Function invoked at {}:{}:\n", at.file, at.line)?;
        ctx
          .events()
          .iter()
          .try_for_each(|e| writeln!(f, "  {}", e.fmt_with(self.arena)))
      }
      TraceEvent::History { event } => {
        write!(f, "Event {}", event)
      }
    }
  }
}
