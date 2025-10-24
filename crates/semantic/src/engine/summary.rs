use std::collections::{HashSet, VecDeque};

use crate::engine::domain::{ALoc, ALocKind, AMem, AValue, SExpr, Seg};
use crate::engine::history::{ValueId, ValueKind};

fn is_param_star_loc(loc: &ALoc) -> bool {
  match &loc.kind {
    ALocKind::ASymStar(inner) => matches!(inner.kind, ALocKind::AParam(_)),
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
  // Build seeds from parameter pointer locations: *(param(...))
  let mut keys_to_keep: HashSet<ALoc> = HashSet::new();
  let mut queue_keys: VecDeque<ALoc> = VecDeque::new();

  for key in mem.memory.keys() {
    if is_param_star_loc(key) {
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
}
