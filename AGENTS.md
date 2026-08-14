## Throughline

Project work lives in `.throughline/line.md` as one ordered line. Read it in
full for complete context: it is simultaneously the plan, the queue, and the
record.

- `tl window --json` — what is currently in focus
- `tl now --json` — the next item ahead of Now
- `tl add "title" --after <ref>` — placement is required; there is no backlog
- `tl advance --result "what happened"` — completion moves Now forward
- `tl check` — lint the line against the practice before finishing

Completion is position, not state: an item is done when it sits **behind Now**.
Record what actually happened with `--result`; that is where the practice keeps
what it learned.

Then ask the question that moves things: **what does that change?** Act on the
answer by editing the line ahead — `tl move`, `tl add`, `tl drop`. A result that
changes nothing was not worth recording.
