pub mod go_nodes {
  include!(concat!(env!("OUT_DIR"), "/go_nodes.rs"));
}

pub mod convert_utils;
pub mod go;
pub mod source_info;
