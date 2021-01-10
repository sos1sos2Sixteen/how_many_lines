#![allow(dead_code)]

extern crate regex;
extern crate yaml_rust;
use yaml_rust::YamlLoader;
use regex::Regex;
use std::fs;
use std::env;
use std::path;

#[derive(Debug)]
struct Config {
    ignores : Regex,
    accept : Regex,
    skip_threshold : i32
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
        let ignore_list = ydoc["ignore"].as_vec();
        let ignore_pattern = match ignore_list {
            Some(v) => v.iter()
                    .map(|y|{y.as_str()})
                    .filter(|opt|{match opt {Some(_) => true, _ => false}})
                    .map(|opt|{opt.unwrap()})
                    .collect::<Vec<&str>>()
                    .join("|"),
            _ => String::from("$a")     // a regex that will never be matched to anything
        };

        let ext_list = ydoc["accept"].as_vec().expect("fuck you no accept!");
        let expanded_exts = ext_list.iter()
            .map(|y|{y.as_str()})
            .filter(|opt|{match opt {Some(_) => true, _ => false}})
            .map(|opt|{format!("\\.{}", opt.unwrap())})
            .collect::<Vec<String>>()
            .join("|");

        let ext_pattern = format!("[\\S]+({})$", expanded_exts);

        // 3. read skip thresh
        let skip_threshold = match ydoc["skip_threshold"].as_i64() {
            Some(st) => st as i32, 
            None => 100
        };

        
        Config{
            ignores : Regex::new(&ignore_pattern).expect("error compiling pattern [ignores]"),
            accept : Regex::new(&ext_pattern).expect("error compiling pattern [accept]"),
            skip_threshold
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



fn traverse_directory<T:Journal>(root_dir_name : &str, depth : u32, config : &Config, journal : &mut T, skip_record : &mut Vec<String>) {
    let root_dir = fs::read_dir(root_dir_name).unwrap();
    let mut continuous_miss_count = 0;
    for entry in root_dir {
        if let Ok(item) = entry {
            let filepath = item.path();
            let filename = filepath.file_name().unwrap().to_str().unwrap();
            if let Ok(ftype) = item.file_type() {
                if ftype.is_dir() && !(config.ignores.is_match(filename)){
                    traverse_directory(item.path().to_str().unwrap(), depth + 1, config, journal, skip_record);
                } else if ftype.is_file(){
                    if config.accept.is_match(filename){
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


fn main() {

    let home_dir = match env::var("HOME") {
        Ok(s) => path::PathBuf::from(s),
        _ => path::PathBuf::from(".")
    };


    let conf_filename = path::Path::new(".howmany_conf.yaml");
    let config = Config::new(home_dir.join(conf_filename).to_str().unwrap());
    println!("{:?}", config);
    let mut lc = LineCounter::new();
    let mut skip_record = Vec::<String>::new();
    traverse_directory(".", 0, &config, &mut lc, &mut skip_record);

    if skip_record.len() > 0 {
        println!("the following directories are automatically skipped due to large miss rates :");
        for skipped in skip_record {
            println!("* {}", skipped);
        }
    }
    lc.summary();
}
