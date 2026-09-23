use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordVerifier},
};
use axum::{
    Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{delete, get, post},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    env, io,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    net::TcpListener,
};
use tokio_util::io::ReaderStream;

const LOGIN: &str = include_str!("../static/login.html");
const DRIVE: &str = include_str!("../static/drive.html");
const JS: &str = include_str!("../static/app.js");
const CSS: &str = include_str!("../static/style.css");

#[derive(Clone)]
struct AppState {
    root: Arc<PathBuf>,
    user: Arc<String>,
    password: Arc<String>,
    password_hash: Arc<Option<String>>,
    sessions: Arc<Mutex<HashMap<String, u64>>>,
}

#[derive(Deserialize)]
struct PathQuery {
    path: Option<String>,
}

#[derive(Deserialize)]
struct SearchQuery {
    q: String,
}

#[derive(Deserialize)]
struct LoginReq {
    user: String,
    password: String,
}

#[derive(Deserialize)]
struct MkdirReq {
    path: String,
    name: String,
}

#[derive(Deserialize)]
struct RenameReq {
    path: String,
    to: String,
}

#[derive(Deserialize)]
struct RestoreReq {
    path: String,
}

#[derive(Deserialize)]
struct BatchReq {
    paths: Vec<String>,
    to: Option<String>,
}

#[derive(Serialize)]
struct Entry {
    name: String,
    path: String,
    dir: bool,
    size: u64,
    modified: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    original_path: Option<String>,
}

#[derive(Serialize)]
struct StorageInfo {
    total: u64,
    used: u64,
    free: u64,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let root = PathBuf::from(env::var("DRIVE_ROOT").unwrap_or_else(|_| "./drive".into()));
    fs::create_dir_all(&root).await?;

    let state = AppState {
        root: Arc::new(root.canonicalize()?),
        user: Arc::new(env::var("DRIVE_USER").unwrap_or_else(|_| "tegar".into())),
        password: Arc::new(env::var("DRIVE_PASSWORD").unwrap_or_default()),
        password_hash: Arc::new(env::var("DRIVE_PASSWORD_HASH").ok()),
        sessions: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = Router::new()
        .route("/", get(login_page))
        .route("/login", get(login_page))
        .route("/drive", get(drive_page))
        .route("/app.js", get(js))
        .route("/style.css", get(css))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
        .route("/api/list", get(list))
        .route("/api/search", get(search))
        .route("/api/storage", get(storage))
        .route("/api/mkdir", post(mkdir))
        .route("/api/rename", post(rename))
        .route("/api/move", post(move_items))
        .route("/api/copy", post(copy_items))
        .route("/api/delete", delete(remove))
        .route("/api/delete/bulk", post(remove_bulk))
        .route("/api/trash", get(trash))
        .route("/api/trash/restore", post(restore))
        .route("/api/trash/delete", delete(remove_forever))
        .route("/api/upload", post(upload))
        .route("/view", get(view))
        .route("/download", get(download))
        .layer(DefaultBodyLimit::disable())
        .with_state(state);

    let addr = env::var("DRIVE_ADDR").unwrap_or_else(|_| "0.0.0.0:8083".into());
    let listener = TcpListener::bind(&addr).await?;
    println!("tegar-drive listening on http://{addr}");
    axum::serve(listener, app).await
}

async fn login_page() -> Html<&'static str> {
    Html(LOGIN)
}

async fn drive_page() -> Html<&'static str> {
    Html(DRIVE)
}

async fn js() -> impl IntoResponse {
    typed(JS, "application/javascript")
}

async fn css() -> impl IntoResponse {
    typed(CSS, "text/css")
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginReq>,
) -> Result<Response, Response> {
    if req.user != state.user.as_str() || !verify_password(&state, &req.password) {
        return Err((StatusCode::UNAUTHORIZED, "login salah").into_response());
    }

    let token = session_token().map_err(err)?;
    let expires = now() + 60 * 60 * 24 * 30;
    state
        .sessions
        .lock()
        .map_err(err)?
        .insert(token.clone(), expires);

    let mut res = StatusCode::NO_CONTENT.into_response();
    res.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "td_session={token}; Path=/; Max-Age=2592000; HttpOnly; SameSite=Lax"
        ))
        .map_err(err)?,
    );
    Ok(res)
}

