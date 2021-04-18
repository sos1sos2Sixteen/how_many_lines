#![allow(dead_code)]
#![feature(never_type)]

extern crate regex;
extern crate yaml_rust;
#[macro_use]
extern crate lazy_static;
use regex::Regex;
use std::env;
use std::ops::Range;
use std::path;
use std::{collections::HashMap, fs, process::exit};
use yaml_rust::YamlLoader;
use rayon::prelude::*;

/// default configure file, could be overriden (completely) by a .howmany_conf.yaml
static DEFAULT_CONFIG: &str = "
ignore : 

accept : 
  - py
  - yaml
  - rs
  - c
  - h
  - cpp
  - hpp
  - java
  - cu

skip_threshold : 
";

#[derive(Debug)]
struct Config {
    ignores: Regex,
    accept: Regex,
    skip_threshold: i32,
}

trait Journal {
    fn observe(&mut self, fpath: std::path::PathBuf) -> ();
    fn summary(&mut self) -> ();
}

#[derive(Debug)]
struct LineCounter {
    total_line: i32,
    per_file_log: Vec<(String, i32)>,
}

struct TodoItem {
    tag_idx: Range<usize>,
    content_idx: Range<usize>,
    source_loc: String,
    line: String,
}

struct CollectJournal{
    codes : Vec<String>
}
impl CollectJournal{
    fn new() -> Self{
        CollectJournal{
            codes : vec![]
        }
    }
}
impl Journal for CollectJournal {
    fn observe(&mut self, fpath: std::path::PathBuf) -> () {
        self.codes.push(String::from(fpath.to_str().unwrap()));
    }
    fn summary(&mut self) -> () {
        let mut odd:Vec<(&String, usize)> = self.codes.par_iter().map(|path|{
            if let Ok(content) = fs::read_to_string(path) {
                let mut local_line = 0;
                for _ in content.split('\n') {
                    local_line += 1;
                }
                (path, local_line)
            }else{
                (path, 0)
            }
        }).filter(|(_, lc)|{*lc > 0}).collect();

        odd.par_sort_by_key(|(_,lc)|{*lc});

        let total_count = odd.par_iter()
            .map(|(_,lc)|{*lc})
            .reduce(||{0}, |a,b|{a+b});

        for (file_name, line_count) in &odd {
            println!("* {} \t lines of {}", line_count, file_name);
        }
        println!(
            "summary for line counter : total : {} lines from {} files",
            total_count,
            odd.len()
        );
    }
}

struct TodoCatcher {
    todos: Vec<TodoItem>,
}

impl TodoCatcher {
    fn new() -> TodoCatcher {
        TodoCatcher { todos: Vec::new() }
    }
}

impl Journal for TodoCatcher {
    fn observe(&mut self, fpath: std::path::PathBuf) -> () {
        fn catch_line(fpath: &std::path::PathBuf, line_no: usize, line: &str) -> Option<TodoItem> {
            // implement later
            lazy_static! {
                static ref RE: Regex = Regex::new(r"TODO\[(?P<tag>[\S]+)\].*$").unwrap();
            }

            // let RE : Regex;

            if RE.is_match(line) {
                let line = line.trim().to_string();
                let captures = RE.captures(&line).unwrap();
                let tag_idx = captures.name("tag").unwrap().range();
                let content_idx = captures.get(0).unwrap().range();
                Some(TodoItem {
                    tag_idx,
                    content_idx,
                    source_loc: format!("{}:{}", fpath.to_str().unwrap(), line_no),
                    line
                })
            } else {
                None
            }
        }

        if let Ok(content) = fs::read_to_string(&fpath) {
            for (idx, line) in content.lines().enumerate() {
                if let Some(item) = catch_line(&fpath, idx + 1, line) {
                    self.todos.push(item);
                }
            }
        }
    }
    fn summary(&mut self) -> () {
        let mut tag_mapped: HashMap<String, Vec<&TodoItem>> = HashMap::new();
        for item in self.todos.iter() {
            let key = item.line.get(item.tag_idx.clone()).unwrap();
            if !tag_mapped.contains_key(key) {
                tag_mapped.insert(key.to_string(), Vec::new());
            }
            tag_mapped.get_mut(key).unwrap().push(item)
        }

        for key in tag_mapped.keys() {
            println!("# tag : {} : ", key);
            for item in tag_mapped.get(key).unwrap() {
                println!(
                    "\t[ ] {} @ {}",
                    item.line.get(item.content_idx.clone()).unwrap(),
                    item.source_loc
                );
            }
        }
        println!(
            "gathered in total {} todos in {} tags",
            self.todos.len(),
            tag_mapped.keys().len()
        );
    }
}

