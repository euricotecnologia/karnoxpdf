//! Number and date formats of spreadsheet cells (ISO/IEC 29500-1 §18.8.30 and §18.2.28): the
//! format code a cell's style names turns its stored number into the text a spreadsheet shows.
//!
//! Excel formats numbers with the user's regional settings, which a file does not record, so the
//! caller picks a [`Locale`].

use crate::drawingml::Rgb;

/// The regional conventions a format code is shown with.
#[derive(Clone, Debug, PartialEq)]
pub struct Locale {
    pub decimal: char,
    pub thousands: char,
    /// The short date a spreadsheet's built-in date format 14 stands for.
    pub short_date: &'static str,
    months: [&'static str; 12],
    days: [&'static str; 7],
    /// How TRUE and FALSE are shown.
    pub(crate) booleans: (&'static str, &'static str),
}

const EN_MONTHS: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const EN_DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

impl Locale {
    pub fn english() -> Locale {
        Locale { decimal: '.', thousands: ',', short_date: "m/d/yyyy", months: EN_MONTHS, days: EN_DAYS, booleans: ("TRUE", "FALSE") }
    }

    /// The conventions of a UI language code (`pt-br`, `es`, `fr`, ...); English for the rest.
    pub fn from_language(code: &str) -> Locale {
        let base = code.split(['-', '_']).next().unwrap_or("").to_ascii_lowercase();
        match base.as_str() {
            "pt" => Locale {
                decimal: ',',
                thousands: '.',
                short_date: "dd/mm/yyyy",
                months: ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"],
                days: ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"],
                booleans: ("VERDADEIRO", "FALSO"),
            },
            "es" => Locale {
                decimal: ',',
                thousands: '.',
                short_date: "dd/mm/yyyy",
                months: ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"],
                days: ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"],
                booleans: ("VERDADERO", "FALSO"),
            },
            "fr" => Locale {
                decimal: ',',
                thousands: '\u{a0}',
                short_date: "dd/mm/yyyy",
                months: ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"],
                days: ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"],
                booleans: ("VRAI", "FAUX"),
            },
            "cs" => Locale { decimal: ',', thousands: '\u{a0}', short_date: "d.m.yyyy", ..Locale::english() },
            _ => Locale::english(),
        }
    }

    fn month(&self, index: usize) -> &'static str {
        self.months.get(index).copied().unwrap_or("")
    }

    fn day(&self, index: usize) -> &'static str {
        self.days.get(index).copied().unwrap_or("")
    }
}

/// A formatted cell value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Formatted {
    pub text: String,
    /// A colour the format asks for (`[Red]`).
    pub color: Option<Rgb>,
}

fn plain(text: String) -> Formatted {
    Formatted { text, color: None }
}

/// The format code of a built-in number format id (§18.8.30), where the id is not tied to a
/// regional setting.
pub(crate) fn builtin_code(id: u32) -> Option<&'static str> {
    Some(match id {
        0 => "General",
        1 => "0",
        2 => "0.00",
        3 => "#,##0",
        4 => "#,##0.00",
        5 => "\"$\"#,##0_);(\"$\"#,##0)",
        6 => "\"$\"#,##0_);[Red](\"$\"#,##0)",
        7 => "\"$\"#,##0.00_);(\"$\"#,##0.00)",
        8 => "\"$\"#,##0.00_);[Red](\"$\"#,##0.00)",
        9 => "0%",
        10 => "0.00%",
        11 => "0.00E+00",
        12 => "# ?/?",
        13 => "# ??/??",
        14 => "<short date>",
        15 => "d-mmm-yy",
        16 => "d-mmm",
        17 => "mmm-yy",
        18 => "h:mm AM/PM",
        19 => "h:mm:ss AM/PM",
        20 => "h:mm",
        21 => "h:mm:ss",
        22 => "<short date> h:mm",
        37 => "#,##0_);(#,##0)",
        38 => "#,##0_);[Red](#,##0)",
        39 => "#,##0.00_);(#,##0.00)",
        40 => "#,##0.00_);[Red](#,##0.00)",
        41 => "_(* #,##0_);_(* (#,##0);_(* \"-\"_);_(@_)",
        42 => "_(\"$\"* #,##0_);_(\"$\"* (#,##0);_(\"$\"* \"-\"_);_(@_)",
        43 => "_(* #,##0.00_);_(* (#,##0.00);_(* \"-\"??_);_(@_)",
        44 => "_(\"$\"* #,##0.00_);_(\"$\"* (#,##0.00);_(\"$\"* \"-\"??_);_(@_)",
        45 => "mm:ss",
        46 => "[h]:mm:ss",
        47 => "mm:ss.0",
        48 => "##0.0E+0",
        49 => "@",
        _ => return None,
    })
}

// ── dates ───────────────────────────────────────────────────────────────────────────────────

/// A calendar date and time of day.
#[derive(Clone, Copy, Debug, PartialEq)]
struct DateTime {
    year: i64,
    month: u32,
    day: u32,
    /// 0 = Sunday.
    weekday: u32,
    hour: u32,
    minute: u32,
    second: u32,
    /// Hundredths of a second.
    centis: u32,
}

