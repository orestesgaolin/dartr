// Dart source: pkg/analyzer/lib/src/lint/analysis_rule_timers.dart
use indexmap::IndexMap;
use std::time::{Duration, Instant};

/// Timers belong to a runner, rather than a process-global mutable registry.
#[derive(Default)]
pub struct AnalysisRuleTimers {
    pub timers: IndexMap<&'static str, RuleTimer>,
}
impl AnalysisRuleTimers {
    pub fn get_timer(&mut self, name: &'static str) -> &mut RuleTimer {
        self.timers.entry(name).or_default()
    }
}
#[derive(Default)]
pub struct RuleTimer {
    elapsed: Duration,
    started: Option<Instant>,
}
impl RuleTimer {
    pub fn start(&mut self) {
        if self.started.is_none() {
            self.started = Some(Instant::now());
        }
    }
    pub fn stop(&mut self) {
        if let Some(start) = self.started.take() {
            self.elapsed += start.elapsed();
        }
    }
    pub fn elapsed(&self) -> Duration {
        self.elapsed + self.started.map_or(Duration::ZERO, |start| start.elapsed())
    }
}
