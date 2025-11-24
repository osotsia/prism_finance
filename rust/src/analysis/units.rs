use std::collections::HashMap;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ParsedUnit {
    terms: HashMap<String, i32>,
}

impl ParsedUnit {
    pub fn from_str(s: &str) -> Result<Self, ()> {
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
        if d_str == "1" { if n_str == "1" { "".into() } else { n_str } } else { format!("{}/{}", n_str, d_str) }
    }
}