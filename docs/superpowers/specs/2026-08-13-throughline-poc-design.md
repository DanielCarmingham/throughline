# Throughline POC — Design

**Date:** 2026-08-13
**Status:** Approved for planning

## 1. Summary

Throughline is a project-management method in which a project is represented as
one ordered line running from past, through Now, into the future. This document
specifies a proof of concept: a written description of the method, and `tl`, a
combined CLI and TUI tool that manages a project's line.

The POC has three deliverables:

1. `docs/method.md` — the Throughline Method, described thoroughly.
2. `tl` — a single Rust binary providing both a CLI and a TUI.
3. `.throughline/line.md` — the line for building Throughline itself, so the
   tool manages its own construction from the first commit.

## 2. Background

The method originates from a sketch (`initial-sketch-chat.pdf`) that observes:
work happens over time, so organize it that way. Most project-management systems
organize work by state — backlog, ready, doing, blocked, done, someday, v2 —
which produces buckets that quietly mean "not now." Throughline makes sequence
the primary organizing principle instead.

The sketch establishes ten properties. Restated compactly:

1. **The project is a line.** Completed work becomes history rather than
   accumulating in a Done column. Future work sits ahead of the present in the
   order we currently believe it should happen.
2. **Work is cyclical, but progress is linear.** Plan → Do → Study → Adjust
   repeats, but each pass is unrolled onto the line rather than circling in
   place. The loop is how we learn; the line is how we move.
3. **The sliding window.** Attention is local. A window of focus moves forward
   along the line; the project is not reorganized when focus shifts.
4. **Any slice tells a story.** Look back and a slice says what happened; look
   ahead and it says what we expect; look around Now and it says where we are.
   The timeline is simultaneously the plan, the queue, and the record.
5. **Strict ordering.** Order is an explicit statement of intent, not a
   dependency graph. Changing it is planning, not a failure of planning.
6. **Hierarchy exists, but should be rare.** A parent is one unit of work whose
   parts must all be completed. If children can be independently prioritized,
   deferred, or evaluated, they belong on the line.
7. **Avoid temporal buckets.** If something should happen later, put it later.
8. **History is first-class.** Old work is not clutter; it is farther away.
9. **Planning is progressive.** Detail decreases with distance from Now.
   Rolling-wave planning becomes a property of the representation.
10. **The core model is small.** A line, work items, a Now position, a sliding
    window, limited hierarchy, and lightweight metadata. Everything else must
    justify its existence.

Naming, per the sketch: the method is **Throughline**; the core abstraction is
**the Line**; the current focus area is **the Window**.

## 3. Design principles added by this spec

The sketch describes the model. These three claims make it mechanical — things a
tool can enforce rather than merely encourage.

### 3.1 Status is position

"Done" is not a state; it is a location — behind Now. Completing an item means
moving it behind Now, which records that it happened before everything still
ahead of Now. There is no completion flag that can drift out of sync with the
ordering, because completion *is* the ordering.

Consequences:

- Finishing work out of order is expressed by moving that item behind Now. The
  line therefore remains a true chronological record rather than a plan with
  scattered checkmarks.
- Checkbox rendering in the file is derived from position, not authored.

### 3.2 Markers are landmarks, not buckets

`Deploy`, `v0.1`, and `Cycle 2` are zero-width entries occupying positions on
the line. "Post-go-live work" is expressed literally as *after the go-live
marker*. This turns sketch property 7 into a data type: there is no container to
put work in, only a place to put it.

`NOW` is the single reserved marker.

### 3.3 The Window is a view, not data

Now is stored. The Window is derived from wherever the user is looking. This is
what makes property 4 ("any slice tells a story") free rather than a feature: a
slice is just a different view over the same ordered data.

## 4. Architecture

### 4.1 Approach: file-first, tool-as-lens

`.throughline/line.md` is the source of truth. It is hand-editable, and `tl`
reads, normalizes, renders, and mutates it.

The rejected alternative is tool-first, where `tl` owns all writes and the file
is an export. That buys write safety but costs the property that makes this
useful to coding agents: an agent can read one file and hold the entire project
— plan, queue, and history — in a single read. This is not merely convenient; it
is the method's own claim that plan, queue, and record are the same artifact,
made literal.

Consequences of file-first:

