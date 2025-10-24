use derive_visitor::Drive;

use crate::naming::{ScopeId, SymbolId};

use super::pretty_print::PrettyPrinter;
use std::{fmt, path::PathBuf};

#[derive(Debug, Clone, Drive, PartialEq, Eq, Hash)]
pub enum Type {
  Byte,
  UByte,
  Short,
  UShort,
  Int,
  UInt,
  Long,
  ULong,
  Float,
  Double,
  Named { name: String, generics: Vec<Type> },
  Null,
  String,
  Array(Box<Type>, Option<usize>),
  Slice(Box<Type>),
  Pointer(Box<Type>),
  Function { params: Vec<Type>, ret: Vec<Type> },
  Bool,
  Any,
  Void,
}

#[derive(Debug, Clone, Drive, PartialEq, Default)]
pub struct ValueExtra {
  pub type_declared: Option<Type>,
  pub type_inferred: Option<Type>,
  pub symbol_id: Option<SymbolId>,
}

impl ValueExtra {
  pub fn new(type_declared: Option<Type>, type_inferred: Option<Type>) -> Self {
    Self {
      type_declared,
      type_inferred,
      symbol_id: None,
    }
  }
}

#[derive(Debug, Clone, Drive, PartialEq)]
pub struct Value {
  pub kind: ValueKind,
  pub extra: ValueExtra,
  pub tag: Option<usize>,
}

#[derive(Debug, Clone, Drive, PartialEq)]
pub enum ValueKind {
  NullLit,
  Ident(String),
  IntLit(i32),
  LongLit(i64),
  FloatLit(f32),
  DoubleLit(f64),
  StringLit(String),
  TypeLit(Type),
  CompositeLit(Type, Box<Value>),
  Exp(Expr),
}

#[derive(Debug, Clone, Drive, PartialEq, Eq)]
pub enum BinaryOp {
  Add,
  Sub,
  Mul,
  Div,
  And,
  AndAnd,
  Or,
  OrOr,
  Xor,
  AndXor,
  Shl,
  Shr,
  Ushr,
  Mod,
  Eq,
  Neq,
  Lt,
  Lte,
  Gt,
  Gte,
}

#[derive(Debug, Clone, Drive, PartialEq, Eq)]
pub enum UnaryOp {
  Neg,
  Not,
  And,
  Mul,
  Add,
  Sub,
  LtSub,
  BitXor,
}

#[derive(Debug, Clone, Drive, PartialEq)]
pub enum CompositeItem {
  KV(Value, Value),
  Value(Value),
}

#[derive(Debug, Clone, Drive, PartialEq)]
pub enum Expr {
  BinOp {
    left: Box<Value>,
    right: Box<Value>,
    op: BinaryOp,
  },
  UnOp {
    value: Box<Value>,
    op: UnaryOp,
  },
  Composite {
    elements: Vec<CompositeItem>,
  },
  New {
    t: Type,
  },
  Deref {
    value: Box<Value>,
  },
  DotAccess {
    base: Box<Value>,
    field: String,
  },
  InstanceOf {
    value: Box<Value>,
    t: Type,
  },
}

impl Value {
  pub fn new(kind: ValueKind) -> Self {
    Self {
      kind,
      extra: ValueExtra::default(),
      tag: None,
    }
  }

  pub fn with_extra(kind: ValueKind, extra: ValueExtra, tag: Option<usize>) -> Self {
    Self { kind, extra, tag }
  }

  pub fn get_type(&self) -> Option<&Type> {
    self
      .extra
      .type_inferred
      .as_ref()
      .or(self.extra.type_declared.as_ref())
  }
}

impl ValueKind {
  pub fn logical_not(&self) -> Option<ValueKind> {
    match self {
      ValueKind::Exp(Expr::BinOp { left, right, op }) => {
        if let Some(neg_op) = op.logical_not() {
          Some(ValueKind::Exp(Expr::BinOp {
            left: left.clone(),
            right: right.clone(),
            op: neg_op,
          }))
        } else {
          None
        }
      }
      ValueKind::Exp(Expr::UnOp { value, op }) if op.is_negation() => Some(value.kind.clone()),
      _ => None,
    }
  }
}

impl BinaryOp {
  pub fn logical_not(&self) -> Option<BinaryOp> {
    match self {
      BinaryOp::Eq => Some(BinaryOp::Neq),
      BinaryOp::Neq => Some(BinaryOp::Eq),
      BinaryOp::Lt => Some(BinaryOp::Gte),
      BinaryOp::Lte => Some(BinaryOp::Gt),
      BinaryOp::Gt => Some(BinaryOp::Lte),
      BinaryOp::Gte => Some(BinaryOp::Lt),
      _ => None,
    }
  }
}

