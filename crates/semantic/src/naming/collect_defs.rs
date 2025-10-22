use crate::{
  il::{DefineKind, Program, Statement},
  naming::{naming_context::ScopeKind, NamingContext},
};

pub fn collect_pgm(ctx: &mut NamingContext, pgm: &mut Program) {
  ctx.clear_scope_stack();
  ctx.enter_scope(ScopeKind::File);
  for stmt in &mut pgm.statements {
    collect_stmt(ctx, stmt);
  }
  ctx.exit_scope();
}

fn collect_stmt(ctx: &mut NamingContext, stmt: &mut Statement) {
  match stmt {
    Statement::Define { kind, scope_id } => {
      let scope_kind = match kind {
        DefineKind::Class { .. } => ScopeKind::Module,
        DefineKind::Method { .. } => ScopeKind::Function,
        DefineKind::Package(_) => ScopeKind::Package,
        DefineKind::Constructor { .. } => ScopeKind::Function,
        DefineKind::Interface { .. } => ScopeKind::Module,
        DefineKind::Field { .. } => ScopeKind::Block,
      };
      let new_scope_id = ctx.enter_scope(scope_kind);
      *scope_id = Some(new_scope_id);
      let inner_stmts = match kind {
        DefineKind::Class { body, .. } => body,
        DefineKind::Method { body, .. } => body,
        DefineKind::Constructor { body, .. } => body,
        _ => &mut vec![],
      };
      for inner_stmt in inner_stmts {
        collect_stmt(ctx, inner_stmt);
      }
      ctx.exit_scope();
    }
    _ => {}
  }
}
