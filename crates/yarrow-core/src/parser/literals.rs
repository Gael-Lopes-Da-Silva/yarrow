//! Decoding of literal lexemes into values.
//!
//! The tokenizer produces raw lexemes (`"-900"`, `"0x1F"`, `"'\n'"`, `"1e3"`)
//! and the AST stores them losslessly as `String`. These helpers turn a lexeme
//! into a semantic value. The parser calls them to validate literals at the
//! token's real source location; the compiler reuses them during lowering.
//!
//! Integer literals decode to `i128`, the widest signed type, so every integer
//! primitive (`i8`..`i128`, and `u8`..`u128` up to `i128::MAX`) round-trips.

/// Decode an integer literal lexeme (`"-900"`, `"+0x1F"`, `"-0b101"`, `"1_000"`)
/// into its `i128` value. Sign and radix prefixes are handled; `_` is ignored.
pub fn decode_int_literal(s: &str) -> Result<i128, String> {
    let clean = s.replace('_', "");
    let (sign, unsigned) = match clean.strip_prefix('-') {
        Some(rest) => (-1i128, rest),
        None => (1i128, clean.strip_prefix('+').unwrap_or(&clean)),
    };
    let (radix, digits): (u32, &str) = if let Some(hex) = unsigned
        .strip_prefix("0x")
        .or_else(|| unsigned.strip_prefix("0X"))
    {
        (16, hex)
    } else if let Some(bin) = unsigned
        .strip_prefix("0b")
        .or_else(|| unsigned.strip_prefix("0B"))
    {
        (2, bin)
    } else {
        (10, unsigned)
    };
    let magnitude = i128::from_str_radix(digits, radix)
        .map_err(|_| format!("invalid integer literal '{s}'"))?;
    Ok(sign * magnitude)
}

/// Decode a float literal lexeme (`"3.14"`, `"-1.5e-3"`) into an `f64`.
pub fn decode_float_literal(s: &str) -> Result<f64, String> {
    s.replace('_', "")
        .parse::<f64>()
        .map_err(|_| format!("invalid float literal '{s}'"))
}

/// Decode a rune literal lexeme (`'a'`, `'\n'`) into a Unicode code point.
pub fn decode_rune_literal(s: &str) -> Result<u32, String> {
    let inner = s
        .strip_prefix('\'')
        .and_then(|x| x.strip_suffix('\''))
        .ok_or_else(|| format!("invalid rune literal '{s}'"))?;
    let mut chars = inner.chars();
    let ch = chars
        .next()
        .ok_or_else(|| format!("empty rune literal '{s}'"))?;
    if ch != '\\' {
        if chars.next().is_some() {
            return Err(format!("rune must be a single character: '{s}'"));
        }
        return Ok(ch as u32);
    }
    let esc = chars
        .next()
        .ok_or_else(|| format!("incomplete escape in rune '{s}'"))?;
    let cp = match esc {
        'n' => 0x0a,
        'r' => 0x0d,
        't' => 0x09,
        'v' => 0x0b,
        'b' => 0x08,
        'a' => 0x07,
        'f' => 0x0c,
        '\\' => 0x5c,
        '\'' => 0x27,
        '"' => 0x22,
        _ => return Err(format!("invalid escape sequence in rune '{s}'")),
    };
    Ok(cp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_decimal_literals() {
        assert_eq!(decode_int_literal("-900").unwrap(), -900i128);
        assert_eq!(decode_int_literal("+300").unwrap(), 300i128);
        assert_eq!(decode_int_literal("42").unwrap(), 42i128);
    }

    #[test]
    fn signed_radix_literals() {
        assert_eq!(decode_int_literal("-0x10").unwrap(), -16i128);
        assert_eq!(decode_int_literal("+0x1F").unwrap(), 31i128);
        assert_eq!(decode_int_literal("-0b101").unwrap(), -5i128);
        assert_eq!(decode_int_literal("0b11").unwrap(), 3i128);
    }

    #[test]
    fn underscores_are_ignored() {
        assert_eq!(decode_int_literal("1_000_000").unwrap(), 1_000_000i128);
        assert_eq!(decode_int_literal("-0xF_F").unwrap(), -255i128);
    }

    #[test]
    fn wide_integer_round_trip() {
        let big = "170141183460469231731687303715884105727"; // i128::MAX
        assert_eq!(decode_int_literal(big).unwrap(), i128::MAX);
        assert!(decode_int_literal("340282366920938463463374607431768211456").is_err());
    }

    #[test]
    fn float_literals() {
        assert_eq!(decode_float_literal("1.25").unwrap(), 1.25);
        assert_eq!(decode_float_literal("-1.5e-3").unwrap(), -1.5e-3);
        assert_eq!(decode_float_literal("1_000.5").unwrap(), 1000.5);
    }

    #[test]
    fn rune_literals() {
        assert_eq!(decode_rune_literal("'a'").unwrap(), 'a' as u32);
        assert_eq!(decode_rune_literal("'\\n'").unwrap(), 0x0a);
        assert_eq!(decode_rune_literal("'\\''").unwrap(), 0x27);
        assert_eq!(decode_rune_literal("'€'").unwrap(), 0x20ac);
        assert!(decode_rune_literal("'ab'").is_err());
    }
}
