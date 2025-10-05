use anyhow::Result;
#[cfg(feature = "telemetry")]
use nc_telemetry as telemetry;
#[cfg(feature = "telemetry")]
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct DeploySpec {
    pub target: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeStatus {
    pub running: bool,
}

pub fn deploy(_spec: &DeploySpec) -> Result<()> {
    #[cfg(feature = "telemetry")]
    let app = std::env::var("NC_PROFILE_JSONL")
        .ok()
        .and_then(|p| telemetry::profiling::Appender::open(p).ok());

    #[cfg(feature = "telemetry")]
    let _timer = {
        if let Some(a) = app.as_ref() {
            let mut labels = BTreeMap::new();
            labels.insert("target".to_string(), _spec.target.clone());
            Some(a.start_timer("runtime.deploy_ms", labels))
        } else { None }
    };

    #[cfg(feature = "telemetry")]
    if let Some(a) = &app {
        let mut l = BTreeMap::new();
        l.insert("target".to_string(), _spec.target.clone());
        let _ = a.counter("runtime.deploy_requests", 1.0, l);
    }

    Ok(())
}

pub fn start() -> Result<()> {
    #[cfg(feature = "telemetry")]
    let app = std::env::var("NC_PROFILE_JSONL")
        .ok()
        .and_then(|p| telemetry::profiling::Appender::open(p).ok());
    #[cfg(feature = "telemetry")]
    let _t = app.as_ref().map(|a| {
        let labels = BTreeMap::new();
        a.start_timer("runtime.start_ms", labels)
    });
    Ok(())
}

pub fn stop() -> Result<()> {
    #[cfg(feature = "telemetry")]
    let app = std::env::var("NC_PROFILE_JSONL")
        .ok()
        .and_then(|p| telemetry::profiling::Appender::open(p).ok());
    #[cfg(feature = "telemetry")]
    let _t = app.as_ref().map(|a| {
        let labels = BTreeMap::new();
        a.start_timer("runtime.stop_ms", labels)
    });
    Ok(())
}

pub fn status() -> RuntimeStatus {
    RuntimeStatus { running: false }
}

pub fn version() -> &'static str { "0.0.1" }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lifecycle_stubs_succeed() {
        let spec = DeploySpec { target: "riscv64gcv_linux".to_string() };
        deploy(&spec).expect("deploy ok");
        start().expect("start ok");
        stop().expect("stop ok");
        let s = status();
        assert!(!s.running);
    }
}

pub mod adaptive {
    use super::*;

    #[cfg(feature = "telemetry")]
    use nc_telemetry as telemetry;
    use std::collections::HashSet;
    use std::sync::{Mutex, OnceLock};

    #[derive(Debug, Clone)]
    pub struct ResourceSnapshot {
        pub utilization_pct: f32,
        pub buffer_occupancy_pct: f32,
    }

    impl ResourceSnapshot {
        pub fn new(utilization_pct: f32, buffer_occupancy_pct: f32) -> Self {
            Self { utilization_pct, buffer_occupancy_pct }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Decision {
        NoChange,
        Repartition,
        Reschedule,
        Throttle,
    }

    pub trait Policy {
        fn name(&self) -> &str;
        fn decide(&self, snapshot: &ResourceSnapshot) -> Decision;
    }

    pub struct NoOpPolicy;

    impl Policy for NoOpPolicy {
        fn name(&self) -> &str { "noop-policy" }
        fn decide(&self, _snapshot: &ResourceSnapshot) -> Decision {
            Decision::NoChange
        }
    }

    /// Apply options for runtime decisions.
    #[derive(Debug, Clone, Default)]
    pub struct ApplyOptions {
        /// Optional idempotency key to de-duplicate repeated requests
        pub idempotency_key: Option<String>,
        /// When true, record intent only without applying side effects
        pub dry_run: bool,
    }

    /// Typed error taxonomy for decision application.
    #[derive(Debug)]
    pub enum ApplyError {
        InvalidState(&'static str),
        NotSupported(&'static str),
        Backend(String),
    }

    impl std::fmt::Display for ApplyError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                ApplyError::InvalidState(s) => write!(f, "invalid state: {}", s),
                ApplyError::NotSupported(s) => write!(f, "not supported: {}", s),
                ApplyError::Backend(s) => write!(f, "backend error: {}", s),
            }
        }
    }

    impl std::error::Error for ApplyError {}

    // Simple in-process idempotency registry (thread-safe).
    static IDEM_REGISTRY: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

    fn idem() -> &'static Mutex<HashSet<String>> {
        IDEM_REGISTRY.get_or_init(|| Mutex::new(HashSet::new()))
    }

    /// Returns true if the key has already been seen; otherwise records it and returns false.
    fn register_idem_if_new(key: &str) -> bool {
        let m = idem();
        let mut set = m.lock().expect("idempotency mutex poisoned");
        if set.contains(key) {
            true
        } else {
            set.insert(key.to_string());
            false
        }
    }

    /// Apply a decision to the running system with options.
    pub fn apply_with_options(decision: &Decision, opts: &ApplyOptions) -> Result<()> {
        // Optional telemetry: count decisions with labels
        #[cfg(feature = "telemetry")]
        let app = std::env::var("NC_PROFILE_JSONL")
            .ok()
            .and_then(|p| telemetry::profiling::Appender::open(p).ok());

        #[cfg(feature = "telemetry")]
        if let Some(a) = app.as_ref() {
            let mut labels = std::collections::BTreeMap::new();
            labels.insert("decision".to_string(), format!("{:?}", decision));
            if let Some(k) = &opts.idempotency_key {
                labels.insert("idem".to_string(), k.clone());
            }
            let _ = a.counter("runtime.decisions", 1.0, labels);
        }

        // Dry-run: no side effects beyond optional telemetry above
        if opts.dry_run {
            return Ok(());
        }

        // Idempotency: short-circuit if this key has already been applied.
        if let Some(k) = &opts.idempotency_key {
            if register_idem_if_new(k) {
                return Ok(());
            }
        }

        match decision {
            Decision::NoChange => Ok(()),
            Decision::Repartition => {
                // TODO(H2-5): invoke orchestrator handoff and update partition context
                Ok(())
            }
            Decision::Reschedule => {
                // TODO(H2-5): integrate scheduler to adjust execution ordering
                Ok(())
            }
            Decision::Throttle => {
                // TODO(H2-5): apply rate limiting via backend/runtime shims
                Ok(())
            }
        }
    }

    /// Apply a decision to the running system (defaults: no idempotency key, not dry-run).
    pub fn apply(decision: &Decision) -> Result<()> {
        apply_with_options(decision, &ApplyOptions::default())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn noop_policy_decides_no_change() {
            let p = NoOpPolicy;
            let s = ResourceSnapshot::new(50.0, 10.0);
            assert_eq!(p.decide(&s), Decision::NoChange);
        }

        #[test]
        fn apply_with_options_dry_run_is_ok() {
            let opts = ApplyOptions { idempotency_key: Some("key".into()), dry_run: true };
            apply_with_options(&Decision::Repartition, &opts).expect("dry run ok");
        }

        #[test]
        fn apply_handles_all_decisions_ok() {
            apply(&Decision::NoChange).expect("no change ok");
            apply(&Decision::Repartition).expect("repartition ok");
            apply(&Decision::Reschedule).expect("reschedule ok");
            apply(&Decision::Throttle).expect("throttle ok");
        }
    }
}
