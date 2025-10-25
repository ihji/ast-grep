use petgraph::graph::DiGraph;

use crate::{
  il::{self, MethodSig},
  naming::{
    naming_context::{DeclKind, Namespace},
    NamingContext, ScopeId, SymbolId,
  },
};

pub type CallGraph = DiGraph<SymbolId, ()>;

pub fn annotate_pgm(ctx: &mut NamingContext, pgm: &mut il::Program) -> CallGraph {
  let mut call_graph = CallGraph::new();
  for stmt in &mut pgm.statements {
    annotate_stmt(ctx, &mut call_graph, pgm.scope_id, None, stmt);
  }
  ctx.exit_scope();
  call_graph
}

fn annotate_stmt(
  ctx: &mut NamingContext,
  call_graph: &mut CallGraph,
  parent_scope_id: Option<ScopeId>,
  parent_symbol_id: Option<SymbolId>,
  stmt: &mut il::Statement,
) {
  match stmt {
    il::Statement::Define { kind, scope_id, .. } => match kind {
      il::DefineKind::Class { body, .. } => {
        for stmt in body {
          annotate_stmt(ctx, call_graph, *scope_id, parent_symbol_id, stmt);
        }
      }
      il::DefineKind::Method {
        sig: MethodSig { id, .. },
        body,
        ..
      } => {
        for stmt in body {
          annotate_stmt(ctx, call_graph, *scope_id, *id, stmt);
        }
      }
      il::DefineKind::Constructor { body, .. } => {
        for stmt in body {
          annotate_stmt(ctx, call_graph, *scope_id, parent_symbol_id, stmt);
        }
      }
      _ => {}
    },
    il::Statement::Assign { left, right, .. } => {
      annotate_value(
        ctx,
        call_graph,
        parent_scope_id,
        parent_symbol_id,
        left,
        Namespace::Value,
      );
      annotate_value(
        ctx,
        call_graph,
        parent_scope_id,
        parent_symbol_id,
        right,
        Namespace::Value,
      );
    }
    il::Statement::Invoke {
      callee,
      args,
      ret_loc,
      ..
    } => {
      annotate_value(
        ctx,
        call_graph,
        parent_scope_id,
        parent_symbol_id,
        callee,
        Namespace::Method,
      );
      for arg in args {
        annotate_value(
          ctx,
          call_graph,
          parent_scope_id,
          parent_symbol_id,
          arg,
          Namespace::Value,
        );
      }
      if let Some(ret_loc) = ret_loc {
        annotate_value(
          ctx,
          call_graph,
          parent_scope_id,
          parent_symbol_id,
          ret_loc,
          Namespace::Value,
        );
      }
    }
    _ => {}
  }
}

fn annotate_value(
  ctx: &mut NamingContext,
  call_graph: &mut CallGraph,
  parent_scope_id: Option<ScopeId>,
  parent_symbol_id: Option<SymbolId>,
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
        if namespace == Namespace::Method {
          if let Some(caller_id) = parent_symbol_id {
            let caller = call_graph.add_node(caller_id);
            let callee = call_graph.add_node(symbol.id);
            call_graph.add_edge(caller, callee, ());
          }
        }
      }
    }
    il::ValueKind::Exp(e) => annotate_expr(ctx, call_graph, parent_scope_id, parent_symbol_id, e),
    _ => {}
  }
}

fn annotate_expr(
  ctx: &mut NamingContext,
  call_graph: &mut CallGraph,
  parent_scope_id: Option<ScopeId>,
  parent_symbol_id: Option<SymbolId>,
  expr: &mut il::Expr,
) {
  match expr {
    il::Expr::UnOp { value, .. } => {
      annotate_value(
        ctx,
        call_graph,
        parent_scope_id,
        parent_symbol_id,
        value,
        Namespace::Value,
      );
    }
    il::Expr::BinOp { left, right, .. } => {
      annotate_value(
        ctx,
        call_graph,
        parent_scope_id,
        parent_symbol_id,
        left,
        Namespace::Value,
      );
      annotate_value(
        ctx,
        call_graph,
        parent_scope_id,
        parent_symbol_id,
        right,
        Namespace::Value,
      );
    }
    il::Expr::DotAccess { base, .. } => {
      annotate_value(
        ctx,
        call_graph,
        parent_scope_id,
        parent_symbol_id,
        base,
        Namespace::Value,
      );
    }
    _ => {}
  }
}
