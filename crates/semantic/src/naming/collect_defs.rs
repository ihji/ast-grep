use crate::{
  il::{DefineKind, MethodSig, Program, Statement},
  naming::{
    naming_context::{DeclKind, Namespace, ScopeKind, Visibility},
    NamingContext, ScopeId,
  },
};

pub fn collect_pgm(ctx: &mut NamingContext, pgm: &mut Program) {
  ctx.clear_scope_stack();
  let pgm_scope_id = ctx.enter_scope(ScopeKind::File, pgm.path.to_str());
  pgm.scope_id = Some(pgm_scope_id);
  for stmt in &mut pgm.statements {
    collect_stmt(ctx, stmt);
  }
  ctx.exit_scope();
}

fn collect_stmts_in_scope(
  ctx: &mut NamingContext,
  stmts: &mut Vec<Statement>,
  scope_kind: ScopeKind,
  name: Option<&str>,
) -> ScopeId {
  let new_scope_id = ctx.enter_scope(scope_kind, name);
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
      DefineKind::Class { name, body, .. } => {
        *scope_id = Some(collect_stmts_in_scope(
          ctx,
          body,
          ScopeKind::Module,
          Some(name),
        ));
      }
      DefineKind::Method {
        sig: MethodSig { name, .. },
        body,
        ..
      } => {
        *scope_id = Some(collect_stmts_in_scope(
          ctx,
          body,
          ScopeKind::Function,
          Some(name),
        ));
      }
      DefineKind::Package(_) => {}
      DefineKind::Constructor { body, .. } => {
        *scope_id = Some(collect_stmts_in_scope(ctx, body, ScopeKind::Function, None));
      }
      DefineKind::Interface { .. } => {}
      DefineKind::Field { .. } => {}
      DefineKind::Var { name, t, .. } => {
        ctx.register_symbol(
          name,
          Namespace::Value,
          DeclKind::Var(t.clone()),
          Visibility::Private,
          tag.clone(),
        );
      }
    },
    _ => {}
  }
}
