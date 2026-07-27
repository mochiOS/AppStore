use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Component, Path},
};

use anyhow::{Context, Result, bail, ensure};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use ed25519_dalek::{Signature, VerifyingKey};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tar::Archive;

pub const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;
const MAX_METADATA_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct About {
    name: String,
    #[serde(alias = "package_id")]
    bundle_id: String,
    version: String,
    developer: String,
    entry: String,
    description: String,
    icon: String,
    #[serde(default)]
    resources: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format_version: u32,
    package_id: String,
    version: String,
    minimum_mochios_version: String,
    files: Vec<ManifestFile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestFile {
    path: String,
    size: u64,
    sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EmbeddedSignature {
    format_version: u32,
    algorithm: String,
    certificate_id: String,
    manifest_sha256: String,
    signature: String,
}

#[derive(Debug, Clone)]
struct ObservedFile {
    size: u64,
    sha256: String,
}

type ObservedFiles = HashMap<String, ObservedFile>;
type MetadataFiles = HashMap<String, Vec<u8>>;

#[derive(Debug)]
pub struct Expectations<'a> {
    pub package_id: &'a str,
    pub version: &'a str,
    pub certificate_id: &'a str,
    pub minimum_mochios_version: &'a str,
    pub public_key: &'a str,
    pub expected_file_size: u64,
}

#[derive(Debug, Serialize)]
pub struct ValidationReport {
    pub package_id: String,
    pub version: String,
    pub file_size: u64,
    pub sha256: String,
    pub manifest_hash: String,
    pub signature: String,
    pub certificate_id: String,
    pub minimum_mochios_version: String,
}

pub fn inspect_mpkg(path: &Path, expected: &Expectations<'_>) -> Result<ValidationReport> {
    let mut package = File::open(path).context("failed to open downloaded .mpkg")?;
    let file_size = package.metadata()?.len();
    ensure!(file_size > 0, ".mpkg is empty");
    ensure!(file_size <= MAX_PACKAGE_BYTES, ".mpkg exceeds 128 MiB");
    ensure!(
        file_size == expected.expected_file_size,
        ".mpkg size differs from registered GitHub asset"
    );

    let package_sha256 = hash_reader(&mut package)?;
    package.seek(SeekFrom::Start(0))?;
    let (files, metadata) = inspect_archive(package)?;

    let about_bytes = metadata
        .get("about.toml")
        .context("about.toml is required")?;
    let manifest_bytes = metadata
        .get("META/manifest.toml")
        .context("META/manifest.toml is required")?;
    let signature_bytes = metadata
        .get("META/signature.toml")
        .context("META/signature.toml is required")?;

    let about: About =
        toml::from_str(std::str::from_utf8(about_bytes)?).context("about.toml is invalid")?;
    let manifest: Manifest = toml::from_str(std::str::from_utf8(manifest_bytes)?)
        .context("META/manifest.toml is invalid")?;
    let embedded: EmbeddedSignature = toml::from_str(std::str::from_utf8(signature_bytes)?)
        .context("META/signature.toml is invalid")?;

    validate_about(&about, &files)?;
    validate_manifest(&manifest, &files)?;
    ensure!(manifest.format_version == 1, "unsupported manifest format");
    ensure!(embedded.format_version == 1, "unsupported signature format");
    ensure!(
        embedded.algorithm == "ed25519",
        "unsupported signature algorithm"
    );
    ensure!(
        about.bundle_id == manifest.package_id,
        "about and manifest package IDs differ"
    );
    ensure!(
        about.version == manifest.version,
        "about and manifest versions differ"
    );
    ensure!(
        manifest.package_id == expected.package_id,
        "package ID differs from release"
    );
    ensure!(
        manifest.version == expected.version,
        "version differs from release"
    );
    ensure!(
        embedded.certificate_id == expected.certificate_id,
        "certificate differs from release"
    );
    ensure!(
        manifest.minimum_mochios_version == expected.minimum_mochios_version,
        "minimum mochiOS version differs from release"
    );

    let manifest_hash = hex::encode(Sha256::digest(manifest_bytes));
    ensure!(
        embedded
            .manifest_sha256
            .eq_ignore_ascii_case(&manifest_hash),
        "embedded manifest hash is invalid"
    );
    verify_signature(expected.public_key, &embedded.signature, &manifest_hash)?;

    Ok(ValidationReport {
        package_id: manifest.package_id,
        version: manifest.version,
        file_size,
        sha256: package_sha256,
        manifest_hash,
        signature: embedded.signature,
        certificate_id: embedded.certificate_id,
        minimum_mochios_version: manifest.minimum_mochios_version,
    })
}

fn hash_reader(reader: &mut impl Read) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn inspect_archive(package: File) -> Result<(ObservedFiles, MetadataFiles)> {
    let mut archive = Archive::new(GzDecoder::new(package));
    let mut files = HashMap::new();
    let mut metadata = HashMap::new();
    let mut paths = HashSet::new();
    let mut expanded = 0_u64;

    for (index, entry) in archive.entries()?.enumerate() {
        ensure!(index < MAX_ENTRIES, ".mpkg has too many entries");
        let mut entry = entry?;
        let path = safe_path(&entry.path()?)?;
        ensure!(paths.insert(path.clone()), "duplicate archive path: {path}");
        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            continue;
        }
        ensure!(
            entry_type.is_file(),
            "unsupported archive entry type: {path}"
        );
        let declared_size = entry.size();
        expanded = expanded
            .checked_add(declared_size)
            .context("expanded size overflow")?;
        ensure!(
            expanded <= MAX_EXPANDED_BYTES,
            ".mpkg expands beyond 512 MiB"
        );

        let keep = matches!(
            path.as_str(),
            "about.toml" | "META/manifest.toml" | "META/signature.toml"
        );
        if keep {
            ensure!(
                declared_size <= MAX_METADATA_BYTES,
                "metadata file is too large: {path}"
            );
        }
        let mut hasher = Sha256::new();
        let mut bytes = keep.then(Vec::new);
        let mut actual_size = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = entry.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            actual_size += read as u64;
            hasher.update(&buffer[..read]);
            if let Some(saved) = &mut bytes {
                saved.extend_from_slice(&buffer[..read]);
            }
        }
        ensure!(
            actual_size == declared_size,
            "archive entry size mismatch: {path}"
        );
        files.insert(
            path.clone(),
            ObservedFile {
                size: actual_size,
                sha256: hex::encode(hasher.finalize()),
            },
        );
        if let Some(bytes) = bytes {
            metadata.insert(path, bytes);
        }
    }
    Ok((files, metadata))
}

