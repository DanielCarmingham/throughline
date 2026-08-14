# Throughline POC — Design

**Date:** 2026-08-13
**Status:** Approved — implementation plan next

## 1. Summary

Throughline is **a practice of continuous inquiry and forward flow**, in which
work is represented as one ordered line running from past, through Now, into
the future. Its tagline: *Learn in cycles. Move in a line.*

Deliberately **not** a project-management methodology. "Method" implies follow
these steps; "framework" implies here are the structures; "project management"
implies control the project. This is a way of engaging with work over time —
observe, try something, learn, adjust, continue. Medicine is a practice, science
is a practice; you get better at one by doing it and you never complete it. Real
product work has no clean beginning and end either, so the practice does not
pretend to manage a thing that terminates.

This document specifies a proof of concept: a written description of the
practice, and `tlflow`, a combined CLI and TUI tool that manages a line.

The POC has three deliverables:

1. `docs/method.md` — the practice, described thoroughly.
2. `tlflow` — a single Rust binary providing both a CLI and a TUI.
3. `.throughline/line.md` — the line for building Throughline itself, so the
   tool manages its own construction from the first commit.

## 2. Background

The practice originates from two sketches — `initial-sketch-chat.pdf`, which
establishes the model, and `Throughline Practice.pdf`, which settles the naming
and adds Inquiry. The founding observation: work happens over time, so organize
it that way. Most systems organize work by state — backlog, ready, doing,
blocked, done, someday, v2 — which produces buckets that quietly mean "not now."
Throughline makes sequence the primary organizing principle instead.

The first sketch establishes ten properties. Restated compactly:

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

### 2.1 Vocabulary

Seven words carry the practice. They are terms of art and must be used
consistently across the tool, the document, and the site.

| term | meaning |
|---|---|
| **Practice** | the whole of it; you get better by doing it and never complete it |
| **the Line** | the ordered continuity of work through time |
| **Flow** | movement along the Line; one direction, forward |
| **the Window** | the current field of attention |
| **Inquiry** | the learning that decides what comes next |
| **Cycles** | repeated experiments, unrolled onto the Line |
| **Now** | the moving boundary between observation and intention |

Name: **Throughline**. Domain: **tlflow.cc**. Tagline: *Learn in cycles. Move in
a line.* Description: *a practice of continuous inquiry and forward flow.*

The logo reads as the practice states it: the curve is Inquiry, the arrow is
Flow, neither dominates.

## 3. Design principles added by this spec

The sketches describe the model. These four claims make it mechanical — things a
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

### 3.4 Inquiry is the loop, and it needs no new field

Inquiry is the learning that decides what comes next. The unit of progress is
not a completed task but something learned. Each pass asks five questions:

1. What do we know?
2. What do we need to learn?
3. What should we try next?
4. What happened?
5. What does that change?

**Decision: Inquiry adds no field to the data model.** The tool already
expresses it:

| question | where it lives |
|---|---|
| what should we try next | position on the Line — the item ahead of Now |
| what happened | the item's `result` (5.3) |
| what does that change | `tlflow move`, `tlflow add`, `tlflow drop` — the Line ahead is edited |

The fifth question is the one that moves things, and its answer is *a changed
Line*, not a stored sentence. A `changed:` field would record an intention the
ordering already states, and the two would drift — the same failure `status is
position` (3.1) exists to prevent.

What the tool owes Inquiry is a **prompt, not a field**: after `tlflow advance` or
`tlflow done` records a result, it prints "What does that change?" with the three
verbs that answer it. That keeps the loop visible at the moment it applies
without inventing state.

## 4. Architecture

### 4.1 Approach: file-first, tool-as-lens

`.throughline/line.md` is the source of truth. It is hand-editable, and `tlflow`
reads, normalizes, renders, and mutates it.

The rejected alternative is tool-first, where `tlflow` owns all writes and the file
is an export. That buys write safety but costs the property that makes this
useful to coding agents: an agent can read one file and hold the entire project
— plan, queue, and history — in a single read. This is not merely convenient; it
is the practice's own claim that plan, queue, and record are the same artifact,
made literal.

Consequences of file-first:

- `tlflow` must tolerate hand-edited files, including malformed ones, and report
  parse problems with line numbers rather than failing opaquely.
- `tlflow fmt` normalizes derived content (checkboxes) so hand-edits converge.
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

### 4.4 Revision-control affinity: git and jj

