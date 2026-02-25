/*!
Этот крейт предоставляет функциональные и быстрые принтеры, которые взаимодействуют с
крейтом [`grep-searcher`](https://docs.rs/grep-searcher).

# Краткий обзор

Принтер [`Standard`] показывает результаты в читаемом человеком формате и
смоделирован по форматам, используемым стандартными grep-подобными инструментами. Функции включают,
но не ограничиваются, кроссплатформенную окраску терминала, поиск и замену,
обработку многострочных результатов и отчётность о сводной статистике.

Принтер [`JSON`] показывает результаты в машиночитаемом формате.
Для облегчения потока результатов поиска формат использует [JSON
Lines](https://jsonlines.org/), испуская серию сообщений по мере нахождения
результатов поиска.

Принтер [`Summary`] показывает *агрегированные* результаты для одного поиска в
читаемом человеком формате и смоделирован по аналогичным форматам, найденным в стандартных
grep-подобных инструментах. Этот принтер полезен для отображения общего количества совпадений
и/или вывода путей к файлам, которые содержат или не содержат совпадения.

# Пример

Этот пример показывает, как создать "стандартный" принтер и выполнить поиск.

```
use {
    grep_regex::RegexMatcher,
    grep_printer::Standard,
    grep_searcher::Searcher,
};

const SHERLOCK: &'static [u8] = b"\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.
";

let matcher = RegexMatcher::new(r"Sherlock")?;
let mut printer = Standard::new_no_color(vec![]);
Searcher::new().search_slice(&matcher, SHERLOCK, printer.sink(&matcher))?;

// into_inner возвращает нам нижележащий writer, который мы предоставили
// new_no_color, который обёрнут в termcolor::NoColor. Таким образом, второй
// into_inner возвращает нам фактический буфер.
let output = String::from_utf8(printer.into_inner().into_inner())?;
let expected = "\
1:For the Doctor Watsons of this world, as opposed to the Sherlock
3:be, to a very large extent, the result of luck. Sherlock Holmes
";
assert_eq!(output, expected);
# Ok::<(), Box<dyn std::error::Error>>(())
```
*/

#![deny(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use crate::{
    color::{ColorError, ColorSpecs, UserColorSpec, default_color_specs},
    hyperlink::{
        HyperlinkAlias, HyperlinkConfig, HyperlinkEnvironment,
        HyperlinkFormat, HyperlinkFormatError, hyperlink_aliases,
    },
    path::{PathPrinter, PathPrinterBuilder},
    standard::{Standard, StandardBuilder, StandardSink},
    stats::Stats,
    summary::{Summary, SummaryBuilder, SummaryKind, SummarySink},
};

#[cfg(feature = "serde")]
pub use crate::json::{JSON, JSONBuilder, JSONSink};

// Максимальное количество байтов для выполнения поиска с учётом look-ahead.
//
// Это неприятный костыль, поскольку PCRE2 не предоставляет способа поиска
// подстроки некоторого входа с учётом look-ahead. В теории мы могли бы
// рефакторить различные интерфейсы 'grep' для учёта этого, но это было бы
// большим изменением. Поэтому пока мы просто позволяем PCRE2 немного поискать
// совпадение без поиска всего остального содержимого.
//
// Обратите внимание, что этот костыль активен только в многострочном режиме.
const MAX_LOOK_AHEAD: usize = 128;

#[macro_use]
mod macros;

mod color;
mod counter;
mod hyperlink;
#[cfg(feature = "serde")]
mod json;
#[cfg(feature = "serde")]
mod jsont;
mod path;
mod standard;
mod stats;
mod summary;
mod util;
