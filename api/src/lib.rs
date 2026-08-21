mod auth;
mod media;
mod model;
mod store;
pub mod workflow;

use std::collections::HashMap;

use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use ed25519_dalek::{Signature, VerifyingKey};
use futures_util::StreamExt;
use model::*;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use worker::*;

const STATUS_ORIGIN: &str = "https://status.mochios.org";
const STORE_ORIGIN: &str = "https://store.mochios.org";
const CONSOLE_ORIGIN: &str = "https://console.mochios.org";
const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_SAFE_JS_INTEGER: u64 = 9_007_199_254_740_991;
const MAX_JSON_BODY_BYTES: usize = 128 * 1024;

fn now() -> i64 {
    (Date::now().as_millis() / 1000) as i64
}
fn id(prefix: &str) -> String {
    store::id(prefix, now())
}
fn param<'a>(ctx: &'a RouteContext<()>, name: &str) -> &'a str {
    ctx.param(name).map(String::as_str).unwrap_or("")
}

async fn bounded_json<T: DeserializeOwned>(
    req: &mut Request,
) -> Result<std::result::Result<T, Response>> {
    if req
        .headers()
        .get("Content-Length")?
        .and_then(|value| value.parse::<usize>().ok())
        .is_some_and(|length| length > MAX_JSON_BODY_BYTES)
    {
        return Ok(Err(error(
            "REQUEST_TOO_LARGE",
            "JSON request body is too large",
            413,
        )?));
    }
    let bytes = req.bytes().await?;
    if bytes.len() > MAX_JSON_BODY_BYTES {
        return Ok(Err(error(
            "REQUEST_TOO_LARGE",
            "JSON request body is too large",
            413,
        )?));
    }
    Ok(match serde_json::from_slice(&bytes) {
        Ok(value) => Ok(value),
        Err(_) => Err(error("JSON_INVALID", "JSON request body is invalid", 400)?),
    })
}
fn db(ctx: &RouteContext<()>) -> Result<D1Database> {
    ctx.env.d1("DB")
}

async fn rate_limited(
    req: &Request,
    env: &Env,
    binding: &str,
    scope: &str,
) -> Result<Option<Response>> {
    let client = req
        .headers()
        .get("CF-Connecting-IP")?
        .unwrap_or_else(|| "unknown".into());
    if env
        .rate_limiter(binding)?
        .limit(format!("{scope}:{client}"))
        .await?
        .success
    {
        return Ok(None);
    }
    let mut response = error("RATE_LIMITED", "Too many requests", 429)?;
    response.headers_mut().set("Retry-After", "60")?;
    Ok(Some(response))
}

fn json_response<T: Serialize>(value: &T, status: u16) -> Result<Response> {
    Ok(Response::from_json(value)?.with_status(status))
}

fn error(code: &str, message: &str, status: u16) -> Result<Response> {
    json_response(&json!({"error":{"code":code,"message":message}}), status)
}

fn value_str<'a>(value: &'a Value, name: &str) -> Option<&'a str> {
    value.get(name)?.as_str()
}

fn allowed_origin(origin: &str) -> bool {
    matches!(origin, STATUS_ORIGIN | STORE_ORIGIN | CONSOLE_ORIGIN)
        || origin.starts_with("http://127.0.0.1:")
        || origin.starts_with("http://localhost:")
}

fn with_cors(mut response: Response, origin: Option<&str>) -> Result<Response> {
    let Some(origin) = origin.filter(|origin| allowed_origin(origin)) else {
        return Ok(response);
    };
    let headers = response.headers_mut();
    headers.set("Access-Control-Allow-Origin", origin)?;
    headers.set(
        "Access-Control-Allow-Methods",
        "GET, POST, PUT, PATCH, DELETE, OPTIONS",
    )?;
    headers.set(
        "Access-Control-Allow-Headers",
        "Authorization, Content-Type, X-Developer-ID, X-Admin-Token, X-Admin-Account-ID",
    )?;
    headers.set("Access-Control-Max-Age", "3600")?;
    headers.set("Vary", "Origin")?;
    Ok(response)
}

fn valid_bundle_id(value: &str) -> bool {
    mochios_certificate::is_valid_package_id(value)
}

fn valid_optional_text(value: Option<&str>, max_len: usize) -> bool {
    value.is_none_or(|value| value.trim().len() <= max_len)
}

fn valid_icon_url(value: Option<&str>) -> bool {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    Url::parse(value).is_ok_and(|url| {
        let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
        url.scheme() == "https"
            && !host.is_empty()
            && host != "localhost"
            && !host.ends_with(".localhost")
            && !host.ends_with(".local")
            && !host.ends_with(".internal")
            && host.parse::<std::net::IpAddr>().is_err()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none()
    })
}

async fn remote_image(url: &str) -> Result<Option<media::ImageInfo>> {
    let headers = Headers::new();
    headers.set("Accept", "image/png, image/jpeg")?;
    headers.set(
        "Range",
        &format!("bytes=0-{}", media::MAX_INSPECTION_BYTES - 1),
    )?;
    let mut init = RequestInit::new();
    init.with_method(Method::Get).with_headers(headers);
    let request = Request::new_with_init(url, &init)?;
    let mut response = match Fetch::Request(request).send().await {
        Ok(response) => response,
        Err(_) => return Ok(None),
    };
    if !matches!(response.status_code(), 200 | 206)
        || response
            .headers()
            .get("Content-Length")?
            .and_then(|value| value.parse::<usize>().ok())
            .is_some_and(|length| length > media::MAX_INSPECTION_BYTES)
    {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    match response.stream() {
        Ok(mut stream) => {
            while let Some(chunk) = stream.next().await {
                let mut chunk = chunk?;
                if bytes.len() + chunk.len() > media::MAX_INSPECTION_BYTES {
                    return Ok(None);
                }
                bytes.append(&mut chunk);
            }
        }
        Err(_) => {
            bytes = response.bytes().await?;
            if bytes.len() > media::MAX_INSPECTION_BYTES {
                return Ok(None);
            }
        }
    }
    Ok(media::inspect(&bytes))
}

async fn valid_submission_media(input: &SubmissionDraftInput) -> Result<bool> {
    let Some(icon) = remote_image(&input.icon_url).await? else {
        return Ok(false);
    };
    if icon.media_type != input.icon_media_type
        || icon.width != input.icon_width
        || icon.height != input.icon_height
    {
        return Ok(false);
    }
    for screenshot in &input.screenshots {
        if remote_image(&screenshot.image_url).await?.is_none() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn valid_app_metadata(
    display_name: &str,
    subtitle: Option<&str>,
    description: &str,
    icon_url: Option<&str>,
    category: Option<&str>,
    kind: &str,
    age_rating: Option<&str>,
) -> bool {
    !display_name.trim().is_empty()
        && display_name.trim().len() <= 120
        && description.trim().len() <= 4000
        && valid_optional_text(subtitle, 160)
        && valid_optional_text(category, 80)
        && valid_optional_text(age_rating, 40)
        && valid_icon_url(icon_url)
        && matches!(kind, "app" | "game")
}

fn valid_submission_draft(input: &SubmissionDraftInput) -> bool {
    let domains = input
        .external_domains
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let declarations = workflow::DeclarationSummary {
        external_communication: input.external_communication,
        external_communication_reason: input.external_communication_reason.as_deref(),
        external_communication_purpose: input.external_communication_purpose.as_deref(),
        external_domains: &domains,
        collects_data: input.collects_data,
        data_collection_description: input.data_collection_description.as_deref(),
        executes_dynamic_code: input.executes_dynamic_code,
        dynamic_code_explanation: input.dynamic_code_explanation.as_deref(),
        uses_external_updates: input.uses_external_updates,
        external_updates_explanation: input.external_updates_explanation.as_deref(),
        tracks_across_services: input.tracks_across_services,
        tracking_user_consent: input.tracking_user_consent,
        uses_location_for_advertising: input.uses_location_for_advertising,
        requires_login: input.requires_login,
        test_account: input.test_account.as_deref(),
        test_instructions: input.test_instructions.as_deref(),
    };
    workflow::valid_app_name(&input.app_name)
        && workflow::valid_release_channel_name(&input.app_name, &input.release_channel)
        && input.developer_name.trim().chars().count() <= 128
        && !input.developer_name.trim().is_empty()
        && !input.description.trim().is_empty()
        && input.description.trim().chars().count() <= 4000
        && valid_icon_url(Some(&input.icon_url))
        && workflow::valid_icon(&input.icon_media_type, input.icon_width, input.icon_height)
        && workflow::valid_screenshot_set(
            input.screenshots.len(),
            input
                .screenshots
                .iter()
                .filter(|screenshot| screenshot.contains_actual_app_ui)
                .count(),
        )
        && input
            .screenshots
            .iter()
            .all(|screenshot| valid_icon_url(Some(&screenshot.image_url)))
        && matches!(input.kind.as_str(), "app" | "game")
        && matches!(
            input.submission_kind.as_str(),
            "new_app" | "update" | "re_review"
        )
        && input.primary_purpose == "general"
        && input.content_declarations.is_object()
        && valid_optional_text(input.category.as_deref(), 80)
        && valid_optional_text(input.age_rating.as_deref(), 40)
        && input
            .capability_reasons
            .values()
            .all(|reason| reason.trim().chars().count() <= 2000)
        && input.data_categories.iter().all(|category| {
            !category.category.trim().is_empty()
                && category.category.trim().chars().count() <= 80
                && valid_optional_text(category.details.as_deref(), 2000)
        })
        && workflow::valid_declarations(&declarations)
}

fn optional_value(value: Option<&str>) -> worker::wasm_bindgen::JsValue {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map_or(worker::wasm_bindgen::JsValue::NULL, store::value)
}

fn valid_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+'))
}

fn valid_notification_id(value: &str) -> bool {
    value.starts_with("audit_")
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn github_repository(value: &str) -> Option<(&str, &str)> {
    let (owner, repository) = value.split_once('/')?;
    let valid = |part: &str, max: usize| {
        !part.is_empty()
            && part.len() <= max
            && part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    };
    (valid(owner, 39) && valid(repository, 100) && !repository.contains('/'))
        .then_some((owner, repository))
}

fn github_download_url(location: &str) -> Option<Url> {
    let url = Url::parse(location).ok()?;
    let valid = url.scheme() == "https"
        && url.host_str() == Some("github.com")
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
        && url.path().contains("/releases/download/");
    valid.then_some(url)
}

fn redirect_to(location: &str, cache_control: &str) -> Result<Response> {
    let Some(url) = github_download_url(location) else {
        return error(
            "DOWNLOAD_URL_INVALID",
            "Release download URL is invalid",
            500,
        );
    };
    let mut response = Response::empty()?.with_status(302);
    response.headers_mut().set("Location", url.as_str())?;
    response.headers_mut().set("Cache-Control", cache_control)?;
    Ok(response)
}

fn valid_role(value: &str) -> bool {
    matches!(value, "owner" | "admin" | "developer" | "viewer")
}

fn decode_base64(value: &str) -> Option<Vec<u8>> {
    STANDARD
        .decode(value)
        .ok()
        .or_else(|| URL_SAFE_NO_PAD.decode(value).ok())
}

fn valid_hash_signature(public_key: &str, signature: &str, signed_hash: &str) -> bool {
    let Some(key_bytes) = decode_base64(public_key) else {
        return false;
    };
    let Some(signature_bytes) = decode_base64(signature) else {
        return false;
    };
    let Ok(key_array): std::result::Result<[u8; 32], _> = key_bytes.try_into() else {
        return false;
    };
    let Ok(key) = VerifyingKey::from_bytes(&key_array) else {
        return false;
    };
    let Ok(signature) = Signature::from_slice(&signature_bytes) else {
        return false;
    };
    let Ok(hash) = hex::decode(signed_hash) else {
        return false;
    };
    let mut message = b"mochios-mpkg-manifest-v1\0".to_vec();
    message.extend_from_slice(&hash);
    key.verify_strict(&message, &signature).is_ok()
}

fn certificate_matches_release(identity: &auth::CertificateIdentity, release: &Value) -> bool {
    identity.public_key == value_str(release, "developer_public_key").unwrap_or("")
        && identity.serial_number
            == value_str(release, "developer_certificate_serial").unwrap_or("")
        && identity.subject_key_id
            == value_str(release, "developer_certificate_subject_key_id").unwrap_or("")
        && identity.developer_id
            == value_str(release, "developer_certificate_developer_id").unwrap_or("")
        && identity.developer_record_id == value_str(release, "registered_by").unwrap_or("")
        && identity.issuer_key_id
            == value_str(release, "developer_certificate_issuer_key_id").unwrap_or("")
        && identity.issuer_public_key
            == value_str(release, "developer_certificate_issuer_public_key").unwrap_or("")
        && identity.issuance_source
            == value_str(release, "developer_certificate_issuance_source").unwrap_or("")
}

fn js_integer(value: u64) -> Option<f64> {
    (value <= MAX_SAFE_JS_INTEGER).then_some(value as f64)
}

fn page(req: &Request) -> (i64, i64) {
    let Ok(url) = req.url() else {
        return (50, 0);
    };
    let values: HashMap<_, _> = url.query_pairs().into_owned().collect();
    let limit = values
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50)
        .clamp(1, 100);
    let offset = values
        .get("offset")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
        .max(0);
    (limit, offset)
}

async fn require_developer(
    req: &Request,
    env: &Env,
) -> Result<std::result::Result<String, Response>> {
    Ok(match auth::developer(req, env).await? {
        Some(actor) => Ok(actor.developer_id),
        None => Err(error(
            "DEVELOPER_AUTH_REQUIRED",
            "Developer authentication required",
            401,
        )?),
    })
}

async fn require_developer_actor(
    req: &Request,
    env: &Env,
) -> Result<std::result::Result<auth::DeveloperActor, Response>> {
    Ok(match auth::developer(req, env).await? {
        Some(actor) => Ok(actor),
        None => Err(error(
            "DEVELOPER_AUTH_REQUIRED",
            "An active verified Developer membership with owner, admin, or developer role is required",
            403,
        )?),
    })
}

async fn require_account(
    req: &Request,
    env: &Env,
) -> Result<std::result::Result<String, Response>> {
    Ok(match auth::account(req, env).await? {
        Some(account_id) => Ok(account_id),
        None => Err(error(
            "ACCOUNT_AUTH_REQUIRED",
            "An active mochiOS ID account session is required",
            401,
        )?),
    })
}

fn require_admin(req: &Request, env: &Env) -> Result<std::result::Result<String, Response>> {
    Ok(match auth::admin(req, env)? {
        Some(id) => Ok(id),
        None => Err(error(
            "ADMIN_AUTH_REQUIRED",
            "Admin authentication required",
            401,
        )?),
    })
}

fn require_reviewer(req: &Request, env: &Env) -> Result<std::result::Result<(), Response>> {
    Ok(if auth::reviewer(req, env)? {
        Ok(())
    } else {
        Err(error(
            "REVIEWER_AUTH_REQUIRED",
            "Reviewer authentication required",
            401,
        )?)
    })
}

async fn health(_: Request, _: RouteContext<()>) -> Result<Response> {
    let mut response = json_response(&json!({"status":"ok","service":"app-store-api"}), 200)?;
    response.headers_mut().set("Cache-Control", "no-store")?;
    Ok(response)
}

async fn list_apps(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) = rate_limited(&req, &ctx.env, "PUBLIC_RATE_LIMITER", "apps").await? {
        return Ok(response);
    }
    let url = req.url()?;
    let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
    let (limit, offset) = page(&req);
    let apps = store::public_apps(
        &db(&ctx)?,
        query.get("kind").map(String::as_str),
        query.get("category").map(String::as_str),
        None,
        limit,
        offset,
    )
    .await?;
    json_response(&json!({"apps":apps}), 200)
}

async fn search(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) = rate_limited(&req, &ctx.env, "PUBLIC_RATE_LIMITER", "search").await? {
        return Ok(response);
    }
    let url = req.url()?;
    let query = url
        .query_pairs()
        .find(|(key, _)| key == "q")
        .map(|(_, value)| value.into_owned())
        .unwrap_or_default();
    if query.trim().is_empty() {
        return json_response(&json!({"query":"","results":[]}), 200);
    }
    let (limit, offset) = page(&req);
    let results =
        store::public_apps(&db(&ctx)?, None, None, Some(query.trim()), limit, offset).await?;
    json_response(&json!({"query":query,"results":results}), 200)
}

async fn storefront(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) =
        rate_limited(&req, &ctx.env, "PUBLIC_RATE_LIMITER", "storefront").await?
    {
        return Ok(response);
    }
    json_response(&store::storefront(&db(&ctx)?).await?, 200)
}

async fn app_detail(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) =
        rate_limited(&req, &ctx.env, "PUBLIC_RATE_LIMITER", "app-detail").await?
    {
        return Ok(response);
    }
    let bundle_id = param(&ctx, "bundle_id");
    let Some(app) = store::public_app(&db(&ctx)?, bundle_id).await? else {
        return error("APP_NOT_FOUND", "App not found", 404);
    };
    let releases = store::public_releases(&db(&ctx)?, bundle_id).await?;
    let screenshots: Vec<Value> = store::rows(
        &db(&ctx)?,
        "SELECT image_url FROM app_screenshots WHERE bundle_id=?1 ORDER BY position",
        &[store::value(bundle_id)],
    )
    .await?;
    let mut result = serde_json::to_value(app)?;
    result["releases"] = serde_json::to_value(releases)?;
    result["screenshots"] = Value::Array(
        screenshots
            .into_iter()
            .filter_map(|row| row.get("image_url").cloned())
            .collect(),
    );
    json_response(&result, 200)
}

async fn app_releases(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) = rate_limited(&req, &ctx.env, "PUBLIC_RATE_LIMITER", "releases").await? {
        return Ok(response);
    }
    let bundle_id = param(&ctx, "bundle_id");
    if store::public_app(&db(&ctx)?, bundle_id).await?.is_none() {
        return error("APP_NOT_FOUND", "App not found", 404);
    }
    json_response(
        &json!({"bundle_id":bundle_id,"releases":store::public_releases(&db(&ctx)?, bundle_id).await?}),
        200,
    )
}

