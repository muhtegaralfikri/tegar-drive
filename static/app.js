let cwd = "";
let entries = [];
let trashMode = false;
let selected = new Set();
let searchTimer = 0;

const icons = {
  "arrow-up": '<svg viewBox="0 0 24 24"><path d="m5 12 7-7 7 7"/><path d="M12 19V5"/></svg>',
  "cloud-upload": '<svg viewBox="0 0 24 24"><path d="M12 13v8"/><path d="m8 17 4-4 4 4"/><path d="M20.39 18.39A5 5 0 0 0 18 9h-1.26A8 8 0 1 0 3 16.3"/></svg>',
  download: '<svg viewBox="0 0 24 24"><path d="M12 15V3"/><path d="m7 10 5 5 5-5"/><path d="M5 21h14"/></svg>',
  eye: '<svg viewBox="0 0 24 24"><path d="M2.06 12.35a1 1 0 0 1 0-.7A11.8 11.8 0 0 1 12 5a11.8 11.8 0 0 1 9.94 6.65 1 1 0 0 1 0 .7A11.8 11.8 0 0 1 12 19a11.8 11.8 0 0 1-9.94-6.65Z"/><circle cx="12" cy="12" r="3"/></svg>',
  "eye-off": '<svg viewBox="0 0 24 24"><path d="m2 2 20 20"/><path d="M6.7 6.7A12.3 12.3 0 0 0 2.06 11.65a1 1 0 0 0 0 .7A11.8 11.8 0 0 0 12 19a10.8 10.8 0 0 0 4.2-.84"/><path d="M9.88 9.88A3 3 0 0 0 14.12 14.12"/><path d="M12 5a11.8 11.8 0 0 1 9.94 6.65 1 1 0 0 1 0 .7 12.5 12.5 0 0 1-2.01 2.9"/></svg>',
  file: '<svg viewBox="0 0 24 24"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/></svg>',
  folder: '<svg viewBox="0 0 24 24"><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/></svg>',
  "folder-plus": '<svg viewBox="0 0 24 24"><path d="M12 10v6"/><path d="M9 13h6"/><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/></svg>',
  "hard-drive": '<svg viewBox="0 0 24 24"><line x1="22" x2="2" y1="12" y2="12"/><path d="M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11Z"/><line x1="6" x2="6.01" y1="16" y2="16"/><line x1="10" x2="10.01" y1="16" y2="16"/></svg>',
  logout: '<svg viewBox="0 0 24 24"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><path d="m16 17 5-5-5-5"/><path d="M21 12H9"/></svg>',
  more: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/><circle cx="5" cy="12" r="1"/></svg>',
  pencil: '<svg viewBox="0 0 24 24"><path d="M12 20h9"/><path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>',
  search: '<svg viewBox="0 0 24 24"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>',
  trash: '<svg viewBox="0 0 24 24"><path d="M3 6h18"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/></svg>',
  upload: '<svg viewBox="0 0 24 24"><path d="M12 3v12"/><path d="m17 8-5-5-5 5"/><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/></svg>',
  x: '<svg viewBox="0 0 24 24"><path d="M18 6 6 18"/><path d="m6 6 12 12"/></svg>',
};

const $ = (id) => document.getElementById(id);
const api = (path, opts = {}) => fetch(path, opts);
const icon = (name) => icons[name] || "";

document.querySelectorAll("[data-icon]").forEach((el) => (el.innerHTML = icon(el.dataset.icon)));
if ($("loginBtn")) initLogin();
if ($("files")) initDrive();

function initLogin() {
  window.addEventListener("pageshow", () => setLoginLoading(false));
  $("togglePassword").innerHTML = icon("eye");
  $("togglePassword").onclick = () => {
    const visible = $("password").type === "text";
    $("password").type = visible ? "password" : "text";
    $("togglePassword").innerHTML = icon(visible ? "eye" : "eye-off");
  };
  $("loginBtn").onclick = login;
  $("user").onkeydown = (e) => e.key === "Enter" && $("password").focus();
  $("password").onkeydown = (e) => e.key === "Enter" && login();
}

async function login() {
  $("loginError").hidden = true;
  setLoginLoading(true);
  try {
    const res = await api("/api/login", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ user: $("user").value.trim(), password: $("password").value.trim() }),
    });
    if (!res.ok) throw new Error(res.status === 401 ? "User atau password salah." : "Gagal login.");
    location.href = "/drive";
  } catch (err) {
    $("loginError").textContent = err.message || "Gagal terhubung ke server.";
    $("loginError").hidden = false;
    setLoginLoading(false);
  }
}