/// Days since 1970-01-01 of a civil date (proleptic Gregorian).
pub(crate) fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The date and time a serial number stands for. In the 1900 system serial 60 is the day
/// Lotus 1-2-3 wrongly believed was 29 February 1900, which Excel keeps.
fn from_serial(serial: f64, date1904: bool) -> Option<DateTime> {
    if !serial.is_finite() || !(0.0..2_958_466.0).contains(&serial) {
        return None;
    }
    // Round to a hundredth of a second, as the time display does.
    let total_centis = (serial * 8_640_000.0).round() as i64;
    let days = total_centis.div_euclid(8_640_000);
    let rest = total_centis.rem_euclid(8_640_000);
    let (year, month, day, weekday);
    if date1904 {
        let unix = days_from_civil(1904, 1, 1) + days;
        (year, month, day) = civil_from_days(unix);
        weekday = (unix + 4).rem_euclid(7) as u32;
    } else if days == 60 {
        (year, month, day) = (1900, 2, 29);
        weekday = 3; // the leap day Excel invents is shown as a Wednesday
    } else {
        // Before the invented day, serial 1 is 1900-01-01; after it, serial 61 is 1900-03-01.
        let unix = if days < 60 { days_from_civil(1899, 12, 31) + days } else { days_from_civil(1899, 12, 30) + days };
        (year, month, day) = civil_from_days(unix);
        weekday = (unix + 4).rem_euclid(7) as u32;
    }
    let seconds = rest / 100;
    Some(DateTime {
        year,
        month,
        day,
        weekday,
        hour: (seconds / 3600) as u32,
        minute: (seconds % 3600 / 60) as u32,
        second: (seconds % 60) as u32,
        centis: (rest % 100) as u32,
    })
}

// ── format code parsing ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Literal(String),
    /// A digit placeholder: `0`, `#` or `?`.
    Digit(char),
    Point,
    Comma,
    Percent,
    /// `E+` / `E-`
    Exponent(bool),
    /// `/` between digit placeholders: a fraction.
    Slash,
    /// `@`
    Text,
    /// Date or time field letters, with their run length (`yyyy` is ('y', 4)).
    Field(char, usize),
    /// `[h]`, `[m]`, `[s]`: elapsed time.
    Elapsed(char, usize),
    AmPm(String),
}

#[derive(Clone, Debug, Default)]
struct Section {
    tokens: Vec<Token>,
    color: Option<Rgb>,
    condition: Option<(String, f64)>,
}

fn named_color(name: &str) -> Option<Rgb> {
    let n = |v: u8| v as f64 / 255.0;
    Some(match name.to_ascii_lowercase().as_str() {
        "black" => [0.0; 3],
        "blue" => [0.0, 0.0, 1.0],
        "cyan" => [0.0, 1.0, 1.0],
        "green" => [0.0, n(128), 0.0],
        "magenta" => [1.0, 0.0, 1.0],
        "red" => [1.0, 0.0, 0.0],
        "white" => [1.0; 3],
        "yellow" => [1.0, 1.0, 0.0],
        _ => return None,
    })
}

