use base64::{Engine, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use worker::{Headers, Method, Request, RequestInit, Result, wasm_bindgen::JsValue};

use serde::Deserialize;

use crate::model::{
    AccountsReleaseAssetEnvelope, GitHubReleaseAssetRequest, VerifiedGitHubReleaseAsset,
};

pub struct ServiceError {
    pub status: u16,
    pub code: String,
    pub message: String,
}

fn authorization_headers(req: &Request) -> Result<Option<Headers>> {
    let authorization = req.headers().get("Authorization")?.unwrap_or_default();
    if !authorization.starts_with("Bearer ") {
        return Ok(None);
    }
    let headers = Headers::new();
    headers.set("Authorization", &authorization)?;
    Ok(Some(headers))
}

fn constant_time_eq(expected: &str, provided: &str) -> bool {
    expected.len() == provided.len() && bool::from(expected.as_bytes().ct_eq(provided.as_bytes()))
}

#[derive(Debug, Deserialize)]
struct DeveloperEnvelope {
    developer: DeveloperRecord,
    membership: DeveloperMembership,
}

#[derive(Debug, Deserialize)]
struct DeveloperRecord {
    id: String,
    display_name: String,
    status: String,
    verification_status: String,
}

#[derive(Debug, Deserialize)]
struct DeveloperMembership {
    developer_id: String,
    account_id: String,
    role: String,
    status: String,
}

#[derive(Debug)]
pub struct DeveloperActor {
    pub developer_id: String,
    pub account_id: String,
    pub display_name: String,
    pub role: String,
}

pub async fn developer(req: &Request, env: &worker::Env) -> Result<Option<DeveloperActor>> {
    let developer_id = req.headers().get("X-Developer-ID")?.unwrap_or_default();
    let Some(headers) = authorization_headers(req)? else {
        return Ok(None);
    };
    if !mochios_certificate::is_valid_developer_id(&developer_id) {
        return Ok(None);
    }
    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);
    let request = Request::new_with_init(
        &format!("https://developer-ca/v1/developers/{developer_id}"),
        &init,
    )?;
    let mut response = env.service("DEVELOPER_CA")?.fetch_request(request).await?;
    if response.status_code() != 200 {
        return Ok(None);
    }
    let envelope: DeveloperEnvelope = response.json().await?;
    let authorized = envelope.developer.id == developer_id
        && envelope.developer.status == "active"
        && envelope.developer.verification_status == "verified"
        && envelope.membership.developer_id == developer_id
        && envelope.membership.status == "active"
        && matches!(
            envelope.membership.role.as_str(),
            "owner" | "admin" | "developer"
        )
        && !envelope.membership.account_id.is_empty();
    Ok(authorized.then_some(DeveloperActor {
        developer_id,
        account_id: envelope.membership.account_id,
        display_name: envelope.developer.display_name,
        role: envelope.membership.role,
    }))
}

pub struct CertificateIdentity {
    pub public_key: String,
    pub serial_number: String,
    pub subject_key_id: String,
    pub developer_id: String,
    pub developer_record_id: String,
    pub issuer_key_id: String,
    pub issuer_public_key: String,
    pub issuance_source: String,
}

