use crate::engine::findings::Findings;

pub trait Reporter {
  fn report(&self, findings: &Findings);
}

pub struct CliReporter;

impl Reporter for CliReporter {
  fn report(&self, findings: &Findings) {
    for finding in &findings.all {
      println!("Finding: {}", finding.message());
      println!("Location: {:?}", finding.location());
      println!("Trace: {:?}", finding.trace());
    }
  }
}
