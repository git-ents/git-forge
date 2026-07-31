//! [`Status`]: the open/closed lifecycle shared by issues and reviews.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Open,
    Closed,
}

impl Status {
    pub const fn as_str(self) -> &'static str {
        match self {
            Status::Open => "open",
            Status::Closed => "closed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "open" => Some(Status::Open),
            "closed" => Some(Status::Closed),
            _ => None,
        }
    }
}
