use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, VerifyingKey};
use mochios_certificate::{DeveloperCertificate, SIGNATURE_LEN, key_id};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tar::Archive;

pub const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_EXPANDED_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;
const MAX_METADATA_BYTES: u64 = 1024 * 1024;
const MPKG_HEADER_LEN: usize = 32;
const MANIFEST_PATH: &str = "manifest.toml";
const CERTIFICATE_PATH: &str = "signatures/developer.cert";
const MANIFEST_SIGNATURE_PATH: &str = "signatures/manifest.sig";
const MANIFEST_DOMAIN: &[u8] = b"mochios-mpkg-manifest-v1\0";

#[derive(Debug, Clone)]
struct ObservedFile {
    size: u64,
    sha256: [u8; 32],
}

type ObservedFiles = HashMap<String, ObservedFile>;
type MetadataFiles = HashMap<String, Vec<u8>>;

#[derive(Debug)]
pub struct Expectations<'a> {
    pub package_id: &'a str,
    pub version: &'a str,
    pub certificate_id: &'a str,
    pub certificate_serial: &'a str,
    pub certificate_subject_key_id: &'a str,
    pub certificate_developer_id: &'a str,
    pub certificate_issuer_key_id: &'a str,
    pub minimum_mochios_version: &'a str,
    pub public_key: &'a str,
    pub issuer_public_key: &'a [u8; 32],
    pub expected_file_size: u64,
    pub unix_time: u64,
}

#[derive(Debug, Serialize)]
pub struct ValidationReport {
    pub release_id: String,
    pub asset_id: u64,
    pub reviewer_version: String,
    pub validated_at: u64,
    pub package_id: String,
    pub version: String,
    pub file_size: u64,
    pub asset_sha256: String,
    pub package_digest: String,
    pub manifest_digest: String,
    pub signature: String,
    pub certificate_id: String,
    pub certificate_serial: String,
    pub certificate_subject_key_id: String,
    pub certificate_developer_id: String,
    pub certificate_issuer_key_id: String,
    pub minimum_mochios_version: String,
    pub capabilities: Vec<String>,
    pub payloads: Vec<PayloadReport>,
}

#[derive(Debug, Serialize)]
pub struct PayloadReport {
    pub file_id: String,
    pub container_path: String,
    pub install_path: String,
    pub size: u64,
    pub sha256: String,
    pub mode: String,
}

