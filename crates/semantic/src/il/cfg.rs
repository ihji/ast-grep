use std::collections::HashMap;

use derive_visitor::{Drive, Visitor};
use petgraph::{
  graph::{DiGraph, NodeIndex},
  visit::EdgeRef,
};

use crate::il::{DefineKind, InvokeKind, MethodSig, Program, Statement, Type, Value, ValueKind};

#[derive(Debug, Clone, PartialEq)]
pub enum CfgStatement {
  Assign {
    left: Value,
    right: Value,
    tag: Option<usize>,
  },
  Invoke {
    kind: InvokeKind,
    callee: Value,
    args: Vec<Value>,
    ret_type: Type,
    ret_loc: Option<Value>,
    tag: Option<usize>,
  },
  Assume {
    value: Value,
    tag: Option<usize>,
  },
  Return {
    value: Option<Value>,
    tag: Option<usize>,
  },
  Nop {
    tag: Option<usize>,
  },
  Break,
}

impl CfgStatement {
  pub fn get_tag(&self) -> Option<usize> {
    match self {
      CfgStatement::Assign { tag, .. } => *tag,
      CfgStatement::Invoke { tag, .. } => *tag,
      CfgStatement::Assume { tag, .. } => *tag,
      CfgStatement::Return { tag, .. } => *tag,
      CfgStatement::Nop { tag } => *tag,
      CfgStatement::Break => None,
    }
  }
}

#[derive(Debug, Clone)]
pub struct BasicBlock {
  pub stmts: Vec<CfgStatement>,
}

impl std::fmt::Display for CfgStatement {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      CfgStatement::Assign { left, right, tag } => {
        write!(f, "{} = {}", left, right)?;
        if let Some(tag) = tag {
          write!(f, " [{}]", tag)?;
        }
      }
      CfgStatement::Invoke {
        kind,
        callee,
        args,
        ret_loc,
        tag,
        ..
      } => {
        if let Some(ret) = ret_loc {
          write!(f, "{} = ", ret)?;
        }
        write!(f, "{:?}({})(", kind, callee)?;
        for (j, arg) in args.iter().enumerate() {
          if j > 0 {
            write!(f, ", ")?;
          }
          write!(f, "{}", arg)?;
        }
        write!(f, ")")?;
        if let Some(tag) = tag {
          write!(f, " [{}]", tag)?;
        }
      }
      CfgStatement::Assume { value, tag } => {
        write!(f, "assume {}", value)?;
        if let Some(tag) = tag {
          write!(f, " [{}]", tag)?;
        }
      }
      CfgStatement::Return { value, tag } => {
        write!(f, "return")?;
        if let Some(v) = value {
          write!(f, " {}", v)?;
        }
        if let Some(tag) = tag {
          write!(f, " [{}]", tag)?;
        }
      }
      CfgStatement::Nop { tag } => {
        write!(f, "nop")?;
        if let Some(tag) = tag {
          write!(f, " [{}]", tag)?;
        }
      }
      CfgStatement::Break => {
        write!(f, "break")?;
      }
    }
    Ok(())
  }
}

impl std::fmt::Display for BasicBlock {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    for (i, stmt) in self.stmts.iter().enumerate() {
      write!(f, "{}", stmt)?;
      if i < self.stmts.len() - 1 {
        writeln!(f)?;
      }
    }
    Ok(())
  }
}

#[derive(Debug, Clone, Eq)]
pub enum CfgEdgeKind {
  Uncond,
  True(Option<usize>),
  False(Option<usize>),
  Call,
  Return,
  Goto,
}

impl std::fmt::Display for CfgEdgeKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      CfgEdgeKind::Uncond => write!(f, "u"),
      CfgEdgeKind::True(tag) => {
        write!(f, "true")?;
        if let Some(tag) = tag {
          write!(f, " [{}]", tag)?;
        }
        Ok(())
      }
      CfgEdgeKind::False(tag) => {
        write!(f, "false")?;
        if let Some(tag) = tag {
          write!(f, " [{}]", tag)?;
        }
        Ok(())
      }
      CfgEdgeKind::Call => write!(f, "call"),
      CfgEdgeKind::Return => write!(f, "return"),
      CfgEdgeKind::Goto => write!(f, "goto"),
    }
  }
}

