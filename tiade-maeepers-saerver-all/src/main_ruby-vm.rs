use magnus::embed::init;
use std::env;
extern crate load_file;

fn main() {
    let current_dir = env::current_dir().unwrap().display().to_string();
    println!("Current directory: {}", current_dir);
    let ruby = unsafe { init() };
    let rb_file_path = include_str!("main.rb");
    let result: Result<bool, magnus::Error> = ruby.eval(&rb_file_path);
}