pub fn inspect_mpkg(path: &Path, expected: &Expectations<'_>) -> Result<ValidationReport> {
    let mut package = File::open(path).context("failed to open downloaded .mpkg")?;
    let file_size = package.metadata()?.len();
    ensure!(file_size >= MPKG_HEADER_LEN as u64, ".mpkg is truncated");
    ensure!(file_size <= MAX_PACKAGE_BYTES, ".mpkg exceeds 128 MiB");
    ensure!(
        file_size == expected.expected_file_size,
        ".mpkg size differs from registered GitHub asset"
    );
    let package_sha256 = hash_reader(&mut package)?;
    package.seek(SeekFrom::Start(0))?;
    validate_header(&mut package, file_size)?;
    validate_ustar_stream(&mut package, file_size - MPKG_HEADER_LEN as u64)?;
    package.seek(SeekFrom::Start(MPKG_HEADER_LEN as u64))?;
    let (files, metadata) = inspect_archive(package)?;

    let manifest_bytes = metadata
        .get(MANIFEST_PATH)
        .context("manifest.toml is required")?;
    let certificate_bytes = metadata
        .get(CERTIFICATE_PATH)
        .context("signatures/developer.cert is required")?;
    let signature_bytes = metadata
        .get(MANIFEST_SIGNATURE_PATH)
        .context("signatures/manifest.sig is required")?;
    let manifest: toml::Value =
        toml::from_str(std::str::from_utf8(manifest_bytes)?).context("manifest.toml is invalid")?;
    let payloads = validate_manifest(&manifest, &files)?;
    let package_id = manifest_string(&manifest, &["package", "id"], "package.id")?;
    let version = manifest_string(&manifest, &["package", "version"], "package.version")?;
    ensure!(
        package_id == expected.package_id,
        "package ID differs from release"
    );
    ensure!(version == expected.version, "version differs from release");

    let certificate = DeveloperCertificate::decode(certificate_bytes)
        .map_err(|error| anyhow::anyhow!("developer.cert is invalid: {error}"))?;
    let mut canonical_certificate = vec![
        0;
        certificate.encoded_len().map_err(
            |error| anyhow::anyhow!("developer.cert cannot be encoded: {error}")
        )?
    ];
    certificate
        .encode(&mut canonical_certificate)
        .map_err(|error| anyhow::anyhow!("developer.cert cannot be encoded: {error}"))?;
    ensure!(
        canonical_certificate == *certificate_bytes,
        "developer.cert is not canonical MCER encoding"
    );
    ensure!(
        key_id(expected.issuer_public_key) == certificate.issuer_key_id,
        "embedded Developer Certificate issuer differs from the trusted DeveloperCA issuer"
    );
    ensure!(
        hex::encode(certificate.issuer_key_id) == expected.certificate_issuer_key_id,
        "embedded Developer Certificate issuer key ID differs from registered certificate"
    );
    certificate
        .verify(expected.issuer_public_key, expected.unix_time, package_id)
        .map_err(|error| anyhow::anyhow!("developer.cert verification failed: {error}"))?;
    ensure!(
        certificate.serial_number.to_string() == expected.certificate_serial,
        "embedded Developer Certificate serial differs from registered certificate"
    );
    let expected_public_key = STANDARD
        .decode(expected.public_key.trim())
        .context("registered Developer public key is not Base64")?;
    ensure!(
        expected_public_key == certificate.subject_public_key,
        "embedded Developer Certificate differs from registered certificate"
    );
    ensure!(
        hex::encode(certificate.subject_key_id) == expected.certificate_subject_key_id,
        "embedded Developer Certificate Subject Key ID differs from registered certificate"
    );
    ensure!(
        certificate.developer_id == expected.certificate_developer_id,
        "embedded Developer Certificate Developer ID differs from registered certificate"
    );
    verify_manifest_signature(&certificate, manifest_bytes, signature_bytes)?;
    let capabilities = validate_capabilities(&manifest, &certificate)?;

    let manifest_digest = hex::encode(Sha256::digest(manifest_bytes));
    Ok(ValidationReport {
        release_id: String::new(),
        asset_id: 0,
        reviewer_version: env!("CARGO_PKG_VERSION").into(),
        validated_at: expected.unix_time,
        package_id: package_id.into(),
        version: version.into(),
        file_size,
        asset_sha256: package_sha256.clone(),
        package_digest: package_sha256,
        manifest_digest,
        signature: STANDARD.encode(signature_bytes),
        certificate_id: expected.certificate_id.into(),
        certificate_serial: expected.certificate_serial.into(),
        certificate_subject_key_id: expected.certificate_subject_key_id.into(),
        certificate_developer_id: expected.certificate_developer_id.into(),
        certificate_issuer_key_id: expected.certificate_issuer_key_id.into(),
        minimum_mochios_version: expected.minimum_mochios_version.into(),
        capabilities,
        payloads,
    })
}

fn validate_header(package: &mut File, file_size: u64) -> Result<()> {
    let mut header = [0u8; MPKG_HEADER_LEN];
    package.read_exact(&mut header)?;
    ensure!(&header[..4] == b"MPKG", "invalid MPKG magic");
    ensure!(
        read_u16(&header, 4) == 1 && read_u16(&header, 6) == 0,
        "unsupported MPKG version"
    );
    ensure!(
        read_u16(&header, 8) as usize == MPKG_HEADER_LEN,
        "invalid MPKG header length"
    );
    ensure!(header[10] == 0, "compressed MPKG is not supported");
    ensure!(
        header[11] == 0 && header[20..].iter().all(|byte| *byte == 0),
        "unknown MPKG flags or non-zero reserved field"
    );
    ensure!(
        read_u64(&header, 12) == file_size - MPKG_HEADER_LEN as u64,
        "MPKG tar stream length mismatch"
    );
    Ok(())
}

