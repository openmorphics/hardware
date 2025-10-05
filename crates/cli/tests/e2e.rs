use assert_cmd::Command;
use predicates::prelude::*;

fn bin() -> Command {
    Command::cargo_bin("neuro-compiler").expect("binary exists")
}

#[test]
fn list_targets_runs() {
    let mut cmd = bin();
    cmd.arg("list-targets");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("loihi2"));
}

#[test]
fn import_simple_json_validates() {
    use std::path::PathBuf;
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let crate_dir = PathBuf::from(manifest_dir);
    let ws_root = crate_dir.parent().and_then(|p| p.parent()).expect("ws root");
    let input = ws_root.join("examples/nir/simple.json");

    let mut cmd = bin();
    cmd.arg("import").arg("--input").arg(&input);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("import ok: name="));
}

#[test]
fn lower_with_mapping_passes_and_dumps() {
    // Run lower with our new pipeline passes and request dump artifacts
    let mut cmd = bin();
    cmd.args([
        "lower",
        "--pipeline",
        "validate,partition,placement,routing,timing,resource-check",
        "--dump-dir",
        "target/test-dumps",
        "--dump-format",
        "json,yaml",
    ]);
    cmd.assert()
        .success()
        .stdout(
            predicate::str::contains("lower completed")
        );
    // Verify orchestrator→passes handoff metadata exists in partition dump (best-effort).
    {
        use std::fs;
        if let Ok(data) = fs::read_to_string("target/test-dumps/01_partition.json") {
            assert!(data.contains("\"orchestrator_plan\""), "expected orchestrator_plan metadata in partition dump");
        }
    }
}

