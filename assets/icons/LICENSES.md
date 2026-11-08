# Icons and program table

Everything in this folder but `vscode/` and `activity/` comes from [Herdr](https://github.com/herdrdev/herdr-app)'s
`defaults/` folder, unmodified. Repodeck draws the icons in tabs and sidebar rows, and uses `programs.json`
to match a running command to its name and icon. The files are compiled into the app, and this
notice and the license texts beside it ship in the app bundle's `Contents/Resources/assets/icons`.

| Files | Source | License |
| --- | --- | --- |
| `programs.json`: program names, aliases, tile colours, which icon to use | Herdr | Apache-2.0, [LICENSE-Herdr.txt](LICENSE-Herdr.txt) |
| Brand icons: git, cargo, vite, and so on | [Simple Icons](https://simpleicons.org) | CC0 1.0 (public domain dedication) |
| Glyphs: shell, terminal, bot, and so on | [Lucide](https://lucide.dev) | ISC, with MIT-licensed icons derived from Feather, [LICENSE-Lucide.txt](LICENSE-Lucide.txt) |
| Agent icons: Claude, Codex, OpenCode, and so on | [LobeHub Icons](https://github.com/lobehub/lobe-icons) and the agents' own marks | MIT, [LICENSE-LobeHub.txt](LICENSE-LobeHub.txt) |
| `activity/`: the Code sidebar's activity bar (files, search, git-branch, library-big) | [Lucide](https://lucide.dev), unmodified; drawn a little thinner | ISC, [LICENSE-Lucide.txt](LICENSE-Lucide.txt) |
| `vscode/`: file and folder icons in the Code view's explorer and tabs | [vscode-icons](https://github.com/vscode-icons/vscode-icons), a subset, unmodified | MIT, [LICENSE-vscode-icons.txt](LICENSE-vscode-icons.txt) |

Brand and agent marks are their owners' trademarks. Repodeck shows them only to name the
program running in a pane. None of these licenses grants trademark rights, and their use here
implies no endorsement.

Repodeck's own license ([LICENSE.md](../../LICENSE.md)) covers Repodeck's code, not these files.