- `tl` must tolerate hand-edited files, including malformed ones, and report
  parse problems with line numbers rather than failing opaquely.
- `tl fmt` normalizes derived content (checkboxes) so hand-edits converge.
- Writes are atomic: write to a temporary file in the same directory, then
  rename.

### 4.2 Stack

Rust, with:

| crate | purpose |
|---|---|
| `clap` | CLI parsing (derive API) |
| `ratatui` + `crossterm` | TUI rendering and terminal control |
| `serde` / `serde_json` | `--json` output |
| `terminal-light` | OSC 11 background-colour query for theme detection |
| `tui-textarea` | inline editing in the TUI |
| `anyhow` / `thiserror` | error handling |

Rust over Go was chosen for its fit with the bespoke line/window rendering
(ratatui's immediate-mode model suits custom drawing better than a stock widget
set that this project would largely bypass) and for serde. Both produce a single
static binary with instant startup, which is the property that matters for
agent shell-outs.

### 4.3 Crate layout

Units are separated so each can be understood and tested independently.

```
tl/
├── src/
│   ├── main.rs           # entry: dispatch CLI vs TUI
│   ├── model/            # Line, Item, Marker, Now, ordering ops
│   ├── format/           # parse + serialize line.md; fmt normalization
│   ├── view/             # derived views: window, slice, now
│   ├── check/            # method lints
│   ├── cli/              # clap commands; text and JSON renderers
│   ├── theme/            # semantic tokens, dark/light palettes, detection
│   ├── glyphs/           # nerdfont/unicode/ascii sets, capability resolution
│   └── tui/              # ribbon, window list, keymap, app state
└── tests/
```

- `model` knows nothing about files or terminals.
- `format` is the only module that touches `line.md` syntax.
- `view` computes windows and slices from a `Line`; it has no I/O.
- `theme` and `glyphs` are pure lookups; `tui` consumes both and may not
  construct a colour or a literal glyph itself.

## 5. File format

### 5.1 Example

```markdown
# Throughline — Building Throughline

## Line

- [x] Sketch the method  ^k3f
- [x] Pick the name  ^m2a

── NOW ──

- [ ] Write docs/method.md  ^q1d
      The full method: line, Now, window, markers, hierarchy, detail.
- [ ] Parse and write line.md  ^r7e
- [ ] Build account recovery  ^x2d
      - [ ] Generate recovery token
      - [ ] Send recovery email
      - [ ] Expire used token

◆ v0.1 — tl renders the line ◆

- [ ] TUI ribbon + window list  ^t9a
- [ ] Multi-project support?  ^v6c
```

### 5.2 Rules

- **Items** are Markdown list entries. Order in the file is order on the line.
- **Ids** are Obsidian-style block references (`^k3f`): a caret followed by 3–8
  lowercase alphanumerics, appended to the title line. Ids are stable across
  reordering and are how the CLI and agents refer to items.
- **Now** is the line `── NOW ──`. Exactly one must exist; `tl fmt` inserts one
  at the top if missing.
- **Markers** are `◆ label ◆`.
- **The file syntax is fixed and independent of the viewer's glyph mode.** The
  canonical written form is always the unicode form above. The ascii forms
  `-- NOW --` and `<> label <>` are *accepted on read*, for hand-editing in
  constrained environments, but `tl fmt` always writes the canonical form.
  Rendering glyph mode must never influence the file, or the same line would
  churn between machines with different terminal capabilities.
- **Checkboxes are derived.** Items above Now serialize as `[x]`, below as
  `[ ]`. Dropped items serialize as `[-]`. A human who moves a line above Now
  has completed it; `tl fmt` corrects the checkbox.
- **Bodies** are indented continuation lines beneath the title. Presence of a
  body is what "sharpened" means — see 5.3.
- **Children** are indented checkbox lines beneath a parent. Children have no
  ids and therefore cannot be positioned on the line. This makes sketch property
  6 unrepresentable-if-violated rather than merely discouraged.
- **Exceptional metadata** is inline and rare: `@blocked(reason)`,
  `@dropped(reason)`, `@active`. No other status vocabulary exists.

### 5.3 Progressive detail requires no syntax

Detail level is derived: a bare title is coarse, a title with a body is
sharpened. Far-future items are naturally bare because nobody has written a body
for them yet.

