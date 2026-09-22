use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{fs, path::PathBuf, sync::mpsc::channel};
use std::io::{self, BufRead};
use tokio::runtime::Runtime;

// Cargo.toml:
// [package]
// name = "magnus_vm_async"
// version = "0.1.0"
// edition = "2021"
//
// [dependencies]
// magnus = "0.3"
// tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
// notify = "5.0"

use magnus::{embed, eval, prelude::*, Error, Ruby, Value, Object, Module, RString, RHash, RArray, RModule, RClass };
use magnus::embed::init;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
      std::thread::spawn(|| {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            if let Ok(input) = line {
                match input.trim() {
                    "exit" => {
                        println!("Exiting server abruptly.");
                        std::process::exit(0);
                    }

                    // When the "rustby" command is input, write the Ruby code to a .rb file
                    // in a shared directory ("./rustby_scripts"). Then, immediately load (evaluate)
                    // the file using Magnus. The file is deleted after evaluation. The Ruby code in
                    // the file is expected to return a string.
                    "rustby" => {
                        let script_dir = "./rustby_scripts";
                        if let Err(e) = std::fs::create_dir_all(script_dir) {
                            eprintln!("Failed to create script directory: {}", e);
                            continue;
                        }
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_nanos())
                            .unwrap_or(0u128);
                        let filename = format!(
                            "{}/script_{}.rb",
                            script_dir,
                            timestamp
                        );
                        // Replace the Ruby code below as needed. It must return a string value.
                        // Replace the Ruby code below as needed. It must return a string value.
                        let ruby_code = r#"nil
       'RustbySpace'
      "#;
                        if let Err(e) = std::fs::write(&filename, ruby_code) {
                            eprintln!("Error writing script file: {}", e);
                            continue;
                        }
                        println!("Script file written: {}", filename);

                        // Instead of calling the Ruby evaluator directly (which cannot be done in a thread),
                        // write the Ruby load command to a named pipe for external processing.
                        let pipe_path = "/tmp/ruby_pipe";
                        if let Err(e) = std::fs::write(pipe_path, format!("load '{}'\n", filename))
                        {
                            eprintln!("Error writing to named pipe: {}", e);
                        } else {
                            println!("Command sent to Ruby evaluator via pipe: {}", pipe_path);
                        }

                        // Wait briefly for the external process to evaluate the script and write the result.
                        std::thread::sleep(std::time::Duration::from_millis(100));

                        // Read the evaluation result from an output file.
                        let result_path = "/tmp/ruby_output.txt";
                        let script_result = match std::fs::read_to_string(result_path) {
                            Ok(output) => Ok(output),
                            Err(e) => {
                                eprintln!("Error reading Ruby output: {}", e);
                                Err(magnus::Error::new(
                                    magnus::exception::runtime_error(),
                                    format!("Error reading Ruby output: {}", e),
                                ))
                            }
                        };

                        // Remove the script file after evaluation.
                        if let Err(e) = std::fs::remove_file(&filename) {
                            eprintln!("Failed to remove script file: {}", e);
                        }

                        match script_result {
                            Ok(output) => println!("Ruby output: {}", output),
                            Err(e) => eprintln!("Error running Ruby code: {}", e),
                        }
                    }

                    "restart" => {
                        println!("Restarting all servers...");
                        std::process::Command::new("sh")
                            .arg("-c")
                            .arg("killall -HUP tiade-maeepers-saerver-all") // Replace with your server binary name
                            .spawn()
                            .expect("Failed to restart servers");
                    }
                    _ => {
                        println!("Unknown command: {}", input.trim());
                    }
                }
            }
        }
    });
    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                tx.send(event).ok();
            }
        },
        notify::Config::default()
    )?;



    let folder = std::env::args()
        .skip_while(|arg| arg != "--folder")
        .skip(1)
        .next()
        .unwrap_or("scripts".to_string());

    std::fs::create_dir_all(&folder)?;

    watcher.watch(std::path::Path::new(&folder), RecursiveMode::Recursive)?;
    watcher.watch(std::path::Path::new("scripts"), RecursiveMode::Recursive)?;
    fs::write("scripts/pid.txt", std::process::id().to_string())?;

    let ruby = unsafe{ init() };
    println!("Watching for Ruby scripts in ./scripts...");


    loop {
        if let Ok(event) = rx.recv() {
            if let EventKind::Create(_) = event.kind {
                for path in event.paths {
                    if path.extension().and_then(|e| e.to_str()) == Some("rb") {
                        let code = fs::read_to_string(&path)?;
                        let result = match ruby.eval::<Value>(&code) {
                            Ok(val) => {
                                // Call Ruby .to_s to get a proper string representation
                                let s_val: Value = val.funcall("to_s", ()).unwrap_or(val);
                                // Try to extract as RString
                                if let Ok(rstr) = RString::try_convert(s_val) {
                                    unsafe { rstr.as_str().unwrap_or("").to_owned() }
                                } else {
                                    format!("{}", val)
                                }
                            },
                            Err(e) => format!("Error: {}", e),
                        };
                        let txt_path = path.with_extension("txt");
                        fs::write(&txt_path, result.to_string())?;
                        //fs::remove_file(&path)?;
                        println!("Evaluated {:?} -> {:?}", path, txt_path);
                    }
                }
            }
        }
    }
}
