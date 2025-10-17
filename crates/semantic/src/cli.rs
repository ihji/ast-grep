use anyhow::Result;
use ast_grep_core::language::Language;
use ast_grep_language::SupportLang;
use clap::Parser;
use std::fs::read_to_string;
use std::path::PathBuf;

use crate::analyze_source;

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

  let findings = analyze_source(content, lang, arg.path.to_str().map(|s| s.to_string()));
  match findings {
    Ok(findings) => {
      if findings.all.is_empty() {
        println!("No issues found.");
      } else {
        println!("Found {} issues.", findings.all.len());
        for finding in &findings.all {
          println!("-----------------------------------------");
          println!("Kind: {:?}", finding.kind());
          println!("Location: {:?}", finding.location());
          println!("Trace: {}", finding.trace());
        }
      }
    }
    Err(e) => {
      eprintln!("Error during analysis: {}", e);
    }
  }
  Ok(())
}
