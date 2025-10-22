use crate::il::ValueKind;

use super::syntax::{
  BinaryOp, DefineKind, Expr, InvokeKind, MethodSig, Program, Statement, StatementValue, Type,
  UnaryOp, Value,
};

pub struct PrettyPrinter;

impl PrettyPrinter {
  pub fn print_type(t: &Type) -> String {
    match t {
      Type::Byte => "byte".to_string(),
      Type::UByte => "ubyte".to_string(),
      Type::Short => "short".to_string(),
      Type::UShort => "ushort".to_string(),
      Type::Int => "int".to_string(),
      Type::UInt => "uint".to_string(),
      Type::Long => "long".to_string(),
      Type::ULong => "ulong".to_string(),
      Type::Float => "float".to_string(),
      Type::Double => "double".to_string(),
      Type::Named { name, generics } => {
        let type_params = if generics.is_empty() {
          String::new()
        } else {
          format!(
            "<{}>",
            generics
              .iter()
              .map(Self::print_type)
              .collect::<Vec<_>>()
              .join(", ")
          )
        };
        format!("{}{}", name, type_params)
      }
      Type::Null => "null".to_string(),
      Type::String => "string".to_string(),
      Type::Array(elem_type, size) => {
        let size_str = size.map_or("?".to_string(), |s| s.to_string());
        format!("{}[{}]", Self::print_type(elem_type), size_str)
      }
      Type::Slice(elem_type) => format!("{}[]", Self::print_type(elem_type)),
      Type::Pointer(elem_type) => format!("{}*", Self::print_type(elem_type)),
      Type::Function { params, ret } => {
        let params_str = params
          .iter()
          .map(Self::print_type)
          .collect::<Vec<_>>()
          .join(", ");
        let return_str = ret
          .iter()
          .map(Self::print_type)
          .collect::<Vec<_>>()
          .join(", ");
        format!("({}) -> {}", params_str, return_str)
      }
      Type::Bool => "boolean".to_string(),
      Type::Any => "any".to_string(),
      Type::Void => "void".to_string(),
    }
  }

