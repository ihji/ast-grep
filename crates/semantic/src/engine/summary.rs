use std::collections::{HashMap, HashSet, VecDeque};

use crate::engine::domain::{ALoc, ALocKind, AMem, AValue, SExpr, Seg};
use crate::engine::history::{ValueId, ValueKind};
use crate::il::MethodSig;
use crate::naming::SymbolId;

#[derive(Debug)]
pub struct SummaryRegistry {
  summaries: HashMap<SymbolId, Summary>,
}

#[derive(Debug)]
pub struct Summary {
  pub parametrized_memories: Vec<AMem>,
  pub method_sig: MethodSig,
}

impl SummaryRegistry {
  pub fn new() -> Self {
    Self {
      summaries: HashMap::new(),
    }
  }
  pub fn create_summary(&mut self, id: SymbolId, sig: &MethodSig) {
    self.summaries.insert(
      id,
      Summary {
        parametrized_memories: vec![],
        method_sig: sig.clone(),
      },
    );
  }
  pub fn add_memory(&mut self, id: SymbolId, mut mem: AMem) -> anyhow::Result<()> {
    summarize(&mut mem);
    self
      .summaries
      .get_mut(&id)
      .map(|s| s.parametrized_memories.push(mem))
      .ok_or_else(|| anyhow::anyhow!("Summary not found"))
  }
  pub fn get_summary(&self, id: &SymbolId) -> Option<&Summary> {
    self.summaries.get(id)
  }
}

fn is_symbolic_root_star_loc(loc: &ALoc) -> bool {
  match &loc.kind {
    ALocKind::ASymStar(inner) => matches!(
      inner.kind,
      ALocKind::AParam(_) | ALocKind::AGlobal(_) | ALocKind::AThis
    ),
    _ => false,
  }
}

fn path_is_prefix(prefix: &[Seg], full: &[Seg]) -> bool {
  if prefix.len() > full.len() {
    return false;
  }
  for (a, b) in prefix.iter().zip(full.iter()) {
    if a != b {
      return false;
    }
  }
  true
}

fn key_matches_loc(key: &ALoc, base: &ALoc) -> bool {
  // Do not keep local-only memory entries in summaries
  if matches!(key.kind, ALocKind::ALocal(_)) {
    return false;
  }
  key.kind == base.kind && path_is_prefix(&base.path, &key.path)
}

fn collect_from_sexpr(expr: &SExpr, out_locs: &mut HashSet<ALoc>, used_ids: &mut HashSet<ValueId>) {
  match expr {
    SExpr::SAdd(a, b)
    | SExpr::SSub(a, b)
    | SExpr::SMul(a, b)
    | SExpr::SEquals(a, b)
    | SExpr::SNotEquals(a, b)
    | SExpr::SLessThan(a, b)
    | SExpr::SGreaterThan(a, b)
    | SExpr::SLessThanOrEqual(a, b)
    | SExpr::SGreaterThanOrEqual(a, b) => {
      collect_from_value(a, out_locs, used_ids);
      collect_from_value(b, out_locs, used_ids);
    }
    SExpr::SNeg(a) => collect_from_value(a, out_locs, used_ids),
    SExpr::SCond {
      condition,
      then_branch,
      else_branch,
    } => {
      collect_from_value(condition, out_locs, used_ids);
      collect_from_value(then_branch, out_locs, used_ids);
      collect_from_value(else_branch, out_locs, used_ids);
    }
    SExpr::SFieldAccess { base_value, .. } => {
      collect_from_value(base_value, out_locs, used_ids);
    }
    SExpr::STop | SExpr::SStar(_) => { /* should we handle these cases? */ }
  }
}

fn collect_from_value(val: &AValue, out_locs: &mut HashSet<ALoc>, used_ids: &mut HashSet<ValueId>) {
  match val {
    AValue::ARef { location, .. } => {
      out_locs.insert((**location).clone());
    }
    AValue::ANull { id } => {
      used_ids.insert(ValueId {
        kind: ValueKind::Null,
        id: *id,
      });
    }
    AValue::AStruct { fields } => {
      for v in fields.values() {
        collect_from_value(v, out_locs, used_ids);
      }
    }
    AValue::AArray { elements } => {
      for v in elements {
        collect_from_value(v, out_locs, used_ids);
      }
    }
    AValue::ASym(expr) => collect_from_sexpr(expr, out_locs, used_ids),
    AValue::StructMarker { .. }
    | AValue::AInt(_)
    | AValue::AString(_)
    | AValue::ATop { .. } /* TODO: ATop can be used as ALoc */ => {}
  }
}

