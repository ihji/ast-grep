use ast_grep_core::{tree_sitter::StrDoc, AstGrep};
use ast_grep_language::SupportLang;

use crate::ast::source_info::SourceInfo;
use crate::report::reporter::Reporter;

pub struct Context<'a> {
  pub root: &'a AstGrep<StrDoc<SupportLang>>,
  pub source_info: SourceInfo<'a>,
  pub file_path: String,
  pub reporter: Box<dyn Reporter>,
}

impl Context<'_> {
  pub fn get_line(&self, tag: usize) -> Option<usize> {
    self
      .source_info
      .get(tag)
      .map(|n| n.start_position().row + 1)
  }
}
