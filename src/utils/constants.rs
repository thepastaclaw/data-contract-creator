//! Application constants

/// Maximum length for indexed string properties in Dash Platform
pub const MAX_INDEXED_STRING_LENGTH: u32 = 63;

/// Maximum items for indexed array properties in Dash Platform
pub const MAX_INDEXED_ARRAY_ITEMS: u32 = 255;

/// Default formats for string properties
pub const STRING_FORMATS: &[&str] = &[
    "uri",
    "email",
    "date",
    "date-time",
    "time",
    "hostname",
    "ipv4",
    "ipv6",
    "uuid",
];

/// Available sort orders (Dash Platform only supports ascending)
pub const SORT_ORDERS: &[&str] = &["asc"];

/// System properties that can be automatically added
pub const SYSTEM_PROPERTIES: &[&str] = &["$createdAt", "$updatedAt"];

/// OpenAI model to use
pub const OPENAI_MODEL: &str = "gpt-5-mini";

/// Maximum completion tokens for OpenAI responses (`max_completion_tokens`).
/// gpt-5-mini uses `max_completion_tokens`, not the deprecated `max_tokens`.
pub const OPENAI_MAX_COMPLETION_TOKENS: u32 = 8192;
