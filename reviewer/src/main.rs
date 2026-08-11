use std::{
    env,
    fs::File,
    io::{Read, copy},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail, ensure};
use clap::{ArgGroup, Parser};
use mochios_mpkg_reviewer::{Expectations, MAX_PACKAGE_BYTES, ValidationReport, inspect_mpkg};
use reqwest::{
    StatusCode, Url,
    blocking::{Client, Response},
    redirect::{Action, Attempt, Policy},
};
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

#[derive(Parser)]
#[command(about = "GitHub Releases上のmochiOS .mpkgを安全に検証する")]
#[command(group(
    ArgGroup::new("mode")
        .required(true)
        .args(["release_id", "queue"])
))]
struct Args {
    /// AppStore Release ID
    release_id: Option<String>,
    /// 審査待ちReleaseを自動取得して連続処理する
    #[arg(long)]
    queue: bool,
    /// Queueを1回だけ確認し、最大1件を処理して終了する
    #[arg(long, requires = "queue")]
    once: bool,
    /// Queueが空のときの確認間隔（5〜300秒）
    #[arg(long, default_value_t = 15)]
    poll_seconds: u64,
    /// AppStore API origin
    #[arg(long, default_value = "https://api.store.mochios.org")]
    api: String,
    /// DeveloperCA origin
    #[arg(long, default_value = "https://ca.mochios.org")]
    developer_ca: String,
}

#[derive(Deserialize)]
struct ReleaseEnvelope {
    release: Release,
}

#[derive(Deserialize)]
struct Release {
    release_id: String,
    github_asset_id: u64,
    validation_attempt_id: String,
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
    validation_status: String,
    review_status: String,
    publish_status: String,
}

#[derive(Deserialize)]
struct CertificateStatus {
    certificate_id: String,
    status: String,
    valid: bool,
    #[serde(default)]
    serial_number: Option<u64>,
    #[serde(default)]
    developer_id: Option<String>,
    #[serde(default)]
    developer_record_id: Option<String>,
    #[serde(default)]
    subject_key_id: Option<String>,
    #[serde(default)]
    issuer_key_id: Option<String>,
    #[serde(default)]
    issuer_public_key: Option<String>,
}

#[derive(Serialize)]
struct ValidationFailure<'a> {
    release_id: &'a str,
    asset_id: u64,
    validation_attempt_id: &'a str,
    reviewer_version: &'static str,
    validated_at: u64,
    error_code: &'static str,
    error_summary: &'a str,
}

enum ReviewOutcome {
    Valid { package_id: String, version: String },
    Invalid { summary: String },
}

fn main() -> Result<()> {
    let args = Args::parse();
    let token =
        env::var("APPSTORE_REVIEWER_TOKEN").context("APPSTORE_REVIEWER_TOKEN is required")?;
    let api = validated_api_origin(&args.api)?;
    let developer_ca = validated_service_origin(&args.developer_ca, "--developer-ca")?;
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .redirect(Policy::custom(github_redirect))
        .user_agent("mochiOS-mpkg-reviewer/0.1")
        .build()?;

    if args.queue {
        return run_queue(&args, &client, &api, &developer_ca, &token);
    }
    let release_id = args
        .release_id
        .as_deref()
        .context("release ID is required")?;
    let release = claim_release(&client, &api, &token, release_id)?;
    match process_release(&client, &api, &developer_ca, &token, &release)? {
        ReviewOutcome::Valid {
            package_id,
            version,
        } => {
            println!("validated {package_id} {version} ({release_id})");
            Ok(())
        }
        ReviewOutcome::Invalid { summary } => bail!(summary),
    }
}

fn claim_release(client: &Client, api: &str, token: &str, release_id: &str) -> Result<Release> {
    let release_url = format!("{api}/v1/admin/releases/{release_id}");
    let envelope: ReleaseEnvelope = checked(
        client
            .get(&release_url)
            .header("X-Reviewer-Token", token)
            .send()?,
    )?
    .json()
    .context("AppStore returned an invalid release response")?;
    Ok(envelope.release)
}

fn claim_next_release(client: &Client, api: &str, token: &str) -> Result<Option<Release>> {
    let response = client
        .post(format!("{api}/v1/reviewer/releases/claim"))
        .header("X-Reviewer-Token", token)
        .send()
        .context("AppStore Reviewer queue request failed")?;
    if response.status() == StatusCode::NO_CONTENT {
        return Ok(None);
    }
    let envelope: ReleaseEnvelope = checked(response)?
        .json()
        .context("AppStore returned an invalid Reviewer queue response")?;
    Ok(Some(envelope.release))
}

