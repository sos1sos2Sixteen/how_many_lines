

use std::fs;
// use std::io;






fn main(){
    println!("Hello, world!");
    println!("hello, shiyao!");
    let dir = match fs::read_dir("."){
        Ok(dir) => dir,
        Err(_) => panic!("i dont know")
    };

    for entry in dir {
        if let Ok(name) = entry{
            println!("entry : {:?}", name.path());
        }
    }

}