function setLoginLoading(on) {
  $("loginBtn").disabled = on;
  document.querySelector(".spinner").hidden = !on;
  $("loginText").textContent = on ? "Memeriksa..." : "Masuk";
}

function initDrive() {
  $("upBtn").innerHTML = `${icon("arrow-up")} <span>Back</span>`;
  $("logoutBtn").innerHTML = icon("logout");
  $("closePreview").innerHTML = icon("x");
  $("filesBtn").onclick = goFiles;
  $("upBtn").onclick = up;
  $("newFolderBtn").onclick = mkdir;
  $("trashBtn").onclick = goTrash;
  $("logoutBtn").onclick = logout;
  $("search").oninput = queueSearch;
  $("sortBy").onchange = render;
  $("uploadInput").onchange = (e) => upload(e.target.files);
  $("bulkMove").onclick = () => moveItems([...selected]);
  $("bulkCopy").onclick = () => copyItems([...selected]);
  $("bulkDelete").onclick = () => deleteItems([...selected]);
  $("closePreview").onclick = () => $("previewDialog").close();
  $("previewDialog").addEventListener("close", () => $("previewBody").replaceChildren());
  $("drop").ondragover = (e) => e.preventDefault();
  $("drop").ondrop = (e) => {
    e.preventDefault();
    upload(e.dataTransfer.files);
  };
  load();
  loadStorage();
}

function up() {
  if (!cwd) return;
  cwd = cwd.split("/").slice(0, -1).join("/");
  load();
}

function goFiles() {
  trashMode = false;
  cwd = "";
  selected.clear();
  $("search").value = "";
  load();
}

function goTrash() {
  trashMode = true;
  cwd = "";
  selected.clear();
  $("search").value = "";
  load();
}

async function logout() {
  await api("/api/logout", { method: "POST" });
  location.href = "/login";
}

function queueSearch() {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(searchOrFilter, 250);
}

async function load() {
  selected.clear();
  const res = await api(trashMode ? "/api/trash" : `/api/list?path=${encodeURIComponent(cwd)}`);
  if (res.status === 401) return (location.href = "/login");
  entries = await res.json();
  $("topContext").textContent = trashMode ? "Trash" : "My Files";
  $("pageTitle").textContent = trashMode ? "Trash" : "My Files";
  $("pageHint").hidden = !trashMode;
  $("crumb").textContent = trashMode ? "Trash" : "/" + cwd;
  $("search").placeholder = trashMode ? "Search trash..." : "Search files...";
  $("upBtn").hidden = trashMode || !cwd;
  $("newMenu").hidden = trashMode;
  $("filesBtn").classList.toggle("primary", !trashMode);
  $("trashBtn").classList.toggle("primary", trashMode);
  document.body.classList.toggle("trash-mode", trashMode);
  render();
}

async function loadStorage() {
  const res = await api("/api/storage");
  if (!res.ok) return;
  const info = await res.json();
  const percent = info.total ? Math.round((info.used / info.total) * 100) : 0;
  $("storageUsed").textContent = `${percent}% used`;
  $("storageText").textContent = `${formatBytes(info.used)} of ${formatBytes(info.total)}`;
  $("storageProgress").value = percent;
}

async function searchOrFilter() {
  const q = $("search").value.trim();
  selected.clear();
  if (q.length >= 2 && !trashMode) {
    const res = await api(`/api/search?q=${encodeURIComponent(q)}`);
    if (res.ok) {
      entries = await res.json();
      $("crumb").textContent = `/Search: ${q}`;
      render();
    }
  } else if (q) {
    render();
  } else {
    load();
  }
}

function render() {
  const q = $("search").value.trim().toLowerCase();
  const shown = q && q.length < 2 ? entries.filter((f) => f.name.toLowerCase().includes(q)) : [...entries];
  shown.sort(compareFiles);
  $("files").replaceChildren(...(shown.length ? shown.map(row) : [emptyState()]));
  updateBulkBar();
}

function compareFiles(a, b) {
  const dir = Number(b.dir) - Number(a.dir);
  if (dir) return dir;
  const [by, dirName] = $("sortBy").value.split("-");
  const desc = dirName === "desc";
  let result = a.name.localeCompare(b.name, "id", { numeric: true, sensitivity: "base" });
  if (by === "modified") result = a.modified - b.modified;
  if (by === "size") result = a.size - b.size;
  if (by === "type") result = kind(a.name).localeCompare(kind(b.name), "id");
  return desc ? -result : result;
}

