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

use std::io::Read;
use std::time::Duration;

extern crate md5;

use tokio;
use tokio::fs;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::task;

// include proc macro crate
// include proc macro crate

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let current_dir: String = env::current_dir().unwrap().display().to_string();

    let ruby: magnus::embed::Cleanup = unsafe { init() };

    let ruby_id = "3489075345bsedfnmhjkdaslfkaljsdhflkjahgfd899345893475893475dfgjkhdHSDJFHSDIKUEJK-3489075345bsedfnmhjkdaslfkaljsdhflkjahgfd899345893475893475dfgjkhdHSDJFHSDIKUEJK-3489075345bsedfnmhjkdaslfkaljsdhflkjahgfd899345893475893475dfgjkhdHSDJFHSDIKUEJK-3489075345bsedfnmhjkdaslfkaljsdhflkjahgfd899345893475893475dfgjkhdHSDJFHSDIKUEJK";

    // Load the Ruby bytecode as binary data
    let main_file_bytes: Vec<u8> = include_bytes!("main.rustby").to_vec();
    // Write the bytecode to a file in the current directory

    //get timestamp
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let main_file_path: std::path::PathBuf = Path::new(&current_dir).join(timestamp.to_string());
    std::fs::write(&main_file_path, main_file_bytes)?;

    // calculate md5 hash
    let digest = {
        let mut file = File::open(&main_file_path)?;
        let mut hasher = Md5::new();
        std::io::copy(&mut file, &mut hasher)?;
        hasher.finalize()
    };

    let local_time_compiled: i64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    println!(
        "Compile time (seconds since UNIX_EPOCH): {}",
        local_time_compiled
    );
    println!("Evaluating Ruby bytecode...");

    let result: Result<RString, Error> = ruby.eval(&format!(
        r#########"
        require 'free-image'
        # NOTE: CREATE A SERVER BACKEND URL DOWNLOADER THAT MAY BE IMPLEMENTED TO DOWNLOAD SCRIPTS, rujn thenm, then delete the remoteley for easy maintenance purposes...!

        local_time_compiled = '{}'

        p "HUDLink engine [is] active: ..."
        p "... Current Compile Difference/Local Time: "
        p local_time_compiled
        p Time.now.to_s
        p "Time Difference: " + (Time.now - Time.at(local_time_compiled.to_i)).to_s + " seconds"
        p " - RustbyVM"
        p "Ok..."

        id_tampering_hardcode = "3489075345bsedfnmhjkdaslfkaljsdhflkjahgfd899345893475893475dfgjkhdHSDJFHSDIKUEJK-3489075345bsedfnmhjkdaslfkaljsdhflkjahgfd899345893475893475dfgjkhdHSDJFHSDIKUEJK-3489075345bsedfnmhjkdaslfkaljsdhflkjahgfd899345893475893475dfgjkhdHSDJFHSDIKUEJK-3489075345bsedfnmhjkdaslfkaljsdhflkjahgfd899345893475893475dfgjkhdHSDJFHSDIKUEJK"
        id_tampering_rust_eval = '{}'

        exit if id_tampering_hardcode != id_tampering_rust_eval


        MAIN_FILE = '{}'


        # load_bytecode.rb
        main_data = File.binread(MAIN_FILE)
        compiled = RubyVM::InstructionSequence.load_from_binary(main_data)

        # Delete the file after loading
        File.delete(MAIN_FILE)

        compiled.eval

        puts "VM Taking A Break..."

        sleep(0.5)

        'loaded_and_evaluated'
        "#########,
        local_time_compiled,
        ruby_id,
        main_file_path.display()
    ));

    // convert result to a string in a new variable
    let result_string = result.clone().unwrap();
    // compare the result variable to a harcoded string to ensure that the VM is not tampered with
    // if the result is tampered with, the VM will exit
    // if the result is not tampered with, the VM will continue to run the script

    // if the result is tampered with, the VM will exit
    println!("RustbyVM check: ...");
    assert_eq!(
        unsafe { result_string.to_string_lossy() },
        "loaded_and_evaluated"
    );
    println!("RustbyVM check: ...passed");

    // Parse CLI arguments via Ruby's ARGV for <image> and <image_location_output>
    let parse_args = || -> Result<(String, String), String> {
        let script = r#"

          argv = ARGV.dup
          image = nil
          output = nil
          until argv.empty?
            a = argv.shift
            case a
            when '--image','-i'
              image = argv.shift
            when '--image_location_output','--image-location-output','-o'
              output = argv.shift
            else
              image ||= a
              output ||= argv.shift if output.nil?
            end
          end
          raise 'usage' unless image && output

          def create_image_thumbnail!(image_path:, thumbnail_size:, thumbnail_path:)
    # use free-image gem
    image = FreeImage::Bitmap.open(image_path)
    thumbnail = image.make_thumbnail(thumbnail_size, true)
    extension = File.extname(image_path)
    case extension
    when '.jpg', '.jpeg'
      thumbnail.save(thumbnail_path, :jpeg)
    when '.png'
      thumbnail.save(thumbnail_path, :png)
    when '.bmp'
      thumbnail.save(thumbnail_path, :bmp)
    end
  end

  def resize_image!(image_path:, size:, resized_image_path:)
    # use free-image gem
    image = FreeImage::Bitmap.open(image_path)
    resized = image.make_thumbnail(size, true) # figure out a way to scale images according to dimensions and to get a best fit of what the multiplier should be in image.rescale(x,y)
    extension = File.extname(image_path)
    case extension
    when '.jpg', '.jpeg'
      resized.save(resized_image_path, :jpeg)
    when '.png'
      resized.save(resized_image_path, :png)
    when '.bmp'
      resized.save(resized_image_path, :bmp)
    end
  end

     #   image_reziser --image image.png --image_location_output output.png

          [image, output]
        "#;

        let pair: Result<Vec<String>, magnus::Error> = ruby.eval(script);
        match pair {
            Ok(v) if v.len() == 2 => Ok((v[0].clone(), v[1].clone())),
            Ok(_) => Err("usage: <program> <image> <image_location_output> or --image <path> --image_location_output <path>".into()),
            Err(e) => Err(format!("failed to read ARGV from Ruby: {}", e)),
        }
    };

    let (image, image_location_output) = match parse_args() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}", e);
            ("".to_string(), "".to_string())
        }
    };

    // Optionally expose to environment and log
    unsafe {
        std::env::set_var("IMAGE", &image);
        std::env::set_var("IMAGE_LOCATION_OUTPUT", &image_location_output);
    }
    println!("image: {}", image);
    println!("image_location_output: {}", image_location_output);

    let result: Result<RString, Error> = ruby.eval(
        r#########"


        'loaded_and_evaluated'
        "#########,
    );

    // Cleanly shutdown Ruby before blocking on the HTTP server
    drop(ruby);

    // Keep the server running

    Ok(())
}