/// Split a format code into its `;`-separated sections and read each one's tokens.
fn parse_sections(code: &str) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut section = Section::default();
    let mut literal = String::new();
    let chars: Vec<char> = code.chars().collect();
    let mut i = 0;
    let flush = |section: &mut Section, literal: &mut String| {
        if !literal.is_empty() {
            section.tokens.push(Token::Literal(std::mem::take(literal)));
        }
    };
    while i < chars.len() {
        let c = chars[i];
        match c {
            '"' => {
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    literal.push(chars[i]);
                    i += 1;
                }
            }
            '\\' => {
                i += 1;
                if let Some(next) = chars.get(i) {
                    literal.push(*next);
                }
            }
            '_' => {
                // Skip the width of the next character: a space of about that width.
                i += 1;
                literal.push(' ');
            }
            '*' => {
                // Repeat the next character to fill the cell: not representable, dropped.
                i += 1;
            }
            '[' => {
                let end = chars[i..].iter().position(|c| *c == ']').map_or(chars.len(), |p| i + p);
                let inner: String = chars[i + 1..end.min(chars.len())].iter().collect();
                i = end;
                if let Some(color) = named_color(&inner) {
                    section.color = Some(color);
                } else if inner.len() > 5 && inner[..5].eq_ignore_ascii_case("color") {
                    // [ColorN]: the legacy palette; shown in its own colour by the caller if needed.
                } else if let Some(first) = inner.chars().next().filter(|c| matches!(c, 'h' | 'H' | 'm' | 'M' | 's' | 'S')) {
                    if inner.chars().all(|c| c.eq_ignore_ascii_case(&first)) {
                        flush(&mut section, &mut literal);
                        section.tokens.push(Token::Elapsed(first.to_ascii_lowercase(), inner.len()));
                    }
                } else if let Some(rest) = inner.strip_prefix('$') {
                    // [$€-407]: a currency symbol, then a locale id.
                    let symbol = rest.split('-').next().unwrap_or("");
                    literal.push_str(symbol);
                } else {
                    let op_len = inner.chars().take_while(|c| matches!(c, '<' | '>' | '=')).count();
                    if op_len > 0
                        && let Ok(value) = inner[op_len..].trim().parse::<f64>()
                    {
                        section.condition = Some((inner[..op_len].to_owned(), value));
                    }
                }
            }
            ';' => {
                flush(&mut section, &mut literal);
                sections.push(std::mem::take(&mut section));
            }
            '0' | '#' | '?' => {
                flush(&mut section, &mut literal);
                section.tokens.push(Token::Digit(c));
            }
            '.' => {
                flush(&mut section, &mut literal);
                section.tokens.push(Token::Point);
            }
            ',' => {
                flush(&mut section, &mut literal);
                section.tokens.push(Token::Comma);
            }
            '%' => {
                flush(&mut section, &mut literal);
                section.tokens.push(Token::Percent);
            }
            '/' => {
                flush(&mut section, &mut literal);
                section.tokens.push(Token::Slash);
            }
            '@' => {
                flush(&mut section, &mut literal);
                section.tokens.push(Token::Text);
            }
            'E' | 'e' if matches!(chars.get(i + 1), Some('+' | '-')) => {
                flush(&mut section, &mut literal);
                section.tokens.push(Token::Exponent(chars[i + 1] == '+'));
                i += 1;
            }
            'y' | 'Y' | 'm' | 'M' | 'd' | 'D' | 'h' | 'H' | 's' | 'S' => {
                flush(&mut section, &mut literal);
                let lower = c.to_ascii_lowercase();
                let mut run = 1;
                while chars.get(i + run).is_some_and(|n| n.to_ascii_lowercase() == lower) {
                    run += 1;
                }
                section.tokens.push(Token::Field(lower, run));
                i += run - 1;
            }
            'A' | 'a' | 'P' | 'p' => {
                let rest: String = chars[i..].iter().take(5).collect::<String>().to_ascii_uppercase();
                if rest.starts_with("AM/PM") {
                    flush(&mut section, &mut literal);
                    section.tokens.push(Token::AmPm("AM/PM".into()));
                    i += 4;
                } else if rest.starts_with("A/P") {
                    flush(&mut section, &mut literal);
                    section.tokens.push(Token::AmPm("A/P".into()));
                    i += 2;
                } else {
                    literal.push(c);
                }
            }
            _ => literal.push(c),
        }
        i += 1;
    }
    flush(&mut section, &mut literal);
    sections.push(section);
    sections
}

fn is_date_section(section: &Section) -> bool {
    section.tokens.iter().any(|t| matches!(t, Token::Field(..) | Token::Elapsed(..) | Token::AmPm(_)))
}

/// Whether a format code shows dates or times.
#[cfg(test)]
pub(crate) fn is_date_format(code: &str, locale: &Locale) -> bool {
    let code = expand_builtin_date(code, locale);
    parse_sections(&code).iter().any(is_date_section)
}

fn expand_builtin_date(code: &str, locale: &Locale) -> String {
    code.replace("<short date>", locale.short_date)
}

// ── numbers ─────────────────────────────────────────────────────────────────────────────────

/// `abs` rounded half-up to `decimals` places, as integer digits and fraction digits. Excel works
/// from the 15 significant digits a double carries, which is why 2.675 shows as 2.68.
fn round_decimal(abs: f64, decimals: usize) -> (String, String) {
    if abs == 0.0 || !abs.is_finite() {
        return ("0".into(), "0".repeat(decimals));
    }
    let exp = abs.log10().floor() as i32;
    // 15 significant digits as a digit string with the decimal point `exp + 1` digits in.
    let sci = format!("{:.14e}", abs);
    let (mantissa, e) = sci.split_once('e').unwrap_or((&sci, "0"));
    let e: i32 = e.parse().unwrap_or(exp);
    let mut digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).map(|b| b - b'0').collect();
    let point = e + 1; // digits before the decimal point (may be <= 0)
    let keep = point + decimals as i32; // digits to keep
    if keep < 0 {
        return ("0".into(), "0".repeat(decimals));
    }
    let keep = keep as usize;
    if keep < digits.len() {
        let round_up = digits.get(keep).is_some_and(|d| *d >= 5);
        digits.truncate(keep);
        if round_up {
            let mut i = keep;
            loop {
                if i == 0 {
                    digits.insert(0, 1);
                    // The value gained a digit before the point.
                    return split_digits(&digits, point + 1, decimals);
                }
                i -= 1;
                if digits[i] == 9 {
                    digits[i] = 0;
                } else {
                    digits[i] += 1;
                    break;
                }
            }
        }
    }
    split_digits(&digits, point, decimals)
}