function row(file) {
  const el = document.createElement("article");
  el.className = "file";
  el.innerHTML = `
    <div class="name">
      <input class="pick" type="checkbox" ${selected.has(file.path) ? "checked" : ""} aria-label="Pilih ${escapeHtml(file.name)}" />
      <span class="file-icon ${file.dir ? "dir" : "doc"}">${icon(file.dir ? "folder" : "file")}</span>
      <span>
        <strong>${escapeHtml(file.name)}</strong>
        <small>${subtitle(file)}</small>
      </span>
    </div>
    <span class="file-size">${file.dir ? "-" : formatBytes(file.size)}</span>
    <span class="file-date">${date(file.modified)}</span>
    <details class="row-menu">
      <summary title="Menu">${icon("more")}</summary>
      <div class="menu-panel">
        ${trashMode ? `<button data-act="restore">${icon("arrow-up")} Restore</button>` : ""}
        ${file.dir || trashMode ? "" : `<button data-act="download">${icon("download")} Download</button>`}
        ${trashMode ? "" : `<button data-act="move">${icon("arrow-up")} Move</button><button data-act="copy">${icon("file")} Copy</button><button data-act="rename">${icon("pencil")} Rename</button>`}
        <button class="danger" data-act="delete">${icon("trash")} ${trashMode ? "Hapus permanen" : "Hapus"}</button>
      </div>
    </details>`;
  el.querySelector(".pick").onchange = (e) => {
    e.stopPropagation();
    e.target.checked ? selected.add(file.path) : selected.delete(file.path);
    updateBulkBar();
  };
  el.querySelector(".name").onclick = (e) => {
    if (e.target.classList.contains("pick") || trashMode) return;
    if (file.dir) {
      cwd = file.path;
      $("search").value = "";
      load();
    } else {
      preview(file);
    }
  };
  el.querySelector('[data-act="download"]')?.addEventListener("click", () => download(file));
  el.querySelector('[data-act="move"]')?.addEventListener("click", () => moveItems([file.path]));
  el.querySelector('[data-act="copy"]')?.addEventListener("click", () => copyItems([file.path]));
  el.querySelector('[data-act="rename"]')?.addEventListener("click", () => rename(file));
  el.querySelector('[data-act="restore"]')?.addEventListener("click", () => restore(file));
  el.querySelector('[data-act="delete"]').onclick = () => (trashMode ? removeForever(file) : remove(file));
  return el;
}

function updateBulkBar() {
  $("bulkBar").hidden = !selected.size || trashMode;
  $("selectedCount").textContent = `${selected.size} dipilih`;
}

async function mkdir() {
  const name = prompt("Nama folder");
  if (!name) return;
  await api("/api/mkdir", { method: "POST", headers: json(), body: JSON.stringify({ path: cwd, name }) });
  $("newMenu").removeAttribute("open");
  load();
}

async function rename(file) {
  const to = prompt("Nama baru", file.name);
  if (!to || to === file.name) return;
  await api("/api/rename", { method: "POST", headers: json(), body: JSON.stringify({ path: file.path, to }) });
  load();
}

async function moveItems(paths) {
  const to = prompt("Pindah ke folder path", "");
  if (to === null) return;
  await api("/api/move", { method: "POST", headers: json(), body: JSON.stringify({ paths, to }) });
  selected.clear();
  load();
}

async function copyItems(paths) {
  const to = prompt("Copy ke folder path", "");
  if (to === null) return;
  await api("/api/copy", { method: "POST", headers: json(), body: JSON.stringify({ paths, to }) });
  selected.clear();
  load();
  loadStorage();
}

async function deleteItems(paths) {
  if (!confirm(`Pindahkan ${paths.length} item ke Trash?`)) return;
  await api("/api/delete/bulk", { method: "POST", headers: json(), body: JSON.stringify({ paths }) });
  selected.clear();
  load();
  loadStorage();
}

async function remove(file) {
  if (!confirm(`Pindahkan ${file.name} ke Trash?`)) return;
  await api(`/api/delete?path=${encodeURIComponent(file.path)}`, { method: "DELETE" });
  load();
  loadStorage();
}

async function restore(file) {
  await api("/api/trash/restore", { method: "POST", headers: json(), body: JSON.stringify({ path: file.path }) });
  load();
  loadStorage();
}

async function removeForever(file) {
  if (!confirm(`Hapus permanen ${file.name}?`)) return;
  await api(`/api/trash/delete?path=${encodeURIComponent(file.path)}`, { method: "DELETE" });
  load();
  loadStorage();
}