pub fn summarize(mem: &mut AMem) {
  mem.findings.clear();

  // Build seeds from parameter pointer locations: *(param(...))
  let mut keys_to_keep: HashSet<ALoc> = HashSet::new();
  let mut queue_keys: VecDeque<ALoc> = VecDeque::new();

  for key in mem.memory.keys() {
    if is_symbolic_root_star_loc(key) {
      if keys_to_keep.insert(key.clone()) {
        queue_keys.push_back(key.clone());
      }
    }
  }

  let mut seen_keys: HashSet<ALoc> = HashSet::new();
  let mut seen_locs: HashSet<ALoc> = HashSet::new();
  let mut queue_locs: VecDeque<ALoc> = VecDeque::new();
  let mut used_value_ids: HashSet<ValueId> = HashSet::new();

  while let Some(k) = queue_keys.pop_front() {
    if !seen_keys.insert(k.clone()) {
      continue;
    }
    if let Some(v) = mem.memory.get(&k) {
      // Collect referenced locations and used history ids
      let mut out_locs: HashSet<ALoc> = HashSet::new();
      collect_from_value(v, &mut out_locs, &mut used_value_ids);
      for l in out_locs {
        if seen_locs.insert(l.clone()) {
          queue_locs.push_back(l);
        }
      }
    }

    // Expand discovered locations to concrete keys in memory
    while let Some(l) = queue_locs.pop_front() {
      for mk in mem.memory.keys() {
        if key_matches_loc(mk, &l) && keys_to_keep.insert(mk.clone()) {
          queue_keys.push_back(mk.clone());
        }
      }
    }
  }

  mem.memory.retain(|k, _| keys_to_keep.contains(k));

  mem.history_registry.retain_only(&used_value_ids);
}

fn substitute_aloc(loc: &ALoc, param_map: &HashMap<String, AValue>) -> ALoc {
  match &loc.kind {
    ALocKind::ASymStar(inner) => match &inner.kind {
      ALocKind::AParam(name) => param_map
        .get(name)
        .and_then(|arg| arg.to_aloc())
        .map(|mut arg_loc| {
          arg_loc.path.extend(loc.path.clone());
          arg_loc
        })
        .unwrap_or_else(|| loc.clone()),
      _ => loc.clone(),
    },
    _ => loc.clone(),
  }
}

fn substitute_value(val: &AValue, param_map: &HashMap<String, AValue>) -> AValue {
  match val {
    AValue::ARef {
      location,
      type_info,
      history,
    } => AValue::ARef {
      location: Box::new(substitute_aloc(location, param_map)),
      type_info: type_info.clone(),
      history: history.clone(),
    },
    AValue::ASym(expr) => match expr {
      SExpr::SStar(loc) => match &loc.kind {
        ALocKind::AParam(name) => param_map
          .get(name)
          .cloned()
          .unwrap_or_else(|| AValue::ASym(SExpr::SStar(loc.clone()))),
        _ => AValue::ASym(SExpr::SStar(loc.clone())),
      },
      SExpr::SAdd(a, b) => AValue::ASym(SExpr::SAdd(
        Box::new(substitute_value(a, param_map)),
        Box::new(substitute_value(b, param_map)),
      )),
      SExpr::SSub(a, b) => AValue::ASym(SExpr::SSub(
        Box::new(substitute_value(a, param_map)),
        Box::new(substitute_value(b, param_map)),
      )),
      SExpr::SMul(a, b) => AValue::ASym(SExpr::SMul(
        Box::new(substitute_value(a, param_map)),
        Box::new(substitute_value(b, param_map)),
      )),
      SExpr::SEquals(a, b) => AValue::ASym(SExpr::SEquals(
        Box::new(substitute_value(a, param_map)),
        Box::new(substitute_value(b, param_map)),
      )),
      SExpr::SNotEquals(a, b) => AValue::ASym(SExpr::SNotEquals(
        Box::new(substitute_value(a, param_map)),
        Box::new(substitute_value(b, param_map)),
      )),
      SExpr::SLessThan(a, b) => AValue::ASym(SExpr::SLessThan(
        Box::new(substitute_value(a, param_map)),
        Box::new(substitute_value(b, param_map)),
      )),
      SExpr::SGreaterThan(a, b) => AValue::ASym(SExpr::SGreaterThan(
        Box::new(substitute_value(a, param_map)),
        Box::new(substitute_value(b, param_map)),
      )),
      SExpr::SLessThanOrEqual(a, b) => AValue::ASym(SExpr::SLessThanOrEqual(
        Box::new(substitute_value(a, param_map)),
        Box::new(substitute_value(b, param_map)),
      )),
      SExpr::SGreaterThanOrEqual(a, b) => AValue::ASym(SExpr::SGreaterThanOrEqual(
        Box::new(substitute_value(a, param_map)),
        Box::new(substitute_value(b, param_map)),
      )),
      SExpr::SNeg(a) => AValue::ASym(SExpr::SNeg(Box::new(substitute_value(a, param_map)))),
      SExpr::SCond {
        condition,
        then_branch,
        else_branch,
      } => AValue::ASym(SExpr::SCond {
        condition: Box::new(substitute_value(condition, param_map)),
        then_branch: Box::new(substitute_value(then_branch, param_map)),
        else_branch: Box::new(substitute_value(else_branch, param_map)),
      }),
      SExpr::SFieldAccess {
        base_value,
        field_id,
      } => AValue::ASym(SExpr::SFieldAccess {
        base_value: Box::new(substitute_value(base_value, param_map)),
        field_id: field_id.clone(),
      }),
      SExpr::STop => AValue::ASym(SExpr::STop),
    },
    AValue::AStruct { fields } => {
      let mut nf = std::collections::BTreeMap::new();
      for (k, v) in fields {
        nf.insert(k.clone(), substitute_value(v, param_map));
      }
      AValue::AStruct { fields: nf }
    }
    AValue::AArray { elements } => {
      let mut ne = Vec::with_capacity(elements.len());
      for v in elements {
        ne.push(substitute_value(v, param_map));
      }
      AValue::AArray { elements: ne }
    }
    _ => val.clone(),
  }
}

