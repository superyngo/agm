# Windows release build failure: `link_creates_missing_parent_of_nested_feature_dir`

Status: diagnosing (Bug-Fix Protocol step 1). Not yet fixed — waiting on real Windows repro output.

## Symptom

`v0.17.0` tag push triggered `.github/workflows/release.yml` → "Release Build". Both
`x86_64-pc-windows-msvc` and `i686-pc-windows-msvc` jobs failed `cargo test --test e2e` with:

```
thread 'link_creates_missing_parent_of_nested_feature_dir' panicked at tests\e2e.rs:141:5:
assertion failed: fs::symlink_metadata(h.join(".gemini/config/skills")).is_ok()
```

(The 32-bit job also failed a second, apparently unrelated test,
`link_migrates_existing_agents_instead_of_deleting`, with `agm.exe init` returning
`Error: The system cannot find the file specified. (os error 2)` — not investigated yet,
possibly flaky/unrelated; keep an eye on it after the main fix.)

CI run: https://github.com/superyngo/agm/actions/runs/44013045653 (tag `v0.17.0`).
Linux/macOS jobs and all other Windows e2e tests in the same run passed (5/6, 4/6 on 32-bit).

## The test (tests/e2e.rs:134-143)

```rust
#[test]
fn link_creates_missing_parent_of_nested_feature_dir() {
    let home = tempfile::tempdir().unwrap();
    let h = home.path();
    agm(h).arg("init").assert().success();
    // Antigravity CLI without the App: `~/.gemini` exists, `~/.gemini/config` does not.
    fs::create_dir_all(h.join(".gemini")).unwrap();
    agm(h).args(["tool", "link"]).assert().success();
    assert!(fs::symlink_metadata(h.join(".gemini/config/skills")).is_ok());
    assert!(fs::symlink_metadata(h.join(".gemini/config/agents")).is_ok());
}
```

It was added in `fix(config): resolve 2026-11-17 config audit findings` (00a7583) together with
the `do_link` parent-creation fix in `src/linker.rs`:

```rust
fn do_link(target: &Path, link_path: &Path, is_dir: bool) -> std::io::Result<()> {
    // Nested Feature dirs (e.g. `config/skills`) may have no parent yet.
    if let Some(parent) = link_path.parent() {
        fs::create_dir_all(parent)?;
    }
    ...
}
```

`agy` (Antigravity CLI, `src/config.rs:181-198`) is the only tool whose `skills_dir`/`agents_dir`
are nested (`"config/skills"`, `"config/agents"`) instead of a single path component — this is
the only tool that exercises this code path at all.

## Key fact established

`agm(h).args(["tool", "link"]).assert().success()` **passed** — the process exited 0. The
assertion failure happened only on the test's own `fs::symlink_metadata` check afterward. That
rules out `do_link`/`junction::create` returning an `Err` (that would propagate through
`link_tool` → `for_each_installed_tool` → `anyhow::bail!` → non-zero exit → `.assert().success()`
would panic with a *different* message showing the process's stdout/stderr, like the sibling
`link_migrates_existing_agents_instead_of_deleting` failure above does).

So the "skills" feature link for `agy` was **silently skipped** without error, most likely via a
`continue` in `link_tool` (`src/main.rs:247-279`):

```rust
let link = match tool.resolve_link(feature.key) {
    config::LinkPath::Path(p) => p,
    config::LinkPath::NotConfigured => continue,
    config::LinkPath::CollidesWithConfigDir(dir) => { println!("warn ..."); continue; }
    config::LinkPath::EscapesConfigDir(dir) => { println!("warn ..."); continue; }
};
```

Any `warn`/skip message would have gone to the **child process's** stdout, which `assert_cmd`
only surfaces in its own panic message when the command *fails*. Since the command succeeded,
that stdout was silently discarded by `cargo test` — we have no evidence of which branch fired.

## Hypotheses considered and NOT confirmed by static reading

Traced `resolve_link` (`src/config.rs:307-338`), `resolved_config_dir`/`resolve_path`
(`src/config.rs:259-273`), `paths::expand_tilde`/`is_absolute_like` (`src/paths.rs`), and
Windows's `junction::create` internals (crate `junction = "1.4.2"`, which does
`fs::create_dir(junction)` — note: **not** `create_dir_all`, requires the immediate parent to
already exist, which `do_link` ensures) — none of this reveals an obvious Windows-only branch
that would make `resolve_link("skills")` for `agy` return anything other than
`LinkPath::Path(...)`. In particular:

- `EscapesConfigDir` requires a `..` component in `"config/skills"` — there isn't one.
- `CollidesWithConfigDir` requires the canonicalized link path to equal the canonicalized
  `config_dir`. Since `.gemini/config` doesn't exist yet, the canonicalization falls back to an
  uncanonicalized `link_path.clone()`, which is extremely unlikely to equal the canonicalized
  (Windows-verbatim `\\?\...`) `config_dir`.
- `dirs::home_dir()` on Windows ignores the `HOME` env var in general (uses the Known Folder
  API), yet the test's sibling tests that *also* rely on `agm(h).env("HOME", home)` passed on
  Windows in the same CI run — so whatever makes `HOME` override work there should apply equally
  here. Not fully explained, but not the differentiator either since it would break all tests
  uniformly, not just this one.

None of the static analysis found a conclusive smoking gun — need a real repro.

## Next step: repro on real Windows (handed off to user)

Run on the Windows machine, from a clone of this repo at `v0.17.0` (or current `main`, same code):

```powershell
git clone git@github.com:superyngo/agm.git
cd agm
git checkout v0.17.0

cargo test --test e2e link_creates_missing_parent_of_nested_feature_dir -- --nocapture --test-threads=1
```

Paste the full output — look especially for any `warn` line (e.g. "Skipping skills: link path
resolves to config_dir ..." or "... leaves config_dir ... via `..`").

If that doesn't show the cause (e.g. output is still empty because `--nocapture` only affects the
*test* process's own prints, not necessarily the child `agm.exe`'s — `assert_cmd` buffers child
output internally regardless), reproduce manually to see real filesystem state:

```powershell
$h = (New-Item -ItemType Directory -Path "$env:TEMP\agm-diag-$(Get-Random)").FullName
$env:HOME = $h
cargo run -- init
New-Item -ItemType Directory -Path "$h\.gemini" -Force
cargo run -- tool link
Get-Item "$h\.gemini\config\skills" -ErrorAction SilentlyContinue
Get-Item "$h\.gemini\config" -ErrorAction SilentlyContinue
Get-ChildItem "$h\.gemini" -Recurse
```

Paste:
1. Full stdout of `cargo run -- tool link` (this is where the "warn"/skip message, if any, will
   show directly — no `assert_cmd` buffering since we're not going through the test harness).
2. The `Get-Item`/`Get-ChildItem` results.

With that, the root cause should be pinpointed (silently-skipped link vs. a `junction::create`
failure that still built the dir vs. something else entirely), and a surgical fix can follow the
Bug-Fix Protocol: report root cause + fix → wait for approval → patch → reproduce → confirm fixed
→ re-run the Windows CI job on a new tag/commit.

## Related, not yet investigated

`link_migrates_existing_agents_instead_of_deleting` failing only on `i686-pc-windows-msvc` with
`agm.exe init` → `os error 2` ("cannot find the file specified"). Could be a real 32-bit-only bug
or CI flakiness (e.g. antivirus/indexer lock, parallel test contention over `$TEMP`). Revisit
after the main nested-feature-dir issue is resolved; rerun the 32-bit job alone to see if it's
flaky.
