use subtle::ConstantTimeEq;
use worker::{Headers, Method, Request, RequestInit, Result};

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

pub async fn certificate_public_key(
    req: &Request,
    env: &worker::Env,
    certificate_id: &str,
    developer_id: &str,
) -> Result<Option<String>> {
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
        && value
            .pointer("/certificate/content/developer_id")
            .and_then(|v| v.as_str())
            == Some(developer_id);
    Ok(valid
        .then(|| {
            value
                .pointer("/certificate/content/subject_public_key")
                .and_then(|v| v.as_str())
                .map(str::to_owned)
        })
        .flatten())
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