async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = cookie(&headers, "td_session") {
        if let Ok(mut sessions) = state.sessions.lock() {
            sessions.remove(token);
        }
    }

    let mut res = StatusCode::NO_CONTENT.into_response();
    res.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("td_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax"),
    );
    res
}

async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PathQuery>,
) -> Result<Json<Vec<Entry>>, Response> {
    auth(&state, &headers)?;
    let rel = q.path.unwrap_or_default();
    let dir = resolve_existing(&state.root, &rel).await?;
    let mut rd = fs::read_dir(&dir).await.map_err(err)?;
    let mut out = Vec::new();

    while let Some(item) = rd.next_entry().await.map_err(err)? {
        let meta = item.metadata().await.map_err(err)?;
        let name = item.file_name().to_string_lossy().to_string();
        if rel.is_empty() && name == ".trash" {
            continue;
        }
        let path = join_rel(&rel, &name);
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        out.push(Entry {
            name,
            path,
            dir: meta.is_dir(),
            size: meta.len(),
            modified,
            original_path: None,
        });
    }

    out.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.cmp(&b.name)));
    Ok(Json(out))
}

async fn storage(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<StorageInfo>, Response> {
    auth(&state, &headers)?;
    storage_info(&state.root).map(Json).map_err(err)
}

async fn search(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SearchQuery>,
) -> Result<Json<Vec<Entry>>, Response> {
    auth(&state, &headers)?;
    let needle = q.q.trim().to_ascii_lowercase();
    if needle.len() < 2 {
        return Ok(Json(Vec::new()));
    }
    search_entries(&state.root, &needle).map(Json).map_err(err)
}

async fn mkdir(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<MkdirReq>,
) -> Result<StatusCode, Response> {
    auth(&state, &headers)?;
    let name = clean_name(&req.name)?;
    let parent = resolve_existing(&state.root, &req.path).await?;
    fs::create_dir(parent.join(name)).await.map_err(err)?;
    Ok(StatusCode::CREATED)
}

async fn rename(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RenameReq>,
) -> Result<StatusCode, Response> {
    auth(&state, &headers)?;
    let from = resolve_existing(&state.root, &req.path).await?;
    let parent = Path::new(&req.path)
        .parent()
        .and_then(Path::to_str)
        .unwrap_or("");
    let to = resolve_existing(&state.root, parent)
        .await?
        .join(clean_name(&req.to)?);
    fs::rename(from, to).await.map_err(err)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn move_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<BatchReq>,
) -> Result<StatusCode, Response> {
    auth(&state, &headers)?;
    let dest = resolve_existing(&state.root, req.to.as_deref().unwrap_or("")).await?;
    for rel in req.paths {
        let from = resolve_existing(&state.root, &rel).await?;
        let name = from
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| (StatusCode::BAD_REQUEST, "path tidak valid").into_response())?;
        let to = unique_path(&dest, name).await?;
        if dest.starts_with(&from) {
            return Err((StatusCode::BAD_REQUEST, "tujuan tidak valid").into_response());
        }
        fs::rename(from, to).await.map_err(err)?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn copy_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<BatchReq>,
) -> Result<StatusCode, Response> {
    auth(&state, &headers)?;
    let dest = resolve_existing(&state.root, req.to.as_deref().unwrap_or("")).await?;
    for rel in req.paths {
        let from = resolve_existing(&state.root, &rel).await?;
        let name = from
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| (StatusCode::BAD_REQUEST, "path tidak valid").into_response())?
            .to_string();
        let to = unique_path(&dest, &name).await?;
        let from2 = from.clone();
        tokio::task::spawn_blocking(move || copy_path(&from2, &to))
            .await
            .map_err(err)?
            .map_err(err)?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PathQuery>,
) -> Result<StatusCode, Response> {
    auth(&state, &headers)?;
    move_to_trash(&state.root, q.path.as_deref().unwrap_or("")).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_bulk(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<BatchReq>,
) -> Result<StatusCode, Response> {
    auth(&state, &headers)?;
    for rel in req.paths {
        move_to_trash(&state.root, &rel).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn trash(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Entry>>, Response> {
    auth(&state, &headers)?;
    let trash_dir = state.root.join(".trash");
    fs::create_dir_all(trash_dir.join(".meta"))
        .await
        .map_err(err)?;
    let mut rd = fs::read_dir(&trash_dir).await.map_err(err)?;
    let mut out = Vec::new();

    while let Some(item) = rd.next_entry().await.map_err(err)? {
        let meta = item.metadata().await.map_err(err)?;
        let name = item.file_name().to_string_lossy().to_string();
        if name == ".meta" {
            continue;
        }
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        let original_path = fs::read_to_string(trash_dir.join(".meta").join(&name))
            .await
            .ok();
        out.push(Entry {
            name: display_trash_name(&name),
            path: name,
            dir: meta.is_dir(),
            size: meta.len(),
            modified,
            original_path,
        });
    }

    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(Json(out))
}

async fn restore(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RestoreReq>,
) -> Result<StatusCode, Response> {
    auth(&state, &headers)?;
    let name = clean_name(&req.path)?;
    let trash_dir = state.root.join(".trash");
    let from = trash_dir.join(&name);
    let original = fs::read_to_string(trash_dir.join(".meta").join(&name))
        .await
        .map_err(err)?;
    let parent = Path::new(&original)
        .parent()
        .and_then(Path::to_str)
        .unwrap_or("");
    let file_name = Path::new(&original)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "path tidak valid").into_response())?;
    let parent_dir = resolve_existing(&state.root, parent)
        .await
        .unwrap_or_else(|_| state.root.as_ref().clone());
    let to = unique_path(&parent_dir, file_name).await?;
    fs::rename(from, to).await.map_err(err)?;
    let _ = fs::remove_file(trash_dir.join(".meta").join(name)).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_forever(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PathQuery>,
) -> Result<StatusCode, Response> {
    auth(&state, &headers)?;
    let name = clean_name(q.path.as_deref().unwrap_or(""))?;
    let trash_dir = state.root.join(".trash");
    let target = trash_dir.join(&name);
    let meta = fs::metadata(&target).await.map_err(err)?;
    if meta.is_dir() {
        fs::remove_dir_all(target).await.map_err(err)?;
    } else {
        fs::remove_file(target).await.map_err(err)?;
    }
    let _ = fs::remove_file(trash_dir.join(".meta").join(name)).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PathQuery>,
    mut multipart: Multipart,
) -> Result<StatusCode, Response> {
    auth(&state, &headers)?;
    let dir = resolve_existing(&state.root, q.path.as_deref().unwrap_or("")).await?;

    while let Some(mut field) = multipart.next_field().await.map_err(err)? {
        let Some(name) = field.file_name().map(clean_name).transpose()? else {
            continue;
        };
        let final_path = unique_path(&dir, &name).await?;
        let temp_path = dir.join(format!(".{}.uploading-{}", name, now()));
        let mut file = fs::File::create(&temp_path).await.map_err(err)?;
        while let Some(chunk) = field.chunk().await.map_err(err)? {
            file.write_all(&chunk).await.map_err(err)?;
        }
        file.sync_all().await.map_err(err)?;
        drop(file);
        fs::rename(temp_path, final_path).await.map_err(err)?;
    }

    Ok(StatusCode::CREATED)
}

async fn download(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PathQuery>,
) -> Result<Response, Response> {
    auth(&state, &headers)?;
    let path = q.path.unwrap_or_default();
    let target = resolve_existing(&state.root, &path).await?;
    let file = fs::File::open(&target).await.map_err(err)?;
    let name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");
    let mut res = Body::from_stream(ReaderStream::new(file)).into_response();
    res.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename=\"{}\"",
            name.replace('"', "")
        ))
        .map_err(err)?,
    );
    Ok(res)
}

async fn view(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PathQuery>,
) -> Result<Response, Response> {
    auth(&state, &headers)?;
    let path = q.path.unwrap_or_default();
    let target = resolve_existing(&state.root, &path).await?;
    let mut file = fs::File::open(&target).await.map_err(err)?;
    let len = file.metadata().await.map_err(err)?.len();
    let content_type = content_type(&target);

    if let Some((start, end)) = parse_range(&headers, len) {
        file.seek(std::io::SeekFrom::Start(start))
            .await
            .map_err(err)?;
        let stream = ReaderStream::new(file.take(end - start + 1));
        let mut res = Body::from_stream(stream).into_response();
        *res.status_mut() = StatusCode::PARTIAL_CONTENT;
        res.headers_mut().insert(
            header::CONTENT_RANGE,
            HeaderValue::from_str(&format!("bytes {start}-{end}/{len}")).map_err(err)?,
        );
        res.headers_mut().insert(
            header::CONTENT_LENGTH,
            HeaderValue::from_str(&(end - start + 1).to_string()).map_err(err)?,
        );
        res.headers_mut()
            .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
        res.headers_mut()
            .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
        return Ok(res);
    }

    let mut res = Body::from_stream(ReaderStream::new(file)).into_response();
    res.headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    res.headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    res.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&len.to_string()).map_err(err)?,
    );
    Ok(res)
}

