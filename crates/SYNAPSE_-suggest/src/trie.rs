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
    if let Some(best_child) = node.children.values().find(|c| c.max_count == node.max_count) {
        return best_command(best_child);
    }
    // Fallback: return own terminal if present, otherwise scan children.
    node.full_cmd
        .as_deref()
        .or_else(|| node.children.values().find_map(best_command))
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
