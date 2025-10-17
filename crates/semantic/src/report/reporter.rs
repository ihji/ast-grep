use crate::engine::{context::SessionCtx, reports::Reports};

pub trait Reporter {
  fn report(&self, ctx: &SessionCtx, findings: &mut Reports);
}

pub struct CliReporter;

impl Reporter for CliReporter {
  fn report(&self, ctx: &SessionCtx, findings: &mut Reports) {
    findings.finalize(ctx);
    for finding in &findings.all {
      println!("-----------------------------------------");
      println!("Kind: {:?}", finding.kind());
      println!("Location: {:?}", finding.location());
      println!("Trace: {}", finding.trace());
    }
  }
}