fn safe_path(path: &Path) -> Result<String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let part = part.to_str().context("archive path is not UTF-8")?;
                ensure!(
                    !part.is_empty() && part != "." && part != "..",
                    "unsafe archive path"
                );
                parts.push(part);
            }
            _ => bail!("unsafe archive path"),
        }
    }
    ensure!(!parts.is_empty(), "empty archive path");
    Ok(parts.join("/"))
}

fn validate_about(about: &About, files: &ObservedFiles) -> Result<()> {
    for (name, value) in [
        ("name", about.name.as_str()),
        ("bundle_id", about.bundle_id.as_str()),
        ("version", about.version.as_str()),
        ("developer", about.developer.as_str()),
        ("entry", about.entry.as_str()),
        ("description", about.description.as_str()),
        ("icon", about.icon.as_str()),
    ] {
        ensure!(!value.trim().is_empty(), "about.toml {name} is required");
    }
    let entry = safe_path(Path::new(&about.entry))?;
    let icon = safe_path(Path::new(&about.icon))?;
    ensure!(
        entry.ends_with(".elf"),
        "application entry must be an ELF file"
    );
    ensure!(files.contains_key(&entry), "application entry is missing");
    ensure!(files.contains_key(&icon), "application icon is missing");
    for resource in &about.resources {
        ensure!(
            files.contains_key(&safe_path(Path::new(resource))?),
            "declared resource is missing"
        );
    }
    Ok(())
}

fn validate_manifest(manifest: &Manifest, files: &ObservedFiles) -> Result<()> {
    let actual: HashSet<_> = files
        .keys()
        .filter(|path| *path != "META/manifest.toml" && *path != "META/signature.toml")
        .cloned()
        .collect();
    let mut declared = HashSet::new();
    for file in &manifest.files {
        let path = safe_path(Path::new(&file.path))?;
        ensure!(
            path != "META/manifest.toml" && path != "META/signature.toml",
            "manifest cannot list its metadata files"
        );
        ensure!(
            declared.insert(path.clone()),
            "duplicate manifest path: {path}"
        );
        let observed = files
            .get(&path)
            .with_context(|| format!("manifest file is missing: {path}"))?;
        ensure!(observed.size == file.size, "manifest size mismatch: {path}");
        ensure!(
            file.sha256.len() == 64 && hex::decode(&file.sha256).is_ok(),
            "invalid manifest hash: {path}"
        );
        ensure!(
            observed.sha256.eq_ignore_ascii_case(&file.sha256),
            "manifest hash mismatch: {path}"
        );
    }
    ensure!(
        declared == actual,
        "manifest does not describe the exact package contents"
    );
    Ok(())
}

