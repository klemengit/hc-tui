# Sidebar data and presentation

The sidebar follows herdr **0.9.5-followcat at `d00953e0`**. Its renderer,
configuration, glyphs, colors, ordering, truncation, selection and mouse
behavior come from the reference source. See [parity](parity.md) for scope,
data mappings and validation. M2 implementation and final conformance checks
are still in progress.

## Reported status

Core is the authority for status. hc-tui does not infer it from screen text,
program arguments or foreground process names. It reads:

- `status`, the root OSC 7501 record;
- `records`, named OSC 7501 records;
- `reported`, whether a terminal has reported status.

The effective record is the most urgent root or named record. Record
authority uses `blocked > error > working > unknown > done > idle`.
Ties prefer the root, then named records in name order. This chooses the
record; it does not define the displayed sidebar order.

The winning record supplies the agent's application and reported title.
Unknown words and extension fields remain in the protocol inventory. The
renderer consumes herdr's status vocabulary, with the exact mapping recorded
in [parity](parity.md#status-vocabulary). Record names, `source` extensions,
progress and blocked-kind fields do not add hc-tui-specific badges.

There is one deliberate exception to "no inferred status": **Waiting**, an
overlay that stands in for a shown `Idle` — never a core word, never a core
state change, and no agent's record maps to it. While evidence says an
agent's own turn is quiet but work it started still runs, the shown `Idle`
presents as `waiting`. In one line: `◌ waiting` comes from a process check
every few seconds and Claude Code's own session file. The evidence, per
eligible terminal (live, a foreground pid, a current `idle` or `done`
effective word — never blocked, error, unknown, finished or lost):

- **Process tree, every agent.** Once per machine per cycle, `machine.exec`
  runs `ps -A -o pid=,ppid=,etime=,args=` (the `on` field routes to the
  terminal's machine). A live descendant of the foreground pid at any depth
  whose argv basename is `sh`, `bash`, `zsh`, `dash`, `fish` or `ksh` with a
  `-c`-form argument counts — unless the shell started within ~20 s of the
  agent itself (startup helpers). Long-lived helpers (`node`, `npm exec`,
  `uvx`) are not shells and never count. A sighted shell is only a
  candidate: it becomes evidence when the same pid shows in two of the
  terminal's own consecutive listings, so a transient `sh -c` (a
  statusLine, or a Stop/Notification hook that runs as the turn ends)
  never flips the row.
- **The agent's own word, the exact layer.** A table keyed by the effective
  record's `app` holds entries verified against the program's own binary or
  docs — today only Claude Code: `file.read` of
  `<home>/.claude/sessions/<pid>.json` (home resolved once per machine by
  `machine.exec`), where a parse whose `pid` equals the foreground pid and
  whose `status` is `shell` means background tasks outlive the turn. Only
  `pid` and `status` are read; the file's other fields are never stored or
  shown. Agents without an entry — including Devin, Codex, Cursor Agent,
  agy and OpenCode — get the process layer only; nothing about them was
  verified as a wire-visible "waiting on sub agents" message.

A verdict is dropped the moment its terminal stops matching the
fingerprint it was probed for — state word, foreground pid, incarnation or
winning record — and re-probes on a two-speed cadence: ~3 s while the
terminal has a candidate shell or a Waiting verdict (so the row shows and
clears quickly), ~10 s once a listing found nothing. Probe calls are
bound to ~5 s, run off the event loop and share the operations opener; a
connection opens only when a cycle has an eligible terminal, and a method
the core does not offer turns its layer off quietly. Waiting never mutates
core state, seen state, attention or notifications: no status word maps to
it and it creates no alert. An unseen completion still shows `Done` until
visited (herdr's rule); once acknowledged, the same evidence overlays the
then-idle presentation.

## Program lifecycle

Lifecycle is separate from a program's last report:

| Core facts | Lifecycle |
|---|---|
| `state = "finished"` | Finished; retain exit code, signal and reason |
| `state = "live"`, no pid, reason other than `spawn_pending` | Lost |
| `state = "live"`, no pid, no reason or `spawn_pending` | Pending |
| `state = "live"`, pid present | Live |
| Another `state` word | Preserve the unfamiliar lifecycle |

A finished, lost or pending program's old report does not make it a live
agent. Its terminal stays in the inventory. A lost connection retains the
last inventory and pane frame; it does not change program lifecycle.

An agent is a live terminal with `reported`, a root record or named records.
Shells without reports do not become agents. Each agent appears once.
Workers keep their own terminal identity and endpoint. Core parent addresses
are already localized and must be matched verbatim; a bare parent is not
automatically prefixed with the worker's machine. The reference sidebar has
no separate parent-note field, so workers use its ordinary agent rows.

## Spaces, tabs and titles

M2 groups terminals by `herdr.space`, falling back to endpoint plus full cwd.
Only the cwd basename is displayed. Identical basenames with different paths
remain different spaces. Within a space, `herdr.tab` identifies a tab; an
unlabelled tab is distinct from an explicitly labelled `default` tab and
displays its numeric position. Creation order and terminal-number tie breaks
provide stable ordering. M3 adds the herdr-app document reader and shared
layout state.

Identity uses the raw endpoint, space and tab values. Sanitizing or clipping
display text must not merge identities or change focus. Every API string
that reaches the display is control-sanitized and fitted using the ported
text helpers.

Titles remain distinct data:

- `labels["herdr.title"]`: the manual pane label;
- the winning status record's `title`: the reported agent title;
- terminal `title`: the program's OSC title;
- terminal `name`: the core terminal's optional name.

These feed separate reference fields. The ported renderer decides title
precedence; available titles must not be replaced by invented terminal-id
labels.

## Attention, seen state and order

Herdr's display priority is `Blocked > Done > Working > Idle > Unknown`;
hc-tui inserts the Waiting overlay between Working and Idle (it never
outranks real work). Workspace and tab aggregates use that priority. The
default agent sort is
`priority` (an hc-tui difference from herdr, whose configured default is
space order); an explicit `agent_panel_sort = "spaces"` keeps space order.
`agent_panel_space_headers = true` heads each space's run of agents in that
order with the space's name, as the herdr app's grouped list does. With
several machines, each machine's spaces are headed separately and the header
does not name the machine.
The priority sort uses the activity chronology and deterministic receipt
rule below. Chrome choices the user makes by hand — the sort toggle,
sidebar width, section split, collapse state and workspace group
collapses — persist per endpoint under
`$XDG_STATE_HOME/hc-tui/client-shell/` and win over config at the next
start, as herdr does.

Completion comes only from a present root or named record's non-null
`activity.records[record].completion_ordinal`. Idle/done without a new marker
is readiness. Unseen markers show Done when the aggregate raw state is
idle/done; a current Working, Blocked or Unknown state retains its priority.
Turns, timestamps, repeated words and locally observed transitions never
manufacture completions. Missing Activity means unknown completion history.

Acknowledgement is client-local, per terminal and record. A composed, fresh,
visible paint acknowledges its current markers only when the outer terminal
is not known unfocused. An unseen completion on another tab stays Done until
visited. Ordinary transport reconnect retains acknowledgement. A change of
`core_id`, activity `epoch` or `scope`, or terminal incarnation baselines the
current markers. First observation of an epoch also baselines existing facts;
previously seen epochs survive an empty inventory. Named-record removal drops
its acknowledgement. No labels or shared seen state are written.

hc-tui only: `[ui] seen_after_ms` (default `0`, in milliseconds) adds a dwell.
With a value above 0 a completion is acknowledged only after its terminal has
been the focused, visible pane for that long without a break, with the outer
terminal not known unfocused. Passing over a Done agent (for example with
`next_agent`) leaves it Done. Switching to another pane, tab or space, or the
outer terminal losing focus, resets the clock. The sidebar redraws when the
dwell ends, without new output. The `herdr.seen` write follows at that moment
instead of on the first paint. Only the focused pane counts, not other panes
visible in a split. `0` keeps the immediate acknowledgement described above.

Priority recency follows reference ingestion receipts. Before watcher views
coalesce, new agents and changed clocks allocate from the maximum receipt in
the current cache; unchanged agents retain theirs and absent agents are pruned
after allocation. The maximum present record `state_ordinal` or
`completion_ordinal` orders comparable members of a new batch only, within
`(core_id, epoch, scope)`. Snapshot input slots retain incomparable-domain and
unknown-history order; foreign counters and domain names never order receipts.
Saturation preserves equal ranks and stable presentation traversal.

Thus A20, then B, then a newly discovered A10 displays A10 > B > A20.
There are no permanent per-domain queues or lifetime high-water counter.
Metadata, revision, readiness aliases with unchanged clocks, turns, timestamps
and acknowledgement do not advance receipts. These are local UI ranks, not
core ordinals. The existing activity-domain baseline retains its receipt until
a fresh clock change; reference-compatible epoch display rebuilding remains
pending, including genuinely new post-epoch work coalesced before a draw.

Inventory and output arrive on different connections. Fresh paint does not
prove that the viewer consumed output corresponding to a completion; the
stalled-stream race remains a limitation, never a parity pass.

## hc-tui sidebar options

These keys exist only in hc-tui. They are opt-in: every default keeps the
herdr layout. Herdr itself does not know them, so a terminal herdr that reads
the same `config.toml` reports them as unknown keys.

| Key | Default | Effect |
|---|---|---|
| `[ui.sidebar.spaces] agents` | `"none"` | Lists agents as indented rows under their space: `"none"` lists nothing, `"active"` lists the space's agents that are not idle or unknown (working, waiting, done, blocked), `"all"` lists every agent. A row uses the `[ui.sidebar.agents]` row layout and tokens. Rows are ordered like the agents panel's priority view. With several machines each machine lists its own agents; a collapsed machine lists none. A click on a row focuses the agent. |
| Styled token `format` and `separator` | none | In the styled form `{ token = "...", ... }` of a spaces or agents row. `format` is a string with one `{}` that is replaced by the token's text, for example `format = "({})"`. It works on text tokens only (not `state_icon` or `git_status`); width and truncation use the formatted text, and `rules` still match the raw value. `separator` is the text drawn before the token and replaces the default (`" · "`, or a plain space after `state_icon`). When the row is too narrow, an `agent_count` keeps its full width and the name before it is clipped first. The row `● assistant (2)`, with the count shown only above one agent: `rows = [["state_icon", "workspace", { token = "agent_count", dim = true, format = "({})", separator = " ", rules = [{ lt = 2, hide = true }] }]]` |
| `[ui.sidebar.spaces] show_last_agent` | `false` | In a space with two or more agents, always list the agent you focused there last as a child row, idle or not, on top of what `agents` lists (an agent is never listed twice; it also works with `agents = "none"`). With tree navigation, `navigate_enter` then puts the cursor on that agent instead of the first. The last agent is remembered while hc-tui runs (a restart forgets it), per machine and space. Spaces with one agent are unaffected. |
| `[ui.sidebar.spaces] rows` token `agent_count` | not in the default rows | Add `"agent_count"` (or `{ token = "agent_count", dim = true }`) to a spaces row to show the number of agents in the space, idle ones included (the set tree navigation lists when you open the space). Nothing is drawn for a space without agents. It is a text token, so `rules` (for example `gt`, `equals`, `hide`) work on it. |
| `[ui.sidebar] focused_style`, `cursor_style`, `open_style` | none | Separate looks for the three highlights that share one background by default. `focused_style`: the space row and the listed agent row where the focused terminal is. `cursor_style`: the navigate-mode cursor row (a space or an agent). `open_style`: a space whose agents the cursor is inside (after `navigate_enter`). Each is a table with optional `fg`, `bg` (a `#RGB` / `#RRGGBB` colour or a theme name: `accent`, `panel`, `sidebar`, `active_row`, `selection`, `surface0`, `surface1`, `overlay0`, `overlay1`, `text`, `subtext0`, `mauve`, `green`, `yellow`, `red`, `blue`, `teal`, `peach`), `bold`, `dim`, `reverse`, `marker` (one character) and `marker_fg` (the marker's colour; `fg` when unset). An unset field keeps the default look, so with none set nothing changes. `fg`, `bold`, `dim` and `reverse` apply to the whole row, status icon included. When looks overlap, each field comes from the highest of cursor, focused, open that sets it: with the cursor on the focused row the cursor's `bg` wins and the focused marker and `bold` still show. The marker is drawn in the cell just left of the row's text. Once any look has a marker, space rows keep at least one cell of `space_padding` for it on every row, so the text stays in the same column; agent rows already have that cell. They apply to the spaces list only (not the agents panel, the collapsed sidebar or mobile layout). |
| `[ui.sidebar] machine_padding` | `1` | Blank columns before a machine header's `▾` marker (multi-machine sidebar). The marker's click target moves with it. |
| `[ui.sidebar] machine_indent` | `2` | Columns the spaces and agent rows under a machine header are shifted right. |
| `[ui.sidebar] space_padding` | `1` | Blank columns before a space row's first line; its further lines sit two columns deeper. Applies with one machine too. |
| `[ui.sidebar] agent_indent` | `2` | Columns an agent row under a space is indented by, on top of the row's own padding (worktree child spaces add 4). |
| `[ui] goto_footer_rule` | `false` | `true` draws a horizontal rule, like the one under the search line, between the Go to list (`prefix+g`) and its three footer lines (selected row, path, key help). The list loses one row. |
| `[ui] agent_cycle` | `"all"` | `"active"` makes previous/next agent (`Alt+Up` / `Alt+Down`) step only through agents that are not idle or unknown, the same set as `agents = "active"`. List order is unchanged; see [keys](keys.md#alt-shortcuts). |
| `[ui] recent_agent_window_ms` | `1000` | Milliseconds within which a further `[keys] recent_agent` press steps one more agent back instead of toggling; `0` makes every press a toggle. See [keys](keys.md#alt-shortcuts). |
| `[ui] agent_panel` | `true` | `false` hides the agents panel. The spaces list takes the whole sidebar height and the section divider disappears. Agent next/previous and the agent shortcuts keep working. The collapsed sidebar drops its agent icons too. The mobile switcher is not affected. |

### Tree navigation in navigate mode

`[keys] navigate_tree = true` makes navigate mode (prefix then `w`) walk the
spaces list as a tree. It needs the expanded desktop sidebar; on mobile and
with the collapsed sidebar navigate mode stays flat.

| Key | Default | Effect |
|---|---|---|
| `[keys] navigate_tree` | `false` | Turns tree navigation on. |
| `[keys] navigate_enter` | `"l"` | Moves the cursor from the space into its agents. The space lists all its agents, idle ones included, while the cursor is inside, even with `agents = "none"`. Does nothing for a space without agents. Pressed again on an agent row, it does what `enter` does: focuses that agent and leaves navigate mode. |
| `[keys] navigate_enter_single` | `"enter"` | What `navigate_enter` does on a space with exactly one agent. `"enter"` moves the cursor into the space, as for any space. `"jump"` focuses that agent and leaves navigate mode, like `enter` on the agent row. Spaces with several agents or none behave as before. |
| `[keys] navigate_leave` | `"h"` | Moves the cursor back to the space. The idle agents hide again. |
| `[keys] navigate_next_machine` | `"shift+j"` | Jumps to the first space of the next machine. Also moves the selection to the next machine row in the Go to list (`prefix+g`), with several machines; it does not wrap, and does nothing while the search line has focus. |
| `[keys] navigate_prev_machine` | `"shift+k"` | Jumps to the first space of the current machine; when the cursor is already there, to the first space of the previous machine (like a paragraph motion in vim). In the Go to list it moves to the previous machine row. |

With tree navigation on, the cursor starts on a space and the workspace up/down
keys (`up`/`down`) move between spaces and skip the child agent rows. `j` and
`k` do the same (they are `navigate_pane_down` / `navigate_pane_up` acting as
extra up/down keys). Inside a space the same keys move between its agents and
wrap around. `enter` on an agent focuses its pane or tab and leaves navigate
mode; `enter` on a space jumps to the space as before. `esc` cancels from
either level. `1..9` still select a space.

Machine jumps work at the space level and from inside a space; from inside
they also close the space. Both wrap around at the ends. Machines are the
groups in sidebar order. A machine that is not online or has no spaces has
nothing to land on and is skipped. A collapsed project group does not hide
its machine: its first visible space is the target. With one machine, the
jumps move to that machine's first space.

Key conflicts: hc-tui does not use `navigate_pane_left/down/up/right` for
focus in navigate mode, so the default `h` / `l` for leave / enter share keys
with them without a conflict. `navigate_enter` and `navigate_leave` are checked
before the `j`/`k` aliases, so rebinding them to those keys works. `navigate_next_machine` and
`navigate_prev_machine` follow the same checks. They cannot
take the prefix, `esc`, `enter`, `tab`, `left`, `right`, `1..9` or the
workspace up/down keys; a rejected binding gives a config diagnostic.

## Machines, selection and geometry

Core machine and connection states map to herdr's endpoint states. The
reference's stale presentation applies to retained endpoint snapshots.
Subscription loss and RPC-only loss have different data freshness; neither
may discard cached terminals or invent program-status changes. Machines
without terminals and terminals with an unlisted machine prefix remain in
the projection; the reference controls which headings are visible.

Selection follows stable identities across inventory reordering. Navigation
highlight and committed focus are distinct, as in herdr. Lists reveal the
selected row using the reference scrolling rules. The agents list additionally
scrolls to the focused agent's row whenever the focused pane changes — by a
tab or workspace switch, a click, a key or a core-driven focus — and stays
where the user scrolled it until the next focus change.

Sidebar widths, collapse behavior, mobile header, tab bar, pane gutters and
borders use the reference configuration and geometry. Narrow terminals do
not introduce a separate hc-tui status-line format. Divider dragging updates
both drawing and the size allocated to the attached program.