impl UnaryOp {
  pub fn is_negation(&self) -> bool {
    matches!(self, UnaryOp::Not)
  }
}

#[derive(Debug, Clone, Drive, PartialEq)]
pub enum InvokeKind {
  Static,
  Virtual { base: Box<Value> },
  Interface { base: Box<Value> },
}

#[derive(Debug, Clone, Drive, PartialEq, Eq, Hash)]
pub struct MethodSig {
  pub name: String,
  pub params: Vec<(Type, Option<String>)>,
  pub id: Option<SymbolId>,
}

#[derive(Debug, Clone, Drive, PartialEq)]
pub enum DefineKind {
  Class {
    name: String,
    super_class: Option<String>,
    interfaces: Vec<String>,
    body: Vec<Statement>,
  },
  Interface {
    name: String,
    extends: Option<String>,
  },
  Method {
    sig: MethodSig,
    body: Vec<Statement>,
    ret_type: Vec<Type>,
  },
  Field {
    name: String,
    init: Option<Value>,
    t: Option<Type>,
  },
  Var {
    name: String,
    init: Option<Value>,
    t: Option<Type>,
  },
  Constructor {
    sig: MethodSig,
    body: Vec<Statement>,
  },
  Package(String),
}

#[derive(Debug, Clone, Drive, PartialEq)]
pub struct StatementValue {
  pub statements: Vec<Statement>,
  pub result: Option<Value>,
}

#[derive(Debug, Clone, Drive, PartialEq)]
pub enum Statement {
  Assign {
    left: Value,
    right: Value,
    tag: Option<usize>,
  },
  Invoke {
    kind: InvokeKind,
    callee: Value,
    args: Vec<Value>,
    ret_type: Type,
    ret_loc: Option<Value>,
    tag: Option<usize>,
  },
  If {
    condition: Value,
    then_stmts: Vec<Statement>,
    else_stmts: Vec<Statement>,
    tag: Option<usize>,
  },
  For {
    init: Vec<Statement>,
    condition: StatementValue,
    update: Vec<Statement>,
    body: Vec<Statement>,
    tag: Option<usize>,
  },
  DoWhile {
    body: Vec<Statement>,
    condition: StatementValue,
    tag: Option<usize>,
  },
  Switch {
    value: StatementValue,
    cases: Vec<(StatementValue, Vec<Statement>)>,
    default: Option<Vec<Statement>>,
    tag: Option<usize>,
  },
  Return {
    value: Option<Value>,
    tag: Option<usize>,
  },
  Goto {
    target: usize,
  },
  Define {
    kind: DefineKind,
    scope_id: Option<ScopeId>,
    tag: Option<usize>,
  },
  Break,
  Continue,
}

#[derive(Debug, Clone, Drive)]
pub struct Program {
  pub statements: Vec<Statement>,
  #[drive(skip)]
  pub path: PathBuf,
  pub scope_id: Option<ScopeId>,
}

impl Program {
  pub fn new(path: PathBuf) -> Self {
    Self {
      statements: Vec::new(),
      path,
      scope_id: None,
    }
  }

  pub fn add_statements(&mut self, stmts: Vec<Statement>) {
    self.statements.extend(stmts)
  }

  pub fn add_statement(&mut self, stmt: Statement) {
    self.statements.push(stmt)
  }
}

impl fmt::Display for Program {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", PrettyPrinter::print_program(self))
  }
}

impl MethodSig {
  pub fn from_path(path: &PathBuf) -> Self {
    MethodSig {
      name: path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown_method")
        .to_string(),
      params: Vec::new(),
      id: None,
    }
  }
}

impl fmt::Display for Value {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.kind)
  }
}

impl fmt::Display for ValueKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ValueKind::NullLit => write!(f, "null"),
      ValueKind::Ident(name) => write!(f, "{}", name),
      ValueKind::IntLit(i) => write!(f, "{}", i),
      ValueKind::LongLit(l) => write!(f, "{}", l),
      ValueKind::FloatLit(fl) => write!(f, "{}", fl),
      ValueKind::DoubleLit(d) => write!(f, "{}", d),
      ValueKind::StringLit(s) => write!(f, "\"{}\"", s),
      ValueKind::TypeLit(t) => write!(f, "{}", t),
      ValueKind::CompositeLit(t, e) => write!(f, "{}{}", t, e),
      ValueKind::Exp(e) => write!(f, "{}", e),
    }
  }
}

