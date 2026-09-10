use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Unit {
    pub dimensions: BTreeMap<String, i32>,
    pub scale: f64,
    pub display: String,
}

impl Unit {
    pub fn dimensionless() -> Self {
        Self {
            dimensions: BTreeMap::new(),
            scale: 1.0,
            display: "1".into(),
        }
    }
    pub fn compatible(&self, other: &Self) -> bool {
        self.dimensions == other.dimensions
    }
    pub fn mul(&self, other: &Self) -> Self {
        let mut dimensions = self.dimensions.clone();
        for (k, v) in &other.dimensions {
            *dimensions.entry(k.clone()).or_default() += v;
        }
        dimensions.retain(|_, v| *v != 0);
        Self {
            dimensions,
            scale: self.scale * other.scale,
            display: format!("{}*{}", self.display, other.display),
        }
    }
    pub fn div(&self, other: &Self) -> Self {
        let mut dimensions = self.dimensions.clone();
        for (k, v) in &other.dimensions {
            *dimensions.entry(k.clone()).or_default() -= v;
        }
        dimensions.retain(|_, v| *v != 0);
        Self {
            dimensions,
            scale: self.scale / other.scale,
            display: format!("{}/{}", self.display, other.display),
        }
    }
}

pub fn parse(text: Option<&str>) -> Result<Unit, String> {
    let Some(text) = text else {
        return Ok(Unit::dimensionless());
    };
    if text == "1" || text.is_empty() {
        return Ok(Unit::dimensionless());
    }
    let chars: Vec<char> = text.chars().collect();
    let mut at = 0;
    let mut result = Unit::dimensionless();
    let mut divide = false;
    while at < chars.len() {
        if chars[at] == '*' {
            divide = false;
            at += 1;
            continue;
        }
        if chars[at] == '/' {
            divide = true;
            at += 1;
            continue;
        }
        let begin = at;
        while at < chars.len() && (chars[at].is_ascii_alphanumeric() || chars[at] == '_') {
            at += 1;
        }
        if begin == at {
            return Err(format!("invalid unit expression '{text}'"));
        }
        let name: String = chars[begin..at].iter().collect();
        let mut exponent = 1i32;
        if chars.get(at) == Some(&'^') {
            at += 1;
            let negative = chars.get(at) == Some(&'-');
            if negative {
                at += 1;
            }
            let e0 = at;
            while at < chars.len() && chars[at].is_ascii_digit() {
                at += 1;
            }
            if e0 == at {
                return Err(format!("missing exponent in unit '{text}'"));
            }
            exponent = chars[e0..at]
                .iter()
                .collect::<String>()
                .parse()
                .map_err(|_| format!("invalid exponent in unit '{text}'"))?;
            if negative {
                exponent = -exponent;
            }
        }
        if divide {
            exponent = -exponent;
        }
        let atom = atom(&name).ok_or_else(|| format!("unknown unit '{name}'"))?;
        let mut powered = Unit::dimensionless();
        powered.display = name;
        powered.scale = atom.scale.powi(exponent);
        for (k, v) in atom.dimensions {
            powered.dimensions.insert(k, v * exponent);
        }
        result = result.mul(&powered);
    }
    result.display = text.into();
    Ok(result)
}

fn atom(name: &str) -> Option<Unit> {
    let (scale, terms): (f64, &[(&str, i32)]) = match name {
        "m" => (1.0, &[("length", 1)]),
        "km" => (1e3, &[("length", 1)]),
        "s" => (1.0, &[("time", 1)]),
        "Hz" => (1.0, &[("time", -1)]),
        "kHz" => (1e3, &[("time", -1)]),
        "MHz" => (1e6, &[("time", -1)]),
        "kg" => (1.0, &[("mass", 1)]),
        "g" => (1e-3, &[("mass", 1)]),
        "A" => (1.0, &[("current", 1)]),
        "K" => (1.0, &[("temperature", 1)]),
        "mol" => (1.0, &[("amount", 1)]),
        "cd" => (1.0, &[("luminosity", 1)]),
        "J" => (1.0, &[("mass", 1), ("length", 2), ("time", -2)]),
        "kJ" => (1e3, &[("mass", 1), ("length", 2), ("time", -2)]),
        "N" => (1.0, &[("mass", 1), ("length", 1), ("time", -2)]),
        "Pa" => (1.0, &[("mass", 1), ("length", -1), ("time", -2)]),
        "rad" => (1.0, &[]),
        _ => return None,
    };
    Some(Unit {
        dimensions: terms.iter().map(|(k, v)| ((*k).into(), *v)).collect(),
        scale,
        display: name.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn derived_unit_matches() {
        assert!(
            parse(Some("kg*m^2/s^2"))
                .unwrap()
                .compatible(&parse(Some("J")).unwrap())
        );
    }
}
