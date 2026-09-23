# Tegar Drive

File manager ringan untuk HDD STB.

## Run Lokal

```powershell
$env:DRIVE_ROOT="D:\github_project\tegar-drive\drive-test"
$env:DRIVE_ADDR="127.0.0.1:18083"
$env:DRIVE_USER="tegar"
$env:DRIVE_PASSWORD="change-me"
cargo run --release
```

Buka:

```text
http://127.0.0.1:18083/
```

## Fitur

- Login sederhana
- List file dan folder
- Buat folder
- Upload file
- Download file
- Rename
- Hapus file atau folder

## Target STB

Root data nanti:

```text
/mnt/hdd/drive
```

Port yang disarankan:

```text
8084
```

## Build GitHub

Push ke `main` akan menjalankan test dan build Linux ARM64 sebagai artifact.

Untuk membuat release yang bisa diambil STB:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

Artifact release:

```text
tegar-drive-linux-arm64.tar.gz
```

Install/update di STB:

```bash
curl -fsSL https://raw.githubusercontent.com/muhtegaralfikri/tegar-drive/main/scripts/install-stb.sh | DRIVE_PASSWORD='change-me' bash
```