fn split_digits(digits: &[u8], point: i32, decimals: usize) -> (String, String) {
    let at = |i: i32| -> char { if i >= 0 && (i as usize) < digits.len() { (b'0' + digits[i as usize]) as char } else { '0' } };
    let int: String = if point <= 0 { "0".into() } else { (0..point).map(at).collect() };
    let frac: String = (0..decimals as i32).map(|k| at(point + k)).collect();
    let int = int.trim_start_matches('0');
    (if int.is_empty() { "0".into() } else { int.to_owned() }, frac)
}

fn group_thousands(int: &str, sep: char) -> String {
    let mut out = String::new();
    let len = int.len();
    for (i, c) in int.chars().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            out.push(sep);
        }
        out.push(c);
    }
    out
}

/// Excel's General format: up to 11 characters, scientific notation for very large and small numbers.
pub(crate) fn general(value: f64, locale: &Locale) -> String {
    if !value.is_finite() {
        return "#NUM!".into();
    }
    if value == 0.0 {
        return "0".into();
    }
    let abs = value.abs();
    let sign = if value < 0.0 { "-" } else { "" };
    let swap = |s: String| if locale.decimal == '.' { s } else { s.replace('.', &locale.decimal.to_string()) };
    let scientific = || {
        let sci = format!("{:.5e}", abs);
        let (m, e) = sci.split_once('e').unwrap_or((&sci, "0"));
        let m = if m.contains('.') { m.trim_end_matches('0').trim_end_matches('.') } else { m };
        let e: i32 = e.parse().unwrap_or(0);
        swap(format!("{sign}{m}E{}{:02}", if e < 0 { '-' } else { '+' }, e.abs()))
    };
    if abs >= 1e11 {
        return scientific();
    }
    // Plain notation if it fits in 11 characters and keeps at least six significant digits;
    // otherwise scientific.
    let int_digits = if abs >= 1.0 { abs.log10().floor() as usize + 1 } else { 1 };
    let decimals = 10usize.saturating_sub(int_digits).min(15);
    let (int, frac) = round_decimal(abs, decimals);
    let frac_trimmed = frac.trim_end_matches('0');
    let leading_zeros = frac.chars().take_while(|c| *c == '0').count();
    let significant = if int != "0" { int.len() + frac_trimmed.len() } else { frac_trimmed.len().saturating_sub(leading_zeros) };
    let exact = (abs - format!("{int}.{frac}").parse::<f64>().unwrap_or(abs)).abs() < abs * 1e-14;
    if abs < 1.0 && significant < 6 && !exact {
        return scientific();
    }
    swap(if frac_trimmed.is_empty() { format!("{sign}{int}") } else { format!("{sign}{int}.{frac_trimmed}") })
}

/// The best fraction of `x` (in 0..1) with a denominator below `10^digits`.
fn best_fraction(x: f64, max_den: u64) -> (u64, u64) {
    let (mut best_n, mut best_d, mut best_err) = (0u64, 1u64, f64::INFINITY);
    for d in 1..=max_den {
        let n = (x * d as f64).round() as u64;
        let err = (x - n as f64 / d as f64).abs();
        if err < best_err - 1e-12 {
            (best_n, best_d, best_err) = (n, d, err);
            if err < 1e-12 {
                break;
            }
        }
    }
    (best_n, best_d)
}

