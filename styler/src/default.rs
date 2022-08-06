use crate::ThemeConfig;

pub struct DefaultTheme(pub ThemeConfig);

impl Default for DefaultTheme {
    fn default() -> Self {
        DefaultTheme(ThemeConfig {
            main_button: "rounded-full".to_string(),
            read_button: "rounded-md".to_string(),
        })
    }
}