fn validate_ustar_stream(package: &mut File, stream_len: u64) -> Result<()> {
    let mut consumed = 0u64;
    let mut entry_count = 0usize;
    let mut paths = BTreeSet::new();
    let mut header = [0u8; 512];
    while consumed < stream_len {
        ensure!(
            stream_len - consumed >= header.len() as u64,
            "MPKG tar stream has a partial header"
        );
        package.read_exact(&mut header)?;
        consumed += header.len() as u64;
        if header.iter().all(|byte| *byte == 0) {
            let mut buffer = [0u8; 64 * 1024];
            while consumed < stream_len {
                let count = usize::try_from((stream_len - consumed).min(buffer.len() as u64))?;
                package.read_exact(&mut buffer[..count])?;
                ensure!(
                    buffer[..count].iter().all(|byte| *byte == 0),
                    "MPKG tar stream contains data after terminator"
                );
                consumed += count as u64;
            }
            return Ok(());
        }
        ensure!(entry_count < MAX_ENTRIES, ".mpkg has too many entries");
        entry_count += 1;
        ensure!(
            &header[257..263] == b"ustar\0" && &header[263..265] == b"00",
            "MPKG tar entry is not canonical ustar"
        );
        ensure!(
            parse_tar_octal(&header[148..156])? == tar_header_checksum(&header),
            "MPKG tar entry checksum mismatch"
        );
        ensure!(
            matches!(header[156], 0 | b'0' | b'5'),
            "MPKG contains unsupported tar entry type"
        );
        let name = tar_cstr(&header[..100])?;
        let prefix = tar_cstr(&header[345..500])?;
        let path = if prefix.is_empty() {
            name
        } else {
            format!("{prefix}/{name}")
        };
        let path = safe_archive_path(path.as_bytes())?;
        ensure!(paths.insert(path), "duplicate archive path");
        let size = parse_tar_octal(&header[124..136])?;
        let padded = size.checked_add(511).context("tar entry size overflow")? / 512 * 512;
        ensure!(
            consumed
                .checked_add(padded)
                .is_some_and(|end| end <= stream_len),
            "MPKG tar entry exceeds stream length"
        );
        package.seek(SeekFrom::Current(i64::try_from(padded)?))?;
        consumed += padded;
    }
    bail!("MPKG tar stream is missing its zero terminator")
}

fn tar_cstr(bytes: &[u8]) -> Result<String> {
    let length = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    ensure!(
        bytes[length..].iter().all(|byte| *byte == 0),
        "tar path field is not canonical"
    );
    Ok(std::str::from_utf8(&bytes[..length])
        .context("tar path is not UTF-8")?
        .to_owned())
}

fn parse_tar_octal(bytes: &[u8]) -> Result<u64> {
    let mut value = 0u64;
    let mut seen_digit = false;
    let mut terminated = false;
    for byte in bytes {
        if matches!(*byte, 0 | b' ') {
            terminated = true;
            continue;
        }
        ensure!(!terminated, "tar numeric field is not canonical");
        ensure!((b'0'..=b'7').contains(byte), "invalid tar numeric field");
        seen_digit = true;
        value = value
            .checked_mul(8)
            .and_then(|current| current.checked_add(u64::from(*byte - b'0')))
            .context("tar numeric field overflow")?;
    }
    Ok(if seen_digit { value } else { 0 })
}

fn tar_header_checksum(header: &[u8; 512]) -> u64 {
    header
        .iter()
        .enumerate()
        .map(|(index, byte)| {
            if (148..156).contains(&index) {
                u64::from(b' ')
            } else {
                u64::from(*byte)
            }
        })
        .sum()
}