fn format_numeric(tokens: &[Token], value: f64, locale: &Locale, show_sign: bool) -> String {
    let percent = tokens.iter().filter(|t| matches!(t, Token::Percent)).count();
    let mut v = value.abs();
    for _ in 0..percent {
        v *= 100.0;
    }
    // Commas right after the last digit placeholder scale the number down by 1000 each.
    let last_digit = tokens.iter().rposition(|t| matches!(t, Token::Digit(_)));
    let mut scale = 0;
    if let Some(last) = last_digit {
        for t in tokens.iter().skip(last + 1) {
            if matches!(t, Token::Comma) {
                scale += 1;
            } else {
                break;
            }
        }
    }
    for _ in 0..scale {
        v /= 1000.0;
    }
    let point = tokens.iter().position(|t| matches!(t, Token::Point));
    let slash = tokens.iter().position(|t| matches!(t, Token::Slash));
    let exp = tokens.iter().position(|t| matches!(t, Token::Exponent(_)));
    let thousands = tokens.iter().enumerate().any(|(i, t)| {
        matches!(t, Token::Comma)
            && tokens.get(..i).is_some_and(|b| b.iter().any(|x| matches!(x, Token::Digit(_))))
            && tokens.get(i + 1..).is_some_and(|a| a.iter().any(|x| matches!(x, Token::Digit(_))))
    });
    let digit_kinds = |range: &[Token]| -> Vec<char> { range.iter().filter_map(|t| if let Token::Digit(k) = t { Some(*k) } else { None }).collect() };
    let int_end = point.or(exp).or(slash).unwrap_or(tokens.len());
    let int_kinds = digit_kinds(&tokens[..int_end]);
    let dec_kinds: Vec<char> = match point {
        Some(p) => digit_kinds(&tokens[p + 1..exp.unwrap_or(tokens.len())]),
        None => Vec::new(),
    };

    // Fractions: `# ?/?`, `0/8`.
    if let Some(s) = slash.filter(|_| exp.is_none()) {
        let den_digits = tokens[s + 1..].iter().filter(|t| matches!(t, Token::Digit(_))).count().max(1);
        let num_digits = tokens[..s].iter().rev().take_while(|t| !matches!(t, Token::Literal(_))).filter(|t| matches!(t, Token::Digit(_))).count();
        let whole_kinds = tokens[..s].len() - num_digits;
        let has_whole = tokens[..s].iter().take(whole_kinds).any(|t| matches!(t, Token::Digit(_)));
        let max_den = 10u64.pow(den_digits as u32) - 1;
        let (whole, frac) = if has_whole { (v.trunc(), v.fract()) } else { (0.0, v) };
        let (mut n, d) = best_fraction(if has_whole { frac } else { frac.fract() }, max_den);
        let mut whole = whole;
        if !has_whole {
            n += (v.trunc() as u64) * d;
        }
        let mut d = d;
        if has_whole && n == d {
            whole += 1.0;
            n = 0;
        }
        if n == 0 && has_whole {
            d = 0;
        }
        // Literals before the first digit and after the last one frame the fraction.
        let first = tokens.iter().position(|t| matches!(t, Token::Digit(_))).unwrap_or(0);
        let last = tokens.iter().rposition(|t| matches!(t, Token::Digit(_))).unwrap_or(0);
        let literals =
            |range: &[Token]| -> String { range.iter().filter_map(|t| if let Token::Literal(l) = t { Some(l.as_str()) } else { None }).collect() };
        let prefix = literals(&tokens[..first]);
        let out = literals(&tokens[last + 1..]);
        let body = if d == 0 {
            format!("{}", whole as u64)
        } else if has_whole && whole > 0.0 {
            format!("{} {n}/{d}", whole as u64)
        } else {
            format!("{n}/{d}")
        };
        return format!("{}{prefix}{body}{out}", if show_sign { "-" } else { "" });
    }

    let (int_str, frac_str, exp_str) = if let Some(e) = exp {
        // Scientific: mantissa with the integer placeholders' digits, then the exponent.
        let int_places = int_kinds.len().max(1);
        let mut exponent = if v == 0.0 { 0 } else { v.log10().floor() as i32 };
        // Engineering formats (`##0.0E+0`) keep the exponent a multiple of the integer places.
        if int_places > 1 {
            exponent -= exponent.rem_euclid(int_places as i32);
        }
        let mantissa = if v == 0.0 { 0.0 } else { v / 10f64.powi(exponent) };
        let (mut i, mut f) = round_decimal(mantissa, dec_kinds.len());
        if i.len() > int_places {
            // Rounding carried into a new digit: renormalise.
            exponent += 1;
            let (i2, f2) = round_decimal(mantissa / 10.0, dec_kinds.len());
            i = i2;
            f = f2;
        }
        let exp_digits = tokens[e + 1..].iter().filter(|t| matches!(t, Token::Digit(_))).count().max(1);
        let sign = if exponent < 0 {
            "-"
        } else if matches!(tokens[e], Token::Exponent(true)) {
            "+"
        } else {
            ""
        };
        (i, f, Some(format!("E{sign}{:0>width$}", exponent.abs(), width = exp_digits)))
    } else {
        let (i, f) = round_decimal(v, dec_kinds.len());
        (i, f, None)
    };

    // Integer digits, right to left over the placeholders.
    let mut int_digits: Vec<char> = int_str.chars().collect();
    // A zero integer part is left out when every integer placeholder is `#` (`#.##` shows 0.5 as `.5`).
    if int_digits == ['0'] && !int_kinds.is_empty() && int_kinds.iter().all(|k| *k == '#') {
        int_digits.clear();
    }
    let mut padded: Vec<char> = int_digits.clone();
    // Missing digits are filled according to the placeholders that call for them.
    for k in (0..int_kinds.len()).rev() {
        let from_right = int_kinds.len() - 1 - k;
        if from_right >= int_digits.len() {
            match int_kinds[k] {
                '0' => padded.insert(0, '0'),
                '?' => padded.insert(0, ' '),
                _ => {}
            }
        }
    }
    let int_text: String = padded.iter().collect();
    let int_text = if thousands { group_thousands(&int_text, locale.thousands) } else { int_text };
    // Fraction digits: `#` drops trailing zeros, `?` turns them into spaces.
    let mut frac_chars: Vec<char> = frac_str.chars().collect();
    for k in (0..frac_chars.len()).rev() {
        let kind = dec_kinds.get(k).copied().unwrap_or('0');
        if frac_chars[k] == '0' && kind != '0' {
            if kind == '#' {
                frac_chars.truncate(k);
            } else {
                frac_chars[k] = ' ';
            }
        } else {
            break;
        }
    }
    let frac_text: String = frac_chars.iter().collect();

    let mut out = String::new();
    if show_sign {
        out.push('-');
    }
    let mut int_done = false;
    let mut dec_done = false;
    for (i, t) in tokens.iter().enumerate() {
        match t {
            Token::Literal(l) => out.push_str(l),
            Token::Percent => out.push('%'),
            Token::Digit(_) => {
                if i < int_end {
                    if !int_done {
                        int_done = true;
                        out.push_str(&int_text);
                    }
                } else if point.is_some_and(|p| i > p) && !dec_done && exp.is_none_or(|e| i < e) {
                    dec_done = true;
                    out.push_str(&frac_text);
                }
            }
            Token::Point => {
                if !frac_text.trim().is_empty() || dec_kinds.contains(&'0') {
                    out.push(locale.decimal);
                }
            }
            Token::Exponent(_) => {
                if let Some(e) = &exp_str {
                    out.push_str(e);
                }
                break;
            }
            Token::Comma | Token::Slash | Token::Text | Token::Field(..) | Token::Elapsed(..) | Token::AmPm(_) => {}
        }
    }
    out
}