fn verify_signature(public_key: &str, signature: &str, manifest_hash: &str) -> Result<()> {
    let decode = |value: &str| {
        STANDARD
            .decode(value.trim())
            .or_else(|_| URL_SAFE_NO_PAD.decode(value.trim()))
    };
    let key: [u8; 32] = decode(public_key)
        .context("certificate public key is not Base64")?
        .try_into()
        .map_err(|_| anyhow::anyhow!("certificate public key has an invalid length"))?;
    let signature = decode(signature).context("embedded signature is not Base64")?;
    let key = VerifyingKey::from_bytes(&key).context("certificate public key is invalid")?;
    let signature =
        Signature::from_slice(&signature).context("embedded signature has an invalid length")?;
    let hash = hex::decode(manifest_hash)?;
    key.verify_strict(&hash, &signature)
        .context("embedded signature verification failed")
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD;
    use ed25519_dalek::{Signer, SigningKey};
    use flate2::{Compression, write::GzEncoder};
    use std::io::Write;
    use tar::{Builder, Header};
    use tempfile::NamedTempFile;

    fn append(builder: &mut Builder<GzEncoder<File>>, path: &str, bytes: &[u8]) {
        let mut header = Header::new_gnu();
        header.set_mode(0o644);
        header.set_size(bytes.len() as u64);
        header.set_cksum();
        builder.append_data(&mut header, path, bytes).unwrap();
    }

    #[test]
    fn validates_a_signed_package_without_extracting_it() {
        let about = br#"name = "Example"
bundle_id = "org.mochios.example"
version = "1.0.0"
developer = "Example Developer"
entry = "entry.elf"
description = "Example application"
icon = "assets/icon.png"
"#;
        let elf = b"ELF fixture";
        let icon = b"PNG fixture";
        let hash = |bytes: &[u8]| hex::encode(Sha256::digest(bytes));
        let manifest = format!(
            r#"format_version = 1
package_id = "org.mochios.example"
version = "1.0.0"
minimum_mochios_version = "0.1.0"

[[files]]
path = "about.toml"
size = {}
sha256 = "{}"

[[files]]
path = "entry.elf"
size = {}
sha256 = "{}"

[[files]]
path = "assets/icon.png"
size = {}
sha256 = "{}"
"#,
            about.len(),
            hash(about),
            elf.len(),
            hash(elf),
            icon.len(),
            hash(icon)
        );
        let manifest_hash = Sha256::digest(manifest.as_bytes());
        let signing_key = SigningKey::from_bytes(&[7_u8; 32]);
        let signature = STANDARD.encode(signing_key.sign(&manifest_hash).to_bytes());
        let signature_toml = format!(
            r#"format_version = 1
algorithm = "ed25519"
certificate_id = "cert_test"
manifest_sha256 = "{}"
signature = "{}"
"#,
            hex::encode(manifest_hash),
            signature
        );

        let package = NamedTempFile::new().unwrap();
        let file = package.reopen().unwrap();
        let encoder = GzEncoder::new(file, Compression::default());
        let mut builder = Builder::new(encoder);
        append(&mut builder, "about.toml", about);
        append(&mut builder, "entry.elf", elf);
        append(&mut builder, "assets/icon.png", icon);
        append(&mut builder, "META/manifest.toml", manifest.as_bytes());
        append(
            &mut builder,
            "META/signature.toml",
            signature_toml.as_bytes(),
        );
        let encoder = builder.into_inner().unwrap();
        encoder.finish().unwrap().flush().unwrap();
        let size = package.as_file().metadata().unwrap().len();

        let report = inspect_mpkg(
            package.path(),
            &Expectations {
                package_id: "org.mochios.example",
                version: "1.0.0",
                certificate_id: "cert_test",
                minimum_mochios_version: "0.1.0",
                public_key: &STANDARD.encode(signing_key.verifying_key().to_bytes()),
                expected_file_size: size,
            },
        )
        .unwrap();
        assert_eq!(report.manifest_hash, hex::encode(manifest_hash));
        assert_eq!(report.file_size, size);
    }

    #[test]
    fn rejects_parent_and_absolute_paths() {
        assert!(safe_path(Path::new("../evil")).is_err());
        assert!(safe_path(Path::new("/absolute")).is_err());
    }
}