impl fmt::Display for Type {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Type::Byte => write!(f, "byte"),
      Type::UByte => write!(f, "ubyte"),
      Type::Short => write!(f, "short"),
      Type::UShort => write!(f, "ushort"),
      Type::Int => write!(f, "int"),
      Type::UInt => write!(f, "uint"),
      Type::Long => write!(f, "long"),
      Type::ULong => write!(f, "ulong"),
      Type::Float => write!(f, "float"),
      Type::Double => write!(f, "double"),
      Type::Named { name, generics } => {
        write!(f, "{}", name)?;
        if !generics.is_empty() {
          write!(f, "<")?;
          for (i, t) in generics.iter().enumerate() {
            if i > 0 {
              write!(f, ", ")?;
            }
            write!(f, "{}", t)?;
          }
          write!(f, ">")?;
        }
        Ok(())
      }
      Type::Null => write!(f, "null"),
      Type::String => write!(f, "string"),
      Type::Array(t, size) => {
        write!(f, "{}", t)?;
        if let Some(s) = size {
          write!(f, "[{}]", s)
        } else {
          write!(f, "[]")
        }
      }
      Type::Slice(t) => write!(f, "{}[]", t),
      Type::Pointer(t) => write!(f, "*{}", t),
      Type::Function { params, ret } => {
        write!(f, "(")?;
        for (i, t) in params.iter().enumerate() {
          if i > 0 {
            write!(f, ", ")?;
          }
          write!(f, "{}", t)?;
        }
        write!(f, ") -> ")?;
        if ret.len() > 1 {
          write!(f, "(")?;
        }
        for (i, t) in ret.iter().enumerate() {
          if i > 0 {
            write!(f, ", ")?;
          }
          write!(f, "{}", t)?;
        }
        if ret.len() > 1 {
          write!(f, ")")?;
        }
        Ok(())
      }
      Type::Bool => write!(f, "bool"),
      Type::Any => write!(f, "any"),
      Type::Void => write!(f, "void"),
    }
  }
}

impl fmt::Display for Expr {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Expr::BinOp { left, right, op } => write!(f, "{} {} {}", left, op, right),
      Expr::UnOp { value, op } => write!(f, "{}{}", op, value),
      Expr::Composite { elements } => {
        write!(f, "{{ ")?;
        for (i, element) in elements.iter().enumerate() {
          if i > 0 {
            write!(f, ", ")?;
          }
          match element {
            CompositeItem::KV(key, value) => write!(f, "{}: {}", key, value)?,
            CompositeItem::Value(value) => write!(f, "{}", value)?,
          }
        }
        write!(f, " }}")
      }
      Expr::New { t } => write!(f, "new {}", t),
      Expr::Deref { value } => write!(f, "*{}", value),
      Expr::DotAccess { base, field } => write!(f, "{}.{}", base, field),
      Expr::InstanceOf { value, t } => write!(f, "{} instanceof {}", value, t),
    }
  }
}

impl fmt::Display for BinaryOp {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let s = match self {
      BinaryOp::Add => "+",
      BinaryOp::Sub => "-",
      BinaryOp::Mul => "*",
      BinaryOp::Div => "/",
      BinaryOp::And => "&",
      BinaryOp::AndAnd => "&&",
      BinaryOp::Or => "|",
      BinaryOp::OrOr => "||",
      BinaryOp::Xor => "^",
      BinaryOp::AndXor => "&^",
      BinaryOp::Shl => "<<",
      BinaryOp::Shr => ">>",
      BinaryOp::Ushr => ">>>",
      BinaryOp::Mod => "%",
      BinaryOp::Eq => "==",
      BinaryOp::Neq => "!=",
      BinaryOp::Lt => "<",
      BinaryOp::Lte => "<=",
      BinaryOp::Gt => ">",
      BinaryOp::Gte => ">=",
    };
    write!(f, "{}", s)
  }
}

impl fmt::Display for UnaryOp {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let s = match self {
      UnaryOp::Neg => "-",
      UnaryOp::Not => "!",
      UnaryOp::And => "&",
      UnaryOp::Mul => "*",
      UnaryOp::Add => "+",
      UnaryOp::Sub => "-",
      UnaryOp::LtSub => "<-",
      UnaryOp::BitXor => "^",
    };
    write!(f, "{}", s)
  }
}
