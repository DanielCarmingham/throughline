# Throughline

## Line

- [x] Workspace scaffold and the core model  ^qac
      → Workspace + tl crate. Id, Item, Child, Marker, ItemState, Entry, Line, Ref, Position. No Done variant — completion is position. 4 tests.
- [x] Ordering operations  ^ycg
      → insert/move_entry/advance/complete/drop_item + OpError. move_entry resolves the destination before removal to avoid an index shift. 11 tests.
- [x] Parse `line.md`  ^3xu
      → Line-numbered ParseError; ids, markers, Now, bodies, children, results, @metadata; ascii accepted on read. Mutation-verified: breaking result capture and duplicate-id rejection failed exactly the right tests. 12 tests.
- [x] Serialize and normalize  ^umx
      → render() with derived checkboxes and canonical unicode. Round-trip and idempotence hold. Hand-moving an item across NOW is a valid way to complete it. 8 tests.
- [x] Config and atomic file I/O  ^gzy
      → Config 3/7/3 + [check].allow_markers; walk-up find_line_file; temp+rename atomic write; parse errors carry path:line. 9 tests.
- [x] Views — now, window, slice  ^khs
      → Span, window(), slice(), at_now(), distance_from_now(). Window counts ITEMS not entries, so markers never consume budget. 9 tests.
- [x] Glyph sets  ^2sh
      → 18 roles x 3 modes, Codicon codepoints from glyphnames.json v3.5.0. Non-TTY forces ascii. 9 tests.
- [x] Theme tokens and palettes  ^inn
      → 13 tokens, dark/light, truecolor->256->16->none, sgr() for plain stdout. Carries the lint that forbids naming a colour outside theme/. 10 tests.
- [x] Method lints (`tl check`)  ^3sy
      → 8 lints with warning/error tiers; warnings exit 0. Results excluded from false-certainty because history may be as detailed as it likes. 14 tests.
- [x] CLI read commands  ^sr1
      → line/now/window/slice with --json and non-TTY degradation. FOUND A BUG by running it: --glyphs was ignored whenever stdout was not a TTY, so 'tl line --glyphs unicode > out.txt' silently produced ascii. An explicit flag is an instruction, not a preference. 10 tests.
- [x] CLI write commands and `tl check`  ^v7s
      → 10 write verbs, jj change-ID preference for --commit auto, and the Inquiry prompt. Suppressing the prompt for piped callers turned out to defeat its purpose — an agent shelling out is exactly who should be asked. 17 tests.
- [x] The ribbon  ^l2p
      → Pure Segment builder, window bracket, Now-preserving elision. The width budget was a constant that undercounted what elision adds, so it overflowed by a character; it now measures the assembled output. Ascii bracket changed from [ ] to { } because it was invisible against [x]. 9 tests.
- [x] Window list, app state, and keymap  ^pop
      → Two-zoom-level screen: ribbon + window list + status bar. Cursor is a view position and never writes. Draw tested from 10x3 to 200x60. 28 tests.
- [x] `tl init`, `tl doctor`, `tl plan`  ^0wk
      → init writes the practice summary and the AGENTS.md stanza that teaches an agent the vocabulary; a test guards against the old framing returning. doctor shows three glyph rows so you pick by looking. 8 tests.
- [x] Generated diagrams  ^885
      → Seven diagrams from the ribbon code. Labels are computed from the rendered output — the first attempt put NOW fourteen columns from the marker it labelled. 4 tests.
- [x] `docs/method.md`  ^s1x
      → Sixteen sections. A test asserts every generated diagram appears verbatim, verified by editing one and watching the suite fail with the regeneration command. 4 tests.
- [-] decide what comes first  ^aaa  @dropped(superseded by the seeded plan)
      Placement is the thinking; put it where it belongs.
- [x] Dogfood  ^r2t  @commit(xyltnlwnoork)
      Seed the line from the plan, record every result, and verify tl check passes on its own line.
      → Seeded 17 items from the plan, recorded every result, and the line passes its own lints. The three unsharpened warnings were correct: with 4 items ahead and a window of 7, a short line has no far future.

◆ v0.1 — tlflow manages its own line ◆

- [x] MCP server over stdio  ^5pw  @commit(olormkwqzmoq)
      Typed tools for agents instead of parsing CLI output. Keep model/ and view/ free of I/O so this stays a thin addition.
      → Seven MCP tools over stdio, verified against real JSON-RPC. Two defects the unit tests could not see: the server introduced itself as rmcp (from_build_env expands CARGO_CRATE_NAME inside the SDK), and instructions came through empty because the const was never wired up. Async runtime costs 4.8MB release.
- [x] rename the binary to tlflow  ^paa  @commit(olormkwqzmoq)
      → tl was shadowed by oh-my-zsh's tmux plugin (alias tl='tmux list-sessions') and the crate name is taken on crates.io by a package with 4M downloads, so it was never publishable. Package is now throughline, binary tlflow. Results recorded before this point still say tl — that is what the tool was called at the time, and history is not rewritten to match the present.
- [x] user-authored themes  ^a67  @commit(vvokmtvztmvt)
      TOML theme files overriding the 13 semantic tokens, with a base to inherit from. Must reject unknown token names — a typo that silently does nothing is worse than an error.
      → TOML themes over a dark/light base, overriding only named tokens; unknown names rejected. The token layer made it a drop-in as the spec predicted. Found and fixed --color always producing no colour when piped — the same bug class as --glyphs, which I had fixed without checking for its twin.
- [x] publish tlflow as a released binary  ^hky
      Cross-compiled binaries per platform, plus an install path that is not cargo build.
- [x] reframe Throughline around learning continuity  ^reb
      Documentation-only reframing: Throughline as a business line connecting intentions, events, evidence, outcomes, and learning. Keep accountability non-punitive: learning must be carried forward into action; tooling can follow after the idea settles.
      → Documentation reframed Throughline around learning continuity: a business line of intentions, events, evidence, outcomes, and accountability as learning carried into action. Tooling deliberately left behind the idea so it can catch up after the model settles.

── NOW ──

- [ ] design event/evidence model for tlflow  ^8h5
      Figure out the smallest model change that lets tlflow represent intentions, events, evidence, outcomes, exclusions, and unconnected threads without turning reconciliation into punitive metrics. Start from documentation language before changing file format or commands.
