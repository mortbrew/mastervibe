//! Stub shell. The terminal UI is a later step; this binary only proves the
//! rules core links and that a share code resolves to a secret.

use mastervibes_core::{embedded_catalog, generate_secret, RunSeed};

fn main() {
    let catalog = embedded_catalog();
    let seed = RunSeed::parse("MV-7F3A-9C21-L4").expect("built-in example seed");
    let secret = generate_secret(seed);
    println!("mastervibes {}", env!("CARGO_PKG_VERSION"));
    println!("rules core online — terminal UI is a later step");
    println!(
        "example {seed} → layer {} code {code} (fair: {fair})",
        secret.spec.layer(),
        code = secret.code,
        fair = secret.fair,
    );
    println!("{} signatures loaded", catalog.signatures().len());
}
