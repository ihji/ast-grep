use std::fmt::Display;

use rpds::HashTrieMap;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEvent {
  pub kind: HistoryEventKind,
  pub location: Option<Pos>,
  pub description: String,
  pub trace_event_idx: Option<u64>, // Link to the trace event
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
  histories: HashTrieMap<ValueId, ValueHistory>,
}

impl Display for HistoryEvent {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{:?} at {}: {}",
      self.kind,
      self.location.as_ref().unwrap_or(&Pos::default()),
      self.description
    )
  }
}

impl Display for HistoryRegistry {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    for (value_id, history) in &self.histories {
      writeln!(f, "History for {:?} (ID: {}):", value_id.kind, value_id.id)?;
      if let Some(creation) = &history.creation {
        writeln!(f, "  {}", creation)?;
      } else {
        writeln!(f, "  Creation event not recorded.")?;
      }
      for event in &history.events {
        writeln!(f, "  {}", event)?;
      }
    }
    Ok(())
  }
}

impl HistoryRegistry {
  pub fn new() -> Self {
    HistoryRegistry {
      histories: HashTrieMap::new(),
    }
  }

  pub fn retain_only(&mut self, keep: &std::collections::HashSet<ValueId>) {
    let mut new_histories = self.histories.clone();
    for (key, _) in &self.histories {
      if !keep.contains(&key) {
        new_histories = new_histories.remove(key);
      }
    }
    self.histories = new_histories;
  }

  fn register_creation(
    &mut self,
    kind: ValueKind,
    id: u32,
    location: Option<Pos>,
    description: String,
    trace_idx: Option<u64>,
  ) {
    let value_id = ValueId { kind, id };
    let event = HistoryEvent {
      kind: HistoryEventKind::Creation,
      location,
      description,
      trace_event_idx: trace_idx,
    };

    let history = match self.histories.get_mut(&value_id) {
      Some(history) => history,
      None => {
        let new_history = ValueHistory::new();
        self.histories = self.histories.insert(value_id, new_history);
        self.histories.get_mut(&value_id).unwrap()
      }
    };
    history.add_creation_event(event);
  }

  fn register_event(
    &mut self,
    kind: ValueKind,
    id: u32,
    event_kind: HistoryEventKind,
    location: Option<Pos>,
    description: String,
    trace_idx: Option<u64>,
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
    self.register_creation(
      ValueKind::Null,
      null_id,
      trace.current_pos(),
      format!("Null created: {}", code_snippet),
      trace.tail(),
    );
  }

  pub fn track_null_assignment(&mut self, null_id: u32, trace: &Trace, code_snippet: String) {
    self.register_event(
      ValueKind::Null,
      null_id,
      HistoryEventKind::Assignment,
      trace.current_pos(),
      format!("Null assigned: {}", code_snippet),
      trace.tail(),
    );
  }

  pub fn track_null_dereference(&mut self, null_id: u32, trace: &Trace, code_snippet: String) {
    self.register_event(
      ValueKind::Null,
      null_id,
      HistoryEventKind::Dereference,
      trace.current_pos(),
      format!("Null dereferenced: {}", code_snippet),
      trace.tail(),
    );
  }
}
