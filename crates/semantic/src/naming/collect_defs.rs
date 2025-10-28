use std::sync::atomic::{AtomicU32, Ordering};

use crate::{
  il::{DefineKind, MethodSig, Program, Statement, Type},
  naming::{
    naming_context::{DeclKind, Namespace, ScopeKind, Visibility},
    NamingContext, ScopeId,
  },
};

static ID_COUNTER: AtomicU32 = AtomicU32::new(1);

pub fn collect_pgm(ctx: &mut NamingContext, pgm: &mut Program) {
  ctx.clear_scope_stack();
  let pgm_scope_id = ctx.enter_scope(ScopeKind::File, pgm.path.to_str());
  let pgm_name = pgm
    .path
    .canonicalize()
    .ok()
    .and_then(|p| p.to_str().map(|s| s.to_string()))
    .unwrap_or_else(|| {
      let id = ID_COUNTER.fetch_add(1, Ordering::Relaxed);
      format!("__file_unnamed_{}", id)
    });
  let pgm_symbol_id = ctx.register_symbol(
    pgm_name.as_str(),
    Namespace::File,
    DeclKind::File,
    Visibility::Public,
    None,
  );
  pgm.scope_id = Some(pgm_scope_id);
  pgm.symbol_id = Some(pgm_symbol_id);
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
  bindings: Option<&Vec<(Type, String)>>,
) -> ScopeId {
  let new_scope_id = ctx.enter_scope(scope_kind, name);
  for (t, name) in bindings.unwrap_or(&Vec::new()) {
    ctx.register_symbol(
      name,
      Namespace::Value,
      DeclKind::Param(Some(t.clone())),
      Visibility::Private,
      None,
    );
  }
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
        ctx.register_symbol(
          name,
          Namespace::Type,
          DeclKind::Class,
          Visibility::Public, // TODO: handle visibility
          tag.clone(),
        );
        *scope_id = Some(collect_stmts_in_scope(
          ctx,
          body,
          ScopeKind::Module,
          Some(name),
          None,
        ));
      }
      DefineKind::Method {
        sig: MethodSig {
          name, params, id, ..
        },
        body,
        ..
      } => {
        let symbol_id = ctx.register_symbol(
          name,
          Namespace::Method,
          DeclKind::Function,
          Visibility::Public, // TODO: handle visibility
          tag.clone(),
        );
        *scope_id = Some(collect_stmts_in_scope(
          ctx,
          body,
          ScopeKind::Function,
          Some(name),
          Some(params),
        ));
        *id = Some(symbol_id);
      }
      DefineKind::Package(_) => {}
      DefineKind::Constructor { body, .. } => {
        *scope_id = Some(collect_stmts_in_scope(
          ctx,
          body,
          ScopeKind::Function,
          None,
          None,
        ));
      }
      DefineKind::Interface { .. } => {}
      DefineKind::Field { .. } => {}
      DefineKind::Var { name, t, .. } => {
        ctx.register_symbol(
          name,
          Namespace::Value,
          DeclKind::Var(t.clone()),
          Visibility::Private, // TODO: handle visibility
          tag.clone(),
        );
      }
    },
    _ => {}
  }
}
