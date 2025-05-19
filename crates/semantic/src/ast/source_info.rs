use std::{
  collections::HashMap,
  sync::atomic::{AtomicUsize, Ordering},
};

use type_sitter::raw::Node;

pub struct SourceInfo<'a> {
  node_map: HashMap<usize, Node<'a>>,
}

static KEY_COUNTER: AtomicUsize = AtomicUsize::new(0);

impl<'a> SourceInfo<'a> {
  pub fn new() -> Self {
    Self {
      node_map: HashMap::new(),
    }
  }

  pub fn register(&mut self, node: Node<'a>) -> usize {
    let counter = KEY_COUNTER.fetch_add(1, Ordering::Relaxed);
    self.node_map.insert(counter, node);
    counter
  }

  pub fn get(&self, id: usize) -> Option<&Node<'a>> {
    self.node_map.get(&id)
  }
}
