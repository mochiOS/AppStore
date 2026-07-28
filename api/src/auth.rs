use subtle::ConstantTimeEq;
use worker::{Headers, Method, Request, RequestInit, Result, wasm_bindgen::JsValue};

use crate::model::{AccountsReleaseAssetEnvelope, GitHubReleaseAsset, GitHubReleaseAssetRequest};

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

pub async fn developer(req: &Request, env: &worker::Env) -> Result<Option<String>> {
    let developer_id = req.headers().get("X-Developer-ID")?.unwrap_or_default();
    let Some(headers) = authorization_headers(req)? else {
        return Ok(None);
    };
    if developer_id.is_empty() {
        return Ok(None);
    }
    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);
    let request = Request::new_with_init(
        &format!("https://developer-ca/v1/developers/{developer_id}"),
        &init,
    )?;
    let response = env.service("DEVELOPER_CA")?.fetch_request(request).await?;
    Ok((response.status_code() == 200).then_some(developer_id))
}

pub struct CertificateIdentity {
    pub public_key: String,
    pub serial_number: String,
}

pub async fn certificate_identity(
    req: &Request,
    env: &worker::Env,
    certificate_id: &str,
    developer_id: &str,
) -> Result<Option<CertificateIdentity>> {
    let Some(headers) = authorization_headers(req)? else {
        return Ok(None);
    };
    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);
    let request = Request::new_with_init(
        &format!("https://developer-ca/v1/certificates/{certificate_id}"),
        &init,
    )?;
    let mut response = env.service("DEVELOPER_CA")?.fetch_request(request).await?;
    if response.status_code() != 200 {
        return Ok(None);
    }
    let value: serde_json::Value = response.json().await?;
    let valid = value.get("status").and_then(|v| v.as_str()) == Some("active")
        && value.get("developer_id").and_then(|v| v.as_str()) == Some(developer_id);
    let public_key = value
        .pointer("/certificate/content/subject_public_key")
        .and_then(|v| v.as_str());
    let serial_number = value
        .pointer("/certificate/content/serial_number")
        .and_then(|v| v.as_str());
    Ok(match (valid, public_key, serial_number) {
        (true, Some(public_key), Some(serial_number)) => Some(CertificateIdentity {
            public_key: public_key.to_owned(),
            serial_number: serial_number.to_owned(),
        }),
        _ => None,
    })
}

pub async fn certificate_is_valid(env: &worker::Env, certificate_id: &str) -> Result<bool> {
    let request = Request::new(
        &format!("https://developer-ca/v1/certificates/{certificate_id}/status"),
        Method::Get,
    )?;
    let mut response = env.service("DEVELOPER_CA")?.fetch_request(request).await?;
    if response.status_code() != 200 {
        return Ok(false);
    }
    let value: serde_json::Value = response.json().await?;
    Ok(value.get("valid").and_then(|value| value.as_bool()) == Some(true))
}

pub async fn github_release_asset(
    req: &Request,
    env: &worker::Env,
    input: &GitHubReleaseAssetRequest<'_>,
) -> Result<std::result::Result<GitHubReleaseAsset, ServiceError>> {
    let authorization = req.headers().get("Authorization")?.unwrap_or_default();
    if !authorization.starts_with("Bearer ") {
        return Ok(Err(ServiceError {
            status: 401,
            code: "UNAUTHENTICATED".into(),
            message: "An Accounts session is required".into(),
        }));
    }
    let headers = Headers::new();
    headers.set("Authorization", &authorization)?;
    headers.set(
        "X-AppStore-Service-Token",
        &env.secret("APPSTORE_SERVICE_TOKEN")?.to_string(),
    )?;
    headers.set("Content-Type", "application/json")?;
    let body = serde_json::to_string(input)?;
    let mut init = RequestInit::new();
    init.with_method(Method::Post)
        .with_headers(headers)
        .with_body(Some(JsValue::from_str(&body)));
    let request =
        Request::new_with_init("https://accounts/v1/internal/github/release-asset", &init)?;
    let mut response = env.service("ACCOUNTS")?.fetch_request(request).await?;
    let status = response.status_code();
    if status == 200 {
        let envelope: AccountsReleaseAssetEnvelope = response.json().await?;
        return Ok(Ok(envelope.release_asset));
    }
    let value: serde_json::Value = response.json().await.unwrap_or_default();
    Ok(Err(ServiceError {
        status,
        code: value
            .pointer("/error/code")
            .and_then(|value| value.as_str())
            .unwrap_or("GITHUB_LOOKUP_FAILED")
            .to_owned(),
        message: value
            .pointer("/error/message")
            .and_then(|value| value.as_str())
            .unwrap_or("GitHub release verification failed")
            .to_owned(),
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