function upload(files) {
  files = Array.from(files || []);
  if (!files.length || trashMode) return;
  $("newMenu").removeAttribute("open");
  const form = new FormData();
  files.forEach((file) => form.append("file", file));
  const xhr = new XMLHttpRequest();
  xhr.open("POST", `/api/upload?path=${encodeURIComponent(cwd)}`);
  showUpload(files, 0, "Menghubungkan...");
  xhr.upload.onprogress = (event) => {
    if (event.lengthComputable) {
      showUpload(files, Math.round((event.loaded / event.total) * 100), `${formatBytes(event.loaded)} / ${formatBytes(event.total)}`);
    }
  };
  xhr.onload = () => {
    if (xhr.status >= 200 && xhr.status < 300) {
      showUpload(files, 100, "Upload selesai.");
      $("uploadInput").value = "";
      load();
      loadStorage();
      setTimeout(hideUpload, 1200);
    } else {
      showUpload(files, 0, `Upload gagal: ${xhr.responseText || xhr.status}`);
    }
  };
  xhr.onerror = () => showUpload(files, 0, "Upload gagal: koneksi bermasalah.");
  xhr.send(form);
}

function download(file) {
  const a = document.createElement("a");
  a.href = `/download?path=${encodeURIComponent(file.path)}`;
  a.download = file.name;
  document.body.appendChild(a);
  a.click();
  a.remove();
}

async function preview(file) {
  $("previewTitle").textContent = file.name;
  $("previewMeta").innerHTML = `
    <div><dt>Tipe</dt><dd>${file.dir ? "Folder" : kind(file.name)}</dd></div>
    <div><dt>Ukuran</dt><dd>${file.dir ? "-" : formatBytes(file.size)}</dd></div>
    <div><dt>Diubah</dt><dd>${date(file.modified)}</dd></div>`;
  $("previewBody").replaceChildren(message("Memuat preview..."));
  $("previewDownload").onclick = () => download(file);
  $("previewDialog").showModal();

  const ext = file.name.split(".").pop().toLowerCase();
  const directUrl = `/view?${new URLSearchParams({ path: file.path })}`;
  let node;
  if (["jpg", "jpeg", "png", "gif", "webp", "svg", "avif", "apng"].includes(ext)) {
    node = document.createElement("img");
    node.src = directUrl;
  } else if (["mp4", "webm"].includes(ext)) {
    node = document.createElement("video");
    node.src = directUrl;
    node.controls = true;
    node.preload = "metadata";
  } else if (["mp3", "wav", "ogg"].includes(ext)) {
    node = document.createElement("audio");
    node.src = directUrl;
    node.controls = true;
    node.preload = "metadata";
  } else if (ext === "pdf") {
    node = document.createElement("iframe");
    node.src = directUrl;
  } else if (["css", "csv", "html", "js", "json", "log", "md", "mjs", "rs", "sh", "toml", "txt", "xml", "yaml", "yml"].includes(ext) && file.size <= 1048576) {
    node = document.createElement("pre");
    node.textContent = await (await api(directUrl)).text();
  } else {
    node = message("Preview belum tersedia untuk format ini. Gunakan tombol download.");
  }
  $("previewBody").replaceChildren(node);
}

function showUpload(files, percent, detail) {
  $("uploadPanel").hidden = false;
  $("uploadTitle").textContent = `Mengirim ${files.length} file`;
  $("uploadPercent").textContent = `${percent}%`;
  $("uploadProgress").value = percent;
  $("uploadDetail").textContent = detail;
}

function hideUpload() {
  $("uploadPanel").hidden = true;
}

function emptyState() {
  const node = document.createElement("div");
  node.className = "empty-state";
  node.innerHTML = trashMode
    ? "<strong>Trash is empty</strong><small>Deleted files will appear here.</small>"
    : "<strong>Folder is empty</strong><small>Use + New to upload files or create a folder.</small>";
  return node;
}

function subtitle(file) {
  if (trashMode && file.original_path) return escapeHtml(file.original_path);
  if (file.dir) return "Folder";
  return `${kind(file.name).replace(" file", "")} · ${formatBytes(file.size)}`;
}

function json() {
  return { "content-type": "application/json" };
}

function message(text) {
  const node = document.createElement("div");
  node.className = "empty-preview";
  node.textContent = text;
  return node;
}

function escapeHtml(text) {
  return text.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}

function formatBytes(n) {
  if (n < 1024) return `${n} B`;
  if (n < 1048576) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1073741824) return `${(n / 1048576).toFixed(1)} MB`;
  return `${(n / 1073741824).toFixed(1)} GB`;
}

function date(seconds) {
  if (!seconds) return "-";
  return new Intl.DateTimeFormat("id-ID", { dateStyle: "medium", timeStyle: "short" }).format(new Date(seconds * 1000));
}

function kind(name) {
  const ext = name.includes(".") ? name.split(".").pop().toUpperCase() : "FILE";
  return `${ext} file`;
}
