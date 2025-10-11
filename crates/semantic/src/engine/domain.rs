use std::collections::{BTreeMap, HashMap};
use std::ops::{Add, Mul, Neg, Sub};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::engine::constraints::Constraints;
use crate::engine::findings::{Finding, Findings};
use crate::engine::history::HistoryRegistry;
use crate::engine::trace::Trace;
use crate::engine::triggers::{Checked, Trigger, Triggers};
use crate::il::{MethodSig, Type};
use std::fmt::{self, Display, Formatter};

use AValue::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AValue {
  AInt(i64),
  AString(String),
  ANull {
    id: u32,
  },
  ATop {
    id: u32,
  },
  ARef {
    location: Box<ALoc>,
    type_info: Option<Type>,
    history: Option<String>,
  },
  ASym(SExpr),
  AStruct {
    fields: BTreeMap<String, AValue>,
  },
}

impl AValue {
  pub fn top() -> Self {
    static TOP_ID_COUNTER: AtomicU32 = AtomicU32::new(1);
    let id = TOP_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    AValue::ATop { id }
  }
  pub fn null() -> Self {
    static NULL_ID_COUNTER: AtomicU32 = AtomicU32::new(1);
    let id = NULL_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    AValue::ANull { id }
  }
  pub fn is_unknown(&self) -> bool {
    match self {
      AValue::ATop { .. } => true,
      AValue::ASym(_) => true,
      _ => false,
    }
  }
  pub fn is_symbolic(&self) -> bool {
    matches!(self, AValue::ASym(_))
  }
  pub fn to_aloc(&self) -> Option<ALoc> {
    match self {
      AValue::ARef { location, .. } => Some((**location).clone()),
      _ => None,
    }
  }
  pub fn equals(&self, other: &AValue) -> AValue {
    match (self, other) {
      (AInt(a), AInt(b)) => AValue::AInt(if a == b { 1 } else { 0 }),
      (ATop { .. }, _) | (_, ATop { .. }) | (ASym(_), _) | (_, ASym(_)) => ASym(SExpr::SEquals(
        Box::new(self.clone()),
        Box::new(other.clone()),
      )),
      _ => AValue::top(),
    }
  }
  pub fn not_equals(&self, other: &AValue) -> AValue {
    match (self, other) {
      (AInt(a), AInt(b)) => AValue::AInt(if a != b { 1 } else { 0 }),
      (ATop { .. }, _) | (_, ATop { .. }) | (ASym(_), _) | (_, ASym(_)) => ASym(SExpr::SNotEquals(
        Box::new(self.clone()),
        Box::new(other.clone()),
      )),
      _ => AValue::top(),
    }
  }
  pub fn less(&self, other: &AValue) -> AValue {
    match (self, other) {
      (AInt(a), AInt(b)) => AValue::AInt(if a < b { 1 } else { 0 }),
      (ATop { .. }, _) | (_, ATop { .. }) | (ASym(_), _) | (_, ASym(_)) => ASym(SExpr::SLessThan(
        Box::new(self.clone()),
        Box::new(other.clone()),
      )),
      _ => AValue::top(),
    }
  }
  pub fn leq(&self, other: &AValue) -> AValue {
    match (self, other) {
      (AInt(a), AInt(b)) => AValue::AInt(if a <= b { 1 } else { 0 }),
      (ATop { .. }, _) | (_, ATop { .. }) | (ASym(_), _) | (_, ASym(_)) => ASym(
        SExpr::SLessThanOrEqual(Box::new(self.clone()), Box::new(other.clone())),
      ),
      _ => AValue::top(),
    }
  }
  pub fn greater(&self, other: &AValue) -> AValue {
    match (self, other) {
      (AInt(a), AInt(b)) => AValue::AInt(if a > b { 1 } else { 0 }),
      (ATop { .. }, _) | (_, ATop { .. }) | (ASym(_), _) | (_, ASym(_)) => ASym(
        SExpr::SGreaterThan(Box::new(self.clone()), Box::new(other.clone())),
      ),
      _ => AValue::top(),
    }
  }
  pub fn geq(&self, other: &AValue) -> AValue {
    match (self, other) {
      (AInt(a), AInt(b)) => AValue::AInt(if a >= b { 1 } else { 0 }),
      (ATop { .. }, _) | (_, ATop { .. }) | (ASym(_), _) | (_, ASym(_)) => ASym(
        SExpr::SGreaterThanOrEqual(Box::new(self.clone()), Box::new(other.clone())),
      ),
      _ => AValue::top(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ALoc {
  pub kind: ALocKind,
  pub path: Vec<Seg>,
}

impl ALoc {
  pub fn new_local(name: String) -> Self {
    ALoc {
      kind: ALocKind::ALocal(name),
      path: vec![],
    }
  }
  pub fn new_unknown(id: String) -> Self {
    ALoc {
      kind: ALocKind::AUnknown(id),
      path: vec![],
    }
  }
  pub fn new_param(name: String) -> Self {
    ALoc {
      kind: ALocKind::AParam(name),
      path: vec![],
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ALocKind {
  ALocal(String),
  AGlobal(String),
  AParam(String),
  AThis,
  AHeap(String),    // site_id
  AUnknown(String), // id
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Seg {
  Field(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SExpr {
  SStar(ALoc),
  SAdd(Box<AValue>, Box<AValue>),
  SSub(Box<AValue>, Box<AValue>),
  SMul(Box<AValue>, Box<AValue>),
  SNeg(Box<AValue>),
  SEquals(Box<AValue>, Box<AValue>),
  SNotEquals(Box<AValue>, Box<AValue>),
  SLessThan(Box<AValue>, Box<AValue>),
  SGreaterThan(Box<AValue>, Box<AValue>),
  SLessThanOrEqual(Box<AValue>, Box<AValue>),
  SGreaterThanOrEqual(Box<AValue>, Box<AValue>),
  SCond {
    condition: Box<AValue>,
    then_branch: Box<AValue>,
    else_branch: Box<AValue>,
  },
  SFieldAccess {
    base_value: Box<AValue>,
    field_id: String,
  },
  STop,
}

impl Add for AValue {
  type Output = AValue;

  fn add(self, other: AValue) -> AValue {
    use AValue::*;
    match (&self, &other) {
      (AInt(a), AInt(b)) => AInt(a + b),
      (ATop { .. }, _) | (_, ATop { .. }) | (ASym(_), _) | (_, ASym(_)) => {
        ASym(SExpr::SAdd(Box::new(self), Box::new(other)))
      }
      _ => AValue::top(),
    }
  }
}

impl Sub for AValue {
  type Output = AValue;

  fn sub(self, other: AValue) -> AValue {
    use AValue::*;
    match (&self, &other) {
      (AInt(a), AInt(b)) => AInt(a - b),
      (ATop { .. }, _) | (_, ATop { .. }) | (ASym(_), _) | (_, ASym(_)) => {
        ASym(SExpr::SSub(Box::new(self), Box::new(other)))
      }
      _ => AValue::top(),
    }
  }
}

impl Mul for AValue {
  type Output = AValue;

  fn mul(self, other: AValue) -> AValue {
    use AValue::*;
    match (&self, &other) {
      (AInt(a), AInt(b)) => AInt(a * b),
      (ATop { .. }, _) | (_, ATop { .. }) | (ASym(_), _) | (_, ASym(_)) => {
        ASym(SExpr::SMul(Box::new(self), Box::new(other)))
      }
      _ => AValue::top(),
    }
  }
}

impl Neg for AValue {
  type Output = AValue;

  fn neg(self) -> AValue {
    use AValue::*;
    match self {
      AInt(a) => AInt(-a),
      ATop { .. } => AValue::top(),
      ASym(SExpr::SNeg(expr)) => *expr,
      ASym(expr) => ASym(SExpr::SNeg(Box::new(ASym(expr)))),
      _ => AValue::top(),
    }
  }
}

#[derive(Debug)]
pub struct AMem {
  pub memory: HashMap<ALoc, AValue>,
  pub history_registry: HistoryRegistry,
  pub triggers: Triggers,
  pub constraints: Constraints,
  pub trace: Trace,
  pub findings: Findings,
}

impl AMem {
  pub fn new() -> Self {
    AMem {
      memory: HashMap::new(),
      history_registry: HistoryRegistry::new(),
      triggers: Triggers::new(),
      constraints: Constraints::new(),
      trace: Trace::new(),
      findings: Findings::new(),
    }
  }

  pub fn initialize(&mut self, sig: &MethodSig) {
    for (_ty, name) in &sig.params {
      if let Some(name) = name {
        let loc = ASym(SExpr::SStar(ALoc::new_param(name.clone())));
        self.memory.insert(ALoc::new_local(name.clone()), loc);
      }
    }
  }

  pub fn read(&self, loc: &ALoc) -> Option<&AValue> {
    self.memory.get(loc)
  }

  pub fn update(&mut self, loc: ALoc, value: AValue) {
    self.memory.insert(loc, value);
  }

  pub fn contains(&self, loc: &ALoc) -> bool {
    self.memory.contains_key(loc)
  }

  pub fn add_finding(&mut self, finding: Box<dyn Finding>) {
    self.findings.all.push(finding);
  }

  pub fn add_trigger(&mut self, trigger: Box<dyn Trigger>) {
    match trigger.check(self) {
      Checked::Fired(mut effect) => {
        println!("Trigger {} fired at trace: {}", trigger.name(), self.trace);
        effect(self);
      }
      Checked::Disarmed => return,
      Checked::OnHold => {
        self.triggers.triggers.push(trigger);
      }
    }
  }

  // TODO: add and check methods for constraints
}

impl Display for AValue {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      AValue::AInt(i) => write!(f, "{}", i),
      AValue::AString(s) => write!(f, "\"{}\"", s),
      AValue::ANull { id } => write!(f, "null#{}", id),
      AValue::ATop { id } => write!(f, "T#{}", id),
      AValue::ARef { location, .. } => write!(f, "&{}", location),
      AValue::ASym(sexpr) => write!(f, "{}", sexpr),
      AValue::AStruct { fields } => {
        write!(f, "{{ ")?;
        for (i, (key, value)) in fields.iter().enumerate() {
          if i > 0 {
            write!(f, ", ")?;
          }
          write!(f, "{}: {}", key, value)?;
        }
        write!(f, " }}")
      }
    }
  }
}

impl Display for ALoc {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.kind)?;
    for seg in &self.path {
      write!(f, "{}", seg)?;
    }
    Ok(())
  }
}

impl Display for ALocKind {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      ALocKind::ALocal(s) => write!(f, "local({})", s),
      ALocKind::AGlobal(s) => write!(f, "global({})", s),
      ALocKind::AParam(s) => write!(f, "param({})", s),
      ALocKind::AThis => write!(f, "this"),
      ALocKind::AHeap(s) => write!(f, "heap({})", s),
      ALocKind::AUnknown(s) => write!(f, "unknown({})", s),
    }
  }
}

impl Display for Seg {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      Seg::Field(s) => write!(f, ".{}", s),
    }
  }
}

impl Display for SExpr {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      SExpr::SStar(loc) => write!(f, "*{}", loc),
      SExpr::SAdd(a, b) => write!(f, "({} + {})", a, b),
      SExpr::SSub(a, b) => write!(f, "({} - {})", a, b),
      SExpr::SMul(a, b) => write!(f, "({} * {})", a, b),
      SExpr::SNeg(a) => write!(f, "-({})", a),
      SExpr::SEquals(a, b) => write!(f, "({} == {})", a, b),
      SExpr::SNotEquals(a, b) => write!(f, "({} != {})", a, b),
      SExpr::SLessThan(a, b) => write!(f, "({} < {})", a, b),
      SExpr::SGreaterThan(a, b) => write!(f, "({} > {})", a, b),
      SExpr::SLessThanOrEqual(a, b) => write!(f, "({} <= {})", a, b),
      SExpr::SGreaterThanOrEqual(a, b) => write!(f, "({} >= {})", a, b),
      SExpr::SCond {
        condition,
        then_branch,
        else_branch,
      } => write!(
        f,
        "if {} then {} else {}",
        condition, then_branch, else_branch
      ),
      SExpr::SFieldAccess {
        base_value,
        field_id,
      } => write!(f, "{}.{}", base_value, field_id),
      SExpr::STop => write!(f, "S_T"),
    }
  }
}

