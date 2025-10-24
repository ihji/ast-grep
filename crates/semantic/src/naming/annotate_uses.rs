use crate::{
  il,
  naming::{
    naming_context::{DeclKind, Namespace},
    NamingContext, ScopeId,
  },
};

pub fn annotate_pgm(ctx: &mut NamingContext, pgm: &mut il::Program) {
  for stmt in &mut pgm.statements {
    annotate_stmt(ctx, pgm.scope_id, stmt);
  }
  ctx.exit_scope();
}

fn annotate_stmt(
  ctx: &mut NamingContext,
  parent_scope_id: Option<ScopeId>,
  stmt: &mut il::Statement,
) {
  match stmt {
    il::Statement::Define { kind, scope_id, .. } => match kind {
      il::DefineKind::Class { body, .. } => {
        for stmt in body {
          annotate_stmt(ctx, *scope_id, stmt);
        }
      }
      il::DefineKind::Method { body, .. } => {
        for stmt in body {
          annotate_stmt(ctx, *scope_id, stmt);
        }
      }
      il::DefineKind::Constructor { body, .. } => {
        for stmt in body {
          annotate_stmt(ctx, *scope_id, stmt);
        }
      }
      _ => {}
    },
    il::Statement::Assign { left, right, .. } => {
      annotate_value(ctx, parent_scope_id, left, Namespace::Value);
      annotate_value(ctx, parent_scope_id, right, Namespace::Value);
    }
    il::Statement::Invoke {
      callee,
      args,
      ret_loc,
      ..
    } => {
      annotate_value(ctx, parent_scope_id, callee, Namespace::Method);
      for arg in args {
        annotate_value(ctx, parent_scope_id, arg, Namespace::Value);
      }
      if let Some(ret_loc) = ret_loc {
        annotate_value(ctx, parent_scope_id, ret_loc, Namespace::Value);
      }
    }
    _ => {}
  }
}

fn annotate_value(
  ctx: &mut NamingContext,
  parent_scope_id: Option<ScopeId>,
  value: &mut il::Value,
  namespace: Namespace,
) {
  match &mut value.kind {
    il::ValueKind::Ident(name) => {
      if let Some(symbol) = ctx.lookup_symbol(name, namespace, parent_scope_id) {
        value.extra.symbol_id = Some(symbol.id);
        match &symbol.kind {
          DeclKind::Var(t) => {
            value.extra.type_declared = t.clone();
          }
          _ => {}
        }
      }
    }
    il::ValueKind::Exp(e) => annotate_expr(ctx, parent_scope_id, e),
    _ => {}
  }
}

fn annotate_expr(ctx: &mut NamingContext, parent_scope_id: Option<ScopeId>, expr: &mut il::Expr) {
  match expr {
    il::Expr::UnOp { value, .. } => {
      annotate_value(ctx, parent_scope_id, value, Namespace::Value);
    }
    il::Expr::BinOp { left, right, .. } => {
      annotate_value(ctx, parent_scope_id, left, Namespace::Value);
      annotate_value(ctx, parent_scope_id, right, Namespace::Value);
    }
    il::Expr::DotAccess { base, .. } => {
      annotate_value(ctx, parent_scope_id, base, Namespace::Value);
    }
    _ => {}
  }
}
