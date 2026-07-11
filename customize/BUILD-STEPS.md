# Build steps: regenerate `custom.txt` and ship a new build

Runbook for producing a new ProDesk portable build after changing config
(server, app-name, `conn-type`, etc.). All commands run from the repo root in
**Git Bash**.

## Prerequisites (one-time)

- Signing key exists at `tools/custom-signer/custom-signer.key` (from
  `custom-signer keygen`). The matching **public** key is already committed in
  `src/common.rs` (`KEY` in `read_custom_client`). Keep the `.key` file backed
  up and out of git.
- Signer built once:
  ```bash
  (cd tools/custom-signer && cargo build --release)
  ```
  (Use `./target/release/custom-signer` below; `debug` also works.)

## 1. Edit your config

Edit `tools/custom-signer/prodesk-config.json` (git-ignored — your server/key
stay local):

```json
{
  "app-name": "ProDesk",
  "conn-type": "incoming",
  "disable-installation": "Y",
  "override-settings": {
    "custom-rendezvous-server": "...",
    "relay-server": "....",
    "api-server": "",
    "key": "GCCNqgqI..."
  }
}
```

- `conn-type`: `"incoming"` = receive-only, `"outgoing"` = send-only, omit = normal.
- `disable-installation`: `"Y"` = portable-only (no install button; also disables the
  rename-to-`*install.exe` trick). Omit to keep an install-capable build.
- `override-settings` = forced & never persisted. Use `default-settings` for changeable defaults.

## 2. Sign it into `custom.txt`

```bash
cd tools/custom-signer
./target/release/custom-signer sign prodesk-config.json custom-signer.key custom.txt
cd ../..
```

Optional — verify the embedded payload:
```bash
python -c "import base64; d=base64.standard_b64decode(open('tools/custom-signer/custom.txt','rb').read()); print(d[d.find(b'{'):].decode('utf-8','replace'))"
```

## 3. Put `custom.txt` at the repo root (CI embeds this one)

```bash
cp tools/custom-signer/custom.txt ./custom.txt
```

> The portable exe extracts to `%LOCALAPPDATA%\prodesk` and reads `custom.txt`
> from THERE, so it must be embedded in the bundle — the root `custom.txt` is what
> CI bakes into the exe.

## 4. Commit and push (triggers the build)

```bash
git add custom.txt
git commit -m "custom: update embedded config"
git push origin custom-build
```

Pushing `custom-build` runs the **ProDesk Windows x64** workflow
(`.github/workflows/prodesk-windows.yml`).

> Each push **cancels the previous in-progress run** — after pushing, let this run
> finish before pushing again.

## 5. Watch the build and grab the exe

```bash
# list recent runs
gh run list --repo fsbflavio/rustdesk --branch custom-build --limit 3

# follow the latest run to completion
gh run watch --repo fsbflavio/rustdesk $(gh run list --repo fsbflavio/rustdesk --branch custom-build --limit 1 --json databaseId --jq '.[0].databaseId')

# download the single portable exe artifact
gh run download --repo fsbflavio/rustdesk -n prodesk-portable-exe -D ./dist
```

Result: `./dist/ProDesk-portable.exe` — one self-extracting portable exe
(incoming-only, pointing at your server, isolated from official RustDesk).
Double-click to run portable. Note: with `disable-installation: Y` the rename-to-`*install.exe`
trick is disabled — for an installed/service version, build a **separate** variant without that flag.

## Notes

- Changing **app-name** also changes the config dir, IPC pipe, service name, and
  `is_installed()` — that's what lets it coexist with official RustDesk. If you
  change it, also update `APP_PREFIX` in `libs/portable/src/main.rs` (portable
  extraction dir) to match.
- Branding images (top logo, icons) are compile-time assets, not `custom.txt`:
  add `flutter/assets/logo.png` (≤300×60), `flutter/windows/runner/resources/app_icon.ico`,
  `res/tray-icon.ico`, `res/icon.*`.
- See `CUSTOMIZATION.md` for the full fork/customization reference.
