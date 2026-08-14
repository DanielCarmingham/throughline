# `tlflow` — the manual

The practice itself is described in [method.md](method.md). This is the tool.

- [Install](#install)
- [The shape of it](#the-shape-of-it)
- [Refs](#refs)
- [Reading the line](#reading-the-line)
- [Changing the line](#changing-the-line)
- [Seeding and generating](#seeding-and-generating)
- [The terminal UI](#the-terminal-ui)
- [Checking the line](#checking-the-line)
- [For agents](#for-agents)
- [Appearance](#appearance)
- [Configuration](#configuration)
- [The file format](#the-file-format)

## Install

```
cargo install --path tlflow      # from a clone
tlflow --version
```

Then, in any repository:

```
tlflow init      # creates .throughline/line.md, THROUGHLINE.md, AGENTS.md
tlflow           # opens the terminal UI
```

`tlflow init` never overwrites an existing line.

## The shape of it

Work lives on **one ordered line** running from past, through **Now**, into the
future. Everything behind Now is history; everything ahead is intention.

Three rules explain most of the tool:

- **Completion is position.** An item is done when it sits behind Now. There is
  no done flag — `advance` and `done` *move* things.
- **Placement is required.** `add` refuses to guess. Choosing where work goes is
  the thinking the practice asks for, and a default would recreate the backlog.
- **The file is the source of truth.** `.throughline/line.md` is plain Markdown.
  Hand-edit it freely; `tlflow fmt` normalises what is derived.

## Refs

Anything taking a `<ref>` accepts:

| form | means |
|---|---|
| `^k3f` | an item, by the id shown after its title |
| `v0.1` | a marker, by label |
| `now` | the Now boundary itself |

## Reading the line

```
tlflow line                   the whole thing
tlflow now                    the next item ahead of Now
tlflow window                 what is currently in focus
tlflow window --back 5 --ahead 10
tlflow slice ^k3f..now        any span; any span tells a story
```

Add `--json` to any of these for a stable machine-readable shape.

## Changing the line

```
tlflow add "parse the file" --after now
tlflow add "cleanup" --after v0.1        # "post-launch" is a PLACE
tlflow add "someday thing" --end
```

Completing work — both of these are moves, not flags:

```
tlflow advance --result "round-trip tests pass"   # Now moves past the next item
tlflow advance ^r7e --result "..."                # ...up to and including a ref
tlflow done ^t9a --result "..."                   # complete out of order
tlflow drop ^u4f --why "superseded by ^v6c"       # the other outcome
```

After recording a result, `tlflow` asks **what does that change?** That is the
question that moves things, and you answer it by editing the line ahead:

```
tlflow move ^k3f --before v0.1     # reordering IS planning
tlflow mark "v0.2" --after ^t9a    # place a landmark
tlflow retitle ^hky "new title"
tlflow sharpen ^t9a --body "why this matters, and what done looks like"
tlflow split ^x2d                  # promote children onto the line
```

Linking a revision:

```
tlflow advance --result "..." --commit auto
```

`auto` asks the repository, preferring a **jj change ID** over a git SHA —
change IDs survive rebasing and amending, where a SHA silently stops resolving.

## Seeding and generating

```
tlflow plan docs/plan.md      # `### Task N: Title` headings become items
```

Seeded work lands ahead of Now, in document order. Useful when a plan already
exists; the line then becomes the living version of it and the plan stops being
maintained.

```
tlflow diagram                # list the available names
tlflow diagram the-line       # print one
tlflow diagram --all          # every one, fenced and ready to paste
```

These are the line-shaped figures in [method.md](method.md). They are produced
by the same code that draws the ribbon, and a test asserts each appears in the
document verbatim — so the prose cannot drift from the tool.

## The terminal UI

`tlflow` with no arguments. The screen shows two zoom levels at once: the whole
project as a ribbon, and readable titles for whatever is in the window.

| key | does |
|---|---|
| `j` `k` | move the cursor (never writes) |
| `J` `K` | reorder the item under the cursor |
| `n` | jump back to Now |
| `g` `G` | start / end of the line |
| `space` | advance Now past the cursor |
| `a` | add an item after the cursor |
| `s` | sharpen — edit the body |
| `m` | place a marker |
| `d` | drop the item |
| `/` | search |
| `[` `]` | narrow / widen the window |
| `t` | toggle light and dark |
| `?` | help |
| `q` | quit |

Prompts (`a`, `s`, `m`, `/`) take text; `Enter` applies, `Esc` cancels. Every
mutation writes straight through to `.throughline/line.md`.

## Checking the line

```
tlflow check          # exits non-zero on errors, zero on warnings alone
tlflow check --json
```

| lint | severity | says |
|---|---|---|
| `bucket` | warning | a marker reads as a disguised bucket |
| `unsharpened` | warning | work inside the window with no body |
| `false-certainty` | warning | detailed work far from Now |
| `result-ahead` | error | an outcome recorded ahead of Now |
| `orphan-parent` | error | behind Now with unfinished children |
| `independent-children` | warning | children that belong on the line |
| `duplicate-id` | error | two items share an id |
| `no-now` | error | missing or duplicated Now |

Vocabulary lints are warnings so the tool stays useful rather than righteous.
Silence a legitimate marker with `check.allow_markers` in config.

## For agents

```
tlflow mcp        # MCP server over stdio
```

```json
{ "mcpServers": { "throughline": { "command": "tlflow", "args": ["mcp"] } } }
```

Tools: `line`, `window`, `now`, `add`, `advance`, `move_item`, `check`. The
handshake carries instructions that teach the vocabulary, so a fresh agent
learns the practice without being briefed. Every tool re-reads the file first —
a person may have edited it between calls.

Without MCP, `--json` on any read command works just as well, and piped output
is always plain ascii with no escape codes.

## Appearance

```
tlflow doctor                     # shows all three glyph sets, and lists themes
tlflow --glyphs nerdfont
tlflow --theme solarized
```

Glyphs come in three tiers: `nerdfont`, `unicode`, `ascii`. Piped output uses
ascii unless `--glyphs` says otherwise — an explicit flag is an instruction, not
a preference.

A theme is a TOML file naming only the tokens it changes:

```toml
# .throughline/themes/solarized.toml
base = "dark"

[tokens]
now     = "#268bd2"
marker  = "#d33682"
```

Tokens: `past` `now` `near` `mid` `far` `marker` `blocked` `dropped` `cursor`
`window` `muted` `bg` `fg`. An unknown name is an error, not silence.

## Configuration

`.throughline/config.toml`, falling back to `~/.config/throughline/config.toml`:

```toml
window_back    = 3     # items of history in the window
window_ahead   = 7     # items of intention
far_body_lines = 3     # body length that trips false-certainty
glyphs         = "nerdfont"
theme          = "solarized"

[check]
allow_markers = ["v2"]   # legitimate names that look like buckets
```

Every setting is also a flag, and flags win. `TL_GLYPHS` and `TL_THEME` sit
between the two.

## The file format

```markdown
# Throughline — Building Throughline

## Line

- [x] Sketch the practice  ^k3f
      → Ten properties. The window idea is the original one.
- [x] Pick the name  ^m2a  @commit(88ca65b)

── NOW ──

- [ ] Write the manual  ^q1d
      The tool, as opposed to the practice.
- [ ] Build account recovery  ^x2d
      - [ ] Generate recovery token
      - [ ] Expire used token

◆ v0.1 ◆

- [ ] Ship it  ^t9a  @blocked(waiting on keys)
```

- **Checkboxes are derived** from position, never authored. Move a line above
  `── NOW ──` by hand and it is complete; `tlflow fmt` corrects the box.
- **`^k3f`** is a stable id. It survives reordering.
- **`→`** marks a result — what happened. Only items behind Now may have one.
- **Indented text** without an arrow is the description: intent, written ahead
  of Now. A body is what "sharpened" means.
- **Children** are indented checkboxes with no ids, so they cannot hold a
  position. `tlflow split` promotes them if they deserve one.
- **`@blocked(…)` `@dropped(…)` `@active` `@commit(…)`** are the only metadata.
- Ascii forms `-- NOW --` and `<> label <>` are accepted on read and normalised
  on write.