async fn app_status(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) =
        rate_limited(&req, &ctx.env, "PUBLIC_RATE_LIMITER", "app-status").await?
    {
        return Ok(response);
    }
    let bundle_id = param(&ctx, "bundle_id");
    let Some(app) = store::first::<Value>(
        &db(&ctx)?,
        "SELECT a.bundle_id,a.display_name,COALESCE(v.status,'not_available') status,
                CASE WHEN v.status='removed' THEN v.reason ELSE NULL END reason,v.changed_at
           FROM apps a LEFT JOIN app_availability v ON v.app_id=a.app_id
          WHERE a.bundle_id=?1 LIMIT 1",
        &[store::value(bundle_id)],
    )
    .await?
    else {
        return error("APP_NOT_FOUND", "App not found", 404);
    };
    json_response(&json!({"app":app}), 200)
}

async fn download(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) = rate_limited(&req, &ctx.env, "PUBLIC_RATE_LIMITER", "download").await? {
        return Ok(response);
    }
    let bundle_id = param(&ctx, "bundle_id");
    let version = req
        .url()?
        .query_pairs()
        .find(|(key, _)| key == "version")
        .map(|(_, value)| value.into_owned());
    let database = db(&ctx)?;
    let Some(app) = store::first::<Value>(
        &database,
        "SELECT a.app_id,CASE WHEN v.status IS NULL AND a.visibility='public' THEN 'available'
                    ELSE COALESCE(v.status,'not_available') END status
           FROM apps a LEFT JOIN app_availability v ON v.app_id=a.app_id
           JOIN bundle_ids b ON b.bundle_id=a.bundle_id AND b.status='active'
          WHERE a.bundle_id=?1 LIMIT 1",
        &[store::value(bundle_id)],
    )
    .await?
    else {
        return error("APP_NOT_FOUND", "App not found", 404);
    };
    let status = value_str(&app, "status").unwrap_or("not_available");
    let releases = if status == "available" {
        store::public_releases(&database, bundle_id).await?
    } else if matches!(status, "developer_unpublished" | "removed") {
        let account_id = match require_account(&req, &ctx.env).await? {
            Ok(account_id) => account_id,
            Err(response) => return Ok(response),
        };
        if store::first::<Value>(
            &database,
            "SELECT app_id FROM app_acquisitions WHERE app_id=?1 AND account_id=?2",
            &[
                store::value(value_str(&app, "app_id").unwrap_or("")),
                store::value(&account_id),
            ],
        )
        .await?
        .is_none()
        {
            return error(
                "APP_NOT_ACQUIRED",
                "This account did not acquire the App before it became unavailable",
                403,
            );
        }
        store::acquired_releases(&database, bundle_id).await?
    } else {
        return error("APP_NOT_AVAILABLE", "App is not available", 404);
    };
    let release = releases.into_iter().find(|release| {
        version
            .as_deref()
            .is_none_or(|value| release.version == value)
    });
    let Some(release) = release else {
        return error("RELEASE_NOT_FOUND", "Release not found", 404);
    };
    redirect_to(&release.download_url, "public, max-age=300")
}

async fn acquire_app(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) =
        rate_limited(&req, &ctx.env, "MUTATION_RATE_LIMITER", "app-acquire").await?
    {
        return Ok(response);
    }
    let account_id = match require_account(&req, &ctx.env).await? {
        Ok(account_id) => account_id,
        Err(response) => return Ok(response),
    };
    let bundle_id = param(&ctx, "bundle_id");
    let database = db(&ctx)?;
    let Some(app) = store::first::<Value>(
        &database,
        "SELECT a.app_id FROM apps a JOIN app_availability v ON v.app_id=a.app_id
           JOIN bundle_ids b ON b.bundle_id=a.bundle_id AND b.status='active'
          WHERE a.bundle_id=?1 AND v.status='available'",
        &[store::value(bundle_id)],
    )
    .await?
    else {
        return error(
            "APP_NOT_AVAILABLE",
            "App is not available for acquisition",
            409,
        );
    };
    let app_id = value_str(&app, "app_id").unwrap_or("");
    let timestamp = now();
    database
        .batch(vec![
            database
                .prepare(
                    "INSERT OR IGNORE INTO app_acquisitions(app_id,account_id,first_acquired_at)
                     VALUES(?1,?2,?3)",
                )
                .bind(&[
                    store::value(app_id),
                    store::value(&account_id),
                    store::number(timestamp),
                ])?,
            store::audit_statement(
                &database,
                Some(&account_id),
                "app.acquire",
                "app",
                bundle_id,
                json!({}),
                timestamp,
            )?,
        ])
        .await?;
    let acquisition: Option<Value> = store::first(
        &database,
        "SELECT first_acquired_at FROM app_acquisitions WHERE app_id=?1 AND account_id=?2",
        &[store::value(app_id), store::value(&account_id)],
    )
    .await?;
    json_response(
        &json!({"bundle_id":bundle_id,"account_id":account_id,"acquisition":acquisition}),
        200,
    )
}

async fn bundle_ids(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let developer = match require_developer(&req, &ctx.env).await? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    if req.method() == Method::Get {
        let rows: Vec<Value> = store::rows(
            &db(&ctx)?,
            "SELECT * FROM bundle_ids WHERE developer_id=?1 ORDER BY created_at DESC",
            &[store::value(&developer)],
        )
        .await?;
        return json_response(&json!({"bundle_ids":rows}), 200);
    }
    let mut req = req;
    let input: BundleInput = req.json().await?;
    if !valid_bundle_id(input.bundle_id.trim()) || input.app_name.trim().is_empty() {
        return error("VALIDATION_ERROR", "bundle_id or app_name is invalid", 422);
    }
    let database = db(&ctx)?;
    let bundle_id = input.bundle_id.trim();
    let app_name = input.app_name.trim();
    let existing: Option<Value> = store::first(
        &database,
        "SELECT bundle_id,developer_id,app_name,status,created_at FROM bundle_ids WHERE bundle_id=?1 LIMIT 1",
        &[store::value(bundle_id)],
    )
    .await?;
    if let Some(existing) = existing {
        if value_str(&existing, "developer_id") == Some(developer.as_str()) {
            return json_response(&json!({"bundle_id":bundle_id,"already_reserved":true}), 200);
        }
        return error("BUNDLE_ID_ALREADY_EXISTS", "Bundle ID already exists", 409);
    }
    let timestamp = now();
    database
        .batch(vec![
            database
                .prepare("INSERT INTO bundle_ids(bundle_id,developer_id,app_name,status,created_at) VALUES(?1,?2,?3,'reserved',?4)")
                .bind(&[
                    store::value(bundle_id),
                    store::value(&developer),
                    store::value(app_name),
                    store::number(timestamp),
                ])?,
            store::audit_statement(
                &database,
                Some(&developer),
                "bundle.reserve",
                "bundle_id",
                bundle_id,
                json!({}),
                timestamp,
            )?,
        ])
        .await?;
    json_response(
        &json!({"bundle_id":bundle_id,"already_reserved":false}),
        201,
    )
}

async fn developer_apps(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let developer = match require_developer(&req, &ctx.env).await? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    if req.method() == Method::Get {
        return json_response(
            &json!({"apps":store::developer_apps(&db(&ctx)?, &developer).await?}),
            200,
        );
    }
    let mut req = req;
    let input: AppInput = req.json().await?;
    if !valid_bundle_id(input.bundle_id.trim())
        || !valid_app_metadata(
            &input.display_name,
            input.subtitle.as_deref(),
            &input.description,
            input.icon_url.as_deref(),
            input.category.as_deref(),
            &input.kind,
            input.age_rating.as_deref(),
        )
    {
        return error("VALIDATION_ERROR", "App metadata is invalid", 422);
    }
    let reserved: Option<Value> = store::first(&db(&ctx)?, "SELECT bundle_id FROM bundle_ids WHERE bundle_id=?1 AND developer_id=?2 AND status='reserved'", &[store::value(input.bundle_id.trim()),store::value(&developer)]).await?;
    if reserved.is_none() {
        return error(
            "BUNDLE_ID_NOT_FOUND",
            "Bundle ID is not reserved by this developer",
            404,
        );
    }
    let app_id = id("app");
    let timestamp = now();
    let result = store::run(&db(&ctx)?,
        "INSERT INTO apps(app_id,bundle_id,developer_id,display_name,subtitle,description,icon_url,category,kind,age_rating,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?11)",
        &[store::value(&app_id),store::value(input.bundle_id.trim()),store::value(&developer),store::value(input.display_name.trim()),optional_value(input.subtitle.as_deref()),store::value(input.description.trim()),optional_value(input.icon_url.as_deref()),optional_value(input.category.as_deref()),store::value(&input.kind),optional_value(input.age_rating.as_deref()),store::number(timestamp)]).await;
    if result.is_err() {
        return error("APP_ALREADY_EXISTS", "App already exists", 409);
    }
    store::run(
        &db(&ctx)?,
        "UPDATE bundle_ids SET status='active' WHERE bundle_id=?1",
        &[store::value(input.bundle_id.trim())],
    )
    .await?;
    store::audit(
        &db(&ctx)?,
        Some(&developer),
        "app.create",
        "app",
        input.bundle_id.trim(),
        json!({"app_id":app_id}),
        timestamp,
    )
    .await?;
    json_response(
        &json!({"app":store::developer_app(&db(&ctx)?, &developer, input.bundle_id.trim()).await?}),
        201,
    )
}

async fn developer_app(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if req.method() != Method::Get
        && let Some(response) =
            rate_limited(&req, &ctx.env, "MUTATION_RATE_LIMITER", "app-metadata").await?
    {
        return Ok(response);
    }
    let actor = match require_developer_actor(&req, &ctx.env).await? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let developer = actor.developer_id;
    let bundle_id = param(&ctx, "bundle_id");
    let database = db(&ctx)?;
    let Some(app) = store::developer_app(&database, &developer, bundle_id).await? else {
        return error("APP_NOT_FOUND", "App not found", 404);
    };
    if req.method() == Method::Get {
        return json_response(&json!({"app":app}), 200);
    }
    let mut req = req;
    let input: AppUpdateInput = req.json().await?;
    if !valid_app_metadata(
        &input.display_name,
        input.subtitle.as_deref(),
        &input.description,
        input.icon_url.as_deref(),
        input.category.as_deref(),
        &input.kind,
        input.age_rating.as_deref(),
    ) {
        return error("VALIDATION_ERROR", "App metadata is invalid", 422);
    }
    let timestamp = now();
    database
        .batch(vec![
            database
                .prepare(
                    "UPDATE apps SET display_name=?1,subtitle=?2,description=?3,icon_url=?4,
                        category=?5,kind=?6,age_rating=?7,updated_at=?8
                      WHERE developer_id=?9 AND bundle_id=?10",
                )
                .bind(&[
                    store::value(input.display_name.trim()),
                    optional_value(input.subtitle.as_deref()),
                    store::value(input.description.trim()),
                    optional_value(input.icon_url.as_deref()),
                    optional_value(input.category.as_deref()),
                    store::value(&input.kind),
                    optional_value(input.age_rating.as_deref()),
                    store::number(timestamp),
                    store::value(&developer),
                    store::value(bundle_id),
                ])?,
            store::audit_statement(
                &database,
                Some(&actor.account_id),
                "app.metadata_update",
                "app",
                bundle_id,
                json!({"developer_id":developer}),
                timestamp,
            )?,
        ])
        .await?;
    json_response(
        &json!({"app":store::developer_app(&database, &developer, bundle_id).await?}),
        200,
    )
}

fn bool_value(value: bool) -> worker::wasm_bindgen::JsValue {
    store::number(i64::from(value))
}

fn draft_detail_statements(
    database: &D1Database,
    submission_id: &str,
    input: &SubmissionDraftInput,
    capabilities: &[String],
    replace: bool,
) -> Result<Vec<worker::D1PreparedStatement>> {
    let mut statements = Vec::new();
    if replace {
        for table in [
            "submission_screenshots",
            "submission_capabilities",
            "submission_network_domains",
            "submission_data_categories",
            "submission_details",
        ] {
            statements.push(
                database
                    .prepare(format!("DELETE FROM {table} WHERE submission_id=?1"))
                    .bind(&[store::value(submission_id)])?,
            );
        }
    }
    statements.push(
        database.prepare(
            "INSERT INTO submission_details(
               submission_id,app_name,developer_name,description,icon_url,icon_media_type,
               icon_width,icon_height,category,kind,release_channel,primary_purpose,age_rating,
               external_communication,external_communication_reason,external_communication_purpose,
               collects_data,data_collection_description,uses_advertising,uses_analytics,
               tracks_across_services,tracking_user_consent,uses_location_for_advertising,
               has_payments,content_declarations_json,executes_dynamic_code,dynamic_code_explanation,
               uses_external_updates,external_updates_explanation,is_emulator,is_virtual_machine,
               supports_plugins,is_external_app_store,uses_ai_generated_content,
               disclose_ai_generated_content,reviewer_notes,requires_login,test_account,test_instructions)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,
                    ?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31,?32,?33,?34,
                    ?35,?36,?37,?38,?39)",
        ).bind(&[
            store::value(submission_id),
            store::value(input.app_name.trim()),
            store::value(input.developer_name.trim()),
            store::value(input.description.trim()),
            store::value(input.icon_url.trim()),
            store::value(&input.icon_media_type),
            store::number(input.icon_width as i64),
            store::number(input.icon_height as i64),
            optional_value(input.category.as_deref()),
            store::value(&input.kind),
            store::value(&input.release_channel),
            store::value(&input.primary_purpose),
            optional_value(input.age_rating.as_deref()),
            bool_value(input.external_communication),
            optional_value(input.external_communication_reason.as_deref()),
            optional_value(input.external_communication_purpose.as_deref()),
            bool_value(input.collects_data),
            optional_value(input.data_collection_description.as_deref()),
            bool_value(input.uses_advertising),
            bool_value(input.uses_analytics),
            bool_value(input.tracks_across_services),
            bool_value(input.tracking_user_consent),
            bool_value(input.uses_location_for_advertising),
            bool_value(input.has_payments),
            store::value(input.content_declarations.to_string()),
            bool_value(input.executes_dynamic_code),
            optional_value(input.dynamic_code_explanation.as_deref()),
            bool_value(input.uses_external_updates),
            optional_value(input.external_updates_explanation.as_deref()),
            bool_value(input.is_emulator),
            bool_value(input.is_virtual_machine),
            bool_value(input.supports_plugins),
            bool_value(input.is_external_app_store),
            bool_value(input.uses_ai_generated_content),
            bool_value(input.disclose_ai_generated_content),
            optional_value(input.reviewer_notes.as_deref()),
            bool_value(input.requires_login),
            optional_value(input.test_account.as_deref()),
            optional_value(input.test_instructions.as_deref()),
        ])?,
    );
    for (position, screenshot) in input.screenshots.iter().enumerate() {
        statements.push(
            database.prepare(
                "INSERT INTO submission_screenshots(submission_id,position,image_url,contains_actual_app_ui)
                 VALUES(?1,?2,?3,?4)",
            ).bind(&[
                store::value(submission_id),
                store::number(position as i64),
                store::value(screenshot.image_url.trim()),
                bool_value(screenshot.contains_actual_app_ui),
            ])?,
        );
    }
    for capability in capabilities {
        statements.push(
            database.prepare(
                "INSERT INTO submission_capabilities(submission_id,capability,source,usage_reason)
                 VALUES(?1,?2,'manifest',?3)",
            ).bind(&[
                store::value(submission_id),
                store::value(capability),
                optional_value(input.capability_reasons.get(capability).map(String::as_str)),
            ])?,
        );
    }
    for domain in &input.external_domains {
        statements.push(
            database
                .prepare(
                    "INSERT INTO submission_network_domains(submission_id,domain) VALUES(?1,?2)",
                )
                .bind(&[store::value(submission_id), store::value(domain.trim())])?,
        );
    }
    for category in &input.data_categories {
        statements.push(
            database
                .prepare(
                    "INSERT INTO submission_data_categories(submission_id,category,details)
                 VALUES(?1,?2,?3)",
                )
                .bind(&[
                    store::value(submission_id),
                    store::value(category.category.trim()),
                    optional_value(category.details.as_deref()),
                ])?,
        );
    }
    Ok(statements)
}

async fn submission_payload(database: &D1Database, submission_id: &str) -> Result<Option<Value>> {
    let Some(mut submission) = store::first::<Value>(
        database,
        "SELECT s.*,d.*,a.bundle_id,a.developer_id,b.machine_status,b.github_repository,
                b.github_release_tag,b.asset_name,b.file_size,b.sha256,b.package_digest,
                b.manifest_digest,b.certificate_id
           FROM submissions s JOIN submission_details d USING(submission_id)
           JOIN apps a ON a.app_id=s.app_id JOIN app_builds b ON b.build_id=s.build_id
          WHERE s.submission_id=?1",
        &[store::value(submission_id)],
    )
    .await?
    else {
        return Ok(None);
    };
    for (key, table, columns) in [
        (
            "screenshots",
            "submission_screenshots",
            "position,image_url,contains_actual_app_ui",
        ),
        (
            "capabilities",
            "submission_capabilities",
            "capability,source,usage_reason",
        ),
        ("external_domains", "submission_network_domains", "domain"),
        (
            "data_categories",
            "submission_data_categories",
            "category,details",
        ),
        (
            "reviews",
            "submission_reviews",
            "review_id,reviewer_account_id,decision,reason,created_at",
        ),
        (
            "messages",
            "submission_messages",
            "message_id,author_account_id,author_role,body,created_at",
        ),
    ] {
        let order = match table {
            "submission_screenshots" => "position",
            "submission_reviews" | "submission_messages" => "created_at",
            _ => "1",
        };
        let rows: Vec<Value> = store::rows(
            database,
            &format!("SELECT {columns} FROM {table} WHERE submission_id=?1 ORDER BY {order}"),
            &[store::value(submission_id)],
        )
        .await?;
        submission[key] = Value::Array(rows);
    }
    Ok(Some(submission))
}

