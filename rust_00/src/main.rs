use clap::Parser;

#[derive(Parser, Debug)]
#[clap(about = "A CLI tool to greet people", version, author)]
struct CliArgs {
    /// Name to greet [default: world]
    name: Option<String>,

    /// Convert greeting to uppercase
    #[clap(short, long)]
    upper: bool,

    /// Repeat greeting N times [default: 1]
    #[clap(short, long, default_value_t = 1)]
    repeat: u32,
}
fn main() {
    let args = CliArgs::parse();

    // default name is "world" if not provided
    let name = args.name.unwrap_or_else(|| "world".to_string());

    // generate greeting message
    let mut greeting = format!("Hello, {}!", name);

    // dealing with uppercase option
    if args.upper {
        greeting = greeting.to_uppercase();
    }

    // dealing with repeat option
    for _ in 0..args.repeat {
        println!("{}", greeting);
    }
}