impl PartialEq for CfgEdgeKind {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::True(_), Self::True(_)) => true,
      (Self::False(_), Self::False(_)) => true,
      (Self::Uncond, Self::Uncond) => true,
      (Self::Call, Self::Call) => true,
      (Self::Return, Self::Return) => true,
      (Self::Goto, Self::Goto) => true,
      _ => false,
    }
  }
}

pub type CfgGraph = DiGraph<BasicBlock, CfgEdgeKind>;

pub struct CFGs(pub HashMap<MethodSig, CFG>);

#[derive(Visitor)]
#[visitor(DefineKind(enter))]
struct MethodVisitor<'a> {
  cfgs: &'a mut CFGs,
}

impl MethodVisitor<'_> {
  fn enter_define_kind(&mut self, define: &crate::il::DefineKind) {
    if let DefineKind::Method { sig, body, .. } = define {
      println!("Method: {}", sig.name);
      self.cfgs.0.insert(sig.clone(), CFG::convert(body));
    }
  }
}

impl CFGs {
  pub fn new() -> Self {
    CFGs(HashMap::new())
  }
  pub fn insert(&mut self, pgm: Program) {
    let top_sig = MethodSig::from_path(&pgm.path);
    let cfg = CFG::convert(&pgm.statements);
    self.0.insert(top_sig, cfg);
    let mut visitor = MethodVisitor { cfgs: self };
    pgm.drive(&mut visitor);
  }
}

#[derive(Debug)]
pub struct CFG {
  pub graph: CfgGraph,
  pub entry: NodeIndex,
}

