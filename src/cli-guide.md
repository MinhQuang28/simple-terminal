# Driving Repodeck from a pane

You are running inside a Repodeck terminal pane. `repodeck` talks to the app that hosts you.
`$REPODECK_PANE` is your own pane's id; commands that take `--pane` default to it.

## See what is open

    repodeck list

One line per pane, tab-separated: repo, repo path, tab title, pane id, kind (shell/claude),
agent status (Working, Done, Your turn, Approve?, Error, or empty).

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

## Talk to a pane

    repodeck send --pane <id> "yes"          # types the text and presses Return
    repodeck send --pane <id> --no-enter "partial input"
    repodeck read --pane <id> --lines 80     # last 80 lines it shows, scrollback included

A split's command runs once; it doesn't run again when the app restarts. The user can ⌘-click a
pane id you print to see that pane.

Text is sent as one line (newlines and tabs become spaces). Poll `read` or `list` to follow a
worker; an agent pane's status in `list` turns to Done or Your turn when its turn ends.

## Other

    repodeck focus <id>                      # bring a pane to the front
    repodeck notify "Build finished"         # post to the inbox and as a system notification