async fn valid_previous_submission(
    database: &D1Database,
    app_id: &str,
    previous_submission_id: Option<&str>,
) -> Result<bool> {
    let Some(previous_submission_id) = previous_submission_id else {
        return Ok(true);
    };
    Ok(store::first::<Value>(
        database,
        "SELECT submission_id FROM submissions WHERE submission_id=?1 AND app_id=?2
          AND state IN ('changes_required','rejected')",
        &[store::value(previous_submission_id), store::value(app_id)],
    )
    .await?
    .is_some())
}

async fn developer_submissions(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_developer_actor(&req, &ctx.env).await? {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let bundle_id = param(&ctx, "bundle_id");
    let database = db(&ctx)?;
    let Some(app) = store::first::<Value>(
        &database,
        "SELECT app_id FROM apps WHERE bundle_id=?1 AND developer_id=?2",
        &[store::value(bundle_id), store::value(&actor.developer_id)],
    )
    .await?
    else {
        return error("APP_NOT_FOUND", "App not found", 404);
    };
    let app_id = value_str(&app, "app_id").unwrap_or("");
    if req.method() == Method::Get {
        let submissions: Vec<Value> = store::rows(
            &database,
            "SELECT s.submission_id,s.build_id,s.version,s.submission_number,s.submission_kind,
                    s.state,s.previous_submission_id,s.created_at,s.updated_at,s.submitted_at,
                    d.app_name,b.machine_status
               FROM submissions s JOIN submission_details d USING(submission_id)
               JOIN app_builds b ON b.build_id=s.build_id
              WHERE s.app_id=?1 ORDER BY s.created_at DESC,s.submission_number DESC",
            &[store::value(app_id)],
        )
        .await?;
        return json_response(&json!({"submissions":submissions}), 200);
    }
    if let Some(response) =
        rate_limited(&req, &ctx.env, "MUTATION_RATE_LIMITER", "submission-draft").await?
    {
        return Ok(response);
    }
    let input: SubmissionDraftInput = match bounded_json(&mut req).await? {
        Ok(input) => input,
        Err(response) => return Ok(response),
    };
    if !valid_submission_draft(&input) {
        return error(
            "SUBMISSION_INVALID",
            "Submission information is invalid",
            422,
        );
    }
    if !valid_submission_media(&input).await? {
        return error(
            "SUBMISSION_MEDIA_INVALID",
            "Icon and screenshots must be reachable PNG or JPEG images; the icon must be exactly 512 by 512 pixels",
            422,
        );
    }
    let Some(build) = store::first::<Value>(
        &database,
        "SELECT build_id,version,capabilities_json FROM app_builds WHERE build_id=?1 AND app_id=?2",
        &[store::value(input.build_id.trim()), store::value(app_id)],
    )
    .await?
    else {
        return error("BUILD_NOT_FOUND", "Build not found", 404);
    };
    if !valid_previous_submission(&database, app_id, input.previous_submission_id.as_deref())
        .await?
    {
        return error(
            "PREVIOUS_SUBMISSION_INVALID",
            "Previous Submission must be a Changes Required or Rejected Submission for this App",
            422,
        );
    }
    let capabilities = value_str(&build, "capabilities_json")
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default();
    if input
        .capability_reasons
        .keys()
        .any(|capability| !capabilities.contains(capability))
    {
        return error(
            "CAPABILITY_MISMATCH",
            "Capabilities must come from the verified Build",
            422,
        );
    }
    if let Some(previous) = input.previous_submission_id.as_deref() {
        let valid_previous: Option<Value> = store::first(
            &database,
            "SELECT submission_id FROM submissions WHERE submission_id=?1 AND app_id=?2
              AND state IN ('changes_required','rejected')",
            &[store::value(previous), store::value(app_id)],
        )
        .await?;
        if valid_previous.is_none() {
            return error(
                "PREVIOUS_SUBMISSION_INVALID",
                "Previous Submission cannot be reused",
                409,
            );
        }
    }
    let sequence: Option<Value> = store::first(
        &database,
        "SELECT COALESCE(MAX(submission_number),0)+1 AS value FROM submissions WHERE app_id=?1",
        &[store::value(app_id)],
    )
    .await?;
    let submission_number = sequence
        .as_ref()
        .and_then(|value| value.get("value"))
        .and_then(Value::as_f64)
        .unwrap_or(1.0) as i64;
    let submission_id = id("sub");
    let timestamp = now();
    let mut statements = vec![
        database.prepare(
            "INSERT INTO submissions(submission_id,app_id,build_id,version,submission_number,
               submission_kind,state,previous_submission_id,created_by_account_id,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,'draft',?7,?8,?9,?9)",
        ).bind(&[
            store::value(&submission_id), store::value(app_id), store::value(input.build_id.trim()),
            store::value(value_str(&build, "version").unwrap_or("")), store::number(submission_number),
            store::value(&input.submission_kind), optional_value(input.previous_submission_id.as_deref()),
            store::value(&actor.account_id), store::number(timestamp),
        ])?,
    ];
    statements.extend(draft_detail_statements(
        &database,
        &submission_id,
        &input,
        &capabilities,
        false,
    )?);
    statements.push(store::audit_statement(
        &database,
        Some(&actor.account_id),
        "submission.create",
        "submission",
        &submission_id,
        json!({"developer_id":actor.developer_id,"bundle_id":bundle_id}),
        timestamp,
    )?);
    database.batch(statements).await?;
    json_response(
        &json!({"submission":submission_payload(&database, &submission_id).await?}),
        201,
    )
}