fn process_release(
    client: &Client,
    api: &str,
    developer_ca: &str,
    token: &str,
    release: &Release,
) -> Result<ReviewOutcome> {
    let unix_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    let release_url = format!("{api}/v1/admin/releases/{}", release.release_id);
    let validation_url = format!("{release_url}/validate");
    let report = match review(client, developer_ca, release, unix_time) {
        Ok(report) => report,
        Err(cause) => {
            let summary = format!("{cause:#}").chars().take(500).collect::<String>();
            let error_code = validation_error_code(&summary);
            let failure_url = format!("{release_url}/validation-failure");
            checked(
                client
                    .post(failure_url)
                    .header("X-Reviewer-Token", token)
                    .json(&ValidationFailure {
                        release_id: &release.release_id,
                        asset_id: release.github_asset_id,
                        validation_attempt_id: &release.validation_attempt_id,
                        reviewer_version: env!("CARGO_PKG_VERSION"),
                        validated_at: unix_time,
                        error_code,
                        error_summary: &summary,
                    })
                    .send()?,
            )?;
            return Ok(ReviewOutcome::Invalid { summary });
        }
    };
    checked(
        client
            .post(validation_url)
            .header("X-Reviewer-Token", token)
            .json(&report)
            .send()?,
    )?;
    Ok(ReviewOutcome::Valid {
        package_id: report.package_id,
        version: report.version,
    })
}

fn validation_error_code(summary: &str) -> &'static str {
    if summary.contains("download") || summary.contains("request failed") {
        "download_failed"
    } else if summary.contains("GitHub") || summary.contains("registered") {
        "asset_mismatch"
    } else if summary.contains("certificate") || summary.contains("Certificate") {
        "certificate_invalid"
    } else {
        "package_invalid"
    }
}

fn queue_poll_duration(seconds: u64) -> Result<Duration> {
    ensure!(
        (5..=300).contains(&seconds),
        "--poll-seconds must be between 5 and 300"
    );
    Ok(Duration::from_secs(seconds))
}

fn sleep_while_running(duration: Duration, running: &AtomicBool) {
    let deadline = Instant::now() + duration;
    while running.load(Ordering::SeqCst) {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        thread::sleep((deadline - now).min(Duration::from_secs(1)));
    }
}

fn run_queue(
    args: &Args,
    client: &Client,
    api: &str,
    developer_ca: &str,
    token: &str,
) -> Result<()> {
    let poll = queue_poll_duration(args.poll_seconds)?;
    let running = Arc::new(AtomicBool::new(true));
    let signal = Arc::clone(&running);
    ctrlc::set_handler(move || signal.store(false, Ordering::SeqCst))
        .context("failed to install shutdown handler")?;
    let mut retry_delay = poll;
    println!(
        "Reviewer queue started (poll={}s, mode={})",
        poll.as_secs(),
        if args.once { "once" } else { "continuous" }
    );

    while running.load(Ordering::SeqCst) {
        match claim_next_release(client, api, token) {
            Ok(Some(release)) => {
                retry_delay = poll;
                let release_id = release.release_id.clone();
                println!("claimed {release_id}");
                match process_release(client, api, developer_ca, token, &release) {
                    Ok(ReviewOutcome::Valid {
                        package_id,
                        version,
                    }) => println!("validated {package_id} {version} ({release_id})"),
                    Ok(ReviewOutcome::Invalid { summary }) => {
                        eprintln!("rejected {release_id}: {summary}")
                    }
                    Err(cause) => {
                        eprintln!("Reviewer processing failed for {release_id}: {cause:#}");
                        if args.once {
                            return Err(cause);
                        }
                        sleep_while_running(retry_delay, &running);
                        retry_delay = (retry_delay * 2).min(Duration::from_secs(300));
                    }
                }
                if args.once {
                    return Ok(());
                }
            }
            Ok(None) => {
                retry_delay = poll;
                if args.once {
                    println!("Reviewer queue is empty");
                    return Ok(());
                }
                sleep_while_running(poll, &running);
            }
            Err(cause) => {
                if args.once {
                    return Err(cause);
                }
                eprintln!("Reviewer queue request failed: {cause:#}");
                sleep_while_running(retry_delay, &running);
                retry_delay = (retry_delay * 2).min(Duration::from_secs(300));
            }
        }
    }
    println!("Reviewer queue stopped");
    Ok(())
}

