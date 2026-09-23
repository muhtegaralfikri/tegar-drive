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
    env, io,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::UNIX_EPOCH,
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
}

#[derive(Deserialize)]
struct PathQuery {
    path: Option<String>,
    user: Option<String>,
    password: Option<String>,
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

#[derive(Serialize)]
struct Entry {
    name: String,
    path: String,
    dir: bool,
    size: u64,
    modified: u64,
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let root = PathBuf::from(env::var("DRIVE_ROOT").unwrap_or_else(|_| "./drive".into()));
    fs::create_dir_all(&root).await?;

    let state = AppState {
        root: Arc::new(root.canonicalize()?),
        user: Arc::new(env::var("DRIVE_USER").unwrap_or_else(|_| "tegar".into())),
        password: Arc::new(env::var("DRIVE_PASSWORD").unwrap_or_else(|_| "change-me".into())),
    };

    let app = Router::new()
        .route("/", get(login_page))
        .route("/login", get(login_page))
        .route("/drive", get(drive_page))
        .route("/app.js", get(js))
        .route("/style.css", get(css))
        .route("/api/list", get(list))
        .route("/api/mkdir", post(mkdir))
        .route("/api/rename", post(rename))
        .route("/api/delete", delete(remove))
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

async fn list(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PathQuery>,
) -> Result<Json<Vec<Entry>>, Response> {
    auth(&state, &headers)?;
    let rel = q.path.unwrap_or_default();
    let dir = resolve(&state.root, &rel)?;
    let mut rd = fs::read_dir(&dir).await.map_err(err)?;
    let mut out = Vec::new();

    while let Some(item) = rd.next_entry().await.map_err(err)? {
        let meta = item.metadata().await.map_err(err)?;
        let name = item.file_name().to_string_lossy().to_string();
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
        });
    }

    out.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| a.name.cmp(&b.name)));
    Ok(Json(out))
}

async fn mkdir(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<MkdirReq>,
) -> Result<StatusCode, Response> {
    auth(&state, &headers)?;
    let name = clean_name(&req.name)?;
    fs::create_dir(resolve(&state.root, &join_rel(&req.path, &name))?)
        .await
        .map_err(err)?;
    Ok(StatusCode::CREATED)
}

async fn rename(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<RenameReq>,
) -> Result<StatusCode, Response> {
    auth(&state, &headers)?;
    let from = resolve(&state.root, &req.path)?;
    let parent = Path::new(&req.path)
        .parent()
        .and_then(Path::to_str)
        .unwrap_or("");
    let to = resolve(&state.root, &join_rel(parent, &clean_name(&req.to)?))?;
    fs::rename(from, to).await.map_err(err)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PathQuery>,
) -> Result<StatusCode, Response> {
    auth(&state, &headers)?;
    let target = resolve(&state.root, q.path.as_deref().unwrap_or(""))?;
    let meta = fs::metadata(&target).await.map_err(err)?;
    if meta.is_dir() {
        fs::remove_dir_all(target).await.map_err(err)?;
    } else {
        fs::remove_file(target).await.map_err(err)?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PathQuery>,
    mut multipart: Multipart,
) -> Result<StatusCode, Response> {
    auth(&state, &headers)?;
    let dir = resolve(&state.root, q.path.as_deref().unwrap_or(""))?;

    while let Some(mut field) = multipart.next_field().await.map_err(err)? {
        let Some(name) = field.file_name().map(clean_name).transpose()? else {
            continue;
        };
        let mut file = fs::File::create(dir.join(name)).await.map_err(err)?;
        while let Some(chunk) = field.chunk().await.map_err(err)? {
            file.write_all(&chunk).await.map_err(err)?;
        }
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
    let target = resolve(&state.root, &path)?;
    let file = fs::File::open(&target).await.map_err(err)?;
    let name = target.file_name().and_then(|n| n.to_str()).unwrap_or("download");
    let mut res = Body::from_stream(ReaderStream::new(file)).into_response();
    res.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{}\"", name.replace('"', "")))
            .map_err(err)?,
    );
    Ok(res)
}

async fn view(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<PathQuery>,
) -> Result<Response, Response> {
    if q.user.as_deref() != Some(state.user.as_str())
        || q.password.as_deref() != Some(state.password.as_str())
    {
        auth(&state, &headers)?;
    }
    let path = q.path.unwrap_or_default();
    let target = resolve(&state.root, &path)?;
    let mut file = fs::File::open(&target).await.map_err(err)?;
    let len = file.metadata().await.map_err(err)?.len();
    let content_type = content_type(&target);

    if let Some((start, end)) = parse_range(&headers, len) {
        file.seek(std::io::SeekFrom::Start(start)).await.map_err(err)?;
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
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(content_type),
    );
    res.headers_mut()
        .insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    res.headers_mut().insert(
        header::CONTENT_LENGTH,
        HeaderValue::from_str(&len.to_string()).map_err(err)?,
    );
    Ok(res)
}

fn auth(state: &AppState, headers: &HeaderMap) -> Result<(), Response> {
    let user = headers
        .get("x-drive-user")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let password = headers
        .get("x-drive-password")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if user == state.user.as_str() && password == state.password.as_str() {
        Ok(())
    } else {
        Err((StatusCode::UNAUTHORIZED, "login salah").into_response())
    }
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
