use clap::Parser;
use rand::Rng;
use std::io::{self, Write};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// Diffie-Hellman public parameters (64-bit prime; use larger primes for real security)
const P: u64 = 0xD87FA3E291B4C7F3;  // Large prime modulus `p`
const G: u64 = 2;                   // Generator `g` (primitive root modulo `p`)

/// Secure modular exponentiation: (base^exp) % modu
/// Uses u128 for intermediate calculations to avoid overflow
/// No unused variable warnings
fn mod_pow(mut base: u64, mut exp: u64, modu: u64) -> u64 {
    if modu == 1 {
        return 0;
    }
    let mut result = 1u128;          // Intermediate result stored in u128 to prevent overflow
    base %= modu;
    let mut base_u128 = base as u128;
    let modu_u128 = modu as u128;

    while exp > 0 {
        if exp % 2 == 1 {
            result = (result * base_u128) % modu_u128;
        }
        exp >>= 1;
        // Only update base_u128 (base variable is unused afterward)
        // Removed redundant assignment to eliminate warning
        base_u128 = (base_u128 * base_u128) % modu_u128;
    }
    result as u64
}

/// Diffie-Hellman key pair (private + public keys)
struct DHKeys {
    private: u64,  // Private key (kept secret)
    public: u64,   // Public key (shared with peer)
}

impl DHKeys {
    /// Generate new DH key pair
    /// Private key range: 2 < private < P (follows DH standard)
    fn new() -> Self {
        let mut rng = rand::thread_rng();
        let private = rng.gen_range(2..P);  // Avoid private key = 0/1/P
        let public = mod_pow(G, private, P);
        DHKeys { private, public }
    }
}

/// Compute shared secret using peer's public key and our private key
fn compute_shared_secret(their_public: u64, our_private: u64) -> u64 {
    mod_pow(their_public, our_private, P)
}

/// Linear Congruential Generator (LCG) for stream cipher keystream
/// Completely fixed overflow issue
struct LCG {
    a: u64,        // Multiplier
    c: u64,        // Increment
    m: u64,        // Modulus
    state: u64,    // Current state (initialized with shared secret)
}

impl LCG {
    /// Initialize LCG with shared secret as seed
    fn new(seed: u64) -> Self {
        LCG {
            a: 1103515245,  // Classic LCG parameters (compatible with C standard library rand())
            c: 12345,
            m: 1u64 << 32,  // 2^32 for stable output range
            state: seed,
        }
    }

    /// Generate next 8-bit key (0-255)
    /// All calculations performed in u128 to prevent overflow
    fn next(&mut self) -> u8 {
        // Force all variables to u128 to avoid multiplication overflow
        let a_u128 = self.a as u128;
        let state_u128 = self.state as u128;
        let c_u128 = self.c as u128;
        let m_u128 = self.m as u128;

        // Perform calculations entirely in u128, then convert back to u64 (safe after mod)
        self.state = ((a_u128 * state_u128 + c_u128) % m_u128) as u64;
        (self.state >> 24) as u8  // Use top 8 bits as key byte
    }
}

/// XOR encryption/decryption (core of stream cipher)
/// Same keystream reverses the process (encrypt → decrypt with same keystream)
fn xor_crypt(data: &[u8], keystream: &mut LCG) -> Vec<u8> {
    data.iter().map(|&b| b ^ keystream.next()).collect()
}

/// CLI argument parser
/// Client `addr` is positional parameter (no need for --addr)
#[derive(Parser, Debug)]
#[clap(about = "Encrypted Chat (Diffie-Hellman + Stream Cipher)")]
enum Command {
    #[clap(name = "server", about = "Start server")]
    Server {
        #[clap(default_value = "8080", help = "Listening port (default: 8080)")]
        port: u16,  // Server port: positional parameter (optional, uses default)
    },
    #[clap(name = "client", about = "Connect to server")]
    Client {
        #[clap(help = "Server address (format: IP:port, e.g., 127.0.0.1:8080)")]
        addr: String,  // Client address: positional parameter (enter directly, no --addr needed)
    },
}