Jujutsu (`jj`) shares this practice's central commitments to an unusual degree,
and the resemblance is load-bearing in one place and useful prior art in
several others. Verified against jj 0.44.0.

**Load-bearing: `@commit(rev)` prefers jj change IDs.**

A git SHA is not a stable identifier. The moment a linked commit is rebased,
amended, or squashed, an item behind Now points at nothing — and a practice that
claims history is first-class cannot have its history links rot. A jj **change
ID** is a property of the change rather than of a particular commit object, and
survives rewriting. Therefore: when `.jj` is present, `tlflow` records change IDs;
otherwise it falls back to git SHAs and accepts that they are best-effort.

**Prior art to match rather than reinvent:**

| jj | Throughline |
|---|---|
| `@`, the working-copy commit — "you are here" | Now |
| `jj rebase -A/--insert-after`, `-B/--insert-before` | `tlflow move --after` / `--before` |
| revsets `a..b` | `tlflow slice <ref>..<ref>` |
| change IDs stable across rewrite | `^k3f` ids stable across reordering |
| `jj op log` — every state change, recoverable | git/jj history of `line.md` |

Where jj has already settled a naming or semantic question, `tlflow` should adopt
its answer. `--after`/`--before` and `a..b` were arrived at independently in
section 6; matching jj's exact semantics costs nothing and buys familiarity for
anyone who uses both.

**Explicitly not in the POC:** deriving the past portion of the line from
`jj log`, or mapping items onto jj changes. The correspondence is a vocabulary
and an identifier choice, not an integration. Named here so it is not
re-proposed mid-build.

## 5. File format

### 5.1 Example

