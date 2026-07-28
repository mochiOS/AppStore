use std::{
    env,
    fs::File,
    io::{Read, copy},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;
use mochios_mpkg_reviewer::{Expectations, MAX_PACKAGE_BYTES, inspect_mpkg};
use reqwest::{
    Url,
    blocking::{Client, Response},
    redirect::{Action, Attempt, Policy},
};
use serde::Deserialize;
use tempfile::NamedTempFile;

#[derive(Parser)]
#[command(about = "GitHub Releases上のmochiOS .mpkgを安全に検証する")]
struct Args {
    /// AppStore Release ID
    release_id: String,
    /// AppStore API origin
    #[arg(long, default_value = "https://api.store.mochios.org")]
    api: String,
}

#[derive(Deserialize)]
struct ReleaseEnvelope {
    release: Release,
}

#[derive(Deserialize)]
struct Release {
    bundle_id: String,
    version: String,
    file_size: u64,
    download_url: String,
    developer_certificate_id: String,
    developer_certificate_serial: String,
    developer_certificate_subject_key_id: String,
    developer_certificate_developer_id: String,
    developer_certificate_issuer_key_id: String,
    developer_certificate_issuer_public_key: String,
    developer_public_key: String,
    minimum_mochios_version: String,
    validation_status: String,
    review_status: String,
    publish_status: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let token = env::var("APPSTORE_ADMIN_TOKEN").context("APPSTORE_ADMIN_TOKEN is required")?;
    let account_id =
        env::var("APPSTORE_ADMIN_ACCOUNT_ID").context("APPSTORE_ADMIN_ACCOUNT_ID is required")?;
    let unix_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let api = validated_api_origin(&args.api)?;
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .redirect(Policy::custom(github_redirect))
        .user_agent("mochiOS-mpkg-reviewer/0.1")
        .build()?;

    let release_url = format!("{api}/v1/admin/releases/{}", args.release_id);
    let release: ReleaseEnvelope = checked(
        client
            .get(&release_url)
            .header("X-Admin-Token", &token)
            .header("X-Admin-Account-ID", &account_id)
            .send()?,
    )?
    .json()
    .context("AppStore returned an invalid release response")?;
    let release = release.release;
    let issuer_public_key: [u8; 32] = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        release.developer_certificate_issuer_public_key.trim(),
    )
    .context("registered certificate issuer public key is not Base64")?
    .try_into()
    .map_err(|_| anyhow::anyhow!("registered certificate issuer public key must be 32 bytes"))?;
    ensure!(
        release.validation_status == "pending"
            && release.review_status == "pending"
            && release.publish_status == "draft",
        "release is not a pending draft"
    );
    validate_github_url(&release.download_url)?;
    ensure!(
        release.file_size <= MAX_PACKAGE_BYTES,
        "registered package exceeds 128 MiB"
    );

    let mut response = checked(client.get(&release.download_url).send()?)?;
    if let Some(length) = response.content_length() {
        ensure!(
            length == release.file_size,
            "GitHub Content-Length differs from registered size"
        );
    }
    let mut package = NamedTempFile::new().context("failed to create temporary package file")?;
    copy_limited(&mut response, package.as_file_mut(), MAX_PACKAGE_BYTES)?;

    let report = inspect_mpkg(
        package.path(),
        &Expectations {
            package_id: &release.bundle_id,
            version: &release.version,
            certificate_id: &release.developer_certificate_id,
            certificate_serial: &release.developer_certificate_serial,
            certificate_subject_key_id: &release.developer_certificate_subject_key_id,
            certificate_developer_id: &release.developer_certificate_developer_id,
            certificate_issuer_key_id: &release.developer_certificate_issuer_key_id,
            minimum_mochios_version: &release.minimum_mochios_version,
            public_key: &release.developer_public_key,
            issuer_public_key: &issuer_public_key,
            expected_file_size: release.file_size,
            unix_time,
        },
    )?;
    let validation_url = format!("{release_url}/validate");
    checked(
        client
            .post(validation_url)
            .header("X-Admin-Token", token)
            .header("X-Admin-Account-ID", account_id)
            .json(&report)
            .send()?,
    )?;
    println!(
        "validated {} {} ({})",
        report.package_id, report.version, args.release_id
    );
    Ok(())
}

fn validated_api_origin(value: &str) -> Result<String> {
    let url = Url::parse(value).context("--api must be an absolute URL")?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    ensure!(
        url.scheme() == "https" || (local && url.scheme() == "http"),
        "--api must use HTTPS except on localhost"
    );
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "--api cannot contain credentials"
    );
    ensure!(
        url.query().is_none() && url.fragment().is_none(),
        "--api cannot contain a query or fragment"
    );
    Ok(value.trim_end_matches('/').to_owned())
}

fn validate_github_url(value: &str) -> Result<()> {
    let url = Url::parse(value).context("invalid GitHub download URL")?;
    ensure!(
        url.scheme() == "https" && url.host_str() == Some("github.com"),
        "download URL is not GitHub HTTPS"
    );
    ensure!(
        url.username().is_empty() && url.password().is_none() && url.fragment().is_none(),
        "download URL contains forbidden components"
    );
    ensure!(
        url.path().contains("/releases/download/"),
        "download URL is not a fixed release asset URL"
    );
    Ok(())
}

fn github_redirect(attempt: Attempt<'_>) -> Action {
    if attempt.previous().len() > 10 {
        return attempt.error("too many redirects");
    }
    let url = attempt.url();
    let allowed_host = matches!(
        url.host_str(),
        Some(
            "github.com"
                | "objects.githubusercontent.com"
                | "release-assets.githubusercontent.com"
                | "github-releases.githubusercontent.com"
        )
    );
    if url.scheme() == "https"
        && allowed_host
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
    {
        attempt.follow()
    } else {
        attempt.error("GitHub redirected to an untrusted URL")
    }
}

fn checked(response: Response) -> Result<Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let body = response.text().unwrap_or_default();
    bail!(
        "request failed with {status}: {}",
        body.chars().take(512).collect::<String>()
    )
}

fn copy_limited(response: &mut Response, destination: &mut File, limit: u64) -> Result<()> {
    let copied = copy(&mut response.take(limit + 1), destination)?;
    ensure!(copied <= limit, "download exceeds 128 MiB");
    Ok(())
}