fn format_date(tokens: &[Token], dt: &DateTime, serial: f64, locale: &Locale) -> String {
    let twelve = tokens.iter().any(|t| matches!(t, Token::AmPm(_)));
    let mut out = String::new();
    for (i, t) in tokens.iter().enumerate() {
        match t {
            Token::Literal(l) => out.push_str(l),
            Token::Field(c, n) => {
                // `m` is minutes straight after an hour field or straight before a seconds field.
                let minutes = *c == 'm' && {
                    let before = tokens[..i].iter().rev().find(|t| !matches!(t, Token::Literal(l) if l.trim().is_empty() || l == ":" || l == "."));
                    let after = tokens[i + 1..].iter().find(|t| !matches!(t, Token::Literal(l) if l.trim().is_empty() || l == ":" || l == "."));
                    matches!(before, Some(Token::Field('h', _) | Token::Elapsed('h', _))) || matches!(after, Some(Token::Field('s', _)))
                };
                match (*c, minutes) {
                    ('y', _) => {
                        if *n <= 2 {
                            out.push_str(&format!("{:02}", dt.year.rem_euclid(100)));
                        } else {
                            out.push_str(&format!("{:04}", dt.year));
                        }
                    }
                    ('m', false) => match n {
                        1 => out.push_str(&dt.month.to_string()),
                        2 => out.push_str(&format!("{:02}", dt.month)),
                        3 => out.extend(locale.month(dt.month as usize - 1).chars().take(3)),
                        4 => out.push_str(locale.month(dt.month as usize - 1)),
                        _ => out.extend(locale.month(dt.month as usize - 1).chars().take(1)),
                    },
                    ('m', true) => out.push_str(&if *n == 1 { dt.minute.to_string() } else { format!("{:02}", dt.minute) }),
                    ('d', _) => match n {
                        1 => out.push_str(&dt.day.to_string()),
                        2 => out.push_str(&format!("{:02}", dt.day)),
                        3 => out.extend(locale.day(dt.weekday as usize).chars().take(3)),
                        _ => out.push_str(locale.day(dt.weekday as usize)),
                    },
                    ('h', _) => {
                        let h = if twelve { (dt.hour + 11) % 12 + 1 } else { dt.hour };
                        out.push_str(&if *n == 1 { h.to_string() } else { format!("{h:02}") });
                    }
                    ('s', _) => {
                        out.push_str(&if *n == 1 { dt.second.to_string() } else { format!("{:02}", dt.second) });
                        // `ss.00`: fractions of a second follow as literal point and zeros.
                    }
                    _ => {}
                }
            }
            Token::Elapsed(c, n) => {
                let total_seconds = (serial * 86_400.0).round() as i64;
                let value = match c {
                    'h' => total_seconds / 3600,
                    'm' => total_seconds / 60,
                    _ => total_seconds,
                };
                out.push_str(&format!("{value:0>width$}", width = *n));
            }
            Token::AmPm(kind) => {
                let pm = dt.hour >= 12;
                out.push_str(match (kind.as_str(), pm) {
                    ("AM/PM", false) => "AM",
                    ("AM/PM", true) => "PM",
                    (_, false) => "A",
                    (_, true) => "P",
                });
            }
            Token::Slash => out.push('/'),
            Token::Comma => out.push(','),
            Token::Percent => out.push('%'),
            // Fractions of a second: ".0", ".00" after the seconds.
            Token::Point => out.push('.'),
            // The hundredths of a second, after `s.`: the first digit placeholder writes them all.
            Token::Digit(_) if matches!(tokens.get(i.wrapping_sub(1)), Some(Token::Point)) => {
                let width = tokens[i..].iter().take_while(|t| matches!(t, Token::Digit(_))).count();
                out.extend(format!("{:02}", dt.centis).chars().take(width.min(2)));
            }
            _ => {}
        }
    }
    out
}