async fn developer_submission(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_developer_actor(&req, &ctx.env).await? {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let bundle_id = param(&ctx, "bundle_id");
    let submission_id = param(&ctx, "submission_id");
    let database = db(&ctx)?;
    let owned: Option<Value> = store::first(
        &database,
        "SELECT s.state,s.app_id FROM submissions s JOIN apps a ON a.app_id=s.app_id
          WHERE s.submission_id=?1 AND a.bundle_id=?2 AND a.developer_id=?3",
        &[
            store::value(submission_id),
            store::value(bundle_id),
            store::value(&actor.developer_id),
        ],
    )
    .await?;
    let Some(owned) = owned else {
        return error("SUBMISSION_NOT_FOUND", "Submission not found", 404);
    };
    if req.method() == Method::Get {
        return json_response(
            &json!({"submission":submission_payload(&database, submission_id).await?}),
            200,
        );
    }
    if value_str(&owned, "state") != Some("draft") {
        return error(
            "SUBMISSION_NOT_EDITABLE",
            "Only Draft Submissions can be edited",
            409,
        );
    }
    let input: SubmissionDraftInput = match bounded_json(&mut req).await? {
        Ok(input) => input,
        Err(response) => return Ok(response),
    };
    if !valid_submission_draft(&input) {
        return error(
            "SUBMISSION_INVALID",
            "Submission information is invalid",
            422,
        );
    }
    if !valid_submission_media(&input).await? {
        return error(
            "SUBMISSION_MEDIA_INVALID",
            "Icon and screenshots must be reachable PNG or JPEG images; the icon must be exactly 512 by 512 pixels",
            422,
        );
    }
    let Some(build) = store::first::<Value>(
        &database,
        "SELECT build_id,version,capabilities_json FROM app_builds WHERE build_id=?1 AND app_id=?2",
        &[
            store::value(input.build_id.trim()),
            store::value(value_str(&owned, "app_id").unwrap_or("")),
        ],
    )
    .await?
    else {
        return error("BUILD_NOT_FOUND", "Build not found", 404);
    };
    if !valid_previous_submission(
        &database,
        value_str(&owned, "app_id").unwrap_or(""),
        input.previous_submission_id.as_deref(),
    )
    .await?
    {
        return error(
            "PREVIOUS_SUBMISSION_INVALID",
            "Previous Submission must be a Changes Required or Rejected Submission for this App",
            422,
        );
    }
    let capabilities = value_str(&build, "capabilities_json")
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default();
    if input
        .capability_reasons
        .keys()
        .any(|capability| !capabilities.contains(capability))
    {
        return error(
            "CAPABILITY_MISMATCH",
            "Capabilities must come from the verified Build",
            422,
        );
    }
    let timestamp = now();
    let mut statements = vec![
        database
            .prepare(
                "UPDATE submissions SET build_id=?1,version=?2,submission_kind=?3,
           previous_submission_id=?4,updated_at=?5 WHERE submission_id=?6 AND state='draft'",
            )
            .bind(&[
                store::value(input.build_id.trim()),
                store::value(value_str(&build, "version").unwrap_or("")),
                store::value(&input.submission_kind),
                optional_value(input.previous_submission_id.as_deref()),
                store::number(timestamp),
                store::value(submission_id),
            ])?,
    ];
    statements.extend(draft_detail_statements(
        &database,
        submission_id,
        &input,
        &capabilities,
        true,
    )?);
    statements.push(store::audit_statement(
        &database,
        Some(&actor.account_id),
        "submission.update",
        "submission",
        submission_id,
        json!({"developer_id":actor.developer_id,"bundle_id":bundle_id}),
        timestamp,
    )?);
    database.batch(statements).await?;
    json_response(
        &json!({"submission":submission_payload(&database, submission_id).await?}),
        200,
    )
}

async fn submit_developer_submission(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_developer_actor(&req, &ctx.env).await? {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let bundle_id = param(&ctx, "bundle_id");
    let submission_id = param(&ctx, "submission_id");
    let database = db(&ctx)?;
    let Some(submission) = store::first::<Value>(
        &database,
        "SELECT s.state,s.version,s.submission_kind,s.app_id,b.machine_status,
                d.external_communication,d.external_communication_reason,
                d.external_communication_purpose,d.collects_data,d.data_collection_description,
                d.executes_dynamic_code,d.dynamic_code_explanation,d.uses_external_updates,
                d.external_updates_explanation,d.requires_login,d.test_account,d.test_instructions,
                (SELECT COUNT(*) FROM submission_screenshots x WHERE x.submission_id=s.submission_id) screenshot_count,
                (SELECT COUNT(*) FROM submission_screenshots x WHERE x.submission_id=s.submission_id AND x.contains_actual_app_ui=1) actual_ui_count,
                (SELECT COUNT(*) FROM submission_network_domains x WHERE x.submission_id=s.submission_id) domain_count,
                (SELECT COUNT(*) FROM published_versions p WHERE p.app_id=s.app_id) published_count,
                COALESCE(v.status,'not_available') availability_status
           FROM submissions s JOIN submission_details d USING(submission_id)
           JOIN app_builds b ON b.build_id=s.build_id JOIN apps a ON a.app_id=s.app_id
           LEFT JOIN app_availability v ON v.app_id=s.app_id
          WHERE s.submission_id=?1 AND a.bundle_id=?2 AND a.developer_id=?3",
        &[store::value(submission_id), store::value(bundle_id), store::value(&actor.developer_id)],
    ).await? else {
        return error("SUBMISSION_NOT_FOUND", "Submission not found", 404);
    };
    if value_str(&submission, "state") != Some("draft") {
        return error(
            "SUBMISSION_NOT_SUBMITTABLE",
            "Only a Draft can be submitted",
            409,
        );
    }
    if value_str(&submission, "machine_status") != Some("valid") {
        return error(
            "BUILD_NOT_VALIDATED",
            "Build machine validation must complete first",
            409,
        );
    }
    let number = |name: &str| {
        submission
            .get(name)
            .and_then(Value::as_i64)
            .or_else(|| {
                submission
                    .get(name)
                    .and_then(Value::as_f64)
                    .map(|value| value as i64)
            })
            .unwrap_or(0)
    };
    let present =
        |name: &str| value_str(&submission, name).is_some_and(|value| !value.trim().is_empty());
    let valid_required = number("screenshot_count") >= 3
        && number("actual_ui_count") >= 1
        && (number("external_communication") == 0
            || (present("external_communication_reason")
                && present("external_communication_purpose")
                && number("domain_count") >= 1))
        && (number("collects_data") == 0 || present("data_collection_description"))
        && (number("executes_dynamic_code") == 0 || present("dynamic_code_explanation"))
        && (number("uses_external_updates") == 0 || present("external_updates_explanation"))
        && (number("requires_login") == 0
            || present("test_account")
            || present("test_instructions"));
    if !valid_required {
        return error(
            "SUBMISSION_INCOMPLETE",
            "Required review information is incomplete",
            422,
        );
    }
    let kind = value_str(&submission, "submission_kind").unwrap_or("");
    let availability = value_str(&submission, "availability_status").unwrap_or("not_available");
    let published_count = number("published_count");
    let kind_matches_lifecycle = match kind {
        "new_app" => published_count == 0,
        "update" => published_count > 0 && availability == "available",
        "re_review" => {
            published_count > 0 && matches!(availability, "developer_unpublished" | "removed")
        }
        _ => false,
    };
    if !kind_matches_lifecycle {
        return error(
            "SUBMISSION_KIND_INVALID",
            "Submission kind does not match the App publication lifecycle",
            409,
        );
    }
    if store::first::<Value>(
        &database,
        "SELECT version FROM published_versions WHERE app_id=?1 AND version=?2",
        &[
            store::value(value_str(&submission, "app_id").unwrap_or("")),
            store::value(value_str(&submission, "version").unwrap_or("")),
        ],
    )
    .await?
    .is_some()
    {
        return error(
            "VERSION_ALREADY_PUBLISHED",
            "This version has already been published",
            409,
        );
    }
    let timestamp = now();
    database
        .batch(vec![
            database
                .prepare(
                    "UPDATE submissions SET state='submitted',submitted_at=?1,updated_at=?1
              WHERE submission_id=?2 AND state='draft'",
                )
                .bind(&[store::number(timestamp), store::value(submission_id)])?,
            store::audit_statement(
                &database,
                Some(&actor.account_id),
                "submission.submit",
                "submission",
                submission_id,
                json!({"developer_id":actor.developer_id,"bundle_id":bundle_id}),
                timestamp,
            )?,
        ])
        .await?;
    json_response(
        &json!({"submission":submission_payload(&database, submission_id).await?}),
        200,
    )
}

async fn answer_submission(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_developer_actor(&req, &ctx.env).await? {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let bundle_id = param(&ctx, "bundle_id");
    let submission_id = param(&ctx, "submission_id");
    let input: SubmissionMessageInput = match bounded_json(&mut req).await? {
        Ok(input) => input,
        Err(response) => return Ok(response),
    };
    if input.body.trim().is_empty() || input.body.trim().chars().count() > 8000 {
        return error("MESSAGE_INVALID", "Additional information is invalid", 422);
    }
    let database = db(&ctx)?;
    let owned: Option<Value> = store::first(
        &database,
        "SELECT s.state FROM submissions s JOIN apps a ON a.app_id=s.app_id
          WHERE s.submission_id=?1 AND a.bundle_id=?2 AND a.developer_id=?3",
        &[
            store::value(submission_id),
            store::value(bundle_id),
            store::value(&actor.developer_id),
        ],
    )
    .await?;
    if owned.as_ref().and_then(|value| value_str(value, "state"))
        != Some("more_information_required")
    {
        return error(
            "INFORMATION_NOT_REQUESTED",
            "This Submission is not waiting for information",
            409,
        );
    }
    let timestamp = now();
    let message_id = id("msg");
    database.batch(vec![
        database.prepare(
            "INSERT INTO submission_messages(message_id,submission_id,author_account_id,author_role,body,created_at)
             VALUES(?1,?2,?3,'developer',?4,?5)",
        ).bind(&[
            store::value(&message_id), store::value(submission_id), store::value(&actor.account_id),
            store::value(input.body.trim()), store::number(timestamp),
        ])?,
        database.prepare(
            "UPDATE submissions SET state='in_review',updated_at=?1 WHERE submission_id=?2
              AND state='more_information_required'",
        ).bind(&[store::number(timestamp), store::value(submission_id)])?,
        store::audit_statement(
            &database, Some(&actor.account_id), "submission.information_provided", "submission",
            submission_id, json!({"developer_id":actor.developer_id,"bundle_id":bundle_id}), timestamp,
        )?,
    ]).await?;
    json_response(&json!({"message_id":message_id,"state":"in_review"}), 201)
}

async fn developer_appeals(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_developer_actor(&req, &ctx.env).await? {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let bundle_id = param(&ctx, "bundle_id");
    let database = db(&ctx)?;
    let Some(app) = store::first::<Value>(
        &database,
        "SELECT app_id FROM apps WHERE bundle_id=?1 AND developer_id=?2",
        &[store::value(bundle_id), store::value(&actor.developer_id)],
    )
    .await?
    else {
        return error("APP_NOT_FOUND", "App not found", 404);
    };
    let app_id = value_str(&app, "app_id").unwrap_or("");
    if req.method() == Method::Get {
        let appeals: Vec<Value> = store::rows(
            &database,
            "SELECT * FROM appeals WHERE app_id=?1 ORDER BY created_at DESC",
            &[store::value(app_id)],
        )
        .await?;
        return json_response(&json!({"appeals":appeals}), 200);
    }
    let input: AppealInput = match bounded_json(&mut req).await? {
        Ok(input) => input,
        Err(response) => return Ok(response),
    };
    if input.reason.trim().is_empty()
        || input.reason.trim().chars().count() > 8000
        || !matches!(
            input.appealed_action.as_str(),
            "review_decision" | "removed"
        )
    {
        return error("APPEAL_INVALID", "Appeal is invalid", 422);
    }
    if input.appealed_action == "review_decision" {
        let Some(submission_id) = input.submission_id.as_deref() else {
            return error("SUBMISSION_REQUIRED", "Submission is required", 422);
        };
        if store::first::<Value>(
            &database,
            "SELECT submission_id FROM submissions WHERE submission_id=?1 AND app_id=?2
              AND state IN ('changes_required','rejected')",
            &[store::value(submission_id), store::value(app_id)],
        )
        .await?
        .is_none()
        {
            return error(
                "SUBMISSION_NOT_APPEALABLE",
                "Submission cannot be appealed",
                409,
            );
        }
    } else if store::first::<Value>(
        &database,
        "SELECT app_id FROM app_availability WHERE app_id=?1 AND status='removed'",
        &[store::value(app_id)],
    )
    .await?
    .is_none()
    {
        return error(
            "APP_NOT_REMOVED",
            "Only a removed App can appeal removal",
            409,
        );
    }
    let appeal_id = id("apl");
    let timestamp = now();
    database.batch(vec![
        database.prepare(
            "INSERT INTO appeals(appeal_id,app_id,submission_id,appealed_action,reason,state,
               submitted_by_account_id,created_at) VALUES(?1,?2,?3,?4,?5,'submitted',?6,?7)",
        ).bind(&[
            store::value(&appeal_id), store::value(app_id), optional_value(input.submission_id.as_deref()),
            store::value(&input.appealed_action), store::value(input.reason.trim()),
            store::value(&actor.account_id), store::number(timestamp),
        ])?,
        store::audit_statement(
            &database, Some(&actor.account_id), "appeal.submit", "appeal", &appeal_id,
            json!({"developer_id":actor.developer_id,"bundle_id":bundle_id,"action":input.appealed_action}), timestamp,
        )?,
    ]).await?;
    json_response(&json!({"appeal_id":appeal_id,"state":"submitted"}), 201)
}

async fn developer_unpublish(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_developer_actor(&req, &ctx.env).await? {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let input: UnpublishInput = match bounded_json(&mut req).await? {
        Ok(input) => input,
        Err(response) => return Ok(response),
    };
    if input.reason.trim().is_empty() || input.reason.trim().chars().count() > 2000 {
        return error("REASON_REQUIRED", "Unpublish reason is required", 422);
    }
    let bundle_id = param(&ctx, "bundle_id");
    let database = db(&ctx)?;
    let Some(app) = store::first::<Value>(
        &database,
        "SELECT a.app_id,v.status FROM apps a JOIN app_availability v ON v.app_id=a.app_id
          WHERE a.bundle_id=?1 AND a.developer_id=?2",
        &[store::value(bundle_id), store::value(&actor.developer_id)],
    )
    .await?
    else {
        return error("APP_NOT_FOUND", "App not found", 404);
    };
    if value_str(&app, "status") != Some("available") {
        return error(
            "APP_NOT_AVAILABLE",
            "Only an available App can be unpublished",
            409,
        );
    }
    let app_id = value_str(&app, "app_id").unwrap_or("");
    let timestamp = now();
    let event_id = id("availability");
    database.batch(vec![
        database.prepare(
            "UPDATE app_availability SET status='developer_unpublished',reason=?1,
               changed_by_account_id=?2,changed_at=?3 WHERE app_id=?4 AND status='available'",
        ).bind(&[
            store::value(input.reason.trim()), store::value(&actor.account_id),
            store::number(timestamp), store::value(app_id),
        ])?,
        database.prepare("UPDATE apps SET visibility='private',updated_at=?1 WHERE app_id=?2")
            .bind(&[store::number(timestamp), store::value(app_id)])?,
        database.prepare(
            "INSERT INTO availability_history(event_id,app_id,from_status,to_status,reason,
               actor_account_id,created_at) VALUES(?1,?2,'available','developer_unpublished',?3,?4,?5)",
        ).bind(&[
            store::value(&event_id), store::value(app_id), store::value(input.reason.trim()),
            store::value(&actor.account_id), store::number(timestamp),
        ])?,
        store::audit_statement(
            &database, Some(&actor.account_id), "app.developer_unpublish", "app", bundle_id,
            json!({"developer_id":actor.developer_id,"reason":input.reason}), timestamp,
        )?,
    ]).await?;
    json_response(
        &json!({"status":"developer_unpublished","requires_re_review":true}),
        200,
    )
}

async fn replace_app_certificate(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_developer_actor(&req, &ctx.env).await? {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let input: CertificateReplacementInput = match bounded_json(&mut req).await? {
        Ok(input) => input,
        Err(response) => return Ok(response),
    };
    if input.confirmation != "REPLACE" || input.certificate_id.trim().is_empty() {
        return error(
            "CERTIFICATE_REPLACEMENT_CONFIRMATION_REQUIRED",
            "New certificate and REPLACE confirmation are required",
            422,
        );
    }
    let bundle_id = param(&ctx, "bundle_id");
    let database = db(&ctx)?;
    let Some(current) = store::first::<Value>(
        &database,
        "SELECT a.app_id,c.certificate_id FROM apps a JOIN app_certificates c ON c.app_id=a.app_id
          WHERE a.bundle_id=?1 AND a.developer_id=?2 AND c.is_current=1",
        &[store::value(bundle_id), store::value(&actor.developer_id)],
    )
    .await?
    else {
        return error(
            "APP_CERTIFICATE_NOT_FOUND",
            "App certificate not found",
            404,
        );
    };
    let current_id = value_str(&current, "certificate_id").unwrap_or("");
    if current_id == input.certificate_id.trim() {
        return error(
            "CERTIFICATE_ALREADY_ASSIGNED",
            "The certificate is already assigned to this App",
            409,
        );
    }
    let Some(current_status) = auth::certificate_status(&ctx.env, current_id).await? else {
        return error(
            "CURRENT_CERTIFICATE_STATUS_UNAVAILABLE",
            "Current certificate status is unavailable",
            503,
        );
    };
    if current_status.developer_record_id != actor.developer_id
        || current_status.status != "revoked"
    {
        return error(
            "CURRENT_CERTIFICATE_NOT_REVOKED",
            "The current certificate must be revoked before replacement",
            409,
        );
    }
    if auth::certificate_identity(&ctx.env, input.certificate_id.trim(), &actor.developer_id)
        .await?
        .is_none()
    {
        return error(
            "REPLACEMENT_CERTIFICATE_INVALID",
            "The replacement certificate must be active and belong to this Developer",
            403,
        );
    }
    let app_id = value_str(&current, "app_id").unwrap_or("");
    let timestamp = now();
    let result = database
        .batch(vec![
            database
                .prepare(
                    "UPDATE app_certificates SET is_current=0,observed_status='revoked',
                       last_verified_at=?1 WHERE app_id=?2 AND certificate_id=?3 AND is_current=1",
                )
                .bind(&[
                    store::number(timestamp),
                    store::value(app_id),
                    store::value(current_id),
                ])?,
            database
                .prepare(
                    "INSERT INTO app_certificates(app_id,certificate_id,assigned_by_account_id,
                       assigned_at,last_verified_at,observed_status,is_current)
                     VALUES(?1,?2,?3,?4,?4,'active',1)",
                )
                .bind(&[
                    store::value(app_id),
                    store::value(input.certificate_id.trim()),
                    store::value(&actor.account_id),
                    store::number(timestamp),
                ])?,
            store::audit_statement(
                &database,
                Some(&actor.account_id),
                "app.certificate_replaced",
                "app",
                bundle_id,
                json!({"developer_id":actor.developer_id,"previous_certificate_id":current_id,"certificate_id":input.certificate_id}),
                timestamp,
            )?,
        ])
        .await;
    if result.is_err() {
        return error(
            "CERTIFICATE_REPLACEMENT_CONFLICT",
            "The replacement certificate is already assigned or the current assignment changed",
            409,
        );
    }
    json_response(
        &json!({"bundle_id":bundle_id,"certificate_id":input.certificate_id,"status":"active"}),
        200,
    )
}

async fn create_release(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) =
        rate_limited(&req, &ctx.env, "MUTATION_RATE_LIMITER", "release-register").await?
    {
        return Ok(response);
    }
    let actor = match require_developer_actor(&req, &ctx.env).await? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let developer = actor.developer_id.clone();
    let bundle_id = param(&ctx, "bundle_id").to_string();
    let database = db(&ctx)?;
    let active_app: Option<Value> = store::first(
        &database,
        "SELECT a.app_id FROM apps a JOIN bundle_ids b ON b.bundle_id=a.bundle_id
         WHERE a.developer_id=?1 AND a.bundle_id=?2 AND b.status='active' LIMIT 1",
        &[store::value(&developer), store::value(&bundle_id)],
    )
    .await?;
    let Some(active_app) = active_app else {
        return error("APP_NOT_FOUND", "App not found", 404);
    };
    let app_id = value_str(&active_app, "app_id").unwrap_or("").to_owned();
    let input: ReleaseInput = req.json().await?;
    let Some((owner, repository)) = github_repository(input.repository.trim()) else {
        return error("VALIDATION_ERROR", "GitHub repository is invalid", 422);
    };
    if !valid_version(input.version.trim())
        || input.release_tag.trim().is_empty()
        || input.release_tag.eq_ignore_ascii_case("latest")
        || !input.asset.trim().ends_with(".mpkg")
        || input.certificate_id.trim().is_empty()
    {
        return error("VALIDATION_ERROR", "Release metadata is invalid", 422);
    }
    let Some(certificate) =
        auth::certificate_identity(&ctx.env, input.certificate_id.trim(), &developer).await?
    else {
        return error(
            "CERTIFICATE_INVALID",
            "An active certificate for this developer is required",
            403,
        );
    };
    let assigned_certificate: Option<Value> = store::first(
        &database,
        "SELECT certificate_id,observed_status FROM app_certificates WHERE app_id=?1 AND is_current=1",
        &[store::value(&app_id)],
    )
    .await?;
    if let Some(assigned) = assigned_certificate.as_ref()
        && (value_str(assigned, "certificate_id") != Some(input.certificate_id.trim())
            || value_str(assigned, "observed_status") != Some("active"))
    {
        return error(
            "APP_CERTIFICATE_MISMATCH",
            "Each App must keep its assigned active Developer Certificate",
            409,
        );
    }
    let lookup = GitHubReleaseAssetRequest {
        owner,
        repository,
        release_tag: input.release_tag.trim(),
        asset_name: input.asset.trim(),
    };
    let verified_asset =
        match auth::github_release_asset_for_account(&ctx.env, &actor.account_id, &lookup).await? {
            Ok(asset) => asset,
            Err(cause) => return error(&cause.code, &cause.message, cause.status),
        };
    if verified_asset.account_id != actor.account_id {
        return error(
            "ACTOR_IDENTITY_MISMATCH",
            "Accounts and DeveloperCA authenticated different actors",
            403,
        );
    }
    let asset = verified_asset.release_asset;
    if !asset
        .repository
        .eq_ignore_ascii_case(input.repository.trim())
        || asset.release_tag != input.release_tag.trim()
        || asset.asset_name != input.asset.trim()
        || !matches!(
            asset.repository_permission.as_str(),
            "push" | "maintain" | "admin"
        )
        || asset.file_size == 0
        || asset.file_size > MAX_PACKAGE_BYTES
        || github_download_url(&asset.download_url).is_none()
    {
        return error(
            "GITHUB_ASSET_INVALID",
            "GitHub release asset metadata is invalid",
            422,
        );
    }
    let (Some(repository_id), Some(github_release_id), Some(github_asset_id)) = (
        js_integer(asset.repository_id),
        js_integer(asset.release_id),
        js_integer(asset.asset_id),
    ) else {
        return error(
            "GITHUB_ID_UNSUPPORTED",
            "GitHub returned an identifier that cannot be represented safely",
            422,
        );
    };
    let release_id = id("rel");
    let timestamp = now();
    let build_sequence: Option<Value> = store::first(
        &database,
        "SELECT COALESCE(MAX(build_number),0)+1 AS value FROM app_builds
          WHERE app_id=?1 AND version=?2",
        &[store::value(&app_id), store::value(input.version.trim())],
    )
    .await?;
    let build_number = build_sequence
        .as_ref()
        .and_then(|value| value.get("value"))
        .and_then(Value::as_f64)
        .unwrap_or(1.0) as i64;
    let release_statement = database
        .prepare(
            "INSERT INTO releases(
           release_id,bundle_id,version,github_repository_id,github_repository,
           github_release_id,github_release_tag,github_release_immutable,github_prerelease,
           github_asset_id,asset_name,download_url,file_size,github_digest,
           github_asset_created_at,github_asset_updated_at,developer_certificate_id,
           developer_public_key,developer_certificate_serial,developer_certificate_subject_key_id,
           developer_certificate_developer_id,developer_certificate_issuer_key_id,
           developer_certificate_issuer_public_key,developer_certificate_issuance_source,
           changelog,
           registered_by,registered_by_account_id,developer_display_name,created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29)",
        )
        .bind(&[
            store::value(&release_id),
            store::value(&bundle_id),
            store::value(input.version.trim()),
            store::value(repository_id),
            store::value(&asset.repository),
            store::value(github_release_id),
            store::value(&asset.release_tag),
            store::value(if asset.immutable { 1.0 } else { 0.0 }),
            store::value(if asset.prerelease { 1.0 } else { 0.0 }),
            store::value(github_asset_id),
            store::value(&asset.asset_name),
            store::value(&asset.download_url),
            store::value(asset.file_size as f64),
            asset
                .github_digest
                .as_deref()
                .map_or(worker::wasm_bindgen::JsValue::NULL, store::value),
            store::value(&asset.asset_created_at),
            store::value(&asset.asset_updated_at),
            store::value(input.certificate_id.trim()),
            store::value(&certificate.public_key),
            store::value(&certificate.serial_number),
            store::value(&certificate.subject_key_id),
            store::value(&certificate.developer_id),
            store::value(&certificate.issuer_key_id),
            store::value(&certificate.issuer_public_key),
            store::value(&certificate.issuance_source),
            input
                .changelog
                .as_deref()
                .map_or(worker::wasm_bindgen::JsValue::NULL, store::value),
            store::value(&developer),
            store::value(&actor.account_id),
            store::value(&actor.display_name),
            store::number(timestamp),
        ])?;
    let build_statement = database
        .prepare(
            "INSERT INTO app_builds(
               build_id,app_id,certificate_id,version,build_number,github_repository_id,
               github_repository,github_release_id,github_release_tag,github_asset_id,
               asset_name,download_url,file_size,machine_status,registered_by_account_id,created_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,'pending',?14,?15)",
        )
        .bind(&[
            store::value(&release_id),
            store::value(&app_id),
            store::value(input.certificate_id.trim()),
            store::value(input.version.trim()),
            store::number(build_number),
            store::value(repository_id),
            store::value(&asset.repository),
            store::value(github_release_id),
            store::value(&asset.release_tag),
            store::value(github_asset_id),
            store::value(&asset.asset_name),
            store::value(&asset.download_url),
            store::value(asset.file_size as f64),
            store::value(&actor.account_id),
            store::number(timestamp),
        ])?;
    let mut statements = Vec::new();
    if assigned_certificate.is_none() {
        statements.push(
            database
                .prepare(
                    "INSERT INTO app_certificates(app_id,certificate_id,assigned_by_account_id,
                       assigned_at,last_verified_at,observed_status)
                     VALUES(?1,?2,?3,?4,?4,'active')",
                )
                .bind(&[
                    store::value(&app_id),
                    store::value(input.certificate_id.trim()),
                    store::value(&actor.account_id),
                    store::number(timestamp),
                ])?,
        );
    }
    statements.push(release_statement);
    statements.push(build_statement);
    statements.push(store::audit_statement(
        &database,
        Some(&actor.account_id),
        "release.create",
        "release",
        &release_id,
        json!({"developer_id":developer,"developer_role":actor.role,"package_id":bundle_id,"version":input.version,"asset_id":asset.asset_id,"result":"registered"}),
        timestamp,
    )?);
    if database.batch(statements).await.is_err() {
        return error(
            "RELEASE_ALREADY_EXISTS",
            "Release version, Build number, or GitHub asset already exists",
            409,
        );
    }
    json_response(
        &json!({"release_id":release_id,"build_id":release_id,"build_number":build_number,"validation_status":"pending","review_status":"pending","publish_status":"draft"}),
        201,
    )
}

async fn list_developer_releases(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let developer = match require_developer(&req, &ctx.env).await? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let bundle_id = param(&ctx, "bundle_id");
    if store::developer_app(&db(&ctx)?, &developer, bundle_id)
        .await?
        .is_none()
    {
        return error("APP_NOT_FOUND", "App not found", 404);
    }
    let rows: Vec<Value> = store::rows(
        &db(&ctx)?,
        "SELECT * FROM releases WHERE bundle_id=?1 ORDER BY created_at DESC",
        &[store::value(bundle_id)],
    )
    .await?;
    json_response(&json!({"bundle_id":bundle_id,"releases":rows}), 200)
}

