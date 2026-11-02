use std::rc::Rc;

use indexmap::IndexMap;

use crate::string::PhpString;
use crate::value::Value;

/// A PHP array key. PHP only has integer and string keys.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Int(i64),
    Str(PhpString),
}

impl Key {
    /// Builds a key from string bytes, applying PHP's normalization:
    /// canonical decimal integers (`"5"`, `"-3"`) become int keys, anything
    /// else (`"05"`, `"-0"`, `"1.5"`) stays a string key.
    pub fn from_bytes(bytes: &[u8]) -> Key {
        if is_canonical_int(bytes)
            && let Some(i) = std::str::from_utf8(bytes).ok().and_then(|s| s.parse::<i64>().ok())
        {
            return Key::Int(i);
        }
        Key::Str(PhpString::from(bytes))
    }

    pub fn to_value(&self) -> Value {
        match self {
            Key::Int(i) => Value::Int(*i),
            Key::Str(s) => Value::Str(s.clone()),
        }
    }
}

fn is_canonical_int(b: &[u8]) -> bool {
    let digits = b.strip_prefix(b"-").unwrap_or(b);
    match digits {
        [] => false,
        [b'0'] => b.len() == 1,
        [first, rest @ ..] => (b'1'..=b'9').contains(first) && rest.iter().all(u8::is_ascii_digit),
    }
}

#[derive(Clone, Debug, Default)]
struct ArrayData {
    map: IndexMap<Key, Value>,
    /// The key `$a[] = ...` uses next; `None` until an int key is inserted.
    /// PHP 8.3+ semantics: after `[-5 => x]` the next key is -4.
    next_free: Option<i64>,
}

/// A PHP array: an insertion-ordered map with copy-on-write value semantics.
#[derive(Clone, Debug, Default)]
pub struct PhpArray(Rc<ArrayData>);

impl PhpArray {
    pub fn new() -> Self {
        PhpArray::default()
    }

    /// A list (`[0 => a, 1 => b, ...]`) of the given values.
    pub fn from_list(values: impl IntoIterator<Item = Value>) -> Self {
        let mut array = PhpArray::new();
        for v in values {
            array.push(v);
        }
        array
    }

    fn data(&mut self) -> &mut ArrayData {
        Rc::make_mut(&mut self.0)
    }

    pub fn len(&self) -> usize {
        self.0.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.map.is_empty()
    }

    pub fn get(&self, key: &Key) -> Option<&Value> {
        self.0.map.get(key)
    }

    pub fn contains_key(&self, key: &Key) -> bool {
        self.0.map.contains_key(key)
    }

    pub fn get_index(&self, index: usize) -> Option<(&Key, &Value)> {
        self.0.map.get_index(index)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Key, &Value)> {
        self.0.map.iter()
    }

    pub fn keys(&self) -> impl Iterator<Item = &Key> {
        self.0.map.keys()
    }

    pub fn values(&self) -> impl Iterator<Item = &Value> {
        self.0.map.values()
    }

    pub fn insert(&mut self, key: Key, value: Value) {
        let data = self.data();
        if let Key::Int(k) = key {
            data.bump_next_free(k);
        }
        data.map.insert(key, value);
    }

    /// `$a[] = value`. Returns false if the next key is already taken
    /// (only possible after inserting `PHP_INT_MAX`).
    pub fn push(&mut self, value: Value) -> bool {
        match self.push_mut() {
            Some(slot) => {
                *slot = value;
                true
            }
            None => false,
        }
    }

    /// Appends a null element and returns it for writing.
    pub fn push_mut(&mut self) -> Option<&mut Value> {
        let data = self.data();
        let key = data.next_free.unwrap_or(0);
        if data.map.contains_key(&Key::Int(key)) {
            return None;
        }
        data.bump_next_free(key);
        let (index, _) = data.map.insert_full(Key::Int(key), Value::Null);
        Some(&mut data.map[index])
    }

    /// The element at `key`, inserting null first if it is missing.
    pub fn entry_mut(&mut self, key: Key) -> &mut Value {
        let data = self.data();
        if let Key::Int(k) = key {
            data.bump_next_free(k);
        }
        data.map.entry(key).or_insert(Value::Null)
    }

    pub fn get_mut(&mut self, key: &Key) -> Option<&mut Value> {
        if !self.contains_key(key) {
            return None;
        }
        self.data().map.get_mut(key)
    }

    /// Removes `key`, preserving the order of the remaining elements.
    pub fn remove(&mut self, key: &Key) -> Option<Value> {
        if !self.contains_key(key) {
            return None;
        }
        self.data().map.shift_remove(key)
    }

    /// Removes and returns the last element (`array_pop`).
    pub fn pop(&mut self) -> Option<Value> {
        let data = self.data();
        let (key, value) = data.map.pop()?;
        if let Key::Int(k) = key
            && data.next_free == k.checked_add(1)
        {
            data.next_free = Some(k);
        }
        Some(value)
    }

    /// Renumbers integer keys from 0 in order, keeping string keys (what
    /// `array_shift`, `array_merge` and friends do).
    pub fn renumbered(&self) -> PhpArray {
        let mut out = PhpArray::new();
        for (k, v) in self.iter() {
            match k {
                Key::Int(_) => {
                    out.push(v.clone());
                }
                Key::Str(_) => out.insert(k.clone(), v.clone()),
            }
        }
        out
    }
}

impl ArrayData {
    fn bump_next_free(&mut self, k: i64) {
        if self.next_free.is_none_or(|n| k >= n) {
            self.next_free = Some(k.saturating_add(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_keys() {
        assert_eq!(Key::from_bytes(b"5"), Key::Int(5));
        assert_eq!(Key::from_bytes(b"-3"), Key::Int(-3));
        assert_eq!(Key::from_bytes(b"0"), Key::Int(0));
        assert!(matches!(Key::from_bytes(b"05"), Key::Str(_)));
        assert!(matches!(Key::from_bytes(b"-0"), Key::Str(_)));
        assert!(matches!(Key::from_bytes(b""), Key::Str(_)));
        assert!(matches!(Key::from_bytes(b"9223372036854775808"), Key::Str(_)));
    }

    #[test]
    fn copy_on_write() {
        let mut a = PhpArray::from_list([Value::Int(1)]);
        let b = a.clone();
        a.push(Value::Int(2));
        assert_eq!((a.len(), b.len()), (2, 1));
    }

    #[test]
    fn next_key_follows_php83() {
        let mut a = PhpArray::new();
        a.insert(Key::Int(-5), Value::Null);
        a.push(Value::Null);
        assert!(a.contains_key(&Key::Int(-4)));
        let mut b = PhpArray::new();
        b.insert(Key::Int(7), Value::Null);
        b.remove(&Key::Int(7));
        b.push(Value::Null);
        assert!(b.contains_key(&Key::Int(8)));
    }
}
