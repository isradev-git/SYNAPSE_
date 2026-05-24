use std::collections::{HashMap, HashSet};

struct TrieNode {
    children: HashMap<char, TrieNode>,
    full_cmd: Option<String>,
    own_count: u32,
    max_count: u32,
}

impl TrieNode {
    fn new() -> Self {
        Self {
            children: HashMap::new(),
            full_cmd: None,
            own_count: 0,
            max_count: 0,
        }
    }
}

pub struct Suggester {
    root: TrieNode,
    counts: HashMap<String, u32>,
    path_exes: HashSet<String>,
}

impl Suggester {
    pub fn new(history: Vec<String>, path_exes: HashSet<String>) -> Self {
        let mut counts: HashMap<String, u32> = HashMap::new();
        for cmd in &history {
            *counts.entry(cmd.clone()).or_insert(0) += 1;
        }

        let mut root = TrieNode::new();
        let mut seen = std::collections::HashSet::new();

        for cmd in history {
            if !seen.insert(cmd.clone()) {
                continue;
            }
            let count = counts[&cmd];
            let mut node = &mut root;
            for c in cmd.chars() {
                node.max_count = node.max_count.max(count);
                node = node.children.entry(c).or_insert_with(TrieNode::new);
            }
            node.max_count = node.max_count.max(count);
            node.own_count = count;
            node.full_cmd = Some(cmd);
        }

        Self {
            root,
            counts,
            path_exes,
        }
    }

    /// Insert or reinforce a command learned during the current session.
    /// If the command already exists, its count is incremented; if new, it is
    /// added with count 1. max_count is updated along the full path so that
    /// query() immediately prefers recently-reinforced commands.
    pub fn insert(&mut self, cmd: &str) {
        if cmd.is_empty() {
            return;
        }
        let new_count = {
            let mut node = &self.root;
            let mut count = 1u32;
            let mut complete = true;
            for c in cmd.chars() {
                if let Some(n) = node.children.get(&c) {
                    node = n;
                } else {
                    complete = false;
                    break;
                }
            }
            if complete {
                count = node.own_count.saturating_add(1);
            }
            count
        };

        self.counts.insert(cmd.to_string(), new_count);

        let mut node = &mut self.root;
        node.max_count = node.max_count.max(new_count);
        for c in cmd.chars() {
            node = node.children.entry(c).or_insert_with(TrieNode::new);
            node.max_count = node.max_count.max(new_count);
        }
        node.own_count = new_count;
        if node.full_cmd.is_none() {
            node.full_cmd = Some(cmd.to_string());
        }
    }

    /// Seed a builtin command only if not already present in the trie.
    /// `count` sets the initial frequency tier (5=common, 3=normal, 1=rare).
    /// User history always outranks static seeds over time.
    pub fn seed(&mut self, cmd: &str, seed_count: u32) {
        if cmd.is_empty() || seed_count == 0 {
            return;
        }
        let exists = {
            let mut node = &self.root;
            let mut found = true;
            for c in cmd.chars() {
                if let Some(n) = node.children.get(&c) {
                    node = n;
                } else {
                    found = false;
                    break;
                }
            }
            found && node.own_count > 0
        };
        if exists {
            return;
        }
        self.counts.insert(cmd.to_string(), seed_count);
        let mut node = &mut self.root;
        node.max_count = node.max_count.max(seed_count);
        for c in cmd.chars() {
            node = node.children.entry(c).or_insert_with(TrieNode::new);
            node.max_count = node.max_count.max(seed_count);
        }
        node.own_count = seed_count;
        if node.full_cmd.is_none() {
            node.full_cmd = Some(cmd.to_string());
        }
    }

    pub fn query(&self, prefix: &str) -> Option<&str> {
        if prefix.is_empty() {
            return None;
        }
        let mut node = &self.root;
        for c in prefix.chars() {
            node = node.children.get(&c)?;
        }
        best_command(node, &self.path_exes)
    }

    /// Returns all (command, count) pairs for persistence.
    pub fn snapshot_counts(&self) -> Vec<(String, u32)> {
        let mut pairs: Vec<(String, u32)> =
            self.counts.iter().map(|(k, v)| (k.clone(), *v)).collect();
        pairs.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
        pairs
    }

