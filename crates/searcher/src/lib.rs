/*!
Этот крейт предоставляет реализацию построчного поиска с опциональной
поддержкой многострочного поиска.

# Краткий обзор

Основной тип в этом крейте — [`Searcher`], который может быть настроен
и создан с помощью [`SearcherBuilder`]. `Searcher` отвечает за чтение
байтов из источника (например, файла), выполнение поиска этих байтов с
помощью `Matcher` (например, регулярного выражения) и затем передачу результатов
этого поиска в [`Sink`] (например, stdout). Сам `Searcher` в первую очередь отвечает
за управление потреблением байтов из источника и эффективное применение `Matcher`
к этим байтам. `Searcher` также отвечает за инвертирование поиска, подсчёт строк,
вывод контекстных строк, обнаружение двоичных данных и даже решение о том,
использовать ли отображение памяти.

`Matcher` (который определён в крейте
[`grep-matcher`](https://crates.io/crates/grep-matcher)) — это трейт
для описания низкоуровневого поиска шаблонов в общем виде.
Сам интерфейс очень похож на интерфейс регулярного выражения.
Например, крейт [`grep-regex`](https://crates.io/crates/grep-regex)
предоставляет реализацию трейта `Matcher` с использованием Rust-крейта
[`regex`](https://crates.io/crates/regex).

Наконец, `Sink` описывает, как вызывающий код получает результаты поиска от
`Searcher`. Это включает процедуры, которые вызываются в начале и конце
поиска, а также процедуры, которые вызываются при нахождении совпадающих или контекстных
строк `Searcher`. Реализации `Sink` могут быть тривиально
простыми или чрезвычайно сложными, такими как принтер `Standard` в крейте
[`grep-printer`](https://crates.io/crates/grep-printer), который
эффективно реализует вывод в стиле grep. Этот крейт также предоставляет удобные
реализации `Sink` в подмодуле [`sinks`] для простого поиска с
замыканиями.

# Пример

Этот пример показывает, как выполнить поиск и прочитать результаты поиска,
используя реализацию [`UTF8`](sinks::UTF8) для `Sink`.

```
use {
    grep_matcher::Matcher,
    grep_regex::RegexMatcher,
    grep_searcher::Searcher,
    grep_searcher::sinks::UTF8,
};

const SHERLOCK: &'static [u8] = b"\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.
";

let matcher = RegexMatcher::new(r"Doctor \w+")?;
let mut matches: Vec<(u64, String)> = vec![];
Searcher::new().search_slice(&matcher, SHERLOCK, UTF8(|lnum, line| {
    // We are guaranteed to find a match, so the unwrap is OK.
    let mymatch = matcher.find(line.as_bytes())?.unwrap();
    matches.push((lnum, line[mymatch].to_string()));
    Ok(true)
}))?;

assert_eq!(matches.len(), 2);
assert_eq!(
    matches[0],
    (1, "Doctor Watsons".to_string())
);
assert_eq!(
    matches[1],
    (5, "Doctor Watson".to_string())
);

# Ok::<(), Box<dyn std::error::Error>>(())
```

См. также `examples/search-stdin.rs` в корневом каталоге этого крейта,
чтобы увидеть похожий пример, который принимает шаблон в командной строке и
выполняет поиск в stdin.
*/

#![deny(missing_docs)]

pub use crate::{
    lines::{LineIter, LineStep},
    searcher::{
        BinaryDetection, ConfigError, Encoding, MmapChoice, Searcher,
        SearcherBuilder,
    },
    sink::{
        Sink, SinkContext, SinkContextKind, SinkError, SinkFinish, SinkMatch,
        sinks,
    },
};

#[macro_use]
mod macros;

mod line_buffer;
mod lines;
mod searcher;
mod sink;
#[cfg(test)]
mod testutil;