impl Display for AMem {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    writeln!(f, "---- AMem State ----")?;
    writeln!(f, "Triggers: {:?}", self.triggers)?;

    writeln!(f, "Memory:")?;
    let mut sorted_memory: Vec<_> = self.memory.iter().collect();
    // Note: Sorting is based on the Debug representation for deterministic output.
    // This might be slow for very large memory maps.
    sorted_memory.sort_by_key(|(k, _)| format!("{:?}", k));
    for (loc, val) in sorted_memory {
      writeln!(f, "  {} -> {}", loc, val)?;
    }

    writeln!(f, "Constraints: {:?}", self.constraints)?;
    writeln!(f, "Trace:\n{}", self.trace)?;
    writeln!(f, "Findings: {:?}", self.findings)?;
    writeln!(f, "--------------------")
  }
}

impl Default for AMem {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_aint_add() {
    let a = AValue::AInt(5);
    let b = AValue::AInt(3);
    assert_eq!(a + b, AValue::AInt(8));
  }

  #[test]
  fn test_aint_add_top() {
    let a = AValue::AInt(5);
    let b = AValue::top();
    let result = a + b;
    match result {
      AValue::ASym(SExpr::SAdd(left, right)) => {
        assert_eq!(*left, AValue::AInt(5));
        assert!(matches!(*right, AValue::ATop { .. }));
      }
      _ => panic!("Expected ASym(SAdd) but got {:?}", result),
    }
  }

