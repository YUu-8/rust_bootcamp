use clap::Parser;

#[derive(Parser, Debug)]
#[clap(about = "Count word frequency in text", version, author)]
struct CliArgs {
    /// Text to analyze (or use stdin)
    text: Option<String>,

    /// Show top N words [default: 10]
    #[clap(long, default_value_t = 10)]
    top: u32,

    /// Ignore words shorter than N [default: 1]
    #[clap(long, default_value_t = 1)]
    min_length: u32,

    /// Case insensitive counting
    #[clap(long)]
    ignore_case: bool,
}
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use regex::Regex;
use std::path::Path;

fn main() -> io::Result<()> {
    let args = CliArgs::parse();

    // read input text
let content = if let Some(input) = &args.text {
    if Path::new(input).exists() {
        let mut file = match File::open(input) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Error opening file {}: {}", input, e);
                std::process::exit(1); 
            }
        };
        let mut s = String::new();
        if let Err(e) = file.read_to_string(&mut s) {
            eprintln!("Error reading file {}: {}", input, e);
            std::process::exit(1); 
        }
        s
    } else {
        input.clone()
    }
} else {
    let mut stdin = io::stdin();
    let mut s = String::new();
    if let Err(e) = stdin.read_to_string(&mut s) {
        eprintln!("Error reading from stdin: {}", e);
        std::process::exit(1); // 错误退出
    }
    s
};
    // define regex to match words
    let re = Regex::new(r"\b\w+\b").unwrap();

    // cault words frequency
    let mut word_count: HashMap<String, u32> = HashMap::new();
    for mat in re.find_iter(&content) {
        let word = mat.as_str();
        let word = if args.ignore_case {
            word.to_lowercase()  // if ignore case, convert to lowercase
        } else {
            word.to_string()
        };
        if word.len() >= args.min_length as usize {  // fliter by min_length
            *word_count.entry(word).or_insert(0) += 1;
        }
    }

    // sort words by frequency
    let mut sorted_words: Vec<(&String, &u32)> = word_count.iter().collect();
    sorted_words.sort_by(|a, b| b.1.cmp(a.1));

    // get top N words
    let top = args.top as usize;
    let top_words = &sorted_words[0..std::cmp::min(top, sorted_words.len())];

    println!("Word frequency:");
    for (word, count) in top_words {
        println!("{}: {}", word, count);
    }

    Ok(())
}