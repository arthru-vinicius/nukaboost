//! Parser mínimo de duração para `--ttl` (seção 12), aceitando um número
//! seguido de uma unidade: `s` (segundos), `m` (minutos), `h` (horas) ou
//! `d` (dias). Deliberadamente simples para não justificar uma dependência
//! externa só para isto — todo lease exige TTL, então o parser precisa ser
//! confiável, não flexível.

use std::time::Duration;

/// Interpreta uma string como `"2h"`, `"30m"`, `"90s"` ou `"1d"`.
pub fn parse_ttl(text: &str) -> Result<Duration, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("empty TTL; use something like '2h', '30m' or '90s'".to_string());
    }

    let (number_part, unit) = text.split_at(text.len() - 1);
    let multiplier_secs: u64 = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3_600,
        "d" => 86_400,
        _ => return Err(format!("unknown TTL unit in '{text}': use s, m, h or d")),
    };

    let amount: u64 = number_part.parse().map_err(|_| {
        format!("invalid TTL value in '{text}': must be an integer followed by s/m/h/d")
    })?;
    if amount == 0 {
        return Err("TTL must be greater than zero".to_string());
    }

    let seconds = amount
        .checked_mul(multiplier_secs)
        .ok_or_else(|| format!("TTL '{text}' is too large"))?;
    if seconds > nukaboost_core::state::lease::MAX_LEASE_TTL.as_secs() {
        return Err(format!(
            "TTL cannot exceed {} days",
            nukaboost_core::state::lease::MAX_LEASE_TTL.as_secs() / 86_400
        ));
    }
    Ok(Duration::from_secs(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_each_unit() {
        assert_eq!(parse_ttl("90s").unwrap(), Duration::from_secs(90));
        assert_eq!(parse_ttl("30m").unwrap(), Duration::from_secs(30 * 60));
        assert_eq!(parse_ttl("2h").unwrap(), Duration::from_secs(2 * 3600));
        assert_eq!(parse_ttl("1d").unwrap(), Duration::from_secs(86_400));
    }

    #[test]
    fn rejects_zero_and_garbage() {
        assert!(parse_ttl("0h").is_err());
        assert!(parse_ttl("").is_err());
        assert!(parse_ttl("2x").is_err());
        assert!(parse_ttl("h").is_err());
        assert!(parse_ttl("8d").is_err());
        assert!(parse_ttl("18446744073709551615d").is_err());
    }
}
