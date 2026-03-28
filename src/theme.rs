use ratatui::style::Color;

// Tokyo Night colour palette — hardcoded per ADR-08

pub const BG: Color = Color::Rgb(0x1a, 0x1b, 0x26);
pub const FG: Color = Color::Rgb(0xa9, 0xb1, 0xd6);
pub const GREEN: Color = Color::Rgb(0x9e, 0xce, 0x6a);
pub const AMBER: Color = Color::Rgb(0xe0, 0xaf, 0x68);
pub const RED: Color = Color::Rgb(0xf7, 0x76, 0x8e);
pub const BLUE: Color = Color::Rgb(0x7a, 0xa2, 0xf7);
pub const MUTED: Color = Color::Rgb(0x56, 0x5f, 0x89);
pub const STATUS_BAR_BG: Color = Color::Rgb(0x16, 0x16, 0x1e);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bg_matches_tokyo_night() {
        assert_eq!(BG, Color::Rgb(0x1a, 0x1b, 0x26));
    }

    #[test]
    fn fg_matches_tokyo_night() {
        assert_eq!(FG, Color::Rgb(0xa9, 0xb1, 0xd6));
    }

    #[test]
    fn green_matches_tokyo_night() {
        assert_eq!(GREEN, Color::Rgb(0x9e, 0xce, 0x6a));
    }

    #[test]
    fn amber_matches_tokyo_night() {
        assert_eq!(AMBER, Color::Rgb(0xe0, 0xaf, 0x68));
    }

    #[test]
    fn red_matches_tokyo_night() {
        assert_eq!(RED, Color::Rgb(0xf7, 0x76, 0x8e));
    }

    #[test]
    fn blue_matches_tokyo_night() {
        assert_eq!(BLUE, Color::Rgb(0x7a, 0xa2, 0xf7));
    }

    #[test]
    fn muted_matches_tokyo_night() {
        assert_eq!(MUTED, Color::Rgb(0x56, 0x5f, 0x89));
    }

    #[test]
    fn status_bar_bg_matches_tokyo_night() {
        assert_eq!(STATUS_BAR_BG, Color::Rgb(0x16, 0x16, 0x1e));
    }
}
