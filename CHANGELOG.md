TBD
===
Невыпущенные изменения. Заметки о выпуске ещё не написаны.

Исправления ошибок:

* [BUG #3212](https://github.com/BurntSushi/ripgrep/pull/3212):
  Не проверять наличие `.jj` при использовании `--no-ignore`.


15.1.0
======
Это небольшой выпуск, который исправляет ошибку обработки буферизации строк.
Это может проявляться в том, что ripgrep выводит данные позже ожидаемого
или не работает корректно с `tail -f` (даже при использовании флага
`--line-buffered`).

Исправления ошибок:

* [BUG #3194](https://github.com/BurntSushi/ripgrep/issues/3194):
  Исправлена регрессия с `--line-buffered`, появившаяся в ripgrep 15.0.0.

Улучшения функциональности:

* [FEATURE #3192](https://github.com/BurntSushi/ripgrep/pull/3192):
  Добавлен псевдоним hyperlink для Cursor.


15.0.0 (2025-10-15)
===================
ripgrep 15 — это новый выпуск основной версии ripgrep, который в основном
содержит исправления ошибок, некоторые незначительные улучшения
производительности и небольшие новые функции. Вот основные моменты:

* Исправлено несколько ошибок, связанных с сопоставлением gitignore. Это
  включает часто сообщаемую ошибку, связанную с применением правил gitignore
  из родительских каталогов.
* Исправлена регрессия использования памяти при обработке очень больших
  файлов gitignore.
* `rg -vf file`, где `file` пустой, теперь сопоставляет всё.
* Флаг `-r/--replace` теперь работает с `--json`.
* Подмножество репозиториев Jujutsu (`jj`) теперь обрабатывается как
  git-репозитории. То есть ripgrep будет соблюдать gitignore для `jj`.
* Глобы теперь могут использовать вложенные фигурные скобки.

Поддержка платформ:

* Появились артефакты выпуска для `aarch64` для Windows.
* Для `powerpc64` больше не генерируются артефакты выпуска. Рабочий процесс
  CI-выпуска перестал работать, и я не счёл нужным тратить время на
  отладку. Если кому-то это нужно и может протестировать, я буду рад
  добавить это обратно.
* Двоичные файлы ripgrep теперь компилируются с полным LTO. Вы можете
  заметить небольшие улучшения производительности и умеренное уменьшение
  размера двоичного файла.

Улучшения производительности:

* [PERF #2111](https://github.com/BurntSushi/ripgrep/issues/2111):
  Не разрешать вспомогательные двоичные файлы в Windows, когда не используется
  `-z/--search-zip`.
* [PERF #2865](https://github.com/BurntSushi/ripgrep/pull/2865):
  Избегать использования канонизации путей в Windows при выводе hyperlinks.

Исправления ошибок:

* [BUG #829](https://github.com/BurntSushi/ripgrep/issues/829),
  [BUG #2731](https://github.com/BurntSushi/ripgrep/issues/2731),
  [BUG #2747](https://github.com/BurntSushi/ripgrep/issues/2747),
  [BUG #2770](https://github.com/BurntSushi/ripgrep/issues/2770),
  [BUG #2778](https://github.com/BurntSushi/ripgrep/issues/2778),
  [BUG #2836](https://github.com/BurntSushi/ripgrep/issues/2836),
  [BUG #2933](https://github.com/BurntSushi/ripgrep/pull/2933),
  [BUG #3067](https://github.com/BurntSushi/ripgrep/pull/3067):
  Исправлена ошибка, связанная с gitignore из родительских каталогов.
* [BUG #1332](https://github.com/BurntSushi/ripgrep/issues/1332),
  [BUG #3001](https://github.com/BurntSushi/ripgrep/issues/3001):
  Сделать так, чтобы `rg -vf file`, где `file` пустой, сопоставлял всё.
* [BUG #2177](https://github.com/BurntSushi/ripgrep/issues/2177):
  Игнорировать маркер BOM UTF-8 в начале `.gitignore` (и подобных файлов).
* [BUG #2750](https://github.com/BurntSushi/ripgrep/issues/2750):
  Исправлена регрессия использования памяти для некоторых действительно
  больших файлов gitignore.
* [BUG #2944](https://github.com/BurntSushi/ripgrep/pull/2944):
  Исправлена ошибка, из-за которой «проверено байт» в выводе `--stats`
  могло быть неверным.
* [BUG #2990](https://github.com/BurntSushi/ripgrep/issues/2990):
  Исправлена ошибка, из-за которой ripgrep неправильно обрабатывал глобы,
  заканчивающиеся на `.`.
* [BUG #2094](https://github.com/BurntSushi/ripgrep/issues/2094),
  [BUG #3076](https://github.com/BurntSushi/ripgrep/issues/3076):
  Исправлена ошибка с `-m/--max-count` и `-U/--multiline`, показывающая
  слишком много совпадений.
* [BUG #3100](https://github.com/BurntSushi/ripgrep/pull/3100):
  Сохранять разделители строк при использовании флага `-r/--replace`.
* [BUG #3108](https://github.com/BurntSushi/ripgrep/issues/3108):
  Исправлена ошибка, из-за которой `-q --files-without-match` инвертировал
  код выхода.
* [BUG #3131](https://github.com/BurntSushi/ripgrep/issues/3131):
  Документировано несоответствие между `-c/--count` и `--files-with-matches`.
* [BUG #3135](https://github.com/BurntSushi/ripgrep/issues/3135):
  Исправлена редкая паника для некоторых классов больших regex на больших
  haystack.
* [BUG #3140](https://github.com/BurntSushi/ripgrep/issues/3140):
  Убедиться, что дефисы в именах флагов экранированы в roff-тексте для
  man-страницы.
* [BUG #3155](https://github.com/BurntSushi/ripgrep/issues/3155):
  Статически компилировать PCRE2 в артефакты выпуска macOS на `aarch64`.
* [BUG #3173](https://github.com/BurntSushi/ripgrep/issues/3173):
  Исправлена ошибка фильтра ancestor ignore при поиске whitelisted
  скрытых файлов.
* [BUG #3178](https://github.com/BurntSushi/ripgrep/discussions/3178):
  Исправлена ошибка, вызывающая некорректную сводную статистику с флагом
  `--json`.
* [BUG #3179](https://github.com/BurntSushi/ripgrep/issues/3179):
  Исправлена ошибка gitignore при поиске абсолютных путей с глобальными
  gitignore.
* [BUG #3180](https://github.com/BurntSushi/ripgrep/issues/3180):
  Исправлена паника при использовании `-U/--multiline` и `-r/--replace`.

Улучшения функциональности:

* Множество улучшений набора типов файлов по умолчанию для фильтрации.
* [FEATURE #1872](https://github.com/BurntSushi/ripgrep/issues/1872):
  Сделать `-r/--replace` работающим с `--json`.
* [FEATURE #2708](https://github.com/BurntSushi/ripgrep/pull/2708):
  Автодополнения для оболочки fish учитывают файл конфигурации ripgrep.
* [FEATURE #2841](https://github.com/BurntSushi/ripgrep/pull/2841):
  Добавить `italic` к списку доступных атрибутов стиля в `--color`.
* [FEATURE #2842](https://github.com/BurntSushi/ripgrep/pull/2842):
  Каталоги, содержащие `.jj`, теперь обрабатываются как git-репозитории.
* [FEATURE #2849](https://github.com/BurntSushi/ripgrep/pull/2849):
  При использовании многопоточности планировать файлы для поиска в порядке,
  указанном в CLI.
* [FEATURE #2943](https://github.com/BurntSushi/ripgrep/issues/2943):
  Добавить артефакты выпуска `aarch64` для Windows.
* [FEATURE #3024](https://github.com/BurntSushi/ripgrep/issues/3024):
  Добавить тип цвета `highlight` для стилизации несовпадающего текста в
  совпадающей строке.
* [FEATURE #3048](https://github.com/BurntSushi/ripgrep/pull/3048):
  Глобы в ripgrep (и крейте `globset`) теперь поддерживают вложенные
  альтернативы.
* [FEATURE #3096](https://github.com/BurntSushi/ripgrep/pull/3096):
  Улучшить автодополнения для `--hyperlink-format` в bash и fish.
* [FEATURE #3102](https://github.com/BurntSushi/ripgrep/pull/3102):
  Улучшить автодополнения для `--hyperlink-format` в zsh.


14.1.1 (2024-09-08)
===================
Это незначительный выпуск с исправлением ошибки сопоставления. В частности,
обнаружена ошибка, которая могла привести к тому, что ripgrep игнорировал
строки, которые должны совпадать. То есть ложные отрицания. Трудно
охарактеризовать конкретный набор regex, в которых это происходит, поскольку
это требует столкновения нескольких различных стратегий оптимизации и
получения неверного результата. Но в качестве одного из примеров, в ripgrep
regex `(?i:e.x|ex)` не сопоставляет `e-x`, когда должен. (Эта ошибка
является результатом оптимизации внутренних литералов, выполненной в крейте
`grep-regex`, а не в крейте `regex`.)

Исправления ошибок:

* [BUG #2884](https://github.com/BurntSushi/ripgrep/issues/2884):
  Исправлена ошибка, из-за которой ripgrep мог пропустить некоторые
  совпадения, о которых должен сообщить.

Разное:

* [MISC #2748](https://github.com/BurntSushi/ripgrep/issues/2748):
  Удалить функцию `simd-accel` ripgrep, потому что она часто ломалась.


14.1.0 (2024-01-06)
===================
Это незначительный выпуск с несколькими небольшими новыми функциями и
исправлениями ошибок. Этот выпуск содержит исправление ошибки неограниченного
роста памяти при обходе дерева каталогов. Этот выпуск также включает
улучшения автодополнений для оболочки `fish` и двоичные файлы выпуска для
нескольких дополнительных ARM-целевых платформ.

Исправления ошибок:

* [BUG #2664](https://github.com/BurntSushi/ripgrep/issues/2690):
  Исправлен неограниченный рост памяти в крейте `ignore`.

Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для Lean и Meson.
* [FEATURE #2684](https://github.com/BurntSushi/ripgrep/issues/2684):
  Улучшить автодополнения для оболочки `fish`.
* [FEATURE #2702](https://github.com/BurntSushi/ripgrep/pull/2702):
  Добавить двоичные файлы выпуска для `armv7-unknown-linux-gnueabihf`,
  `armv7-unknown-linux-musleabihf` и `armv7-unknown-linux-musleabi`.


14.0.3 (2023-11-28)
===================
Это патч-выпуск с исправлением ошибки флага `--sortr`.

Исправления ошибок:

* [BUG #2664](https://github.com/BurntSushi/ripgrep/issues/2664):
  Исправлен `--sortr=path`. Я оставил `todo!()` в исходном коде. Ой.


14.0.2 (2023-11-27)
===================
Это патч-выпуск с несколькими небольшими исправлениями ошибок.

Исправления ошибок:

* [BUG #2654](https://github.com/BurntSushi/ripgrep/issues/2654):
  Исправлен файл sha256 sum выпуска `deb`.
* [BUG #2658](https://github.com/BurntSushi/ripgrep/issues/2658):
  Исправлена частичная регрессия в поведении `--null-data --line-regexp`.
* [BUG #2659](https://github.com/BurntSushi/ripgrep/issues/2659):
  Исправлены автодополнения оболочки Fish.
* [BUG #2662](https://github.com/BurntSushi/ripgrep/issues/2662):
  Исправлена опечатка в документации для `-i/--ignore-case`.


14.0.1 (2023-11-26)
===================
Это патч-выпуск, предназначенный для исправления `cargo install ripgrep` в
Windows.

Исправления ошибок:

* [BUG #2653](https://github.com/BurntSushi/ripgrep/issues/2653):
  Включить `pkg/windows/Manifest.xml` в пакет крейта.


14.0.0 (2023-11-26)
===================
ripgrep 14 — это новый выпуск основной версии ripgrep, который содержит
несколько новых функций, улучшения производительности и множество
исправлений ошибок.

Главной функцией этого выпуска является поддержка hyperlinks. В этом выпуске
они являются опциональной функцией, но в будущем могут стать функцией по
умолчанию. Чтобы включить их, попробуйте передать `--hyperlink-format
default`. Если вы используете [VS Code], то попробуйте передать
`--hyperlink-format vscode`. Пожалуйста, [сообщите о своём опыте использования
hyperlinks][report-hyperlinks], положительном или отрицательном.

[VS Code]: https://code.visualstudio.com/
[report-hyperlinks]: https://github.com/BurntSushi/ripgrep/discussions/2611

Другой главной разработкой в этом выпуске является переписанный движок regex.
Обычно вы не должны замечать никаких изменений, за исключением того, что
некоторые поиски могут стать быстрее. Вы можете прочитать больше о
[переписанном движке regex в моём блоге][regex-internals]. Пожалуйста,
[сообщите о замеченных улучшениях или регрессиях
производительности][report-perf].

[report-perf]: https://github.com/BurntSushi/ripgrep/discussions/2652

Наконец, ripgrep переключился на библиотеку, которую использует для разбора
аргументов. Пользователи в большинстве случаев не должны заметить разницы
(сообщения об ошибках несколько изменились), но переопределения флагов
должны стать более согласованными. Например, такие вещи, как `--no-ignore
--ignore-vcs`, работают так, как ожидается (отключает всю фильтрацию,
связанную с правилами ignore, кроме правил, найденных в системах контроля
версий, таких как `git`).

[regex-internals]: https://blog.burntsushi.net/regex-internals/

**КРУПНЫЕ ИЗМЕНЕНИЯ**:

* `rg -C1 -A2` раньше было эквивалентно `rg -A2`, но теперь это эквивалентно
  `rg -B1 -A2`. То есть `-A` и `-B` больше не переопределяют полностью `-C`.
  Вместо этого они только частично переопределяют `-C`.

Изменения процесса сборки:

* Автодополнения оболочки и man-страница ripgrep теперь создаются путём
  запуска ripgrep с новым флагом `--generate`. Например, `rg --generate man`
  запишет man-страницу в формате `roff` в stdout. Архивы выпуска не
  изменились.
* Опциональная зависимость сборки от `asciidoc` или `asciidoctor` удалена.
  Ранее она использовалась для создания man-страницы ripgrep. Теперь ripgrep
  владеет этим процессом самостоятельно, записывая `roff` напрямую.

Улучшения производительности:

* [PERF #1746](https://github.com/BurntSushi/ripgrep/issues/1746):
  Сделать некоторые случаи с внутренними литералами быстрее.
* [PERF #1760](https://github.com/BurntSushi/ripgrep/issues/1760):
  Сделать большинство поисков с `\b` look-arounds (среди прочих) намного
  быстрее.
* [PERF #2591](https://github.com/BurntSushi/ripgrep/pull/2591):
  Параллельный обход каталогов теперь использует work stealing для более
  быстрого поиска.
* [PERF #2642](https://github.com/BurntSushi/ripgrep/pull/2642):
  Параллельный обход каталогов имеет уменьшенную contention.

Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для Ada, DITA, Elixir,
  Fuchsia, Gentoo, Gradle, GraphQL, Markdown, Prolog, Raku, TypeScript, USD, V
* [FEATURE #665](https://github.com/BurntSushi/ripgrep/issues/665):
  Добавить новый флаг `--hyperlink-format`, который превращает пути к файлам
  в hyperlinks.
* [FEATURE #1709](https://github.com/BurntSushi/ripgrep/issues/1709):
  Улучшить документацию поведения ripgrep, когда stdout является tty.
* [FEATURE #1737](https://github.com/BurntSushi/ripgrep/issues/1737):
  Предоставить двоичные файлы для Apple silicon.
* [FEATURE #1790](https://github.com/BurntSushi/ripgrep/issues/1790):
  Добавить новый флаг `--stop-on-nonmatch`.
* [FEATURE #1814](https://github.com/BurntSushi/ripgrep/issues/1814):
  Флаги теперь сгруппированы по категориям в выводе `-h/--help` и man-странице
  ripgrep.
* [FEATURE #1838](https://github.com/BurntSushi/ripgrep/issues/1838):
  Показывается ошибка при поиске байтов NUL с включённым обнаружением
  двоичных файлов.
* [FEATURE #2195](https://github.com/BurntSushi/ripgrep/issues/2195):
  Когда в zsh включён режим `extra-verbose`, показывается дополнительная
  информация о типах файлов.
* [FEATURE #2298](https://github.com/BurntSushi/ripgrep/issues/2298):
  Добавить инструкции по установке ripgrep с помощью `cargo binstall`.
* [FEATURE #2409](https://github.com/BurntSushi/ripgrep/pull/2409):
  Добавлены инструкции по установке для `winget`.
* [FEATURE #2425](https://github.com/BurntSushi/ripgrep/pull/2425):
  Автодополнения оболочки (и man-страница) могут быть созданы через
  `rg --generate`.
* [FEATURE #2524](https://github.com/BurntSushi/ripgrep/issues/2524):
  Флаг `--debug` теперь указывает, ищется ли stdin или `./`.
* [FEATURE #2643](https://github.com/BurntSushi/ripgrep/issues/2643):
  Сделать `-d` коротким флагом для `--max-depth`.
* [FEATURE #2645](https://github.com/BurntSushi/ripgrep/issues/2645):
  Вывод `--version` теперь также содержит информацию о доступности PCRE2.

Исправления ошибок:

* [BUG #884](https://github.com/BurntSushi/ripgrep/issues/884):
  Не выдавать ошибку, когда `-v/--invert-match` используется несколько раз.
* [BUG #1275](https://github.com/BurntSushi/ripgrep/issues/1275):
  Исправить ошибку с утверждением `\b` в движке regex.
* [BUG #1376](https://github.com/BurntSushi/ripgrep/issues/1376):
  Использование `--no-ignore --ignore-vcs` теперь работает, как ожидается.
* [BUG #1622](https://github.com/BurntSushi/ripgrep/issues/1622):
  Добавить примечание о сообщениях об ошибках в документацию
  `-z/--search-zip`.
* [BUG #1648](https://github.com/BurntSushi/ripgrep/issues/1648):
  Исправить ошибку, из-за которой иногда короткие флаги со значениями,
  например `-M 900`, не работали.
* [BUG #1701](https://github.com/BurntSushi/ripgrep/issues/1701):
  Исправить ошибку, из-за которой некоторые флаги нельзя было повторять.
* [BUG #1757](https://github.com/BurntSushi/ripgrep/issues/1757):
  Исправить ошибку при поиске в подкаталоге, где ignore применялись
  некорректно.
* [BUG #1891](https://github.com/BurntSushi/ripgrep/issues/1891):
  Исправить ошибку при использовании `-w` с regex, который может
  сопоставлять пустую строку.
* [BUG #1911](https://github.com/BurntSushi/ripgrep/issues/1911):
  Отключить поиск mmap во всех не-64-битных средах.
* [BUG #1966](https://github.com/BurntSushi/ripgrep/issues/1966):
  Исправить ошибку, из-за которой ripgrep мог паниковать при выводе в stderr.
* [BUG #2046](https://github.com/BurntSushi/ripgrep/issues/2046):
  Уточнить, что `--pre` может принимать любой вид пути в документации.
* [BUG #2108](https://github.com/BurntSushi/ripgrep/issues/2108):
  Улучшить документацию для синтаксиса `-r/--replace`.
* [BUG #2198](https://github.com/BurntSushi/ripgrep/issues/2198):
  Исправить ошибку, из-за которой `--no-ignore-dot` не игнорировал
  `.rgignore`.
* [BUG #2201](https://github.com/BurntSushi/ripgrep/issues/2201):
  Улучшить документацию для флага `-r/--replace`.
* [BUG #2288](https://github.com/BurntSushi/ripgrep/issues/2288):
  `-A` и `-B` теперь только частично переопределяют `-C`.
* [BUG #2236](https://github.com/BurntSushi/ripgrep/issues/2236):
  Исправить ошибку парсинга gitignore, где завершающий `\/` приводил к
  ошибке.
* [BUG #2243](https://github.com/BurntSushi/ripgrep/issues/2243):
  Исправить флаг `--sort` для значений, отличных от `path`.
* [BUG #2246](https://github.com/BurntSushi/ripgrep/issues/2246):
  Добавить примечание в логах `--debug`, когда двоичные файлы игнорируются.
* [BUG #2337](https://github.com/BurntSushi/ripgrep/issues/2337):
  Улучшить документацию, упомянув, что `--stats` всегда подразумевается
  `--json`.
* [BUG #2381](https://github.com/BurntSushi/ripgrep/issues/2381):
  Сделать `-p/--pretty` переопределяющим такие флаги, как `--no-line-number`.
* [BUG #2392](https://github.com/BurntSushi/ripgrep/issues/2392):
  Улучшить парсинг глобальной git-конфигурации поля `excludesFile`.
* [BUG #2418](https://github.com/BurntSushi/ripgrep/pull/2418):
  Уточнить семантику сортировки `--sort=path`.
* [BUG #2458](https://github.com/BurntSushi/ripgrep/pull/2458):
  Сделать `--trim` выполняющимся до того, как `--max-columns` вступит в силу.
* [BUG #2479](https://github.com/BurntSushi/ripgrep/issues/2479):
  Добавить документацию о файлах `.ignore`/`.rgignore` в родительских
  каталогах.
* [BUG #2480](https://github.com/BurntSushi/ripgrep/issues/2480):
  Исправить ошибку при использовании inline-флагов regex с `-e/--regexp`.
* [BUG #2505](https://github.com/BurntSushi/ripgrep/issues/2505):
  Улучшить документацию для `--vimgrep`, упомянув footguns и некоторые
  обходные пути.
* [BUG #2519](https://github.com/BurntSushi/ripgrep/issues/2519):
  Исправить неверное значение по умолчанию в документации для
  `--field-match-separator`.
* [BUG #2523](https://github.com/BurntSushi/ripgrep/issues/2523):
  Сделать поиск исполняемых файлов учитывающим `.com` в Windows.
* [BUG #2574](https://github.com/BurntSushi/ripgrep/issues/2574):
  Исправить ошибку в `-w/--word-regexp`, которая приводила к неверным
  смещениям совпадений.
* [BUG #2623](https://github.com/BurntSushi/ripgrep/issues/2623):
  Исправить ряд ошибок с флагом `-w/--word-regexp`.
* [BUG #2636](https://github.com/BurntSushi/ripgrep/pull/2636):
  Strip-ить двоичные файлы выпуска для macOS.


13.0.0 (2021-06-12)
===================
ripgrep 13 — это новый выпуск основной версии ripgrep, который в основном
содержит исправления ошибок, некоторые улучшения производительности и
несколько незначительных критических изменений. Также есть исправление
уязвимости безопасности в Windows
([CVE-2021-3013](https://cve.mitre.org/cgi-bin/cvename.cgi?name=CVE-2021-3013)).

Некоторые основные моменты:

Добавлен новый короткий флаг `-.`. Это псевдоним для флага `--hidden`,
который указывает ripgrep искать скрытые файлы и каталоги.

ripgrep теперь использует новую
[векторизованную реализацию `memmem`](https://github.com/BurntSushi/memchr/pull/82),
которая ускоряет многие распространённые поиски. Если вы заметили какие-либо
регрессии производительности (или значительные улучшения), я бы хотел
услышать об этом через отчёт об ошибке!

Также для пользователей Windows, использующих MSVC, Cargo теперь будет
собирать полностью статические исполняемые файлы ripgrep. Двоичные файлы
выпуска для ripgrep 13 были скомпилированы с использованием этой конфигурации.

**КРУПНЫЕ ИЗМЕНЕНИЯ**:

**Вывод обнаружения двоичных файлов несколько изменился.**

В этом выпуске было внесено небольшое изменение в формат вывода при
обнаружении двоичного файла. Ранее это выглядело так:

```
Binary file FOO matches (found "\0" byte around offset XXX)
```

Теперь это выглядит так:

```
FOO: binary file matches (found "\0" byte around offset XXX)
```

**Вывод vimgrep в многострочном режиме теперь печатает только первую строку
для каждого совпадения.**

Смотрите [issue 1866](https://github.com/BurntSushi/ripgrep/issues/1866) для
более подробного обсуждения этого. Ранее каждая строка в совпадении
дублировалась, даже когда она охватывала несколько строк. Нет никаких
изменений в выводе vimgrep, когда многострочный режим отключён.

**В многострочном режиме --count теперь эквивалентен --count-matches.**

Похоже, это соответствует тому, как `pcre2grep` реализует `--count`. Ранее
ripgrep выдавал совершенно неверные подсчёты. Другой альтернативой было бы
просто подсчитывать количество строк — даже если оно больше количества
совпадений — но это кажется крайне неинтуитивным.

**ПОЛНЫЙ СПИСОК ИСПРАВЛЕНИЙ И УЛУЧШЕНИЙ:**

Исправления безопасности:

* [CVE-2021-3013](https://cve.mitre.org/cgi-bin/cvename.cgi?name=CVE-2021-3013):
  Исправляет брешь безопасности в Windows, где запуск ripgrep с флагами
  `-z/--search-zip` или `--pre` может привести к запуску произвольных
  исполняемых файлов из текущего каталога.
* [VULN #1773](https://github.com/BurntSushi/ripgrep/issues/1773):
  Это публичный issue, отслеживающий CVE-2021-3013. README ripgrep теперь
  содержит раздел, описывающий, как сообщить об уязвимости.

Улучшения производительности:

* [PERF #1657](https://github.com/BurntSushi/ripgrep/discussions/1657):
  Сначала проверять, следует ли игнорировать файл, прежде чем выполнять
  вызовы stat.
* [PERF memchr#82](https://github.com/BurntSushi/memchr/pull/82):
  ripgrep теперь использует новую векторизованную реализацию `memmem`.

Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для ASP, Bazel, dvc,
  FlatBuffers, Futhark, минифицированных файлов, Mint, pofiles (из GNU
  gettext), Racket, Red, Ruby, VCL, Yang.
* [FEATURE #1404](https://github.com/BurntSushi/ripgrep/pull/1404):
  ripgrep теперь выводит предупреждение, если ничего не ищется.
* [FEATURE #1613](https://github.com/BurntSushi/ripgrep/pull/1613):
  Cargo теперь будет создавать статические исполняемые файлы в Windows при
  использовании MSVC.
* [FEATURE #1680](https://github.com/BurntSushi/ripgrep/pull/1680):
  Добавить `-.` как короткий псевдоним флага для `--hidden`.
* [FEATURE #1842](https://github.com/BurntSushi/ripgrep/issues/1842):
  Добавить `--field-{context,match}-separator` для настройки разделителей
  полей.
* [FEATURE #1856](https://github.com/BurntSushi/ripgrep/pull/1856):
  README теперь ссылается на
  [испанский перевод](https://github.com/UltiRequiem/traducciones/tree/master/ripgrep).

Исправления ошибок:

* [BUG #1277](https://github.com/BurntSushi/ripgrep/issues/1277):
  Документировать поведение трансляции путей cygwin в FAQ.
* [BUG #1739](https://github.com/BurntSushi/ripgrep/issues/1739):
  Исправить ошибку, из-за которой замены были ошибочными, если regex
  сопоставлял разделитель строк.
* [BUG #1311](https://github.com/BurntSushi/ripgrep/issues/1311):
  Исправить многострочную ошибку, из-за которой поиск и замена `\n` не
  работали, как ожидалось.
* [BUG #1401](https://github.com/BurntSushi/ripgrep/issues/1401):
  Исправить ошибочное взаимодействие между PCRE2 look-around и
  `-o/--only-matching`.
* [BUG #1412](https://github.com/BurntSushi/ripgrep/issues/1412):
  Исправить многострочную ошибку с поисками, использующими look-around за
  пределами совпадающих строк.
* [BUG #1577](https://github.com/BurntSushi/ripgrep/issues/1577):
  Автодополнения оболочки Fish будут продолжать автоматически генерироваться.
* [BUG #1642](https://github.com/BurntSushi/ripgrep/issues/1642):
  Исправляет ошибку, из-за которой использование `-m` и `-A` выводило больше
  совпадений, чем лимит.
* [BUG #1703](https://github.com/BurntSushi/ripgrep/issues/1703):
  Уточнить функцию `-u/--unrestricted`.
* [BUG #1708](https://github.com/BurntSushi/ripgrep/issues/1708):
  Уточнить, как работает `-S/--smart-case`.
* [BUG #1730](https://github.com/BurntSushi/ripgrep/issues/1730):
  Уточнить, что вызов CLI всегда должен быть допустимым, независимо от файла
  конфигурации.
* [BUG #1741](https://github.com/BurntSushi/ripgrep/issues/1741):
  Исправить обнаружение stdin при использовании PowerShell в средах UNIX.
* [BUG #1756](https://github.com/BurntSushi/ripgrep/pull/1756):
  Исправить ошибку, из-за которой `foo/**` сопоставлял `foo`, но не должен
  был.
* [BUG #1765](https://github.com/BurntSushi/ripgrep/issues/1765):
  Исправить панику, когда `--crlf` используется в некоторых случаях.
* [BUG #1638](https://github.com/BurntSushi/ripgrep/issues/1638):
  Корректно определять UTF-8 и выполнять транскодирование, как мы делаем
  для UTF-16.
* [BUG #1816](https://github.com/BurntSushi/ripgrep/issues/1816):
  Добавить документацию для альтернативного синтаксиса глобов, например
  `{a,b,..}`.
* [BUG #1847](https://github.com/BurntSushi/ripgrep/issues/1847):
  Уточнить, как работает флаг `--hidden`.
* [BUG #1866](https://github.com/BurntSushi/ripgrep/issues/1866#issuecomment-841635553):
  Исправить ошибку при вычислении номеров столбцов в режиме `--vimgrep`.
* [BUG #1868](https://github.com/BurntSushi/ripgrep/issues/1868):
  Исправить ошибку, из-за которой `--passthru` и `-A/-B/-C` не переопределяли
  друг друга.
* [BUG #1869](https://github.com/BurntSushi/ripgrep/pull/1869):
  Уточнить документацию для `--files-with-matches` и `--files-without-match`.
* [BUG #1878](https://github.com/BurntSushi/ripgrep/issues/1878):
  Исправить ошибку, из-за которой `\A` мог выдавать неанкерные совпадения
  в многострочном поиске.
* [BUG 94e4b8e3](https://github.com/BurntSushi/ripgrep/commit/94e4b8e3):
  Исправить номера столбцов, когда `--vimgrep` используется с
  `-U/--multiline`.


12.1.1 (2020-05-29)
===================
ripgrep 12.1.1 — это патч-выпуск, который исправляет пару небольших ошибок.
В частности, выпуск ripgrep 12.1.0 не пометил новые выпуски для всех его
зависимостей в дереве. В результате ripgrep, собранный с зависимостями из
crates.io, создавал другую сборку, чем компиляция ripgrep из исходного кода
на теге `12.1.0`. А именно, некоторые крейты, такие как `grep-cli`, имели
невыпущенные изменения.

Исправления ошибок:

* [BUG #1581](https://github.com/BurntSushi/ripgrep/issues/1581):
  Исправляет некоторые вопиющие ошибки разметки в выводе `--help`.
* [BUG #1591](https://github.com/BurntSushi/ripgrep/issues/1591):
  Упомянуть специальную группу захвата `$0` в документации для флага
  `-r/--replace`.
* [BUG #1602](https://github.com/BurntSushi/ripgrep/issues/1602):
  Исправить падающий тест, возникший из-за рассинхронизированных зависимостей.


12.1.0 (2020-05-09)
===================
ripgrep 12.1.0 — это небольшой незначительный выпуск, который в основном
включает исправления ошибок и улучшения документации. Этот выпуск также
содержит некоторые важные уведомления для downstream-упаковщиков.

**Уведомления для downstream-сопровождающих пакетов ripgrep:**

* Автодополнения оболочки Fish будут удалены в выпуске ripgrep 13.
  Смотрите [#1577](https://github.com/BurntSushi/ripgrep/issues/1577)
  для более подробной информации.
* ripgrep переключился с `a2x` на `asciidoctor` для генерации man-страницы.
  Если `asciidoctor` отсутствует, то ripgrep временно вернётся к
  `a2x`. Поддержка `a2x` будет удалена в выпуске ripgrep 13.
  Смотрите [#1544](https://github.com/BurntSushi/ripgrep/issues/1544)
  для более подробной информации.

Улучшения функциональности:

* [FEATURE #1547](https://github.com/BurntSushi/ripgrep/pull/1547):
  Поддержка распаковки файлов `.Z` через `uncompress`.

Исправления ошибок:

* [BUG #1252](https://github.com/BurntSushi/ripgrep/issues/1252):
  Добавить раздел о флаге `--pre` в GUIDE.
* [BUG #1339](https://github.com/BurntSushi/ripgrep/issues/1339):
  Улучшить сообщение об ошибке, когда предоставлен шаблон с недопустимым
  UTF-8.
* [BUG #1524](https://github.com/BurntSushi/ripgrep/issues/1524):
  Отметить, как экранировать `$` при использовании `--replace`.
* [BUG #1537](https://github.com/BurntSushi/ripgrep/issues/1537):
  Исправить ошибку сопоставления, вызванную оптимизацией внутренних
  литералов.
* [BUG #1544](https://github.com/BurntSushi/ripgrep/issues/1544):
  ripgrep теперь использует `asciidoctor` вместо `a2x` для генерации своей
  man-страницы.
* [BUG #1550](https://github.com/BurntSushi/ripgrep/issues/1550):
  Существенно уменьшить пиковое использование памяти при поиске в широких
  каталогах.
* [BUG #1571](https://github.com/BurntSushi/ripgrep/issues/1571):
  Добавить примечание о файлах конфигурации в документации
  `--type-{add,clear}`.
* [BUG #1573](https://github.com/BurntSushi/ripgrep/issues/1573):
  Исправить неверный вывод `--count-matches` при использовании look-around.


12.0.1 (2020-03-29)
===================
ripgrep 12.0.1 — это небольшой патч-выпуск, который включает незначительное
исправление ошибки, связанное с избыточными сообщениями об ошибках при поиске
в git-репозиториях с подмодулями. Это была регрессия, появившаяся в выпуске
12.0.0.

Исправления ошибок:

* [BUG #1520](https://github.com/BurntSushi/ripgrep/issues/1520):
  Не выдавать ложные сообщения об ошибках в git-репозиториях с подмодулями.


12.0.0 (2020-03-15)
===================
ripgrep 12 — это новый выпуск основной версии ripgrep, который содержит
множество исправлений ошибок, несколько важных улучшений производительности
и несколько небольших новых функций.

В ближайшем выпуске я надеюсь добавить функцию
[индексирования](https://github.com/BurntSushi/ripgrep/issues/1497)
в ripgrep, которая значительно ускорит поиск путём построения индекса.
Отзывы будут очень признательны, особенно о пользовательском опыте, который
будет сложно сделать правильно.

Этот выпуск не имеет известных критических изменений.

Устаревания:

* Флаг `--no-pcre2-unicode` устарел. Вместо этого используйте флаг
  `--no-unicode`, который применяется как к движку regex по умолчанию, так и
  к PCRE2. Пока что `--no-pcre2-unicode` и `--pcre2-unicode` являются
  псевдонимами для `--no-unicode` и `--unicode` соответственно. Флаги
  `--[no-]pcre2-unicode` могут быть удалены в будущем выпуске.
* Флаг `--auto-hybrid-regex` устарел. Вместо этого используйте новый флаг
  `--engine` со значением `auto`.

Улучшения производительности:

* [PERF #1087](https://github.com/BurntSushi/ripgrep/pull/1087):
  ripgrep стал умнее, когда обнаруженные литералы являются пробельными
  символами.
* [PERF #1381](https://github.com/BurntSushi/ripgrep/pull/1381):
  Обход каталогов ускорен с помощью спекулятивных проверок существования
  файлов ignore.
* [PERF cd8ec38a](https://github.com/BurntSushi/ripgrep/commit/cd8ec38a):
  Улучшить обнаружение внутренних литералов для более эффективного покрытия
  большего количества случаев. Например, ` +Sherlock Holmes +` теперь
  извлекает ` Sherlock Holmes ` вместо ` `.
* [PERF 6a0e0147](https://github.com/BurntSushi/ripgrep/commit/6a0e0147):
  Улучшить обнаружение литералов, когда используется флаг
  `-w/--word-regexp`.
* [PERF ad97e9c9](https://github.com/BurntSushi/ripgrep/commit/ad97e9c9):
  Улучшить общую производительность флага `-w/--word-regexp`.

Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для erb, diff, Gradle, HAML,
  Org, Postscript, Skim, Slim, Slime, RPM Spec files, Typoscript, xml.
* [FEATURE #1370](https://github.com/BurntSushi/ripgrep/pull/1370):
  Добавить флаг `--include-zero`, который показывает файлы, searched без
  совпадений.
* [FEATURE #1390](https://github.com/BurntSushi/ripgrep/pull/1390):
  Добавить флаг `--no-context-separator`, который всегда скрывает разделители
  контекста.
* [FEATURE #1414](https://github.com/BurntSushi/ripgrep/pull/1414):
  Добавить флаг `--no-require-git`, чтобы позволить ripgrep соблюдать
  gitignore везде.
* [FEATURE #1420](https://github.com/BurntSushi/ripgrep/pull/1420):
  Добавить `--no-ignore-exclude` для игнорирования правил в файлах
  `.git/info/exclude`.
* [FEATURE #1466](https://github.com/BurntSushi/ripgrep/pull/1466):
  Добавить флаг `--no-ignore-files` для отключения всех флагов `--ignore-file`.
* [FEATURE #1488](https://github.com/BurntSushi/ripgrep/pull/1488):
  Добавить флаг `--engine` для более лёгкого переключения между движками
  regex.
* [FEATURE 75cbe88f](https://github.com/BurntSushi/ripgrep/commit/75cbe88f):
  Добавить флаг `--no-unicode`. Это работает на всех поддерживаемых движках
  regex.

Исправления ошибок:

* [BUG #1291](https://github.com/BurntSushi/ripgrep/issues/1291):
  ripgrep теперь работает в несуществующих каталогах.
* [BUG #1319](https://github.com/BurntSushi/ripgrep/issues/1319):
  Исправить ошибку сопоставления из-за ошибочного обнаружения литералов.
* [**BUG #1335**](https://github.com/BurntSushi/ripgrep/issues/1335):
  Исправляет ошибку производительности при поиске в текстовых файлах с очень
  длинными строками. Это была серьёзная регрессия производительности в
  некоторых случаях.
* [BUG #1344](https://github.com/BurntSushi/ripgrep/issues/1344):
  Документировать использование `--type all`.
* [BUG #1389](https://github.com/BurntSushi/ripgrep/issues/1389):
  Исправляет ошибку, из-за которой ripgrep паниковал при поиске в
  symlinked-каталоге.
* [BUG #1439](https://github.com/BurntSushi/ripgrep/issues/1439):
  Улучшить документацию для автоматического обнаружения stdin в ripgrep.
* [BUG #1441](https://github.com/BurntSushi/ripgrep/issues/1441):
  Удалить функции CPU из man-страницы.
* [BUG #1442](https://github.com/BurntSushi/ripgrep/issues/1442),
  [BUG #1478](https://github.com/BurntSushi/ripgrep/issues/1478):
  Улучшить документацию для флага `-g/--glob`.
* [BUG #1445](https://github.com/BurntSushi/ripgrep/issues/1445):
  ripgrep теперь соблюдает правила ignore из .git/info/exclude в worktrees.
* [BUG #1485](https://github.com/BurntSushi/ripgrep/issues/1485):
  Автодополнения оболочки Fish из выпуска Debian пакета теперь устанавливаются
  в `/usr/share/fish/vendor_completions.d/rg.fish`.


11.0.2 (2019-08-01)
===================
ripgrep 11.0.2 — это новый патч-выпуск, который исправляет несколько ошибок,
включая регрессию производительности и ошибку сопоставления при использовании
флага `-F/--fixed-strings`.

Улучшения функциональности:

* [FEATURE #1293](https://github.com/BurntSushi/ripgrep/issues/1293):
  Добавлен флаг `--glob-case-insensitive`, который заставляет `--glob`
  вести себя как `--iglob`.

Исправления ошибок:

* [BUG #1246](https://github.com/BurntSushi/ripgrep/issues/1246):
  Добавить переводы в README, начиная с неофициального китайского перевода.
* [BUG #1259](https://github.com/BurntSushi/ripgrep/issues/1259):
  Исправить ошибку, из-за которой последний байт `-f file` отрезался, если
  это не был `\n`.
* [BUG #1261](https://github.com/BurntSushi/ripgrep/issues/1261):
  Документировать, что ошибка не сообщается при поиске `\n` с
  `-P/--pcre2`.
* [BUG #1284](https://github.com/BurntSushi/ripgrep/issues/1284):
  Упомянуть `.ignore` и `.rgignore` более заметно в README.
* [BUG #1292](https://github.com/BurntSushi/ripgrep/issues/1292):
  Исправить ошибку, из-за которой `--with-filename` иногда включался
  некорректно.
* [BUG #1268](https://github.com/BurntSushi/ripgrep/issues/1268):
  Исправить серьёзную регрессию производительности в выпуске двоичного файла
  GitHub `x86_64-linux`.
* [BUG #1302](https://github.com/BurntSushi/ripgrep/issues/1302):
  Показывать лучшие сообщения об ошибках, когда дана несуществующая команда
  препроцессора.
* [BUG #1334](https://github.com/BurntSushi/ripgrep/issues/1334):
  Исправить регрессию сопоставления с флагом `-F`, когда шаблоны содержат
  мета-символы.


11.0.1 (2019-04-16)
===================
ripgrep 11.0.1 — это новый патч-выпуск, который исправляет регрессию поиска,
появившуюся в предыдущем выпуске 11.0.0. В частности, ripgrep может войти в
бесконечный цикл для некоторых шаблонов поиска при поиске недопустимого
UTF-8.

Исправления ошибок:

* [BUG #1247](https://github.com/BurntSushi/ripgrep/issues/1247):
  Исправить ошибку поиска, которая может привести к тому, что ripgrep войдёт
  в бесконечный цикл.


11.0.0 (2019-04-15)
===================
ripgrep 11 — это новый выпуск основной версии ripgrep, который содержит
множество исправлений ошибок, некоторые улучшения производительности и
несколько улучшений функциональности. В частности, улучшен пользовательский
опыт ripgrep для фильтрации двоичных файлов. Смотрите
[новый раздел руководства о двоичных данных](GUIDE.md#binary-data) для
более подробной информации.

Этот выпуск также знаменует изменение в версионировании ripgrep. Если
предыдущая версия была `0.10.0`, то эта версия — `11.0.0`. В дальнейшем
основная версия ripgrep будет увеличиваться несколько раз в год. ripgrep
продолжит быть консервативным в отношении обратной совместимости, но может
иногда вводить критические изменения, которые всегда будут документированы
в этом CHANGELOG. Смотрите
[issue 1172](https://github.com/BurntSushi/ripgrep/issues/1172) для более
подробной информации о том, почему это изменение версионирования было сделано.

Этот выпуск увеличивает **минимальную поддерживаемую версию Rust** с 1.28.0
до 1.34.0.

**КРУПНЫЕ ИЗМЕНЕНИЯ**:

* ripgrep настроил свои коды статуса выхода, чтобы быть более похожими на
  GNU grep. А именно, если во время поиска происходит не фатальная ошибка,
  то ripgrep теперь всегда выдаёт код выхода `2`, независимо от того, найдено
  совпадение или нет. Ранее ripgrep выдавал код выхода `2` только для
  катастрофической ошибки (например, ошибка синтаксиса regex). Одним
  исключением из этого является, если ripgrep запущен с `-q/--quiet`. В этом
  случае, если происходит ошибка и найдено совпадение, то ripgrep выйдет с
  кодом выхода `0`.
* Предоставление флага `-u/--unrestricted` три раза теперь эквивалентно
  предоставлению `--no-ignore --hidden --binary`. Ранее `-uuu` было
  эквивалентно `--no-ignore --hidden --text`. Разница в том, что `--binary`
  отключает фильтрацию двоичных файлов без потенциального вывода двоичных
  данных в ваш терминал. То есть `rg -uuu foo` теперь должно быть
  эквивалентно `grep -r foo`.
* Функция `avx-accel` ripgrep была удалена, поскольку она больше не нужна.
  Все использования AVX в ripgrep теперь включаются автоматически через
  обнаружение функций CPU во время выполнения. Функция `simd-accel` остаётся
  доступной (только для включения SIMD для транскодирования), однако в
  настоящее время она значительно увеличивает время компиляции.

Улучшения производительности:

* [PERF #497](https://github.com/BurntSushi/ripgrep/issues/497),
  [PERF #838](https://github.com/BurntSushi/ripgrep/issues/838):
  Сделать `rg -F -f dictionary-of-literals` намного быстрее.

Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для Apache Thrift, ASP,
  Bazel, Brotli, BuildStream, bzip2, C, C++, Cython, gzip, Java, Make,
  Postscript, QML, Tex, XML, xz, zig и zstd.
* [FEATURE #855](https://github.com/BurntSushi/ripgrep/issues/855):
  Добавить флаг `--binary` для отключения фильтрации двоичных файлов.
* [FEATURE #1078](https://github.com/BurntSushi/ripgrep/pull/1078):
  Добавить флаг `--max-columns-preview` для показа превью длинных строк.
* [FEATURE #1099](https://github.com/BurntSushi/ripgrep/pull/1099):
  Добавить поддержку Brotli и Zstd для флага `-z/--search-zip`.
* [FEATURE #1138](https://github.com/BurntSushi/ripgrep/pull/1138):
  Добавить флаг `--no-ignore-dot` для игнорирования файлов `.ignore`.
* [FEATURE #1155](https://github.com/BurntSushi/ripgrep/pull/1155):
  Добавить флаг `--auto-hybrid-regex` для автоматического отката к PCRE2.
* [FEATURE #1159](https://github.com/BurntSushi/ripgrep/pull/1159):
  Логика статуса выхода ripgrep теперь должна соответствовать GNU grep.
  Смотрите обновлённую man-страницу.
* [FEATURE #1164](https://github.com/BurntSushi/ripgrep/pull/1164):
  Добавить `--ignore-file-case-insensitive` для регистронезависимых глобов
  ignore.
* [FEATURE #1185](https://github.com/BurntSushi/ripgrep/pull/1185):
  Добавить флаг `-I` как короткую опцию для флага `--no-filename`.
* [FEATURE #1207](https://github.com/BurntSushi/ripgrep/pull/1207):
  Добавить значение `none` для `-E/--encoding` для принудительного
  отключения всего транскодирования.
* [FEATURE da9d7204](https://github.com/BurntSushi/ripgrep/commit/da9d7204):
  Добавить `--pcre2-version` для запроса информации о версии PCRE2.

Исправления ошибок:

* [BUG #306](https://github.com/BurntSushi/ripgrep/issues/306),
  [BUG #855](https://github.com/BurntSushi/ripgrep/issues/855):
  Улучшить пользовательский опыт для фильтрации двоичных файлов ripgrep.
* [BUG #373](https://github.com/BurntSushi/ripgrep/issues/373),
  [BUG #1098](https://github.com/BurntSushi/ripgrep/issues/1098):
  `**` теперь принимается как допустимый синтаксис в любом месте глоба.
* [BUG #916](https://github.com/BurntSushi/ripgrep/issues/916):
  ripgrep больше не зависает при поиске `/proc` при наличии зомби-процесса.
* [BUG #1052](https://github.com/BurntSushi/ripgrep/issues/1052):
  Исправить ошибку, из-за которой ripgrep мог паниковать при транскодировании
  файлов UTF-16.
* [BUG #1055](https://github.com/BurntSushi/ripgrep/issues/1055):
  Предложить `-U/--multiline`, когда шаблон содержит `\n`.
* [BUG #1063](https://github.com/BurntSushi/ripgrep/issues/1063):
  Всегда удалять BOM, если он присутствует, даже для UTF-8.
* [BUG #1064](https://github.com/BurntSushi/ripgrep/issues/1064):
  Исправить обнаружение внутренних литералов, которое могло привести к
  неверным совпадениям.
* [BUG #1079](https://github.com/BurntSushi/ripgrep/issues/1079):
  Исправляет ошибку, из-за которой порядок глобов мог привести к пропуску
  совпадения.
* [BUG #1089](https://github.com/BurntSushi/ripgrep/issues/1089):
  Исправить ещё одну ошибку, из-за которой ripgrep мог паниковать при
  транскодировании файлов UTF-16.
* [BUG #1091](https://github.com/BurntSushi/ripgrep/issues/1091):
  Добавить примечание об инвертированных флагах в man-страницу.
* [BUG #1093](https://github.com/BurntSushi/ripgrep/pull/1093):
  Исправить обработку литеральных слэшей в шаблонах gitignore.
* [BUG #1095](https://github.com/BurntSushi/ripgrep/issues/1095):
  Исправить угловые случаи, связанные с флагом `--crlf`.
* [BUG #1101](https://github.com/BurntSushi/ripgrep/issues/1101):
  Исправить экранирование AsciiDoc для вывода man-страницы.
* [BUG #1103](https://github.com/BurntSushi/ripgrep/issues/1103):
  Уточнить, что делает `--encoding auto`.
* [BUG #1106](https://github.com/BurntSushi/ripgrep/issues/1106):
  `--files-with-matches` и `--files-without-match` работают с одним файлом.
* [BUG #1121](https://github.com/BurntSushi/ripgrep/issues/1121):
  Исправить ошибку, которая запускала Windows antimalware при использовании
  флага `--files`.
* [BUG #1125](https://github.com/BurntSushi/ripgrep/issues/1125),
  [BUG #1159](https://github.com/BurntSushi/ripgrep/issues/1159):
  ripgrep не должен паниковать для `rg -h | rg` и должен выдавать корректный
  статус выхода.
* [BUG #1144](https://github.com/BurntSushi/ripgrep/issues/1144):
  Исправляет ошибку, из-за которой номера строк могли быть неверными на
  big-endian машинах.
* [BUG #1154](https://github.com/BurntSushi/ripgrep/issues/1154):
  Файлы Windows с атрибутом "hidden" теперь обрабатываются как скрытые.
* [BUG #1173](https://github.com/BurntSushi/ripgrep/issues/1173):
  Исправить обработку шаблонов `**` в файлах gitignore.
* [BUG #1174](https://github.com/BurntSushi/ripgrep/issues/1174):
  Исправить обработку повторяющихся шаблонов `**` в файлах gitignore.
* [BUG #1176](https://github.com/BurntSushi/ripgrep/issues/1176):
  Исправить ошибку, из-за которой `-F`/`-x` не применялись к шаблонам,
  данным через `-f`.
* [BUG #1189](https://github.com/BurntSushi/ripgrep/issues/1189):
  Документировать случаи, когда ripgrep может использовать много памяти.
* [BUG #1203](https://github.com/BurntSushi/ripgrep/issues/1203):
  Исправить ошибку сопоставления, связанную с оптимизацией суффиксных
  литералов.
* [BUG 8f14cb18](https://github.com/BurntSushi/ripgrep/commit/8f14cb18):
  Увеличить размер стека по умолчанию для JIT PCRE2.


0.10.0 (2018-09-07)
===================
Это новый незначительный выпуск ripgrep, который содержит некоторые крупные
новые функции, огромное количество исправлений ошибок и является первым
выпуском, основанным на libripgrep. Весь основной код поиска и вывода ripgrep
был переписан и обобщён, чтобы любой мог его использовать.

Основные новые функции включают поддержку PCRE2, многострочный поиск и формат
вывода JSON.

**КРУПНЫЕ ИЗМЕНЕНИЯ**:

* Минимальная версия, необходимая для компиляции Rust, теперь изменена, чтобы
  отслеживать последнюю стабильную версию Rust. Патч-выпуски продолжат
  компилироваться с той же версией Rust, что и предыдущий патч-выпуск, но
  новые незначительные версии будут использовать текущую стабильную версию
  компилятора Rust как свою минимальную поддерживаемую версию.
* Семантика сопоставления `-w/--word-regexp` несколько изменилась. Раньше
  это было `\b(?:<ваш шаблон>)\b`, но теперь это
  `(?:^|\W)(?:<ваш шаблон>)(?:$|\W)`. Это соответствует поведению GNU grep
  и считается более близким к предполагаемой семантике флага. Смотрите
  [#389](https://github.com/BurntSushi/ripgrep/issues/389) для более
  подробной информации.

Улучшения функциональности:

* [FEATURE #162](https://github.com/BurntSushi/ripgrep/issues/162):
  libripgrep теперь существует. Основной крейт —
  [`grep`](https://docs.rs/grep).
* [FEATURE #176](https://github.com/BurntSushi/ripgrep/issues/176):
  Добавить флаг `-U/--multiline`, который разрешает сопоставление по
  нескольким строкам.
* [FEATURE #188](https://github.com/BurntSushi/ripgrep/issues/188):
  Добавить флаг `-P/--pcre2`, который добавляет поддержку look-around и
  обратных ссылок.
* [FEATURE #244](https://github.com/BurntSushi/ripgrep/issues/244):
  Добавить флаг `--json`, который выводит результаты в формате JSON Lines.
* [FEATURE #321](https://github.com/BurntSushi/ripgrep/issues/321):
  Добавить флаг `--one-file-system` для пропуска каталогов на разных файловых
  системах.
* [FEATURE #404](https://github.com/BurntSushi/ripgrep/issues/404):
  Добавить флаги `--sort` и `--sortr` для большей сортировки. Удалить
  `--sort-files`.
* [FEATURE #416](https://github.com/BurntSushi/ripgrep/issues/416):
  Добавить флаг `--crlf` для разрешения работы `$` с возвратами каретки в
  Windows.
* [FEATURE #917](https://github.com/BurntSushi/ripgrep/issues/917):
  Флаг `--trim` удаляет префиксный пробел из всех выводимых строк.
* [FEATURE #993](https://github.com/BurntSushi/ripgrep/issues/993):
  Добавить флаг `--null-data`, который заставляет ripgrep использовать NUL
  как разделитель строк.
* [FEATURE #997](https://github.com/BurntSushi/ripgrep/issues/997):
  Флаг `--passthru` теперь работает с флагом `--replace`.
* [FEATURE #1038-1](https://github.com/BurntSushi/ripgrep/issues/1038):
  Добавить `--line-buffered` и `--block-buffered` для принудительной
  стратегии буферизации.
* [FEATURE #1038-2](https://github.com/BurntSushi/ripgrep/issues/1038):
  Добавить `--pre-glob` для фильтрации файлов через флаг `--pre`.

Исправления ошибок:

* [BUG #2](https://github.com/BurntSushi/ripgrep/issues/2):
  Поиск с ненулевым контекстом теперь может использовать memory maps, если
  это уместно.
* [BUG #200](https://github.com/BurntSushi/ripgrep/issues/200):
  ripgrep теперь корректно остановится, когда его выходной pipe закрыт.
* [BUG #389](https://github.com/BurntSushi/ripgrep/issues/389):
  Флаг `-w/--word-regexp` теперь работает более интуитивно.
* [BUG #643](https://github.com/BurntSushi/ripgrep/issues/643):
  Обнаружение читаемого stdin улучшено в Windows.
* [BUG #441](https://github.com/BurntSushi/ripgrep/issues/441),
  [BUG #690](https://github.com/BurntSushi/ripgrep/issues/690),
  [BUG #980](https://github.com/BurntSushi/ripgrep/issues/980):
  Сопоставление пустых строк теперь работает корректно в нескольких угловых
  случаях.
* [BUG #764](https://github.com/BurntSushi/ripgrep/issues/764):
  Цветовые escape-последовательности теперь объединяются, что уменьшает
  размер вывода.
* [BUG #842](https://github.com/BurntSushi/ripgrep/issues/842):
  Добавить man-страницу в двоичный пакет Debian.
* [BUG #922](https://github.com/BurntSushi/ripgrep/issues/922):
  ripgrep теперь более устойчив к сбоям memory maps.
* [BUG #937](https://github.com/BurntSushi/ripgrep/issues/937):
  Цветовые escape-последовательности больше не выдаются для пустых
  совпадений.
* [BUG #940](https://github.com/BurntSushi/ripgrep/issues/940):
  Контекст от флага `--passthru` не должен влиять на статус выхода процесса.
* [BUG #984](https://github.com/BurntSushi/ripgrep/issues/984):
  Исправляет ошибку в крейте `ignore`, где первый путь всегда обрабатывался
  как symlink.
* [BUG #990](https://github.com/BurntSushi/ripgrep/issues/990):
  Читать stderr асинхронно при запуске процесса.
* [BUG #1013](https://github.com/BurntSushi/ripgrep/issues/1013):
  Добавить функции CPU времени компиляции и выполнения в вывод `--version`.
* [BUG #1028](https://github.com/BurntSushi/ripgrep/pull/1028):
  Не дополнять голый шаблон после `-f` в zsh.


0.9.0 (2018-08-03)
==================
Это новый незначительный выпуск ripgrep, который содержит некоторые
незначительные новые функции и множество исправлений ошибок.

Выпуски, предоставленные на Github для `x86_64`, теперь будут работать на
всех целевых CPU и также будут автоматически использовать преимущества
функций, найденных на современных CPU (таких как AVX2) для дополнительной
оптимизации.

Этот выпуск увеличивает **минимальную поддерживаемую версию Rust** с 1.20.0
до 1.23.0.

Ожидается, что следующий выпуск ripgrep (0.10.0) предоставит поддержку
многострочного поиска и формат вывода JSON.

**КРУПНЫЕ ИЗМЕНЕНИЯ**:

* Когда `--count` и `--only-matching` предоставлены одновременно, поведение
  ripgrep таково, как если бы был дан флаг `--count-matches`. То есть
  сообщается общее количество совпадений, где может быть несколько
  совпадений на строку. Ранее поведение ripgrep заключалось в сообщении
  общего количества совпадающих строк. (Обратите внимание, что это поведение
  расходится с поведением GNU grep.)
* Восьмеричный синтаксис больше не поддерживается. Ранее ripgrep принимал
  выражения вроде `\1` как синтаксис для сопоставления `U+0001`, но теперь
  ripgrep вместо этого выдаст ошибку.
* Флаг `--line-number-width` был удалён. Его функциональность не была
  тщательно рассмотрена со всеми форматами вывода ripgrep.
  Смотрите [#795](https://github.com/BurntSushi/ripgrep/issues/795) для
  более подробной информации.

Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для Android, Bazel, Fuchsia,
  Haskell, Java и Puppet.
* [FEATURE #411](https://github.com/BurntSushi/ripgrep/issues/411):
  Добавить флаг `--stats`, который выдаёт агрегированную статистику после
  результатов поиска.
* [FEATURE #646](https://github.com/BurntSushi/ripgrep/issues/646):
  Добавить флаг `--no-ignore-messages`, который подавляет ошибки парсинга
  от чтения файлов `.ignore` и `.gitignore`.
* [FEATURE #702](https://github.com/BurntSushi/ripgrep/issues/702):
  Поддержка Unicode escape-последовательностей `\u{..}`.
* [FEATURE #812](https://github.com/BurntSushi/ripgrep/issues/812):
  Добавить флаг `-b/--byte-offset`, который показывает смещение байт каждой
  совпадающей строки.
* [FEATURE #814](https://github.com/BurntSushi/ripgrep/issues/814):
  Добавить флаг `--count-matches`, который похож на `--count`, но для каждого
  совпадения.
* [FEATURE #880](https://github.com/BurntSushi/ripgrep/issues/880):
  Добавить флаг `--no-column`, который отключает номера столбцов в выводе.
* [FEATURE #898](https://github.com/BurntSushi/ripgrep/issues/898):
  Добавить поддержку `lz4` при использовании флага `-z/--search-zip`.
* [FEATURE #924](https://github.com/BurntSushi/ripgrep/issues/924):
  `termcolor` перемещён в свой собственный репозиторий:
  https://github.com/BurntSushi/termcolor
* [FEATURE #934](https://github.com/BurntSushi/ripgrep/issues/934):
  Добавить новый флаг `--no-ignore-global`, который разрешает отключение
  глобальных gitignore.
* [FEATURE #967](https://github.com/BurntSushi/ripgrep/issues/967):
  Переименовать `--maxdepth` в `--max-depth` для согласованности. Сохранить
  `--maxdepth` для обратной совместимости.
* [FEATURE #978](https://github.com/BurntSushi/ripgrep/issues/978):
  Добавить опцию `--pre` для фильтрации входов произвольной программой.
* [FEATURE fca9709d](https://github.com/BurntSushi/ripgrep/commit/fca9709d):
  Улучшить автодополнение zsh.

Исправления ошибок:

* [BUG #135](https://github.com/BurntSushi/ripgrep/issues/135):
  Выпускать портативные двоичные файлы, которые условно используют SSSE3,
  AVX2 и т.д. во время выполнения.
* [BUG #268](https://github.com/BurntSushi/ripgrep/issues/268):
  Выводить описательное сообщение об ошибке при попытке использовать
  look-around или обратные ссылки.
* [BUG #395](https://github.com/BurntSushi/ripgrep/issues/395):
  Показывать понятные сообщения об ошибках для regex вроде `\s*{`.
* [BUG #526](https://github.com/BurntSushi/ripgrep/issues/526):
  Поддержка backslash-экранирования в глобах.
* [BUG #795](https://github.com/BurntSushi/ripgrep/issues/795):
  Исправить проблемы с `--line-number-width` путём его удаления.
* [BUG #832](https://github.com/BurntSushi/ripgrep/issues/832):
  Уточнить инструкции использования для флага `-f/--file`.
* [BUG #835](https://github.com/BurntSushi/ripgrep/issues/835):
  Исправить небольшую регрессию производительности при обходе очень больших
  деревьев каталогов.
* [BUG #851](https://github.com/BurntSushi/ripgrep/issues/851):
  Исправить обнаружение `-S/--smart-case` раз и навсегда.
* [BUG #852](https://github.com/BurntSushi/ripgrep/issues/852):
  Быть устойчивым к ошибкам `ENOMEM`, возвращаемым `mmap`.
* [BUG #853](https://github.com/BurntSushi/ripgrep/issues/853):
  Обновить крейт `grep` до `regex-syntax 0.6.0`.
* [BUG #893](https://github.com/BurntSushi/ripgrep/issues/893):
  Улучшить поддержку подмодулей git.
* [BUG #900](https://github.com/BurntSushi/ripgrep/issues/900):
  Когда шаблоны не даны, ripgrep никогда не должен ничего сопоставлять.
* [BUG #907](https://github.com/BurntSushi/ripgrep/issues/907):
  ripgrep теперь остановит обход после первого файла, когда используется
  `--quiet --files`.
* [BUG #918](https://github.com/BurntSushi/ripgrep/issues/918):
  Не пропускать tar-архивы, когда используется `-z/--search-zip`.
* [BUG #934](https://github.com/BurntSushi/ripgrep/issues/934):
  Не соблюдать файлы gitignore при поиске вне git-репозиториев.
* [BUG #948](https://github.com/BurntSushi/ripgrep/issues/948):
  Использовать код выхода 2 для указания ошибки и использовать код выхода 1
  для указания отсутствия совпадений.
* [BUG #951](https://github.com/BurntSushi/ripgrep/issues/951):
  Добавить пример stdin в документацию использования ripgrep.
* [BUG #955](https://github.com/BurntSushi/ripgrep/issues/955):
  Использовать буферизованную запись при выводе не в tty, что исправляет
  регрессию производительности.
* [BUG #957](https://github.com/BurntSushi/ripgrep/issues/957):
  Улучшить сообщение об ошибке, показываемое для `--path separator /` в
  некоторых оболочках Windows.
* [BUG #964](https://github.com/BurntSushi/ripgrep/issues/964):
  Добавить флаг `--no-fixed-strings` для отключения `-F/--fixed-strings`.
* [BUG #988](https://github.com/BurntSushi/ripgrep/issues/988):
  Исправить ошибку в крейте `ignore`, которая предотвращала использование
  явных файлов ignore после отключения всех других правил ignore.
* [BUG #995](https://github.com/BurntSushi/ripgrep/issues/995):
  Соблюдать `$XDG_CONFIG_DIR/git/config` для обнаружения `core.excludesFile`.


0.8.1 (2018-02-20)
==================
Это патч-выпуск ripgrep, который в основном исправляет регрессии, появившиеся
в 0.8.0 (#820 и #824) в обходе каталогов в Windows. Эти регрессии не влияют
на пользователей не-Windows.

Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для csv и VHDL.
* [FEATURE #798](https://github.com/BurntSushi/ripgrep/issues/798):
  Добавить поддержку `underline` для `termcolor` и ripgrep. Смотрите
  документацию о флаге `--colors` для деталей.

Исправления ошибок:

* [BUG #684](https://github.com/BurntSushi/ripgrep/issues/684):
  Улучшить документацию для флага `--ignore-file`.
* [BUG #789](https://github.com/BurntSushi/ripgrep/issues/789):
  Не показывать `(rev )`, если ревизия не была доступна во время сборки.
* [BUG #791](https://github.com/BurntSushi/ripgrep/issues/791):
  Добавить man-страницу в выпуск ARM.
* [BUG #797](https://github.com/BurntSushi/ripgrep/issues/797):
  Улучшить документацию для настройки "intense" в `termcolor`.
* [BUG #800](https://github.com/BurntSushi/ripgrep/issues/800):
  Исправить ошибку в крейте `ignore` для пользовательских файлов ignore.
  Это не повлияло на ripgrep.
* [BUG #807](https://github.com/BurntSushi/ripgrep/issues/807):
  Исправить ошибку, из-за которой `rg --hidden .` вёл себя иначе, чем
  `rg --hidden ./`.
* [BUG #815](https://github.com/BurntSushi/ripgrep/issues/815):
  Уточнить распространённый режим отказа в руководстве пользователя.
* [BUG #820](https://github.com/BurntSushi/ripgrep/issues/820):
  Исправляет ошибку в Windows, где symlinks отслеживались, даже если не
  запрашивались.
* [BUG #824](https://github.com/BurntSushi/ripgrep/issues/824):
  Исправить регрессию производительности в обходе каталогов в Windows.


0.8.0 (2018-02-11)
==================
Это новый незначительный выпуск ripgrep, который удовлетворяет нескольким
популярным запросам функций (файлы конфигурации, поиск сжатых файлов,
true colors), исправляет множество ошибок и улучшает качество жизни для
сопровождающих ripgrep. Этот выпуск также включает значительно улучшенную
документацию в виде
[Руководства пользователя](GUIDE.md) и [FAQ](FAQ.md).

Этот выпуск увеличивает **минимальную поддерживаемую версию Rust** с 1.17 до
1.20.

**КРУПНЫЕ ИЗМЕНЕНИЯ**:

Обратите внимание, что все они очень незначительны и вряд ли повлияют на
большинство пользователей.

* Для поддержки файлов конфигурации переопределения флагов нужно было
  переосмыслить. В некоторых случаях это изменило поведение ripgrep.
  Например, в ripgrep 0.7.1 `rg foo -s -i` выполнит регистрозависимый поиск,
  поскольку флаг `-s/--case-sensitive` был определён как всегда имеющий
  приоритет над флагом `-i/--ignore-case`, независимо от позиции. В ripgrep
  0.8.0, однако, правило переопределения для всех флагов изменено на
  «последний флаг побеждает среди конкурирующих флагов». То есть
  `rg foo -s -i` теперь выполняет регистронезависимый поиск.
* Флаг `-M/--max-columns` был изменён так, что указание значения `0` теперь
  заставляет ripgrep вести себя так, как если бы флаг отсутствовал. Это
  делает возможным установить значение по умолчанию в файле конфигурации и
  затем переопределить его. Предыдущее поведение ripgrep заключалось в
  подавлении всех совпадающих непустых строк.
* Во всех глобах `[^...]` теперь эквивалентно `[!...]` (указывая на
  отрицание класса). Ранее `^` не имел специального значения в классе
  символов.
* Для **downstream-упаковщиков** иерархия каталогов в архивных выпусках
  ripgrep изменилась. Корневой каталог теперь содержит только исполняемый
  файл, README и лицензию. Теперь есть новый каталог `doc`, который содержит
  man-страницу (ранее в корне), руководство пользователя (новое), FAQ (новое)
  и CHANGELOG (ранее не включался в выпуск). Каталог `complete` остаётся
  тем же.

Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для
  Apache Avro, C++, GN, Google Closure Templates, Jupyter notebooks,
  man pages, Protocol Buffers, Smarty и Web IDL.
* [FEATURE #196](https://github.com/BurntSushi/ripgrep/issues/196):
  Поддержка файла конфигурации. Смотрите
  [новое руководство пользователя](GUIDE.md#configuration-file)
  для деталей.
* [FEATURE #261](https://github.com/BurntSushi/ripgrep/issues/261):
  Добавить расширенную или "true" поддержку цвета. Работает в Windows 10!
  [Смотрите FAQ для деталей.](FAQ.md#colors)
* [FEATURE #539](https://github.com/BurntSushi/ripgrep/issues/539):
  Искать файлы gzip, bzip2, lzma или xz при использовании флага
  `-z/--search-zip`.
* [FEATURE #544](https://github.com/BurntSushi/ripgrep/issues/544):
  Добавить поддержку выравнивания номеров строк через новый флаг
  `--line-number-width`.
* [FEATURE #654](https://github.com/BurntSushi/ripgrep/pull/654):
  Поддержка linuxbrew в Brew tap ripgrep.
* [FEATURE #673](https://github.com/BurntSushi/ripgrep/issues/673):
  Вернуть файлы `.rgignore`. (Более высокий приоритет, специфичная для
  приложения версия `.ignore`.)
* [FEATURE #676](https://github.com/BurntSushi/ripgrep/issues/676):
  Предоставить двоичные файлы ARM. **ПРЕДУПРЕЖДЕНИЕ:** Это будет
  предоставляться на основе best effort.
* [FEATURE #709](https://github.com/BurntSushi/ripgrep/issues/709):
  Предложить флаг `-F/--fixed-strings` при ошибке синтаксиса regex.
* [FEATURE #740](https://github.com/BurntSushi/ripgrep/issues/740):
  Добавить флаг `--passthru`, который заставляет ripgrep печатать каждую
  строку, которую он читает.
* [FEATURE #785](https://github.com/BurntSushi/ripgrep/pull/785):
  Переработать документацию. Очистить README, добавить руководство
  пользователя и FAQ.
* [FEATURE 7f5c07](https://github.com/BurntSushi/ripgrep/commit/7f5c07434be92103b5bf7e216b9c7494aed2d8cb):
  Добавить скрытые флаги для удобных переопределений (например, `--no-text`).

Исправления ошибок:

* [BUG #553](https://github.com/BurntSushi/ripgrep/issues/553):
  Разрешить повторение флагов.
* [BUG #633](https://github.com/BurntSushi/ripgrep/issues/633):
  Исправить ошибку, из-за которой ripgrep паниковал в Windows при
  отслеживании symlinks.
* [BUG #649](https://github.com/BurntSushi/ripgrep/issues/649):
  Исправить обработку `!**/` в `.gitignore`.
* [BUG #663](https://github.com/BurntSushi/ripgrep/issues/663):
  **КРУПНОЕ ИЗМЕНЕНИЕ:** Поддержка синтаксиса глоба `[^...]` (как
  идентичного `[!...]`).
* [BUG #693](https://github.com/BurntSushi/ripgrep/issues/693):
  Не отображать разделители контекста, когда совпадения не печатаются.
* [BUG #705](https://github.com/BurntSushi/ripgrep/issues/705):
  Исправить ошибку, которая предотвращала ripgrep от поиска каталогов
  OneDrive.
* [BUG #717](https://github.com/BurntSushi/ripgrep/issues/717):
  Улучшить обнаружение символов верхнего регистра `--smart-case`.
* [BUG #725](https://github.com/BurntSushi/ripgrep/issues/725):
  Уточнить, что глобы не переопределяют явно данные пути для поиска.
* [BUG #742](https://github.com/BurntSushi/ripgrep/pull/742):
  Записывать код сброса ANSI как `\x1B[0m` вместо `\x1B[m`.
* [BUG #747](https://github.com/BurntSushi/ripgrep/issues/747):
  Удалить `yarn.lock` из типа файла YAML.
* [BUG #760](https://github.com/BurntSushi/ripgrep/issues/760):
  ripgrep теперь может искать файлы
  `/sys/devices/system/cpu/vulnerabilities/*`.
* [BUG #761](https://github.com/BurntSushi/ripgrep/issues/761):
  Исправить обработку шаблонов gitignore, содержащих `/`.
* [BUG #776](https://github.com/BurntSushi/ripgrep/pull/776):
  **КРУПНОЕ ИЗМЕНЕНИЕ:** `--max-columns=0` теперь отключает лимит.
* [BUG #779](https://github.com/BurntSushi/ripgrep/issues/779):
  Уточнить документацию для `--files-without-match`.
* [BUG #780](https://github.com/BurntSushi/ripgrep/issues/780),
  [BUG #781](https://github.com/BurntSushi/ripgrep/issues/781):
  Исправить ошибку, из-за которой ripgrep пропускал некоторые совпадающие
  строки.

Исправления обслуживания:

* [MAINT #772](https://github.com/BurntSushi/ripgrep/pull/772):
  Отказаться от `env_logger` в пользу более простого logger, чтобы избежать
  многих новых зависимостей.
* [MAINT #772](https://github.com/BurntSushi/ripgrep/pull/772):
  Добавить хэш ревизии git в строку версии ripgrep.
* [MAINT #772](https://github.com/BurntSushi/ripgrep/pull/772):
  (По-видимому) улучшить время компиляции.
* [MAINT #776](https://github.com/BurntSushi/ripgrep/pull/776):
  Автоматически генерировать man-страницу во время сборки.
* [MAINT #786](https://github.com/BurntSushi/ripgrep/pull/786):
  Удалить использование `unsafe` в `globset`. :tada:
* [MAINT e9d448](https://github.com/BurntSushi/ripgrep/commit/e9d448e93bb4e1fb3b0c1afc29adb5af6ed5283d):
  Добавить шаблон issue (уже резко улучшил отчёты об ошибках).
* [MAINT ae2d03](https://github.com/BurntSushi/ripgrep/commit/ae2d036dd4ba2a46acac9c2d77c32e7c667eb850):
  Удалить скрипт `compile`.

Друзья ripgrep:

Я хочу выразить свою благодарность
[@balajisivaraman](https://github.com/balajisivaraman)
за их недавнюю тяжёлую работу в ряде областей, и в частности, за реализацию
функции "поиск сжатых файлов". Их работа в наброске спецификации для этого
и другой работы была образцовой.

Спасибо
[@balajisivaraman](https://github.com/balajisivaraman)!


0.7.1 (2017-10-22)
==================
Это патч-выпуск ripgrep, который включает исправление очень плохой регрессии,
появившейся в ripgrep 0.7.0.

Исправления ошибок:

* [BUG #648](https://github.com/BurntSushi/ripgrep/issues/648):
  Исправить ошибку, из-за которой было очень легко превысить стандартные
  лимиты файловых дескрипторов.


0.7.0 (2017-10-20)
==================
Это новый незначительный выпуск ripgrep, который включает в основном
исправления ошибок.

ripgrep продолжает требовать Rust 1.17, и в этом выпуске нет известных
критических изменений.

Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для файлов конфигурации и
  лицензии, Elm, Purescript, Standard ML, sh, systemd, Terraform
* [FEATURE #593](https://github.com/BurntSushi/ripgrep/pull/593):
  Использование одновременно `-o/--only-matching` и `-r/--replace` делает
  правильную вещь.

Исправления ошибок:

* [BUG #200](https://github.com/BurntSushi/ripgrep/issues/200):
  ripgrep остановится, когда его pipe закрыт.
* [BUG #402](https://github.com/BurntSushi/ripgrep/issues/402):
  Исправить ошибку печати контекста, когда используется флаг
  `-m/--max-count`.
* [BUG #521](https://github.com/BurntSushi/ripgrep/issues/521):
  Исправить взаимодействие между `-r/--replace` и цветами терминала.
* [BUG #559](https://github.com/BurntSushi/ripgrep/issues/559):
  Игнорировать тест, который пытался читать путь файла не-UTF-8 на macOS.
* [BUG #599](https://github.com/BurntSushi/ripgrep/issues/599):
  Исправить цветовые escape-последовательности на пустых совпадениях.
* [BUG #600](https://github.com/BurntSushi/ripgrep/issues/600):
  Избегать дорогой (в Windows) проверки дескриптора файла при использовании
  --files.
* [BUG #618](https://github.com/BurntSushi/ripgrep/issues/618):
  Уточнить инструкции по установке для пользователей Ubuntu.
* [BUG #633](https://github.com/BurntSushi/ripgrep/issues/633):
  Более быстрая проверка цикла symlinks в Windows.


0.6.0 (2017-08-23)
==================
Это новый незначительный выпуск ripgrep, который включает множество
исправлений ошибок и несколько новых функций, таких как `--iglob` и
`-x/--line-regexp`.

Обратите внимание, что этот выпуск увеличивает минимальную поддерживаемую
версию Rust с 1.12 до 1.17.

Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для BitBake, C++, Cabal,
  cshtml, Julia, Make, msbuild, QMake, Yocto
* [FEATURE #163](https://github.com/BurntSushi/ripgrep/issues/163):
  Добавить флаг `--iglob`, который похож на `-g/--glob`, но сопоставляет
  глобы регистронезависимо.
* [FEATURE #520](https://github.com/BurntSushi/ripgrep/pull/518):
  Добавить флаг `-x/--line-regexp`, который требует, чтобы совпадение
  охватывало всю строку.
* [FEATURE #551](https://github.com/BurntSushi/ripgrep/pull/551),
  [FEATURE #554](https://github.com/BurntSushi/ripgrep/pull/554):
  `ignore`: добавить новый метод `matched_path_or_any_parents`.

Исправления ошибок:

* [BUG #342](https://github.com/BurntSushi/ripgrep/issues/342):
  Исправить невидимый текст в некоторых средах PowerShell путём изменения
  схемы цветов по умолчанию в Windows.
* [BUG #413](https://github.com/BurntSushi/ripgrep/issues/413):
  Двоичные файлы выпуска на Unix теперь `strip`'ятся по умолчанию. Это
  уменьшает размер двоичного файла на порядок.
* [BUG #483](https://github.com/BurntSushi/ripgrep/issues/483):
  Когда передан `--quiet`, `--files` должен быть тихим.
* [BUG #488](https://github.com/BurntSushi/ripgrep/pull/488):
  Когда передан `--vimgrep`, `--with-filename` должен быть включён
  автоматически.
* [BUG #493](https://github.com/BurntSushi/ripgrep/issues/493):
  Исправить ещё одну ошибку в реализации флага `-o/--only-matching`.
* [BUG #499](https://github.com/BurntSushi/ripgrep/pull/499):
  Разрешить определённым флагам переопределять другие.
* [BUG #523](https://github.com/BurntSushi/ripgrep/pull/523):
  `wincolor`: Повторно получать консоль Windows при всех вызовах.
* [BUG #523](https://github.com/BurntSushi/ripgrep/issues/524):
  `--version` теперь показывает включённые функции времени компиляции.
* [BUG #532](https://github.com/BurntSushi/ripgrep/issues/532),
  [BUG #536](https://github.com/BurntSushi/ripgrep/pull/536),
  [BUG #538](https://github.com/BurntSushi/ripgrep/pull/538),
  [BUG #540](https://github.com/BurntSushi/ripgrep/pull/540),
  [BUG #560](https://github.com/BurntSushi/ripgrep/pull/560),
  [BUG #565](https://github.com/BurntSushi/ripgrep/pull/565):
  Улучшить автодополнение zsh.
* [BUG #578](https://github.com/BurntSushi/ripgrep/pull/578):
  Включить SIMD для `encoding_rs`, когда это уместно.
* [BUG #580](https://github.com/BurntSushi/ripgrep/issues/580):
  Исправить `-w/--word-regexp` в присутствии групп захвата.
* [BUG #581](https://github.com/BurntSushi/ripgrep/issues/581):
  Документировать, что ripgrep может неожиданно завершиться при поиске через
  memory maps (что может происходить при использовании настроек по умолчанию).

Друзья ripgrep:

Я хочу выразить большую благодарность @okdana за их недавнюю тяжёлую работу
над ripgrep. Это включает новые функции, такие как `--line-regexp`, героические
усилия по автодополнению zsh и обдумывание со мной некоторых колючих проблем
argv.

Я также хочу поблагодарить @ericbn за их работу по улучшению парсинга argv
ripgrep путём разрешения некоторым флагам переопределять другие.

Спасибо @okdana и @ericbn!


0.5.2 (2017-05-11)
==================
Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для Nix.
* [FEATURE #362](https://github.com/BurntSushi/ripgrep/issues/362):
  Добавить флаги `--regex-size-limit` и `--dfa-size-limit`.
* [FEATURE #444](https://github.com/BurntSushi/ripgrep/issues/444):
  Улучшить сообщения об ошибках для недопустимых глобов.

Исправления ошибок:

* [BUG #442](https://github.com/BurntSushi/ripgrep/issues/442):
  Исправить перенос строк в выводе `--help`.
* [BUG #451](https://github.com/BurntSushi/ripgrep/issues/451):
  Исправить ошибку с дублирующимся выводом при использовании флага
  `-o/--only-matching`.


0.5.1 (2017-04-09)
==================
Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для vim.
* [FEATURE #34](https://github.com/BurntSushi/ripgrep/issues/34):
  Добавить флаг `-o/--only-matching`.
* [FEATURE #377](https://github.com/BurntSushi/ripgrep/issues/377):
  Номера столбцов теперь можно настроить с помощью цвета. (По умолчанию —
  без цвета.)
* [FEATURE #419](https://github.com/BurntSushi/ripgrep/issues/419):
  Добавлена короткая опция флага `-0` для `--null`.

Исправления ошибок:

* [BUG #381](https://github.com/BurntSushi/ripgrep/issues/381):
  Включить текст лицензии во все подкрейты.
* [BUG #418](https://github.com/BurntSushi/ripgrep/issues/418),
  [BUG #426](https://github.com/BurntSushi/ripgrep/issues/426),
  [BUG #439](https://github.com/BurntSushi/ripgrep/issues/439):
  Исправить несколько ошибок с выводом `-h/--help`.


0.5.0 (2017-03-12)
==================
Это новый незначительный выпуск ripgrep, который включает одно незначительное
критическое изменение, исправления ошибок и несколько новых функций, включая
поддержку текстовых кодировок, отличных от UTF-8.

Заметным достижением в отношении Rust является то, что сам ripgrep теперь
содержит только одно использование `unsafe` (для доступа к содержимому
memory map).

**Критическое изменение**:

* [FEATURE #380](https://github.com/BurntSushi/ripgrep/issues/380):
  Номера строк теперь скрыты по умолчанию, когда ripgrep выводит в tty
  **и** единственное, что ищется — это stdin.

Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для Ceylon, CSS, Elixir,
  HTML, log, SASS, SVG, Twig
* [FEATURE #1](https://github.com/BurntSushi/ripgrep/issues/1):
  Добавить поддержку дополнительных текстовых кодировок, включая
  автоматическое обнаружение для UTF-16 через BOM sniffing. Явная поддержка
  текстовой кодировки с флагом `-E/--encoding` также была добавлена для
  latin-1, GBK, EUC-JP и Shift_JIS, среди прочих. Полный список можно найти
  здесь: https://encoding.spec.whatwg.org/#concept-encoding-get
* [FEATURE #129](https://github.com/BurntSushi/ripgrep/issues/129):
  Добавить новый флаг `-M/--max-columns`, который пропускает строки длиннее
  данного количества байт. (Отключено по умолчанию!)
* [FEATURE #369](https://github.com/BurntSushi/ripgrep/issues/369):
  Новый флаг `--max-filesize` был добавлен для ограничения поиска файлами с
  максимальным размером файла.

Исправления ошибок:

* [BUG #52](https://github.com/BurntSushi/ripgrep/issues/52),
  [BUG #311](https://github.com/BurntSushi/ripgrep/issues/311):
  Настроить, как обнаруживаются и обрабатываются двоичные файлы. (Мы стали
  немного менее консервативными и больше не будем использовать память без
  границ.)
* [BUG #326](https://github.com/BurntSushi/ripgrep/issues/326):
  Когда дан флаг --files, мы никогда не должны пытаться разбирать позиционные
  аргументы как regex.
* [BUG #327](https://github.com/BurntSushi/ripgrep/issues/327):
  Разрешить флагу --heading переопределять флаг --no-heading.
* [BUG #340](https://github.com/BurntSushi/ripgrep/pull/340):
  Уточнить, что флаги `-u/--unrestricted` являются псевдонимами.
* [BUG #343](https://github.com/BurntSushi/ripgrep/pull/343):
  Глобальная конфигурация git ignore должна использовать
  `$HOME/.config/git/ignore`, а не `$HOME/git/ignore`.
* [BUG #345](https://github.com/BurntSushi/ripgrep/pull/345):
  Уточнить документацию для флага `-g/--glob`.
* [BUG #381](https://github.com/BurntSushi/ripgrep/issues/381):
  Добавить файлы лицензии в каждый подкрейт.
* [BUG #383](https://github.com/BurntSushi/ripgrep/issues/383):
  Использовать последнюю версию clap (для парсинга argv).
* [BUG #392](https://github.com/BurntSushi/ripgrep/issues/391):
  Исправить трансляцию set-глобов (например, `{foo,bar,quux}`) в regex.
* [BUG #401](https://github.com/BurntSushi/ripgrep/pull/401):
  Добавить файл автодополнения PowerShell в выпуск Windows.
* [BUG #405](https://github.com/BurntSushi/ripgrep/issues/405):
  Исправить ошибку при исключении абсолютных путей с флагом `-g/--glob`.


0.4.0
=====
Это новый незначительный выпуск ripgrep, который включает пару очень
незначительных критических изменений, несколько новых функций и множество
исправлений ошибок.

Эта версия ripgrep обновляет свою зависимость `regex` с `0.1` до `0.2`,
что включает несколько незначительных изменений синтаксиса:

* POSIX character classes теперь требуют двойных скобок. Ранее regex
  `[:upper:]` разбирался как POSIX character class `upper`. Теперь он
  разбирается как класс символов, содержащий символы `:upper:`. Исправление
  этого изменения — использовать `[[:upper:]]` вместо. Обратите внимание,
  что варианты вроде `[[:upper:][:blank:]]` продолжают работать.
* Символ `[` всегда должен быть экранирован внутри класса символов.
* Символы `&`, `-` и `~` должны быть экранированы, если любой из них
  повторяется последовательно. Например, `[&]`, `[\&]`, `[\&\&]`, `[&-&]`
  все эквивалентны, в то время как `[&&]` незаконно. (Мотивация для этого
  и предыдущего изменения — предоставить обратно совместимый путь для
  добавления нотации наборов классов символов.)

Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для Crystal, Kotlin, Perl,
  PowerShell, Ruby, Swig
* [FEATURE #83](https://github.com/BurntSushi/ripgrep/issues/83):
  Определения типов теперь могут включать другие определения типов.
* [FEATURE #243](https://github.com/BurntSushi/ripgrep/issues/243):
  **КРУПНОЕ ИЗМЕНЕНИЕ**: Флаг `--column` теперь подразумевает `--line-number`.
* [FEATURE #263](https://github.com/BurntSushi/ripgrep/issues/263):
  Добавить новый флаг `--sort-files`.
* [FEATURE #275](https://github.com/BurntSushi/ripgrep/issues/275):
  Добавить новый флаг `--path-separator`. Полезно в cygwin.

Исправления ошибок:

* [BUG #182](https://github.com/BurntSushi/ripgrep/issues/182):
  Redux: использовать более портативные ANSI color escape sequences, когда
  это возможно.
* [BUG #258](https://github.com/BurntSushi/ripgrep/issues/258):
  Исправить ошибку, из-за которой параллельный итератор ripgrep вращался и
  сжигал CPU.
* [BUG #262](https://github.com/BurntSushi/ripgrep/issues/262):
  Документировать, как устанавливать файлы автодополнения оболочки.
* [BUG #266](https://github.com/BurntSushi/ripgrep/issues/266),
  [BUG #293](https://github.com/BurntSushi/ripgrep/issues/293):
  Исправить обработку жирного стиля и изменить цвета по умолчанию.
* [BUG #268](https://github.com/BurntSushi/ripgrep/issues/268):
  Сделать отсутствие поддержки обратных ссылок более явным.
* [BUG #271](https://github.com/BurntSushi/ripgrep/issues/271):
  Удалить зависимость `~` на clap.
* [BUG #277](https://github.com/BurntSushi/ripgrep/issues/277):
  Исправить косметическую проблему в документации крейта `globset`.
* [BUG #279](https://github.com/BurntSushi/ripgrep/issues/279):
  ripgrep не завершался, когда был дан `-q/--quiet`.
* [BUG #281](https://github.com/BurntSushi/ripgrep/issues/281):
  **КРУПНОЕ ИЗМЕНЕНИЕ**: Полностью удалить обработку `^C` из ripgrep.
* [BUG #284](https://github.com/BurntSushi/ripgrep/issues/284):
  Сделать документацию для `-g/--glob` более ясной.
* [BUG #286](https://github.com/BurntSushi/ripgrep/pull/286):
  Когда stdout перенаправлен в файл, не искать этот файл.
* [BUG #287](https://github.com/BurntSushi/ripgrep/pull/287):
  Исправить автодополнения ZSH.
* [BUG #295](https://github.com/BurntSushi/ripgrep/pull/295):
  Удалить избыточную зависимость `memmap` в крейте `grep`.
* [BUG #308](https://github.com/BurntSushi/ripgrep/pull/308):
  Улучшить документацию для `-r/--replace`.
* [BUG #313](https://github.com/BurntSushi/ripgrep/pull/313):
  Обновить зависимость bytecount до последней версии.
* [BUG #318](https://github.com/BurntSushi/ripgrep/pull/318):
  Исправить ошибку вывода недопустимого UTF-8 в консолях Windows.


0.3.2
=====
Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для Less, Sass, stylus, Zsh

Исправления ошибок:

* [BUG #229](https://github.com/BurntSushi/ripgrep/issues/229):
  Сделать smart case немного менее консервативным.
* [BUG #247](https://github.com/BurntSushi/ripgrep/issues/247):
  Уточнить использование --heading/--no-heading.
* [BUG #251](https://github.com/BurntSushi/ripgrep/issues/251),
  [BUG #264](https://github.com/BurntSushi/ripgrep/issues/264),
  [BUG #267](https://github.com/BurntSushi/ripgrep/issues/267):
  Исправить ошибку сопоставления, вызванную оптимизациями литералов.
* [BUG #256](https://github.com/BurntSushi/ripgrep/issues/256):
  Исправить ошибку, из-за которой `rg foo` и `rg foo/` имели разное
  поведение, когда `foo` был symlink.
* [BUG #270](https://github.com/BurntSushi/ripgrep/issues/270):
  Исправить ошибку, из-за которой шаблоны, начинающиеся с `-`, не могли
  использоваться с флагом `-e/--regexp`. (Это решает регрессию, которая
  появилась в ripgrep 0.3.0.)


0.3.1
=====
Исправления ошибок:

* [BUG #242](https://github.com/BurntSushi/ripgrep/issues/242):
  ripgrep не соблюдал `--colors foo:none` корректно. Теперь соблюдает.


0.3.0
=====
Это новый незначительный выпуск ripgrep, который включает два критических
изменения с множеством исправлений ошибок и некоторыми новыми функциями и
улучшениями производительности. В частности, если у вас была проблема с
цветами или pipe в Windows раньше, то теперь это должно быть исправлено в
этом выпуске.

**КРУПНЫЕ ИЗМЕНЕНИЯ**:

* ripgrep теперь требует Rust 1.11 для компиляции. Ранее он мог
  компилироваться на Rust 1.9. Причиной этого был переход от
  [Docopt к Clap](https://github.com/BurntSushi/ripgrep/pull/233)
  для парсинга аргументов.
* Флаг `-e/--regexp` больше не может принимать шаблон, начинающийся с `-`.
  Есть два обходных пути: `rg -- -foo` и `rg [-]foo` или `rg -e [-]foo` —
  все будут искать один и тот же шаблон `-foo`. Причиной этого был переход
  от [Docopt к Clap](https://github.com/BurntSushi/ripgrep/pull/233)
  для парсинга аргументов.
  [Это может быть исправлено в
  будущем.](https://github.com/kbknapp/clap-rs/issues/742).

Улучшения производительности:

* [PERF #33](https://github.com/BurntSushi/ripgrep/issues/33):
  ripgrep теперь работает аналогично GNU grep на малых корпусах.
* [PERF #136](https://github.com/BurntSushi/ripgrep/issues/136):
  ripgrep больше не замедляется из-за парсинга аргументов при большом
  списке аргументов.

Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для Elixir.
* [FEATURE #7](https://github.com/BurntSushi/ripgrep/issues/7):
  Добавить флаг `-f/--file`, который заставляет ripgrep читать шаблоны из
  файла.
* [FEATURE #51](https://github.com/BurntSushi/ripgrep/issues/51):
  Добавить флаг `--colors`, который позволяет настроить цвета,
  используемые в выводе ripgrep.
* [FEATURE #138](https://github.com/BurntSushi/ripgrep/issues/138):
  Добавить флаг `--files-without-match`, который показывает только пути к
  файлам, которые содержат ноль совпадений.
* [FEATURE #230](https://github.com/BurntSushi/ripgrep/issues/230):
  Добавить файлы автодополнения в выпуск (Bash, Fish и PowerShell).

Исправления ошибок:

* [BUG #37](https://github.com/BurntSushi/ripgrep/issues/37):
  Использовать корректные ANSI escape sequences, когда `TERM=screen.linux`.
* [BUG #94](https://github.com/BurntSushi/ripgrep/issues/94):
  ripgrep теперь автоматически обнаруживает stdin в Windows.
* [BUG #117](https://github.com/BurntSushi/ripgrep/issues/117):
  Цвета теперь должны работать корректно и автоматически внутри mintty.
* [BUG #182](https://github.com/BurntSushi/ripgrep/issues/182):
  Цвета теперь должны работать внутри Emacs. В частности, `--color=always`
  будет выдавать цвета независимо от текущей среды.
* [BUG #189](https://github.com/BurntSushi/ripgrep/issues/189):
  Показывать меньше контента при запуске `rg -h`. Полный контент справки
  можно получить с `rg --help`.
* [BUG #210](https://github.com/BurntSushi/ripgrep/issues/210):
  Поддержка имён файлов не-UTF-8 на платформах Unix.
* [BUG #231](https://github.com/BurntSushi/ripgrep/issues/231):
  Переключиться с блочной буферизации на строковую.
* [BUG #241](https://github.com/BurntSushi/ripgrep/issues/241):
  Некоторые сообщения об ошибках не подавлялись, когда использовался
  `--no-messages`.


0.2.9
=====
Исправления ошибок:

* [BUG #226](https://github.com/BurntSushi/ripgrep/issues/226):
  Пути к файлам, явно данные в командной строке, не искали параллельно.
  (Это была регрессия в `0.2.7`.)
* [BUG #228](https://github.com/BurntSushi/ripgrep/issues/228):
  Если каталог был дан для `--ignore-file`, использование памяти ripgrep
  росло без границ.


0.2.8
=====
Исправления ошибок:

* Исправлена ошибка с функциями SIMD/AVX для использования bytecount в
  коммите `4ca15a`.


0.2.7
=====
Улучшения производительности:

* [PERF #223](https://github.com/BurntSushi/ripgrep/pull/223):
  Добавлен параллельный рекурсивный итератор каталогов. Это приводит к
  значительным улучшениям производительности на больших репозиториях.
* [PERF #11](https://github.com/BurntSushi/ripgrep/pull/11):
  ripgrep теперь использует библиотеку `bytecount` для подсчёта новых
  строк. В некоторых случаях ripgrep работает в два раза быстрее.
  Используйте
  `RUSTFLAGS="-C target-cpu=native" cargo build --release --features 'simd-accel avx-accel'`
  для получения максимально быстрого двоичного файла.

Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для Agda, Tex, Taskpaper,
  Markdown, asciidoc, textile, rdoc, org, creole, wiki, pod, C#, PDF, C, C++.
* [FEATURE #149](https://github.com/BurntSushi/ripgrep/issues/149):
  Добавить новый флаг `--no-messages`, который подавляет сообщения об
  ошибках. Обратите внимание, что `rg foo 2> /dev/null` также работает.
* [FEATURE #159](https://github.com/BurntSushi/ripgrep/issues/159):
  Добавить новый флаг `-m/--max-count`, который ограничивает общее
  количество совпадений, выводимых для каждого искомого файла.

Исправления ошибок:

* [BUG #199](https://github.com/BurntSushi/ripgrep/issues/199):
  Исправлена ошибка, из-за которой `-S/--smart-case` не применялся
  корректно к оптимизациям литералов.
* [BUG #203](https://github.com/BurntSushi/ripgrep/issues/203):
  Упоминать полное имя, ripgrep, в большем количестве мест. Теперь оно
  появляется в выводе `--help` и `--version`. URL репозитория теперь
  также в выводе `--help` и man-страницы.
* [BUG #215](https://github.com/BurntSushi/ripgrep/issues/215):
  Включить небольшое примечание о том, как искать шаблон, начинающийся с `-`.


0.2.6
=====
Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для Fish.

Исправления ошибок:

* [BUG #206](https://github.com/BurntSushi/ripgrep/issues/206):
  Исправлена регрессия с флагом `-g/--glob` в `0.2.5`.


0.2.5
=====
Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для Groovy, Handlebars,
  Tcl, zsh и Python.
* [FEATURE #9](https://github.com/BurntSushi/ripgrep/issues/9):
  Поддержка глобальной конфигурации gitignore и файлов `.git/info/exclude`.
* [FEATURE #45](https://github.com/BurntSushi/ripgrep/issues/45):
  Добавить флаг --ignore-file для указания дополнительных файлов ignore.
* [FEATURE #202](https://github.com/BurntSushi/ripgrep/pull/202):
  Ввести новый
  [`ignore`](https://github.com/BurntSushi/ripgrep/tree/master/ignore)
  крейт, который инкапсулирует всю логику сопоставления gitignore ripgrep.

Исправления ошибок:

* [BUG #44](https://github.com/BurntSushi/ripgrep/issues/44):
  ripgrep работает медленно, когда дано много позиционных аргументов,
  которые являются каталогами.
* [BUG #119](https://github.com/BurntSushi/ripgrep/issues/119):
  ripgrep не сбрасывал цвета терминала, если был прерван с помощью `^C`.
  Исправлено в [PR #187](https://github.com/BurntSushi/ripgrep/pull/187).
* [BUG #184](https://github.com/BurntSushi/ripgrep/issues/184):
  Исправлена ошибка, связанная с интерпретацией файлов gitignore в
  родительских каталогах.


0.2.4
=====
ПРОПУЩЕНО.


0.2.3
=====
Исправления ошибок:

* [BUG #164](https://github.com/BurntSushi/ripgrep/issues/164):
  Исправляет segfault в сборках macos.
* [BUG #167](https://github.com/BurntSushi/ripgrep/issues/167):
  Уточнить документацию для --threads.


0.2.2
=====
Обновления упаковки:

* `ripgrep` теперь в homebrew-core. `brew install ripgrep` сделает дело
  на Mac.
* `ripgrep` теперь в репозитории сообщества Archlinux.
  `pacman -S ripgrep` сделает дело на Archlinux.
* Поддержка прекращена для i686-darwin.
* Сопоставление глобов перемещено в отдельный крейт:
  [`globset`](https://crates.io/crates/globset).

Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для CMake, config, Jinja,
  Markdown, Spark.
* [FEATURE #109](https://github.com/BurntSushi/ripgrep/issues/109):
  Добавить флаг --max-depth для обхода каталогов.
* [FEATURE #124](https://github.com/BurntSushi/ripgrep/issues/124):
  Добавить флаг -s/--case-sensitive. Переопределяет --smart-case.
* [FEATURE #139](https://github.com/BurntSushi/ripgrep/pull/139):
  Репозиторий `ripgrep` теперь является Homebrew tap. Это полезно для
  установки двоичных файлов с ускорением SIMD, которые недоступны в
  homebrew-core.

Исправления ошибок:

* [BUG #87](https://github.com/BurntSushi/ripgrep/issues/87),
  [BUG #127](https://github.com/BurntSushi/ripgrep/issues/127),
  [BUG #131](https://github.com/BurntSushi/ripgrep/issues/131):
  Различные проблемы, связанные с сопоставлением глобов.
* [BUG #116](https://github.com/BurntSushi/ripgrep/issues/116):
  --quiet должен останавливать поиск после первого совпадения.
* [BUG #121](https://github.com/BurntSushi/ripgrep/pull/121):
  --color always должен показывать цвета, даже когда используется --vimgrep.
* [BUG #122](https://github.com/BurntSushi/ripgrep/pull/122):
  Раскрашивать путь к файлу в начале строки.
* [BUG #134](https://github.com/BurntSushi/ripgrep/issues/134):
  Обработка большого файла ignore (тысячи глобов) была очень медленной.
* [BUG #137](https://github.com/BurntSushi/ripgrep/issues/137):
  Всегда отслеживать symlinks, когда они даны как явный аргумент.
* [BUG #147](https://github.com/BurntSushi/ripgrep/issues/147):
  Уточнить документацию для --replace.


0.2.1
=====
Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для Clojure и
  SystemVerilog.
* [FEATURE #89](https://github.com/BurntSushi/ripgrep/issues/89):
  Добавить флаг --null, который выводит байт NUL после каждого пути к файлу.

Исправления ошибок:

* [BUG #98](https://github.com/BurntSushi/ripgrep/issues/98):
  Исправлена ошибка в однопоточном режиме, когда при неудачном открытии
  файла ripgrep завершался вместо продолжения поиска.
* [BUG #99](https://github.com/BurntSushi/ripgrep/issues/99):
  Исправлена ещё одна ошибка в однопоточном режиме, где пустые строки
  печатались по ошибке.
* [BUG #105](https://github.com/BurntSushi/ripgrep/issues/105):
  Исправлена ошибка off-by-one с --column.
* [BUG #106](https://github.com/BurntSushi/ripgrep/issues/106):
  Исправлена ошибка, из-за которой строка только с пробелами в файле
  gitignore вызывала панику ripgrep (т.е. сбой).


0.2.0
=====
Улучшения функциональности:

* Добавлена или улучшена фильтрация типов файлов для VB, R, F#, Swift, Nim,
  JavaScript, TypeScript
* [FEATURE #20](https://github.com/BurntSushi/ripgrep/issues/20):
  Добавляет флаг --no-filename.
* [FEATURE #26](https://github.com/BurntSushi/ripgrep/issues/26):
  Добавляет флаг --files-with-matches. Как --count, но только печатает
  пути к файлам и не нужно подсчитывать каждое совпадение.
* [FEATURE #40](https://github.com/BurntSushi/ripgrep/issues/40):
  Переключиться с использования `.rgignore` на `.ignore`. Обратите
  внимание, что `.rgignore` всё ещё поддерживается, но устарел.
* [FEATURE #68](https://github.com/BurntSushi/ripgrep/issues/68):
  Добавить флаг --no-ignore-vcs, который игнорирует .gitignore, но не
  .ignore.
* [FEATURE #70](https://github.com/BurntSushi/ripgrep/issues/70):
  Добавить флаг -S/--smart-case (но отключён по умолчанию).
* [FEATURE #80](https://github.com/BurntSushi/ripgrep/issues/80):
  Добавить поддержку глобов `{foo,bar}`.

Много-много исправлений ошибок. Спасибо всем за сообщения об этом и помощь
в улучшении `ripgrep`! (Обратите внимание, что я не зафиксировал каждый
отслеживающий issue здесь, некоторые были закрыты как дубликаты.)

* [BUG #8](https://github.com/BurntSushi/ripgrep/issues/8):
  Не использовать промежуточный буфер, когда --threads=1. (Разрешает
  использование постоянной памяти.)
* [BUG #15](https://github.com/BurntSushi/ripgrep/issues/15):
  Улучшает документацию для --type-add.
* [BUG #16](https://github.com/BurntSushi/ripgrep/issues/16),
  [BUG #49](https://github.com/BurntSushi/ripgrep/issues/49),
  [BUG #50](https://github.com/BurntSushi/ripgrep/issues/50),
  [BUG #65](https://github.com/BurntSushi/ripgrep/issues/65):
  Некоторые глобы gitignore обрабатывались как анкерные, когда они не были.
* [BUG #18](https://github.com/BurntSushi/ripgrep/issues/18):
  --vimgrep сообщал неверный номер столбца.
* [BUG #19](https://github.com/BurntSushi/ripgrep/issues/19):
  ripgrep зависал в ожидании stdin в некоторых терминалах Windows. Обратите
  внимание, что это ввело новую ошибку:
  [#94](https://github.com/BurntSushi/ripgrep/issues/94).
* [BUG #21](https://github.com/BurntSushi/ripgrep/issues/21):
  Удаляет ведущий `./` при печати путей к файлам.
* [BUG #22](https://github.com/BurntSushi/ripgrep/issues/22):
  Запуск `rg --help | echo` вызывал панику `rg`.
* [BUG #24](https://github.com/BurntSushi/ripgrep/issues/22):
  Уточнить центральную цель rg в его сообщении об использовании.
* [BUG #25](https://github.com/BurntSushi/ripgrep/issues/25):
  Анкерные глобы gitignore не применялись в подкаталогах корректно.
* [BUG #30](https://github.com/BurntSushi/ripgrep/issues/30):
  Глобы вроде `foo/**` должны сопоставлять содержимое `foo`, но не сам
  `foo`.
* [BUG #35](https://github.com/BurntSushi/ripgrep/issues/35),
  [BUG #81](https://github.com/BurntSushi/ripgrep/issues/81):
  При автоматическом обнаружении stdin читать только если это файл или fifo.
  То есть игнорировать stdin в `rg foo < /dev/null`.
* [BUG #36](https://github.com/BurntSushi/ripgrep/issues/36):
  Не выбирать автоматически memory maps на MacOS. Никогда.
* [BUG #38](https://github.com/BurntSushi/ripgrep/issues/38):
  Завершающие пробелы в gitignore не игнорировались.
* [BUG #43](https://github.com/BurntSushi/ripgrep/issues/43):
  --glob не работал с каталогами.
* [BUG #46](https://github.com/BurntSushi/ripgrep/issues/46):
  Использовать на один рабочий поток меньше, чем предоставлено в CLI.
* [BUG #47](https://github.com/BurntSushi/ripgrep/issues/47):
  --help/--version теперь работают, даже если другие опции установлены.
* [BUG #55](https://github.com/BurntSushi/ripgrep/issues/55):
  ripgrep отказывался искать /proc/cpuinfo. Исправлено отключением memory
  maps для файлов с нулевым размером.
* [BUG #64](https://github.com/BurntSushi/ripgrep/issues/64):
  Первый путь, данный с --files, игнорировался.
* [BUG #67](https://github.com/BurntSushi/ripgrep/issues/67):
  Иногда whitelist-глобы вроде `!/dir` не интерпретировались как анкерные.
* [BUG #77](https://github.com/BurntSushi/ripgrep/issues/77):
  Когда флаг -q/--quiet был передан, ripgrep продолжал поиск даже после
  того, как совпадение было найдено.
* [BUG #90](https://github.com/BurntSushi/ripgrep/issues/90):
  Разрешить whitelisting скрытых файлов.
* [BUG #93](https://github.com/BurntSushi/ripgrep/issues/93):
  ripgrep извлекал ошибочный внутренний литерал из повторяющегося шаблона.