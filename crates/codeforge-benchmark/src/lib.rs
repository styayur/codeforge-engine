//! Benchmark execution and statistics. Results are only emitted from measured runs.

use std::collections::BTreeMap;

use codeforge_protocol::{BenchmarkResult, TimingSummary};
use codeforge_verification::{CommandSpec, VerificationError, VerificationPipeline};

#[derive(Debug, thiserror::Error)]
pub enum BenchmarkError {
    #[error(transparent)]
    Verification(#[from] VerificationError),
    #[error("benchmark {0} exited unsuccessfully: exit={1:?}")]
    CommandFailed(String, Option<i32>),
    #[error("benchmark command timed out: {0}")]
    Timeout(String),
    #[error("benchmark requires at least one sample")]
    NoSamples,
    #[error("cannot calculate delta because the baseline median is zero")]
    ZeroBaseline,
}

#[derive(Debug, Clone)]
pub struct BenchmarkRunner {
    warmup: usize,
    samples: usize,
}

impl Default for BenchmarkRunner {
    fn default() -> Self {
        Self {
            warmup: 1,
            samples: 5,
        }
    }
}

impl BenchmarkRunner {
    pub fn new(warmup: usize, samples: usize) -> Result<Self, BenchmarkError> {
        if samples == 0 {
            return Err(BenchmarkError::NoSamples);
        }
        Ok(Self { warmup, samples })
    }

    pub async fn measure(&self, spec: CommandSpec) -> Result<TimingSummary, BenchmarkError> {
        let pipeline = VerificationPipeline;
        for _ in 0..self.warmup {
            let result = pipeline.run_command(spec.clone()).await?;
            if result.timed_out {
                return Err(BenchmarkError::Timeout(spec.label));
            }
            if !result.success() {
                return Err(BenchmarkError::CommandFailed(spec.label, result.exit_code));
            }
        }

        let mut samples = Vec::with_capacity(self.samples);
        for _ in 0..self.samples {
            let result = pipeline.run_command(spec.clone()).await?;
            if result.timed_out {
                return Err(BenchmarkError::Timeout(spec.label));
            }
            if !result.success() {
                return Err(BenchmarkError::CommandFailed(spec.label, result.exit_code));
            }
            samples.push(result.elapsed.as_secs_f64() * 1000.0);
        }
        Ok(TimingSummary::from_samples("milliseconds", samples))
    }

    pub async fn compare(
        &self,
        metric: impl Into<String>,
        before_command: CommandSpec,
        after_command: CommandSpec,
    ) -> Result<BenchmarkResult, BenchmarkError> {
        let command = Some(before_command.command_line());
        let before = self.measure(before_command).await?;
        let after = self.measure(after_command).await?;
        if before.median == 0.0 {
            return Err(BenchmarkError::ZeroBaseline);
        }
        let delta_percent = ((before.median - after.median) / before.median) * 100.0;
        Ok(BenchmarkResult {
            metric: metric.into(),
            before,
            after,
            delta_percent,
            warmup: self.warmup,
            samples: self.samples,
            command,
            environment: environment(),
            timestamp: chrono::Utc::now(),
        })
    }
}

pub fn environment() -> BTreeMap<String, String> {
    let mut values = BTreeMap::new();
    values.insert("os".to_owned(), std::env::consts::OS.to_owned());
    values.insert("arch".to_owned(), std::env::consts::ARCH.to_owned());
    values.insert("family".to_owned(), std::env::consts::FAMILY.to_owned());
    if let Ok(cpus) = std::thread::available_parallelism() {
        values.insert("logical_cpus".to_owned(), cpus.get().to_string());
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn measures_real_process_samples() {
        let spec = CommandSpec::new(
            "rustc",
            ["--version"],
            std::env::current_dir().expect("cwd"),
        )
        .timeout(Duration::from_secs(10));
        let runner = BenchmarkRunner::new(0, 2).expect("runner");
        let summary = runner.measure(spec).await.expect("measure");
        assert_eq!(summary.samples.len(), 2);
        assert!(summary.median >= 0.0);
    }
}
