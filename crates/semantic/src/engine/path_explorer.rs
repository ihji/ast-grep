use std::collections::HashSet;

use bitvec::prelude::*;

pub trait PathExplorer {
  fn next(&mut self) -> bool;
  fn next_path(&mut self) -> bool;
  fn is_done(&self) -> bool;
  fn mark_done(&mut self);
}

pub struct DumbPathExplorer {
  total_paths: i32,
  path_seed: i32,
  current_path_index: i32,
  done: bool,
  explored_paths: HashSet<BitVec>,
}

impl DumbPathExplorer {
  pub fn new(total_paths: i32) -> Self {
    Self {
      total_paths,
      path_seed: 0,
      current_path_index: 0,
      done: false,
      explored_paths: HashSet::new(),
    }
  }
}

impl PathExplorer for DumbPathExplorer {
  fn mark_done(&mut self) {
    if self.done {
      return;
    }
    let mut explored = BitVec::new();
    for i in 0..self.current_path_index {
      explored.push(((self.path_seed >> i) & 1) == 1);
    }
    println!(
      "Explored path: [seed] {}, [index] {}, {:?}",
      self.path_seed, self.current_path_index, explored
    );
    self.explored_paths.insert(explored);
    self.done = true;
  }
  fn next(&mut self) -> bool {
    let ret = if self.current_path_index >= 32 {
      0
    } else {
      (self.path_seed >> self.current_path_index) & 1
    };
    self.current_path_index += 1;
    if ret == 0 {
      true
    } else {
      false
    }
  }
  fn next_path(&mut self) -> bool {
    if self.explored_paths.len() as i32 >= self.total_paths {
      return false;
    }
    for _ in 0..100 {
      self.path_seed += 1;
      let mut found = false;
      for explored in &self.explored_paths {
        let mut is_prefix = true;
        for (i, bit) in explored.iter().enumerate() {
          let path_seed_bit = ((self.path_seed >> i) & 1) == 1;
          if *bit != path_seed_bit {
            is_prefix = false;
            break;
          }
        }
        if is_prefix {
          found = true;
          break;
        }
      }
      if !found {
        self.current_path_index = 0;
        self.done = false;
        return true;
      }
    }
    println!("Failed to find next path after 100 tries");
    false
  }
  fn is_done(&self) -> bool {
    self.done
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_dumb_path_explorer_simple_flow() {
    let mut explorer = DumbPathExplorer::new(10);
    assert!(!explorer.is_done());
    assert_eq!(explorer.next(), true);
    explorer.mark_done();
    assert!(explorer.is_done());

    assert!(explorer.next_path());
    assert!(!explorer.is_done());
    assert_eq!(explorer.next(), false);
    assert_eq!(explorer.next(), true);
    explorer.mark_done();
    assert!(explorer.is_done());

    assert!(explorer.next_path());
    assert!(!explorer.is_done());
    assert_eq!(explorer.next(), false);
    assert_eq!(explorer.next(), false);
    explorer.mark_done();
    assert!(explorer.is_done());
  }

  #[test]
  fn test_dumb_path_explorer_next_path() {
    let mut explorer = DumbPathExplorer::new(10);
    // path_seed = 0, initial path
    assert_eq!(explorer.next(), true);
    assert_eq!(explorer.next(), true);
    assert_eq!(explorer.next(), true);
    explorer.mark_done(); // explored_paths will contain {0} for path 000...
    assert!(explorer.next_path()); // should find path_seed = 1
                                   // path_seed = 1, next path
    assert_eq!(explorer.next(), false);
    assert_eq!(explorer.next(), true);
    assert_eq!(explorer.next(), true);
  }
}
