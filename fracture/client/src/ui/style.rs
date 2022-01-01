pub mod menubar {
    use iced::{button, container, Background, Color};
    pub struct MenuBar;

    impl container::StyleSheet for MenuBar {
        fn style(&self) -> container::Style {
            container::Style {
                background: Some(Background::Color(Color::from_rgb8(255, 255, 255))),
                text_color: Some(Color::BLACK),
                border_radius: 0f32,
                border_width: 0f32,
                ..container::Style::default()
            }
        }
    }

    pub struct Spacer;

    impl container::StyleSheet for Spacer {
        fn style(&self) -> container::Style {
            container::Style {
                background: Some(Background::Color(Color::from_rgb8(220, 220, 220))),
                border_radius: 0f32,
                border_width: 0f32,
                ..container::Style::default()
            }
        }
    }

    pub struct CloseButton;

    impl button::StyleSheet for CloseButton {
        fn active(&self) -> button::Style {
            button::Style {
                background: Some(Background::Color(Color::from_rgb8(174, 25, 25))),
                text_color: Color::WHITE,
                border_radius: 0f32,
                border_width: 0f32,
                border_color: Color::from_rgb8(140, 140, 140),
                ..button::Style::default()
            }
        }
    }
}
