mod builtins;
mod history;
mod trie;

pub use trie::Suggester;

pub fn load_suggester() -> Suggester {
    let history = history::load_all();
    let mut s = Suggester::new(history);
    for &cmd in builtins::BUILTINS {
        s.seed(cmd);
    }
    s
}
