use crate::engine::{context::SessionCtx, findings::Findings};

pub trait Reporter {
  fn report(&self, ctx: &SessionCtx, findings: &Findings);
}

pub struct CliReporter;

impl Reporter for CliReporter {
  fn report(&self, ctx: &SessionCtx, findings: &Findings) {
    for finding in &findings.all {
      println!("-----------------------------------------");
      println!("Finding: {}", finding.message());
      println!("Location: {:?}", finding.location());
      println!(
        "Trace: {}",
        finding.trace().fmt_with(ctx, finding.history())
      );
    }
  }
}