async fn developer_notifications(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_developer_actor(&req, &ctx.env).await? {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let (limit, offset) = page(&req);
    let database = db(&ctx)?;
    let notifications = store::developer_notifications(
        &database,
        &actor.developer_id,
        &actor.account_id,
        limit,
        offset,
    )
    .await?;
    let unread_count =
        store::developer_unread_count(&database, &actor.developer_id, &actor.account_id).await?;
    json_response(
        &json!({
            "developer_id":actor.developer_id,
            "notifications":notifications,
            "unread_count":unread_count
        }),
        200,
    )
}

async fn read_developer_notification(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) =
        rate_limited(&req, &ctx.env, "MUTATION_RATE_LIMITER", "notification-read").await?
    {
        return Ok(response);
    }
    let actor = match require_developer_actor(&req, &ctx.env).await? {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let notification_id = param(&ctx, "notification_id");
    if !valid_notification_id(notification_id) {
        return error("NOTIFICATION_ID_INVALID", "Notification ID is invalid", 422);
    }
    if !store::mark_developer_notification_read(
        &db(&ctx)?,
        notification_id,
        &actor.developer_id,
        &actor.account_id,
        now(),
    )
    .await?
    {
        return error("NOTIFICATION_NOT_FOUND", "Notification not found", 404);
    }
    Ok(Response::empty()?.with_status(204))
}

async fn read_all_developer_notifications(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) = rate_limited(
        &req,
        &ctx.env,
        "MUTATION_RATE_LIMITER",
        "notifications-read-all",
    )
    .await?
    {
        return Ok(response);
    }
    let actor = match require_developer_actor(&req, &ctx.env).await? {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    store::mark_all_developer_notifications_read(
        &db(&ctx)?,
        &actor.developer_id,
        &actor.account_id,
        now(),
    )
    .await?;
    Ok(Response::empty()?.with_status(204))
}

async fn developer_app_history(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_developer_actor(&req, &ctx.env).await? {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let bundle_id = param(&ctx, "bundle_id");
    let database = db(&ctx)?;
    if store::developer_app(&database, &actor.developer_id, bundle_id)
        .await?
        .is_none()
    {
        return error("APP_NOT_FOUND", "App not found", 404);
    }
    json_response(
        &json!({"bundle_id":bundle_id,"history":store::app_history(&database,bundle_id).await?}),
        200,
    )
}

async fn admin_notifications(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let (limit, offset) = page(&req);
    let database = db(&ctx)?;
    json_response(
        &json!({
            "notifications":store::operator_notifications(&database,&actor,limit,offset).await?,
            "unread_count":store::operator_unread_count(&database,&actor).await?
        }),
        200,
    )
}

async fn read_admin_notification(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) = rate_limited(
        &req,
        &ctx.env,
        "MUTATION_RATE_LIMITER",
        "admin-notification-read",
    )
    .await?
    {
        return Ok(response);
    }
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let notification_id = param(&ctx, "notification_id");
    if !valid_notification_id(notification_id) {
        return error("NOTIFICATION_ID_INVALID", "Notification ID is invalid", 422);
    }
    if !store::mark_operator_notification_read(&db(&ctx)?, notification_id, &actor, now()).await? {
        return error("NOTIFICATION_NOT_FOUND", "Notification not found", 404);
    }
    Ok(Response::empty()?.with_status(204))
}

async fn read_all_admin_notifications(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) = rate_limited(
        &req,
        &ctx.env,
        "MUTATION_RATE_LIMITER",
        "admin-notifications-read-all",
    )
    .await?
    {
        return Ok(response);
    }
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    store::mark_all_operator_notifications_read(&db(&ctx)?, &actor, now()).await?;
    Ok(Response::empty()?.with_status(204))
}

async fn admin_release_history(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let release_id = param(&ctx, "release_id");
    let database = db(&ctx)?;
    if store::release_by_id(&database, release_id).await?.is_none() {
        return error("RELEASE_NOT_FOUND", "Release not found", 404);
    }
    json_response(
        &json!({"admin":actor,"release_id":release_id,"history":store::release_history(&database,release_id).await?}),
        200,
    )
}

async fn admin_release(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let release_id = param(&ctx, "release_id");
    if auth::reviewer(&req, &ctx.env)? {
        if let Some(response) =
            rate_limited(&req, &ctx.env, "REVIEWER_RATE_LIMITER", "start").await?
        {
            return Ok(response);
        }
        let timestamp = now();
        let attempt_id = id("attempt");
        let claimed: Option<Value> = store::first(
            &db(&ctx)?,
            "UPDATE releases SET validation_attempt_id=?1,validation_started_at=?2
              WHERE release_id=?3 AND validation_status='pending' AND review_status='pending'
                AND publish_status='draft'
                AND (validation_started_at IS NULL OR validation_started_at<?4)
              RETURNING *",
            &[
                store::value(&attempt_id),
                store::number(timestamp),
                store::value(release_id),
                store::number(timestamp - 600),
            ],
        )
        .await?;
        let Some(release) = claimed else {
            return if store::release_by_id(&db(&ctx)?, release_id)
                .await?
                .is_some()
            {
                error(
                    "VALIDATION_ALREADY_RUNNING",
                    "Release is not pending or already has an active Reviewer lease",
                    409,
                )
            } else {
                error("RELEASE_NOT_FOUND", "Release not found", 404)
            };
        };
        store::audit(
            &db(&ctx)?,
            None,
            "release.validation_started",
            "release",
            release_id,
            json!({"developer_id":value_str(&release,"registered_by"),"asset_id":release.get("github_asset_id"),"package_id":value_str(&release,"bundle_id"),"validation_attempt_id":attempt_id,"result":"started"}),
            timestamp,
        )
        .await?;
        return json_response(&json!({"admin":"mpkg-reviewer","release":release}), 200);
    }
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    match store::release_by_id(&db(&ctx)?, release_id).await? {
        Some(release) => json_response(&json!({"admin":actor,"release":release}), 200),
        None => error("RELEASE_NOT_FOUND", "Release not found", 404),
    }
}

async fn claim_next_release(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    match require_reviewer(&req, &ctx.env)? {
        Ok(()) => {}
        Err(response) => return Ok(response),
    }
    if let Some(response) = rate_limited(&req, &ctx.env, "REVIEWER_RATE_LIMITER", "claim").await? {
        return Ok(response);
    }

    let timestamp = now();
    let attempt_id = id("attempt");
    let claimed: Option<Value> = store::first(
        &db(&ctx)?,
        "UPDATE releases SET validation_attempt_id=?1,validation_started_at=?2
          WHERE release_id=(
            SELECT release_id FROM releases
             WHERE validation_status='pending' AND review_status='pending'
               AND publish_status='draft'
               AND (validation_started_at IS NULL OR validation_started_at<?3)
             ORDER BY created_at ASC,release_id ASC LIMIT 1
          )
            AND validation_status='pending' AND review_status='pending'
            AND publish_status='draft'
            AND (validation_started_at IS NULL OR validation_started_at<?3)
          RETURNING *",
        &[
            store::value(&attempt_id),
            store::number(timestamp),
            store::number(timestamp - 600),
        ],
    )
    .await?;
    let Some(release) = claimed else {
        return Ok(Response::empty()?.with_status(204));
    };
    let release_id = value_str(&release, "release_id").unwrap_or_default();
    store::audit(
        &db(&ctx)?,
        None,
        "release.validation_started",
        "release",
        release_id,
        json!({
            "developer_id": value_str(&release, "registered_by"),
            "asset_id": release.get("github_asset_id"),
            "package_id": value_str(&release, "bundle_id"),
            "validation_attempt_id": attempt_id,
            "result": "started",
            "source": "automatic_queue"
        }),
        timestamp,
    )
    .await?;
    json_response(&json!({"admin":"mpkg-reviewer","release":release}), 200)
}

async fn validate_release(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) =
        rate_limited(&req, &ctx.env, "REVIEWER_RATE_LIMITER", "validate").await?
    {
        return Ok(response);
    }
    match require_reviewer(&req, &ctx.env)? {
        Ok(()) => {}
        Err(response) => return Ok(response),
    }
    let release_id = param(&ctx, "release_id");
    let Some(release) = store::release_by_id(&db(&ctx)?, release_id).await? else {
        return error("RELEASE_NOT_FOUND", "Release not found", 404);
    };
    if value_str(&release, "validation_status") != Some("pending")
        || value_str(&release, "review_status") != Some("pending")
        || value_str(&release, "publish_status") != Some("draft")
    {
        return error(
            "INVALID_RELEASE_STATUS",
            "Only pending draft releases can be validated",
            409,
        );
    }
    let input: ValidationInput = req.json().await?;
    let asset_sha256 = input.asset_sha256.to_ascii_lowercase();
    let package_digest = input.package_digest.to_ascii_lowercase();
    let manifest_digest = input.manifest_digest.to_ascii_lowercase();
    let expected_size = release
        .get("file_size")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let expected_asset_id = release
        .get("github_asset_id")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let timestamp = now();
    if input.release_id != release_id
        || input.asset_id != expected_asset_id
        || input.validation_attempt_id != value_str(&release, "validation_attempt_id").unwrap_or("")
        || input.reviewer_version.trim().is_empty()
        || input.reviewer_version.len() > 64
        || input.validated_at.abs_diff(timestamp as u64) > 600
        || input.package_id != value_str(&release, "bundle_id").unwrap_or("")
        || input.version != value_str(&release, "version").unwrap_or("")
        || input.file_size != expected_size
        || input.certificate_id != value_str(&release, "developer_certificate_id").unwrap_or("")
        || input.certificate_serial
            != value_str(&release, "developer_certificate_serial").unwrap_or("")
        || input.certificate_subject_key_id
            != value_str(&release, "developer_certificate_subject_key_id").unwrap_or("")
        || input.certificate_developer_id
            != value_str(&release, "developer_certificate_developer_id").unwrap_or("")
        || input.certificate_issuer_key_id
            != value_str(&release, "developer_certificate_issuer_key_id").unwrap_or("")
        || asset_sha256.len() != 64
        || hex::decode(&asset_sha256).is_err()
        || package_digest.len() != 64
        || hex::decode(&package_digest).is_err()
        || package_digest != asset_sha256
        || manifest_digest.len() != 64
        || hex::decode(&manifest_digest).is_err()
        || input.signature.trim().is_empty()
        || input.capabilities.len() > 256
        || input.payloads.is_empty()
        || input.payloads.len() > 10_000
    {
        return error(
            "PACKAGE_VALIDATION_MISMATCH",
            "Validated .mpkg metadata does not match the registered release",
            422,
        );
    }
    if let Some(github_digest) = value_str(&release, "github_digest")
        && let Some(github_sha256) = github_digest.strip_prefix("sha256:")
        && !github_sha256.eq_ignore_ascii_case(&asset_sha256)
    {
        return error(
            "GITHUB_DIGEST_MISMATCH",
            "GitHub asset digest does not match the reviewed .mpkg",
            422,
        );
    }
    let public_key = value_str(&release, "developer_public_key").unwrap_or("");
    if !valid_hash_signature(public_key, input.signature.trim(), &manifest_digest) {
        return error(
            "SIGNATURE_INVALID",
            "The embedded Developer Certificate signature is invalid",
            422,
        );
    }
    let Some(identity) = auth::certificate_identity(
        &ctx.env,
        input.certificate_id.trim(),
        value_str(&release, "registered_by").unwrap_or(""),
    )
    .await?
    else {
        return error(
            "CERTIFICATE_INVALID",
            "The Developer Certificate is no longer valid",
            403,
        );
    };
    if !certificate_matches_release(&identity, &release) {
        return error(
            "CERTIFICATE_IDENTITY_MISMATCH",
            "Developer Certificate identity differs from the registered release",
            422,
        );
    }
    let capabilities_json = serde_json::to_string(&input.capabilities)?;
    let payloads_json = serde_json::to_string(&input.payloads)?;
    if capabilities_json.len() > 32 * 1024 || payloads_json.len() > 1024 * 1024 {
        return error(
            "VALIDATION_REPORT_TOO_LARGE",
            "Validation report is too large",
            413,
        );
    }
    let updated: Option<Value> = store::first(
        &db(&ctx)?,
        "UPDATE releases
            SET sha256=?1,package_digest=?2,manifest_hash=?3,signature=?4,
                capabilities_json=?5,payloads_json=?6,reviewer_version=?7,
                validation_status='valid',review_status='submitted',validation_message=NULL,
                validation_error_code=NULL,validated_at=?8,validated_by='mpkg-reviewer',submitted_at=?8
          WHERE release_id=?9 AND github_asset_id=?10 AND validation_attempt_id=?11
            AND validation_status='pending' AND review_status='pending'
          RETURNING release_id",
        &[
            store::value(&asset_sha256),
            store::value(&package_digest),
            store::value(&manifest_digest),
            store::value(input.signature.trim()),
            store::value(capabilities_json),
            store::value(payloads_json),
            store::value(input.reviewer_version.trim()),
            store::number(input.validated_at as i64),
            store::value(release_id),
            store::value(expected_asset_id as f64),
            store::value(&input.validation_attempt_id),
        ],
    )
    .await?;
    if updated.is_none() {
        return error(
            "VALIDATION_LEASE_STALE",
            "Reviewer validation lease is stale",
            409,
        );
    }
    store::audit(
        &db(&ctx)?,
        None,
        "release.validation_succeeded",
        "release",
        release_id,
        json!({"developer_id":input.certificate_developer_id,"asset_id":expected_asset_id,"asset_sha256":asset_sha256,"package_digest":package_digest,"reviewer_version":input.reviewer_version,"validation_attempt_id":input.validation_attempt_id,"file_size":input.file_size,"result":"valid"}),
        timestamp,
    )
    .await?;
    json_response(
        &json!({"release":store::release_by_id(&db(&ctx)?,release_id).await?}),
        200,
    )
}

async fn invalidate_release(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) =
        rate_limited(&req, &ctx.env, "REVIEWER_RATE_LIMITER", "invalidate").await?
    {
        return Ok(response);
    }
    match require_reviewer(&req, &ctx.env)? {
        Ok(()) => {}
        Err(response) => return Ok(response),
    }
    let release_id = param(&ctx, "release_id");
    let Some(release) = store::release_by_id(&db(&ctx)?, release_id).await? else {
        return error("RELEASE_NOT_FOUND", "Release not found", 404);
    };
    if value_str(&release, "validation_status") != Some("pending")
        || value_str(&release, "review_status") != Some("pending")
        || value_str(&release, "publish_status") != Some("draft")
    {
        return error(
            "INVALID_RELEASE_STATUS",
            "Only pending draft releases can receive an initial validation failure",
            409,
        );
    }
    let input: model::ValidationFailureInput = req.json().await?;
    let expected_asset_id = release
        .get("github_asset_id")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let timestamp = now();
    if input.release_id != release_id
        || input.asset_id != expected_asset_id
        || input.validation_attempt_id != value_str(&release, "validation_attempt_id").unwrap_or("")
        || input.reviewer_version.trim().is_empty()
        || input.reviewer_version.len() > 64
        || input.validated_at.abs_diff(timestamp as u64) > 600
        || !matches!(
            input.error_code.as_str(),
            "download_failed"
                | "asset_mismatch"
                | "package_invalid"
                | "certificate_invalid"
                | "reviewer_internal_error"
        )
        || input.error_summary.trim().is_empty()
        || input.error_summary.chars().count() > 500
    {
        return error(
            "VALIDATION_FAILURE_INVALID",
            "Validation failure report is invalid",
            422,
        );
    }
    let updated: Option<Value> = store::first(
        &db(&ctx)?,
        "UPDATE releases
            SET validation_status='invalid',validation_error_code=?1,validation_message=?2,
                reviewer_version=?3,validated_at=?4,validated_by='mpkg-reviewer'
          WHERE release_id=?5 AND github_asset_id=?6 AND validation_attempt_id=?7
            AND validation_status='pending'
          RETURNING release_id",
        &[
            store::value(&input.error_code),
            store::value(input.error_summary.trim()),
            store::value(input.reviewer_version.trim()),
            store::number(input.validated_at as i64),
            store::value(release_id),
            store::value(expected_asset_id as f64),
            store::value(&input.validation_attempt_id),
        ],
    )
    .await?;
    if updated.is_none() {
        return error(
            "VALIDATION_LEASE_STALE",
            "Reviewer validation lease is stale",
            409,
        );
    }
    store::audit(
        &db(&ctx)?,
        None,
        "release.validation_failed",
        "release",
        release_id,
        json!({"developer_id":value_str(&release,"registered_by"),"asset_id":expected_asset_id,"package_id":value_str(&release,"bundle_id"),"validation_attempt_id":input.validation_attempt_id,"result":"invalid","reason_code":input.error_code,"summary":input.error_summary}),
        timestamp,
    )
    .await?;
    json_response(
        &json!({"release":store::release_by_id(&db(&ctx)?,release_id).await?}),
        200,
    )
}

async fn admin_submissions(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let queue = req
        .url()?
        .query_pairs()
        .find(|(key, _)| key == "queue")
        .map(|(_, value)| value.into_owned())
        .unwrap_or_else(|| "review".into());
    let condition = match queue.as_str() {
        "review" => "s.state IN ('submitted','in_review')",
        "new" => "s.submission_kind='new_app' AND s.state IN ('submitted','in_review')",
        "updates" => "s.submission_kind='update' AND s.state IN ('submitted','in_review')",
        "more_information" => "s.state='more_information_required'",
        "completed" => "s.state IN ('approved','changes_required','rejected')",
        "all" => "1=1",
        _ => return error("QUEUE_INVALID", "Review queue is invalid", 422),
    };
    let (limit, offset) = page(&req);
    let submissions: Vec<Value> = store::rows(
        &db(&ctx)?,
        &format!(
            "SELECT s.submission_id,s.app_id,s.build_id,s.version,s.submission_number,
                    s.submission_kind,s.state,s.submitted_at,s.created_at,d.app_name,
                    d.developer_name,a.bundle_id,a.developer_id,b.machine_status
               FROM submissions s JOIN submission_details d USING(submission_id)
               JOIN apps a ON a.app_id=s.app_id JOIN app_builds b ON b.build_id=s.build_id
              WHERE {condition}
              ORDER BY COALESCE(s.submitted_at,s.created_at),s.submission_id LIMIT ?1 OFFSET ?2"
        ),
        &[store::number(limit), store::number(offset)],
    )
    .await?;
    json_response(
        &json!({"admin":actor,"queue":queue,"submissions":submissions}),
        200,
    )
}

async fn admin_submission(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let submission_id = param(&ctx, "submission_id");
    let Some(submission) = submission_payload(&db(&ctx)?, submission_id).await? else {
        return error("SUBMISSION_NOT_FOUND", "Submission not found", 404);
    };
    json_response(&json!({"admin":actor,"submission":submission}), 200)
}

async fn start_submission_review(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let submission_id = param(&ctx, "submission_id");
    let database = db(&ctx)?;
    let updated: Option<Value> = store::first(
        &database,
        "UPDATE submissions SET state='in_review',updated_at=?1
          WHERE submission_id=?2 AND state='submitted' RETURNING submission_id",
        &[store::number(now()), store::value(submission_id)],
    )
    .await?;
    if updated.is_none() {
        return error(
            "SUBMISSION_NOT_REVIEWABLE",
            "Only a submitted Submission can enter review",
            409,
        );
    }
    store::audit(
        &database,
        Some(&actor),
        "submission.review_started",
        "submission",
        submission_id,
        json!({}),
        now(),
    )
    .await?;
    json_response(
        &json!({"submission":submission_payload(&database, submission_id).await?}),
        200,
    )
}

