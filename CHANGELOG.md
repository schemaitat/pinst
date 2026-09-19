# Changelog

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