fn inspect_archive(package: File) -> Result<(ObservedFiles, MetadataFiles)> {
    let mut archive = Archive::new(package);
    let mut files = HashMap::new();
    let mut metadata = HashMap::new();
    let mut paths = BTreeSet::new();
    let mut expanded = 0u64;
    for (index, entry) in archive.entries()?.enumerate() {
        ensure!(index < MAX_ENTRIES, ".mpkg has too many entries");
        let mut entry = entry?;
        let path = safe_archive_path(entry.path_bytes().as_ref())?;
        ensure!(paths.insert(path.clone()), "duplicate archive path: {path}");
        let entry_type = entry.header().entry_type();
        ensure!(
            path == MANIFEST_PATH
                || path == "signatures"
                || path.starts_with("signatures/")
                || path == "payload"
                || path.starts_with("payload/"),
            "entry outside MPKG roots: {path}"
        );
        if entry_type.is_dir() {
            ensure!(
                path == "signatures" || !path.starts_with("signatures/"),
                "unknown signature directory: {path}"
            );
            continue;
        }
        ensure!(
            entry_type.is_file(),
            "unsupported archive entry type: {path}"
        );
        ensure!(
            !path.starts_with("signatures/chain/"),
            "MPKG v1 does not support certificate chains"
        );
        if path.starts_with("signatures/") {
            ensure!(
                path == CERTIFICATE_PATH || path == MANIFEST_SIGNATURE_PATH,
                "unknown signature entry: {path}"
            );
        }
        let size = entry.size();
        expanded = expanded
            .checked_add(size)
            .context("expanded size overflow")?;
        ensure!(
            expanded <= MAX_EXPANDED_BYTES,
            ".mpkg expands beyond 512 MiB"
        );
        let keep = matches!(
            path.as_str(),
            MANIFEST_PATH | CERTIFICATE_PATH | MANIFEST_SIGNATURE_PATH
        );
        if keep {
            ensure!(
                size <= MAX_METADATA_BYTES,
                "metadata file is too large: {path}"
            );
        }
        let mut hasher = Sha256::new();
        let mut saved = keep.then(Vec::new);
        let mut actual = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let count = entry.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            actual += count as u64;
            hasher.update(&buffer[..count]);
            if let Some(bytes) = &mut saved {
                bytes.extend_from_slice(&buffer[..count]);
            }
        }
        ensure!(actual == size, "archive entry size mismatch: {path}");
        files.insert(
            path.clone(),
            ObservedFile {
                size: actual,
                sha256: hasher.finalize().into(),
            },
        );
        if let Some(bytes) = saved {
            metadata.insert(path, bytes);
        }
    }
    Ok((files, metadata))
}

fn validate_manifest(manifest: &toml::Value, files: &ObservedFiles) -> Result<Vec<PayloadReport>> {
    ensure!(
        manifest.get("format").and_then(toml::Value::as_integer) == Some(1),
        "unsupported manifest format"
    );
    for field in ["id", "name", "version"] {
        manifest_string(manifest, &["package", field], field)?;
    }
    let package_name = manifest_string(manifest, &["package", "name"], "package.name")?;
    ensure!(
        !package_name.contains(['/', '\\']) && package_name != "." && package_name != "..",
        "package.name is unsafe"
    );
    let kind = manifest
        .get("package")
        .and_then(|value| value.get("kind"))
        .and_then(toml::Value::as_str);
    ensure!(
        matches!(kind, None | Some("binary") | Some("application")),
        "unsupported package kind"
    );
    let declared = manifest
        .get("file")
        .and_then(toml::Value::as_array)
        .context("manifest must contain [[file]]")?;
    ensure!(
        !declared.is_empty(),
        "manifest must contain at least one [[file]]"
    );
    let mut expected_paths = BTreeSet::new();
    let mut file_ids = BTreeMap::new();
    let mut payloads = Vec::with_capacity(declared.len());
    for item in declared {
        let id = item
            .get("id")
            .and_then(toml::Value::as_str)
            .context("file.id is missing")?;
        ensure!(!id.is_empty(), "file.id is empty");
        let path = item
            .get("path")
            .and_then(toml::Value::as_str)
            .context("file.path is missing")?;
        let (payload_path, install_path) = manifest_file_paths(kind, package_name, path)?;
        ensure!(
            file_ids.insert(id, install_path.clone()).is_none(),
            "duplicate file.id: {id}"
        );
        ensure!(
            expected_paths.insert(payload_path.clone()),
            "duplicate payload path: {payload_path}"
        );
        let observed = files
            .get(&payload_path)
            .with_context(|| format!("payload is missing: {payload_path}"))?;
        let size = item
            .get("size")
            .and_then(toml::Value::as_integer)
            .and_then(|value| u64::try_from(value).ok())
            .context("file.size is invalid")?;
        ensure!(
            observed.size == size,
            "payload size mismatch: {payload_path}"
        );
        let digest = item
            .get("digest")
            .and_then(toml::Value::as_str)
            .context("file.digest is missing")?;
        ensure!(
            observed.sha256 == decode_sha256(digest)?,
            "payload digest mismatch: {payload_path}"
        );
        let mode = item
            .get("mode")
            .and_then(toml::Value::as_str)
            .context("file.mode is missing")?;
        ensure!(
            mode.len() == 4
                && mode.starts_with('0')
                && mode.bytes().all(|byte| matches!(byte, b'0'..=b'7')),
            "file.mode is invalid"
        );
        payloads.push(PayloadReport {
            file_id: id.into(),
            container_path: payload_path,
            install_path,
            size,
            sha256: hex::encode(observed.sha256),
            mode: mode.into(),
        });
    }
    for payload in files.keys().filter(|path| path.starts_with("payload/")) {
        ensure!(
            expected_paths.contains(payload),
            "manifest does not declare payload: {payload}"
        );
    }
    if let Some(binaries) = manifest.get("binary").and_then(toml::Value::as_array) {
        for binary in binaries {
            let file = binary
                .get("file")
                .and_then(toml::Value::as_str)
                .context("binary.file is missing")?;
            let install_path = file_ids
                .get(file)
                .with_context(|| format!("binary refers to unknown file: {file}"))?;
            let binary_path = binary
                .get("path")
                .and_then(toml::Value::as_str)
                .context("binary.path is missing")?;
            validate_absolute_path(binary_path)?;
            ensure!(
                binary_path == install_path,
                "binary.path differs from referenced file placement"
            );
        }
    }
    payloads.sort_by(|left, right| left.file_id.cmp(&right.file_id));
    Ok(payloads)
}

