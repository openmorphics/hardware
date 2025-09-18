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
    cmd.args(&[
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
}

#[test]
fn simulate_smoke() {
    let mut cmd = bin();
    cmd.args(&["simulate", "--simulator", "neuron"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("simulate"));
}

#[test]
fn lower_with_target_manifest() {
    let mut cmd = bin();
    cmd.args(&[
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