  #[test]
  fn test_aint_add_sym() {
    let a = AValue::AInt(5);
    let b = AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())));
    let result = a + b;
    match result {
      AValue::ASym(SExpr::SAdd(left, right)) => {
        assert_eq!(*left, AValue::AInt(5));
        assert_eq!(
          *right,
          AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())))
        );
      }
      _ => panic!("Expected ASym(SAdd) but got {:?}", result),
    }
  }

  #[test]
  fn test_top_add_sym() {
    let a = AValue::top();
    let b = AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())));
    let result = a + b;
    match result {
      AValue::ASym(SExpr::SAdd(left, right)) => {
        assert!(matches!(*left, AValue::ATop { .. }));
        assert_eq!(
          *right,
          AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())))
        );
      }
      _ => panic!("Expected ASym(SAdd) but got {:?}", result),
    }
  }

  #[test]
  fn test_sym_add_sym() {
    let a = AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())));
    let b = AValue::ASym(SExpr::SStar(ALoc::new_local("y".to_string())));
    let result = a + b;
    match result {
      AValue::ASym(SExpr::SAdd(left, right)) => {
        assert_eq!(
          *left,
          AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())))
        );
        assert_eq!(
          *right,
          AValue::ASym(SExpr::SStar(ALoc::new_local("y".to_string())))
        );
      }
      _ => panic!("Expected ASym(SAdd) but got {:?}", result),
    }
  }

  #[test]
  fn test_other_combinations() {
    let a = AValue::AString("hello".to_string());
    let b = AValue::ANull { id: 1 };
    let result = a + b;
    assert!(matches!(result, AValue::ATop { .. }));
  }

  // Subtraction tests
  #[test]
  fn test_aint_sub() {
    let a = AValue::AInt(5);
    let b = AValue::AInt(3);
    assert_eq!(a - b, AValue::AInt(2));
  }

  #[test]
  fn test_aint_sub_top() {
    let a = AValue::AInt(5);
    let b = AValue::top();
    let result = a - b;
    match result {
      AValue::ASym(SExpr::SSub(left, right)) => {
        assert_eq!(*left, AValue::AInt(5));
        assert!(matches!(*right, AValue::ATop { .. }));
      }
      _ => panic!("Expected ASym(SSub) but got {:?}", result),
    }
  }

  #[test]
  fn test_aint_sub_sym() {
    let a = AValue::AInt(5);
    let b = AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())));
    let result = a - b;
    match result {
      AValue::ASym(SExpr::SSub(left, right)) => {
        assert_eq!(*left, AValue::AInt(5));
        assert_eq!(
          *right,
          AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())))
        );
      }
      _ => panic!("Expected ASym(SSub) but got {:?}", result),
    }
  }

  #[test]
  fn test_top_sub_sym() {
    let a = AValue::top();
    let b = AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())));
    let result = a - b;
    match result {
      AValue::ASym(SExpr::SSub(left, right)) => {
        assert!(matches!(*left, AValue::ATop { .. }));
        assert_eq!(
          *right,
          AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())))
        );
      }
      _ => panic!("Expected ASym(SSub) but got {:?}", result),
    }
  }

  #[test]
  fn test_sym_sub_sym() {
    let a = AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())));
    let b = AValue::ASym(SExpr::SStar(ALoc::new_local("y".to_string())));
    let result = a - b;
    match result {
      AValue::ASym(SExpr::SSub(left, right)) => {
        assert_eq!(
          *left,
          AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())))
        );
        assert_eq!(
          *right,
          AValue::ASym(SExpr::SStar(ALoc::new_local("y".to_string())))
        );
      }
      _ => panic!("Expected ASym(SSub) but got {:?}", result),
    }
  }

  #[test]
  fn test_sub_other_combinations() {
    let a = AValue::AString("hello".to_string());
    let b = AValue::ANull { id: 1 };
    let result = a - b;
    assert!(matches!(result, AValue::ATop { .. }));
  }

  // Multiplication tests
  #[test]
  fn test_aint_mul() {
    let a = AValue::AInt(5);
    let b = AValue::AInt(3);
    assert_eq!(a * b, AValue::AInt(15));
  }

  #[test]
  fn test_aint_mul_top() {
    let a = AValue::AInt(5);
    let b = AValue::top();
    let result = a * b;
    match result {
      AValue::ASym(SExpr::SMul(left, right)) => {
        assert_eq!(*left, AValue::AInt(5));
        assert!(matches!(*right, AValue::ATop { .. }));
      }
      _ => panic!("Expected ASym(SMul) but got {:?}", result),
    }
  }

  #[test]
  fn test_aint_mul_sym() {
    let a = AValue::AInt(5);
    let b = AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())));
    let result = a * b;
    match result {
      AValue::ASym(SExpr::SMul(left, right)) => {
        assert_eq!(*left, AValue::AInt(5));
        assert_eq!(
          *right,
          AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())))
        );
      }
      _ => panic!("Expected ASym(SMul) but got {:?}", result),
    }
  }

  #[test]
  fn test_top_mul_sym() {
    let a = AValue::top();
    let b = AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())));
    let result = a * b;
    match result {
      AValue::ASym(SExpr::SMul(left, right)) => {
        assert!(matches!(*left, AValue::ATop { .. }));
        assert_eq!(
          *right,
          AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())))
        );
      }
      _ => panic!("Expected ASym(SMul) but got {:?}", result),
    }
  }

  #[test]
  fn test_sym_mul_sym() {
    let a = AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())));
    let b = AValue::ASym(SExpr::SStar(ALoc::new_local("y".to_string())));
    let result = a * b;
    match result {
      AValue::ASym(SExpr::SMul(left, right)) => {
        assert_eq!(
          *left,
          AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())))
        );
        assert_eq!(
          *right,
          AValue::ASym(SExpr::SStar(ALoc::new_local("y".to_string())))
        );
      }
      _ => panic!("Expected ASym(SMul) but got {:?}", result),
    }
  }

  #[test]
  fn test_mul_other_combinations() {
    let a = AValue::AString("hello".to_string());
    let b = AValue::ANull { id: 1 };
    let result = a * b;
    assert!(matches!(result, AValue::ATop { .. }));
  }

  // Negation tests
  #[test]
  fn test_aint_neg() {
    let a = AValue::AInt(5);
    assert_eq!(-a, AValue::AInt(-5));
  }

  #[test]
  fn test_aint_neg_zero() {
    let a = AValue::AInt(0);
    assert_eq!(-a, AValue::AInt(0));
  }

  #[test]
  fn test_aint_neg_negative() {
    let a = AValue::AInt(-5);
    assert_eq!(-a, AValue::AInt(5));
  }

  #[test]
  fn test_sym_neg() {
    let a = AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())));
    let result = -a;
    match result {
      AValue::ASym(SExpr::SNeg(expr)) => {
        assert_eq!(
          *expr,
          AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())))
        );
      }
      _ => panic!("Expected ASym(SNeg) but got {:?}", result),
    }
  }

  #[test]
  fn test_double_neg() {
    let a = AValue::ASym(SExpr::SStar(ALoc::new_local("x".to_string())));
    let neg_a = -a;
    let result = -neg_a;
    match result {
      AValue::ASym(SExpr::SStar(loc)) => {
        assert_eq!(loc, ALoc::new_local("x".to_string()));
      }
      _ => panic!("Expected ASym(SStar) but got {:?}", result),
    }
  }

  #[test]
  fn test_neg_top() {
    let a = AValue::top();
    let result = -a;
    assert!(matches!(result, AValue::ATop { .. }));
  }

  #[test]
  fn test_neg_other_combinations() {
    let a = AValue::AString("hello".to_string());
    let result = -a;
    assert!(matches!(result, AValue::ATop { .. }));
  }
}