fn validate_capabilities(
    manifest: &toml::Value,
    certificate: &DeveloperCertificate,
) -> Result<Vec<String>> {
    let allowed: BTreeSet<_> = certificate
        .allowed_capabilities
        .iter()
        .map(String::as_str)
        .collect();
    let mut required = BTreeSet::new();
    if let Some(binaries) = manifest.get("binary").and_then(toml::Value::as_array) {
        for binary in binaries {
            if let Some(requires) = binary.get("requires").and_then(toml::Value::as_array) {
                for capability in requires {
                    let capability = capability
                        .as_str()
                        .context("binary.requires must contain strings")?;
                    ensure!(
                        required.insert(capability.to_owned()),
                        "duplicate required Capability: {capability}"
                    );
                    ensure!(
                        allowed.contains(capability),
                        "Developer Certificate does not allow Capability: {capability}"
                    );
                }
            }
        }
    }
    Ok(required.into_iter().collect())
}

fn verify_manifest_signature(
    certificate: &DeveloperCertificate,
    manifest: &[u8],
    signature: &[u8],
) -> Result<()> {
    let signature: [u8; SIGNATURE_LEN] = signature
        .try_into()
        .map_err(|_| anyhow::anyhow!("manifest.sig must contain exactly 64 bytes"))?;
    let key = VerifyingKey::from_bytes(&certificate.subject_public_key)
        .context("certificate subject key is invalid")?;
    let digest = Sha256::digest(manifest);
    let mut message = Vec::with_capacity(MANIFEST_DOMAIN.len() + digest.len());
    message.extend_from_slice(MANIFEST_DOMAIN);
    message.extend_from_slice(&digest);
    key.verify_strict(&message, &Signature::from_bytes(&signature))
        .context("manifest signature verification failed")
}

fn manifest_file_paths(
    kind: Option<&str>,
    package_name: &str,
    path: &str,
) -> Result<(String, String)> {
    if path.starts_with('/') {
        validate_absolute_path(path)?;
        match kind {
            Some("application") => ensure!(
                path.starts_with("/applications/"),
                "application file is outside /applications"
            ),
            None | Some("binary") => ensure!(
                [
                    "/bin/",
                    "/libraries/",
                    "/binary/services/",
                    "/binary/resources/",
                    "/system/services/"
                ]
                .iter()
                .any(|prefix| path.starts_with(prefix)),
                "binary file uses a forbidden install prefix"
            ),
            _ => bail!("unsupported package kind"),
        }
        return Ok((format!("payload/root{path}"), path.into()));
    }
    let relative = path
        .strip_prefix("$/")
        .context("file.path must be absolute or start with $/")?;
    validate_relative_path(relative)?;
    match kind {
        Some("application") => Ok((
            format!("payload/bundle/{relative}"),
            format!("/applications/{package_name}.app/{relative}"),
        )),
        None | Some("binary") => Ok((
            format!("payload/root/bin/{relative}"),
            format!("/bin/{relative}"),
        )),
        _ => bail!("unsupported package kind"),
    }
}