fn condition_holds(condition: &(String, f64), value: f64) -> bool {
    match condition.0.as_str() {
        ">" => value > condition.1,
        ">=" => value >= condition.1,
        "<" => value < condition.1,
        "<=" => value <= condition.1,
        "=" => value == condition.1,
        "<>" => value != condition.1,
        _ => false,
    }
}

/// The text a number is shown as under format `code`.
pub(crate) fn format_value(code: &str, value: f64, locale: &Locale, date1904: bool) -> Formatted {
    let code = expand_builtin_date(code, locale);
    if code.trim().is_empty() || code.eq_ignore_ascii_case("general") {
        return plain(general(value, locale));
    }
    let sections = parse_sections(&code);
    // Which section: by explicit conditions if any, else positive / negative / zero by position.
    let chosen = if sections.iter().any(|s| s.condition.is_some()) {
        sections
            .iter()
            .enumerate()
            .find(|(i, s)| match &s.condition {
                Some(c) => condition_holds(c, value),
                None => *i == sections.len().saturating_sub(1) || (*i == 1 && sections.len() == 2),
            })
            .map(|(i, _)| i)
            .unwrap_or(sections.len() - 1)
    } else {
        match (sections.len(), value) {
            (1, _) => 0,
            (_, v) if v > 0.0 => 0,
            (2, v) if v >= 0.0 => 0,
            (_, v) if v < 0.0 => 1,
            (n, _) => n.min(3) - 1,
        }
    };
    let section = &sections[chosen];
    // A negative number takes the minus sign itself unless a later section was made for it.
    let negative_in_own_section =
        sections.len() >= 2 && chosen == 1 && section.condition.is_none() || sections.iter().any(|s| s.condition.is_some()) && chosen > 0;
    let show_sign = value < 0.0 && !negative_in_own_section;
    if is_date_section(section) {
        return match from_serial(value, date1904) {
            Some(dt) => Formatted { text: format_date(&section.tokens, &dt, value, locale), color: section.color },
            None => plain("#".repeat(10)),
        };
    }
    if section.tokens.iter().all(|t| matches!(t, Token::Literal(_) | Token::Text)) {
        // A section with only text: it shows no number.
        let text: String = section.tokens.iter().map(|t| if let Token::Literal(l) = t { l.as_str() } else { "" }).collect();
        return Formatted { text, color: section.color };
    }
    Formatted { text: format_numeric(&section.tokens, value, locale, show_sign), color: section.color }
}