async fn decide_submission(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let input: ReviewDecisionInput = match bounded_json(&mut req).await? {
        Ok(input) => input,
        Err(response) => return Ok(response),
    };
    if !matches!(
        input.decision.as_str(),
        "approved" | "changes_required" | "more_information_required" | "rejected"
    ) || input.reason.trim().is_empty()
        || input.reason.trim().chars().count() > 8000
    {
        return error("REVIEW_DECISION_INVALID", "Review decision is invalid", 422);
    }
    let submission_id = param(&ctx, "submission_id");
    let database = db(&ctx)?;
    let Some(submission) = store::first::<Value>(
        &database,
        "SELECT s.*,a.bundle_id,a.developer_id,b.machine_status,b.certificate_id,
                d.app_name,d.description,d.icon_url,d.category,d.kind,d.age_rating
           FROM submissions s JOIN apps a ON a.app_id=s.app_id
           JOIN app_builds b ON b.build_id=s.build_id
           JOIN submission_details d USING(submission_id)
          WHERE s.submission_id=?1",
        &[store::value(submission_id)],
    )
    .await?
    else {
        return error("SUBMISSION_NOT_FOUND", "Submission not found", 404);
    };
    if value_str(&submission, "state") != Some("in_review") {
        return error(
            "SUBMISSION_NOT_REVIEWABLE",
            "Only an In Review Submission can receive a decision",
            409,
        );
    }
    let timestamp = now();
    let review_id = id("review");
    let mut statements = vec![
        database
            .prepare(
                "INSERT INTO submission_reviews(review_id,submission_id,reviewer_account_id,
                   decision,reason,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
            )
            .bind(&[
                store::value(&review_id),
                store::value(submission_id),
                store::value(&actor),
                store::value(&input.decision),
                store::value(input.reason.trim()),
                store::number(timestamp),
            ])?,
        database
            .prepare(
                "UPDATE submissions SET state=?1,
                   resolved_at=CASE WHEN ?1 IN ('approved','changes_required','rejected') THEN ?2 ELSE NULL END,
                   updated_at=?2
                  WHERE submission_id=?3 AND state='in_review'",
            )
            .bind(&[
                store::value(&input.decision),
                store::number(timestamp),
                store::value(submission_id),
            ])?,
    ];
    if input.decision == "approved" {
        if value_str(&submission, "machine_status") != Some("valid") {
            return error("BUILD_NOT_VALIDATED", "Build is not valid", 409);
        }
        let developer_id = value_str(&submission, "developer_id").unwrap_or("");
        let certificate_id = value_str(&submission, "certificate_id").unwrap_or("");
        if auth::certificate_identity(&ctx.env, certificate_id, developer_id)
            .await?
            .is_none()
        {
            return error(
                "CERTIFICATE_INVALID",
                "The App Developer Certificate is not active",
                409,
            );
        }
        let app_id = value_str(&submission, "app_id").unwrap_or("");
        let version = value_str(&submission, "version").unwrap_or("");
        let kind = value_str(&submission, "submission_kind").unwrap_or("");
        if kind != "re_review"
            && store::first::<Value>(
                &database,
                "SELECT version FROM published_versions WHERE app_id=?1 AND version=?2",
                &[store::value(app_id), store::value(version)],
            )
            .await?
            .is_some()
        {
            return error(
                "VERSION_ALREADY_PUBLISHED",
                "This version has already been published",
                409,
            );
        }
        if kind == "re_review" {
            statements.push(
                database
                    .prepare(
                        "INSERT OR IGNORE INTO published_versions(app_id,version,submission_id,published_at)
                         VALUES(?1,?2,?3,?4)",
                    )
                    .bind(&[
                        store::value(app_id),
                        store::value(version),
                        store::value(submission_id),
                        store::number(timestamp),
                    ])?,
            );
        } else {
            statements.push(
                database
                    .prepare(
                        "INSERT INTO published_versions(app_id,version,submission_id,published_at)
                         VALUES(?1,?2,?3,?4)",
                    )
                    .bind(&[
                        store::value(app_id),
                        store::value(version),
                        store::value(submission_id),
                        store::number(timestamp),
                    ])?,
            );
        }
        let previous_status: Option<Value> = store::first(
            &database,
            "SELECT status FROM app_availability WHERE app_id=?1",
            &[store::value(app_id)],
        )
        .await?;
        let from_status = previous_status
            .as_ref()
            .and_then(|value| value_str(value, "status"));
        statements.extend([
            database.prepare(
                "INSERT INTO app_availability(app_id,status,current_submission_id,reason,
                   changed_by_account_id,changed_at) VALUES(?1,'available',?2,NULL,?3,?4)
                 ON CONFLICT(app_id) DO UPDATE SET status='available',current_submission_id=excluded.current_submission_id,
                   reason=NULL,changed_by_account_id=excluded.changed_by_account_id,changed_at=excluded.changed_at",
            ).bind(&[
                store::value(app_id),store::value(submission_id),store::value(&actor),store::number(timestamp),
            ])?,
            database.prepare(
                "UPDATE apps SET display_name=?1,description=?2,icon_url=?3,category=?4,kind=?5,
                   age_rating=?6,latest_version=?7,visibility='public',updated_at=?8 WHERE app_id=?9",
            ).bind(&[
                store::value(value_str(&submission,"app_name").unwrap_or("")),
                store::value(value_str(&submission,"description").unwrap_or("")),
                store::value(value_str(&submission,"icon_url").unwrap_or("")),
                optional_value(value_str(&submission,"category")),
                store::value(value_str(&submission,"kind").unwrap_or("app")),
                optional_value(value_str(&submission,"age_rating")),store::value(version),
                store::number(timestamp),store::value(app_id),
            ])?,
            database.prepare("DELETE FROM app_screenshots WHERE bundle_id=?1")
                .bind(&[store::value(value_str(&submission,"bundle_id").unwrap_or(""))])?,
            database.prepare(
                "INSERT INTO app_screenshots(bundle_id,position,image_url)
                 SELECT ?1,position,image_url FROM submission_screenshots WHERE submission_id=?2",
            ).bind(&[
                store::value(value_str(&submission,"bundle_id").unwrap_or("")),store::value(submission_id),
            ])?,
            database.prepare(
                "INSERT INTO availability_history(event_id,app_id,from_status,to_status,reason,
                   actor_account_id,created_at) VALUES(?1,?2,?3,'available',?4,?5,?6)",
            ).bind(&[
                store::value(id("availability")),store::value(app_id),optional_value(from_status),
                store::value(input.reason.trim()),store::value(&actor),store::number(timestamp),
            ])?,
        ]);
    }
    statements.push(store::audit_statement(
        &database,
        Some(&actor),
        "submission.decision",
        "submission",
        submission_id,
        json!({"decision":input.decision,"reason":input.reason,
            "developer_id":value_str(&submission,"developer_id"),
            "package_id":value_str(&submission,"bundle_id")}),
        timestamp,
    )?);
    database.batch(statements).await?;
    json_response(
        &json!({"review_id":review_id,"submission":submission_payload(&database, submission_id).await?}),
        200,
    )
}

async fn admin_remove_app(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let input: RemovalInput = match bounded_json(&mut req).await? {
        Ok(input) => input,
        Err(response) => return Ok(response),
    };
    if input.confirmation != "REMOVE"
        || input.reason.trim().is_empty()
        || input.reason.trim().chars().count() > 8000
    {
        return error(
            "REMOVAL_CONFIRMATION_REQUIRED",
            "Removal reason and REMOVE confirmation are required",
            422,
        );
    }
    let bundle_id = param(&ctx, "bundle_id");
    let database = db(&ctx)?;
    let Some(app) = store::first::<Value>(
        &database,
        "SELECT a.app_id,v.status FROM apps a JOIN app_availability v ON v.app_id=a.app_id
          WHERE a.bundle_id=?1",
        &[store::value(bundle_id)],
    )
    .await?
    else {
        return error("APP_NOT_FOUND", "App not found", 404);
    };
    if value_str(&app, "status") != Some("available") {
        return error(
            "APP_NOT_AVAILABLE",
            "Only an available App can be removed",
            409,
        );
    }
    let app_id = value_str(&app, "app_id").unwrap_or("");
    let timestamp = now();
    database.batch(vec![
        database.prepare(
            "UPDATE app_availability SET status='removed',reason=?1,changed_by_account_id=?2,
               changed_at=?3 WHERE app_id=?4 AND status='available'",
        ).bind(&[
            store::value(input.reason.trim()),store::value(&actor),store::number(timestamp),store::value(app_id),
        ])?,
        database.prepare("UPDATE apps SET visibility='private',updated_at=?1 WHERE app_id=?2")
            .bind(&[store::number(timestamp),store::value(app_id)])?,
        database.prepare(
            "INSERT INTO availability_history(event_id,app_id,from_status,to_status,reason,
               actor_account_id,created_at) VALUES(?1,?2,'available','removed',?3,?4,?5)",
        ).bind(&[
            store::value(id("availability")),store::value(app_id),store::value(input.reason.trim()),
            store::value(&actor),store::number(timestamp),
        ])?,
        store::audit_statement(
            &database,Some(&actor),"app.removed","app",bundle_id,
            json!({"reason":input.reason}),timestamp,
        )?,
    ]).await?;
    json_response(&json!({"status":"removed","reason":input.reason}), 200)
}

async fn admin_apps(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let availability = req
        .url()?
        .query_pairs()
        .find(|(key, _)| key == "availability")
        .map(|(_, value)| value.into_owned())
        .unwrap_or_else(|| "all".into());
    if !matches!(
        availability.as_str(),
        "available" | "developer_unpublished" | "removed" | "not_available" | "all"
    ) {
        return error("AVAILABILITY_INVALID", "Availability is invalid", 422);
    }
    let (limit, offset) = page(&req);
    let apps: Vec<Value> = if availability == "all" {
        store::rows(
            &db(&ctx)?,
            "SELECT a.app_id,a.bundle_id,a.developer_id,a.display_name,
                    COALESCE(v.status,'not_available') availability_status,v.reason,
                    v.current_submission_id,v.changed_by_account_id,v.changed_at
               FROM apps a LEFT JOIN app_availability v ON v.app_id=a.app_id
              ORDER BY COALESCE(v.changed_at,a.updated_at) DESC LIMIT ?1 OFFSET ?2",
            &[store::number(limit), store::number(offset)],
        )
        .await?
    } else {
        store::rows(
            &db(&ctx)?,
            "SELECT a.app_id,a.bundle_id,a.developer_id,a.display_name,v.status availability_status,
                    v.reason,v.current_submission_id,v.changed_by_account_id,v.changed_at
               FROM apps a JOIN app_availability v ON v.app_id=a.app_id
              WHERE v.status=?1 ORDER BY v.changed_at DESC LIMIT ?2 OFFSET ?3",
            &[
                store::value(&availability),
                store::number(limit),
                store::number(offset),
            ],
        )
        .await?
    };
    json_response(
        &json!({"admin":actor,"availability":availability,"apps":apps}),
        200,
    )
}

async fn admin_appeals(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let database = db(&ctx)?;
    if req.method() == Method::Get {
        let state = req
            .url()?
            .query_pairs()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value.into_owned())
            .unwrap_or_else(|| "submitted".into());
        if !matches!(
            state.as_str(),
            "submitted" | "in_review" | "resolved" | "all"
        ) {
            return error("APPEAL_STATE_INVALID", "Appeal state is invalid", 422);
        }
        let appeals: Vec<Value> = if state == "all" {
            store::rows(&database,"SELECT p.*,a.bundle_id,a.display_name FROM appeals p JOIN apps a ON a.app_id=p.app_id ORDER BY p.created_at",&[]).await?
        } else {
            store::rows(&database,"SELECT p.*,a.bundle_id,a.display_name FROM appeals p JOIN apps a ON a.app_id=p.app_id WHERE p.state=?1 ORDER BY p.created_at",&[store::value(&state)]).await?
        };
        return json_response(&json!({"admin":actor,"appeals":appeals}), 200);
    }
    let appeal_id = param(&ctx, "appeal_id");
    let input: AppealResolutionInput = match bounded_json(&mut req).await? {
        Ok(input) => input,
        Err(response) => return Ok(response),
    };
    if !matches!(input.outcome.as_str(), "accepted" | "dismissed")
        || input.reason.trim().is_empty()
        || input.reason.trim().chars().count() > 8000
    {
        return error(
            "APPEAL_RESOLUTION_INVALID",
            "Appeal resolution is invalid",
            422,
        );
    }
    let timestamp = now();
    let updated: Option<Value> = store::first(
        &database,
        "UPDATE appeals SET state='resolved',resolution=?1,resolved_by_account_id=?2,resolved_at=?3
          WHERE appeal_id=?4 AND state IN ('submitted','in_review') RETURNING appeal_id",
        &[
            store::value(format!("{}: {}", input.outcome, input.reason.trim())),
            store::value(&actor),
            store::number(timestamp),
            store::value(appeal_id),
        ],
    )
    .await?;
    if updated.is_none() {
        return error("APPEAL_NOT_RESOLVABLE", "Appeal cannot be resolved", 409);
    }
    store::audit(
        &database,
        Some(&actor),
        "appeal.resolve",
        "appeal",
        appeal_id,
        json!({"outcome":input.outcome,"reason":input.reason}),
        timestamp,
    )
    .await?;
    json_response(
        &json!({"appeal_id":appeal_id,"state":"resolved","outcome":input.outcome}),
        200,
    )
}

async fn admin_releases(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let status = req
        .url()?
        .query_pairs()
        .find(|(k, _)| k == "status")
        .map(|(_, v)| v.into_owned())
        .unwrap_or_else(|| "submitted".into());
    let (limit, offset) = page(&req);
    let (sql, bindings) = if status == "queue" {
        (
            "SELECT r.*,a.display_name,a.icon_url,a.description
               FROM releases r LEFT JOIN apps a ON a.bundle_id=r.bundle_id
              WHERE r.review_status IN ('pending','submitted') AND r.publish_status='draft'
              ORDER BY COALESCE(r.submitted_at,r.created_at) DESC LIMIT ?1 OFFSET ?2"
                .to_owned(),
            vec![store::number(limit), store::number(offset)],
        )
    } else {
        let (column, value) = match status.as_str() {
            "pending" | "submitted" | "approved" | "rejected" => ("review_status", status.as_str()),
            "draft" | "published" | "revoked" => ("publish_status", status.as_str()),
            _ => return error("VALIDATION_ERROR", "status is invalid", 422),
        };
        (
            format!(
                "SELECT r.*,a.display_name,a.icon_url,a.description
                   FROM releases r LEFT JOIN apps a ON a.bundle_id=r.bundle_id
                  WHERE r.{column}=?1
                    AND (?1!='submitted' OR (r.validation_status='valid' AND r.publish_status='draft'))
                  ORDER BY r.submitted_at DESC,r.created_at DESC LIMIT ?2 OFFSET ?3"
            ),
            vec![
                store::value(value),
                store::number(limit),
                store::number(offset),
            ],
        )
    };
    let rows: Vec<Value> = store::rows(&db(&ctx)?, &sql, &bindings).await?;
    json_response(&json!({"admin":actor,"status":status,"releases":rows}), 200)
}

async fn approve_release(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) = rate_limited(&req, &ctx.env, "MUTATION_RATE_LIMITER", "approve").await?
    {
        return Ok(response);
    }
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let release_id = param(&ctx, "release_id");
    let Some(release) = store::release_by_id(&db(&ctx)?, release_id).await? else {
        return error("RELEASE_NOT_FOUND", "Release not found", 404);
    };
    if value_str(&release, "validation_status") != Some("valid")
        || value_str(&release, "review_status") != Some("submitted")
        || value_str(&release, "publish_status") != Some("draft")
    {
        return error(
            "INVALID_RELEASE_STATUS",
            "Only validated submitted releases can be approved",
            409,
        );
    }
    let Some(identity) = auth::certificate_identity(
        &ctx.env,
        value_str(&release, "developer_certificate_id").unwrap_or(""),
        value_str(&release, "registered_by").unwrap_or(""),
    )
    .await?
    else {
        return error(
            "CERTIFICATE_INVALID",
            "The Developer Certificate is no longer valid",
            403,
        );
    };
    if !certificate_matches_release(&identity, &release) {
        return error(
            "CERTIFICATE_IDENTITY_MISMATCH",
            "Developer Certificate identity differs from the reviewed release",
            409,
        );
    }
    let timestamp = now();
    store::run(&db(&ctx)?,"UPDATE releases SET review_status='approved',publish_status='published',review_message=NULL,reviewed_at=?1,reviewed_by=?2,published_at=?1 WHERE release_id=?3",&[store::number(timestamp),store::value(&actor),store::value(release_id)]).await?;
    store::run(
        &db(&ctx)?,
        "UPDATE apps SET latest_version=?1,visibility='public',updated_at=?2 WHERE bundle_id=?3",
        &[
            store::value(value_str(&release, "version").unwrap_or("")),
            store::number(timestamp),
            store::value(value_str(&release, "bundle_id").unwrap_or("")),
        ],
    )
    .await?;
    store::audit(
        &db(&ctx)?,
        Some(&actor),
        "release.approve",
        "release",
        release_id,
        json!({"developer_id":value_str(&release,"registered_by"),"asset_id":release.get("github_asset_id"),"package_id":value_str(&release,"bundle_id"),"result":"approved"}),
        timestamp,
    )
    .await?;
    store::audit(
        &db(&ctx)?,
        Some(&actor),
        "release.publish",
        "release",
        release_id,
        json!({"developer_id":value_str(&release,"registered_by"),"asset_id":release.get("github_asset_id"),"package_id":value_str(&release,"bundle_id"),"result":"published"}),
        timestamp,
    )
    .await?;
    json_response(
        &json!({"release":store::release_by_id(&db(&ctx)?,release_id).await?}),
        200,
    )
}

