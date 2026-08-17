## Throughline

The learning log lives in `.throughline/line.md` as one ordered line. Read it in
full for complete context: it is simultaneously the plan, the queue, the
evidence trail, and the historical record.

- `tlflow window --json` — what is currently in focus
- `tlflow now --json` — the next item ahead of Now
- `tlflow add "title" --after <ref>` — placement is required; there is no backlog
- `tlflow advance --result "what happened"` — completion or accounting moves Now forward
- `tlflow check` — lint the line against the practice before finishing

Completion is position, not state: an item is done when it sits **behind Now**.
The file format calls entries items, but the practice can use them for
intentions, events, evidence, outcomes, exclusions, and follow-up threads.
Record what actually happened with `--result`; that is where the practice keeps
what it learned.

Then ask the question that moves things: **what does that change?** Act on the
answer by editing the line ahead — `tlflow move`, `tlflow add`, `tlflow drop`. A result that
changes nothing was not worth recording.
