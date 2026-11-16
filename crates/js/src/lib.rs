//! A JavaScript interpreter for controller mapping scripts.

pub mod ast;
pub mod lexer;
pub mod parser;

/// The shortest text that reads back as the same number, as JavaScript
/// writes numbers.
pub fn number_to_string(v: f64) -> String {
    if v.is_nan() {
        return "NaN".into();
    }
    if v == 0.0 {
        return "0".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let neg = v < 0.0;
    let a = v.abs();
    // Rust's `{:e}` gives the shortest round-tripping digits.
    let e = format!("{a:e}");
    let (mant, exp) = e.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exp + 1;
    let mut s = String::new();
    if neg {
        s.push('-');
    }
    if k <= n && n <= 21 {
        s += &digits;
        s.extend(std::iter::repeat_n('0', (n - k) as usize));
    } else if 0 < n && n <= 21 {
        s += &digits[..n as usize];
        s.push('.');
        s += &digits[n as usize..];
    } else if -6 < n && n <= 0 {
        s += "0.";
        s.extend(std::iter::repeat_n('0', (-n) as usize));
        s += &digits;
    } else {
        s += &digits[..1];
        if k > 1 {
            s.push('.');
            s += &digits[1..];
        }
        s.push('e');
        s.push(if n > 0 { '+' } else { '-' });
        s += &(n - 1).abs().to_string();
    }
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn numbers_print_like_javascript() {
        for (v, s) in [
            (1.0, "1"),
            (0.1, "0.1"),
            (1e21, "1e+21"),
            (1e-7, "1e-7"),
            (123.456, "123.456"),
            (-0.000001, "-0.000001"),
            (1.5e300, "1.5e+300"),
            (100.0, "100"),
        ] {
            assert_eq!(super::number_to_string(v), s);
        }
    }
}
