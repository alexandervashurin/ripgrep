/*!
ripgrep как библиотека.

Это библиотека предназначена для предоставления высокоуровневого фасада к
крейтам, которые составляют основные подпрограммы поиска ripgrep. Однако
пока нет высокоуровневой документации, направляющей пользователей о том,
как собрать все части вместе.

Каждый элемент общедоступного API в составных крейтах задокументирован, но
примеров мало.

Поваренная книга и руководство запланированы.
*/

pub extern crate grep_cli as cli;
pub extern crate grep_matcher as matcher;
#[cfg(feature = "pcre2")]
pub extern crate grep_pcre2 as pcre2;
pub extern crate grep_printer as printer;
pub extern crate grep_regex as regex;
pub extern crate grep_searcher as searcher;
