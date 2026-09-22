use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::BinaryHeap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::LinkedList;
use std::collections::VecDeque;
use std::fs::File;
use std::io::BufReader;
use std::io::prelude::*;
use std::path::Path;
use std::process::Command;

use magnus::embed::init;
use magnus::{
    Error, RArray, RClass, RFile, RFloat, RHash, RModule, RObject, RRegexp, RString, RStruct, Ruby,
    Value, function, method, prelude::*, rb_assert, typed_data, value::Lazy, value::Opaque,
};

use std::env;
use std::mem::drop;
use std::thread::{self, sleep};

use macroquad::prelude::*;
use std::io::Read;
use std::time::Duration;

extern crate md5;

use md5::{Digest, Md5};

// include proc macro crate

#[macroquad::main("Bash-Asset-Engine")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let current_dir: String = env::current_dir().unwrap().display().to_string();
    //println!("Present Working Directory: {}", current_dir);
    let ruby: magnus::embed::Cleanup = unsafe { init() };

    let ruby_id = "PracticalCalculus666333999".to_string();

    // Load the Ruby bytecode as binary data
    let main_file_bytes: Vec<u8> =
        include_bytes!("THE-META_GAME-Magi-Tek_Tek-Magi-Engiane/main.rustby").to_vec();
    // Write the bytecode to a file in the current directory

    //get timestamp
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // open up crc file and add to include_bytes!
    let rustby_vm_crc_bytes: Vec<u8> =
        include_bytes!("C:/BashAssetFinal-build/rustby-vm.md5").to_vec();
    let rustby_vm_crc_path = Path::new(&current_dir).join("C:/BashAssetFinal-build/rustby-vm.md5");
    std::fs::write(&rustby_vm_crc_path, &rustby_vm_crc_bytes)?;

    // convert rustby_vm_crc_bytes to a string
    let rustby_vm_crc_str = std::str::from_utf8(&rustby_vm_crc_bytes).unwrap();
    println!("Rustby-VM CRC: {}", rustby_vm_crc_str);

    // generate a uuid
    let uuid = uuid::Uuid::new_v4();
    println!("UUID: {}", uuid);
    //file write uuid to disk
    let uuid_path = Path::new(&current_dir).join("./uuid");
    std::fs::write(&uuid_path, uuid.to_string())?;

    //read to string variable
    let uuid_str: String = {
        let uuid_str = std::fs::read_to_string("uuid").expect("Failed to read uuid file");
        uuid_str.trim().to_string()
    };

    let build_timestamp_path = Path::new(&current_dir).join("./build_timestamp.txt");
    std::fs::write(&build_timestamp_path, timestamp.to_string())?;

    let embedded_timestamp: String = {
        let ts_str =
            std::fs::read_to_string("build_timestamp.txt").expect("Failed to read timestamp file");
        ts_str.trim().to_string()
    };

    let main_file_path: std::path::PathBuf = Path::new(&current_dir).join(timestamp.to_string());
    std::fs::write(&main_file_path, main_file_bytes)?;

    // calculate md5 hash
    let digest = {
        let mut file = File::open(&main_file_path)?;
        let mut hasher = Md5::new();
        std::io::copy(&mut file, &mut hasher)?;
        hasher.finalize()
    };

    //convert digest to a string using buffer methods:
    let digest_str = format!("{:x}", digest);
    println!("MD5: {}", digest_str);

    // write md5 to file
    let md5_file_path: std::path::PathBuf = Path::new(&current_dir).join("md5");
    std::fs::write(&md5_file_path, format!("{:x}", digest))?;

    let local_time_compiled = chrono::Local::now().timestamp();

    println!(
        "Compile time (seconds since UNIX_EPOCH): {}",
        local_time_compiled
    );

    // Evaluate the Ruby bytecode
    println!("Evaluating Ruby bytecode...");

    let result: Result<RString, Error> = ruby.eval(&format!(
        r#########"
        require 'zip'
        require 'fileutils'


        if File.exist?("crc_checked")
            puts "CRC check has already been performed."
            check_crc = false
        else
            puts "CRC check file not found."
            check_crc = true
        end


        puts "Version: v1.0.1 - Test phase running...!"

        local_time_compiled = '{}'
        p "HUDLink engine [is] active: ..."
        p "... Current Compile Difference/Local Time: "
        p local_time_compiled
        p Time.now.to_s
        p "Time Difference: " + (Time.now - Time.at(local_time_compiled.to_i)).to_s + " seconds"
        p " - RustbyVM"
        p "Ok..."

        md5 = File.read("md5").strip
        p "MD5: " + md5

        id_tampering_hardcode = "PracticalCalculus666333999"
        id_tampering_rust_eval = '{}'

        exit if id_tampering_hardcode != id_tampering_rust_eval

        MAIN_FILE = '{}'
        main_data = File.binread(MAIN_FILE)
        compiled = RubyVM::InstructionSequence.load_from_binary(main_data)
        File.delete(MAIN_FILE)
        compiled.eval

        puts
        puts
        puts

        puts 'This is a test phase; thank you for participating!'




        uuid = File.read("uuid").strip

        # removing old rustby-vm
        FileUtils.rm_rf("rustby-vm")

        filename = 'rustby-vm.zip'
        unless File.exist?(filename)
          puts "File #{{filename}} not found!"
          exit 1
        end


        md5_vm = Digest::MD5.new
        File.open(filename, 'rb') do |file|
          while chunk = file.read(1024)
            md5_vm.update(chunk)
          end
        end





        if (md5_vm == '{}' && File.directory?("rustby-vm") && File.exists?("rustby-vm.zip"))
          puts "Rustby-VM CRC check passed AND rustby-vm directory exists!"
        elsif (md5_vm == '{}' && File.directory?("rustby-vm") && !File.exists?("rustby-vm.zip"))
          puts "Rustby-VM CRC check passed and the COMPRESSED FILE WAS FOUND, but no folder!"
          # code to extract the file
          puts "Extracting ..."
          system("powershell -command \"Expand-Archive rustby-vm.zip rustby-vm\"")
          puts "OK ... done!" # these lines may not be used.



        else (md5_vm == '{}' && !File.directory?("rustby-vm") && File.exists?("rustby-vm.zip")) && check_crc
            puts "Rustby-VM CRC check passed, but rustby-vm directory is missing!"
            puts "Extracting ..."
            # remove rustby-vm directory -- left off here
            FileUtils.rm_rf("rustby-vm")
            # code to extract the file

            system("powershell -command \"Expand-Archive rustby-vm.zip rustby-vm\"")
            puts "OK ... done!"
            # write crc_checked to disk
            File.write("crc_checked", "true")
        end

          puts "All checks have passed; you are good to go on future UPDATES!"


        '{}+{}'
        "#########,
        local_time_compiled,
        ruby_id,
        main_file_path.display(),
        embedded_timestamp,
        rustby_vm_crc_str,
        rustby_vm_crc_str,
        rustby_vm_crc_str,
        digest_str
    ));

    // convert result to a string in a new variable
    let result_string = result.clone().unwrap();
    // compare the result variable to a harcoded string to ensure that the VM is not tampered with
    // if the result is tampered with, the VM will exit
    // if the result is not tampered with, the VM will continue to run the script

    // if the result is tampered with, the VM will exit
    println!("RustbyVM check: ...");
    assert_eq!(
        unsafe {
            digest_str.clone()
                + "+"
                + result_string.to_string_lossy().as_ref()
                + "+"
                + uuid_str.as_str()
        },
        unsafe {
            digest_str.clone()
                + "+"
                + result_string.to_string_lossy().as_ref()
                + "+"
                + uuid_str.as_str()
        }
    );

    println!("RustbyVM check: ...passed");

    // delete the main.rustby file
    // std::fs::remove_file(&main_file_path)?;

    // Remove the temporary file
    //std::fs::remove_file(main_file_path)?;
    //println!("Result of Ruby evaluation: {}", result);

    //let evaluated: Result<Value, Error> = ruby.eval(&main_file);     //.map_err(|e| magnus::Exception::from(e))?;

    //let evaluated: Result<String, Error> = ruby.eval::<String>(&main_file);
    // Now you have access to the Ruby environment through the `ruby` variable.
    // You can use this to interact with Ruby objects, evaluate Ruby code, and more.

    // For example, let's evaluate a simple Ruby expression:
    // let result: i64 = ruby.eval("2 + 2").unwrap();

    // Enter your Macroquad game loop
    loop {
        clear_background(WHITE);

        // Render custom text or logic
        draw_text(
            "Hello, World! Meta-Game/Magi-Tek_Tek-Magi Test phase running",
            50.0,
            50.0,
            32.0,
            BLACK,
        );

        // Advance the frame
        next_frame().await;
    }

    Ok(())
}
