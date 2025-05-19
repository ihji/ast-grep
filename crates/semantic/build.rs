use std::path::PathBuf;
use std::{env, fs};
use type_sitter_gen::generate_nodes;

fn main() {
  // Common setup. Same as before
  let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
  println!("cargo:rerun-if-changed=build.rs");

  // To generate nodes
  fs::write(
    out_dir.join("go_nodes.rs"),
    generate_nodes(tree_sitter_go::NODE_TYPES)
      .unwrap()
      .into_string(),
  )
  .unwrap();
}
