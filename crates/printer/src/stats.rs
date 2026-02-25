use std::{
    ops::{Add, AddAssign},
    time::Duration,
};

use crate::util::NiceDuration;

/// Сводная статистика, полученная в конце поиска.
///
/// Когда статистика сообщается принтером, она соответствует всем поискам,
/// выполненным с этим принтером.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    elapsed: NiceDuration,
    searches: u64,
    searches_with_match: u64,
    bytes_searched: u64,
    bytes_printed: u64,
    matched_lines: u64,
    matches: u64,
}

impl Stats {
    /// Возвращает новое значение для отслеживания сводной статистики по поискам.
    ///
    /// Вся статистика установлена в `0`.
    pub fn new() -> Stats {
        Stats::default()
    }

    /// Возвращает общее количество прошедшего времени.
    pub fn elapsed(&self) -> Duration {
        self.elapsed.0
    }

    /// Возвращает общее количество выполненных поисков.
    pub fn searches(&self) -> u64 {
        self.searches
    }

    /// Возвращает общее количество поисков, которые нашли хотя бы одно совпадение.
    pub fn searches_with_match(&self) -> u64 {
        self.searches_with_match
    }

    /// Возвращает общее количество байтов, которые были найдены.
    pub fn bytes_searched(&self) -> u64 {
        self.bytes_searched
    }

    /// Возвращает общее количество байтов, которые были напечатаны.
    pub fn bytes_printed(&self) -> u64 {
        self.bytes_printed
    }

    /// Возвращает общее количество строк, которые участвовали в совпадении.
    ///
    /// Когда совпадения могут содержать несколько строк, это включает каждую строку,
    /// которая является частью каждого совпадения.
    pub fn matched_lines(&self) -> u64 {
        self.matched_lines
    }

    /// Возвращает общее количество совпадений.
    ///
    /// Может быть несколько совпадений на строку.
    pub fn matches(&self) -> u64 {
        self.matches
    }

    /// Добавляет к прошедшему времени.
    pub fn add_elapsed(&mut self, duration: Duration) {
        self.elapsed.0 += duration;
    }

    /// Добавляет к количеству выполненных поисков.
    pub fn add_searches(&mut self, n: u64) {
        self.searches += n;
    }

    /// Добавляет к количеству поисков, которые нашли хотя бы одно совпадение.
    pub fn add_searches_with_match(&mut self, n: u64) {
        self.searches_with_match += n;
    }

    /// Добавляет к общему количеству байтов, которые были найдены.
    pub fn add_bytes_searched(&mut self, n: u64) {
        self.bytes_searched += n;
    }

    /// Добавляет к общему количеству байтов, которые были напечатаны.
    pub fn add_bytes_printed(&mut self, n: u64) {
        self.bytes_printed += n;
    }

    /// Добавляет к общему количеству строк, которые участвовали в совпадении.
    pub fn add_matched_lines(&mut self, n: u64) {
        self.matched_lines += n;
    }

    /// Добавляет к общему количеству совпадений.
    pub fn add_matches(&mut self, n: u64) {
        self.matches += n;
    }
}

impl Add for Stats {
    type Output = Stats;

    fn add(self, rhs: Stats) -> Stats {
        self + &rhs
    }
}

impl<'a> Add<&'a Stats> for Stats {
    type Output = Stats;

    fn add(self, rhs: &'a Stats) -> Stats {
        Stats {
            elapsed: NiceDuration(self.elapsed.0 + rhs.elapsed.0),
            searches: self.searches + rhs.searches,
            searches_with_match: self.searches_with_match
                + rhs.searches_with_match,
            bytes_searched: self.bytes_searched + rhs.bytes_searched,
            bytes_printed: self.bytes_printed + rhs.bytes_printed,
            matched_lines: self.matched_lines + rhs.matched_lines,
            matches: self.matches + rhs.matches,
        }
    }
}

impl AddAssign for Stats {
    fn add_assign(&mut self, rhs: Stats) {
        *self += &rhs;
    }
}

impl<'a> AddAssign<&'a Stats> for Stats {
    fn add_assign(&mut self, rhs: &'a Stats) {
        self.elapsed.0 += rhs.elapsed.0;
        self.searches += rhs.searches;
        self.searches_with_match += rhs.searches_with_match;
        self.bytes_searched += rhs.bytes_searched;
        self.bytes_printed += rhs.bytes_printed;
        self.matched_lines += rhs.matched_lines;
        self.matches += rhs.matches;
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Stats {
    fn serialize<S: serde::Serializer>(
        &self,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut state = s.serialize_struct("Stats", 7)?;
        state.serialize_field("elapsed", &self.elapsed)?;
        state.serialize_field("searches", &self.searches)?;
        state.serialize_field(
            "searches_with_match",
            &self.searches_with_match,
        )?;
        state.serialize_field("bytes_searched", &self.bytes_searched)?;
        state.serialize_field("bytes_printed", &self.bytes_printed)?;
        state.serialize_field("matched_lines", &self.matched_lines)?;
        state.serialize_field("matches", &self.matches)?;
        state.end()
    }
}
