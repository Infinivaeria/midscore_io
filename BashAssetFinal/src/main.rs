use magnus::embed::init;
use magnus::{
    Error, RArray, RClass, RFile, RFloat, RHash, RModule, RObject, RRegexp, RString, RStruct, Ruby,
    Value, function, method, prelude::*, rb_assert, typed_data, value::Lazy, value::Opaque,
};
use std::env;
use std::mem::drop;
use std::thread::{self, sleep};

fn main() -> Result<(), Error> {
    let current_dir = env::current_dir().unwrap().display().to_string();
    println!("Present Working Directory: {}", current_dir);
    let ruby = unsafe { init() };

    let rb_file_path = include_str!("../hammerspace-uploader/main.rb");
    let evaluated = ruby.eval::<Value>(&rb_file_path); //gets
    // Now you have access to the Ruby environment through the `ruby` variable.
    // You can use this to interact with Ruby objects, evaluate Ruby code, and more.

    // For example, let's evaluate a simple Ruby expression:
    // let result: i64 = ruby.eval("2 + 2").unwrap();

    Ok(())
    //let rb_file_path = include_str!("main.rb");
    //let _result: Result<bool, magnus::Error> = ruby.eval(&rb_file_path);
}
