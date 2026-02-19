// rust/src/analysis/units.rs

use std::collections::HashMap;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ParsedUnit {
    terms: HashMap<String, i32>,
}

impl ParsedUnit {
    pub fn from_str(s: &str) -> Result<Self, ()> {
        // Fix 2: Reject empty or whitespace-only strings explicitly
        if s.trim().is_empty() { return Err(()); }

        let mut terms = HashMap::new();
        let mut parts = s.split('/');
        
        if let Some(num) = parts.next() { Self::parse_product(num, 1, &mut terms)?; }
        if let Some(den) = parts.next() { Self::parse_product(den, -1, &mut terms)?; }
        if parts.next().is_some() { return Err(()); } // Multiple slashes

        Ok(Self { terms })
    }

    fn parse_product(s: &str, sign: i32, terms: &mut HashMap<String, i32>) -> Result<(), ()> {
        if s.trim().is_empty() || s == "1" { return Ok(()); }
        for factor in s.split('*') {
            let mut parts = factor.split('^');
            let base = parts.next().ok_or(())?.trim();
            if base.is_empty() { return Err(()); }
            let exp = parts.next().unwrap_or("1").parse::<i32>().map_err(|_| ())?;
            *terms.entry(base.to_string()).or_insert(0) += exp * sign;
        }
        Ok(())
    }

    pub fn multiply(&mut self, other: &Self) {
        for (k, v) in &other.terms { *self.terms.entry(k.clone()).or_insert(0) += v; }
    }

    pub fn divide(&mut self, other: &Self) {
        for (k, v) in &other.terms { *self.terms.entry(k.clone()).or_insert(0) -= v; }
    }

    pub fn to_string(&self) -> String {
        let (num, den): (Vec<_>, Vec<_>) = self.terms.iter().filter(|&(_, &v)| v != 0).partition(|&(_, &v)| v > 0);
        
        let fmt = |terms: Vec<(&String, &i32)>| -> String {
            if terms.is_empty() { return "1".to_string(); }
            let mut t = terms; t.sort_by_key(|a| a.0);
            t.into_iter().map(|(k, v)| if v.abs() == 1 { k.clone() } else { format!("{}^{}", k, v.abs()) }).collect::<Vec<_>>().join("*")
        };
        
        let n_str = fmt(num);
        let d_str = fmt(den);
        
        // Fix 1: If denominator is 1, return numerator (even if it is "1")
        if d_str == "1" { 
            n_str 
        } else { 
            format!("{}/{}", n_str, d_str) 
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid() {
        let cases = vec![
            ("USD", "USD"),
            ("m/s", "m/s"),
            ("m*m", "m^2"),
            ("m^2/m", "m"), // Cancellation logic check
            ("1", "1"),     // Identity
        ];

        for (input, expected) in cases {
            let u = ParsedUnit::from_str(input).expect("Failed to parse");
            assert_eq!(u.to_string(), expected, "Input: {}", input);
        }
    }

    #[test]
    fn test_parse_invalid() {
        let failures = vec![
            "",           // Empty
            "   ",        // Whitespace
            "USD//MWh",   // Double slash
            "USD^bar",    // Non-numeric exponent
        ];

        for input in failures {
            assert!(ParsedUnit::from_str(input).is_err(), "Should fail: '{}'", input);
        }
    }

    #[test]
    fn test_arithmetic() {
        // (kg * m / s^2) * s = kg * m / s
        let mut force = ParsedUnit::from_str("kg*m/s^2").unwrap();
        let time = ParsedUnit::from_str("s").unwrap();
        
        force.multiply(&time);
        assert_eq!(force.to_string(), "kg*m/s");
    }
}