fn review(
    client: &Client,
    developer_ca: &str,
    release: &Release,
    unix_time: u64,
) -> Result<ValidationReport> {
    verify_certificate_status(client, developer_ca, release)?;
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

    let package = download_package(client, release)?;

    let mut report = inspect_mpkg(
        package.path(),
        &Expectations {
            package_id: &release.bundle_id,
            version: &release.version,
            certificate_id: &release.developer_certificate_id,
            certificate_serial: &release.developer_certificate_serial,
            certificate_subject_key_id: &release.developer_certificate_subject_key_id,
            certificate_developer_id: &release.developer_certificate_developer_id,
            certificate_issuer_key_id: &release.developer_certificate_issuer_key_id,
            public_key: &release.developer_public_key,
            issuer_public_key: &issuer_public_key,
            expected_file_size: release.file_size,
            unix_time,
        },
    )?;
    report.release_id = release.release_id.clone();
    report.asset_id = release.github_asset_id;
    report.validation_attempt_id = release.validation_attempt_id.clone();
    Ok(report)
}

fn verify_certificate_status(client: &Client, origin: &str, release: &Release) -> Result<()> {
    ensure!(
        !release.developer_certificate_id.is_empty()
            && release
                .developer_certificate_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')),
        "registered certificate ID is unsafe"
    );
    let url = format!(
        "{origin}/v1/certificates/{}/status",
        release.developer_certificate_id
    );
    let response = client
        .get(url)
        .send()
        .context("DeveloperCA certificate status request failed")?;
    let status: CertificateStatus = checked(response)?
        .json()
        .context("DeveloperCA returned an invalid certificate status response")?;
    validate_certificate_status(&status, release)
}

fn validate_certificate_status(status: &CertificateStatus, release: &Release) -> Result<()> {
    ensure!(
        status.valid && status.status == "valid",
        "DeveloperCA reports certificate as invalid"
    );
    ensure!(
        status.certificate_id == release.developer_certificate_id
            && status
                .serial_number
                .map(|value| value.to_string())
                .as_deref()
                == Some(release.developer_certificate_serial.as_str())
            && status.developer_id.as_deref()
                == Some(release.developer_certificate_developer_id.as_str())
            && status.developer_record_id.as_deref()
                == Some(release.developer_certificate_developer_id.as_str())
            && status.subject_key_id.as_deref()
                == Some(release.developer_certificate_subject_key_id.as_str())
            && status.issuer_key_id.as_deref()
                == Some(release.developer_certificate_issuer_key_id.as_str())
            && status.issuer_public_key.as_deref()
                == Some(release.developer_certificate_issuer_public_key.as_str()),
        "DeveloperCA certificate metadata differs from the registered release"
    );
    Ok(())
}

fn download_package(client: &Client, release: &Release) -> Result<NamedTempFile> {
    let mut last_error = None;
    for _attempt in 0..=2 {
        let result = (|| -> Result<NamedTempFile> {
            let mut response = checked(client.get(&release.download_url).send()?)?;
            if let Some(length) = response.content_length() {
                ensure!(
                    length == release.file_size,
                    "GitHub Content-Length differs from registered size"
                );
            }
            let mut package =
                NamedTempFile::new().context("failed to create temporary package file")?;
            copy_limited(&mut response, package.as_file_mut(), MAX_PACKAGE_BYTES)?;
            ensure!(
                package.as_file().metadata()?.len() == release.file_size,
                "downloaded GitHub asset size differs from registered size"
            );
            Ok(package)
        })();
        match result {
            Ok(package) => return Ok(package),
            Err(cause) => last_error = Some(cause),
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("download failed")))
}

fn validated_api_origin(value: &str) -> Result<String> {
    validated_service_origin(value, "--api")
}

fn validated_service_origin(value: &str, label: &str) -> Result<String> {
    let url = Url::parse(value).with_context(|| format!("{label} must be an absolute URL"))?;
    let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    ensure!(
        url.scheme() == "https" || (local && url.scheme() == "http"),
        "{label} must use HTTPS except on localhost"
    );
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "{label} cannot contain credentials"
    );
    ensure!(
        url.query().is_none() && url.fragment().is_none(),
        "{label} cannot contain a query or fragment"
    );
    ensure!(
        url.path() == "/",
        "{label} must be an origin without a path"
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
        url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none(),
        "download URL contains forbidden components"
    );
    let segments = url
        .path_segments()
        .context("download URL has no path")?
        .collect::<Vec<_>>();
    ensure!(
        segments.len() == 6
            && segments[2] == "releases"
            && segments[3] == "download"
            && !segments[0].is_empty()
            && !segments[1].is_empty()
            && !segments[4].is_empty()
            && segments[4] != "latest"
            && segments[5].ends_with(".mpkg"),
        "download URL is not a fixed release asset URL"
    );
    Ok(())
}

