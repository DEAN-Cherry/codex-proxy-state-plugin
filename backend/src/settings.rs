use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub enabled: bool,
    pub models: Vec<String>,
    pub length_rules: String,
    pub retention_hours: u16,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            models: Vec::new(),
            length_rules: String::new(),
            retention_hours: 24,
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<LengthRules, &'static str> {
        if !(1..=168).contains(&self.retention_hours) {
            return Err("保留时间应为 1 到 168 小时");
        }
        if self.models.len() > 32
            || self.models.iter().enumerate().any(|(index, model)| {
                !valid_identifier(model) || self.models[..index].contains(model)
            })
        {
            return Err("模型最多 32 个，不能重复或包含空白及控制字符");
        }
        LengthRules::parse(&self.length_rules)
    }

    pub fn observes(&self, model: &str) -> bool {
        self.enabled && (self.models.is_empty() || self.models.iter().any(|item| item == model))
    }

    pub fn retention_ms(&self) -> u64 {
        u64::from(self.retention_hours) * 3_600_000
    }
}

pub fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !value.chars().any(char::is_whitespace)
        && !value.chars().any(char::is_control)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LengthRules(Vec<(u32, u32)>);

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Validation {
    Matched,
    Mismatch,
    Unrestricted,
}

impl LengthRules {
    pub fn parse(input: &str) -> Result<Self, &'static str> {
        if input.len() > 1024 {
            return Err("长度规则不能超过 1024 字节");
        }
        if input.trim().is_empty() {
            return Ok(Self(Vec::new()));
        }
        let mut ranges = Vec::new();
        for item in input.split(',') {
            let item = item.trim();
            let parse = |text: &str| {
                text.trim()
                    .parse::<u32>()
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or("长度必须为正整数，区间示例 400-500")
            };
            let (min, max) = match item.split_once('-') {
                Some((min, max)) => (parse(min)?, parse(max)?),
                None => {
                    let length = parse(item)?;
                    (length, length)
                }
            };
            if min > max || ranges.contains(&(min, max)) || ranges.len() >= 32 {
                return Err("最多 32 条不同长度规则，区间起点不能大于终点");
            }
            ranges.push((min, max));
        }
        Ok(Self(ranges))
    }

    pub fn classify(&self, length: u32) -> Validation {
        if self.0.is_empty() {
            Validation::Unrestricted
        } else if self
            .0
            .iter()
            .any(|(min, max)| (*min..=*max).contains(&length))
        {
            Validation::Matched
        } else {
            Validation::Mismatch
        }
    }
}