  pub fn print_binary_op(op: &BinaryOp) -> &'static str {
    match op {
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
    }
  }

  pub fn print_unary_op(op: &UnaryOp) -> &'static str {
    match op {
      UnaryOp::Neg => "-",
      UnaryOp::Not => "!",
      UnaryOp::And => "&",
      UnaryOp::Mul => "*",
      UnaryOp::Add => "+",
      UnaryOp::Sub => "-",
      UnaryOp::LtSub => "<-",
      UnaryOp::BitXor => "^",
    }
  }

  pub fn print_expr(expr: &Expr) -> String {
    match expr {
      Expr::BinOp { left, right, op } => {
        format!(
          "({} {} {})",
          Self::print_value(left),
          Self::print_binary_op(op),
          Self::print_value(right)
        )
      }
      Expr::UnOp { value, op } => {
        format!("{}{}", Self::print_unary_op(op), Self::print_value(value))
      }
      Expr::New { t } => {
        format!("new {}", Self::print_type(t))
      }
      Expr::Deref { value } => {
        format!("*{}", Self::print_value(value))
      }
      Expr::DotAccess { base, field } => {
        format!("({}).{}", Self::print_value(base), field)
      }
      Expr::Composite { elements } => {
        let elems_str = elements
          .iter()
          .map(|(k, v)| match k {
            Some(key) => format!("{}: {}", key, Self::print_value(v)),
            None => Self::print_value(v),
          })
          .collect::<Vec<_>>()
          .join(", ");
        format!("{{{}}}", elems_str)
      }
      Expr::InstanceOf { value, t } => {
        format!(
          "{} instanceof {}",
          Self::print_value(value),
          Self::print_type(t)
        )
      }
    }
  }

  pub fn print_value(value: &Value) -> String {
    match &value.kind {
      ValueKind::NullLit => "null".to_string(),
      ValueKind::Ident(name) => {
        let type_str = match &value.extra.type_declared {
          Some(t) => format!(": {}", Self::print_type(t)),
          None => String::new(),
        };
        format!("{}{}", name, type_str)
      }
      ValueKind::IntLit(n) => n.to_string(),
      ValueKind::LongLit(n) => format!("{}L", n),
      ValueKind::FloatLit(n) => format!("{}f", n),
      ValueKind::DoubleLit(n) => n.to_string(),
      ValueKind::StringLit(s) => format!("\"{}\"", s),
      ValueKind::TypeLit(t) => format!("{}.type", Self::print_type(t)),
      ValueKind::CompositeLit(t, val) => {
        format!("{}{}", Self::print_type(t), Self::print_value(val))
      }
      ValueKind::Exp(expr) => Self::print_expr(expr),
    }
  }

  pub fn print_method_sig(sig: &MethodSig) -> String {
    let params = sig
      .params
      .iter()
      .map(|(t, name)| match name {
        Some(n) => format!("{}: {}", n, Self::print_type(t)),
        None => Self::print_type(t),
      })
      .collect::<Vec<_>>()
      .join(", ");
    format!("{}({})", sig.name, params)
  }

  pub fn print_invoke_kind(kind: &InvokeKind) -> String {
    match kind {
      InvokeKind::Static => "static".to_string(),
      InvokeKind::Virtual { base } => {
        format!("virtual {}", Self::print_value(base))
      }
      InvokeKind::Interface { base } => {
        format!("interface {}", Self::print_value(base))
      }
    }
  }

  pub fn print_define_kind(kind: &DefineKind) -> String {
    match kind {
      DefineKind::Class {
        name,
        super_class,
        interfaces,
        body,
      } => {
        let super_str = super_class
          .as_ref()
          .map(|s| format!(" extends {}", s))
          .unwrap_or_default();
        let interfaces_str = if interfaces.is_empty() {
          String::new()
        } else {
          format!(" implements {}", interfaces.join(", "))
        };
        let body_str = if body.is_empty() {
          String::new()
        } else {
          format!(
            " {{\n{}\n}}",
            body
              .iter()
              .enumerate()
              .map(|(i, stmt)| Self::print_statement(stmt, i, 1))
              .collect::<Vec<_>>()
              .join("\n")
          )
        };
        format!("class {}{}{}{}", name, super_str, interfaces_str, body_str)
      }
      DefineKind::Interface { name, extends } => {
        let extends_str = extends
          .as_ref()
          .map(|s| format!(" extends {}", s))
          .unwrap_or_default();
        format!("interface {}{}", name, extends_str)
      }
      DefineKind::Method {
        sig,
        body,
        ret_type,
      } => {
        let body_str = if body.is_empty() {
          String::new()
        } else {
          format!(
            " {{\n{}\n}}",
            body
              .iter()
              .enumerate()
              .map(|(i, stmt)| Self::print_statement(stmt, i, 1))
              .collect::<Vec<_>>()
              .join("\n")
          )
        };
        let ret_str = ret_type
          .iter()
          .map(Self::print_type)
          .collect::<Vec<_>>()
          .join(", ");
        format!(
          "method {}: {}{}",
          Self::print_method_sig(sig),
          ret_str,
          body_str
        )
      }
      DefineKind::Field { name, init, t } => {
        let init_str = init
          .as_ref()
          .map(|v| format!(" = {}", Self::print_value(v)))
          .unwrap_or_default();
        let t_str = t
          .as_ref()
          .map(|t| format!(": {}", Self::print_type(t)))
          .unwrap_or_default();
        format!("field {}{}{}", name, t_str, init_str)
      }
      DefineKind::Var { name, init, t } => {
        let init_str = init
          .as_ref()
          .map(|v| format!(" = {}", Self::print_value(v)))
          .unwrap_or_default();
        let t_str = t
          .as_ref()
          .map(|t| format!(": {}", Self::print_type(t)))
          .unwrap_or_default();
        format!("var {}{}{}", name, t_str, init_str)
      }
      DefineKind::Constructor { sig, body } => {
        let body_str = if body.is_empty() {
          String::new()
        } else {
          format!(
            " {{\n{}\n}}",
            body
              .iter()
              .enumerate()
              .map(|(i, stmt)| Self::print_statement(stmt, i, 1))
              .collect::<Vec<_>>()
              .join("\n")
          )
        };
        format!("constructor {}{}", Self::print_method_sig(sig), body_str)
      }
      DefineKind::Package(name) => {
        format!("package {}", name)
      }
    }
  }

  pub fn print_statement_value(sv: &StatementValue) -> String {
    let stmts = sv
      .statements
      .iter()
      .enumerate()
      .map(|(id, stmt)| Self::print_statement(stmt, id, 0))
      .collect::<Vec<_>>()
      .join(",");
    let value = match &sv.result {
      Some(value) => Self::print_value(&value),
      None => String::new(),
    };
    format!("{{{};{}}}", stmts, value)
  }

  pub fn print_statement(stmt: &Statement, id: usize, indent: usize) -> String {
    let indent_str = "  ".repeat(indent);
    let stmt_str = match stmt {
      Statement::Assign {
        left,
        right,
        tag: _tag,
      } => {
        format!(
          "{}{}: {} = {}",
          indent_str,
          id,
          Self::print_value(left),
          Self::print_value(right)
        )
      }
      Statement::Invoke {
        kind,
        callee,
        args,
        ret_type,
        ret_loc,
        tag: _,
      } => {
        let args_str = args
          .iter()
          .map(|arg| Self::print_value(arg))
          .collect::<Vec<_>>()
          .join(", ");
        let ret_str = ret_loc
          .as_ref()
          .map(|v| format!("{} = ", Self::print_value(v),))
          .unwrap_or_default();
        format!(
          "{}{}: {}{} {}({}) : {}",
          indent_str,
          id,
          ret_str,
          Self::print_invoke_kind(kind),
          Self::print_value(callee),
          args_str,
          Self::print_type(ret_type)
        )
      }
      Statement::If {
        condition,
        then_stmts,
        else_stmts,
        tag: _,
      } => {
        let mut result = format!(
          "{}{}: if {} then {{\n",
          indent_str,
          id,
          Self::print_value(condition)
        );
        for (i, stmt) in then_stmts.iter().enumerate() {
          result.push_str(&format!("{}\n", Self::print_statement(stmt, i, indent + 1)));
        }
        result.push_str(&format!("{}}}", indent_str));
        if !else_stmts.is_empty() {
          result.push_str(" else {\n");
          for (i, stmt) in else_stmts.iter().enumerate() {
            result.push_str(&format!("{}\n", Self::print_statement(stmt, i, indent + 1)));
          }
          result.push_str(&format!("{}}}", indent_str));
        }
        result
      }
      Statement::For {
        init,
        condition,
        update,
        body,
        tag: _,
      } => {
        let init_str = if init.is_empty() {
          String::new()
        } else {
          init
            .iter()
            .map(|stmt| Self::print_statement(stmt, 0, indent + 1))
            .collect::<Vec<_>>()
            .join(", ")
        };
        let update_str = if update.is_empty() {
          String::new()
        } else {
          update
            .iter()
            .map(|stmt| Self::print_statement(stmt, 0, indent + 1))
            .collect::<Vec<_>>()
            .join(", ")
        };
        let body_str = body
          .iter()
          .enumerate()
          .map(|(i, stmt)| Self::print_statement(stmt, i, indent + 1))
          .collect::<Vec<_>>()
          .join("\n");
        format!(
          "{}{}: for {};{};{} {{\n{}\n{}}}",
          indent_str,
          id,
          init_str,
          Self::print_statement_value(condition),
          update_str,
          body_str,
          indent_str
        )
      }
      Statement::DoWhile {
        body,
        condition,
        tag: _,
      } => {
        let body_str = body
          .iter()
          .enumerate()
          .map(|(i, stmt)| Self::print_statement(stmt, i, indent + 1))
          .collect::<Vec<_>>()
          .join("\n");
        format!(
          "{}{}: do {{\n{}\n{}}} while {}",
          indent_str,
          id,
          body_str,
          indent_str,
          Self::print_statement_value(condition),
        )
      }
      Statement::Return { value, tag: _ } => match value {
        Some(v) => format!("{}{}: return {}", indent_str, id, Self::print_value(v)),
        None => format!("{}{}: return", indent_str, id),
      },
      Statement::Goto { target } => format!("{}{}: goto {}", indent_str, id, target),
      Statement::Define { kind, .. } => {
        format!(
          "{}{}: define {}",
          indent_str,
          id,
          Self::print_define_kind(kind),
        )
      }
      Statement::Break => format!("{}{}: break", indent_str, id),
      Statement::Continue => format!("{}{}: continue", indent_str, id),
      Statement::Switch {
        value,
        cases,
        default,
        tag: _,
      } => {
        let cases_str = cases
          .iter()
          .map(|(value, stmts)| {
            format!(
              "case {}: {{\n{}\n}}",
              Self::print_statement_value(value),
              stmts
                .iter()
                .map(|stmt| Self::print_statement(stmt, 0, 1))
                .collect::<Vec<_>>()
                .join("\n")
            )
          })
          .collect::<Vec<_>>()
          .join(", ");
        let default_str = default
          .as_ref()
          .map(|stmts| {
            format!(
              "default: {{\n{}\n}}",
              stmts
                .iter()
                .map(|stmt| Self::print_statement(stmt, 0, 1))
                .collect::<Vec<_>>()
                .join("\n")
            )
          })
          .unwrap_or_default();
        format!(
          "{}{}: switch {} {{\n{}\n{}\n}}",
          indent_str,
          id,
          Self::print_statement_value(value),
          cases_str,
          default_str
        )
      }
    };
    stmt_str
  }

  pub fn print_program(program: &Program) -> String {
    program
      .statements
      .iter()
      .enumerate()
      .map(|(id, stmt)| Self::print_statement(stmt, id, 0))
      .collect::<Vec<_>>()
      .join("\n")
  }
}
