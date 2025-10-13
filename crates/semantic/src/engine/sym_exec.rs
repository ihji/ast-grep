use petgraph::graph::NodeIndex;
use petgraph::visit::EdgeRef;

use crate::engine::context::Context;
use crate::engine::domain::AMem;
use crate::engine::path_explorer::{DumbPathExplorer, PathExplorer};
use crate::engine::sym_semantics::transfer_block;
use crate::il::{CFGs, CfgEdgeKind, MethodSig, CFG};

fn get_next_tag(cfg: &CFG, next_id: NodeIndex) -> Option<usize> {
  let mut next_first_stmt = None;
  let mut current_id = next_id;
  let threshold = 10;

  for _ in 0..threshold {
    let current_block = &cfg.graph[current_id];
    if let Some(first_stmt) = current_block.stmts.first() {
      next_first_stmt = Some(first_stmt);
      break;
    }

    let edges = cfg.graph.edges(current_id).collect::<Vec<_>>();
    if edges.len() == 1 {
      current_id = edges[0].target();
    } else {
      break;
    }
  }

  next_first_stmt.and_then(|s| s.get_tag())
}

pub fn execute_path(
  context: &Context,
  method_sig: &MethodSig,
  cfg: &CFG,
  path_explorer: &mut impl PathExplorer,
) -> AMem {
  println!("Symbolic execution engine");
  let mut next_id = Some(cfg.entry);
  let mut memory = AMem::new();
  memory.initialize(method_sig);
  while let Some(id) = next_id {
    let current_block = &cfg.graph[id];
    transfer_block(context, path_explorer, current_block, &mut memory);
    context.reporter.report(&memory.findings);
    let edges = cfg.graph.edges(id).collect::<Vec<_>>();
    if path_explorer.is_done() || edges.is_empty() {
      path_explorer.mark_done();
      next_id = None;
    } else if edges.len() > 1 {
      let chosen = if path_explorer.next() {
        CfgEdgeKind::True(None)
      } else {
        CfgEdgeKind::False(None)
      };
      let edge = edges.iter().find(|e| e.weight() == &chosen);
      next_id = edge.map(|e| e.target()).or_else(|| {
        println!("No edge found for choice: {:?}", chosen);
        Some(edges[0].target())
      });
      if let Some(edge) = edge {
        let next_tag = next_id.and_then(|id| get_next_tag(cfg, id));
        let tag = match edge.weight() {
          CfgEdgeKind::True(t) => *t,
          CfgEdgeKind::False(t) => *t,
          _ => None,
        };
        memory.trace.add_branch(
          context,
          &next_tag,
          &tag,
          *edge.weight() == CfgEdgeKind::True(None),
        );
      }
    } else {
      next_id = Some(edges[0].target());
    }
  }
  println!(
    "Final memory: {}",
    memory.display_with_trace(&context.trace_arena)
  );
  memory
}

pub fn execute_method(context: &Context, method_sig: &MethodSig, cfg: &CFG) {
  let mut path_explorer = DumbPathExplorer::new(10);
  loop {
    execute_path(context, method_sig, cfg, &mut path_explorer);
    if !path_explorer.next_path() {
      break;
    }
  }
}

pub fn execute(context: &Context, cfg: &CFGs) {
  for (method_sig, cfg) in &cfg.0 {
    println!("Executing method: {}", method_sig.name);
    execute_method(context, method_sig, cfg);
  }
}
