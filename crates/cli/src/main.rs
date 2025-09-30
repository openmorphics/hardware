use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
use std::fs;
#[cfg(feature = "telemetry")]
use std::collections::BTreeMap;

#[derive(Parser)]
#[command(name = "neuro-compiler")]
#[command(about = "Universal neuromorphic compiler (skeleton)")]
struct Cli {
    /// Optional OTLP endpoint (overrides NC_OTLP_ENDPOINT) when built with 'telemetry-otlp'
    #[arg(global = true, long)]
    otlp_endpoint: Option<String>,
    /// Optional JSONL profile path (sets NC_PROFILE_JSONL if not set)
    #[arg(global = true, long)]
    profile_jsonl: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// List builtin target names
    ListTargets,
    /// Import a model from a given frontend/framework
    Import(ImportArgs),
    /// Run lowering passes on an input IR/model
    Lower(LowerArgs),
    /// Compile a model to a target backend
    Compile(CompileArgs),
    /// Simulate a model on a specified simulator
    Simulate(SimulateArgs),
    /// Profile a compiled or simulated run
    Profile(ProfileArgs),
    /// Package artifacts for deployment
    Package(PackageArgs),
    /// Deploy to hardware or cluster runtime
    Deploy(DeployArgs),
    /// Export NIR as MLIR (requires 'mlir' feature)
    ExportMlir(ExportMlirArgs),
}

#[derive(Args, Debug)]
struct ImportArgs {
    /// Input file path
    #[arg(long)]
    input: PathBuf,
    /// Frontend/framework name (e.g., pynn, nengo, nest, brian, bindsnet, carlsim, genn, rockpool)
    #[arg(long)]
    framework: Option<String>,
    /// Optional format override (e.g., py, json, yaml)
    #[arg(long)]
    format: Option<String>,
}

#[derive(Args, Debug)]
struct LowerArgs {
    /// Pipeline name or comma-separated list of passes (e.g., noop)
    #[arg(long)]
    pipeline: Option<String>,
    /// Directory to dump intermediate artifacts (JSON/YAML/BIN)
    #[arg(long)]
    dump_dir: Option<PathBuf>,
    /// One or more dump formats: json, yaml, bin (repeat or comma-separated)
    #[arg(long = "dump-format", value_delimiter = ',')]
    dump_format: Vec<String>,
    /// Optional explicit target manifest TOML path (preferred when provided)
    #[arg(long)]
    manifest: Option<PathBuf>,
    /// Optional builtin target name (fallback convenience to load targets/<name>.toml)
    #[arg(long)]
    target: Option<String>,
}

#[derive(Args, Debug)]
struct CompileArgs {
    /// Input NIR file (JSON or YAML)
    #[arg(long)]
    input: PathBuf,
    /// Target backend (e.g., loihi2, akida, spinnaker2)
    #[arg(long)]
    target: String,
}

#[derive(Args, Debug)]
struct SimulateArgs {
    /// Simulator (e.g., neuron, coreneuron, arbor, hw)
    #[arg(long)]
    simulator: String,
    /// Input NIR file (JSON or YAML)
    #[arg(long)]
    input: PathBuf,
    /// Output directory to write simulator artifacts
    #[arg(long)]
    out_dir: Option<PathBuf>,
    /// Optional JSONL telemetry output path (requires feature 'telemetry')
    #[arg(long)]
    profile_jsonl: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct ProfileArgs {
    /// Path to run directory or results
    #[arg(long)]
    input: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct PackageArgs {
    /// Output artifact path
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Args, Debug)]
struct DeployArgs {
    /// Target backend/hardware or cluster
    #[arg(long)]
    target: String,
}

#[derive(Args, Debug)]
struct ExportMlirArgs {
    /// Input NIR file (JSON or YAML)
    #[arg(long)]
    input: PathBuf,
}

fn main() {
    let cli = Cli::parse();
    let _ = tracing_subscriber::fmt().with_env_filter("info").try_init();

    // If provided globally, set NC_PROFILE_JSONL unless already set (works across subcommands)
    if let Some(p) = &cli.profile_jsonl {
        if std::env::var("NC_PROFILE_JSONL").is_err() {
            if let Some(s) = p.to_str() {
                std::env::set_var("NC_PROFILE_JSONL", s);
            }
        }
    }

    // Initialize OpenTelemetry exporter if compiled with feature "telemetry-otlp"
    #[cfg(feature = "telemetry-otlp")]
    {
        let endpoint = cli.otlp_endpoint.clone().or_else(|| std::env::var("NC_OTLP_ENDPOINT").ok());
        let _ = nc_telemetry::init_otel(endpoint.as_deref());
    }

    match cli.command {
        Some(Command::ListTargets) => {
            for t in nc_hal::builtin_targets() {
                println!("{t}");
            }
        }
        Some(Command::Import(args)) => {
            // Detect format either from --format or file extension
            let fmt = args
                .format
                .as_deref()
                .map(|s| s.to_lowercase())
                .or_else(|| args.input.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase()));

            let data = match fs::read_to_string(&args.input) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("import: cannot read {:?}: {e}", args.input);
                    return;
                }
            };

