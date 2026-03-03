//! Theme constants and colors for the DriveWipe GUI.

use iced::Color;

// Primary colors
pub const PRIMARY: Color = Color::from_rgb(0.2, 0.6, 0.9); // Cyan-blue
pub const PRIMARY_DARK: Color = Color::from_rgb(0.1, 0.4, 0.7);
pub const SECONDARY: Color = Color::from_rgb(0.3, 0.8, 0.3); // Green
pub const DANGER: Color = Color::from_rgb(0.9, 0.2, 0.2); // Red
pub const WARNING: Color = Color::from_rgb(0.9, 0.7, 0.1); // Yellow

// Background colors
pub const BG_DARK: Color = Color::from_rgb(0.12, 0.12, 0.15);
pub const BG_MEDIUM: Color = Color::from_rgb(0.18, 0.18, 0.22);
pub const BG_LIGHT: Color = Color::from_rgb(0.25, 0.25, 0.30);

// Text colors
pub const TEXT_PRIMARY: Color = Color::from_rgb(0.9, 0.9, 0.9);
pub const TEXT_SECONDARY: Color = Color::from_rgb(0.6, 0.6, 0.65);
pub const TEXT_MUTED: Color = Color::from_rgb(0.4, 0.4, 0.45);

// Status colors
pub const STATUS_HEALTHY: Color = Color::from_rgb(0.2, 0.8, 0.2);
pub const STATUS_WARNING: Color = Color::from_rgb(0.9, 0.7, 0.1);
pub const STATUS_ERROR: Color = Color::from_rgb(0.9, 0.2, 0.2);
pub const STATUS_INFO: Color = Color::from_rgb(0.3, 0.6, 0.9);

// Spacing constants
pub const SPACING_SM: u16 = 4;
pub const SPACING_MD: u16 = 8;
pub const SPACING_LG: u16 = 16;
pub const SPACING_XL: u16 = 24;

// Font sizes
pub const FONT_SIZE_SM: u16 = 12;
pub const FONT_SIZE_MD: u16 = 14;
pub const FONT_SIZE_LG: u16 = 18;
pub const FONT_SIZE_XL: u16 = 24;
pub const FONT_SIZE_TITLE: u16 = 32;