`tl check` uses this: it warns when an item inside the Window is still bare
(needs sharpening before work starts) and when an item far from Now has grown a
large body (false certainty about distant work). No `resolution:` field is
introduced.

## 6. CLI surface

Verbs are the method's vocabulary. Every read command accepts `--json`.

| command | purpose |
|---|---|
| `tl` | launch the TUI |
| `tl line` | the whole line (zoom out) |
| `tl now` | what sits at Now |
| `tl window [--back N] [--ahead N]` | the current attention window |
| `tl slice <ref>..<ref>` | any span; back, forward, or around Now |
| `tl add "<title>" --after <ref>` | add an item; placement is required |
| `tl move <id> --after <ref>` | reorder — this is replanning |
| `tl advance [<id>]` | move Now forward past the next item, or past everything up to and including `<id>` |
| `tl done <id>` | complete an item out of order: move it to immediately behind Now |
| `tl drop <id> --why "<reason>"` | the other outcome |
| `tl mark "<label>" --after <ref>` | place a landmark |
| `tl split <id>` | promote children onto the line |
| `tl sharpen <id>` | add or edit an item's body |
| `tl check` | lint the line against the method |
| `tl fmt` | normalize derived content |
| `tl init` | create `.throughline/`, method summary, agent stanza |
| `tl doctor` | re-run glyph and theme capability detection |

`--after` and `--before` accept an id (`^k3f`), a marker label, or `now`.
`tl add` has no default placement: choosing where work goes is the thinking the
method asks for, and a default would reintroduce a bucket.

`advance` and `done` are both moves, not flags — consistent with 3.1. `advance`
moves the Now marker; `done` moves an item. Neither writes a completion field,
because there isn't one. `advance` records an outcome for each item it passes,
defaulting to done.

### 6.1 `tl check` lints

`tl check` is what makes the method enforceable rather than aspirational, and is
the primary agent-facing feature. It exits non-zero when any lint fires.

| lint | condition |
|---|---|
| `bucket` | a marker label matching bucket vocabulary (`backlog`, `someday`, `later`, `v2`, `post-launch`, `icebox`, `blocked`) |
| `unsharpened` | an item inside the Window with no body |
| `false-certainty` | an item outside the Window with a body longer than `far_body_lines` |
| `deep-hierarchy` | children nested more than one level |
| `orphan-parent` | a parent behind Now with incomplete children |
| `independent-children` | a parent whose children carry `@blocked`/`@active`, suggesting they are separately trackable and belong on the line |
| `duplicate-id` | two items sharing an id |
| `no-now` | missing or duplicated Now marker |

"Inside the Window" means within the default window span (`window_back` = 3,
`window_ahead` = 7) measured from Now, not from the user's scroll position —
lints must not depend on where someone happens to be looking. `far_body_lines`
defaults to 3. All three are configurable in `.throughline/config.toml`.

### 6.2 Agent integration

`tl init` writes:

- `THROUGHLINE.md` — a short summary of the method and the vocabulary.
- An `AGENTS.md` stanza describing the commands and the discipline, so coding
  agents adopt the vocabulary without being instructed each session.

When stdout is not a TTY, output automatically degrades to ascii glyphs with
colour disabled, so an agent piping `tl window` receives clean text with no
escape codes.

## 7. Presentation layer

Two independent axes, each resolved at startup.

**Resolution order (both axes):** `--glyphs` / `--theme` flag → `TL_GLYPHS` /
`TL_THEME` env → `.throughline/config.toml` → `~/.config/throughline/config.toml`
→ detection.

### 7.1 Glyph modes

Three capability tiers: `nerdfont`, `unicode`, `ascii`. Every visual role has a
glyph in all three. Nerd Font codepoints are from `glyphnames.json` v3.5.0
(2026-08-02).