pub fn substitute(sig: &MethodSig, args: &[AValue], mem: &AMem) -> HashMap<ALoc, AValue> {
  let mut param_map: HashMap<String, AValue> = HashMap::new();
  for ((_, name), arg) in sig.params.iter().zip(args.iter()) {
    param_map.insert(name.clone(), arg.clone());
  }

  let mut new_memory = HashMap::new();
  for (k, v) in mem.memory.iter() {
    let new_k = substitute_aloc(k, &param_map);
    let new_v = substitute_value(v, &param_map);
    new_memory.insert(new_k, new_v);
  }
  new_memory
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::engine::domain::{ALoc, ALocKind, AValue};

  #[test]
  fn test_summarize_keeps_param_star_and_drops_local() {
    let mut mem = AMem::new();

    // local(x) -> 10
    let local_x = ALoc {
      kind: ALocKind::ALocal("x".to_string()),
      path: vec![],
    };
    mem.update(local_x.clone(), AValue::AInt(10));

    // *param(x) -> 10
    let star_param_x = ALoc::new_sym_star(ALoc::new_param("x".to_string()));
    mem.update(star_param_x.clone(), AValue::AInt(10));

    summarize(&mut mem);

    assert!(mem.memory.get(&star_param_x).is_some());
    assert!(mem.memory.get(&local_x).is_none());
  }

  #[test]
  fn test_summarize_transitive_ref_and_prune_history() {
    let mut mem = AMem::new();

    // *param(x) -> &heap(xx)
    let star_param_x = ALoc::new_sym_star(ALoc::new_param("x".to_string()));
    let heap_xx = ALoc {
      kind: ALocKind::AHeap("xx".to_string()),
      path: vec![],
    };
    mem.update(
      star_param_x.clone(),
      AValue::ARef {
        location: Box::new(heap_xx.clone()),
        type_info: None,
        history: None,
      },
    );

    // heap(xx) -> 10
    mem.update(heap_xx.clone(), AValue::AInt(10));

    // local(y) -> 20 (should be removed)
    let local_y = ALoc {
      kind: ALocKind::ALocal("y".to_string()),
      path: vec![],
    };
    mem.update(local_y.clone(), AValue::AInt(20));

    // local(z) -> null(n) with tracked history (should be removed along with history)
    let local_z = ALoc {
      kind: ALocKind::ALocal("z".to_string()),
      path: vec![],
    };
    let null_v = AValue::null();
    let null_id = match null_v {
      AValue::ANull { id } => id,
      _ => unreachable!(),
    };
    // Record a creation event for this null to validate pruning
    mem
      .history_registry
      .track_null_creation(null_id, &mem.trace, "null".to_string());
    mem.update(local_z.clone(), null_v);

    summarize(&mut mem);

    // Kept entries
    assert!(mem.memory.get(&star_param_x).is_some());
    assert!(mem.memory.get(&heap_xx).is_some());

    // Removed local-only entries
    assert!(mem.memory.get(&local_y).is_none());
    assert!(mem.memory.get(&local_z).is_none());

    // History for removed null should be pruned
    assert!(mem
      .history_registry
      .get_history(ValueKind::Null, null_id)
      .is_none());
  }

  #[test]
  fn test_summarize_keeps_global_and_this_roots() {
    let mut mem = AMem::new();

    // *global(G) -> &heap(h1)
    let star_global = ALoc::new_sym_star(ALoc {
      kind: ALocKind::AGlobal("G".to_string()),
      path: vec![],
    });
    let heap_h1 = ALoc {
      kind: ALocKind::AHeap("h1".to_string()),
      path: vec![],
    };
    mem.update(
      star_global.clone(),
      AValue::ARef {
        location: Box::new(heap_h1.clone()),
        type_info: None,
        history: None,
      },
    );
    mem.update(heap_h1.clone(), AValue::AInt(1));

    // *this -> ATop
    let star_this = ALoc::new_sym_star(ALoc {
      kind: ALocKind::AThis,
      path: vec![],
    });
    mem.update(star_this.clone(), AValue::top());

    // local(waste) -> 99, should be removed
    let local_waste = ALoc {
      kind: ALocKind::ALocal("waste".to_string()),
      path: vec![],
    };
    mem.update(local_waste.clone(), AValue::AInt(99));

    summarize(&mut mem);

    assert!(mem.memory.get(&star_global).is_some());
    assert!(mem.memory.get(&heap_h1).is_some());
    assert!(mem.memory.get(&star_this).is_some());
    assert!(mem.memory.get(&local_waste).is_none());
  }

  #[test]
  fn test_substitute_param_deref_into_local() {
    // Setup a parametrized summary mem: *(param(pp)) -> null
    let mut mem = AMem::new();
    let star_pp = ALoc::new_sym_star(ALoc::new_param("pp".to_string()));
    let null_v = AValue::null();
    mem.update(star_pp.clone(), null_v.clone());

    // Signature: foo(x int, pp **int)
    let sig = MethodSig {
      name: "foo".to_string(),
      params: vec![
        (crate::il::Type::Int, "x".to_string()),
        (
          crate::il::Type::Pointer(Box::new(crate::il::Type::Pointer(Box::new(
            crate::il::Type::Int,
          )))),
          "pp".to_string(),
        ),
      ],
      id: None,
    };

    // Call args: (10, &local(p))
    let arg_x = AValue::AInt(10);
    let loc_p = ALoc::new_local("p".to_string());
    let arg_pp = AValue::ARef {
      location: Box::new(loc_p.clone()),
      type_info: None,
      history: None,
    };

    substitute(&sig, &[arg_x, arg_pp], &mut mem);

    // After substitution: local(p) -> null
    assert!(mem.memory.get(&loc_p).is_some());
    assert_eq!(mem.memory.get(&loc_p), Some(&null_v));
    // Original star param key should be gone
    assert!(mem.memory.get(&star_pp).is_none());
  }

  #[test]
  fn test_substitute_nested_expression() {
    // mem: *(param(pp)) -> 1 + *param(x)
    let mut mem = AMem::new();
    let star_pp = ALoc::new_sym_star(ALoc::new_param("pp".to_string()));
    let expr = AValue::ASym(SExpr::SAdd(
      Box::new(AValue::AInt(1)),
      Box::new(AValue::ASym(SExpr::SStar(ALoc::new_param("x".to_string())))),
    ));
    mem.update(star_pp.clone(), expr);

    // Signature: foo(x *int, pp **int)
    let sig = MethodSig {
      name: "foo".to_string(),
      params: vec![
        (
          crate::il::Type::Pointer(Box::new(crate::il::Type::Int)),
          "x".to_string(),
        ),
        (
          crate::il::Type::Pointer(Box::new(crate::il::Type::Pointer(Box::new(
            crate::il::Type::Int,
          )))),
          "pp".to_string(),
        ),
      ],
      id: None,
    };

    // Args: (&q, &p)
    let loc_q = ALoc::new_local("q".to_string());
    let arg_x = AValue::ARef {
      location: Box::new(loc_q.clone()),
      type_info: None,
      history: None,
    };
    let loc_p = ALoc::new_local("p".to_string());
    let arg_pp = AValue::ARef {
      location: Box::new(loc_p.clone()),
      type_info: None,
      history: None,
    };

    substitute(&sig, &[arg_x.clone(), arg_pp], &mut mem);

    // Key collapsed to local(p)
    assert!(mem.memory.get(&loc_p).is_some());
    // Value substituted to 1 + &q
    let new_v = mem.memory.get(&loc_p).unwrap();
    match new_v {
      AValue::ASym(SExpr::SAdd(left, right)) => {
        assert_eq!(**left, AValue::AInt(1));
        assert_eq!(&**right, &arg_x);
      }
      v => panic!("Unexpected value after substitution: {:?}", v),
    }
  }
}
