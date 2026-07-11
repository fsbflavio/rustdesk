# Sync the fork with upstream (rustdesk/master)

Our fork = ~13 commits on top of `upstream/master`. We keep it that way by **rebasing** (not merging)
so the next sync stays simple. Only two upstream files carry our edits and can ever conflict:
`src/common.rs` (the `KEY` constant) and `libs/portable/src/main.rs` (`APP_PREFIX`). Everything else
is new files (`tools/`, `customize/`, the workflow, `custom.txt`) that upstream never touches.

## One-time setup

```bash
git remote add upstream https://github.com/rustdesk/rustdesk.git   # if not already
git config rerere.enabled true                # auto-replay conflict resolutions
git config fetch.recurseSubmodules on-demand  # silence the hbb_common fetch warning
```

## Each sync

```bash
git fetch upstream --no-recurse-submodules
git switch custom-build
git rebase upstream/master
# If it stops on a conflict (only ever KEY / APP_PREFIX): keep OUR value, then:
#   git add <file> && git rebase --continue
git submodule update --init --recursive       # match hbb_common to upstream's pin
git push --force-with-lease origin custom-build   # rebase rewrote history
```

The final push triggers the **ProDesk Windows x64** workflow → a fresh `ProDesk-portable.exe`
with upstream's changes baked in.

## Notes

- **No re-sign needed** for a plain upstream sync — `custom.txt` only changes when *you* change config.
- **Preview what's coming** before rebasing:
  ```bash
  BASE=$(git merge-base custom-build upstream/master)
  git log --oneline $BASE..upstream/master                 # new upstream commits
  git diff --name-only $BASE..upstream/master | grep -E "src/common.rs|libs/portable/src/main.rs" \
    && echo "CONFLICT RISK" || echo "conflict-free"
  ```
- **hbb_common** is used unforked, so `submodule update` just checks out upstream's pinned commit.
  (Only if you later fork it for self-update would you rebase the submodule too.)
- After a big upstream jump, sanity-check that line numbers referenced in `DISCOVERIES.md` still
  point at the right functions (they're a reference, not load-bearing).
