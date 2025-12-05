#[derive(Debug, Default, Clone)]
pub struct LangStats {
    pub total_added: i64,
    pub total_removed: i64,
    pub pure_added: i64,
    pub pure_removed: i64,
}

impl LangStats {
    pub fn net_total(&self) -> i64 {
        self.total_added - self.total_removed
    }

    pub fn net_pure(&self) -> i64 {
        self.pure_added - self.pure_removed
    }
}
