/*!
Предоставляет процедуры для генерации строк версии.

Строки версии могут быть просто цифрами, кратким однострочным описанием
или чем-то более подробным, который включает такие вещи, как поддержка
функций CPU target.
*/

use std::fmt::Write;

/// Генерирует только числовую часть версии ripgrep.
///
/// Это включает хэш ревизии git.
pub(crate) fn generate_digits() -> String {
    let semver = option_env!("CARGO_PKG_VERSION").unwrap_or("N/A");
    match option_env!("RIPGREP_BUILD_GIT_HASH") {
        None => semver.to_string(),
        Some(hash) => format!("{semver} (rev {hash})"),
    }
}

/// Генерирует короткую строку версии вида `ripgrep x.y.z`.
pub(crate) fn generate_short() -> String {
    let digits = generate_digits();
    format!("ripgrep {digits}")
}

/// Генерирует длинную многострочную строку версии.
///
/// Это включает не только версию ripgrep, но и некоторую другую информацию
/// о его сборке. Например, поддержку SIMD и поддержку PCRE2.
pub(crate) fn generate_long() -> String {
    let (compile, runtime) = (compile_cpu_features(), runtime_cpu_features());

    let mut out = String::new();
    writeln!(out, "{}", generate_short()).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "features:{}", features().join(",")).unwrap();
    if !compile.is_empty() {
        writeln!(out, "simd(compile):{}", compile.join(",")).unwrap();
    }
    if !runtime.is_empty() {
        writeln!(out, "simd(runtime):{}", runtime.join(",")).unwrap();
    }
    let (pcre2_version, _) = generate_pcre2();
    writeln!(out, "\n{pcre2_version}").unwrap();
    out
}

/// Генерирует многострочную строку версии с информацией о PCRE2.
///
/// Это также возвращает, доступен ли PCRE2 в этой сборке ripgrep.
pub(crate) fn generate_pcre2() -> (String, bool) {
    let mut out = String::new();

    #[cfg(feature = "pcre2")]
    {
        use grep::pcre2;

        let (major, minor) = pcre2::version();
        write!(out, "PCRE2 {}.{} is available", major, minor).unwrap();
        if cfg!(target_pointer_width = "64") && pcre2::is_jit_available() {
            writeln!(out, " (JIT is available)").unwrap();
        } else {
            writeln!(out, " (JIT is unavailable)").unwrap();
        }
        (out, true)
    }

    #[cfg(not(feature = "pcre2"))]
    {
        writeln!(out, "PCRE2 is not available in this build of ripgrep.")
            .unwrap();
        (out, false)
    }
}

/// Возвращает соответствующие функции SIMD, поддерживаемые CPU во время выполнения.
///
/// Это своего рода грязное нарушение абстракции, поскольку предполагает
/// знание о том, какие конкретные функции SIMD используются различными
/// компонентами.
fn runtime_cpu_features() -> Vec<String> {
    #[cfg(target_arch = "x86_64")]
    {
        let mut features = vec![];

        let sse2 = is_x86_feature_detected!("sse2");
        features.push(format!("{sign}SSE2", sign = sign(sse2)));

        let ssse3 = is_x86_feature_detected!("ssse3");
        features.push(format!("{sign}SSSE3", sign = sign(ssse3)));

        let avx2 = is_x86_feature_detected!("avx2");
        features.push(format!("{sign}AVX2", sign = sign(avx2)));

        features
    }
    #[cfg(target_arch = "aarch64")]
    {
        let mut features = vec![];

        // memchr и aho-corasick используют NEON только когда он доступен во
        /// время компиляции. Это не совсем необходимо, но NEON должен быть
        /// доступен для всех целевых платформ aarch64. Если это не так,
        /// пожалуйста, сообщите об ошибке на https://github.com/BurntSushi/memchr.
        let neon = cfg!(target_feature = "neon");
        features.push(format!("{sign}NEON", sign = sign(neon)));

        features
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        vec![]
    }
}

/// Возвращает функции SIMD, поддерживаемые при компиляции ripgrep.
///
/// По существу, любые функции, перечисленные здесь, требуются для
/// корректной работы ripgrep.
///
/// Это своего рода грязное нарушение абстракции, поскольку предполагает
/// знание о том, какие конкретные функции SIMD используются различными
/// компонентами.
///
/// Простой способ включить все доступное на вашем текущем CPU —
/// скомпилировать ripgrep с `RUSTFLAGS="-C target-cpu=native"`. Но
/// обратите внимание, что созданный бинарный файл не будет переносимым.
fn compile_cpu_features() -> Vec<String> {
    #[cfg(target_arch = "x86_64")]
    {
        let mut features = vec![];

        let sse2 = cfg!(target_feature = "sse2");
        features.push(format!("{sign}SSE2", sign = sign(sse2)));

        let ssse3 = cfg!(target_feature = "ssse3");
        features.push(format!("{sign}SSSE3", sign = sign(ssse3)));

        let avx2 = cfg!(target_feature = "avx2");
        features.push(format!("{sign}AVX2", sign = sign(avx2)));

        features
    }
    #[cfg(target_arch = "aarch64")]
    {
        let mut features = vec![];

        let neon = cfg!(target_feature = "neon");
        features.push(format!("{sign}NEON", sign = sign(neon)));

        features
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        vec![]
    }
}

/// Возвращает список «функций», поддерживаемых (или нет) этой сборкой ripgrep.
fn features() -> Vec<String> {
    let mut features = vec![];

    let pcre2 = cfg!(feature = "pcre2");
    features.push(format!("{sign}pcre2", sign = sign(pcre2)));

    features
}

/// Возвращает `+`, когда `enabled` — `true`, и `-` в противном случае.
fn sign(enabled: bool) -> &'static str {
    if enabled { "+" } else { "-" }
}
