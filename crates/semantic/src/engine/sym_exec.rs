use std::collections::HashSet;

use petgraph::algo::tarjan_scc;
use petgraph::graph::NodeIndex;
use petgraph::visit::{DfsPostOrder, EdgeRef, Walker};

use crate::engine::context::SessionCtx;
use crate::engine::domain::AMem;
use crate::engine::path_explorer::{DumbPathExplorer, PathExplorer};
use crate::engine::reports::Reports;
use crate::engine::sym_semantics::transfer_block;
use crate::il::{CFGs, CfgEdgeKind, MethodSig, CFG};
use crate::naming::{CallGraph, SymbolId};

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

fn execute_path(
  context: &SessionCtx,
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
  println!("Final memory: {}", memory.display_with_trace(&context));
  memory
}

fn execute_method(context: &mut SessionCtx, method_sig: &MethodSig, cfg: &CFG) -> Reports {
  let mut path_explorer = DumbPathExplorer::new(10);
  let mut findings = Reports::new();
  loop {
    let mem = execute_path(context, method_sig, cfg, &mut path_explorer);
    findings.all.extend(mem.findings.all.iter().cloned());
    if let Some(id) = method_sig.id {
      context.summary.add_memory(id, mem);
    } else {
      println!(
        "Failed to save output memory. MethodSig has no id: {}",
        method_sig.name
      );
    }
    if !path_explorer.next_path() {
      break;
    }
  }
  findings
}

fn scc_order(call_graph: &CallGraph) -> Vec<NodeIndex> {
  let sccs = tarjan_scc(&call_graph.graph);
  let mut order = Vec::new();
  for scc in sccs.iter() {
    // TODO: find best order in SCC
    for node in scc {
      order.push(*node);
    }
  }
  order
}

pub fn execute(
  context: &mut SessionCtx,
  entry_id: Option<SymbolId>,
  call_graph: &CallGraph,
  cfg: &CFGs,
) -> Reports {
  let mut all_findings = Reports::new();

  let order_node_idx = if let Some(entry_id) = entry_id {
    if let Some(entry_idx) = call_graph
      .graph
      .raw_nodes()
      .iter()
      .position(|node| node.weight == entry_id)
      .map(NodeIndex::new)
    {
      let dfs = DfsPostOrder::new(&call_graph.graph, entry_idx);
      let mut output = dfs.iter(&call_graph.graph).collect::<Vec<_>>();
      let reachable = output.iter().cloned().collect::<HashSet<_>>();
      for node in call_graph.graph.node_indices() {
        if !reachable.contains(&node) {
          output.push(node);
        }
      }
      output
    } else {
      scc_order(call_graph)
    }
  } else {
    scc_order(call_graph)
  };
  let order_symbol_ids: Vec<SymbolId> = order_node_idx
    .iter()
    .map(|idx| call_graph.graph[*idx])
    .collect();
  let order: Vec<(SymbolId, &(MethodSig, CFG))> = order_symbol_ids
    .iter()
    .filter_map(|id| cfg.0.get(id).map(|cfg| (*id, cfg)))
    .collect();

  println!("Analyze order:");
  for (id, (method_sig, _)) in &order {
    println!("{}: {}", id, method_sig.name);
  }

  for (_, (method_sig, cfg)) in order {
    println!("Executing method: {}", method_sig.name);
    let findings = execute_method(context, method_sig, cfg);
    all_findings.all.extend(findings.all);
  }
  all_findings
}
