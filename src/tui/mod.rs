pub mod background;
pub mod log;
pub mod popup;
pub mod source;
pub mod tool;

use ratatui::layout::Rect;

/// Calculate centered popup area (80% width × 80% height, min 40×12)
pub fn popup_area(area: Rect) -> Rect {
    let width = (area.width * 80 / 100).max(40).min(area.width);
    let height = (area.height * 80 / 100).max(12).min(area.height);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}

/// Calculate small centered popup area for dialogs (50% width, auto height)
pub fn dialog_area(area: Rect, content_height: u16) -> Rect {
    let width = (area.width * 50 / 100).max(30).min(area.width);
    let height = (content_height + 4).min(area.height); // +4 for borders+padding
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_popup_area_normal() {
        let area = Rect::new(0, 0, 100, 40);
        let popup = popup_area(area);
        assert_eq!(popup, Rect::new(10, 4, 80, 32));
    }

    #[test]
    fn test_popup_area_tiny() {
        let area = Rect::new(0, 0, 20, 6);
        let popup = popup_area(area);
        assert_eq!(popup, Rect::new(0, 0, 20, 6));
    }

    #[test]
    fn test_popup_area_large() {
        let area = Rect::new(0, 0, 200, 60);
        let popup = popup_area(area);
        assert_eq!(popup, Rect::new(20, 6, 160, 48));
    }

    #[test]
    fn test_dialog_area_normal() {
        let area = Rect::new(0, 0, 100, 40);
        let dialog = dialog_area(area, 5);
        assert_eq!(dialog, Rect::new(25, 15, 50, 9));
    }

    #[test]
    fn test_dialog_area_tall_content() {
        let area = Rect::new(0, 0, 100, 10);
        let dialog = dialog_area(area, 20);
        assert_eq!(dialog, Rect::new(25, 0, 50, 10));
    }

    #[test]
    fn test_popup_area_zero() {
        let area = Rect::new(0, 0, 0, 0);
        let popup = popup_area(area);
        assert_eq!(popup, Rect::new(0, 0, 0, 0));
    }
}