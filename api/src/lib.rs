mod auth;
mod model;
mod store;

use std::collections::HashMap;

use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use ed25519_dalek::{Signature, VerifyingKey};
use model::*;
use serde::Serialize;
use serde_json::{Value, json};
use worker::*;

const STATUS_ORIGIN: &str = "https://status.mochios.org";
const STORE_ORIGIN: &str = "https://store.mochios.org";
const CONSOLE_ORIGIN: &str = "https://console.mochios.org";
const MAX_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;

fn now() -> i64 {
    (Date::now().as_millis() / 1000) as i64
}
fn id(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::now_v7().simple())
}
fn param<'a>(ctx: &'a RouteContext<()>, name: &str) -> &'a str {
    ctx.param(name).map(String::as_str).unwrap_or("")
}
fn db(ctx: &RouteContext<()>) -> Result<D1Database> {
    ctx.env.d1("DB")
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
    value.contains('.')
        && value.len() <= 255
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'-'))
}

fn valid_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+'))
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

fn valid_package_signature(public_key: &str, signature: &str, package_sha256: &str) -> bool {
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
    let Ok(hash) = hex::decode(package_sha256) else {
        return false;
    };
    key.verify_strict(&hash, &signature).is_ok()
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
        Some(id) => Ok(id),
        None => Err(error(
            "DEVELOPER_AUTH_REQUIRED",
            "Developer authentication required",
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

async fn health(_: Request, _: RouteContext<()>) -> Result<Response> {
    let mut response = json_response(&json!({"status":"ok","service":"app-store-api"}), 200)?;
    response.headers_mut().set("Cache-Control", "no-store")?;
    Ok(response)
}

async fn list_apps(req: Request, ctx: RouteContext<()>) -> Result<Response> {
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

async fn storefront(_: Request, ctx: RouteContext<()>) -> Result<Response> {
    json_response(&store::storefront(&db(&ctx)?).await?, 200)
}

async fn app_detail(_: Request, ctx: RouteContext<()>) -> Result<Response> {
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

async fn app_releases(_: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bundle_id = param(&ctx, "bundle_id");
    if store::public_app(&db(&ctx)?, bundle_id).await?.is_none() {
        return error("APP_NOT_FOUND", "App not found", 404);
    }
    json_response(
        &json!({"bundle_id":bundle_id,"releases":store::public_releases(&db(&ctx)?, bundle_id).await?}),
        200,
    )
}

async fn download(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let bundle_id = param(&ctx, "bundle_id");
    let version = req
        .url()?
        .query_pairs()
        .find(|(key, _)| key == "version")
        .map(|(_, value)| value.into_owned());
    let release: Option<Value> = store::first(&db(&ctx)?,
        "SELECT package_key,version,package_size,package_sha256 FROM releases WHERE bundle_id=?1 AND status='published' AND (?2 IS NULL OR version=?2) ORDER BY published_at DESC LIMIT 1",
        &[store::value(bundle_id), version.as_deref().map_or(worker::wasm_bindgen::JsValue::NULL, store::value)]).await?;
    let Some(release) = release else {
        return error("RELEASE_NOT_FOUND", "Release not found", 404);
    };
    let key = value_str(&release, "package_key").unwrap_or("");
    let Some(object) = ctx.env.bucket("PACKAGES")?.get(key).execute().await? else {
        return error("PACKAGE_NOT_FOUND", "Package object not found", 404);
    };
    let Some(body) = object.body() else {
        return error("PACKAGE_NOT_FOUND", "Package body not found", 404);
    };
    let headers = Headers::new();
    headers.set("Content-Type", "application/octet-stream")?;
    headers.set(
        "Content-Disposition",
        &format!(
            "attachment; filename=\"{}-{}.pkg\"",
            bundle_id,
            value_str(&release, "version").unwrap_or("release")
        ),
    )?;
    headers.set("Content-Length", &object.size().to_string())?;
    headers.set("ETag", &object.http_etag())?;
    headers.set("Cache-Control", "public, max-age=31536000, immutable")?;
    Ok(Response::from_body(body.response_body()?)?.with_headers(headers))
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
    let result = store::run(&db(&ctx)?, "INSERT INTO bundle_ids(bundle_id,developer_id,app_name,status,created_at) VALUES(?1,?2,?3,'reserved',?4)", &[store::value(input.bundle_id.trim()),store::value(&developer),store::value(input.app_name.trim()),store::value(now())]).await;
    if result.is_err() {
        return error("BUNDLE_ID_ALREADY_EXISTS", "Bundle ID already exists", 409);
    }
    store::audit(
        &db(&ctx)?,
        Some(&developer),
        "bundle.reserve",
        "bundle_id",
        input.bundle_id.trim(),
        json!({}),
        now(),
    )
    .await?;
    json_response(&json!({"bundle_id":input.bundle_id}), 201)
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
        || input.display_name.trim().is_empty()
        || !matches!(input.kind.as_str(), "app" | "game")
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
        "INSERT INTO apps(app_id,bundle_id,developer_id,display_name,subtitle,description,icon_url,category,kind,price_label,age_rating,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?12)",
        &[store::value(&app_id),store::value(input.bundle_id.trim()),store::value(&developer),store::value(input.display_name.trim()),input.subtitle.as_deref().map_or(worker::wasm_bindgen::JsValue::NULL,store::value),store::value(input.description.trim()),input.icon_url.as_deref().map_or(worker::wasm_bindgen::JsValue::NULL,store::value),input.category.as_deref().map_or(worker::wasm_bindgen::JsValue::NULL,store::value),store::value(&input.kind),store::value(&input.price_label),input.age_rating.as_deref().map_or(worker::wasm_bindgen::JsValue::NULL,store::value),store::value(timestamp)]).await;
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
    let developer = match require_developer(&req, &ctx.env).await? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let Some(app) = store::developer_app(&db(&ctx)?, &developer, param(&ctx, "bundle_id")).await?
    else {
        return error("APP_NOT_FOUND", "App not found", 404);
    };
    json_response(&json!({"app":app}), 200)
}

async fn create_release(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let developer = match require_developer(&req, &ctx.env).await? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let bundle_id = param(&ctx, "bundle_id").to_string();
    if store::developer_app(&db(&ctx)?, &developer, &bundle_id)
        .await?
        .is_none()
    {
        return error("APP_NOT_FOUND", "App not found", 404);
    }
    let input: ReleaseInput = req.json().await?;
    if !valid_version(input.version.trim())
        || input.package_size == 0
        || input.package_size > MAX_PACKAGE_BYTES
        || input.package_sha256.len() != 64
        || hex::decode(&input.package_sha256).is_err()
        || input.signature.trim().is_empty()
        || input.certificate_id.trim().is_empty()
    {
        return error("VALIDATION_ERROR", "Release metadata is invalid", 422);
    }
    let Some(public_key) =
        auth::certificate_public_key(&req, &ctx.env, input.certificate_id.trim(), &developer)
            .await?
    else {
        return error(
            "CERTIFICATE_INVALID",
            "An active certificate for this developer is required",
            403,
        );
    };
    if !valid_package_signature(&public_key, input.signature.trim(), &input.package_sha256) {
        return error(
            "SIGNATURE_INVALID",
            "Package SHA-256 signature is invalid",
            422,
        );
    }
    let release_id = id("rel");
    let package_key = format!("packages/{bundle_id}/{}/{}.pkg", input.version, release_id);
    let timestamp = now();
    let result=store::run(&db(&ctx)?,"INSERT INTO releases(release_id,bundle_id,version,package_key,package_size,package_sha256,manifest_hash,signature,certificate_id,changelog,status,created_at) VALUES(?1,?2,?3,?4,?5,lower(?6),?7,?8,?9,?10,'draft',?11)",&[store::value(&release_id),store::value(&bundle_id),store::value(input.version.trim()),store::value(&package_key),store::value(input.package_size as f64),store::value(&input.package_sha256),input.manifest_hash.as_deref().map_or(worker::wasm_bindgen::JsValue::NULL,store::value),store::value(input.signature.trim()),store::value(input.certificate_id.trim()),input.changelog.as_deref().map_or(worker::wasm_bindgen::JsValue::NULL,store::value),store::value(timestamp)]).await;
    if result.is_err() {
        return error(
            "RELEASE_ALREADY_EXISTS",
            "Release version already exists",
            409,
        );
    }
    store::audit(
        &db(&ctx)?,
        Some(&developer),
        "release.create",
        "release",
        &release_id,
        json!({"bundle_id":bundle_id,"version":input.version}),
        timestamp,
    )
    .await?;
    json_response(
        &json!({"release_id":release_id,"status":"draft","package_upload_url":format!("/v1/developer/releases/{release_id}/package")}),
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

async fn upload_package(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let developer = match require_developer(&req, &ctx.env).await? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let release_id = param(&ctx, "release_id");
    let Some(release) = store::owned_release(&db(&ctx)?, &developer, release_id).await? else {
        return error("RELEASE_NOT_FOUND", "Release not found", 404);
    };
    if value_str(&release, "status") != Some("draft") {
        return error(
            "INVALID_RELEASE_STATUS",
            "Only draft releases can receive a package",
            409,
        );
    }
    let expected_size = release
        .get("package_size")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let content_length = req
        .headers()
        .get("Content-Length")?
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    if content_length != expected_size {
        return error(
            "PACKAGE_SIZE_MISMATCH",
            "Content-Length does not match release metadata",
            422,
        );
    }
    let checksum = hex::decode(value_str(&release, "package_sha256").unwrap_or(""))
        .map_err(|e| Error::RustError(e.to_string()))?;
    let key = value_str(&release, "package_key").unwrap_or("").to_string();
    let stream = FixedLengthStream::wrap(req.stream()?, expected_size);
    let mut metadata = HashMap::new();
    metadata.insert("release_id".into(), release_id.into());
    metadata.insert(
        "bundle_id".into(),
        value_str(&release, "bundle_id").unwrap_or("").into(),
    );
    ctx.env
        .bucket("PACKAGES")?
        .put(&key, stream)
        .sha256(checksum)
        .custom_metadata(metadata)
        .execute()
        .await?;
    store::audit(
        &db(&ctx)?,
        Some(&developer),
        "release.package.upload",
        "release",
        release_id,
        json!({"size":expected_size}),
        now(),
    )
    .await?;
    json_response(&json!({"uploaded":true,"release_id":release_id}), 200)
}

async fn submit_release(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let developer = match require_developer(&req, &ctx.env).await? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let release_id = param(&ctx, "release_id");
    let Some(release) = store::owned_release(&db(&ctx)?, &developer, release_id).await? else {
        return error("RELEASE_NOT_FOUND", "Release not found", 404);
    };
    if !matches!(value_str(&release, "status"), Some("draft" | "rejected")) {
        return error(
            "INVALID_RELEASE_STATUS",
            "Only draft or rejected releases can be submitted",
            409,
        );
    }
    let key = value_str(&release, "package_key").unwrap_or("");
    let Some(object) = ctx.env.bucket("PACKAGES")?.head(key).await? else {
        return error("PACKAGE_NOT_FOUND", "Upload package before submitting", 409);
    };
    if object.size()
        != release
            .get("package_size")
            .and_then(Value::as_u64)
            .unwrap_or(0)
    {
        return error(
            "PACKAGE_SIZE_MISMATCH",
            "Stored package size is invalid",
            409,
        );
    }
    let timestamp = now();
    store::run(
        &db(&ctx)?,
        "UPDATE releases SET status='submitted',submitted_at=?1 WHERE release_id=?2",
        &[store::value(timestamp), store::value(release_id)],
    )
    .await?;
    store::audit(
        &db(&ctx)?,
        Some(&developer),
        "release.submit",
        "release",
        release_id,
        json!({}),
        timestamp,
    )
    .await?;
    json_response(
        &json!({"release":store::release_by_id(&db(&ctx)?,release_id).await?}),
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
    if !matches!(
        status.as_str(),
        "draft" | "submitted" | "published" | "rejected"
    ) {
        return error("VALIDATION_ERROR", "status is invalid", 422);
    }
    let (limit, offset) = page(&req);
    let rows:Vec<Value>=store::rows(&db(&ctx)?,"SELECT r.*,a.display_name,a.icon_url,a.description FROM releases r LEFT JOIN apps a ON a.bundle_id=r.bundle_id WHERE r.status=?1 ORDER BY r.submitted_at DESC,r.created_at DESC LIMIT ?2 OFFSET ?3",&[store::value(&status),store::value(limit),store::value(offset)]).await?;
    json_response(&json!({"admin":actor,"status":status,"releases":rows}), 200)
}

async fn approve_release(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let release_id = param(&ctx, "release_id");
    let Some(release) = store::release_by_id(&db(&ctx)?, release_id).await? else {
        return error("RELEASE_NOT_FOUND", "Release not found", 404);
    };
    if value_str(&release, "status") != Some("submitted") {
        return error(
            "INVALID_RELEASE_STATUS",
            "Only submitted releases can be approved",
            409,
        );
    }
    let timestamp = now();
    store::run(&db(&ctx)?,"UPDATE releases SET status='published',review_message=NULL,reviewed_at=?1,reviewed_by=?2,published_at=?1 WHERE release_id=?3",&[store::value(timestamp),store::value(&actor),store::value(release_id)]).await?;
    store::run(
        &db(&ctx)?,
        "UPDATE apps SET latest_version=?1,visibility='public',updated_at=?2 WHERE bundle_id=?3",
        &[
            store::value(value_str(&release, "version").unwrap_or("")),
            store::value(timestamp),
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
        json!({}),
        timestamp,
    )
    .await?;
    json_response(
        &json!({"release":store::release_by_id(&db(&ctx)?,release_id).await?}),
        200,
    )
}

async fn reject_release(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let release_id = param(&ctx, "release_id");
    let input: RejectInput = req.json().await?;
    if input.message.trim().is_empty() {
        return error("VALIDATION_ERROR", "message is required", 422);
    }
    let Some(release) = store::release_by_id(&db(&ctx)?, release_id).await? else {
        return error("RELEASE_NOT_FOUND", "Release not found", 404);
    };
    if value_str(&release, "status") != Some("submitted") {
        return error(
            "INVALID_RELEASE_STATUS",
            "Only submitted releases can be rejected",
            409,
        );
    }
    let timestamp = now();
    store::run(&db(&ctx)?,"UPDATE releases SET status='rejected',review_message=?1,reviewed_at=?2,reviewed_by=?3 WHERE release_id=?4",&[store::value(input.message.trim()),store::value(timestamp),store::value(&actor),store::value(release_id)]).await?;
    store::audit(
        &db(&ctx)?,
        Some(&actor),
        "release.reject",
        "release",
        release_id,
        json!({}),
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
    let key = value_str(&release, "package_key").unwrap_or("");
    let Some(object) = ctx.env.bucket("PACKAGES")?.get(key).execute().await? else {
        return error("PACKAGE_NOT_FOUND", "Package object not found", 404);
    };
    let Some(body) = object.body() else {
        return error("PACKAGE_NOT_FOUND", "Package body not found", 404);
    };
    let headers = Headers::new();
    headers.set("Content-Type", "application/octet-stream")?;
    headers.set(
        "Content-Disposition",
        &format!(
            "attachment; filename=\"{}-{}.pkg\"",
            value_str(&release, "bundle_id").unwrap_or("app"),
            value_str(&release, "version").unwrap_or("release")
        ),
    )?;
    headers.set("Content-Length", &object.size().to_string())?;
    headers.set("Cache-Control", "no-store")?;
    Ok(Response::from_body(body.response_body()?)?.with_headers(headers))
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
    let result=store::run(&db(&ctx)?,"INSERT INTO public_keys(key_id,developer_id,public_key,fingerprint,created_at) VALUES(?1,?2,?3,?4,?5)",&[store::value(input.key_id.trim()),store::value(&developer),store::value(input.public_key.trim()),store::value(input.fingerprint.trim()),store::value(now())]).await;
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
    store::run(&db(&ctx)?,"UPDATE public_keys SET revoked_at=?1 WHERE key_id=?2 AND developer_id=?3 AND revoked_at IS NULL",&[store::value(now()),store::value(key_id),store::value(&developer)]).await?;
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
            store::value(timestamp),
        ],
    )
    .await?;
    store::run(
        &db(&ctx)?,
        "INSERT INTO team_members(team_id,developer_id,role,joined_at) VALUES(?1,?2,'owner',?3)",
        &[
            store::value(&team_id),
            store::value(&developer),
            store::value(timestamp),
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
    store::run(&db(&ctx)?,"INSERT INTO team_members(team_id,developer_id,role,joined_at) VALUES(?1,?2,?3,?4) ON CONFLICT(team_id,developer_id) DO UPDATE SET role=excluded.role",&[store::value(team_id),store::value(&input.developer_id),store::value(&input.role),store::value(now())]).await?;
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
            store::value(now()),
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
        .get_async("/v1/apps/:bundle_id/download", download)
        .get_async("/v1/apps/:bundle_id", app_detail)
        .get_async("/v1/bundle-ids", bundle_ids)
        .post_async("/v1/bundle-ids", bundle_ids)
        .get_async("/v1/developer/apps", developer_apps)
        .post_async("/v1/developer/apps", developer_apps)
        .get_async("/v1/developer/apps/:bundle_id", developer_app)
        .post_async("/v1/developer/apps/:bundle_id/team", assign_team)
        .get_async(
            "/v1/developer/apps/:bundle_id/releases",
            list_developer_releases,
        )
        .post_async("/v1/developer/apps/:bundle_id/releases", create_release)
        .put_async("/v1/developer/releases/:release_id/package", upload_package)
        .post_async("/v1/developer/releases/:release_id/submit", submit_release)
        .get_async("/v1/admin/releases", admin_releases)
        .post_async("/v1/admin/releases/:release_id/approve", approve_release)
        .post_async("/v1/admin/releases/:release_id/reject", reject_release)
        .get_async("/v1/admin/releases/:release_id/download", admin_download)
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_bundle_ids() {
        assert!(valid_bundle_id("org.mochios.example"));
        assert!(!valid_bundle_id("Example"));
        assert!(!valid_bundle_id("../app"));
    }
    #[test]
    fn validates_versions() {
        assert!(valid_version("1.2.3-beta+1"));
        assert!(!valid_version("1/2"));
    }
    #[test]
    fn package_limit_is_bounded() {
        assert_eq!(MAX_PACKAGE_BYTES, 134_217_728);
    }
}