async fn reject_release(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) = rate_limited(&req, &ctx.env, "MUTATION_RATE_LIMITER", "reject").await? {
        return Ok(response);
    }
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let release_id = param(&ctx, "release_id");
    let input: RejectInput = req.json().await?;
    if !matches!(
        input.reason_code.as_str(),
        "metadata_incorrect"
            | "misleading_description"
            | "malicious_behavior"
            | "policy_violation"
            | "duplicate_application"
            | "broken_application"
            | "other"
    ) || input.note.chars().count() > 2000
    {
        return error(
            "VALIDATION_ERROR",
            "rejection reason or note is invalid",
            422,
        );
    }
    let Some(release) = store::release_by_id(&db(&ctx)?, release_id).await? else {
        return error("RELEASE_NOT_FOUND", "Release not found", 404);
    };
    if value_str(&release, "review_status") != Some("submitted")
        || value_str(&release, "publish_status") != Some("draft")
    {
        return error(
            "INVALID_RELEASE_STATUS",
            "Only submitted releases can be rejected",
            409,
        );
    }
    let timestamp = now();
    store::run(&db(&ctx)?,"UPDATE releases SET review_status='rejected',publish_status='draft',review_message=?1,rejection_reason_code=?2,rejection_note=?1,reviewed_at=?3,reviewed_by=?4 WHERE release_id=?5",&[store::value(input.note.trim()),store::value(&input.reason_code),store::number(timestamp),store::value(&actor),store::value(release_id)]).await?;
    store::audit(
        &db(&ctx)?,
        Some(&actor),
        "release.reject",
        "release",
        release_id,
        json!({"developer_id":value_str(&release,"registered_by"),"asset_id":release.get("github_asset_id"),"package_id":value_str(&release,"bundle_id"),"result":"rejected","reason_code":input.reason_code,"note":input.note}),
        timestamp,
    )
    .await?;
    json_response(
        &json!({"release":store::release_by_id(&db(&ctx)?,release_id).await?}),
        200,
    )
}

async fn admin_download(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let _actor = match require_admin(&req, &ctx.env)? {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let release_id = param(&ctx, "release_id");
    let Some(release) = store::release_by_id(&db(&ctx)?, release_id).await? else {
        return error("RELEASE_NOT_FOUND", "Release not found", 404);
    };
    redirect_to(
        value_str(&release, "download_url").unwrap_or(""),
        "no-store",
    )
}

async fn admin_packages(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let status = req
        .url()?
        .query_pairs()
        .find(|(key, _)| key == "status")
        .map(|(_, value)| value.into_owned())
        .unwrap_or_else(|| "active".into());
    if !matches!(status.as_str(), "active" | "blocked") {
        return error("VALIDATION_ERROR", "status is invalid", 422);
    }
    let rows: Vec<Value> = store::rows(
        &db(&ctx)?,
        "SELECT a.*, b.status AS package_status, s.reason AS suspension_reason,
                s.suspended_by_account_id, s.suspended_at
         FROM apps a JOIN bundle_ids b ON b.bundle_id=a.bundle_id
         LEFT JOIN package_suspensions s ON s.bundle_id=a.bundle_id
         WHERE b.status=?1 ORDER BY a.updated_at DESC LIMIT 100",
        &[store::value(&status)],
    )
    .await?;
    json_response(&json!({"admin":actor,"status":status,"packages":rows}), 200)
}

async fn set_package_suspension(
    mut req: Request,
    ctx: RouteContext<()>,
    suspended: bool,
) -> Result<Response> {
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    let bundle_id = param(&ctx, "bundle_id");
    if !valid_bundle_id(bundle_id) {
        return error("VALIDATION_ERROR", "bundle_id is invalid", 422);
    }
    let database = db(&ctx)?;
    let app: Option<Value> = store::first(
        &database,
        "SELECT a.bundle_id,b.status FROM apps a JOIN bundle_ids b ON b.bundle_id=a.bundle_id WHERE a.bundle_id=?1",
        &[store::value(bundle_id)],
    )
    .await?;
    let Some(app) = app else {
        return error("APP_NOT_FOUND", "App not found", 404);
    };
    if !matches!(value_str(&app, "status"), Some("active" | "blocked")) {
        return error("INVALID_PACKAGE_STATUS", "Package cannot be suspended", 409);
    }
    let reason = if suspended {
        let input: SuspensionInput = req.json().await?;
        let reason = input.reason.trim().to_owned();
        if reason.is_empty() || reason.len() > 2000 {
            return error(
                "SUSPENSION_REASON_REQUIRED",
                "Suspension reason required",
                422,
            );
        }
        Some(reason)
    } else {
        None
    };
    let timestamp = now();
    let package_status = if suspended { "blocked" } else { "active" };
    let suspension_statement = if suspended {
        database.prepare(
            "INSERT INTO package_suspensions(bundle_id,suspended_by_account_id,reason,suspended_at)
             VALUES(?1,?2,?3,?4)
             ON CONFLICT(bundle_id) DO UPDATE SET suspended_by_account_id=excluded.suspended_by_account_id,
             reason=excluded.reason,suspended_at=excluded.suspended_at",
        )
        .bind(&[
            store::value(bundle_id),
            store::value(&actor),
            store::value(reason.as_deref().unwrap_or("administrative")),
            store::number(timestamp),
        ])?
    } else {
        database
            .prepare("DELETE FROM package_suspensions WHERE bundle_id=?1")
            .bind(&[store::value(bundle_id)])?
    };
    database
        .batch(vec![
            database
                .prepare("UPDATE bundle_ids SET status=?1 WHERE bundle_id=?2")
                .bind(&[store::value(package_status), store::value(bundle_id)])?,
            suspension_statement,
            database
                .prepare("INSERT INTO audit_logs(audit_id,actor_id,action,target_type,target_id,metadata_json,created_at) VALUES(?1,?2,?3,'package',?4,?5,?6)")
                .bind(&[
                    store::value(id("audit")),
                    store::value(&actor),
                    store::value(if suspended { "package.suspend" } else { "package.restore" }),
                    store::value(bundle_id),
                    store::value(json!({"reason":reason}).to_string()),
                    store::number(timestamp),
                ])?,
        ])
        .await?;
    let package: Option<Value> = store::first(
        &database,
        "SELECT a.*,b.status AS package_status,s.reason AS suspension_reason,s.suspended_at
         FROM apps a JOIN bundle_ids b ON b.bundle_id=a.bundle_id
         LEFT JOIN package_suspensions s ON s.bundle_id=a.bundle_id WHERE a.bundle_id=?1",
        &[store::value(bundle_id)],
    )
    .await?;
    json_response(&json!({"package":package}), 200)
}

async fn public_key(_: Request, ctx: RouteContext<()>) -> Result<Response> {
    let material = param(&ctx, "public_key");
    let key: Option<Value> = store::first(
        &db(&ctx)?,
        "SELECT public_key,fingerprint,revoked_at FROM public_keys WHERE public_key=?1 LIMIT 1",
        &[store::value(material)],
    )
    .await?;
    match key {
        Some(key) => json_response(&json!({"key":key}), 200),
        None => error("KEY_NOT_FOUND", "Key not found", 404),
    }
}

async fn keys(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let developer = match require_developer(&req, &ctx.env).await? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    if req.method() == Method::Get {
        let rows: Vec<Value> = store::rows(
            &db(&ctx)?,
            "SELECT * FROM public_keys WHERE developer_id=?1 ORDER BY created_at DESC",
            &[store::value(&developer)],
        )
        .await?;
        return json_response(&json!({"keys":rows}), 200);
    }
    let input: KeyInput = req.json().await?;
    if input.key_id.trim().is_empty()
        || input.public_key.trim().is_empty()
        || input.fingerprint.trim().is_empty()
    {
        return error("VALIDATION_ERROR", "Key metadata is invalid", 422);
    }
    let result=store::run(&db(&ctx)?,"INSERT INTO public_keys(key_id,developer_id,public_key,fingerprint,created_at) VALUES(?1,?2,?3,?4,?5)",&[store::value(input.key_id.trim()),store::value(&developer),store::value(input.public_key.trim()),store::value(input.fingerprint.trim()),store::number(now())]).await;
    if result.is_err() {
        return error("KEY_ALREADY_EXISTS", "Public key already exists", 409);
    }
    json_response(&json!({"key_id":input.key_id}), 201)
}

async fn revoke_key(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let developer = match require_developer(&req, &ctx.env).await? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let key_id = param(&ctx, "key_id");
    store::run(&db(&ctx)?,"UPDATE public_keys SET revoked_at=?1 WHERE key_id=?2 AND developer_id=?3 AND revoked_at IS NULL",&[store::number(now()),store::value(key_id),store::value(&developer)]).await?;
    let key: Option<Value> = store::first(
        &db(&ctx)?,
        "SELECT * FROM public_keys WHERE key_id=?1 AND developer_id=?2",
        &[store::value(key_id), store::value(&developer)],
    )
    .await?;
    match key {
        Some(key) => json_response(&json!({"key":key}), 200),
        None => error("KEY_NOT_FOUND", "Key not found", 404),
    }
}

async fn teams(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let developer = match require_developer(&req, &ctx.env).await? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    if req.method() == Method::Get {
        let rows:Vec<Value>=store::rows(&db(&ctx)?,"SELECT t.*,m.role,m.joined_at FROM teams t JOIN team_members m ON m.team_id=t.team_id WHERE m.developer_id=?1 ORDER BY t.created_at DESC",&[store::value(&developer)]).await?;
        return json_response(&json!({"teams":rows}), 200);
    }
    let input: TeamInput = req.json().await?;
    if input.name.trim().is_empty() || input.slug.trim().is_empty() {
        return error("VALIDATION_ERROR", "name and slug are required", 422);
    }
    let team_id = id("team");
    let timestamp = now();
    store::run(
        &db(&ctx)?,
        "INSERT INTO teams(team_id,name,slug,created_by,created_at) VALUES(?1,?2,?3,?4,?5)",
        &[
            store::value(&team_id),
            store::value(input.name.trim()),
            store::value(input.slug.trim()),
            store::value(&developer),
            store::number(timestamp),
        ],
    )
    .await?;
    store::run(
        &db(&ctx)?,
        "INSERT INTO team_members(team_id,developer_id,role,joined_at) VALUES(?1,?2,'owner',?3)",
        &[
            store::value(&team_id),
            store::value(&developer),
            store::number(timestamp),
        ],
    )
    .await?;
    json_response(
        &json!({"team_id":team_id,"name":input.name,"slug":input.slug,"role":"owner"}),
        201,
    )
}

async fn team_members(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let developer = match require_developer(&req, &ctx.env).await? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let team_id = param(&ctx, "team_id");
    let membership: Option<Value> = store::first(
        &db(&ctx)?,
        "SELECT role FROM team_members WHERE team_id=?1 AND developer_id=?2",
        &[store::value(team_id), store::value(&developer)],
    )
    .await?;
    let Some(membership) = membership else {
        return error("TEAM_NOT_FOUND", "Team not found", 404);
    };
    if req.method() == Method::Get {
        let rows: Vec<Value> = store::rows(
            &db(&ctx)?,
            "SELECT * FROM team_members WHERE team_id=?1 ORDER BY joined_at",
            &[store::value(team_id)],
        )
        .await?;
        return json_response(&json!({"members":rows}), 200);
    }
    if !matches!(value_str(&membership, "role"), Some("owner" | "admin")) {
        return error("FORBIDDEN", "Team admin permission is required", 403);
    }
    let input: MemberInput = req.json().await?;
    if !valid_role(&input.role) {
        return error("VALIDATION_ERROR", "role is invalid", 422);
    }
    store::run(&db(&ctx)?,"INSERT INTO team_members(team_id,developer_id,role,joined_at) VALUES(?1,?2,?3,?4) ON CONFLICT(team_id,developer_id) DO UPDATE SET role=excluded.role",&[store::value(team_id),store::value(&input.developer_id),store::value(&input.role),store::number(now())]).await?;
    json_response(&json!({"member":input}), 201)
}

async fn assign_team(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let developer = match require_developer(&req, &ctx.env).await? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let bundle_id = param(&ctx, "bundle_id");
    let input: TeamAssignment = req.json().await?;
    if store::developer_app(&db(&ctx)?, &developer, bundle_id)
        .await?
        .is_none()
    {
        return error("APP_NOT_FOUND", "App not found", 404);
    }
    if let Some(team_id) = input.team_id.as_deref() {
        let member:Option<Value>=store::first(&db(&ctx)?,"SELECT role FROM team_members WHERE team_id=?1 AND developer_id=?2 AND role IN ('owner','admin','developer')",&[store::value(team_id),store::value(&developer)]).await?;
        if member.is_none() {
            return error("TEAM_NOT_FOUND", "Assignable team not found", 404);
        }
    }
    store::run(
        &db(&ctx)?,
        "UPDATE apps SET team_id=?1,updated_at=?2 WHERE bundle_id=?3 AND developer_id=?4",
        &[
            input
                .team_id
                .as_deref()
                .map_or(worker::wasm_bindgen::JsValue::NULL, store::value),
            store::number(now()),
            store::value(bundle_id),
            store::value(&developer),
        ],
    )
    .await?;
    json_response(
        &json!({"app":store::developer_app(&db(&ctx)?,&developer,bundle_id).await?}),
        200,
    )
}

fn release_github_identity_matches(release: &Value, asset: &GitHubReleaseAsset) -> bool {
    release.get("github_repository_id").and_then(Value::as_u64) == Some(asset.repository_id)
        && value_str(release, "github_repository") == Some(asset.repository.as_str())
        && release.get("github_release_id").and_then(Value::as_u64) == Some(asset.release_id)
        && value_str(release, "github_release_tag") == Some(asset.release_tag.as_str())
        && release.get("github_asset_id").and_then(Value::as_u64) == Some(asset.asset_id)
        && value_str(release, "asset_name") == Some(asset.asset_name.as_str())
        && release.get("file_size").and_then(Value::as_u64) == Some(asset.file_size)
        && value_str(release, "download_url") == Some(asset.download_url.as_str())
        && value_str(release, "github_asset_created_at") == Some(asset.asset_created_at.as_str())
        && value_str(release, "github_asset_updated_at") == Some(asset.asset_updated_at.as_str())
        && match (
            value_str(release, "github_digest"),
            asset.github_digest.as_deref(),
        ) {
            (Some(expected), Some(actual)) => expected.eq_ignore_ascii_case(actual),
            (None, None) => true,
            _ => false,
        }
}

async fn withdraw_release(
    env: &Env,
    database: &D1Database,
    release: &Value,
    reason_code: &str,
) -> Result<()> {
    let release_id = value_str(release, "release_id").unwrap_or("");
    let bundle_id = value_str(release, "bundle_id").unwrap_or("");
    let timestamp = now();
    store::run(
        database,
        "UPDATE releases SET validation_status='invalid',review_status='rejected',
            publish_status='revoked',validation_error_code=?1,validation_message=?1,
            withdrawn_at=?2,last_integrity_checked_at=?2
          WHERE release_id=?3 AND publish_status='published'",
        &[
            store::value(reason_code),
            store::number(timestamp),
            store::value(release_id),
        ],
    )
    .await?;
    store::run(
        database,
        "UPDATE apps SET visibility='private',latest_version=NULL,updated_at=?1
          WHERE bundle_id=?2 AND NOT EXISTS(
            SELECT 1 FROM releases WHERE bundle_id=?2 AND validation_status='valid'
              AND review_status='approved' AND publish_status='published')",
        &[store::number(timestamp), store::value(bundle_id)],
    )
    .await?;
    store::audit(
        database,
        None,
        "release.withdraw",
        "release",
        release_id,
        json!({"developer_id":value_str(release,"registered_by"),"asset_id":release.get("github_asset_id"),"package_id":bundle_id,"result":"withdrawn","reason_code":reason_code}),
        timestamp,
    )
    .await?;
    console_warn!(
        "{}",
        json!({"message":"release withdrawn","release_id":release_id,"reason_code":reason_code,"service":"app-store-api"})
    );
    let _ = env;
    Ok(())
}

async fn check_release_integrity(
    env: &Env,
    database: &D1Database,
    release: &Value,
) -> Result<bool> {
    let Some(identity) = auth::certificate_identity(
        env,
        value_str(release, "developer_certificate_id").unwrap_or(""),
        value_str(release, "registered_by").unwrap_or(""),
    )
    .await?
    else {
        withdraw_release(env, database, release, "certificate_inactive").await?;
        return Ok(false);
    };
    if !certificate_matches_release(&identity, release) {
        withdraw_release(env, database, release, "certificate_identity_changed").await?;
        return Ok(false);
    }
    let Some((owner, repository)) =
        github_repository(value_str(release, "github_repository").unwrap_or(""))
    else {
        withdraw_release(env, database, release, "asset_identity_invalid").await?;
        return Ok(false);
    };
    let request = GitHubReleaseAssetRequest {
        owner,
        repository,
        release_tag: value_str(release, "github_release_tag").unwrap_or(""),
        asset_name: value_str(release, "asset_name").unwrap_or(""),
    };
    let account_id = value_str(release, "registered_by_account_id").unwrap_or("");
    let verified = auth::github_release_asset_for_account(env, account_id, &request).await?;
    let Ok(verified) = verified else {
        withdraw_release(env, database, release, "github_asset_unavailable").await?;
        return Ok(false);
    };
    if verified.account_id != account_id
        || !release_github_identity_matches(release, &verified.release_asset)
    {
        withdraw_release(env, database, release, "github_asset_changed").await?;
        return Ok(false);
    }
    store::run(
        database,
        "UPDATE releases SET last_integrity_checked_at=?1 WHERE release_id=?2",
        &[
            store::number(now()),
            store::value(value_str(release, "release_id").unwrap_or("")),
        ],
    )
    .await?;
    Ok(true)
}

async fn integrity_check(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) =
        rate_limited(&req, &ctx.env, "MUTATION_RATE_LIMITER", "integrity").await?
    {
        return Ok(response);
    }
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let release_id = param(&ctx, "release_id");
    let Some(release) = store::release_by_id(&db(&ctx)?, release_id).await? else {
        return error("RELEASE_NOT_FOUND", "Release not found", 404);
    };
    let valid = check_release_integrity(&ctx.env, &db(&ctx)?, &release).await?;
    store::audit(
        &db(&ctx)?,
        Some(&actor),
        "release.integrity_check",
        "release",
        release_id,
        json!({"result":if valid {"valid"} else {"withdrawn"}}),
        now(),
    )
    .await?;
    json_response(
        &json!({"valid":valid,"release":store::release_by_id(&db(&ctx)?,release_id).await?}),
        200,
    )
}

