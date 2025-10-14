use std::fmt::Debug;

use crate::engine::{
  constraints::Constraint,
  domain::{AMem, AValue},
  history::ValueKind,
  reports::{Finding, FindingKind},
  trace::Trace,
};

#[derive(Debug)]
pub struct Triggers {
  pub triggers: Vec<Box<dyn Trigger>>,
}

pub enum Checked {
  Disarmed,
  OnHold,
  Fired(Box<Effect>),
}

pub type Effect = dyn FnMut(&mut AMem);

pub trait Trigger: Debug {
  fn name(&self) -> &str;
  fn trace(&self) -> &Trace;
  fn check(&self, memory: &AMem) -> Checked;
}

impl Triggers {
  pub fn new() -> Self {
    Self { triggers: vec![] }
  }
}

#[derive(Debug)]
pub struct NullTrigger {
  value: AValue,
  trace: Trace,
}

impl Trigger for NullTrigger {
  fn name(&self) -> &str {
    "NullTrigger"
  }

  fn trace(&self) -> &Trace {
    &self.trace
  }

  fn check(&self, memory: &AMem) -> Checked {
    println!("Checking NullTrigger for value: {}", self.value);
    let null_constraint = memory.constraints.get_constraint(&self.value);
    let is_null = match null_constraint {
      Some(Constraint::IsNull) => true,
      _ => false,
    };
    let trace = self.trace.clone();
    if is_null {
      Checked::Fired(Box::new(move |mem: &mut AMem| {
        mem.add_finding(Finding::new(
          FindingKind::NullDereference,
          trace.clone(),
          None,
        ));
      }))
    } else if let AValue::ANull { id } = self.value {
      let history = memory
        .history_registry
        .get_history(ValueKind::Null, id)
        .cloned();
      Checked::Fired(Box::new(move |mem: &mut AMem| {
        mem.add_finding(Finding::new(
          FindingKind::NullDereference,
          trace.clone(),
          history.clone(),
        ));
      }))
    } else if self.value.is_symbolic() {
      Checked::OnHold
    } else {
      Checked::Disarmed
    }
  }
}

impl NullTrigger {
  // TODO: should we use &Avalue instead?
  pub fn new(value: AValue, memory: &AMem) -> Self {
    Self {
      value,
      trace: memory.trace.clone(),
    }
  }
}