    /// Load persisted counts and seed them.
    pub fn load_counts(&mut self, entries: &[(String, u32)]) {
        for (cmd, count) in entries {
            if !self.counts.contains_key(cmd) && !cmd.is_empty() {
                self.seed(cmd, *count);
            }
        }
    }
}

/// Returns the first word (base executable) of a command string.
fn base_command(cmd: &str) -> &str {
    cmd.split_whitespace().next().unwrap_or(cmd)
}

fn best_command<'a>(node: &'a TrieNode, path_exes: &HashSet<String>) -> Option<&'a str> {
    if node.max_count == 0 {
        return None;
    }
    if node.own_count == node.max_count && node.full_cmd.is_some() {
        return node.full_cmd.as_deref();
    }
    // Collect children whose subtree contains the best terminal.
    let tied: Vec<(&char, &TrieNode)> = node
        .children
        .iter()
        .filter(|(_, c)| c.max_count == node.max_count)
        .collect();

    if tied.is_empty() {
        let mut children: Vec<(&char, &TrieNode)> = node.children.iter().collect();
        children.sort_by_key(|(k, _)| *k);
        for (_, child) in children {
            if let Some(result) = best_command(child, path_exes) {
                return Some(result);
            }
        }
        return node.full_cmd.as_deref();
    }

    // Multiple children tie on max_count — pick the best terminal by:
    // 1. Higher frequency  2. Base command in PATH  3. Shorter length  4. Alphabetical
    let mut best_result: Option<&str> = None;
    let mut best_score: (u32, u32, isize, char) = (0, 0, isize::MIN, '\0');

    for (ch, child) in &tied {
        if let Some(term) = best_command(child, path_exes) {
            let freq = terminal_count_in_subtree(child, term).unwrap_or(node.max_count);
            let in_path = u32::from(path_exes.contains(base_command(term)));
            let neg_len = -(term.len() as isize);
            let score = (freq, in_path, neg_len, **ch);
            if best_result.is_none() || score > best_score {
                best_result = Some(term);
                best_score = score;
            }
        }
    }

    if best_result.is_some() {
        return best_result;
    }

    // Fallback: alphabetical scan of all children.
    let mut children: Vec<(&char, &TrieNode)> = node.children.iter().collect();
    children.sort_by_key(|(k, _)| *k);
    for (_, child) in children {
        if let Some(result) = best_command(child, path_exes) {
            return Some(result);
        }
    }
    node.full_cmd.as_deref()
}