fn github_redirect(attempt: Attempt<'_>) -> Action {
    if attempt.previous().len() >= 10 {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn release() -> Release {
        Release {
            release_id: "rel_test".into(),
            github_asset_id: 42,
            validation_attempt_id: "attempt_test".into(),
            bundle_id: "com.example.testapp".into(),
            version: "0.1.0".into(),
            file_size: 100,
            download_url: "https://github.com/example/test/releases/download/v0.1.0/TestApp.mpkg"
                .into(),
            developer_certificate_id: "cert_test".into(),
            developer_certificate_serial: "2".into(),
            developer_certificate_subject_key_id: "11".repeat(32),
            developer_certificate_developer_id: "019fad830240772ba6fd5f50596afb4c".into(),
            developer_certificate_issuer_key_id: "22".repeat(32),
            developer_certificate_issuer_public_key: "issuer-public-key".into(),
            developer_public_key: "developer-public-key".into(),
            validation_status: "pending".into(),
            review_status: "pending".into(),
            publish_status: "draft".into(),
        }
    }

    fn status(release: &Release) -> CertificateStatus {
        CertificateStatus {
            certificate_id: release.developer_certificate_id.clone(),
            status: "valid".into(),
            valid: true,
            serial_number: Some(2),
            developer_id: Some(release.developer_certificate_developer_id.clone()),
            developer_record_id: Some(release.developer_certificate_developer_id.clone()),
            subject_key_id: Some(release.developer_certificate_subject_key_id.clone()),
            issuer_key_id: Some(release.developer_certificate_issuer_key_id.clone()),
            issuer_public_key: Some(release.developer_certificate_issuer_public_key.clone()),
        }
    }

    #[test]
    fn certificate_status_must_be_current_and_exact() {
        let release = release();
        let mut current = status(&release);
        validate_certificate_status(&current, &release).unwrap();
        current.valid = false;
        current.status = "invalid".into();
        assert!(validate_certificate_status(&current, &release).is_err());
        let mut mismatch = status(&release);
        mismatch.serial_number = Some(3);
        assert!(validate_certificate_status(&mismatch, &release).is_err());
        let mut mismatch = status(&release);
        mismatch.developer_id = Some("019fad830240772ba6fd5f50596afb4d".into());
        assert!(validate_certificate_status(&mismatch, &release).is_err());
        let mut mismatch = status(&release);
        mismatch.subject_key_id = Some("33".repeat(32));
        assert!(validate_certificate_status(&mismatch, &release).is_err());
    }

    #[test]
    fn only_fixed_github_urls_and_safe_service_origins_are_accepted() {
        validate_github_url(
            "https://github.com/example/test/releases/download/v0.1.0/TestApp.mpkg",
        )
        .unwrap();
        assert!(
            validate_github_url(
                "https://github.com/example/test/releases/latest/download/TestApp.mpkg"
            )
            .is_err()
        );
        assert!(validate_github_url("https://example.com/TestApp.mpkg").is_err());
        assert!(
            validate_github_url(
                "https://github.com/example/test/releases/download/v0.1.0/TestApp.mpkg?raw=1"
            )
            .is_err()
        );
        assert!(
            validate_github_url(
                "https://github.com/example/test/other/releases/download/v0.1.0/TestApp.mpkg"
            )
            .is_err()
        );
        assert!(validated_service_origin("https://ca.mochios.org", "--ca").is_ok());
        assert!(validated_service_origin("http://ca.mochios.org", "--ca").is_err());
        assert!(validated_service_origin("http://127.0.0.1:8787", "--ca").is_ok());
        assert!(validated_service_origin("https://ca.mochios.org/base", "--ca").is_err());
    }

    #[test]
    fn reviewer_modes_are_explicit_and_queue_polling_is_bounded() {
        assert!(Args::try_parse_from(["reviewer", "rel_test"]).is_ok());
        assert!(Args::try_parse_from(["reviewer", "--queue"]).is_ok());
        assert!(Args::try_parse_from(["reviewer"]).is_err());
        assert!(Args::try_parse_from(["reviewer", "rel_test", "--queue"]).is_err());
        assert!(Args::try_parse_from(["reviewer", "--once"]).is_err());
        assert!(queue_poll_duration(5).is_ok());
        assert!(queue_poll_duration(300).is_ok());
        assert!(queue_poll_duration(4).is_err());
        assert!(queue_poll_duration(301).is_err());
    }

    #[test]
    fn validation_failures_use_stable_error_codes() {
        assert_eq!(validation_error_code("download failed"), "download_failed");
        assert_eq!(validation_error_code("GitHub mismatch"), "asset_mismatch");
        assert_eq!(
            validation_error_code("certificate expired"),
            "certificate_invalid"
        );
        assert_eq!(validation_error_code("invalid manifest"), "package_invalid");
    }
}
