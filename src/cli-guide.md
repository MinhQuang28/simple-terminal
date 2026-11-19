# Driving Repodeck from a pane

You are running inside a Repodeck terminal pane. `repodeck` talks to the app that hosts you.
`$REPODECK_PANE` is your own pane's id; commands that take `--pane` default to it.

## See what is open

    repodeck list

One line per pane, tab-separated: repo, repo path, tab title, pane id, kind (shell/claude),
agent status (Working, Done, Your turn, Approve?, Error, or empty).

    repodeck projects                        # project, repo, path, worktree of, tab count, selected
    repodeck tabs [--repo <folder>]          # repo, tab id, title, kind, shown, pane ids
    repodeck status [--pane <id>]            # JSON: status, agent, asked, approval it waits on

`list`, `projects` and `tabs` take `--json` for the same rows as JSON objects.

## Start work beside you

    repodeck split -- npm run dev            # shell to the right running a command; prints its pane id
    repodeck split --down -- cargo test      # same, below
    repodeck split --claude -- "review the diff in src/"   # a second agent with a first prompt
    repodeck claude ~/code/api --task "fix the failing test"  # agent in a new tab of that repo
    repodeck worktree ~/code/api --task "add rate limiting"  # agent in a new worktree of that repo
    repodeck open ~/code/api                 # shell in a new tab; prints its pane id

`split` and `claude --task` start right away and don't move the user's view: a split appears
beside the pane it was made from, a task's tab in its repo's tab list. `open`, and `claude`
without a task, switch the window to the new tab. `worktree` checks out a new branch from the
default branch in a folder beside the repo, opens Claude there, and renames the branch after the
task (e.g. `feat/rate-limiting`); use it for work that shouldn't touch the user's checkout.

## Worktrees

    repodeck worktree list [folder]          # path, branch, main/worktree, open,merged (--json too)
    repodeck worktree new [folder] --branch feat/x   # new worktree with Claude; --shell for a shell
    repodeck worktree new [folder] --task "add rate limiting"  # Claude starts on the task there
    repodeck worktree open <path>            # show that checkout (adds it to the sidebar if needed)
    repodeck worktree done <path>            # remove it and its branch once merged and clean
    repodeck worktree remove <path> [--delete-branch] [--force]

A new worktree is made in the background beside the main checkout; its tab appears when it is
ready, so poll `worktree list` or `tabs` for it. `remove` refuses uncommitted changes without
`--force`, and `--delete-branch --force` deletes the branch even when unmerged. `done` also
refuses a branch with commits the default branch doesn't contain (a squash-merged branch counts
as such).

## Tabs, panes and app commands

    repodeck tab select <tab id>             # show it (switching repo and view as needed)
    repodeck tab rename <tab id> <name>      # empty name hands the title back to its program
    repodeck tab move <tab id> --repo <folder>   # e.g. into a worktree; it shows there
    repodeck tab close <tab id> [--force]    # --force discards unsaved file edits
    repodeck pane close <id>
    repodeck do <command> [--pane <id>]      # run an app command as its key would
    repodeck commands                        # every command `do` takes

`do` acts on the selected repo and its active tab, like a key press; with `--pane` it first
shows that pane. Examples: `pane.zoom`, `pane.split_right`, `tab.reopen`, `view.code`,
`attention.next`, `tab.new_agent_worktree`.

## Talk to a pane

    repodeck send --pane <id> "yes"          # types the text and presses Return
    repodeck send --pane <id> --no-enter "partial input"
    repodeck read --pane <id> --lines 80     # last 80 lines it shows, scrollback included

A split's command runs once; it doesn't run again when the app restarts. The user can ⌘-click a
pane id you print to see that pane.

Text is sent as one line (newlines and tabs become spaces).

## Wait for a worker

    repodeck wait --pane <id>                # blocks until that agent's turn is over
    repodeck wait --pane <id> --timeout 600  # give up after 10 minutes (default 30)

`wait` prints how the turn ended: `Done`, `Your turn` (it asked something), `Approve?` (it is
stopped on a permission prompt the user must answer) or `Error`. It exits 1 on a timeout or when
the pane is gone. Then `read` the pane for the result. An agent you start with `split --claude`,
`claude --task` or `worktree --task` is listed under you in the Agents view, and its finished
turns are yours to read: they aren't sent to the user's inbox while you are running.

## Steer agents

    repodeck interrupt --pane <id>           # Esc for an agent (ends its turn), Ctrl-C for a shell
    repodeck approve --pane <id> [--deny]    # answer the permission prompt of an agent you started
    repodeck sessions [folder]               # past Claude conversations: id, modified, title
    repodeck resume [folder] [--session <id>]   # reopen one (the latest by default) in a new tab

`approve` only answers agents you started (with `split --claude`, `claude --task` or `worktree
--task`); the user answers everything else. Check `status` first: its `approval` field says
what the agent asks for.

## Code view

    repodeck code open <file>[:line[:col]]   # in the repo's Code view (or the user's editor)
    repodeck code diff <file>                # its diff against HEAD in a tab
    repodeck code diff [folder]              # the repo's list of changes

## See the window

    repodeck shot                            # saves a PNG of the window; prints its path
    repodeck shot /tmp/after.png

The picture is of the next frame the window draws, so it needs the window visible (not
minimized or fully covered); otherwise the command fails after 15 seconds.

## Other

    repodeck focus <id>                      # bring a pane to the front
    repodeck notify "Build finished"         # post to the inbox and as a system notification
    repodeck inbox [--json]                  # notices: id, read, pane, label, title, detail
    repodeck inbox read | inbox clear
    repodeck repo add <folder> [--project <name>]   # add a repo (creates the project if needed)
    repodeck project new <name> [--folder <path>]
