use crate::{
  il::{DefineKind, Program, Statement},
  naming::{
    naming_context::{DeclKind, Namespace, ScopeKind, Visibility},
    NamingContext, ScopeId,
  },
};

pub fn collect_pgm(ctx: &mut NamingContext, pgm: &mut Program) {
  ctx.clear_scope_stack();
  ctx.enter_scope(ScopeKind::File);
  for stmt in &mut pgm.statements {
    collect_stmt(ctx, stmt);
  }
  ctx.exit_scope();
}

fn collect_stmts_in_scope(
  ctx: &mut NamingContext,
  stmts: &mut Vec<Statement>,
  scope_kind: ScopeKind,
) -> ScopeId {
  let new_scope_id = ctx.enter_scope(scope_kind);
  for stmt in stmts {
    collect_stmt(ctx, stmt);
  }
  ctx.exit_scope();
  new_scope_id
}

fn collect_stmt(ctx: &mut NamingContext, stmt: &mut Statement) {
  match stmt {
    Statement::Define {
      kind,
      scope_id,
      tag,
    } => match kind {
      DefineKind::Class { body, .. } => {
        *scope_id = Some(collect_stmts_in_scope(ctx, body, ScopeKind::Module));
      }
      DefineKind::Method { body, .. } => {
        *scope_id = Some(collect_stmts_in_scope(ctx, body, ScopeKind::Function));
      }
      DefineKind::Package(_) => {}
      DefineKind::Constructor { body, .. } => {
        *scope_id = Some(collect_stmts_in_scope(ctx, body, ScopeKind::Function));
      }
      DefineKind::Interface { .. } => {}
      DefineKind::Field { .. } => {}
      DefineKind::Var { name, .. } => {
        ctx.register_symbol(
          name,
          Namespace::Value,
          DeclKind::Var,
          Visibility::Private,
          tag.clone(),
        );
      }
    },
    _ => {}
  }
}
