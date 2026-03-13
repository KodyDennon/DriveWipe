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
pub const SPACING_SM: f32 = 4.0;
pub const SPACING_MD: f32 = 8.0;
pub const SPACING_LG: f32 = 16.0;
pub const SPACING_XL: f32 = 24.0;

// Font sizes
pub const FONT_SIZE_SM: f32 = 12.0;
pub const FONT_SIZE_MD: f32 = 14.0;
pub const FONT_SIZE_LG: f32 = 18.0;
pub const FONT_SIZE_XL: f32 = 24.0;
pub const FONT_SIZE_TITLE: f32 = 32.0;
