use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    All,
    IndexesOnly,
    TriggersOnly,
}

impl OutputMode {
    pub fn includes_indexes(self) -> bool {
        matches!(self, OutputMode::All | OutputMode::IndexesOnly)
    }
    pub fn includes_triggers(self) -> bool {
        matches!(self, OutputMode::All | OutputMode::TriggersOnly)
    }
    pub fn includes_tables(self) -> bool {
        matches!(self, OutputMode::All)
    }
    pub fn includes_views(self) -> bool { matches!(self, OutputMode::All) }
    pub fn includes_routines(self) -> bool { matches!(self, OutputMode::All) }
    pub fn includes_other_sql(self) -> bool { matches!(self, OutputMode::All) }
}

impl FromStr for OutputMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "all" => Ok(OutputMode::All),
            "indexes-only" => Ok(OutputMode::IndexesOnly),
            "triggers-only" => Ok(OutputMode::TriggersOnly),
            _ => Err(format!("Unknown output mode: {}", s)),
        }
    }
}