| role | nerdfont | unicode | ascii |
|---|---|---|---|
| behind Now (done) | `cod-pass_filled` U+EBB3 | `●` | `[x]` |
| ahead of Now (open) | `cod-circle_large` U+EBB5 | `○` | `[ ]` |
| active / in flight | `cod-play_circle` U+EBA6 | `◉` | `[>]` |
| dropped | `cod-circle_slash` U+EABD | `⊘` | `[-]` |
| blocked | `cod-warning` U+EA6C | `⚠` | `!` |
| landmark marker | `cod-milestone` U+EB20 | `◆` | `<>` |
| Now | `cod-location` U+EB1A | `│` | `\|` |
| forward arrowhead | `cod-triangle_right` U+EB70 | `▶` | `>` |
| has children | `cod-list_unordered` U+EB17 | `▾` | `+` |
| sharpened (has body) | `cod-note` U+EB26 | `≡` | `=` |
| coarse / far idea | `cod-lightbulb` U+EA61 | `·` | `~` |
| cycle boundary | `cod-sync` U+EA77 | `↻` | `@` |
| history | `cod-history` U+EA82 | `⟲` | `<<` |
| zoom out | `cod-zoom_out` U+EB82 | `⊟` | `-` |
| search | `cod-search` U+EA6D | `⌕` | `/` |

Additional nerdfont-only decoration: the Window bracket in the ribbon uses
`ple-left_half_circle_thick` U+E0B6 and `ple-right_half_circle_thick` U+E0B4 so
the window renders as a rounded pill riding the line; the status bar uses
`pl-left_hard_divider` U+E0B0. In unicode and ascii modes the bracket uses
`┌ ┐` and `[ ]` respectively.

Design constraints on the glyph set:

- **One family.** All icons are Codicons (`cod-`). Mixed icon families vary in
  stroke weight and optical size and read as visual noise at terminal sizes.
- **Consistent optical size.** Done and open use `pass_filled` (U+EBB3) and
  `circle_large` (U+EBB5), which match in size. The smaller `cod-circle`
  (U+EABC) would make items visibly jitter as they cross Now.
- **`cod-git_branch` is deliberately excluded.** A branch glyph contradicts the
  one-line metaphor at exactly the point where the method insists children do
  not get their own position. `list_unordered` correctly says "checklist inside
  this item."
- **The ribbon rule stays Unicode `─` in all modes.** `cod-horizontal_rule`
  U+EB07 is a short centred dash that does not tile seamlessly.

Nerd Font support is not reliably detectable. On first TUI launch, `tl` renders
the same sample row in all three modes and the user selects one with a keypress;
the choice is written to config and not asked again. `tl doctor` re-runs it.

### 7.2 Themes

`dark` and `light` ship as first-class themes. The terminal's actual background
is queried via OSC 11 (`terminal-light`) with a short timeout, falling back to
dark.

Colour depth degrades 24-bit → 256 → 16 → none, gated on `COLORTERM`. `NO_COLOR`
is respected, and `--color=auto|always|never` overrides.

The palette is anchored to the project logo: deep navy and electric cyan. The
light theme is **not** an inversion of the dark theme — bright cyan on white is
unreadable, so the light theme uses a deeper, more saturated blue as its accent.

### 7.3 Semantic tokens

Views name roles only; a theme maps roles to colours. Tokens: `past`, `now`,
`near`, `mid`, `far`, `marker`, `blocked`, `dropped`, `cursor`, `window`,
`muted`, `bg`, `fg`.

Two reasons this indirection is required rather than optional:

1. The progressive-detail fade is the most important visual in the tool, and it
   cannot be one computation across both themes — dark fades toward black, light
   fades toward white. `near`/`mid`/`far` must be theme-authored.
2. It makes user-authored themes a later drop-in rather than a rewrite.

### 7.4 Accessibility

Meaning is never carried by colour alone. Glyph and colour always agree, so
`--glyphs ascii --color never` remains fully legible. This is also the mode used
automatically for non-TTY output.

## 8. TUI

One screen at two zoom levels.

- **Top — the ribbon.** The whole project as a horizontal line of glyphs, with
  Now marked and the Window drawn as a bracket. This is the zoom-out view and
  the visual expression of the method.
- **Below — the window list.** Vertical, with readable titles and bodies for the
  items inside the bracket. This is the zoom-in view.
- Scrolling the list slides the bracket on the ribbon.

Horizontal alone cannot show titles at terminal widths; vertical alone loses the
metaphor. Together they implement "zoom out to understand the project, zoom in
to understand a period" as an interface rather than a slogan.

### 8.1 Keymap

| key | action |
|---|---|
| `j` / `k` | move cursor |
| `J` / `K` | reorder item |
| `n` | return to Now |
| `space` | advance Now past cursor |
| `a` | add item after cursor |
| `s` | sharpen (edit body) |
| `d` | drop item |
| `m` | place marker after cursor |
| `[` / `]` | narrow / widen the Window |
| `g` / `G` | start / end of line |
| `/` | search |
| `t` | toggle theme |
| `?` | help |
| `q` | quit |