/// The text a string is shown as under format `code`: the `@` section, or the string itself.
pub(crate) fn format_text(code: &str, text: &str) -> Formatted {
    let sections = parse_sections(code);
    let section = if sections.len() >= 4 { sections.get(3) } else { sections.iter().find(|s| s.tokens.iter().any(|t| matches!(t, Token::Text))) };
    match section {
        Some(s) if s.tokens.iter().any(|t| matches!(t, Token::Text)) => {
            let mut out = String::new();
            for t in &s.tokens {
                match t {
                    Token::Literal(l) => out.push_str(l),
                    Token::Text => out.push_str(text),
                    _ => {}
                }
            }
            Formatted { text: out, color: s.color }
        }
        _ => plain(text.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn en(code: &str, v: f64) -> String {
        format_value(code, v, &Locale::english(), false).text
    }

    fn pt(code: &str, v: f64) -> String {
        format_value(code, v, &Locale::from_language("pt-br"), false).text
    }

    #[test]
    fn general_follows_excels_eleven_character_rule() {
        let l = Locale::english();
        assert_eq!(general(0.1 + 0.2, &l), "0.3");
        assert_eq!(general(1234567.891, &l), "1234567.891");
        assert_eq!(general(-42.0, &l), "-42");
        assert_eq!(general(12_345_678_901.0, &l), "12345678901");
        assert_eq!(general(123_456_789_012.0, &l), "1.23457E+11");
        assert_eq!(general(0.000012345, &l), "0.000012345");
        assert_eq!(general(0.000012345678, &l), "1.23457E-05");
        assert_eq!(general(1.0 / 3.0, &l), "0.333333333");
        assert_eq!(general(0.5, &Locale::from_language("pt")), "0,5");
    }

    #[test]
    fn fixed_decimals_thousands_and_percent() {
        assert_eq!(en("0.00", 3.14259), "3.14");
        assert_eq!(en("0", 2.5), "3");
        assert_eq!(en("0.00", 2.675), "2.68", "rounds the 15-digit decimal value half up");
        assert_eq!(en("#,##0", 1234567.0), "1,234,567");
        assert_eq!(pt("#,##0.00", 1234567.891), "1.234.567,89");
        assert_eq!(en("0%", 0.256), "26%");
        assert_eq!(en("0.0%", 0.256), "25.6%");
        assert_eq!(en("00000", 42.0), "00042");
        assert_eq!(en("#.##", 0.5), ".5");
        assert_eq!(en("0.0#", 1.5), "1.5");
        assert_eq!(en("0.0#", 1.25), "1.25");
        assert_eq!(en("#,##0,", 1_234_567.0), "1,235", "a trailing comma divides by a thousand");
        assert_eq!(en("0.00", 999.999), "1000.00");
    }

    #[test]
    fn literals_currency_and_sections() {
        assert_eq!(en("\"R$\" #,##0.00", 1234.5), "R$ 1,234.50");
        assert_eq!(en("[$€-407] #,##0.00", 5.0), "€ 5.00");
        assert_eq!(en("0.0\" kg\"", 2.0), "2.0 kg");
        assert_eq!(en("#,##0.00;[Red]-#,##0.00", -5.0), "-5.00");
        let negative = format_value("#,##0.00;[Red]-#,##0.00", -5.0, &Locale::english(), false);
        assert_eq!(negative.color, Some([1.0, 0.0, 0.0]));
        assert_eq!(en("0.00", -5.0), "-5.00");
        assert_eq!(en("#,##0_);(#,##0)", -1234.0), "(1,234)");
        assert_eq!(en("#,##0_);(#,##0)", 1234.0), "1,234 ");
        assert_eq!(en("0;-0;\"zero\"", 0.0), "zero");
        assert_eq!(en("[>=100]\"big\";0", 150.0), "big");
        assert_eq!(en("[>=100]\"big\";0", 5.0), "5");
    }

    #[test]
    fn scientific_and_fractions() {
        assert_eq!(en("0.00E+00", 12345.0), "1.23E+04");
        assert_eq!(en("0.00E+00", 0.000123), "1.23E-04");
        assert_eq!(en("# ?/?", 1.5), "1 1/2");
        assert_eq!(en("# ?/?", 0.25), "1/4");
        assert_eq!(en("# ??/??", 3.14259), "3 1/7");
        assert_eq!(en("# ?/?", 3.14259), "3 1/7");
    }

    #[test]
    fn serial_numbers_are_dates() {
        let d = from_serial(45000.0, false).unwrap();
        assert_eq!((d.year, d.month, d.day, d.weekday), (2023, 3, 15, 3));
        let d = from_serial(1.0, false).unwrap();
        assert_eq!((d.year, d.month, d.day), (1900, 1, 1));
        let d = from_serial(59.0, false).unwrap();
        assert_eq!((d.year, d.month, d.day), (1900, 2, 28));
        let d = from_serial(60.0, false).unwrap();
        assert_eq!((d.year, d.month, d.day), (1900, 2, 29), "Excel's invented leap day");
        let d = from_serial(61.0, false).unwrap();
        assert_eq!((d.year, d.month, d.day), (1900, 3, 1));
        let d = from_serial(0.0, true).unwrap();
        assert_eq!((d.year, d.month, d.day), (1904, 1, 1));
        let d = from_serial(43831.0, false).unwrap();
        assert_eq!((d.year, d.month, d.day), (2020, 1, 1));
    }

    #[test]
    fn date_and_time_formats() {
        assert_eq!(en("yyyy-mm-dd", 45000.0), "2023-03-15");
        assert_eq!(en("dd/mm/yyyy", 45000.0), "15/03/2023");
        assert_eq!(en("d-mmm-yy", 45000.0), "15-Mar-23");
        assert_eq!(en("mmmm d, yyyy", 45000.0), "March 15, 2023");
        assert_eq!(en("dddd", 45000.0), "Wednesday");
        assert_eq!(pt("dddd, d \\d\\e mmmm", 45000.0), "quarta-feira, 15 de março");
        assert_eq!(en("h:mm AM/PM", 0.75), "6:00 PM");
        assert_eq!(en("hh:mm:ss", 0.5 + 1.0 / 86_400.0 * 61.0), "12:01:01");
        assert_eq!(en("yyyy-mm-dd hh:mm", 45000.5), "2023-03-15 12:00");
        assert_eq!(en("[h]:mm", 1.5), "36:00");
        assert_eq!(en("mm:ss", 90.0 / 86_400.0), "01:30");
        assert_eq!(en("<short date>", 45000.0), "3/15/2023");
        assert_eq!(pt("<short date>", 45000.0), "15/03/2023");
        assert!(is_date_format("dd/mm/yyyy", &Locale::english()) && !is_date_format("0.00", &Locale::english()));
        assert!(!is_date_format("\"day\" 0", &Locale::english()), "letters inside quotes are text");
    }

    #[test]
    fn text_sections() {
        assert_eq!(format_text("@", "abc").text, "abc");
        assert_eq!(format_text("\"Name: \"@", "Ana").text, "Name: Ana");
        assert_eq!(format_text("0.00", "abc").text, "abc");
    }

    #[test]
    fn builtin_codes_cover_the_documented_ids() {
        assert_eq!(builtin_code(2), Some("0.00"));
        assert_eq!(builtin_code(14), Some("<short date>"));
        assert_eq!(en(builtin_code(10).unwrap(), 0.5), "50.00%");
        assert_eq!(builtin_code(164), None);
    }
}
