let cwd = "";
let entries = [];
let auth = JSON.parse(localStorage.getItem("drive-auth") || "null");

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
  pencil: '<svg viewBox="0 0 24 24"><path d="M12 20h9"/><path d="M16.5 3.5a2.12 2.12 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>',
  search: '<svg viewBox="0 0 24 24"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>',
  trash: '<svg viewBox="0 0 24 24"><path d="M3 6h18"/><path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6"/><path d="M14 11v6"/></svg>',
  upload: '<svg viewBox="0 0 24 24"><path d="M12 3v12"/><path d="m17 8-5-5-5 5"/><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/></svg>',
};

const $ = (id) => document.getElementById(id);
const icon = (name) => icons[name] || "";
const api = (path, opts = {}) =>
  fetch(path, {
    ...opts,
    headers: {
      "x-drive-user": auth?.user || "",
      "x-drive-password": auth?.password || "",
      ...(opts.headers || {}),
    },
  });

document.querySelectorAll("[data-icon]").forEach((el) => {
  el.innerHTML = icon(el.dataset.icon);
});

if ($("loginBtn")) initLogin();
if ($("files")) initDrive();

function initLogin() {
  $("togglePassword").innerHTML = icon("eye");
  $("togglePassword").onclick = () => {
    const visible = $("password").type === "text";
    $("password").type = visible ? "password" : "text";
    $("togglePassword").innerHTML = icon(visible ? "eye" : "eye-off");
  };
  $("loginBtn").onclick = login;
  $("password").onkeydown = (e) => {
    if (e.key === "Enter") $("loginBtn").click();
  };
  $("user").onkeydown = (e) => {
    if (e.key === "Enter") $("password").focus();
  };
}

async function login() {
  $("loginError").hidden = true;
  setLoginLoading(true);
  auth = { user: $("user").value.trim(), password: $("password").value.trim() };
  localStorage.setItem("drive-auth", JSON.stringify(auth));

  try {
    const res = await api("/api/list");
    if (res.status === 401) {
      throw new Error("User atau password salah.");
    }
    location.href = "/drive";
  } catch (err) {
    localStorage.removeItem("drive-auth");
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
  if (!auth) {
    location.href = "/login";
    return;
  }
  $("newFolderBtn").onclick = mkdir;
  $("newFolderBtn").innerHTML = `${icon("folder-plus")} <span>Folder</span>`;
  $("upBtn").innerHTML = `${icon("arrow-up")} <span>Naik</span>`;
  $("logoutBtn").innerHTML = icon("logout");
  $("logoutBtn").onclick = () => {
    localStorage.removeItem("drive-auth");
    location.href = "/login";
  };
  $("search").oninput = render;
  $("uploadInput").onchange = (e) => upload(e.target.files);
  $("upBtn").onclick = () => {
    cwd = cwd.split("/").slice(0, -1).join("/");
    load();
  };
  $("drop").ondragover = (e) => e.preventDefault();
  $("drop").ondrop = (e) => {
    e.preventDefault();
    upload(e.dataTransfer.files);
  };
  load();
}

async function load() {
  const res = await api(`/api/list?path=${encodeURIComponent(cwd)}`);
  if (res.status === 401) {
    localStorage.removeItem("drive-auth");
    location.href = "/login";
    return;
  }
  entries = await res.json();
  $("crumb").textContent = "/" + cwd;
  render();
}

function render() {
  const q = $("search").value.trim().toLowerCase();
  const shown = q ? entries.filter((f) => f.name.toLowerCase().includes(q)) : entries;
  $("files").replaceChildren(...shown.map(row));
}

function row(file) {
  const el = document.createElement("article");
  el.className = "file";
  el.innerHTML = `
    <button class="name">
      <span class="file-icon ${file.dir ? "dir" : "doc"}">${icon(file.dir ? "folder" : "file")}</span>
      <span>
        <strong>${escapeHtml(file.name)}</strong>
        <small>${file.dir ? "Folder" : size(file.size)}</small>
      </span>
    </button>
    <div class="actions">
      ${file.dir ? "" : `<button class="ghost" data-act="download" title="Download">${icon("download")}</button>`}
      <button class="ghost" data-act="rename" title="Rename">${icon("pencil")}</button>
      <button class="ghost danger" data-act="delete" title="Hapus">${icon("trash")}</button>
    </div>`;

  el.querySelector(".name").onclick = () => {
    if (file.dir) {
      cwd = file.path;
      load();
    } else {
      download(file);
    }
  };
  el.querySelector('[data-act="download"]')?.addEventListener("click", () => download(file));
  el.querySelector('[data-act="rename"]').onclick = () => rename(file);
  el.querySelector('[data-act="delete"]').onclick = () => remove(file);
  return el;
}

async function mkdir() {
  const name = prompt("Nama folder");
  if (!name) return;
  await api("/api/mkdir", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ path: cwd, name }),
  });
  load();
}

async function rename(file) {
  const to = prompt("Nama baru", file.name);
  if (!to || to === file.name) return;
  await api("/api/rename", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ path: file.path, to }),
  });
  load();
}

async function remove(file) {
  if (!confirm(`Hapus ${file.name}?`)) return;
  await api(`/api/delete?path=${encodeURIComponent(file.path)}`, { method: "DELETE" });
  load();
}

async function upload(files) {
  const form = new FormData();
  for (const file of files) form.append("file", file);
  await api(`/api/upload?path=${encodeURIComponent(cwd)}`, { method: "POST", body: form });
  load();
}

async function download(file) {
  const res = await api(`/download?path=${encodeURIComponent(file.path)}`);
  const blob = await res.blob();
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = file.name;
  a.click();
  URL.revokeObjectURL(url);
}

function escapeHtml(text) {
  return text.replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
}

function size(n) {
  if (n < 1024) return `${n} B`;
  if (n < 1048576) return `${(n / 1024).toFixed(1)} KB`;
  if (n < 1073741824) return `${(n / 1048576).toFixed(1)} MB`;
  return `${(n / 1073741824).toFixed(1)} GB`;
}
