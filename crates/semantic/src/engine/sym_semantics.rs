use ast_grep_core::matcher::MatcherExt;

use crate::engine::constraints::Constraint;
use crate::engine::context::Context;
use crate::engine::domain::AValue::*;
use crate::engine::domain::{ALoc, AMem, AValue, SExpr};
use crate::engine::interval::Interval;
use crate::engine::path_explorer::PathExplorer;
use crate::engine::triggers::NullTrigger;
use crate::il::{BasicBlock, CfgStatement, ValueKind};
use crate::il::{BinaryOp, Expr, UnaryOp, Value};

fn eval_expr(memory: &mut AMem, expr: &Expr) -> AValue {
  match expr {
    Expr::BinOp { op, left, right } => {
      let left_val = eval(memory, left);
      let right_val = eval(memory, right);
      match op {
        BinaryOp::Add => left_val + right_val,
        BinaryOp::Sub => left_val - right_val,
        BinaryOp::Mul => left_val * right_val,
        BinaryOp::Div => AValue::top(),
        BinaryOp::Eq => left_val.equals(&right_val),
        BinaryOp::Neq => left_val.not_equals(&right_val),
        BinaryOp::Lt => left_val.less(&right_val),
        BinaryOp::Lte => left_val.leq(&right_val),
        BinaryOp::Gt => left_val.greater(&right_val),
        BinaryOp::Gte => left_val.geq(&right_val),
        _ => AValue::top(),
      }
    }
    Expr::UnOp { value, op } => {
      let value_val = eval(memory, value);
      match op {
        UnaryOp::Neg => -value_val,
        _ => AValue::top(),
      }
    }
    Expr::Deref { value } => {
      let ptr_val = eval(memory, value);
      let nt = NullTrigger::new(ptr_val.clone(), memory);
      memory.add_trigger(Box::new(nt));
      let loc_opt = ptr_val.to_aloc();
      if let Some(loc) = loc_opt {
        memory.read(&loc).cloned().unwrap_or(AValue::top())
      } else {
        AValue::top()
      }
    }
    _ => AValue::top(),
  }
}

fn eval(memory: &mut AMem, value: &Value) -> AValue {
  match &value.kind {
    ValueKind::IntLit(x) => AValue::AInt((*x).into()),
    ValueKind::NullLit => AValue::null(),
    ValueKind::Exp(expr) => eval_expr(memory, expr),
    ValueKind::Var { .. } => {
      let loc = eval_loc(memory, value);
      memory.read(&loc).unwrap_or(&AValue::top()).clone()
    }
    _ => AValue::top(),
  }
}

fn eval_loc(_memory: &AMem, loc: &Value) -> ALoc {
  match &loc.kind {
    ValueKind::Var { name, t: _ } => ALoc::new_local(name.clone()),
    _ => ALoc::new_unknown("".to_string()),
  }
}

