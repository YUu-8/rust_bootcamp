use clap::Parser;
use std::fs::OpenOptions;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

#[derive(Parser, Debug)]
#[clap(about = "Read and write binary files in hexadecimal format", version, author)]
#[clap(group(
    clap::ArgGroup::new("mode")
        .required(true)
        .args(&["read", "write"]),
))]
struct Cli {
    #[clap(short, long)]
    file: String,

    #[clap(short, long, group = "mode")]
    read: bool,

    #[clap(short, long, group = "mode")]
    write: Option<String>,

    #[clap(short, long, default_value = "0")]
    offset: String,

    #[clap(short, long, default_value = "16")]
    size: u64,
}

/// Parses offset value 
fn parse_offset(offset_str: &str) -> io::Result<u64> {
    if offset_str.starts_with("0x") {
        u64::from_str_radix(&offset_str[2..], 16)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("Invalid hex offset: {}", e)))
    } else {
        offset_str.parse()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("Invalid decimal offset: {}", e)))
    }
}

/// Parses a hex string into bytes 
fn parse_hex(hex_str: &str) -> io::Result<Vec<u8>> {
    let hex_str = hex_str.replace(" ", "");
    if hex_str.len() % 2 != 0 {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Hex string length must be even"));
    }
    let mut bytes = Vec::new();
    for chunk in hex_str.as_bytes().chunks(2) {
        let hex_chunk = std::str::from_utf8(chunk)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("Invalid hex character: {}", e)))?;
        let byte = u8::from_str_radix(hex_chunk, 16)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, format!("Invalid hex byte: {}", e)))?;
        bytes.push(byte);
    }
    Ok(bytes)
}

/// Formats hex dump output 
fn format_hex_dump(offset: u64, bytes: &[u8]) {
    print!("{:08x}: ", offset);
    for (i, &byte) in bytes.iter().enumerate() {
        print!("{:02x} ", byte);
        if (i + 1) % 8 == 0 && i + 1 != bytes.len() {
            print!(" "); 
        }
    }
    print!(" |");
    for &byte in bytes {
        if byte >= 0x20 && byte <= 0x7e {
            print!("{}", byte as char); 
        } else {
            print!("."); 
        }
    }
    println!("|");
}

fn main() -> io::Result<()> {
    let cli = Cli::parse();

    let offset = parse_offset(&cli.offset)?;
    let file_path = Path::new(&cli.file);

    if cli.read {
        let mut file = OpenOptions::new().read(true).open(&file_path)?;
        file.seek(SeekFrom::Start(offset))?;
        let mut buffer = vec![0; cli.size as usize];
        let bytes_read = file.read(&mut buffer)?;
        buffer.truncate(bytes_read); 
        if !buffer.is_empty() {
            format_hex_dump(offset, &buffer);
        }
    } else if let Some(hex_str) = cli.write {
        let bytes = parse_hex(&hex_str)?;
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true) 
            .open(&file_path)?;
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(&bytes)?;

        println!("Wrote {} bytes at offset 0x{:08x}", bytes.len(), offset);
        print!("Hex:");
        for byte in &bytes {
            print!(" {:02x}", byte);
        }
        println!();
        print!("ASCII:");
        for &byte in &bytes {
            if byte >= 0x20 && byte <= 0x7e {
                print!("{}", byte as char);
            } else {
                print!(".");
            }
        }
        println!();
        println!("✓ Operation successful");
    }

    Ok(())
}
