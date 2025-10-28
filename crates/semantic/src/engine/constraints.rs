use rpds::HashTrieMap;

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
  preds: HashTrieMap<AValue, Constraint>,
}

impl Constraints {
  pub fn new() -> Self {
    Self {
      preds: HashTrieMap::new(),
    }
  }

  fn check(&self, val: &AValue, constraint: &Constraint) -> bool {
    if let Some(existing) = self.preds.get(val) {
      println!(
        "Checking constraint {:?} against existing {:?}",
        constraint, existing
      );
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
    println!("Adding constraint {:?} to value {:?}", constraint, val);
    if !self.check(&val, &constraint) {
      println!("Constraint {:?} is not satisfied by {:?}", constraint, val);
      return false;
    }
    self.preds = self.preds.insert(val, constraint);
    true
  }

  pub fn get_constraint(&self, val: &AValue) -> Option<&Constraint> {
    self.preds.get(val)
  }
}