#[derive(Parser, Debug)]
struct Args {
    #[clap(subcommand)]
    command: Command,
}

/// Server logic: Listen → Key exchange → Encrypted chat
async fn run_server(port: u16) -> io::Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    println!("[Server] Listening on 0.0.0.0:{}", port);

    let (mut stream, addr) = listener.accept().await?;
    println!("[Server] Client connected from {}", addr);

    // 1. Generate DH key pair and send public key
    let our_keys = DHKeys::new();
    stream.write_all(&our_keys.public.to_be_bytes()).await?;
    println!("[Server] Sent public key: {}", our_keys.public);

    // 2. Receive client's public key
    let mut their_public_buf = [0u8; 8];
    stream.read_exact(&mut their_public_buf).await?;
    let their_public = u64::from_be_bytes(their_public_buf);
    println!("[Server] Received client's public key: {}", their_public);

    // 3. Compute shared secret and initialize LCG
    let shared_secret = compute_shared_secret(their_public, our_keys.private);
    println!("[Server] Computed shared secret: {}", shared_secret);
    let mut lcg = LCG::new(shared_secret);

    // Split stream into reader/writer
    let (mut reader, mut writer) = stream.split();
    let mut input = String::new();

    // Chat loop
    loop {
        print!("[You] ");
        io::stdout().flush()?;
        input.clear();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if input.is_empty() { continue; }

        // Encrypt and send message (send length first, then ciphertext)
        let ciphertext = xor_crypt(input.as_bytes(), &mut lcg);
        writer.write_all(&(ciphertext.len() as u32).to_be_bytes()).await?;
        writer.write_all(&ciphertext).await?;
        println!("[Server] Sent encrypted message (length: {} bytes)", ciphertext.len());

        // Receive and decrypt message
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut ciphertext = vec![0u8; len];
        reader.read_exact(&mut ciphertext).await?;

        let plaintext = xor_crypt(&ciphertext, &mut lcg);
        println!("[Client] {}", String::from_utf8_lossy(&plaintext));
    }
}

/// Client logic: Connect → Key exchange → Encrypted chat
async fn run_client(addr: String) -> io::Result<()> {
    let mut stream = TcpStream::connect(&addr).await?;
    println!("[Client] Connected to server: {}", addr);

    // 1. Receive server's public key
    let mut their_public_buf = [0u8; 8];
    stream.read_exact(&mut their_public_buf).await?;
    let their_public = u64::from_be_bytes(their_public_buf);
    println!("[Client] Received server's public key: {}", their_public);

    // 2. Generate DH key pair and send public key
    let our_keys = DHKeys::new();
    stream.write_all(&our_keys.public.to_be_bytes()).await?;
    println!("[Client] Sent public key: {}", our_keys.public);

    // 3. Compute shared secret and initialize LCG
    let shared_secret = compute_shared_secret(their_public, our_keys.private);
    println!("[Client] Computed shared secret: {}", shared_secret);
    let mut lcg = LCG::new(shared_secret);

    // Split stream into reader/writer
    let (mut reader, mut writer) = stream.split();
    let mut input = String::new();

    // Chat loop
    loop {
        // Receive and decrypt message
        let mut len_buf = [0u8; 4];
        reader.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;
        let mut ciphertext = vec![0u8; len];
        reader.read_exact(&mut ciphertext).await?;

        let plaintext = xor_crypt(&ciphertext, &mut lcg);
        println!("[Server] {}", String::from_utf8_lossy(&plaintext));

        // Encrypt and send message
        print!("[You] ");
        io::stdout().flush()?;
        input.clear();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if input.is_empty() { continue; }

        let ciphertext = xor_crypt(input.as_bytes(), &mut lcg);
        writer.write_all(&(ciphertext.len() as u32).to_be_bytes()).await?;
        writer.write_all(&ciphertext).await?;
        println!("[Client] Sent encrypted message (length: {} bytes)", ciphertext.len());
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