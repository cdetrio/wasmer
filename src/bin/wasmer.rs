extern crate structopt;

use std::fs::File;
use std::io;
use std::io::Read;
use std::path::PathBuf;
use std::process::exit;
use std::time::{Duration, Instant};

use structopt::StructOpt;

use wasmer::webassembly::InstanceABI;
use wasmer::*;
use wasmer_emscripten;

#[derive(Debug, StructOpt)]
#[structopt(name = "wasmer", about = "Wasm execution runtime.")]
/// The options for the wasmer Command Line Interface
enum CLIOptions {
    /// Run a WebAssembly file. Formats accepted: wasm, wast
    #[structopt(name = "run")]
    Run(Run),

    /// Update wasmer to the latest version
    #[structopt(name = "self-update")]
    SelfUpdate,
}

#[derive(Debug, StructOpt)]
struct Run {
    #[structopt(short = "d", long = "debug")]
    debug: bool,

    /// Input file
    #[structopt(parse(from_os_str))]
    path: PathBuf,

    /// Application arguments
    #[structopt(name = "--", raw(multiple = "true"))]
    args: Vec<String>,
}

/// Read the contents of a file
fn read_file_contents(path: &PathBuf) -> Result<Vec<u8>, io::Error> {
    let mut buffer: Vec<u8> = Vec::new();
    let mut file = File::open(path)?;
    file.read_to_end(&mut buffer)?;
    // We force to close the file
    drop(file);
    Ok(buffer)
}

/// Execute a wasm/wat file
fn execute_wasm(options: &Run) -> Result<(), String> {
    let wasm_path = &options.path;

    let mut wasm_binary: Vec<u8> = read_file_contents(wasm_path).map_err(|err| {
        format!(
            "Can't read the file {}: {}",
            wasm_path.as_os_str().to_string_lossy(),
            err
        )
    })?;

    if !utils::is_wasm_binary(&wasm_binary) {
        wasm_binary = wabt::wat2wasm(wasm_binary)
            .map_err(|e| format!("Can't convert from wast to wasm: {:?}", e))?;
    }

    // start compilation time
    let start_compile = Instant::now();

    let module = webassembly::compile(&wasm_binary[..])
        .map_err(|e| format!("Can't compile module: {:?}", e))?;

    let (_abi, import_object, _em_globals) = if wasmer_emscripten::is_emscripten_module(&module) {
        let mut emscripten_globals = wasmer_emscripten::EmscriptenGlobals::new(&module);
        (
            InstanceABI::Emscripten,
            wasmer_emscripten::generate_emscripten_env(&mut emscripten_globals),
            Some(emscripten_globals), // TODO Em Globals is here to extend, lifetime, find better solution
        )
    } else {
        (
            InstanceABI::None,
            wasmer_runtime_core::import::ImportObject::new(),
            None,
        )
    };

    let mut instance = module
        .instantiate(&import_object)
        .map_err(|e| format!("Can't instantiate module: {:?}", e))?;

    // end compilation time
    let compile_duration = start_compile.elapsed();
    println!("compile time: {:?}", compile_duration);

    let start_run = Instant::now();

    webassembly::run_instance(
        &module,
        &mut instance,
        options.path.to_str().unwrap(),
        options.args.iter().map(|arg| arg.as_str()).collect(),
    )
    .map_err(|e| format!("{:?}", e))?;

    let run_duration = start_run.elapsed();
    println!("run time: {:?}", run_duration);

    Ok(())
}

fn run(options: Run) {
    match execute_wasm(&options) {
        Ok(()) => {}
        Err(message) => {
            eprintln!("{:?}", message);
            exit(1);
        }
    }
}

fn main() {
    let options = CLIOptions::from_args();
    match options {
        CLIOptions::Run(options) => run(options),
        #[cfg(not(target_os = "windows"))]
        CLIOptions::SelfUpdate => update::self_update(),
        #[cfg(target_os = "windows")]
        CLIOptions::SelfUpdate => {
            println!("Self update is not supported on Windows. Use install instructions on the Wasmer homepage: https://wasmer.io");
        }
    }
}
