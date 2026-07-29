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
const MAX_SAFE_JS_INTEGER: u64 = 9_007_199_254_740_991;

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
    mochios_certificate::is_valid_package_id(value)
}

fn valid_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+'))
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
    let release: Option<Value> = store::first(
        &db(&ctx)?,
        "SELECT r.download_url FROM releases r JOIN bundle_ids b ON b.bundle_id=r.bundle_id
          WHERE r.bundle_id=?1 AND b.status='active' AND review_status='approved' AND publish_status='published'
            AND validation_status='valid' AND download_url IS NOT NULL
            AND sha256 IS NOT NULL AND signature IS NOT NULL
            AND (?2 IS NULL OR version=?2)
          ORDER BY published_at DESC LIMIT 1",
        &[
            store::value(bundle_id),
            version
                .as_deref()
                .map_or(worker::wasm_bindgen::JsValue::NULL, store::value),
        ],
    )
    .await?;
    let Some(release) = release else {
        return error("RELEASE_NOT_FOUND", "Release not found", 404);
    };
    redirect_to(
        value_str(&release, "download_url").unwrap_or(""),
        "public, max-age=300",
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
    let result = store::run(&db(&ctx)?, "INSERT INTO bundle_ids(bundle_id,developer_id,app_name,status,created_at) VALUES(?1,?2,?3,'reserved',?4)", &[store::value(input.bundle_id.trim()),store::value(&developer),store::value(input.app_name.trim()),store::number(now())]).await;
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
        &[store::value(&app_id),store::value(input.bundle_id.trim()),store::value(&developer),store::value(input.display_name.trim()),input.subtitle.as_deref().map_or(worker::wasm_bindgen::JsValue::NULL,store::value),store::value(input.description.trim()),input.icon_url.as_deref().map_or(worker::wasm_bindgen::JsValue::NULL,store::value),input.category.as_deref().map_or(worker::wasm_bindgen::JsValue::NULL,store::value),store::value(&input.kind),store::value(&input.price_label),input.age_rating.as_deref().map_or(worker::wasm_bindgen::JsValue::NULL,store::value),store::number(timestamp)]).await;
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
    let actor = match require_developer_actor(&req, &ctx.env).await? {
        Ok(v) => v,
        Err(r) => return Ok(r),
    };
    let developer = actor.developer_id.clone();
    let bundle_id = param(&ctx, "bundle_id").to_string();
    let active_app: Option<Value> = store::first(
        &db(&ctx)?,
        "SELECT a.app_id FROM apps a JOIN bundle_ids b ON b.bundle_id=a.bundle_id
         WHERE a.developer_id=?1 AND a.bundle_id=?2 AND b.status='active' LIMIT 1",
        &[store::value(&developer), store::value(&bundle_id)],
    )
    .await?;
    if active_app.is_none() {
        return error("APP_NOT_FOUND", "App not found", 404);
    }
    let input: ReleaseInput = req.json().await?;
    let Some((owner, repository)) = github_repository(input.repository.trim()) else {
        return error("VALIDATION_ERROR", "GitHub repository is invalid", 422);
    };
    if !valid_version(input.version.trim())
        || input.release_tag.trim().is_empty()
        || input.release_tag.eq_ignore_ascii_case("latest")
        || !input.asset.trim().ends_with(".mpkg")
        || input.certificate_id.trim().is_empty()
        || input.minimum_mochios_version.trim().is_empty()
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
    let lookup = GitHubReleaseAssetRequest {
        owner,
        repository,
        release_tag: input.release_tag.trim(),
        asset_name: input.asset.trim(),
    };
    let verified_asset = match auth::github_release_asset(&req, &ctx.env, &lookup).await? {
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
    let result = store::run(
        &db(&ctx)?,
        "INSERT INTO releases(
           release_id,bundle_id,version,github_repository_id,github_repository,
           github_release_id,github_release_tag,github_release_immutable,github_prerelease,
           github_asset_id,asset_name,download_url,file_size,github_digest,
           github_asset_created_at,github_asset_updated_at,developer_certificate_id,
           developer_public_key,developer_certificate_serial,developer_certificate_subject_key_id,
           developer_certificate_developer_id,developer_certificate_issuer_key_id,
           developer_certificate_issuer_public_key,developer_certificate_issuance_source,
           minimum_mochios_version,changelog,
           registered_by,registered_by_account_id,developer_display_name,created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30)",
        &[
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
            store::value(input.minimum_mochios_version.trim()),
            input
                .changelog
                .as_deref()
                .map_or(worker::wasm_bindgen::JsValue::NULL, store::value),
            store::value(&developer),
            store::value(&actor.account_id),
            store::value(&actor.display_name),
            store::number(timestamp),
        ],
    )
    .await;
    if result.is_err() {
        return error(
            "RELEASE_ALREADY_EXISTS",
            "Release version already exists",
            409,
        );
    }
    store::audit(
        &db(&ctx)?,
        Some(&actor.account_id),
        "release.create",
        "release",
        &release_id,
        json!({"developer_id":developer,"developer_role":actor.role,"bundle_id":bundle_id,"version":input.version,"github_asset_id":asset.asset_id}),
        timestamp,
    )
    .await?;
    json_response(
        &json!({"release_id":release_id,"validation_status":"pending","review_status":"pending","publish_status":"draft"}),
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

async fn admin_release(req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
    match store::release_by_id(&db(&ctx)?, param(&ctx, "release_id")).await? {
        Some(release) => json_response(&json!({"admin":actor,"release":release}), 200),
        None => error("RELEASE_NOT_FOUND", "Release not found", 404),
    }
}

async fn validate_release(mut req: Request, ctx: RouteContext<()>) -> Result<Response> {
    let actor = match require_admin(&req, &ctx.env)? {
        Ok(value) => value,
        Err(response) => return Ok(response),
    };
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
    let sha256 = input.sha256.to_ascii_lowercase();
    let manifest_hash = input.manifest_hash.to_ascii_lowercase();
    let expected_size = release
        .get("file_size")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if input.package_id != value_str(&release, "bundle_id").unwrap_or("")
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
        || input.minimum_mochios_version
            != value_str(&release, "minimum_mochios_version").unwrap_or("")
        || sha256.len() != 64
        || hex::decode(&sha256).is_err()
        || manifest_hash.len() != 64
        || hex::decode(&manifest_hash).is_err()
        || input.signature.trim().is_empty()
    {
        return error(
            "PACKAGE_VALIDATION_MISMATCH",
            "Validated .mpkg metadata does not match the registered release",
            422,
        );
    }
    if let Some(github_digest) = value_str(&release, "github_digest")
        && let Some(github_sha256) = github_digest.strip_prefix("sha256:")
        && !github_sha256.eq_ignore_ascii_case(&sha256)
    {
        return error(
            "GITHUB_DIGEST_MISMATCH",
            "GitHub asset digest does not match the reviewed .mpkg",
            422,
        );
    }
    let public_key = value_str(&release, "developer_public_key").unwrap_or("");
    if !valid_hash_signature(public_key, input.signature.trim(), &manifest_hash) {
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
    let timestamp = now();
    store::run(
        &db(&ctx)?,
        "UPDATE releases
            SET sha256=?1,manifest_hash=?2,signature=?3,validation_status='valid',
                review_status='submitted',validation_message=NULL,validated_at=?4,
                validated_by=?5,submitted_at=?4
          WHERE release_id=?6 AND validation_status='pending' AND review_status='pending'",
        &[
            store::value(&sha256),
            store::value(&manifest_hash),
            store::value(input.signature.trim()),
            store::number(timestamp),
            store::value(&actor),
            store::value(release_id),
        ],
    )
    .await?;
    store::audit(
        &db(&ctx)?,
        Some(&actor),
        "release.validate",
        "release",
        release_id,
        json!({"sha256":sha256,"file_size":input.file_size}),
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
    let (column, value) = match status.as_str() {
        "pending" | "submitted" | "approved" | "rejected" => ("review_status", status.as_str()),
        "draft" | "published" | "revoked" => ("publish_status", status.as_str()),
        _ => return error("VALIDATION_ERROR", "status is invalid", 422),
    };
    let (limit, offset) = page(&req);
    let sql = format!(
        "SELECT r.*,a.display_name,a.icon_url,a.description
           FROM releases r LEFT JOIN apps a ON a.bundle_id=r.bundle_id
          WHERE r.{column}=?1
          ORDER BY r.submitted_at DESC,r.created_at DESC LIMIT ?2 OFFSET ?3"
    );
    let rows: Vec<Value> = store::rows(
        &db(&ctx)?,
        &sql,
        &[
            store::value(value),
            store::number(limit),
            store::number(offset),
        ],
    )
    .await?;
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
    store::run(&db(&ctx)?,"UPDATE releases SET review_status='rejected',publish_status='draft',review_message=?1,reviewed_at=?2,reviewed_by=?3 WHERE release_id=?4",&[store::value(input.message.trim()),store::number(timestamp),store::value(&actor),store::value(release_id)]).await?;
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
        .get_async("/v1/admin/releases", admin_releases)
        .get_async("/v1/admin/releases/:release_id", admin_release)
        .post_async("/v1/admin/releases/:release_id/validate", validate_release)
        .post_async("/v1/admin/releases/:release_id/approve", approve_release)
        .post_async("/v1/admin/releases/:release_id/reject", reject_release)
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
}