fn auth(state: &AppState, headers: &HeaderMap) -> Result<(), Response> {
    let Some(token) = cookie(headers, "td_session") else {
        return Err((StatusCode::UNAUTHORIZED, "login salah").into_response());
    };
    let mut sessions = state.sessions.lock().map_err(err)?;
    match sessions.get(token).copied() {
        Some(expires) if expires > now() => Ok(()),
        Some(_) => {
            sessions.remove(token);
            Err((StatusCode::UNAUTHORIZED, "session habis").into_response())
        }
        None => Err((StatusCode::UNAUTHORIZED, "login salah").into_response()),
    }
}

fn verify_password(state: &AppState, password: &str) -> bool {
    if let Some(hash) = state.password_hash.as_deref() {
        let Ok(parsed) = PasswordHash::new(hash) else {
            return false;
        };
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    } else {
        !state.password.is_empty() && password == state.password.as_str()
    }
}

fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|part| part.trim().split_once('='))
        .find_map(|(key, value)| (key == name).then_some(value))
}

fn session_token() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    #[cfg(target_family = "unix")]
    {
        use std::io::Read;
        std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    }
    #[cfg(not(target_family = "unix"))]
    {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        bytes[..16].copy_from_slice(&seed.to_le_bytes());
        bytes[16..].copy_from_slice(&(std::process::id() as u128).to_le_bytes());
    }
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

