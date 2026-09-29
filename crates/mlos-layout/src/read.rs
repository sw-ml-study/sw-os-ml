//! Reading a rendered document's columns back out.
//!
//! Line-based, not a JSON parser: it relies on both emitters writing one
//! column per line. Design: docs/notes/mlos-layout.md.

/// A rendered document's columns, in the order they were written.
pub struct Columns(pub Vec<(String, Vec<String>)>);

impl Columns {
    /// Every `"name": [...]` column in `text`.
    #[must_use]
    pub fn read(text: &str) -> Self {
        Self(
            text.lines()
                .filter_map(|line| {
                    let (name, rest) = line.trim().split_once("\": [")?;
                    let items = rest.trim_end().trim_end_matches([',', ']']);
                    let values = match items.is_empty() {
                        true => Vec::new(),
                        false => items.split(", ").map(str::to_owned).collect(),
                    };
                    Some((name.trim_start_matches('"').to_owned(), values))
                })
                .collect(),
        )
    }

    /// One column as numbers.
    pub fn numbers(&self, name: &str) -> Result<Vec<u64>, String> {
        self.raw(name)?
            .iter()
            .map(|item| {
                item.parse()
                    .map_err(|_| format!("{name}: {item:?} is not a number"))
            })
            .collect()
    }

    /// One column as strings, with the JSON quotes taken off.
    pub fn strings(&self, name: &str) -> Result<Vec<&str>, String> {
        Ok(self
            .raw(name)?
            .iter()
            .map(|item| item.trim_matches('"'))
            .collect())
    }

    /// One column's raw items.
    fn raw(&self, name: &str) -> Result<&[String], String> {
        self.0
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, values)| values.as_slice())
            .ok_or_else(|| format!("column {name} is missing"))
    }
}
