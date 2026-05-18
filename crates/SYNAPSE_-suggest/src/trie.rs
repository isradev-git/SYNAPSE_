use std::collections::HashMap;

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
}

impl Suggester {
    pub fn new(history: Vec<String>) -> Self {
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

        Self { root }
    }

    /// Insert or reinforce a command learned during the current session.
    /// If the command already exists, its count is incremented; if new, it is
    /// added with count 1. max_count is updated along the full path so that
    /// query() immediately prefers recently-reinforced commands.
    pub fn insert(&mut self, cmd: &str) {
        if cmd.is_empty() {
            return;
        }
        // Read existing count before mutating.
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
                count = node.own_count + 1;
            }
            count
        };

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

    pub fn query(&self, prefix: &str) -> Option<&str> {
        if prefix.is_empty() {
            return None;
        }
        let mut node = &self.root;
        for c in prefix.chars() {
            node = node.children.get(&c)?;
        }
        best_command(node)
    }
}

fn best_command(node: &TrieNode) -> Option<&str> {
    if node.max_count == 0 {
        return None;
    }
    // If this node is the best terminal, return it.
    if node.own_count == node.max_count && node.full_cmd.is_some() {
        return node.full_cmd.as_deref();
    }
    // Follow the child whose subtree contains the best terminal.
    // Sort by char key for determinism when max_count is tied.
    let mut candidates: Vec<(&char, &TrieNode)> = node
        .children
        .iter()
        .filter(|(_, c)| c.max_count == node.max_count)
        .collect();
    candidates.sort_by_key(|(k, _)| *k);
    if let Some((_, best_child)) = candidates.into_iter().next() {
        return best_command(best_child);
    }
    // Fallback: return own terminal if present, otherwise scan children sorted by key.
    let mut fallback_children: Vec<(&char, &TrieNode)> = node.children.iter().collect();
    fallback_children.sort_by_key(|(k, _)| *k);
    node.full_cmd
        .as_deref()
        .or_else(|| fallback_children.into_iter().find_map(|(_, c)| best_command(c)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_match() {
        let history = vec![
            "git push origin main".to_string(),
            "git pull".to_string(),
            "ls -la".to_string(),
        ];
        let s = Suggester::new(history);
        let result = s.query("git p");
        assert!(result.is_some());
        assert!(result.unwrap().starts_with("git p"));
    }

    #[test]
    fn empty_history() {
        let s = Suggester::new(vec![]);
        assert_eq!(s.query("git"), None);
        assert_eq!(s.query(""), None);
    }

    #[test]
    fn no_match() {
        let s = Suggester::new(vec!["ls -la".to_string()]);
        assert_eq!(s.query("git"), None);
    }

    #[test]
    fn exact_prefix_returns_command() {
        let s = Suggester::new(vec!["git status".to_string()]);
        assert_eq!(s.query("git"), Some("git status"));
    }

    #[test]
    fn accept_full_suffix() {
        let s = Suggester::new(vec!["git push origin main".to_string()]);
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
        let mut s = Suggester::new(vec![]);
        s.insert("docker ps");
        assert_eq!(s.query("docker"), Some("docker ps"));
    }

    #[test]
    fn insert_increments_count_and_wins() {
        let history = vec!["git pull".to_string(), "git push origin main".to_string()];
        let mut s = Suggester::new(history);
        // After startup "git p" suggests "git push origin main" (1 each; push is first)
        // Reinforce "git pull" twice — it should win.
        s.insert("git pull");
        s.insert("git pull");
        assert_eq!(s.query("git p"), Some("git pull"));
    }

    #[test]
    fn frequency_prefers_repeated() {
        let history = vec![
            "git push origin main".to_string(),
            "git pull".to_string(),
            "git pull".to_string(),
            "git pull".to_string(),
        ];
        let s = Suggester::new(history);
        // "git pull" appears 3 times, "git push origin main" once — prefer pull
        assert_eq!(s.query("git"), Some("git pull"));
    }
}