pub async fn certificate_identity(
    env: &worker::Env,
    certificate_id: &str,
    developer_record_id: &str,
) -> Result<Option<CertificateIdentity>> {
    let request = Request::new(
        &format!("https://developer-ca/v1/certificates/{certificate_id}/status"),
        Method::Get,
    )?;
    let mut response = env.service("DEVELOPER_CA")?.fetch_request(request).await?;
    if response.status_code() != 200 {
        return Ok(None);
    }
    let value: serde_json::Value = response.json().await?;
    let string = |name: &str| value.get(name).and_then(serde_json::Value::as_str);
    let valid = value.get("valid").and_then(serde_json::Value::as_bool) == Some(true)
        && string("status") == Some("valid")
        && string("developer_record_id") == Some(developer_record_id);
    let (
        Some(public_key),
        Some(serial_number),
        Some(subject_key_id),
        Some(developer_id),
        Some(issuer_key_id),
        Some(issuer_public_key),
        Some(issuance_source),
    ) = (
        string("subject_public_key").map(str::to_owned),
        string("serial_number"),
        string("subject_key_id"),
        string("developer_id"),
        string("issuer_key_id"),
        string("issuer_public_key"),
        string("issuance_source"),
    )
    else {
        return Ok(None);
    };
    let public_key_bytes: [u8; 32] = match STANDARD
        .decode(&public_key)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
    {
        Some(bytes) => bytes,
        None => return Ok(None),
    };
    let issuer_public_key_bytes: [u8; 32] = match STANDARD
        .decode(issuer_public_key)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
    {
        Some(bytes) => bytes,
        None => return Ok(None),
    };
    let canonical_serial = serial_number
        .parse::<u64>()
        .ok()
        .filter(|serial| *serial > 0)
        .map(|serial| serial.to_string());
    let identity_is_consistent = valid
        && mochios_certificate::is_valid_developer_id(developer_id)
        && developer_id == developer_record_id
        && canonical_serial.as_deref() == Some(serial_number)
        && hex::encode(Sha256::digest(public_key_bytes)) == subject_key_id
        && hex::encode(Sha256::digest(issuer_public_key_bytes)) == issuer_key_id
        && matches!(issuance_source, "legacy_root" | "online_intermediate");
    Ok(identity_is_consistent.then(|| CertificateIdentity {
        public_key,
        serial_number: serial_number.to_owned(),
        subject_key_id: subject_key_id.to_owned(),
        developer_id: developer_id.to_owned(),
        developer_record_id: developer_record_id.to_owned(),
        issuer_key_id: issuer_key_id.to_owned(),
        issuer_public_key: issuer_public_key.to_owned(),
        issuance_source: issuance_source.to_owned(),
    }))
}

pub async fn github_release_asset_for_account(
    env: &worker::Env,
    account_id: &str,
    input: &GitHubReleaseAssetRequest<'_>,
) -> Result<std::result::Result<VerifiedGitHubReleaseAsset, ServiceError>> {
    let headers = Headers::new();
    headers.set(
        "X-AppStore-Service-Token",
        &env.secret("APPSTORE_SERVICE_TOKEN")?.to_string(),
    )?;
    headers.set("X-AppStore-Account-ID", account_id)?;
    headers.set("Content-Type", "application/json")?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(&serde_json::to_string(input)?)));
    let request =
        Request::new_with_init("https://accounts/v1/internal/github/release-asset", &init)?;
    let mut response = env.service("ACCOUNTS")?.fetch_request(request).await?;
    let status = response.status_code();
    if status == 200 {
        let envelope: AccountsReleaseAssetEnvelope = response.json().await?;
        return Ok(Ok(VerifiedGitHubReleaseAsset {
            account_id: envelope.account_id,
            release_asset: envelope.release_asset,
        }));
    }
    let value: serde_json::Value = response.json().await.unwrap_or_default();
    Ok(Err(ServiceError {
        status,
        code: value
            .pointer("/error/code")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("GITHUB_LOOKUP_FAILED")
            .to_owned(),
        message: "Registered GitHub release asset could not be verified".into(),
    }))
}

pub fn admin(req: &Request, env: &worker::Env) -> Result<Option<String>> {
    let expected = env.secret("ADMIN_TOKEN")?.to_string();
    let provided = req.headers().get("X-Admin-Token")?.unwrap_or_default();
    let actor = req.headers().get("X-Admin-Account-ID")?.unwrap_or_default();
    if expected.is_empty() || actor.is_empty() || !constant_time_eq(&expected, &provided) {
        return Ok(None);
    }
    Ok(Some(actor))
}

pub fn reviewer(req: &Request, env: &worker::Env) -> Result<bool> {
    let expected = env.secret("REVIEWER_TOKEN")?.to_string();
    let provided = req.headers().get("X-Reviewer-Token")?.unwrap_or_default();
    Ok(!expected.is_empty() && constant_time_eq(&expected, &provided))
}
