//! custom-signer — generate a signing key and sign `custom.txt` for a custom RustDesk fork.
//!
//! The RustDesk client loads server/app-name/settings overlays from a base64, signed
//! `custom.txt` placed beside the executable (see `read_custom_client` in `src/common.rs`).
//! The signature is verified against the `KEY` public-key constant in that function.
//!
//! Workflow:
//!   1. `custom-signer keygen`  -> prints your PUBLIC key (paste into src/common.rs KEY)
//!                                 and writes the PRIVATE key to `custom-signer.key`.
//!   2. author a `config.json`  (see CUSTOMIZATION.md for keys: app-name, custom-rendezvous-server, ...)
//!   3. `custom-signer sign config.json custom-signer.key`  -> writes `custom.txt`.
//!   4. ship `custom.txt` beside the executable (for portable: bundle it into the package).

use base64::{engine::general_purpose::STANDARD, Engine as _};
use sodiumoxide::crypto::sign;
use std::{fs, process};

fn main() {
    sodiumoxide::init().expect("failed to init libsodium");
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("keygen") => keygen(&args),
        Some("sign") => sign_config(&args),
        _ => {
            eprintln!(
                "custom-signer — sign custom.txt for a RustDesk fork\n\n\
                 Usage:\n  \
                 custom-signer keygen [out_secret_key_file]        (default: custom-signer.key)\n  \
                 custom-signer sign <config.json> <secret_key_file> [out_custom_txt]   (default: custom.txt)"
            );
            process::exit(2);
        }
    }
}

fn keygen(args: &[String]) {
    let out = args.get(2).map(String::as_str).unwrap_or("custom-signer.key");
    let (pk, sk) = sign::gen_keypair();
    fs::write(out, STANDARD.encode(sk.0)).expect("failed to write secret key file");
    println!(
        "Public key (paste as the KEY constant in src/common.rs::read_custom_client):\n\n    {}\n",
        STANDARD.encode(pk.0)
    );
    println!("Private key written to: {out}");
    println!("KEEP IT SECRET — do not commit it. Anyone with it can sign a custom.txt your build trusts.");
}

fn sign_config(args: &[String]) {
    let cfg_path = args.get(2).unwrap_or_else(|| die("missing <config.json>"));
    let key_path = args.get(3).unwrap_or_else(|| die("missing <secret_key_file>"));
    let out = args.get(4).map(String::as_str).unwrap_or("custom.txt");

    let json = fs::read(cfg_path).expect("failed to read config json");
    // Fail early on malformed JSON so you don't ship a custom.txt the client silently drops.
    serde_json::from_slice::<serde_json::Value>(&json).expect("config.json is not valid JSON");

    let sk_b64 = fs::read_to_string(key_path).expect("failed to read secret key file");
    let sk_bytes = STANDARD
        .decode(sk_b64.trim())
        .expect("secret key file is not valid base64");
    let sk = sign::SecretKey::from_slice(&sk_bytes).expect("secret key has wrong length");

    // sign::sign returns signature||message; the client's decode64 + sign::verify recovers the JSON.
    let custom = STANDARD.encode(sign::sign(&json, &sk));
    fs::write(out, &custom).expect("failed to write custom.txt");
    println!("Wrote {out} ({} base64 bytes). Ship it beside the executable.", custom.len());
}

fn die(msg: &str) -> ! {
    eprintln!("error: {msg}");
    process::exit(2);
}
