#![allow(dead_code)]

extern crate regex;
extern crate yaml_rust;
use yaml_rust::YamlLoader;
use regex::Regex;
use std::fs;
use std::env;

#[derive(Debug)]
struct Config {
    ignores : Regex,
    accept : Regex
}

trait Journal {
    fn observe(&mut self, fpath : std::path::PathBuf) -> ();
    fn summary(&mut self) -> ();
}

#[derive(Debug)]
struct LineCounter {
    total_line : i32,
    per_file_log : Vec<(String, i32)>
}

impl Config {

    fn new(filename : &str) -> Config {

        // 1. read and parse yaml config file
        let contents = fs::read_to_string(filename)
            .expect(&format!("unable to read config file : {}", filename));
        let loaded_docs = YamlLoader::load_from_str(contents.as_str())
            .expect("error parsing config yaml");
        let ydoc = loaded_docs.get(0).unwrap();

        
        // 2. construct appropriate conifg struct
        let ignore_list = ydoc["ignore"].as_vec().expect("fuck you no ignore!");
        let ext_list = ydoc["accept"].as_vec().expect("fuck you no accept!");

        let ignore_pattern = ignore_list.iter()
            .map(|y|{y.as_str()})
            .filter(|opt|{match opt {Some(_) => true, _ => false}})
            .map(|opt|{opt.unwrap()})
            .collect::<Vec<&str>>()
            .join("|");

        let expanded_exts = ext_list.iter()
            .map(|y|{y.as_str()})
            .filter(|opt|{match opt {Some(_) => true, _ => false}})
            .map(|opt|{format!("\\.{}", opt.unwrap())})
            .collect::<Vec<String>>()
            .join("|");

        let ext_pattern = format!("[\\S]+({})$", expanded_exts);

        
        Config{
            ignores : Regex::new(&ignore_pattern).expect("error compiling pattern [ignores]"),
            accept : Regex::new(&ext_pattern).expect("error compiling pattern [accept]")
        }
    }
}

impl LineCounter {
    fn new() -> LineCounter {
        LineCounter{
            total_line : 0,
            per_file_log : Vec::new()
        }
    }
}

impl Journal for LineCounter {
    fn observe(&mut self, fpath : std::path::PathBuf) -> () {
        if let Ok(content) = fs::read_to_string(&fpath) {
            let mut local_line = 0;
            for _ in content.split('\n') {
                local_line += 1;
            }
            self.total_line += local_line;
            self.per_file_log.push((String::from(fpath.to_str().expect("magic")), local_line));
        }

    }

    fn summary(&mut self) -> () {
        self.per_file_log.sort_by(|a,b|{a.1.cmp(&b.1)});
        for (file_name, line_count) in &self.per_file_log {
            println!("* {} \t lines of {}", line_count, file_name);
        }
        println!("summary for line counter : total line : {}", self.total_line);
    }
}


fn indent_print(message : &str, indent_level : u32, indent_character : char) {
    let mut indent_printed = 0;
    while indent_printed < indent_level {
        print!("{}", indent_character);
        indent_printed += 1;
    }
    println!("{}", message);
}



fn traverse_directory<T:Journal>(root_dir : &str, depth : u32, config : &Config, journal : &mut T) {
    let root_dir = fs::read_dir(root_dir).unwrap();
    for entry in root_dir {
        if let Ok(item) = entry {
            let filepath = item.path();
            let filename = filepath.file_name().unwrap().to_str().unwrap();
            if let Ok(ftype) = item.file_type() {
                if ftype.is_dir() && !(config.ignores.is_match(filename)){
                    traverse_directory(item.path().to_str().unwrap(), depth + 1, config, journal);
                } else if ftype.is_file() && config.accept.is_match(filename){
                    // println!("matched a file : {}", filename);
                    journal.observe(item.path());
                }
            }
        }   
    }
}


fn main() {

    let home_dir = match env::var("HOME") {
        Ok(s) => s,
        _ => String::from(".")
    };

    let config = Config::new(&format!("{}/.howmany_conf.yaml", home_dir));
    println!("{:?}", config);
    let mut lc = LineCounter::new();
    traverse_directory(".", 0, &config, &mut lc);
    lc.summary();
}
