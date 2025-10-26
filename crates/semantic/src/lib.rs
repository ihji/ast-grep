pub mod ast;
pub mod cli;
pub mod engine;
pub mod il;
pub mod naming;
pub mod report;

use std::path::PathBuf;

use crate::{
  engine::{reports::Reports, summary::Summary, sym_exec::execute},
  naming::NamingContext,
};
use ast_grep_core::AstGrep;
use ast_grep_language::SupportLang;
pub use cli::{run_deep_scan, DeepScanArg};
pub use il::{PrettyPrinter, Program, Statement, Value};
use petgraph::dot::{Config, Dot};

use crate::{
  ast::go::GoConverter,
  engine::{context::SessionCtx, trace::TraceArena},
  il::CFGs,
  report::reporter::CliReporter,
};

pub fn analyze_source(
  source: String,
  lang: SupportLang,
  file_path: Option<String>,
) -> anyhow::Result<Reports> {
  // If the language is Go, convert to IL
  if lang == SupportLang::Go {
    let root = AstGrep::new(&source, lang);
    let mut converter = GoConverter::new(PathBuf::from(
      file_path
        .clone()
        .unwrap_or_else(|| "unknown.go".to_string()),
    ));
    converter.convert(&root);
    let mut naming_context = NamingContext::new();
    naming::collect_pgm(&mut naming_context, &mut converter.program);
    println!("Naming Context: {:#?}", naming_context);
    let call_graph = naming::annotate_pgm(&mut naming_context, &mut converter.program);
    println!("IL: {}", &converter.program);
    let mut cfgs = CFGs::new();
    cfgs.insert(converter.program);
    for cfg in &cfgs.0 {
      println!("CFG for method: {}", cfg.1 .0.name);
      println!(
        "{}",
        Dot::with_config(&cfg.1 .1.graph, &[Config::EdgeNoLabel])
      );
    }
    let mut context = SessionCtx {
      /* TODO: Refactor context. */
      root: &root,
      source_info: converter.source_info,
      file_path: file_path.unwrap_or_else(|| "unknown".to_string()),
      trace_arena: TraceArena::new(),
      summary: Summary::new(),
      reporter: Box::new(CliReporter {}),
    };
    let mut findings = execute(&mut context, None, &call_graph, &cfgs);
    findings.finalize(&context);
    Ok(findings)
  } else {
    // For other languages, just print the AST
    println!("unsupported language {}", lang);
    Err(anyhow::anyhow!("unsupported language"))
  }
}