impl Config {
    /// load and parse given config file, if the given file could not be opened, switch to the default file
    fn new(filename: &str) -> Config {
        // 1. read and parse yaml config file
        let contents = match fs::read_to_string(filename) {
            Ok(ctt) => ctt,
            _ => {
                println!(
                    "unable to read config file : {}, switch to default",
                    filename
                );
                String::from(DEFAULT_CONFIG)
            }
        };
        let loaded_docs =
            YamlLoader::load_from_str(contents.as_str()).expect("error parsing config yaml");
        let ydoc = loaded_docs.get(0).unwrap();

        // 2. construct appropriate conifg struct
        let ignore_list = ydoc["ignore"].as_vec();
        let ignore_pattern = match ignore_list {
            Some(v) => v
                .iter()
                .map(|y| y.as_str())
                .filter(|opt| match opt {
                    Some(_) => true,
                    _ => false,
                })
                .map(|opt| opt.unwrap())
                .collect::<Vec<&str>>()
                .join("|"),
            _ => String::from("$a"), // a regex that will never be matched to anything
        };

        let ext_list = ydoc["accept"].as_vec().expect("fuck you no accept!");
        let expanded_exts = ext_list
            .iter()
            .map(|y| y.as_str())
            .filter(|opt| match opt {
                Some(_) => true,
                _ => false,
            })
            .map(|opt| format!("\\.{}", opt.unwrap()))
            .collect::<Vec<String>>()
            .join("|");

        let ext_pattern = format!("[\\S]+({})$", expanded_exts);

        // 3. read skip thresh
        let skip_threshold = match ydoc["skip_threshold"].as_i64() {
            Some(st) => st as i32,
            None => 100,
        };

        Config {
            ignores: Regex::new(&ignore_pattern).expect("error compiling pattern [ignores]"),
            accept: Regex::new(&ext_pattern).expect("error compiling pattern [accept]"),
            skip_threshold,
        }
    }
}

impl LineCounter {
    fn new() -> LineCounter {
        LineCounter {
            total_line: 0,
            per_file_log: Vec::new(),
        }
    }
}

impl Journal for LineCounter {
    fn observe(&mut self, fpath: std::path::PathBuf) -> () {
        if let Ok(content) = fs::read_to_string(&fpath) {
            let mut local_line = 0;
            for _ in content.split('\n') {
                local_line += 1;
            }
            self.total_line += local_line;
            self.per_file_log
                .push((String::from(fpath.to_str().expect("magic")), local_line));
        }
    }

    fn summary(&mut self) -> () {
        self.per_file_log.sort_by(|a, b| a.1.cmp(&b.1));
        for (file_name, line_count) in &self.per_file_log {
            println!("* {} \t lines of {}", line_count, file_name);
        }
        println!(
            "summary for line counter : total : {} lines from {} files",
            self.total_line,
            self.per_file_log.len()
        );
    }
}

// TODO[shiyao] : do this and that
fn indent_print(message: &str, indent_level: u32, indent_character: char) {
    let mut indent_printed = 0;
    while indent_printed < indent_level {
        print!("{}", indent_character);
        indent_printed += 1;
    }
    println!("{}", message);
}

fn traverse_directory<T: Journal>(
    root_dir_name: &str,
    depth: u32,
    config: &Config,
    journal: &mut T,
    skip_record: &mut Vec<String>,
) {
    let root_dir = fs::read_dir(root_dir_name).unwrap();
    let mut continuous_miss_count = 0;
    for entry in root_dir {
        if let Ok(item) = entry {
            let filepath = item.path();
            let filename = filepath.file_name().unwrap().to_str().unwrap();
            if let Ok(ftype) = item.file_type() {
                if ftype.is_dir() && !(config.ignores.is_match(filename)) {
                    traverse_directory(
                        item.path().to_str().unwrap(),
                        depth + 1,
                        config,
                        journal,
                        skip_record,
                    );
                } else if ftype.is_file() {
                    if config.accept.is_match(filename) {
                        // println!("matched a file : {}", filename);
                        journal.observe(item.path());
                        continuous_miss_count = 0;
                    } else {
                        continuous_miss_count += 1;
                        if continuous_miss_count >= config.skip_threshold {
                            skip_record.push(String::from(root_dir_name));
                            return;
                        }
                    }
                }
            }
        }
    }
}

fn print_help_and_die() -> ! {
    println!("sixteen's howmanylines.");
    println!("usage : ");
    println!("\t$ howmanylines line|todo|help [DIR]");
    println!("\t* line : count lines");
    println!("\t* todo : gather todos");
    println!("\t* parl : for a line-counter in parallel");
    println!("\t* help : print this message");
    println!("\t optional DIR, the root dir from which the program starts scan");

    exit(0)
}

fn run_journal<J: Journal>(root_traverse_dir: &str, config: &Config, journal: &mut J) -> () {
    // perform count
    let mut skip_record = Vec::<String>::new();
    traverse_directory(&root_traverse_dir, 0, config, journal, &mut skip_record);

    // output summary
    if skip_record.len() > 0 {
        println!("the following directories are automatically skipped due to large miss rates :");
        for skipped in skip_record {
            println!("* {}", skipped);
        }
        println!("------------------------")
    }
    journal.summary();
}

fn main() {
    // parse commandline argument
    let args = env::args().collect::<Vec<String>>();
    if args.len() < 2 {
        print_help_and_die();
    }

    let root_traverse_dir = match args.get(2) {
        Some(tgt) => tgt.clone(),
        None => String::from("."),
    };

    // locate potential config file
    let home_dir = match env::var("HOME") {
        Ok(s) => path::PathBuf::from(s),
        _ => path::PathBuf::from("."),
    };
    let conf_filename = path::Path::new(".howmany_conf.yaml");
    let config = Config::new(home_dir.join(conf_filename).to_str().unwrap());

    match args.get(1).unwrap().as_str() {
        "line" => run_journal(&root_traverse_dir, &config, &mut LineCounter::new()),
        "todo" => run_journal(&root_traverse_dir, &config, &mut TodoCatcher::new()),
        "parl" => run_journal(&root_traverse_dir, &config, &mut CollectJournal::new()),
        "help" => print_help_and_die(),
        other => {
            println!("sub-command {} not recognized!", other);
            print_help_and_die()
        }
    };
}
