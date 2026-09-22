// Version 1.0 Main
// by Aylon Arlon (2024-29-11)

use magnus::{prelude::*,  Error as MagnusError, Ruby, function, typed_data, RClass, Value as MValue, method, rb_assert, RString, RArray, RFloat, RObject, RRegexp, RHash, RFile, RStruct, RModule, value::Opaque, value::Lazy};
use magnus::embed::init;
use std::env;
use std::thread::{self, sleep};
use serde::{Deserialize, Serialize};
use serde_json::{Result, Value, Error as SerdeError};

fn main() -> Result<()> {
    let current_dir = env::current_dir().unwrap().display().to_string();
    println!("PWD: {}", current_dir);

    // Move the Ruby VM initialization into a new thread
    let ruby_thread = thread::spawn(|| {
        let ruby_vm = unsafe { magnus::embed::init() };

        let evaluated: std::result::Result<String, MagnusError> = ruby_vm.eval::<String>(r#####"
            # Ruby has Fibers which are green threads. Green threads use Ruby's native VM in whatever way(??)
        
        require 'rubygems'
        require 'objspace'
        require 'drb/drb'
        require 'uri'
        require 'matrix'
        require 'ipaddr'
        require 'set'
        require 'observer'
        require 'require_all'
        require 'json'
        require 'fileutils'
        require 'fiddle'
        require 'monitor
        require 'async/scheduler'
        require 'strscan'
        require 'fastimage'
        # prime gem
        require 'fiddle'
        require_relative 'rb_main/main'  
        'gem libraries loaded'
        "#####);  //require rubygem libs

        println!("Evaluated: {:#?}", evaluated);
        assert_eq!(evaluated.unwrap(), "gem libraries loaded");
        
        // Main window loop load file into compilation process
        let metagame_compile_path = include_str!("./metagame_compile.rb");
        // execute the main game loop
        let evaluated: std::result::Result<String, MagnusError> = ruby_vm.eval::<String>(&metagame_compile_path);
        // end main game loop

        println!("Game Loop Exited: {:#?}", evaluated);
        assert_eq!(evaluated.unwrap(), "exit gameloop success");
    });   

    // Wait for the Ruby thread to finish before exiting
    ruby_thread.join().unwrap();

    Ok(())
}