## 9. The method document (`docs/method.md`)

### 9.1 Diagrams are a requirement, not decoration

The sketch communicates the method almost entirely through left-to-right ASCII
diagrams, and that is why it reads well: the central claim is spatial, so prose
alone under-argues it. `docs/method.md` must carry a diagram for every structural
claim it makes.

### 9.2 Diagram inventory

Carried forward from the sketch:

| # | diagram | claim it argues |
|---|---|---|
| 1 | the line: completed work, Now, planned work | the project is a line |
| 2 | the PDSA circle | work is cyclical |
| 3 | the same cycle unrolled onto time | but progress is linear |
| 4 | the sliding window over the line | attention is local and moves |
| 5 | a selected slice | any slice tells a story |
| 6 | `A → B → C → D` | strict ordering is a statement of intent |
| 7 | a task tree vs. the same work spread on the line | hierarchy should be rare |
| 8 | a bucket board vs. a line with a `Deploy` marker | avoid temporal buckets |
| 9 | months laid across the line | history is first-class |
| 10 | detail density fading with distance | planning is progressive |
| 11 | Kanban's moving cards vs. "you are here" | the central inversion |

Added by this design (sections 3.1–3.3):

| # | diagram | claim it argues |
|---|---|---|
| 12 | one item crossing Now, before and after | status is position |
| 13 | work placed after a `launch` marker | markers are landmarks, not buckets |
| 14 | two readers viewing different spans of one line | the Window is a view |
| 15 | the ribbon above the window list | the TUI's two zoom levels |

### 9.3 Diagrams and the renderer share one vocabulary

Every line-shaped diagram in the document uses the unicode glyph set from 7.1 —
`●`, `○`, `◆`, `│`, `▶` — so that a reader who runs `tl line` sees the same
shapes the document taught them. The document and the tool are one visual
language, not two.

Unicode rather than ascii, because a document is not terminal-constrained the way
a running TUI is, and the sketch demonstrates that these shapes are most of what
makes the method legible. The ascii set exists for degraded terminals and piped
output, not for prose.

Diagrams 1, 4, 5, 8, 9, 13, and 14 are *line-shaped*: they depict real line
states and are therefore **generated from fixture lines by `tl` itself**, using
the golden-snapshot machinery already required by section 10. The document
cannot drift from the tool's actual output, because the output is the document.

The remaining diagrams (2, 3, 6, 7, 10, 11, 12, 15) are conceptual or
comparative — a PDSA circle, a Kanban board, a before/after pair — and are
hand-drawn. They are checked by review, not by tests.

## 10. Testing

Test-driven throughout.

| layer | approach |
|---|---|
| `format` | round-trip property tests (parse → serialize → parse is stable); golden-file tests for `tl fmt` normalization; malformed-input tests asserting errors carry line numbers |
| `model` | unit tests on ordering, `advance`, `move`, `drop`, id stability |
| `view` | unit tests on window and slice derivation, including boundaries |
| `check` | one test per lint, positive and negative |
| `tui` | ratatui `TestBackend` golden snapshots per view per glyph mode; ascii snapshots double as human-reviewable fixtures |
| `theme` | a lint test asserting no view constructs a colour directly — every style must resolve from a token |
| `cli` | `assert_cmd` integration tests, including `--json` schema stability and non-TTY degradation |

## 11. Scope

**In scope for the POC:**

- `docs/method.md`, written thoroughly, carrying all fifteen diagrams in section
  9.2 — the seven line-shaped ones generated from fixtures by `tl`.
- `tl` with the command surface in section 6, the presentation layer in section
  7, and the TUI in section 8.
- `.throughline/line.md` holding the plan for building Throughline itself.

**Explicitly out of scope:**

- Multiple projects or lines per repository.
- User-authored themes (the token layer makes this a later addition).
- Any web, server, or sync component.
- Multi-user or collaboration features; git is the collaboration mechanism.
- An event log or time-travel beyond what git history provides.
- Dependencies between items. Order expresses intent, not a dependency graph.

## 12. Open questions

None. Items previously undecided — storage location, stack, glyph set, and the
detail-level representation — are resolved above.