impl CFG {
  pub fn convert(stmts: &Vec<Statement>) -> Self {
    let mut graph = DiGraph::new();
    let (entry, _) = Self::convert_statements(&mut graph, stmts);
    let mut cfg = CFG { graph, entry };
    cfg.post_processing();
    cfg
  }
  fn convert_statements(
    graph: &mut CfgGraph,
    stmts: &Vec<Statement>,
  ) -> (NodeIndex, Option<NodeIndex>) {
    let mut in_node = None;
    let mut prev_out = None;
    for stmt in stmts {
      let (curr_in, curr_out) = CFG::convert_statement(graph, stmt);
      if let Some(prev) = prev_out {
        graph.add_edge(prev, curr_in, CfgEdgeKind::Uncond);
      } else {
        in_node = Some(curr_in);
      }
      prev_out = curr_out;
    }
    if in_node.is_none() {
      let block = BasicBlock { stmts: vec![] };
      let block_idx = graph.add_node(block);
      (block_idx, Some(block_idx))
    } else {
      (in_node.unwrap(), prev_out)
    }
  }
  fn convert_statement(graph: &mut CfgGraph, stmt: &Statement) -> (NodeIndex, Option<NodeIndex>) {
    match stmt {
      Statement::Break => {
        let block = BasicBlock {
          stmts: vec![CfgStatement::Break],
        };
        let idx = graph.add_node(block);
        (idx, Some(idx)) // should we return None here?
      }
      Statement::Assign { left, right, tag } => {
        let block = BasicBlock {
          stmts: vec![CfgStatement::Assign {
            left: left.clone(),
            right: right.clone(),
            tag: *tag,
          }],
        };
        let idx = graph.add_node(block);
        (idx, Some(idx))
      }
      Statement::Invoke {
        kind,
        callee,
        args,
        ret_type,
        ret_loc,
        tag,
      } => {
        let block = BasicBlock {
          stmts: vec![CfgStatement::Invoke {
            kind: kind.clone(),
            callee: callee.clone(),
            args: args.clone(),
            ret_type: ret_type.clone(),
            ret_loc: ret_loc.clone(),
            tag: *tag,
          }],
        };
        let idx = graph.add_node(block);
        (idx, Some(idx))
      }
      Statement::Return { value, tag } => {
        let block = BasicBlock {
          stmts: vec![CfgStatement::Return {
            value: value.clone(),
            tag: *tag,
          }],
        };
        let idx = graph.add_node(block);
        (idx, None)
      }
      Statement::If {
        condition,
        then_stmts,
        else_stmts,
        tag,
      } => {
        let entry_block = BasicBlock { stmts: vec![] };
        let entry_idx = graph.add_node(entry_block);

        let then_block = Self::convert_statements(graph, then_stmts);
        let else_block = Self::convert_statements(graph, else_stmts);
        // Create a split block for the condition
        let assume_block = BasicBlock {
          stmts: vec![CfgStatement::Assume {
            value: condition.clone(),
            tag: *tag,
          }],
        };
        let assume_idx = graph.add_node(assume_block);

        // Connect split block to the entry of then and else blocks
        let (then_entry, then_exit) = then_block;
        let (else_entry, else_exit) = else_block;
        graph.add_edge(entry_idx, assume_idx, CfgEdgeKind::True(*tag));
        graph.add_edge(assume_idx, then_entry, CfgEdgeKind::Uncond);

        let negated = condition.kind.logical_not();
        if let Some(neg_cond) = negated {
          let assume_not_block = BasicBlock {
            stmts: vec![CfgStatement::Assume {
              value: Value {
                kind: neg_cond,
                tag: condition.tag,
              },
              tag: *tag,
            }],
          };
          let assume_not_idx = graph.add_node(assume_not_block);
          graph.add_edge(entry_idx, assume_not_idx, CfgEdgeKind::False(*tag));
          graph.add_edge(assume_not_idx, else_entry, CfgEdgeKind::Uncond);
        } else {
          graph.add_edge(entry_idx, else_entry, CfgEdgeKind::False(*tag));
        }

        // Create a join block
        let join_block = BasicBlock { stmts: vec![] };
        let join_idx = graph.add_node(join_block);

        // Connect the exit of then and else blocks to the join block
        if let Some(then_exit) = then_exit {
          graph.add_edge(then_exit, join_idx, CfgEdgeKind::Uncond);
        }
        if let Some(else_exit) = else_exit {
          graph.add_edge(else_exit, join_idx, CfgEdgeKind::Uncond);
        }

        // Return the split and join as the entry and exit of this if statement
        (entry_idx, Some(join_idx))
      }
      Statement::For {
        init,
        condition,
        update,
        body,
        tag,
      } => {
        // Convert the for loop into a CFG
        let (init_in, init_out) = Self::convert_statements(graph, init);
        let (body_in, body_out) = Self::convert_statements(graph, body);
        let (update_in, update_out) = Self::convert_statements(graph, update);

        // Create a block for the condition
        let (cond_in, cond_out) = Self::convert_statements(graph, &condition.statements);
        let cond_assume = BasicBlock {
          stmts: vec![CfgStatement::Assume {
            value: condition.result.clone().unwrap_or(Value {
              kind: ValueKind::IntLit(1),
              tag: None,
            }),
            tag: *tag,
          }],
        };
        let cond_idx = graph.add_node(cond_assume);
        if let Some(cond_out) = cond_out {
          graph.add_edge(cond_out, cond_idx, CfgEdgeKind::Uncond);
        }

        // Create a join block
        let join_block = BasicBlock { stmts: vec![] };
        let join_idx = graph.add_node(join_block);

        // Connect the blocks
        if let Some(init_out) = init_out {
          graph.add_edge(init_out, cond_in, CfgEdgeKind::Uncond);
        }
        graph.add_edge(cond_idx, body_in, CfgEdgeKind::True(*tag));
        graph.add_edge(cond_idx, join_idx, CfgEdgeKind::False(*tag));
        if let Some(body_out) = body_out {
          graph.add_edge(body_out, update_in, CfgEdgeKind::Uncond);
        }
        if let Some(update_out) = update_out {
          graph.add_edge(update_out, cond_in, CfgEdgeKind::Uncond);
        }

        (init_in, Some(join_idx))
      }
      Statement::DoWhile {
        body,
        condition,
        tag,
      } => {
        // Convert the body statements
        let (body_in, body_out) = Self::convert_statements(graph, body);
        // Create a block for the condition
        let (cond_in, cond_out) = Self::convert_statements(graph, &condition.statements);
        let cond_assume = BasicBlock {
          stmts: vec![CfgStatement::Assume {
            value: condition.result.clone().unwrap_or(Value {
              kind: ValueKind::IntLit(1),
              tag: None,
            }),
            tag: None,
          }],
        };
        let cond_idx = graph.add_node(cond_assume);
        if let Some(cond_out) = cond_out {
          graph.add_edge(cond_out, cond_idx, CfgEdgeKind::Uncond);
        }

        // Create a join block
        let join_block = BasicBlock { stmts: vec![] };
        let join_idx = graph.add_node(join_block);

        // Connect the blocks
        if let Some(body_out) = body_out {
          graph.add_edge(body_out, cond_in, CfgEdgeKind::Uncond);
        }
        graph.add_edge(cond_idx, body_in, CfgEdgeKind::True(*tag));
        graph.add_edge(cond_idx, join_idx, CfgEdgeKind::False(*tag));

        (body_in, Some(join_idx))
      }
      Statement::Switch {
        value,
        cases,
        default,
        tag: _,
      } => {
        // Create a block for the switch value
        let switch_block = BasicBlock { stmts: vec![] };
        let switch_idx = graph.add_node(switch_block);
        let exit_block = BasicBlock { stmts: vec![] };
        let exit_idx = graph.add_node(exit_block);

        let (value_in, value_out) = Self::convert_statements(graph, &value.statements);
        graph.add_edge(switch_idx, value_in, CfgEdgeKind::Uncond);

        // Create blocks for each case
        let mut prev_out = value_out;
        let mut break_outs = Vec::new();
        for (case_value, case_stmts) in cases {
          let (case_in, case_out) = Self::convert_statements(graph, case_stmts);
          // Traverse from case_in to case_out following edges, collecting indices of BasicBlocks with a single Break statement
          let mut stack = vec![case_in];
          let mut visited = std::collections::HashSet::new();
          while let Some(node_idx) = stack.pop() {
            if !visited.insert(node_idx) {
              continue;
            }
            if let Some(block) = graph.node_weight(node_idx) {
              if block.stmts.len() == 1 && block.stmts[0] == CfgStatement::Break {
                break_outs.push(node_idx);
              }
            }
            if Some(node_idx) == case_out {
              continue;
            }
            for neighbor in graph.neighbors(node_idx) {
              stack.push(neighbor);
            }
          }
          let (case_stmts_in, case_stmts_out) =
            Self::convert_statements(graph, &case_value.statements);
          let case_value = case_value.result.clone().unwrap_or(Value {
            kind: ValueKind::IntLit(1),
            tag: None,
          });
          let true_cond_value = if let Some(ref v) = value.result {
            Value {
              kind: ValueKind::Exp(crate::il::Expr::BinOp {
                left: Box::new(v.clone()),
                right: Box::new(case_value.clone()),
                op: crate::il::BinaryOp::Eq,
              }),
              tag: case_value.tag,
            }
          } else {
            case_value.clone()
          };
          let false_cond_value = if let Some(ref v) = value.result {
            Value {
              kind: ValueKind::Exp(crate::il::Expr::BinOp {
                left: Box::new(v.clone()),
                right: Box::new(case_value.clone()),
                op: crate::il::BinaryOp::Neq,
              }),
              tag: case_value.tag,
            }
          } else {
            case_value.clone()
          };
          let cond_tag = case_value.tag;
          let true_cond_block = BasicBlock {
            stmts: vec![CfgStatement::Assume {
              value: true_cond_value,
              tag: cond_tag,
            }],
          };
          let true_cond_idx = graph.add_node(true_cond_block);
          let false_cond_block = BasicBlock {
            stmts: vec![CfgStatement::Assume {
              value: false_cond_value,
              tag: cond_tag,
            }],
          };
          let false_cond_idx = graph.add_node(false_cond_block);
          let split_block = BasicBlock { stmts: vec![] };
          let split_idx = graph.add_node(split_block);
          let join_block = BasicBlock { stmts: vec![] };
          let join_idx = graph.add_node(join_block);
          if let Some(prev_out) = prev_out {
            graph.add_edge(prev_out, case_stmts_in, CfgEdgeKind::Uncond);
          }
          if let Some(case_stmts_out) = case_stmts_out {
            graph.add_edge(case_stmts_out, split_idx, CfgEdgeKind::Uncond);
          }
          graph.add_edge(split_idx, true_cond_idx, CfgEdgeKind::True(cond_tag));
          graph.add_edge(split_idx, false_cond_idx, CfgEdgeKind::False(cond_tag));
          graph.add_edge(true_cond_idx, case_in, CfgEdgeKind::Uncond);
          graph.add_edge(false_cond_idx, join_idx, CfgEdgeKind::Uncond);
          if let Some(case_out) = case_out {
            graph.add_edge(case_out, join_idx, CfgEdgeKind::Uncond);
          }
          prev_out = Some(join_idx);
        }

        // Handle the default case if it exists
        if let Some(default_stmts) = default {
          let (default_in, default_out) = Self::convert_statements(graph, default_stmts);
          if let Some(prev_out) = prev_out {
            graph.add_edge(prev_out, default_in, CfgEdgeKind::Uncond);
          }
          prev_out = default_out;
        }

        // Connect all break outs to the exit block
        for break_out in break_outs {
          graph.add_edge(break_out, exit_idx, CfgEdgeKind::Uncond);
        }

        // Finally, connect the last node to the exit block
        if let Some(prev_out) = prev_out {
          graph.add_edge(prev_out, exit_idx, CfgEdgeKind::Uncond);
        }
        (switch_idx, Some(exit_idx))
      }
      _ => {
        println!("Unhandled statement type in CFG conversion: {:?}", stmt);
        // Handle other statement types similarly
        let block = BasicBlock { stmts: vec![] };
        let idx = graph.add_node(block);
        (idx, Some(idx))
      }
    }
  }
  pub fn post_processing(&mut self) {
    Self::merge_basic_blocks(&mut self.graph);
  }
  fn merge_basic_blocks(graph: &mut CfgGraph) {
    let mut changed = true;
    while changed {
      changed = false;
      let node_indices: Vec<_> = graph.node_indices().collect();
      for node_idx in node_indices {
        // Check if this node has exactly one outgoing edge
        let outgoing: Vec<_> = graph.edges(node_idx).collect();
        if outgoing.len() != 1 {
          continue;
        }

        let edge = outgoing[0];
        let target_idx = edge.target();

        // Skip if edge is not unconditional
        if edge.weight() != &CfgEdgeKind::Uncond {
          continue;
        }

        // Check if target has exactly one incoming edge
        let incoming: Vec<_> = graph
          .edges_directed(target_idx, petgraph::Direction::Incoming)
          .collect();
        if incoming.len() != 1 {
          continue;
        }

        // Skip if target is the same as source (self-loop)
        if node_idx == target_idx {
          continue;
        }

        // Merge: append target's statements to source's statements
        let target_stmts = graph
          .node_weight(target_idx)
          .map(|target_block| target_block.stmts.clone());
        if let Some(mut target_stmts) = target_stmts {
          if let Some(source_block) = graph.node_weight_mut(node_idx) {
            source_block.stmts.append(&mut target_stmts);
          }
        }

        // Redirect all outgoing edges from target to source
        let target_outgoing: Vec<_> = graph
          .edges(target_idx)
          .map(|e| (e.target(), e.weight().clone()))
          .collect();
        for (next_target, edge_kind) in target_outgoing {
          graph.add_edge(node_idx, next_target, edge_kind);
        }

        // Remove the target node
        graph.remove_node(target_idx);
        changed = true;
        break;
      }
    }
  }
}