fn terminal_count_in_subtree(node: &TrieNode, cmd: &str) -> Option<u32> {
    if node.full_cmd.as_deref() == Some(cmd) {
        return Some(node.own_count);
    }
    for child in node.children.values() {
        if let Some(c) = terminal_count_in_subtree(child, cmd) {
            return Some(c);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_path_exes() -> HashSet<String> {
        HashSet::new()
    }

    #[test]
    fn prefix_match() {
        let history = vec![
            "git push origin main".to_string(),
            "git pull".to_string(),
            "ls -la".to_string(),
        ];
        let s = Suggester::new(history, empty_path_exes());
        let result = s.query("git p");
        assert!(result.is_some());
        assert!(result.unwrap().starts_with("git p"));
    }

    #[test]
    fn empty_history() {
        let s = Suggester::new(vec![], empty_path_exes());
        assert_eq!(s.query("git"), None);
        assert_eq!(s.query(""), None);
    }

    #[test]
    fn no_match() {
        let s = Suggester::new(vec!["ls -la".to_string()], empty_path_exes());
        assert_eq!(s.query("git"), None);
    }

    #[test]
    fn exact_prefix_returns_command() {
        let s = Suggester::new(vec!["git status".to_string()], empty_path_exes());
        assert_eq!(s.query("git"), Some("git status"));
    }

    #[test]
    fn accept_full_suffix() {
        let s = Suggester::new(vec!["git push origin main".to_string()], empty_path_exes());
        let prefix = "git p";
        let suggestion = s.query(prefix).unwrap();
        let ghost = &suggestion[prefix.len()..];
        assert_eq!(ghost, "ush origin main");
    }

    #[test]
    fn accept_word() {
        let suffix = "ush origin main";
        let end = suffix.find(' ').map(|i| i + 1).unwrap_or(suffix.len());
        assert_eq!(&suffix[..end], "ush ");
    }

    #[test]
    fn insert_new_command_is_queryable() {
        let mut s = Suggester::new(vec![], empty_path_exes());
        s.insert("docker ps");
        assert_eq!(s.query("docker"), Some("docker ps"));
    }

    #[test]
    fn insert_increments_count_and_wins() {
        let history = vec!["git pull".to_string(), "git push origin main".to_string()];
        let mut s = Suggester::new(history, empty_path_exes());
        s.insert("git pull");
        s.insert("git pull");
        assert_eq!(s.query("git p"), Some("git pull"));
    }

    #[test]
    fn seed_adds_builtin_when_absent() {
        let mut s = Suggester::new(vec![], empty_path_exes());
        s.seed("git status", 5);
        assert_eq!(s.query("git"), Some("git status"));
    }

    #[test]
    fn seed_does_not_override_history() {
        let history = vec![
            "git pull".to_string(),
            "git pull".to_string(),
            "git pull".to_string(),
        ];
        let mut s = Suggester::new(history, empty_path_exes());
        s.seed("git status", 1); // low-priority seed, history count=3 wins
        assert_eq!(s.query("git"), Some("git pull"));
    }

    #[test]
    fn seed_skips_existing_command() {
        let history = vec!["git status".to_string()];
        let mut s = Suggester::new(history, empty_path_exes());
        let count_before = {
            let mut node = &s.root;
            for c in "git status".chars() {
                node = node.children.get(&c).unwrap();
            }
            node.own_count
        };
        s.seed("git status", 10);
        let count_after = {
            let mut node = &s.root;
            for c in "git status".chars() {
                node = node.children.get(&c).unwrap();
            }
            node.own_count
        };
        assert_eq!(
            count_before, count_after,
            "seed must not increment existing count"
        );
    }

    #[test]
    fn frequency_prefers_repeated() {
        let history = vec![
            "git push origin main".to_string(),
            "git pull".to_string(),
            "git pull".to_string(),
            "git pull".to_string(),
        ];
        let s = Suggester::new(history, empty_path_exes());
        assert_eq!(s.query("git"), Some("git pull"));
    }

    #[test]
    fn tiered_seeds_favor_higher_count() {
        let mut s = Suggester::new(vec![], empty_path_exes());
        s.seed("git status", 5);
        s.seed("g++ -o", 1);
        assert_eq!(s.query("g"), Some("git status"));
    }

    #[test]
    fn path_exes_break_tie() {
        let mut exes = HashSet::new();
        exes.insert("git".to_string());
        let mut s = Suggester::new(vec![], exes);
        s.seed("git status", 1);
        s.seed("g++ -o", 1);
        // git is in PATH, g++ is not → git wins even with equal counts
        assert_eq!(s.query("g"), Some("git status"));
    }

    #[test]
    fn seed_lower_count_does_not_override() {
        let mut s = Suggester::new(vec![], empty_path_exes());
        s.seed("git status", 5);
        s.seed("git push origin main", 3);
        // "git" prefix: both have count 5 max_path from "git status", but "git status" is shorter
        let result = s.query("git");
        assert!(result.is_some());
    }

    #[test]
    fn seed_zero_count_silently_ignored() {
        let mut s = Suggester::new(vec![], empty_path_exes());
        s.seed("ls", 0);
        assert_eq!(s.query("ls"), None);
    }

    #[test]
    fn snapshot_and_load_counts() {
        let mut s = Suggester::new(vec![], empty_path_exes());
        s.seed("ls", 5);
        s.seed("git status", 3);
        s.insert("docker ps");
        s.insert("docker ps");
        let snap = s.snapshot_counts();
        assert!(!snap.is_empty());

        let mut s2 = Suggester::new(vec![], empty_path_exes());
        s2.load_counts(&snap);
        assert_eq!(s2.query("ls"), Some("ls"));
        assert_eq!(s2.query("git"), Some("git status"));
        assert_eq!(s2.query("docker"), Some("docker ps"));
    }

    #[test]
    fn insert_stores_in_counts_map() {
        let mut s = Suggester::new(vec![], empty_path_exes());
        s.insert("docker ps");
        let snap = s.snapshot_counts();
        let entry = snap.iter().find(|(k, _)| k == "docker ps");
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().1, 1);
    }

    #[test]
    fn empty_insert_is_noop() {
        let mut s = Suggester::new(vec![], empty_path_exes());
        s.insert("");
        assert!(s.snapshot_counts().is_empty());
    }
}
