use super::*;

fn request(line: &str) -> Result<String, String> {
    let mut args: Vec<String> = line.split(' ').skip(1).map(String::from).collect();
    let verb = line.split(' ').next().unwrap();
    match build(verb, &mut args).expect("a verb of this module")? {
        Request::Send(command) | Request::Print(command) => Ok(command),
    }
}

#[test]
fn locations_split_off_line_and_column() {
    assert_eq!(location("src/a.rs"), ("src/a.rs", "", ""));
    assert_eq!(location("src/a.rs:12"), ("src/a.rs", "12", ""));
    assert_eq!(location("src/a.rs:12:3"), ("src/a.rs", "12", "3"));
    assert_eq!(location("weird:name.rs"), ("weird:name.rs", "", ""));
}

#[test]
fn verbs_become_socket_commands() {
    assert_eq!(request("do pane.zoom --pane P").unwrap(), "do\tpane.zoom\tP");
    assert_eq!(request("projects --json").unwrap(), "projects\tjson");
    assert_eq!(request("tab close T --force").unwrap(), "tab.close\tT\tforce");
    assert_eq!(request("tab rename T api work").unwrap(), "tab.rename\tT\tapi work");
    assert_eq!(request("inbox clear").unwrap(), "inbox.mark\tclear");
    assert_eq!(request("pane close P").unwrap(), "pane.close\tP");
    assert!(request("interrupt").is_err(), "a pane is required");
    assert!(request("tab move T").is_err(), "a destination is required");
    assert!(request("approve --pane P --deny").unwrap().starts_with("approve\tP\tdeny\t"));
    assert!(request("commands").unwrap().lines().any(|c| c == "tab.new"));
}

#[test]
fn worktree_subcommands_and_the_older_form() {
    let root = std::fs::canonicalize("/").unwrap().display().to_string();
    assert_eq!(request("worktree list / --json").unwrap(), format!("worktree.list\t{root}\tjson"));
    let new = request("worktree new / --branch feat/x --shell").unwrap();
    assert!(new.starts_with(&format!("worktree.new\t{root}\tfeat/x\t\tshell\t")), "{new}");
    assert_eq!(
        request("worktree remove / --delete-branch").unwrap(),
        format!("worktree.remove\t{root}\tdelete-branch\t")
    );
    let mut args = vec!["/".to_string()];
    assert!(build("worktree", &mut args).is_none(), "`worktree <folder>` stays the older command");
}
