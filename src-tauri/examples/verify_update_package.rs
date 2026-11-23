use anyhow::{Context, Result, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use minisign_verify::{PublicKey, Signature};
use std::{env, fs};

fn decode(encoded: &str) -> Result<String> {
    Ok(String::from_utf8(STANDARD.decode(encoded.trim())?)?)
}

fn main() -> Result<()> {
    let mut args = env::args_os().skip(1);
    let package = args.next().context("missing installer path")?;
    let signature = args.next().context("missing signature path")?;
    let version = args
        .next()
        .and_then(|version| version.into_string().ok())
        .context("missing application version")?;
    ensure!(
        args.next().is_none(),
        "expected installer, signature and version"
    );
    let key = env::var("TAURI_UPDATER_PUBLIC_KEY").context("missing updater public key")?;
    let key = PublicKey::decode(&decode(&key)?)?;
    let signature = Signature::decode(&decode(&fs::read_to_string(signature)?)?)?;
    key.verify(&fs::read(package)?, &signature, false)?;
    let signed_version = signature
        .trusted_comment()
        .split('\t')
        .find_map(|field| field.strip_prefix("version:"));
    ensure!(
        signed_version == Some(version.as_str()),
        "signed updater version does not match the application"
    );
    Ok(())
}