async fn resolve_existing(root: &Path, rel: &str) -> Result<PathBuf, Response> {
    let path = resolve(root, rel)?;
    let path = path.canonicalize().map_err(err)?;
    if path.starts_with(root) {
        Ok(path)
    } else {
        Err((StatusCode::BAD_REQUEST, "path tidak valid").into_response())
    }
}

async fn move_to_trash(root: &Path, rel: &str) -> Result<(), Response> {
    if rel.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "path tidak valid").into_response());
    }
    let target = resolve_existing(root, rel).await?;
    let trash_dir = root.join(".trash");
    let meta_dir = trash_dir.join(".meta");
    fs::create_dir_all(&meta_dir).await.map_err(err)?;
    let name = Path::new(rel)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");
    let trash_name = unique_trash_name(name);
    fs::write(meta_dir.join(&trash_name), rel)
        .await
        .map_err(err)?;
    fs::rename(target, trash_dir.join(trash_name))
        .await
        .map_err(err)?;
    Ok(())
}

fn resolve(root: &Path, rel: &str) -> Result<PathBuf, Response> {
    let mut out = root.to_path_buf();
    for part in Path::new(rel.trim_start_matches('/')).components() {
        match part {
            Component::Normal(p) => out.push(p),
            Component::CurDir => {}
            _ => return Err((StatusCode::BAD_REQUEST, "path tidak valid").into_response()),
        }
    }
    Ok(out)
}

fn clean_name(name: &str) -> Result<String, Response> {
    let name = name.trim();
    if name.is_empty() || name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        Err((StatusCode::BAD_REQUEST, "nama tidak valid").into_response())
    } else {
        Ok(name.to_string())
    }
}

fn join_rel(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", parent.trim_end_matches('/'), name)
    }
}

async fn unique_path(dir: &Path, name: &str) -> Result<PathBuf, Response> {
    let path = dir.join(name);
    if !path.try_exists().map_err(err)? {
        return Ok(path);
    }

    let (stem, ext) = split_name(name);
    for i in 1..10_000 {
        let candidate = if ext.is_empty() {
            dir.join(format!("{stem} ({i})"))
        } else {
            dir.join(format!("{stem} ({i}).{ext}"))
        };
        if !candidate.try_exists().map_err(err)? {
            return Ok(candidate);
        }
    }
    Err((StatusCode::CONFLICT, "terlalu banyak file dengan nama sama").into_response())
}

