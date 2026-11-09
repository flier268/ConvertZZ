use super::error::CoreError;
use super::types::{DictionaryEntry, Direction, IndexedDictionaryEntry};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use uuid::Uuid;

#[derive(Default)]
struct TrieNode {
    children: HashMap<char, TrieNode>,
    replacement: Option<String>,
    order: Option<usize>,
}

struct ReplacementTrie {
    root: TrieNode,
}

impl ReplacementTrie {
    fn new(entries: Vec<(String, String, usize)>) -> Self {
        let mut root = TrieNode::default();
        for (source, target, order) in entries {
            if source.is_empty() {
                continue;
            }
            let mut node = &mut root;
            for character in source.chars() {
                node = node.children.entry(character).or_default();
            }
            if node.order.is_none() || order < node.order.unwrap_or(usize::MAX) {
                node.replacement = Some(target);
                node.order = Some(order);
            }
        }
        Self { root }
    }

    fn replace(&self, input: &str, fallback: impl Fn(&str) -> String) -> String {
        let characters: Vec<char> = input.chars().collect();
        let mut output = String::new();
        let mut unmatched = String::new();
        let mut index = 0;
        while index < characters.len() {
            let mut node = &self.root;
            let mut cursor = index;
            let mut matched: Option<(usize, String, usize)> = None;
            while cursor < characters.len() {
                let Some(child) = node.children.get(&characters[cursor]) else {
                    break;
                };
                node = child;
                cursor += 1;
                if let (Some(replacement), Some(order)) = (&node.replacement, node.order) {
                    if matched.as_ref().is_none_or(|item| order < item.2) {
                        matched = Some((cursor, replacement.clone(), order));
                    }
                }
            }
            if let Some((end, replacement, _)) = matched {
                if !unmatched.is_empty() {
                    output.push_str(&fallback(&unmatched));
                    unmatched.clear();
                }
                output.push_str(&replacement);
                index = end;
            } else {
                unmatched.push(characters[index]);
                index += 1;
            }
        }
        if !unmatched.is_empty() {
            output.push_str(&fallback(&unmatched));
        }
        output
    }
}

pub struct LegacyDictionary {
    s2t: ReplacementTrie,
    t2s: ReplacementTrie,
    protected_words: Vec<String>,
}

impl LegacyDictionary {
    pub fn load(path: &Path) -> Result<Self, CoreError> {
        Ok(Self::from_entries(
            read_dictionary_entries(path)?
                .into_iter()
                .map(DictionaryEntry::from)
                .collect(),
        ))
    }

    pub fn from_entries(entries: Vec<DictionaryEntry>) -> Self {
        let build = |direction: Direction| {
            let mut seen = HashSet::new();
            let mut built = entries
                .iter()
                .filter_map(|entry| {
                    let (source, target, priority) = if direction == Direction::S2t {
                        (
                            entry.simplified.as_str(),
                            entry.traditional.as_str(),
                            entry.simplified_priority,
                        )
                    } else {
                        (
                            entry.traditional.as_str(),
                            entry.simplified.as_str(),
                            entry.traditional_priority,
                        )
                    };
                    if source.is_empty() || !seen.insert(source.to_string()) || !entry.enabled {
                        return None;
                    }
                    Some((
                        source.to_string(),
                        target.to_string(),
                        priority,
                        source.chars().count(),
                    ))
                })
                .collect::<Vec<_>>();
            built.sort_by(|left, right| right.2.cmp(&left.2).then_with(|| right.3.cmp(&left.3)));
            built
                .into_iter()
                .enumerate()
                .map(|(order, (source, target, _, _))| (source, target, order))
                .collect::<Vec<_>>()
        };

        let mut protected_words = entries
            .iter()
            .filter(|entry| {
                entry.enabled
                    && entry.simplified_priority == 9999
                    && entry.traditional_priority == 9999
                    && entry.simplified == entry.traditional
            })
            .map(|entry| entry.simplified.clone())
            .collect::<Vec<_>>();
        protected_words.sort_by_key(|word| std::cmp::Reverse(word.chars().count()));

        Self {
            s2t: ReplacementTrie::new(build(Direction::S2t)),
            t2s: ReplacementTrie::new(build(Direction::T2s)),
            protected_words,
        }
    }

    pub fn replace(
        &self,
        input: &str,
        direction: Direction,
        fallback: impl Fn(&str) -> String,
    ) -> String {
        if direction == Direction::None {
            return input.to_string();
        }
        let mut placeholders = Vec::new();
        let mut protected = input.to_string();
        for word in &self.protected_words {
            if !protected.contains(word) {
                continue;
            }
            let token = format!(
                "__CONVERTZZ_{}_{}__",
                Uuid::new_v4().simple(),
                placeholders.len()
            );
            protected = protected.replace(word, &token);
            placeholders.push((token, word.clone()));
        }
        let mut converted = if direction == Direction::S2t {
            self.s2t.replace(&protected, fallback)
        } else {
            self.t2s.replace(&protected, fallback)
        };
        for (token, word) in placeholders {
            converted = converted.replace(&token, &word);
        }
        converted
    }
}

pub fn read_dictionary_entries(path: &Path) -> Result<Vec<IndexedDictionaryEntry>, CoreError> {
    let raw = std::fs::read_to_string(path)?
        .trim_start_matches('\u{feff}')
        .to_string();
    Ok(raw
        .split('\n')
        .enumerate()
        .filter_map(|(index, line)| {
            let line = line.trim_end_matches('\r');
            if line.is_empty() {
                return None;
            }
            let columns: Vec<&str> = line.split('\t').collect();
            if columns.len() < 6 {
                return None;
            }
            Some(IndexedDictionaryEntry {
                index,
                enabled: matches!(columns[0].to_ascii_lowercase().as_str(), "true" | "1"),
                entry_type: columns[1].to_string(),
                simplified: columns[2].to_string(),
                simplified_priority: columns[3].parse().unwrap_or(0),
                traditional: columns[4].to_string(),
                traditional_priority: columns[5].parse().unwrap_or(0),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests;
