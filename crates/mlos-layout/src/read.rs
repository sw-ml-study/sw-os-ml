//! Reading a rendered document's columns back out.
//!
//! Line-based, which is all either emitter produces and all a validator
//! needs: both write one column per line. It is not a JSON parser and does
//! not pretend to be -- a consumer's real parser is what decides whether
//! this is valid JSON, and `tests/` puts the emitted text through one.

/// A rendered document's columns, in the order they were written.
///
/// The `Vec` is public because checking index alignment means walking
/// every column whatever its name, and an accessor that hands out exactly
/// that is the `Vec` with extra steps.
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
