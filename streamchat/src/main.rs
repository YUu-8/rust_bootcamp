use clap::Parser;
use rand::Rng;
use std::io::{self, Write};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const P: u64 = 0xD87FA3E291B4C7F3;
const G: u64 = 2;

fn mod_pow(mut base: u64, mut exp: u64, modu: u64) -> u64 {
    if modu == 1 {
        return 0;
    }
    let mut result = 1;
    base %= modu;
    while exp > 0 {
        if exp % 2 == 1 {
            result = (result * base) % modu;
        }
        exp >>= 1;
        base = (base * base) % modu;
    }
    result
}

struct DHKeys {
    private: u64,
    public: u64,
}

impl DHKeys {
    fn new() -> Self {
        let private = rand::thread_rng().
        let public = mod_pow(G, private, P);
        DHKeys { private, public }
    }
}

fn compute_shared_secret(their_public: u64, our_private: u64) -> u64 {
    mod_pow(their_public, our_private, P)
}

struct LCG {
    a: u64,
    c: u64,
    m: u64,
    state: u64,
}

impl LCG {
    fn new(seed: u64) -> Self {
        LCG {
            a: 1103515245,
            c: 12345,
            m: 1u64 << 32,
            state: seed,
        }
    }

    fn next(&mut self) -> u8 {
        self.state = (self.a * self.state + self.c) % self.m;
        (self.state >> 24) as u8
    }
}

fn xor_crypt(data: &[u8], keystream: &mut LCG) -> Vec<u8> {
    data.iter().map(|&b| b ^ keystream.next()).collect()
}

#[derive(Parser, Debug)]
#[clap(about = "Encrypted chat with Diffie-Hellman and stream cipher")]
enum Command {
    #[clap(name = "server")]
    Server {
        #[clap(short, long, default_value = "8080")]
        port: u16,
    },
    #[clap(name = "client")]
    Client {
        #[clap(short, long)]
        addr: String,
    },
}

#[derive(Parser, Debug)]
struct Args {
    #[clap(subcommand)]
    command: Command,
}

async fn run_server(port: u16) -> io::Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    println!("[SERVER] Listening on 0.0.0.0:{}", port);

    let (mut stream, addr) = listener.accept().await?;
    println!("[SERVER] Client connected from {}", addr);

    let our_keys = DHKeys::new();
    stream.write_all(&our_keys.public.to_be_bytes()).await?;

    let mut their_public_buf = [0u8; 8];
    stream.read_exact(&mut their_public_buf).await?;
    let their_public = u64::from_be_bytes(their_public_buf);

    let shared_secret = compute_shared_secret(their_public, our_keys.private);
    let mut lcg = LCG::new(shared_secret);

    let (mut reader, mut writer) = stream.split();
    let mut input = String::new();
    loop {
        print!("[YOU] ");
        io::stdout().flush()?;
        input.clear();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        let ciphertext = xor_crypt(input.as_bytes(), &mut lcg);
        writer.write_all(&(ciphertext.len() as u32).to_be_bytes()).await?;
        writer.write_all(&ciphertext).await?;

        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut ciphertext = vec![0u8; len];
        reader.read_exact(&mut ciphertext).await?;

        let plaintext = xor_crypt(&ciphertext, &mut lcg);
        let plaintext_str = String::from_utf8_lossy(&plaintext);
        println!("[CLIENT] {}", plaintext_str);
    }
}

async fn run_client(addr: String) -> io::Result<()> {
    let mut stream = TcpStream::connect(addr).await?;
    println!("[CLIENT] Connected to server");

    let our_keys = DHKeys::new();

    let mut their_public_buf = [0u8; 8];
    stream.read_exact(&mut their_public_buf).await?;
    let their_public = u64::from_be_bytes(their_public_buf);

    stream.write_all(&our_keys.public.to_be_bytes()).await?;

    let shared_secret = compute_shared_secret(their_public, our_keys.private);
    let mut lcg = LCG::new(shared_secret);

    let (mut reader, mut writer) = stream.split();
    let mut input = String::new();
    loop {
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut ciphertext = vec![0u8; len];
        reader.read_exact(&mut ciphertext).await?;

        let plaintext = xor_crypt(&ciphertext, &mut lcg);
        let plaintext_str = String::from_utf8_lossy(&plaintext);
        println!("[SERVER] {}", plaintext_str);

        print!("[YOU] ");
        io::stdout().flush()?;
        input.clear();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        let ciphertext = xor_crypt(input.as_bytes(), &mut lcg);
        writer.write_all(&(ciphertext.len() as u32).to_be_bytes()).await?;
        writer.write_all(&ciphertext).await?;
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Server { port } => run_server(port).await,
        Command::Client { addr } => run_client(addr).await,
    }
}