mod history;
mod trie;

pub use trie::Suggester;

pub fn load_suggester() -> Suggester {
    let history = history::load_all();
    Suggester::new(history)
}
