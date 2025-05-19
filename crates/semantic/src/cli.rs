use anyhow::Result;
use ast_grep_core::language::Language;
use ast_grep_core::AstGrep;
use ast_grep_language::SupportLang;
use clap::Parser;
use std::fs::read_to_string;
use std::path::PathBuf;

use crate::engine::context::Context;
use crate::engine::sym_exec::execute;
use crate::report::reporter::CliReporter;
use crate::{ast::go::GoConverter, il::CFGs};
use petgraph::dot::{Config, Dot};

#[derive(Parser)]
pub struct DeepScanArg {
  /// Path to the file or directory to scan
  #[clap(value_name = "PATH")]
  pub path: PathBuf,

  /// Language of the target file
  #[clap(short, long, value_name = "LANG")]
  pub lang: Option<SupportLang>,
}

pub fn run_deep_scan(arg: DeepScanArg) -> Result<()> {
  // Read the file content
  let content = read_to_string(&arg.path)?;

  // Infer language if not specified
  let lang = if let Some(lang) = arg.lang {
    lang
  } else {
    SupportLang::from_path(&arg.path).ok_or_else(|| {
      anyhow::anyhow!(
        "Could not infer language from file extension. Please specify language with --lang"
      )
    })?
  };

  // If the language is Go, convert to IL
  if lang == SupportLang::Go {
    let root = AstGrep::new(&content, lang);
    let mut converter = GoConverter::new(arg.path.clone());
    converter.convert(&root);
    println!("IL: {}", &converter.program);
    let mut cfgs = CFGs::new();
    cfgs.insert(converter.program);
    for cfg in &cfgs.0 {
      println!("CFG for method: {}", cfg.0.name);
      println!("{}", Dot::with_config(&cfg.1.graph, &[Config::EdgeNoLabel]));
    }
    let context = Context {
      root: &root,
      source_info: converter.source_info,
      file_path: arg.path.to_str().unwrap_or("unknown").to_string(),
      reporter: Box::new(CliReporter {}),
    };
    execute(&context, &cfgs);
  } else {
    // For other languages, just print the AST
    println!("unsupported language {}", lang);
  }

  Ok(())
}