#[cfg(feature = "sim-neuron")]
#[test]
fn simulate_smoke() {
    use std::path::PathBuf;
    use std::fs;
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let crate_dir = PathBuf::from(manifest_dir);
    let ws_root = crate_dir.parent().and_then(|p| p.parent()).expect("ws root");
    let input = ws_root.join("examples/nir/simple.json");
    let out_dir = PathBuf::from("target/sim-neuron-out");

    let mut cmd = bin();
    cmd.args([
        "simulate",
        "--simulator", "neuron",
        "--input", input.to_str().unwrap(),
        "--out-dir", out_dir.to_str().unwrap(),
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("simulate artifacts written to"));
    // Verify artifacts exist when feature is enabled
    let _ = fs::metadata(out_dir.join("RUN.txt")).expect("expected RUN.txt when sim-neuron enabled");
}

#[cfg(not(feature = "sim-neuron"))]
#[test]
fn simulate_smoke_disabled() {
    use std::path::PathBuf;
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let crate_dir = PathBuf::from(manifest_dir);
    let ws_root = crate_dir.parent().and_then(|p| p.parent()).expect("ws root");
    let input = ws_root.join("examples/nir/simple.json");
    let out_dir = PathBuf::from("target/sim-neuron-out");

    let mut cmd = bin();
    cmd.args([
        "simulate",
        "--simulator", "neuron",
        "--input", input.to_str().unwrap(),
        "--out-dir", out_dir.to_str().unwrap(),
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("simulate disabled: feature 'sim-neuron' not enabled"));
}

#[cfg(feature = "telemetry-otlp")]
#[test]
fn otlp_init_smoke_lower() {
    use std::path::PathBuf;
    let mut cmd = bin();
    cmd.env("NC_OTLP_ENDPOINT", "http://localhost:4317");
    cmd.args(&[
        "lower",
        "--pipeline", "validate",
        "--dump-dir", "target/test-dumps-otlp",
        "--dump-format", "json",
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("lower completed"));
}

#[test]
fn lower_with_target_manifest() {
    let mut cmd = bin();
    cmd.args([
        "lower",
        "--pipeline",
        "validate,partition,placement,resource-check",
        "--dump-dir",
        "target/test-dumps-target",
        "--dump-format",
        "json",
        "--target",
        "loihi2",
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("lower completed"));
}

#[cfg(feature = "sim-neuron")]
#[test]
fn simulate_with_profile_jsonl_smoke() {
    use std::path::PathBuf;
    use std::fs;

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let crate_dir = PathBuf::from(manifest_dir);
    let ws_root = crate_dir.parent().and_then(|p| p.parent()).expect("ws root");
    let input = ws_root.join("examples/nir/simple.json");
    let out_dir = PathBuf::from("target/sim-neuron-out");
    let profile = PathBuf::from("target/sim-prof.jsonl");
    let _ = fs::remove_file(&profile);

    let mut cmd = bin();
    cmd.args([
        "simulate",
        "--simulator", "neuron",
        "--input", input.to_str().expect("input path"),
        "--out-dir", out_dir.to_str().expect("out path"),
        "--profile-jsonl", profile.to_str().expect("profile path"),
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("simulate artifacts written to"));

    #[cfg(feature = "telemetry")]
    {
        // With telemetry enabled, profile JSONL should exist and be non-empty
        if let Ok(data) = fs::read_to_string(&profile) {
            assert!(!data.trim().is_empty(), "profile JSONL exists but is empty");
        } else {
            panic!("expected profile JSONL when telemetry is enabled");
        }
    }
}

#[cfg(not(feature = "sim-neuron"))]
#[test]
fn simulate_with_profile_jsonl_smoke_disabled() {
    use std::path::PathBuf;

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let crate_dir = PathBuf::from(manifest_dir);
    let ws_root = crate_dir.parent().and_then(|p| p.parent()).expect("ws root");
    let input = ws_root.join("examples/nir/simple.json");
    let out_dir = PathBuf::from("target/sim-neuron-out");
    let profile = PathBuf::from("target/sim-prof.jsonl");

    let mut cmd = bin();
    cmd.args([
        "simulate",
        "--simulator", "neuron",
        "--input", input.to_str().expect("input path"),
        "--out-dir", out_dir.to_str().expect("out path"),
        "--profile-jsonl", profile.to_str().expect("profile path"),
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("simulate disabled: feature 'sim-neuron' not enabled"));
}

#[test]
fn lower_profile_jsonl_smoke() {
    use std::path::PathBuf;
    use std::fs;

    let profile = PathBuf::from("target/lower-prof.jsonl");
    let _ = fs::remove_file(&profile);

    let mut cmd = bin();
    cmd.env("NC_PROFILE_JSONL", profile.to_str().expect("profile path"));
    cmd.args([
        "lower",
        "--pipeline", "validate,partition,placement",
        "--dump-dir", "target/test-dumps-telemetry",
        "--dump-format", "json",
    ]);
    // Pipeline should run; JSONL emission occurs only when built with telemetry
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("lower completed"));

    if let Ok(data) = fs::read_to_string(&profile) {
        assert!(!data.trim().is_empty(), "lower profiling JSONL exists but is empty");
        assert!(data.contains("metric"), "expected 'metric' field in JSONL");
    }
}

#[cfg(feature = "telemetry")]
#[test]
fn lower_profile_jsonl_labels_schema() {
    use std::fs;
    use std::path::PathBuf;

    // Prepare JSONL path and ensure a clean slate
    let profile = PathBuf::from("target/lower-prof-labels.jsonl");
    let _ = fs::remove_file(&profile);

    // Run the lower pipeline with telemetry JSONL enabled via env var
    let mut cmd = bin();
    cmd.env("NC_PROFILE_JSONL", profile.to_str().expect("profile path"));
    cmd.args([
        "lower",
        "--pipeline", "validate,partition,placement",
        "--dump-dir", "target/test-dumps-telemetry-labels",
        "--dump-format", "json",
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("lower completed"));

    // If telemetry feature was actually compiled in, the JSONL should exist.
    // When telemetry is disabled in the build, this test compiles out via cfg(feature="telemetry").
    if let Ok(data) = fs::read_to_string(&profile) {
        assert!(!data.trim().is_empty(), "profile JSONL exists but is empty");
        // Sanity: at least one record with metric + labels.graph + labels.pass
        let mut found_schema = false;
        for line in data.lines().filter(|l| !l.trim().is_empty()) {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                if v.get("metric").is_some() {
                    if let Some(lbl) = v.get("labels").and_then(|x| x.as_object()) {
                        if lbl.contains_key("graph") && lbl.contains_key("pass") {
                            found_schema = true;
                            break;
                        }
                    }
                }
            }
        }
        assert!(found_schema, "expected at least one JSONL record with labels.graph and labels.pass");
    }
}

#[cfg(feature = "backend-riscv")]
#[test]
fn riscv_compile_smoke_no_qemu() {
    use std::path::PathBuf;
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let crate_dir = PathBuf::from(manifest_dir);
    let ws_root = crate_dir.parent().and_then(|p| p.parent()).expect("ws root");
    let input = ws_root.join("examples/nir/simple.json");

    let mut cmd = bin();
    cmd.args([
        "compile",
        "--input", input.to_str().expect("input path"),
        "--target", "riscv64gcv_linux",
    ]);
    let pred = predicate::str::contains("compile ok").and(predicate::str::contains("artifact:"));
    cmd.assert()
        .success()
        .stdout(pred);
}


#[test]
fn package_creates_artifacts() {
    use std::fs;
    use std::path::PathBuf;

    let out = PathBuf::from("target/pkg-e2e");
    let _ = fs::remove_dir_all(&out);

    let mut cmd = bin();
    cmd.args([
        "package",
        "--output",
        out.to_str().expect("out path"),
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("package created at"));

    let pkg = out.join("PKG.txt");
    let meta = fs::read_to_string(&pkg).expect("PKG.txt exists");
    assert!(meta.contains("neuro-compiler package"), "unexpected PKG.txt contents: {}", meta);
}

#[test]
fn deploy_smoke() {
    let mut cmd = bin();
    cmd.args([
        "deploy",
        "--target",
        "loihi2",
    ]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("deploy ok: target=loihi2"));
}