async fn request_revalidation(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    if let Some(response) =
        rate_limited(&req, &ctx.env, "REVIEWER_RATE_LIMITER", "revalidate").await?
    {
        return Ok(response);
    }
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let release_id = param(&ctx, "release_id");
    let Some(release) = store::release_by_id(&db(&ctx)?, release_id).await? else {
        return error("RELEASE_NOT_FOUND", "Release not found", 404);
    };
    if value_str(&release, "validation_status") == Some("pending") {
        return error(
            "REVALIDATION_ALREADY_PENDING",
            "Release validation is already pending",
            409,
        );
    }
    let timestamp = now();
    store::run(
        &db(&ctx)?,
        "UPDATE releases SET validation_status='pending',review_status='pending',
            publish_status='draft',sha256=NULL,package_digest=NULL,manifest_hash=NULL,
            signature=NULL,capabilities_json=NULL,payloads_json=NULL,reviewer_version=NULL,
            validation_error_code=NULL,validation_message=NULL,validated_at=NULL,validated_by=NULL,
            submitted_at=NULL,reviewed_at=NULL,reviewed_by=NULL,published_at=NULL,withdrawn_at=?1,
            validation_attempt_id=NULL,validation_started_at=NULL
          WHERE release_id=?2",
        &[store::number(timestamp), store::value(release_id)],
    )
    .await?;
    store::run(
        &db(&ctx)?,
        "UPDATE apps SET visibility='private',latest_version=NULL,updated_at=?1
          WHERE bundle_id=?2 AND NOT EXISTS(
            SELECT 1 FROM releases WHERE bundle_id=?2 AND validation_status='valid'
              AND review_status='approved' AND publish_status='published')",
        &[
            store::number(timestamp),
            store::value(value_str(&release, "bundle_id").unwrap_or("")),
        ],
    )
    .await?;
    store::audit(
        &db(&ctx)?,
        Some(&actor),
        "release.revalidation_requested",
        "release",
        release_id,
        json!({"developer_id":value_str(&release,"registered_by"),"asset_id":release.get("github_asset_id"),"package_id":value_str(&release,"bundle_id"),"result":"pending"}),
        timestamp,
    )
    .await?;
    json_response(
        &json!({"release":store::release_by_id(&db(&ctx)?,release_id).await?}),
        200,
    )
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let origin = req.headers().get("Origin")?;
    if req.method() == Method::Options {
        return with_cors(Response::empty()?.with_status(204), origin.as_deref());
    }
    let result = Router::new()
        .get_async("/health", health)
        .get_async("/v1/apps", list_apps)
        .get_async("/v1/search", search)
        .get_async("/v1/storefront", storefront)
        .get_async("/v1/apps/:bundle_id/releases", app_releases)
        .get_async("/v1/apps/:bundle_id/status", app_status)
        .get_async("/v1/apps/:bundle_id/download", download)
        .post_async("/v1/apps/:bundle_id/acquisitions", acquire_app)
        .get_async("/v1/apps/:bundle_id", app_detail)
        .get_async("/v1/bundle-ids", bundle_ids)
        .post_async("/v1/bundle-ids", bundle_ids)
        .get_async("/v1/developer/apps", developer_apps)
        .post_async("/v1/developer/apps", developer_apps)
        .get_async("/v1/developer/apps/:bundle_id", developer_app)
        .patch_async("/v1/developer/apps/:bundle_id", developer_app)
        .get_async(
            "/v1/developer/apps/:bundle_id/submissions",
            developer_submissions,
        )
        .post_async(
            "/v1/developer/apps/:bundle_id/submissions",
            developer_submissions,
        )
        .get_async(
            "/v1/developer/apps/:bundle_id/submissions/:submission_id",
            developer_submission,
        )
        .patch_async(
            "/v1/developer/apps/:bundle_id/submissions/:submission_id",
            developer_submission,
        )
        .post_async(
            "/v1/developer/apps/:bundle_id/submissions/:submission_id/submit",
            submit_developer_submission,
        )
        .post_async(
            "/v1/developer/apps/:bundle_id/submissions/:submission_id/information",
            answer_submission,
        )
        .get_async("/v1/developer/apps/:bundle_id/appeals", developer_appeals)
        .post_async("/v1/developer/apps/:bundle_id/appeals", developer_appeals)
        .post_async(
            "/v1/developer/apps/:bundle_id/unpublish",
            developer_unpublish,
        )
        .patch_async(
            "/v1/developer/apps/:bundle_id/certificate",
            replace_app_certificate,
        )
        .post_async("/v1/developer/apps/:bundle_id/team", assign_team)
        .get_async(
            "/v1/developer/apps/:bundle_id/releases",
            list_developer_releases,
        )
        .post_async("/v1/developer/apps/:bundle_id/releases", create_release)
        .get_async(
            "/v1/developer/apps/:bundle_id/history",
            developer_app_history,
        )
        .get_async("/v1/developer/notifications", developer_notifications)
        .post_async(
            "/v1/developer/notifications/read-all",
            read_all_developer_notifications,
        )
        .post_async(
            "/v1/developer/notifications/:notification_id/read",
            read_developer_notification,
        )
        .get_async("/v1/admin/releases", admin_releases)
        .get_async("/v1/admin/submissions", admin_submissions)
        .get_async("/v1/admin/submissions/:submission_id", admin_submission)
        .post_async(
            "/v1/admin/submissions/:submission_id/start",
            start_submission_review,
        )
        .post_async(
            "/v1/admin/submissions/:submission_id/decision",
            decide_submission,
        )
        .get_async("/v1/admin/appeals", admin_appeals)
        .post_async("/v1/admin/appeals/:appeal_id/resolve", admin_appeals)
        .post_async("/v1/admin/apps/:bundle_id/remove", admin_remove_app)
        .get_async("/v1/admin/apps", admin_apps)
        .get_async("/v1/admin/notifications", admin_notifications)
        .post_async(
            "/v1/admin/notifications/read-all",
            read_all_admin_notifications,
        )
        .post_async(
            "/v1/admin/notifications/:notification_id/read",
            read_admin_notification,
        )
        .post_async("/v1/reviewer/releases/claim", claim_next_release)
        .get_async("/v1/admin/releases/:release_id", admin_release)
        .get_async(
            "/v1/admin/releases/:release_id/history",
            admin_release_history,
        )
        .post_async("/v1/admin/releases/:release_id/validate", validate_release)
        .post_async(
            "/v1/admin/releases/:release_id/validation-failure",
            invalidate_release,
        )
        .post_async("/v1/admin/releases/:release_id/approve", approve_release)
        .post_async("/v1/admin/releases/:release_id/reject", reject_release)
        .post_async(
            "/v1/admin/releases/:release_id/integrity-check",
            integrity_check,
        )
        .post_async(
            "/v1/admin/releases/:release_id/revalidate",
            request_revalidation,
        )
        .get_async("/v1/admin/releases/:release_id/download", admin_download)
        .get_async("/v1/admin/packages", admin_packages)
        .post_async("/v1/admin/packages/:bundle_id/suspend", |req, ctx| {
            set_package_suspension(req, ctx, true)
        })
        .post_async("/v1/admin/packages/:bundle_id/restore", |req, ctx| {
            set_package_suspension(req, ctx, false)
        })
        .get_async("/v1/keys", keys)
        .post_async("/v1/keys", keys)
        .post_async("/v1/keys/:key_id/revoke", revoke_key)
        .get_async("/v1/keys/:public_key", public_key)
        .get_async("/v1/teams", teams)
        .post_async("/v1/teams", teams)
        .get_async("/v1/teams/:team_id/members", team_members)
        .post_async("/v1/teams/:team_id/members", team_members)
        .run(req, env)
        .await;
    let response = match result {
        Ok(response) => response,
        Err(cause) => {
            console_error!(
                "{}",
                json!({"message":"request failed","error":cause.to_string()})
            );
            error("INTERNAL_ERROR", "Internal server error", 500)?
        }
    };
    with_cors(response, origin.as_deref())
}

#[event(scheduled)]
pub async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    let result: Result<()> = async {
        let database = env.d1("DB")?;
        let releases: Vec<Value> = store::rows(
            &database,
            "SELECT * FROM releases WHERE validation_status='valid'
              AND review_status='approved' AND publish_status='published'
              ORDER BY COALESCE(last_integrity_checked_at,0),published_at LIMIT 25",
            &[],
        )
        .await?;
        for release in releases {
            if let Err(cause) = check_release_integrity(&env, &database, &release).await {
                console_error!(
                    "{}",
                    json!({"message":"scheduled integrity check failed closed","release_id":value_str(&release,"release_id"),"error":cause.to_string()})
                );
                withdraw_release(&env, &database, &release, "integrity_check_failed").await?;
            }
        }
        Ok(())
    }
    .await;
    if let Err(cause) = result {
        console_error!("scheduled integrity job failed: {cause}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    #[test]
    fn validates_bundle_ids() {
        for valid in [
            "org.mochios.example",
            "com.example.paint",
            "io.github.user.tool",
            "dev.tas0.volume",
        ] {
            assert!(valid_bundle_id(valid), "rejected {valid}");
        }
        for invalid in ["Example", "app", "com..example", "com.-example", "../app"] {
            assert!(!valid_bundle_id(invalid), "accepted {invalid}");
        }
    }
    #[test]
    fn validates_versions() {
        assert!(valid_version("1.2.3-beta+1"));
        assert!(!valid_version("1/2"));
    }
    #[test]
    fn mpkg_limit_is_bounded() {
        assert_eq!(MAX_PACKAGE_BYTES, 134_217_728);
        assert_eq!(MAX_JSON_BODY_BYTES, 131_072);
    }

    #[test]
    fn registration_inputs_do_not_accept_price_or_minimum_os() {
        assert!(
            serde_json::from_value::<model::AppInput>(json!({
                "bundle_id": "com.example.testapp",
                "display_name": "TestApp",
                "kind": "app"
            }))
            .is_ok()
        );
        assert!(
            serde_json::from_value::<model::AppInput>(json!({
                "bundle_id": "com.example.testapp",
                "display_name": "TestApp",
                "kind": "app",
                "price_label": "入手"
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<model::ReleaseInput>(json!({
                "version": "0.1.0",
                "repository": "example/testapp",
                "release_tag": "v0.1.0",
                "asset": "TestApp.mpkg",
                "certificate_id": "certificate"
            }))
            .is_ok()
        );
        assert!(
            serde_json::from_value::<model::ReleaseInput>(json!({
                "version": "0.1.0",
                "repository": "example/testapp",
                "release_tag": "v0.1.0",
                "asset": "TestApp.mpkg",
                "certificate_id": "certificate",
                "minimum_mochios_version": "0.1.0"
            }))
            .is_err()
        );
    }

    #[test]
    fn app_metadata_updates_are_bounded_and_keep_bundle_ids_immutable() {
        assert!(valid_app_metadata(
            "Example",
            Some("A useful app"),
            "Description",
            Some("https://example.com/icon.png"),
            Some("Utilities"),
            "app",
            Some("4+")
        ));
        assert!(!valid_app_metadata(
            "Example",
            None,
            "Description",
            Some("http://example.com/icon.png"),
            None,
            "app",
            None
        ));
        assert!(!valid_icon_url(Some("https://127.0.0.1/icon.png")));
        assert!(!valid_icon_url(Some("https://service.internal/icon.png")));
        assert!(!valid_icon_url(Some("https://localhost/icon.png")));
        assert!(!valid_app_metadata(
            "",
            None,
            "Description",
            None,
            None,
            "application",
            None
        ));
        assert!(
            serde_json::from_value::<AppUpdateInput>(json!({
                "bundle_id":"com.example.changed",
                "display_name":"Example",
                "description":"Description",
                "kind":"app"
            }))
            .is_err()
        );
    }

    #[test]
    fn accepts_only_fixed_github_release_download_urls() {
        assert!(
            github_download_url(
                "https://github.com/mochiOS/example/releases/download/v1.0.0/example.mpkg"
            )
            .is_some()
        );
        assert!(github_download_url("https://example.com/releases/download/v1/app.mpkg").is_none());
        assert!(
            github_download_url("https://github.com/mochiOS/example/releases/latest").is_none()
        );
        assert!(
            github_download_url("https://user@github.com/a/b/releases/download/v1/a.mpkg")
                .is_none()
        );
    }

    #[test]
    fn rejects_github_ids_outside_javascript_integer_range() {
        assert_eq!(
            js_integer(MAX_SAFE_JS_INTEGER),
            Some(MAX_SAFE_JS_INTEGER as f64)
        );
        assert_eq!(js_integer(MAX_SAFE_JS_INTEGER + 1), None);
    }

    #[test]
    fn accepts_only_owner_repository_pairs() {
        assert_eq!(
            github_repository("mochiOS/TextEditor"),
            Some(("mochiOS", "TextEditor"))
        );
        assert!(github_repository("TextEditor").is_none());
        assert!(github_repository("owner/repo/extra").is_none());
    }

    #[test]
    fn verifies_the_mpkg_v1_manifest_domain_separator() {
        let key = SigningKey::from_bytes(&[7; 32]);
        let hash = [3; 32];
        let mut message = b"mochios-mpkg-manifest-v1\0".to_vec();
        message.extend_from_slice(&hash);
        let signature = key.sign(&message);
        assert!(valid_hash_signature(
            &STANDARD.encode(key.verifying_key().to_bytes()),
            &STANDARD.encode(signature.to_bytes()),
            &hex::encode(hash),
        ));
        assert!(!valid_hash_signature(
            &STANDARD.encode(key.verifying_key().to_bytes()),
            &STANDARD.encode(key.sign(&hash).to_bytes()),
            &hex::encode(hash),
        ));
    }

    #[test]
    fn certificate_identity_requires_every_registered_field_to_match() {
        let identity = auth::CertificateIdentity {
            public_key: "subject-public-key".into(),
            serial_number: "42".into(),
            subject_key_id: "subject-key-id".into(),
            developer_id: "019f9e5ac6687902b0e72fe53abfbef1".into(),
            developer_record_id: "019f9e5ac6687902b0e72fe53abfbef1".into(),
            issuer_key_id: "issuer-key-id".into(),
            issuer_public_key: "issuer-public-key".into(),
            issuance_source: "online_intermediate".into(),
        };
        let release = json!({
            "developer_public_key": identity.public_key,
            "developer_certificate_serial": identity.serial_number,
            "developer_certificate_subject_key_id": identity.subject_key_id,
            "developer_certificate_developer_id": identity.developer_id,
            "registered_by": identity.developer_record_id,
            "developer_certificate_issuer_key_id": identity.issuer_key_id,
            "developer_certificate_issuer_public_key": identity.issuer_public_key,
            "developer_certificate_issuance_source": identity.issuance_source,
        });
        assert!(certificate_matches_release(&identity, &release));
        for field in [
            "developer_public_key",
            "developer_certificate_serial",
            "developer_certificate_subject_key_id",
            "developer_certificate_developer_id",
            "registered_by",
            "developer_certificate_issuer_key_id",
            "developer_certificate_issuer_public_key",
            "developer_certificate_issuance_source",
        ] {
            let mut mismatched = release.clone();
            mismatched[field] = json!("mismatch");
            assert!(
                !certificate_matches_release(&identity, &mismatched),
                "accepted mismatched field: {field}"
            );
        }
    }

    #[test]
    fn admin_review_queue_includes_validation_pending_releases() {
        let source = include_str!("lib.rs");
        let production = source.split("#[cfg(test)]").next().unwrap_or_default();
        assert!(production.contains("status == \"queue\""));
        assert!(production.contains("review_status IN ('pending','submitted')"));
        assert!(production.contains("COALESCE(r.submitted_at,r.created_at)"));
    }

    #[test]
    fn automatic_reviewer_claim_is_authenticated_and_leased() {
        let source = include_str!("lib.rs");
        let production = source.split("#[cfg(test)]").next().unwrap_or_default();
        assert!(production.contains("/v1/reviewer/releases/claim"));
        assert!(production.contains("match require_reviewer(&req, &ctx.env)"));
        assert!(production.contains("ORDER BY created_at ASC,release_id ASC LIMIT 1"));
        assert!(production.contains("validation_started_at<?3"));
        assert!(production.contains("\"source\": \"automatic_queue\""));
    }

    #[test]
    fn notification_and_history_routes_keep_developer_and_operator_scopes_separate() {
        let source = include_str!("lib.rs");
        let production = source.split("#[cfg(test)]").next().unwrap_or_default();
        assert!(valid_notification_id("audit_019f9d57"));
        assert!(!valid_notification_id("../audit"));
        assert!(production.contains("/v1/developer/notifications/:notification_id/read"));
        assert!(production.contains("require_developer_actor(&req, &ctx.env)"));
        assert!(production.contains("/v1/admin/notifications/:notification_id/read"));
        assert!(production.contains("require_admin(&req, &ctx.env)"));
        assert!(production.contains("/v1/developer/apps/:bundle_id/history"));
        assert!(production.contains("/v1/admin/releases/:release_id/history"));
        assert!(production.contains("/v1/admin/submissions/:submission_id/decision"));
        assert!(production.contains("/v1/admin/apps/:bundle_id/remove"));
        let store = include_str!("store.rs");
        assert!(store.contains("'submission.decision','appeal.resolve','app.removed'"));
        assert!(
            store.contains("'submission.submit','submission.information_provided','appeal.submit'")
        );
    }
}