fn manifest_string<'a>(manifest: &'a toml::Value, path: &[&str], label: &str) -> Result<&'a str> {
    let mut value = manifest;
    for part in path {
        value = value
            .get(*part)
            .with_context(|| format!("manifest is missing {label}"))?;
    }
    let value = value
        .as_str()
        .with_context(|| format!("manifest {label} must be a string"))?;
    ensure!(!value.is_empty(), "manifest {label} is empty");
    Ok(value)
}

fn safe_archive_path(bytes: &[u8]) -> Result<String> {
    let path = std::str::from_utf8(bytes).context("archive path is not UTF-8")?;
    ensure!(
        !path.is_empty()
            && !path.starts_with('/')
            && !path.ends_with('/')
            && !path.contains('\\')
            && !path.contains('\0'),
        "unsafe archive path"
    );
    validate_relative_path(path)?;
    Ok(path.into())
}

fn validate_relative_path(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty()
            && !path.starts_with('/')
            && !path.ends_with('/')
            && !path.contains('\\')
            && !path.contains('\0'),
        "unsafe relative path"
    );
    ensure!(
        path.split('/')
            .all(|part| !part.is_empty() && part != "." && part != ".."),
        "unsafe path segment"
    );
    Ok(())
}

fn validate_absolute_path(path: &str) -> Result<()> {
    let relative = path.strip_prefix('/').context("path must be absolute")?;
    validate_relative_path(relative)
}

fn decode_sha256(value: &str) -> Result<[u8; 32]> {
    let value = value
        .strip_prefix("sha256:")
        .context("file.digest must use sha256:")?;
    ensure!(
        value.len() == 64,
        "SHA-256 digest must contain 64 hex characters"
    );
    hex::decode(value)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid SHA-256 digest"))
}