```markdown
# Throughline — Building Throughline

## Line

- [x] Sketch the method  ^k3f
      → Ten properties. The window and past/future-symmetry ideas are the
        original ones; the rest restate known practice.
- [x] Pick the name  ^m2a  @commit(88ca65b)
      → "Linear PM" collides with waterfall. Throughline avoids it and
        names what is actually novel.

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
- **Now** is the line `── NOW ──`. Exactly one must exist; `tlflow fmt` inserts one
  at the top if missing.
- **Markers** are `◆ label ◆`.
- **The file syntax is fixed and independent of the viewer's glyph mode.** The
  canonical written form is always the unicode form above. The ascii forms
  `-- NOW --` and `<> label <>` are *accepted on read*, for hand-editing in
  constrained environments, but `tlflow fmt` always writes the canonical form.
  Rendering glyph mode must never influence the file, or the same line would
  churn between machines with different terminal capabilities.
- **Checkboxes are derived.** Items above Now serialize as `[x]`, below as
  `[ ]`. Dropped items serialize as `[-]`. A human who moves a line above Now
  has completed it; `tlflow fmt` corrects the checkbox.
- **Bodies** are indented continuation lines beneath the title. Presence of a
  body is what "sharpened" means — see 5.3.
- **Children** are indented checkbox lines beneath a parent. Children have no
  ids and therefore cannot be positioned on the line. This makes sketch property
  6 unrepresentable-if-violated rather than merely discouraged.
- **Exceptional metadata** is inline and rare: `@blocked(reason)`,
  `@dropped(reason)`, `@active`. No other status vocabulary exists.
- **Results** record what actually happened. A result is a line inside an item's
  indented block beginning with `→` (canonical) or `->` (accepted on read),
  continuing across further-indented lines.
- **Commit links** attach a revision to an item behind Now via `@commit(rev)` on
  the title line.

### 5.3 Description and result are the past/future pair

A description is written ahead of Now and states intent. A result is written
behind Now and states outcome. They are the same field seen from opposite sides
of Now, which is the practice's past/future symmetry (sketch property 4) applied to
a single item.

This is borrowed from `dex`, whose `result` field is straightforwardly better
than the checkmark this spec previously gave a completed item. A method whose
sketch says "the loop is how we learn" needs somewhere to put what was learned.

Rules:

- Only items behind Now may carry a result; `tlflow check` lints otherwise.
- A dropped item may carry both `@dropped(reason)` and a result.
- Results are written by `tlflow advance --result`, `tlflow done --result`, and
  `tlflow drop --result`, or by hand.
- The `false-certainty` lint counts description lines only. Results are history,
  and history is allowed to be as detailed as it likes.

### 5.4 Progressive detail requires no syntax

Detail level is derived: a bare title is coarse, a title with a body is
sharpened. Far-future items are naturally bare because nobody has written a body
for them yet.

`tlflow check` uses this: it warns when an item inside the Window is still bare
(needs sharpening before work starts) and when an item far from Now has grown a
large body (false certainty about distant work). No `resolution:` field is
introduced.

## 6. CLI surface

Verbs are the practice's vocabulary. Every read command accepts `--json`.

| command | purpose |
|---|---|
| `tlflow` | launch the TUI |
| `tlflow line` | the whole line (zoom out) |
| `tlflow now` | what sits at Now |
| `tlflow window [--back N] [--ahead N]` | the current attention window |
| `tlflow slice <ref>..<ref>` | any span; back, forward, or around Now |
| `tlflow add "<title>" --after <ref>` | add an item; placement is required |
| `tlflow move <id> --after <ref>` | reorder — this is replanning |
| `tlflow advance [<id>] [--result "…"] [--commit <rev>]` | move Now forward past the next item, or past everything up to and including `<id>` |
| `tlflow done <id> [--result "…"] [--commit <rev>]` | complete an item out of order: move it to immediately behind Now |
| `tlflow drop <id> --why "<reason>" [--result "…"]` | the other outcome |
| `tlflow mark "<label>" --after <ref>` | place a landmark |
| `tlflow split <id>` | promote children onto the line |
| `tlflow retitle <id> "<title>"` | change an item's title, or a marker's label |
| `tlflow sharpen <id>` | add or edit an item's body |
| `tlflow check` | lint the line against the practice |
| `tlflow fmt` | normalize derived content |
| `tlflow plan <file>` | seed a line from a plan document |
| `tlflow init` | create `.throughline/`, method summary, agent stanza |
| `tlflow doctor` | re-run glyph and theme capability detection |

`--after` and `--before` accept an id (`^k3f`), a marker label, or `now`.
`tlflow add` has no default placement: choosing where work goes is the thinking the
method asks for, and a default would reintroduce a bucket.

`advance` and `done` are both moves, not flags — consistent with 3.1. `advance`
moves the Now marker; `done` moves an item. Neither writes a completion field,
because there isn't one. `advance` records an outcome for each item it passes,
defaulting to done.

After recording a result, both print the Inquiry prompt (3.4): "What does that
change?" followed by `tlflow move`, `tlflow add`, `tlflow drop`. Suppressed by `--json` and
whenever stdout is not a TTY, so it never pollutes machine-readable output.

### 6.1 `tlflow check` lints

`tlflow check` is what makes the practice enforceable rather than aspirational, and is
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
| `result-ahead` | an item ahead of Now carrying a result — outcomes belong to history |

**Severity.** `bucket`, `unsharpened`, and `false-certainty` are *warnings*: they
concern vocabulary and judgement, and `tlflow check` still exits 0. Everything else
is an *error* and exits non-zero. A tool that refuses to let you name a marker
"v2" is being righteous rather than useful; `check.allow_markers` suppresses the
`bucket` lint for specific labels.

"Inside the Window" means within the default window span (`window_back` = 3,
`window_ahead` = 7) measured from Now, not from the user's scroll position —
lints must not depend on where someone happens to be looking. `far_body_lines`
defaults to 3. All three are configurable in `.throughline/config.toml`.

### 6.2 Agent integration

`tlflow init` writes:

- `THROUGHLINE.md` — a short summary of the practice and the vocabulary.
- An `AGENTS.md` stanza describing the commands and the discipline, so coding
  agents adopt the vocabulary without being instructed each session.

When stdout is not a TTY, output automatically degrades to ascii glyphs with
colour disabled, so an agent piping `tlflow window` receives clean text with no
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
`┌ ┐` and `{ }` respectively — not `[ ]`, which collides with the item
glyphs and renders the bracket invisible.

Design constraints on the glyph set:

- **One family.** All icons are Codicons (`cod-`). Mixed icon families vary in
  stroke weight and optical size and read as visual noise at terminal sizes.
- **Consistent optical size.** Done and open use `pass_filled` (U+EBB3) and
  `circle_large` (U+EBB5), which match in size. The smaller `cod-circle`
  (U+EABC) would make items visibly jitter as they cross Now.
- **`cod-git_branch` is deliberately excluded.** A branch glyph contradicts the
  one-line metaphor at exactly the point where the practice insists children do
  not get their own position. `list_unordered` correctly says "checklist inside
  this item."
- **The ribbon rule stays Unicode `─` in all modes.** `cod-horizontal_rule`
  U+EB07 is a short centred dash that does not tile seamlessly.

Nerd Font support is not reliably detectable. On first TUI launch, `tlflow` renders
the same sample row in all three modes and the user selects one with a keypress;
the choice is written to config and not asked again. `tlflow doctor` re-runs it.

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
  the visual expression of the practice.
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

The sketches communicate the practice almost entirely through left-to-right ASCII
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

Added by this design (sections 3.1–3.4):

| # | diagram | claim it argues |
|---|---|---|
| 12 | one item crossing Now, before and after | status is position |
| 13 | work placed after a `launch` marker | markers are landmarks, not buckets |
| 14 | two readers viewing different spans of one line | the Window is a view |
| 15 | the five questions, the fifth reshaping the line ahead | Inquiry decides what comes next |
| 16 | the ribbon above the window list | the TUI's two zoom levels |

### 9.3 Diagrams and the renderer share one vocabulary

Every line-shaped diagram in the document uses the unicode glyph set from 7.1 —
`●`, `○`, `◆`, `│`, `▶` — so that a reader who runs `tlflow line` sees the same
shapes the document taught them. The document and the tool are one visual
language, not two.

Unicode rather than ascii, because a document is not terminal-constrained the way
a running TUI is, and the sketch demonstrates that these shapes are most of what
makes the practice legible. The ascii set exists for degraded terminals and piped
output, not for prose.

Diagrams 1, 4, 5, 8, 9, 13, and 14 are *line-shaped*: they depict real line
states and are therefore **generated from fixture lines by `tlflow` itself**, using
the golden-snapshot machinery already required by section 10. The document
cannot drift from the tool's actual output, because the output is the document.

The remaining diagrams (2, 3, 6, 7, 10, 11, 12, 15, 16) are conceptual or
comparative — a PDSA circle, a Kanban board, a before/after pair — and are
hand-drawn. They are checked by review, not by tests.

## 10. Testing

Test-driven throughout.

| layer | approach |
|---|---|
| `format` | round-trip property tests (parse → serialize → parse is stable); golden-file tests for `tlflow fmt` normalization; malformed-input tests asserting errors carry line numbers |
| `model` | unit tests on ordering, `advance`, `move`, `drop`, id stability |
| `view` | unit tests on window and slice derivation, including boundaries |
| `check` | one test per lint, positive and negative |
| `tui` | ratatui `TestBackend` golden snapshots per view per glyph mode; ascii snapshots double as human-reviewable fixtures |
| `theme` | a lint test asserting no view constructs a colour directly — every style must resolve from a token |
| `cli` | `assert_cmd` integration tests, including `--json` schema stability and non-TTY degradation |

## 11. Scope

**In scope for the POC:**

- `docs/method.md`, written thoroughly, carrying all sixteen diagrams in section
  9.2 — the seven line-shaped ones generated from fixtures by `tlflow`.
- `tlflow` with the command surface in section 6, the presentation layer in section
  7, and the TUI in section 8.
- `.throughline/line.md` holding the plan for building Throughline itself.

**Explicitly out of scope:**

- Multiple projects or lines per repository.
- User-authored themes (the token layer makes this a later addition).
- Any web, server, or sync component.
- Multi-user or collaboration features; git is the collaboration mechanism.
- An event log or time-travel beyond what git or jj history provides.
- Dependencies between items. Order expresses intent, not a dependency graph,
  and `@blocked(reason)` is the escape hatch for the exceptional case. If this
  chafes during dogfooding, that is the signal dogfooding exists to produce —
  it is not a gap to pre-emptively fill.
- Any jj integration beyond change-ID preference in `@commit(rev)` (see 4.4).
- An MCP server. The CLI plus `--json` plus non-TTY degradation already serves
  Claude and Codex. `model` and `view` stay free of I/O so this remains a thin
  later addition.

**Rejected outright, not merely deferred:**

- **Archiving completed work** (as in `dex archive --older-than`). Sketch
  property 8 is that old work is not clutter, it is farther away. An archive is
  a bucket that history disappears into, which is the failure mode the practice
  exists to avoid.

## 12. Open questions

None. Items previously undecided — storage location, stack, glyph set, and the
detail-level representation — are resolved above.
