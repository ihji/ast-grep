use std::collections::HashMap;

use crate::engine::trace::{Pos, Trace};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId {
  pub kind: ValueKind,
  pub id: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValueKind {
  Null,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoryEventKind {
  Creation,
  Assignment,
  ParameterPassing,
  ReturnValue,
  FieldStore,
  FieldLoad,
  Dereference,
}

#[derive(Debug, Clone)]
pub struct HistoryEvent {
  pub kind: HistoryEventKind,
  pub location: Option<Pos>,
  pub description: String,
  pub trace_event_idx: Option<usize>, // Link to the trace event
}

#[derive(Debug, Clone, Default)]
pub struct ValueHistory {
  pub creation: Option<HistoryEvent>,
  pub events: Vec<HistoryEvent>,
}

impl ValueHistory {
  pub fn new() -> Self {
    ValueHistory {
      creation: None,
      events: Vec::new(),
    }
  }

  pub fn add_creation_event(&mut self, event: HistoryEvent) {
    self.creation = Some(event);
  }

  pub fn add_event(&mut self, event: HistoryEvent) {
    self.events.push(event);
  }
}

#[derive(Debug, Default)]
pub struct HistoryRegistry {
  histories: HashMap<ValueId, ValueHistory>,
}

impl HistoryRegistry {
  pub fn new() -> Self {
    HistoryRegistry {
      histories: HashMap::new(),
    }
  }

  fn register_creation(
    &mut self,
    kind: ValueKind,
    id: u32,
    location: Option<Pos>,
    description: String,
    trace_idx: Option<usize>,
  ) {
    let value_id = ValueId { kind, id };
    let event = HistoryEvent {
      kind: HistoryEventKind::Creation,
      location,
      description,
      trace_event_idx: trace_idx,
    };

    let history = self.histories.entry(value_id).or_default();
    history.add_creation_event(event);
  }

  fn register_event(
    &mut self,
    kind: ValueKind,
    id: u32,
    event_kind: HistoryEventKind,
    location: Option<Pos>,
    description: String,
    trace_idx: Option<usize>,
  ) {
    let value_id = ValueId { kind, id };
    let event = HistoryEvent {
      kind: event_kind,
      location,
      description,
      trace_event_idx: trace_idx,
    };

    if let Some(history) = self.histories.get_mut(&value_id) {
      history.add_event(event);
    }
  }

  pub fn get_history(&self, kind: ValueKind, id: u32) -> Option<&ValueHistory> {
    self.histories.get(&ValueId { kind, id })
  }

  pub fn track_null_creation(&mut self, null_id: u32, trace: &Trace, code_snippet: String) {
    let trace_idx = trace.events().len().checked_sub(1);
    self.register_creation(
      ValueKind::Null,
      null_id,
      trace.current_pos(),
      format!("Null created: {}", code_snippet),
      trace_idx,
    );
  }

  pub fn track_null_assignment(&mut self, null_id: u32, trace: &Trace, code_snippet: String) {
    let trace_idx = trace.events().len().checked_sub(1);
    self.register_event(
      ValueKind::Null,
      null_id,
      HistoryEventKind::Assignment,
      trace.current_pos(),
      format!("Null assigned: {}", code_snippet),
      trace_idx,
    );
  }
}