fn hash_reader(reader: &mut impl Read) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}
fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use mochios_certificate::{KEY_USAGE_PACKAGE_SIGNING, PackageIdScope};
    use std::io::{Cursor, Write};
    use tar::{Builder, EntryType, Header};
    use tempfile::NamedTempFile;

    fn append(builder: &mut Builder<Vec<u8>>, path: &str, bytes: &[u8]) {
        let mut header = Header::new_ustar();
        header.set_size(bytes.len() as u64);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_entry_type(EntryType::Regular);
        header.set_cksum();
        builder
            .append_data(&mut header, path, Cursor::new(bytes))
            .unwrap();
    }

    #[test]
    fn validates_mpkg_with_developer_ca_trusted_issuer() {
        let payload = b"ELF fixture";
        let digest = hex::encode(Sha256::digest(payload));
        let manifest = format!(
            "format = 1\n[package]\nid = \"org.mochios.example\"\nname = \"Example\"\nversion = \"1.0.0\"\nkind = \"application\"\n\n[[file]]\nid = \"main\"\npath = \"$/entry.elf\"\ndigest = \"sha256:{digest}\"\nsize = {}\nmode = \"0755\"\n\n[[binary]]\npath = \"/applications/Example.app/entry.elf\"\nfile = \"main\"\nkind = \"application\"\nrequires = [\"window.create\"]\n",
            payload.len()
        );
        let issuer = SigningKey::from_bytes(&[3; 32]);
        let developer = SigningKey::from_bytes(&[7; 32]);
        let mut certificate = DeveloperCertificate {
            serial_number: 9,
            issuer_key_id: key_id(&issuer.verifying_key().to_bytes()),
            developer_id: "019f9e5ac6687902b0e72fe53abfbef1".into(),
            subject_key_id: key_id(&developer.verifying_key().to_bytes()),
            subject_public_key: developer.verifying_key().to_bytes(),
            not_before: 100,
            not_after: 200,
            key_usage: KEY_USAGE_PACKAGE_SIGNING,
            package_id_scopes: vec![PackageIdScope::exact("org.mochios.example")],
            allowed_capabilities: vec!["window.create".into()],
            signature: [0; 64],
        };
        certificate.signature = issuer
            .sign(&certificate.signing_message().unwrap())
            .to_bytes();
        let mut certificate_wire = vec![0; certificate.encoded_len().unwrap()];
        certificate.encode(&mut certificate_wire).unwrap();
        let mut message = MANIFEST_DOMAIN.to_vec();
        message.extend_from_slice(&Sha256::digest(manifest.as_bytes()));
        let signature = developer.sign(&message).to_bytes();
        let mut builder = Builder::new(Vec::new());
        append(&mut builder, MANIFEST_PATH, manifest.as_bytes());
        append(&mut builder, "payload/bundle/entry.elf", payload);
        append(&mut builder, CERTIFICATE_PATH, &certificate_wire);
        append(&mut builder, MANIFEST_SIGNATURE_PATH, &signature);
        builder.finish().unwrap();
        let tar = builder.into_inner().unwrap();
        let mut bytes = vec![0u8; 32];
        bytes[..4].copy_from_slice(b"MPKG");
        bytes[4..6].copy_from_slice(&1u16.to_le_bytes());
        bytes[8..10].copy_from_slice(&32u16.to_le_bytes());
        bytes[12..20].copy_from_slice(&(tar.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&tar);
        let mut package = NamedTempFile::new().unwrap();
        package.write_all(&bytes).unwrap();
        let serial = certificate.serial_number.to_string();
        let subject_key_id = hex::encode(certificate.subject_key_id);
        let developer_id = certificate.developer_id.clone();
        let issuer_key_id = hex::encode(certificate.issuer_key_id);
        let public_key = STANDARD.encode(developer.verifying_key().to_bytes());
        let issuer_public_key = issuer.verifying_key().to_bytes();
        let inspect_with = |serial_value: &str,
                            subject_value: &str,
                            developer_value: &str,
                            issuer_value: &str,
                            issuer_key: &[u8; 32],
                            unix_time: u64| {
            inspect_mpkg(
                package.path(),
                &Expectations {
                    package_id: "org.mochios.example",
                    version: "1.0.0",
                    certificate_id: "cert_test",
                    certificate_serial: serial_value,
                    certificate_subject_key_id: subject_value,
                    certificate_developer_id: developer_value,
                    certificate_issuer_key_id: issuer_value,
                    minimum_mochios_version: "0.1.0",
                    public_key: &public_key,
                    issuer_public_key: issuer_key,
                    expected_file_size: bytes.len() as u64,
                    unix_time,
                },
            )
        };
        let report = inspect_with(
            &serial,
            &subject_key_id,
            &developer_id,
            &issuer_key_id,
            &issuer_public_key,
            150,
        )
        .unwrap();
        assert_eq!(report.package_id, "org.mochios.example");
        assert!(
            inspect_with(
                "10",
                &subject_key_id,
                &developer_id,
                &issuer_key_id,
                &issuer_public_key,
                150,
            )
            .is_err()
        );
        assert!(
            inspect_with(
                &serial,
                &"00".repeat(32),
                &developer_id,
                &issuer_key_id,
                &issuer_public_key,
                150,
            )
            .is_err()
        );
        assert!(
            inspect_with(
                &serial,
                &subject_key_id,
                "019f9e5ac6687902b0e72fe53abfbef2",
                &issuer_key_id,
                &issuer_public_key,
                150,
            )
            .is_err()
        );
        let unknown_issuer = SigningKey::from_bytes(&[8; 32]).verifying_key().to_bytes();
        assert!(
            inspect_with(
                &serial,
                &subject_key_id,
                &developer_id,
                &issuer_key_id,
                &unknown_issuer,
                150,
            )
            .is_err()
        );
        assert!(
            inspect_with(
                &serial,
                &subject_key_id,
                &developer_id,
                &issuer_key_id,
                &issuer_public_key,
                201,
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_gzip_and_parent_paths() {
        assert!(safe_archive_path(b"../evil").is_err());
        assert!(safe_archive_path(b"payload//evil").is_err());
        assert!(manifest_file_paths(Some("application"), "Example", "/bin/evil").is_err());
        assert!(
            manifest_file_paths(
                Some("binary"),
                "Example",
                "/applications/Evil.app/entry.elf"
            )
            .is_err()
        );
    }
}