fn split_name(name: &str) -> (&str, &str) {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem, ext),
        _ => (name, ""),
    }
}

fn unique_trash_name(name: &str) -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{stamp}-{name}")
}

fn display_trash_name(name: &str) -> String {
    name.split_once('-')
        .map_or(name, |(_, rest)| rest)
        .to_string()
}

fn search_entries(root: &Path, needle: &str) -> io::Result<Vec<Entry>> {
    let mut out = Vec::new();
    search_dir(root, root, needle, &mut out)?;
    Ok(out)
}

fn search_dir(root: &Path, dir: &Path, needle: &str, out: &mut Vec<Entry>) -> io::Result<()> {
    if out.len() >= 200 {
        return Ok(());
    }
    for item in std::fs::read_dir(dir)? {
        let item = item?;
        let path = item.path();
        let name = item.file_name().to_string_lossy().to_string();
        if path == root.join(".trash") {
            continue;
        }
        let meta = item.metadata()?;
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if name.to_ascii_lowercase().contains(needle) {
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            out.push(Entry {
                name: name.clone(),
                path: rel,
                dir: meta.is_dir(),
                size: meta.len(),
                modified,
                original_path: None,
            });
        }
        if meta.is_dir() {
            let _ = search_dir(root, &path, needle, out);
        }
    }
    Ok(())
}

fn copy_path(from: &Path, to: &Path) -> io::Result<()> {
    let meta = std::fs::metadata(from)?;
    if meta.is_dir() {
        if to.starts_with(from) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "tidak bisa copy folder ke dalam dirinya sendiri",
            ));
        }
        std::fs::create_dir(to)?;
        for item in std::fs::read_dir(from)? {
            let item = item?;
            copy_path(&item.path(), &to.join(item.file_name()))?;
        }
    } else {
        std::fs::copy(from, to)?;
    }
    Ok(())
}

fn typed(body: &'static str, content_type: &'static str) -> impl IntoResponse {
    ([(header::CONTENT_TYPE, content_type)], body)
}

fn err<E: std::fmt::Display>(e: E) -> Response {
    (StatusCode::BAD_REQUEST, e.to_string()).into_response()
}

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "apng" => "image/apng",
        "avif" => "image/avif",
        "css" => "text/css; charset=utf-8",
        "csv" => "text/csv; charset=utf-8",
        "gif" => "image/gif",
        "htm" | "html" => "text/html; charset=utf-8",
        "jpeg" | "jpg" => "image/jpeg",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "log" | "md" | "rs" | "sh" | "toml" | "txt" | "xml" | "yaml" | "yml" => {
            "text/plain; charset=utf-8"
        }
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "ogg" => "audio/ogg",
        "pdf" => "application/pdf",
        "png" => "image/png",
        "svg" => "image/svg+xml",
        "webm" => "video/webm",
        "webp" => "image/webp",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    }
}

#[cfg(target_family = "unix")]
fn storage_info(root: &Path) -> io::Result<StorageInfo> {
    let c_path = std::ffi::CString::new(root.to_string_lossy().as_bytes())?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    let stat = unsafe { stat.assume_init() };
    let total = stat.f_blocks as u64 * stat.f_frsize as u64;
    let free = stat.f_bavail as u64 * stat.f_frsize as u64;
    Ok(StorageInfo {
        total,
        free,
        used: total.saturating_sub(free),
    })
}

#[cfg(not(target_family = "unix"))]
fn storage_info(_root: &Path) -> io::Result<StorageInfo> {
    Ok(StorageInfo {
        total: 0,
        used: 0,
        free: 0,
    })
}

fn parse_range(headers: &HeaderMap, len: u64) -> Option<(u64, u64)> {
    let range = headers.get(header::RANGE)?.to_str().ok()?;
    let range = range.strip_prefix("bytes=")?;
    let (start, end) = range.split_once('-')?;
    let start = start.parse::<u64>().ok()?;
    let end = if end.is_empty() {
        len.saturating_sub(1)
    } else {
        end.parse::<u64>().ok()?.min(len.saturating_sub(1))
    };
    (start <= end && end < len).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_parent_paths() {
        assert!(resolve(Path::new("/tmp/root"), "../x").is_err());
        assert!(resolve(Path::new("/tmp/root"), "ok/../x").is_err());
        assert!(resolve(Path::new("/tmp/root"), "ok/file.txt").is_ok());
    }
}