            let parsed = match fmt.as_deref() {
                Some("yaml") | Some("yml") => nc_nir::Graph::from_yaml_str(&data).map_err(|e| e.to_string()),
                _ => nc_nir::Graph::from_json_str(&data).map_err(|e| e.to_string()),
            };

            match parsed {
                Ok(g) => {
                    let valid = g.validate().is_ok();
                    println!(
                        "import ok: name={} populations={} connections={} probes={} valid={}",
                        g.name,
                        g.populations.len(),
                        g.connections.len(),
                        g.probes.len(),
                        valid
                    );
                }
                Err(e) => {
                    eprintln!("import parse error: {e}");
                }
            }
        }
        Some(Command::Lower(args)) => {
            // Structured pipeline config and dump formats
            let names: Vec<String> = args
                .pipeline
                .as_deref()
                .unwrap_or("noop")
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            let mut fmt: Vec<nc_passes::DumpFormat> = Vec::new();
            for f in args.dump_format.iter().map(|s| s.to_lowercase()) {
                match f.as_str() {
                    "json" => fmt.push(nc_passes::DumpFormat::Json),
                    "yaml" => fmt.push(nc_passes::DumpFormat::Yaml),
                    "bin" => {
                        #[cfg(feature = "bin-artifacts")]
                        {
                            fmt.push(nc_passes::DumpFormat::Bin);
                        }
                    }
                    _ => {}
                }
            }

            let cfg = nc_passes::PipelineConfig {
                passes: names.clone(),
                dump_dir: args.dump_dir.clone(),
                dump_formats: if fmt.is_empty() { vec![nc_passes::DumpFormat::Json] } else { fmt },
            };

            // Build a trivial graph and run through the pipeline with dumps
            let mut g = nc_nir::Graph::new("cli-lower-stub");
            // If provided, load a HAL manifest via --manifest or --target and attach its path for capability-aware passes
            let manifest_path: Option<PathBuf> = args
                .manifest
                .clone()
                .or_else(|| args.target.as_ref().map(|t| PathBuf::from(format!("targets/{t}.toml"))));
            if let Some(mp) = manifest_path {
                match nc_hal::parse_target_manifest_path(&mp) {
                    Ok(m) => {
                        if let Err(e) = nc_hal::validate_manifest(&m) {
                            eprintln!("lower: manifest invalid: {e}");
                        } else {
                            g.attributes.insert(
                                "hal_manifest_path".to_string(),
                                serde_json::json!(mp.to_string_lossy().to_string()),
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("lower: cannot load manifest {mp:?}: {e}");
                    }
                }
            }
            let mut pm = nc_passes::PassManager::new();
            if let Err(e) = nc_passes::build_pipeline(&mut pm, &cfg.passes) {
                eprintln!("error: {e}");
            }
            match pm.run_with_config(g, &cfg) {
                Ok(_) => {
                    if let Some(dir) = cfg.dump_dir {
                        println!("lower completed; artifacts dumped under {dir:?}");
                    } else {
                        println!("lower completed; no dump_dir specified");
                    }
                }
                Err(e) => eprintln!("lower failed: {e}"),
            }
        }
        Some(Command::Compile(args)) => {
            // Determine input format by extension and parse NIR
            let fmt = args.input.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase());
            let data = match fs::read_to_string(&args.input) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("compile: cannot read {:?}: {e}", args.input);
                    return;
                }
            };
            let mut g = match fmt.as_deref() {
                Some("yaml") | Some("yml") => match nc_nir::Graph::from_yaml_str(&data) {
                    Ok(g) => g,
                    Err(e) => {
                        eprintln!("compile: parse yaml failed: {e}");
                        return;
                    }
                },
                _ => match nc_nir::Graph::from_json_str(&data) {
                    Ok(g) => g,
                    Err(e) => {
                        eprintln!("compile: parse json failed: {e}");
                        return;
                    }
                },
            };
            if let Err(e) = g.validate() {
                eprintln!("compile: validation failed: {e}");
                return;
            }
            g.ensure_version_tag();

            // Load target manifest
            let manifest_path = PathBuf::from(format!("targets/{}.toml", args.target));
            let manifest = match nc_hal::parse_target_manifest_path(&manifest_path) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("compile: cannot load manifest {manifest_path:?}: {e}");
                    return;
                }
            };
            if let Err(e) = nc_hal::validate_manifest(&manifest) {
                eprintln!("compile: manifest invalid: {e}");
                return;
            }

            match args.target.as_str() {
                "loihi2" => {
                    #[cfg(feature = "backend-loihi")]
                    {
                        match nc_backend_loihi::compile(&g, &manifest) {
                            Ok(art) => println!("compile ok: {}", art),
                            Err(e) => eprintln!("compile error: {e}"),
                        }
                    }
                    #[cfg(not(feature = "backend-loihi"))]
                    {
                        eprintln!("backend 'backend-loihi' is not enabled; rebuild CLI with --features backend-loihi");
                    }
                }
                "truenorth" => {
                    #[cfg(feature = "backend-truenorth")]
                    {
                        match nc_backend_truenorth::compile(&g, &manifest) {
                            Ok(art) => println!("compile ok: {}", art),
                            Err(e) => eprintln!("compile error: {e}"),
                        }
                    }
                    #[cfg(not(feature = "backend-truenorth"))]
                    {
                        eprintln!("backend 'backend-truenorth' is not enabled; rebuild CLI with --features backend-truenorth");
                    }
                }
                "akida" => {
                    #[cfg(feature = "backend-akida")]
                    {
                        match nc_backend_akida::compile(&g, &manifest) {
                            Ok(art) => println!("compile ok: {}", art),
                            Err(e) => eprintln!("compile error: {e}"),
                        }
                    }
                    #[cfg(not(feature = "backend-akida"))]
                    {
                        eprintln!("backend 'backend-akida' is not enabled; rebuild CLI with --features backend-akida");
                    }
                }
                "spinnaker2" => {
                    #[cfg(feature = "backend-spinnaker")]
                    {
                        match nc_backend_spinnaker::compile(&g, &manifest) {
                            Ok(art) => println!("compile ok: {}", art),
                            Err(e) => eprintln!("compile error: {e}"),
                        }
                    }
                    #[cfg(not(feature = "backend-spinnaker"))]
                    {
                        eprintln!("backend 'backend-spinnaker' is not enabled; rebuild CLI with --features backend-spinnaker");
                    }
                }
                "neurogrid" => {
                    #[cfg(feature = "backend-neurogrid")]
                    {
                        match nc_backend_neurogrid::compile(&g, &manifest) {
                            Ok(art) => println!("compile ok: {}", art),
                            Err(e) => eprintln!("compile error: {e}"),
                        }
                    }
                    #[cfg(not(feature = "backend-neurogrid"))]
                    {
                        eprintln!("backend 'backend-neurogrid' is not enabled; rebuild CLI with --features backend-neurogrid");
                    }
                }
                "dynaps" => {
                    #[cfg(feature = "backend-dynaps")]
                    {
                        match nc_backend_dynaps::compile(&g, &manifest) {
                            Ok(art) => println!("compile ok: {}", art),
                            Err(e) => eprintln!("compile error: {e}"),
                        }
                    }
                    #[cfg(not(feature = "backend-dynaps"))]
                    {
                        eprintln!("backend 'backend-dynaps' is not enabled; rebuild CLI with --features backend-dynaps");
                    }
                }
                "memxbar" => {
                    #[cfg(feature = "backend-memxbar")]
                    {
                        match nc_backend_memxbar::compile(&g, &manifest) {
                            Ok(art) => println!("compile ok: {}", art),
                            Err(e) => eprintln!("compile error: {e}"),
                        }
                    }
                    #[cfg(not(feature = "backend-memxbar"))]
                    {
                        eprintln!("backend 'backend-memxbar' is not enabled; rebuild CLI with --features backend-memxbar");
                    }
                }
                "custom_asic" => {
                    #[cfg(feature = "backend-custom-asic")]
                    {
                        match nc_backend_custom_asic::compile(&g, &manifest) {
                            Ok(art) => println!("compile ok: {}", art),
                            Err(e) => eprintln!("compile error: {e}"),
                        }
                    }
                    #[cfg(not(feature = "backend-custom-asic"))]
                    {
                        eprintln!("backend 'backend-custom-asic' is not enabled; rebuild CLI with --features backend-custom-asic");
                    }
                }
                "riscv64gcv_linux" => {
                    #[cfg(feature = "backend-riscv")]
                    {
                        match nc_backend_riscv::compile(&g, &manifest) {
                            Ok(art) => println!("compile ok: {}", art),
                            Err(e) => eprintln!("compile error: {e}"),
                        }
                    }
                    #[cfg(not(feature = "backend-riscv"))]
                    {
                        eprintln!("backend 'backend-riscv' is not enabled; rebuild CLI with --features backend-riscv");
                    }
                }
                "riscv32imac_bare" => {
                    #[cfg(feature = "backend-riscv")]
                    {
                        match nc_backend_riscv::compile(&g, &manifest) {
                            Ok(art) => println!("compile ok: {}", art),
                            Err(e) => eprintln!("compile error: {e}"),
                        }
                    }
                    #[cfg(not(feature = "backend-riscv"))]
                    {
                        eprintln!("backend 'backend-riscv' is not enabled; rebuild CLI with --features backend-riscv");
                    }
                }
                "riscv64gc_ctrl" => {
                    #[cfg(feature = "backend-riscv")]
                    {
                        match nc_backend_riscv::compile(&g, &manifest) {
                            Ok(art) => println!("compile ok: {}", art),
                            Err(e) => eprintln!("compile error: {e}"),
                        }
                    }
                    #[cfg(not(feature = "backend-riscv"))]
                    {
                        eprintln!("backend 'backend-riscv' is not enabled; rebuild CLI with --features backend-riscv");
                    }
                }
                other => {
                    eprintln!("compile: unsupported or not yet integrated target '{other}'");
                }
            }
        }
        Some(Command::Simulate(args)) => {
            // Parse input NIR
            let fmt = args.input.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase());
            let data = match fs::read_to_string(&args.input) {
                Ok(d) => d,
                Err(e) => { eprintln!("simulate: cannot read {:?}: {e}", args.input); return; }
            };
            let mut g = match fmt.as_deref() {
                Some("yaml") | Some("yml") => match nc_nir::Graph::from_yaml_str(&data) {
                    Ok(g) => g,
                    Err(e) => { eprintln!("simulate: parse yaml failed: {e}"); return; }
                },
                _ => match nc_nir::Graph::from_json_str(&data) {
                    Ok(g) => g,
                    Err(e) => { eprintln!("simulate: parse json failed: {e}"); return; }
                },
            };
            if let Err(e) = g.validate() {
                eprintln!("simulate: validation failed: {e}");
                return;
            }
            g.ensure_version_tag();

            #[cfg(feature = "telemetry")]
            let app = if let Some(p) = &args.profile_jsonl {
                nc_telemetry::profiling::Appender::open(p).ok()
            } else {
                None
            };

            #[cfg(feature = "telemetry")]
            let mut labels = BTreeMap::new();
            #[cfg(feature = "telemetry")]
            {
                labels.insert("simulator".to_string(), args.simulator.clone());
                labels.insert("graph".to_string(), g.name.clone());
            }

            let out_dir = args.out_dir.clone().unwrap_or_else(|| PathBuf::from(format!("target/sim-{}-out", args.simulator)));
            // Mark as used even when simulator features are not enabled to avoid unused warnings.
            let _ = &out_dir;

            #[cfg(feature = "telemetry")]
            let __timer_emit = app.as_ref().map(|a| a.start_timer("simulate.emit_ms", labels.clone()));

            match args.simulator.as_str() {
                "neuron" => {
                    #[cfg(feature = "sim-neuron")]
                    {
                        match nc_sim_neuron::emit_artifacts(&g, &out_dir) {
                            Ok(_) => println!("simulate artifacts written to {:?}", out_dir),
                            Err(e) => eprintln!("simulate error: {e}")
                        }
                    }
                    #[cfg(not(feature = "sim-neuron"))]
                    {
                        println!("simulate stub: simulator=neuron (build CLI with feature 'sim-neuron')");
                    }
                }
                "coreneuron" => {
                    #[cfg(feature = "sim-coreneuron")]
                    {
                        match nc_sim_coreneuron::emit_artifacts(&g, &out_dir) {
                            Ok(_) => println!("simulate artifacts written to {:?}", out_dir),
                            Err(e) => eprintln!("simulate error: {e}")
                        }
                    }
                    #[cfg(not(feature = "sim-coreneuron"))]
                    {
                        println!("simulate stub: simulator=coreneuron (build CLI with feature 'sim-coreneuron')");
                    }
                }
                "arbor" => {
                    #[cfg(feature = "sim-arbor")]
                    {
                        match nc_sim_arbor::emit_artifacts(&g, &out_dir) {
                            Ok(_) => println!("simulate artifacts written to {:?}", out_dir),
                            Err(e) => eprintln!("simulate error: {e}")
                        }
                    }
                    #[cfg(not(feature = "sim-arbor"))]
                    {
                        println!("simulate stub: simulator=arbor (build CLI with feature 'sim-arbor')");
                    }
                }
                "hw" => {
                    #[cfg(feature = "sim-hw-specific")]
                    {
                        let r = nc_sim_hw_specific::stub();
                        println!("simulate ok: {}", r);
                    }
                    #[cfg(not(feature = "sim-hw-specific"))]
                    {
                        println!("simulate stub: simulator=hw (build CLI with feature 'sim-hw-specific')");
                    }
                }
                other => {
                    println!("simulate unsupported: {other}");
                }
            }

            #[cfg(feature = "telemetry")]
            {
                if let Some(a) = &app {
                    let _ = a.counter("graph.populations", g.populations.len() as f64, labels.clone());
                    let _ = a.counter("graph.connections", g.connections.len() as f64, labels.clone());
                    let _ = a.counter("graph.probes", g.probes.len() as f64, labels.clone());
                }
            }
        }
        Some(Command::Profile(args)) => {
            if let Some(path) = args.input {
                #[cfg(feature = "telemetry")]
                {
                    match nc_telemetry::profiling::summarize_jsonl(&path) {
                        Ok(stats) => {
                            println!("metric,count,avg,min,max");
                            for (m, (c, sum, min, max)) in stats {
                                let avg = if c > 0 { sum / c as f64 } else { 0.0 };
                                println!("{},{},{:.4},{:.4},{:.4}", m, c, avg, min, max);
                            }
                        }
                        Err(e) => {
                            eprintln!("profile: summarize failed: {e}");
                        }
                    }
                }
                #[cfg(not(feature = "telemetry"))]
                {
                    println!("profile stub: input={path:?} (build CLI with feature 'telemetry' to summarize JSONL)");
                }
            } else {
                println!("profile stub: no input provided");
            }
        }
        Some(Command::Package(args)) => {
            println!("package stub: output={:?}", args.output);
        }
        Some(Command::Deploy(args)) => {
            println!("deploy stub: target={}", args.target);
        }
        Some(Command::ExportMlir(args)) => {
            #[cfg(feature = "mlir")]
            {
                let fmt = args.input.extension().and_then(|e| e.to_str()).map(|s| s.to_lowercase());
                let data = match fs::read_to_string(&args.input) {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("mlir: cannot read {:?}: {e}", args.input);
                        return;
                    }
                };
                let mut g = match fmt.as_deref() {
                    Some("yaml") | Some("yml") => match nc_nir::Graph::from_yaml_str(&data) {
                        Ok(g) => g,
                        Err(e) => { eprintln!("mlir: parse yaml failed: {e}"); return; }
                    },
                    _ => match nc_nir::Graph::from_json_str(&data) {
                        Ok(g) => g,
                        Err(e) => { eprintln!("mlir: parse json failed: {e}"); return; }
                    },
                };
                if let Err(e) = g.validate() {
                    eprintln!("mlir: validation failed: {}", e);
                    return;
                }
                g.ensure_version_tag();
                match nc_mlir_bridge::lower_to_mlir(&g) {
                    Ok(s) => println!("{}", s),
                    Err(e) => eprintln!("mlir: lower failed: {e}"),
                }
            }
            #[cfg(not(feature = "mlir"))]
            {
                let _ = &args;
                println!("mlir export requires building CLI with feature 'mlir'");
            }
        }
        None => {
            println!("Use --help for commands. Example: neuro-compiler list-targets");
        }
    }
}
