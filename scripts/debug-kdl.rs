use hudlc::parser;
use std::fs;

fn main() {
    let path = "examples/marketing.hudl";
    let content = fs::read_to_string(path).expect("Failed to read file");
    
    println!("Testing KDL parse for: {}", path);
    match parser::parse(&content) {
        Ok(doc) => {
            println!("KDL Parse Successful!");
            // println!("{:#?}", doc);
        },
        Err(e) => {
            println!("KDL Parse FAILED:");
            println!("{}", e);
        }
    }
}