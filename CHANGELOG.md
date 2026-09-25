# Changelog

## [1.1.0](https://github.com/schemaitat/pinst/compare/v1.0.0...v1.1.0) (2026-09-23)


### Features

* **agents:** add end-to-end feature implementation skill ([72a5ba3](https://github.com/schemaitat/pinst/commit/72a5ba3e848d22bea23ba2a87f054d5994f5871a))
* **agents:** add harness-agnostic Herdr orchestrate skill ([#22](https://github.com/schemaitat/pinst/issues/22)) ([98a5077](https://github.com/schemaitat/pinst/commit/98a5077eab553127a24d05c378b76e20588b992d))
* **harness:** adopt standard skill eval manifests ([c72d086](https://github.com/schemaitat/pinst/commit/c72d086ec14f52b38725f7341bd94366102349b5))
* **harness:** manage drift and skill assets ([5494b25](https://github.com/schemaitat/pinst/commit/5494b25f2a9a52221e1efe051380bfe6f6b29301))

## [1.0.0](https://github.com/schemaitat/pinst/compare/v0.4.0...v1.0.0) (2026-09-21)


### ⚠ BREAKING CHANGES

* **harness:** scripts/agents-wire.sh no longer exists. Anything that called it directly (a personal script, a forked CI config) should call `pinst harness install`/`pinst harness check` instead.
* **harness:** scripts/ash.sh is gone. Use `pinst harness check|index|skills|new-id`. Anything invoking the script directly needs updating; the finding ids, exit codes and JSON envelope are unchanged.

### Features

* add macOS support ([#19](https://github.com/schemaitat/pinst/issues/19)) ([5a1b28c](https://github.com/schemaitat/pinst/commit/5a1b28cc22b9e6b49ed861742f010962e2b9d8e1))
* **agents:** distil the corpus on a schedule, into a pull request ([#14](https://github.com/schemaitat/pinst/issues/14)) ([d0c63ee](https://github.com/schemaitat/pinst/commit/d0c63ee7a0cc073147856cf60711e1e42f3a7ad5))
* **agents:** make the harness measure itself ([#11](https://github.com/schemaitat/pinst/issues/11)) ([0cc43ca](https://github.com/schemaitat/pinst/commit/0cc43ca7c415cd3562304f87764984639a5465cf))
* **docs:** add a baked-in, searchable catalogue of tool usage ([#12](https://github.com/schemaitat/pinst/issues/12)) ([e96fac0](https://github.com/schemaitat/pinst/commit/e96fac06fece35b356e51fc7547bd115876f9b43))
* **harness:** install and manage the agent harness ([#20](https://github.com/schemaitat/pinst/issues/20)) ([9593185](https://github.com/schemaitat/pinst/commit/9593185e3d8e48d63f48d568d0a5e21a9bec5c8c))
* **harness:** mechanize the lesson-renumbering remedy ([#18](https://github.com/schemaitat/pinst/issues/18)) ([d452ed8](https://github.com/schemaitat/pinst/commit/d452ed816f1e19364eed01e4d14d7418820058d1))
* **harness:** move the agent harness into the pinst binary ([#13](https://github.com/schemaitat/pinst/issues/13)) ([2f4a111](https://github.com/schemaitat/pinst/commit/2f4a111643d64e5c59d02e96175feb4e4b3e69f5))


### Bug Fixes

* **ci:** make the scheduled distillation actually run ([#15](https://github.com/schemaitat/pinst/issues/15)) ([10b538b](https://github.com/schemaitat/pinst/commit/10b538bcf58e095068672ff08ea55638b6199780))
* stop release-please re-releasing the whole history ([#10](https://github.com/schemaitat/pinst/issues/10)) ([5fb3b3b](https://github.com/schemaitat/pinst/commit/5fb3b3b989afabaefa83628d95aa2271f29ad1c5))

## [0.4.0](https://github.com/schemaitat/pinst/compare/v0.3.0...v0.4.0) (2026-09-19)


### Features

* add pinst, a ratatui TUI for managing ~/dotfiles ([3bc5a4a](https://github.com/schemaitat/pinst/commit/3bc5a4a09bc447e94da173af462b07cd7a7f99e1))
* **agents:** add plan lifecycle and commit convention skills ([dafc3dd](https://github.com/schemaitat/pinst/commit/dafc3dd3a27b4b665ec9baf20a4a51901fa72af2))
* **agents:** make the agent harness loadable and self-checking ([#4](https://github.com/schemaitat/pinst/issues/4)) ([a44aa9b](https://github.com/schemaitat/pinst/commit/a44aa9b3f3a4bb20dcdb53119968648d4df97c4a))
* install pinst from its own releases ([c6eac15](https://github.com/schemaitat/pinst/commit/c6eac1507a62be7fc69c9bf81d4288b53eea3219))
* make pinst a self-contained, manifest-driven toolchain CLI ([a3496ef](https://github.com/schemaitat/pinst/commit/a3496ef264c235c92ba6e4e26599b77c35da270f))


### Bug Fixes

* address code review findings in the manifest-driven CLI ([766638d](https://github.com/schemaitat/pinst/commit/766638ddf1b7f6582142b67a7f2aff57aabb8fae))
* **configs:** point .zshenv's uv PATH entry at $HOME ([ff3736a](https://github.com/schemaitat/pinst/commit/ff3736a85f913bdcd9b742f80c71e017a02f77fa))
* **docs:** correct arrowhead rendering in the diagrams ([88dee58](https://github.com/schemaitat/pinst/commit/88dee588da11e0362bbb72ceaa9f8f67a4d759e6))


### Documentation

* add architecture and workflow diagrams ([4fe3e72](https://github.com/schemaitat/pinst/commit/4fe3e728c7f5f542b4c4666ea2d20d6835bc1edb))
* document managing tools and configs in the README ([b360a0a](https://github.com/schemaitat/pinst/commit/b360a0a495587ff95523e765dba4918a9f58e59e))
* **plans:** add a handover note for what is left of plan 0001 ([7d48984](https://github.com/schemaitat/pinst/commit/7d48984e9e664b73d23e973fe166d53e447117db))
* **plans:** close phase 2 and record what the first release run taught ([de1c790](https://github.com/schemaitat/pinst/commit/de1c790e495be46f4bc50fbba894205c00a8101c))
* **plans:** log the ci run and the private-repo finding ([0b85fb9](https://github.com/schemaitat/pinst/commit/0b85fb9743aacd90bf6858868485fc47dc5cefe3))
* **plans:** record plan 0001 for release automation ([09c4f99](https://github.com/schemaitat/pinst/commit/09c4f99c532bf9ace87106eb70b67cc972405757))
* **plans:** record the repo going public ([e7642a9](https://github.com/schemaitat/pinst/commit/e7642a97984f0106e3999520a32a5abe00ef3ca3))
* **plans:** write up learnings from plan 0001 ([2ceb233](https://github.com/schemaitat/pinst/commit/2ceb2334a44a2cd7a85235e12f059cb6ccb437d0))


### Build & Packaging

* package the release binary as a static musl tarball ([4e4cfef](https://github.com/schemaitat/pinst/commit/4e4cfefe81b7d1dfa8ed46e37860df265d94b097))

## 0.3.0 (2026-09-19)


### Features

* add pinst, a ratatui TUI for managing ~/dotfiles ([3bc5a4a](https://github.com/schemaitat/pinst/commit/3bc5a4a09bc447e94da173af462b07cd7a7f99e1))
* **agents:** add plan lifecycle and commit convention skills ([dafc3dd](https://github.com/schemaitat/pinst/commit/dafc3dd3a27b4b665ec9baf20a4a51901fa72af2))
* **agents:** make the agent harness loadable and self-checking ([#4](https://github.com/schemaitat/pinst/issues/4)) ([a44aa9b](https://github.com/schemaitat/pinst/commit/a44aa9b3f3a4bb20dcdb53119968648d4df97c4a))
* install pinst from its own releases ([c6eac15](https://github.com/schemaitat/pinst/commit/c6eac1507a62be7fc69c9bf81d4288b53eea3219))
* make pinst a self-contained, manifest-driven toolchain CLI ([a3496ef](https://github.com/schemaitat/pinst/commit/a3496ef264c235c92ba6e4e26599b77c35da270f))


### Bug Fixes

* address code review findings in the manifest-driven CLI ([766638d](https://github.com/schemaitat/pinst/commit/766638ddf1b7f6582142b67a7f2aff57aabb8fae))
* **configs:** point .zshenv's uv PATH entry at $HOME ([ff3736a](https://github.com/schemaitat/pinst/commit/ff3736a85f913bdcd9b742f80c71e017a02f77fa))
* **docs:** correct arrowhead rendering in the diagrams ([88dee58](https://github.com/schemaitat/pinst/commit/88dee588da11e0362bbb72ceaa9f8f67a4d759e6))


### Documentation

* add architecture and workflow diagrams ([4fe3e72](https://github.com/schemaitat/pinst/commit/4fe3e728c7f5f542b4c4666ea2d20d6835bc1edb))
* document managing tools and configs in the README ([b360a0a](https://github.com/schemaitat/pinst/commit/b360a0a495587ff95523e765dba4918a9f58e59e))
* **plans:** add a handover note for what is left of plan 0001 ([7d48984](https://github.com/schemaitat/pinst/commit/7d48984e9e664b73d23e973fe166d53e447117db))
* **plans:** close phase 2 and record what the first release run taught ([de1c790](https://github.com/schemaitat/pinst/commit/de1c790e495be46f4bc50fbba894205c00a8101c))
* **plans:** log the ci run and the private-repo finding ([0b85fb9](https://github.com/schemaitat/pinst/commit/0b85fb9743aacd90bf6858868485fc47dc5cefe3))
* **plans:** record plan 0001 for release automation ([09c4f99](https://github.com/schemaitat/pinst/commit/09c4f99c532bf9ace87106eb70b67cc972405757))
* **plans:** record the repo going public ([e7642a9](https://github.com/schemaitat/pinst/commit/e7642a97984f0106e3999520a32a5abe00ef3ca3))
* **plans:** write up learnings from plan 0001 ([2ceb233](https://github.com/schemaitat/pinst/commit/2ceb2334a44a2cd7a85235e12f059cb6ccb437d0))


### Build & Packaging

* package the release binary as a static musl tarball ([4e4cfef](https://github.com/schemaitat/pinst/commit/4e4cfefe81b7d1dfa8ed46e37860df265d94b097))
