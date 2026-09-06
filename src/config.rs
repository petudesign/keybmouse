#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Letter(char), Digit(u8), Function(u8), CapsLock, Enter, Space, LeftShift, RightShift,
    Tab, Backspace, Escape, Up, Down, Left, Right,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub activation: Key,
    /// Up, left, down, right.
    pub movement: [Key; 4],
    /// Left, right mouse button.
    pub clicks: [Key; 2],
    /// Up, down.
    pub scroll: [Key; 2],
    pub precision: [Key; 2],
    pub base_speed: f64,
    pub max_speed: f64,
    pub acceleration: f64,
    pub precision_multiplier: f64,
    pub scroll_notches_per_second: f64,
    pub direction_grace_ms: f64,
}

impl Default for Config {
    fn default() -> Self {
        use Key::*;
        Self {
            activation: CapsLock,
            movement: [Letter('W'), Letter('A'), Letter('S'), Letter('D')],
            clicks: [Enter, Space],
            scroll: [Letter('Q'), Letter('E')],
            precision: [LeftShift, RightShift],
            base_speed: 110.0,
            max_speed: 1100.0,
            acceleration: 1500.0,
            precision_multiplier: 0.18,
            scroll_notches_per_second: 6.0,
            direction_grace_ms: 100.0,
        }
    }
}

impl Key {
    pub fn choices() -> Vec<Self> {
        use Key::*;
        [CapsLock, Enter, Space, LeftShift, RightShift, Tab, Backspace, Escape, Up, Down, Left, Right]
            .into_iter().chain(('A'..='Z').map(Letter)).chain((0..=9).map(Digit))
            .chain((1..=12).map(Function)).collect()
    }
    pub fn label(self) -> String {
        match self {
            Self::Letter(c) => c.to_string(), Self::Digit(n) => n.to_string(),
            Self::Function(n) => format!("F{n}"), other => format!("{other:?}"),
        }
    }
    pub fn parse(value: &str) -> Result<Self, String> {
        Self::choices().into_iter().find(|key| key.label() == value)
            .ok_or_else(|| format!("Tuntematon näppäin: {value}"))
    }
}

impl Config {
    pub fn bindings(&self) -> [Key; 11] {
        [self.activation, self.movement[0], self.movement[1], self.movement[2], self.movement[3],
            self.clicks[0], self.clicks[1], self.scroll[0], self.scroll[1], self.precision[0], self.precision[1]]
    }
    pub fn set_bindings(&mut self, keys: [Key; 11]) {
        self.activation = keys[0]; self.movement.copy_from_slice(&keys[1..5]);
        self.clicks.copy_from_slice(&keys[5..7]); self.scroll.copy_from_slice(&keys[7..9]);
        self.precision.copy_from_slice(&keys[9..11]);
    }
    pub fn values(&self) -> [f64; 6] {
        [self.base_speed, self.max_speed, self.acceleration, self.precision_multiplier,
            self.scroll_notches_per_second, self.direction_grace_ms]
    }
    pub fn validate(&self) -> Result<(), String> {
        let keys = self.bindings();
        let mut seen = std::collections::HashSet::new();
        for key in keys {
            if !Key::choices().contains(&key) { return Err("Virheellinen näppäin.".into()); }
            if !seen.insert(key) { return Err(format!("{} on käytössä kahdesti. Valitse eri näppäimet.", key.label())); }
        }
        for (value, low, high, name) in [
            (self.base_speed, 1.0, 5000.0, "Lähtönopeus"),
            (self.max_speed, 1.0, 5000.0, "Enimmäisnopeus"),
            (self.acceleration, 0.0, 20000.0, "Kiihdytys"),
            (self.precision_multiplier, 0.01, 1.0, "Tarkkuuskerroin"),
            (self.scroll_notches_per_second, 0.1, 60.0, "Vieritysnopeus"),
            (self.direction_grace_ms, 0.0, 300.0, "Suunnanvaihdon tauko"),
        ] {
            if !value.is_finite() || !(low..=high).contains(&value) {
                return Err(format!("{name}: sallitut arvot {low}–{high}."));
            }
        }
        if self.base_speed > self.max_speed { return Err("Lähtönopeus ei voi ylittää enimmäisnopeutta.".into()); }
        Ok(())
    }
    pub fn encode(&self) -> String {
        let mut lines = vec!["version=1".to_string()];
        for (i, key) in self.bindings().iter().enumerate() { lines.push(format!("key{i}={}", key.label())); }
        for (name, value) in [
            ("base_speed", self.base_speed), ("max_speed", self.max_speed), ("acceleration", self.acceleration),
            ("precision_multiplier", self.precision_multiplier),
            ("scroll_notches_per_second", self.scroll_notches_per_second), ("direction_grace_ms", self.direction_grace_ms),
        ] { lines.push(format!("{name}={value}")); }
        lines.join("\n") + "\n"
    }
    pub fn decode(text: &str) -> Result<Self, String> {
        let mut entries = std::collections::HashMap::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let (key, value) = line.split_once('=').ok_or("Virheellinen asetusrivi.")?;
            if entries.insert(key.trim(), value.trim()).is_some() { return Err("Asetus esiintyy kahdesti.".into()); }
        }
        if entries.remove("version") != Some("1") { return Err("Tuntematon asetustiedoston versio.".into()); }
        let mut config = Self::default();
        let mut bindings = config.bindings();
        for (i, key) in bindings.iter_mut().enumerate() {
            *key = Key::parse(entries.remove(format!("key{i}").as_str()).ok_or("Näppäinsidonta puuttuu.")?)?;
        }
        config.set_bindings(bindings);
        for (name, value) in [
            ("base_speed", &mut config.base_speed), ("max_speed", &mut config.max_speed),
            ("acceleration", &mut config.acceleration), ("precision_multiplier", &mut config.precision_multiplier),
            ("scroll_notches_per_second", &mut config.scroll_notches_per_second), ("direction_grace_ms", &mut config.direction_grace_ms),
        ] {
            *value = entries.remove(name).ok_or_else(|| format!("Asetus puuttuu: {name}"))?
                .parse().map_err(|_| format!("Virheellinen numero: {name}"))?;
        }
        if !entries.is_empty() { return Err("Tuntematon asetus tiedostossa.".into()); }
        config.validate()?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_round_trip_and_reject_invalid_data() {
        let mut config = Config::default(); config.activation = Key::Function(8);
        config.base_speed = 200.0;
        assert_eq!(Config::decode(&config.encode()).unwrap(), config);
        assert!(Config::decode(&config.encode().replace("base_speed=200", "base_speed=NaN")).is_err());
        assert!(Config::decode(&(config.encode() + "max_speed=100\n")).is_err());
        assert!(Config::decode(&config.encode().replace("key1=W", "key1=Enter")).is_err());
        config.base_speed = 2000.0; assert!(config.validate().is_err());
    }
}
