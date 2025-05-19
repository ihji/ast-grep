use std::collections::HashMap;

use crate::engine::{domain::AValue, interval::Interval};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constraint {
  InInterval(Interval),
  IsNull,
  IsNotNull,
  IsEqual(AValue),
  IsNotEqual(AValue),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Constraints {
  preds: HashMap<AValue, Constraint>,
}

impl Constraints {
  pub fn new() -> Self {
    Self {
      preds: HashMap::new(),
    }
  }

  fn check(&self, val: &AValue, constraint: &Constraint) -> bool {
    if let Some(existing) = self.preds.get(val) {
      match (existing, constraint) {
        (Constraint::IsNull, Constraint::IsNotNull)
        | (Constraint::IsNotNull, Constraint::IsNull) => false,
        (Constraint::IsEqual(v1), Constraint::IsNotEqual(v2))
        | (Constraint::IsNotEqual(v2), Constraint::IsEqual(v1))
          if v1 == v2 =>
        {
          false
        }
        (Constraint::IsEqual(v1), Constraint::IsEqual(v2)) if v1 != v2 => false,
        (Constraint::InInterval(i1), Constraint::InInterval(i2)) => !i1.intersection(i2).is_empty(),
        _ => true,
      }
    } else {
      true
    }
  }

  pub fn add_constraint(&mut self, val: AValue, constraint: Constraint) -> bool {
    if !self.check(&val, &constraint) {
      return false;
    }
    self.preds.insert(val, constraint);
    true
  }

  pub fn get_constraint(&self, val: &AValue) -> Option<&Constraint> {
    self.preds.get(val)
  }
}