pub fn transfer_stmt(
  context: &Context,
  path_explorer: &mut impl PathExplorer,
  stmt: &CfgStatement,
  memory: &mut AMem,
) {
  memory.trace.update_pos(context, &stmt.get_tag());
  match stmt {
    CfgStatement::Assign {
      left,
      right,
      tag: _tag,
    } => {
      let loc = eval_loc(memory, left);
      let val = eval(memory, right);
      if let AValue::ANull { id } = &val {
        if let Value {
          kind: ValueKind::NullLit,
          ..
        } = right
        {
          memory
            .history_registry
            .track_null_creation(*id, &memory.trace, format!("{}", right));
        }
        memory
          .history_registry
          .track_null_assignment(*id, &memory.trace, format!("{}", left));
      }
      memory.update(loc, val);
    }
    CfgStatement::Invoke {
      kind: _,
      callee: _,
      args: _,
      ret_type: _,
      ret_loc: _,
      tag,
    } => {
      if let Some(tag) = tag {
        let node = context.source_info.get(*tag);
        if let Some(node) = node {
          let node = context.root.adopt(*node);
          let pattern = ast_grep_core::Pattern::new("foo()", ast_grep_language::SupportLang::Go);
          if let Some(matched) = pattern.find_node(node) {
            println!(
              "Matched function call: {}, {}",
              matched.text(),
              matched.start_pos().line() + 1
            );
          }
        }
      }
    }
    CfgStatement::Assume { value, tag } => {
      let tag = tag.or(value.tag);
      memory.trace.add_assume(context, &tag, format!("{}", value));
      let v = eval(memory, value);
      if let AInt(0) = v {
        println!("Assumption is false, path ends here: {:?}", memory.trace);
        path_explorer.mark_done();
        return;
      }
      let constr_opt = match &v {
        ASym(SExpr::SEquals(lv, rv)) => match (&**lv, &**rv) {
          (AInt(x), _) if rv.is_unknown() => {
            Some((rv, Constraint::InInterval(Interval::constant(*x))))
          }
          (_, AInt(y)) if lv.is_unknown() => {
            Some((lv, Constraint::InInterval(Interval::constant(*y))))
          }
          (ANull { .. }, _) if rv.is_unknown() => Some((rv, Constraint::IsNull)),
          (_, ANull { .. }) if lv.is_unknown() => Some((lv, Constraint::IsNull)),
          _ => None,
        },
        ASym(SExpr::SNotEquals(lv, rv)) => match (&**lv, &**rv) {
          (AInt(x), _) if rv.is_unknown() => Some((rv, Constraint::IsNotEqual(AValue::AInt(*x)))),
          (_, AInt(y)) if lv.is_unknown() => Some((lv, Constraint::IsNotEqual(AValue::AInt(*y)))),
          (ANull { .. }, _) if rv.is_unknown() => Some((rv, Constraint::IsNotNull)),
          (_, ANull { .. }) if lv.is_unknown() => Some((lv, Constraint::IsNotNull)),
          _ => None,
        },
        ASym(SExpr::SLessThan(lv, rv)) => match (&**lv, &**rv) {
          (AInt(x), _) if rv.is_unknown() => {
            Some((rv, Constraint::InInterval(Interval::create_gt(*x))))
          }
          (_, AInt(y)) if lv.is_unknown() => {
            Some((lv, Constraint::InInterval(Interval::create_lt(*y))))
          }
          _ => None,
        },
        ASym(SExpr::SLessThanOrEqual(lv, rv)) => match (&**lv, &**rv) {
          (AInt(x), _) if rv.is_unknown() => {
            Some((rv, Constraint::InInterval(Interval::create_gte(*x))))
          }
          (_, AInt(y)) if lv.is_unknown() => {
            Some((lv, Constraint::InInterval(Interval::create_lte(*y))))
          }
          _ => None,
        },
        ASym(SExpr::SGreaterThan(lv, rv)) => match (&**lv, &**rv) {
          (AInt(x), _) if rv.is_unknown() => {
            Some((rv, Constraint::InInterval(Interval::create_lt(*x))))
          }
          (_, AInt(y)) if lv.is_unknown() => {
            Some((lv, Constraint::InInterval(Interval::create_gt(*y))))
          }
          _ => None,
        },
        ASym(SExpr::SGreaterThanOrEqual(lv, rv)) => match (&**lv, &**rv) {
          (AInt(x), _) if rv.is_unknown() => {
            Some((rv, Constraint::InInterval(Interval::create_lte(*x))))
          }
          (_, AInt(y)) if lv.is_unknown() => {
            Some((lv, Constraint::InInterval(Interval::create_gte(*y))))
          }
          _ => None,
        },
        _ => None,
      };
      if let Some((val, constr)) = constr_opt {
        let check_result = memory.constraints.add_constraint((**val).clone(), constr);
        if !check_result {
          path_explorer.mark_done();
        }
      }
    }
    CfgStatement::Return { value: _, .. } => (),
    CfgStatement::Break => (),
    CfgStatement::Nop { .. } => (),
  }
}

pub fn transfer_block(
  context: &Context,
  path_explorer: &mut impl PathExplorer,
  block: &BasicBlock,
  memory: &mut AMem,
) {
  for stmt in &block.stmts {
    transfer_stmt(context, path_explorer, stmt, memory);
  }
}
