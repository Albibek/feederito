mod default;

use std::collections::HashMap;

use anyhow::Error;
use serde::{Deserialize, Serialize};

use crate::default::DefaultTheme;
use tailwind_css::TailwindBuilder;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ThemeConfig {
    main_button: String,
    read_button: String,
}

#[derive(Debug)]
pub struct Theme {
    classes: HashMap<String, String>,
}

//#[derive(Debug)]
//struct ThemeEntry {
//scope: String,
//scope_data: String,
//}

fn main() -> Result<(), Error> {
    let dir = std::env::args()
        .take(2)
        .last()
        .expect("dir required as first argument");
    let path = std::path::Path::new(&dir).join("css").join("tachyons.css");
    let css_data = std::fs::read_to_string(path.clone()).unwrap();

    let options = parcel_css::stylesheet::ParserOptions {
        nesting: true,
        custom_media: true,
        //css_modules: Some(false),
        css_modules: None,
        source_index: 0,
    };
    let stylesheet =
        parcel_css::stylesheet::StyleSheet::parse(&path.to_str().unwrap(), &css_data, options)
            .unwrap();

    let mut poptions = parcel_css::printer::PrinterOptions::default();
    poptions.minify = true;

    use parcel_css::printer::PrinterOptions;
    use parcel_css::rules::CssRule;
    //use parcel_css::traits::ToCss;
    use parcel_selectors::parser::Component;

    let mut classes = HashMap::new();
    for rule in &stylesheet.rules.0 {
        if let CssRule::Style(rule) = rule {
            for sel in &rule.selectors.0 {
                for comp in sel.iter() {
                    if let Component::Class(id) = comp {
                        // id should have a class name here
                        //println!("{:?}, {:?}", comp, &id.0);

                        let mut decls = Vec::new();
                        for prop in rule.declarations.important_declarations.iter() {
                            let mut opts = PrinterOptions::default();
                            opts.minify = true;
                            let decl = prop.to_css_string(true, opts).unwrap();
                            decls.push(decl);
                        }
                        for prop in rule.declarations.declarations.iter() {
                            let mut opts = PrinterOptions::default();
                            opts.minify = true;
                            let decl = prop.to_css_string(false, opts).unwrap();
                            decls.push(decl);
                        }
                        classes.insert(id.0.to_string(), decls);
                    }
                }
            }
        }
    }

    println!("{:?}", classes);

    let mut poptions = parcel_css::printer::PrinterOptions::default();
    poptions.minify = true;

    let res = stylesheet.to_css(poptions).unwrap();
    println!("OK");
    if false {
        println!("CSS: {}", res.code);
    }

    /*
    let theme_config = DefaultTheme::default();
    let mut theme = Theme {
        classes: HashMap::new(),
    };
    let mut tailwind = TailwindBuilder::default();

    //use tailwind_css::TailwindInstance;
    //let ab = tailwind_css::TailwindArbitrary::from("");
    //let bgcolor = tailwind_css::TailwindBackgroundColor::parse(&["green", "900"], &ab).unwrap();
    //println!("{:?}", bgcolor.attributes(&tailwind).to_string());
    macro_rules! theme_field {
        ($name:expr, $field:ident) => {
            if theme_config.0.$field.len() > 0 {
                let (_, scope) = tailwind.scope(&theme_config.0.$field).unwrap();
                theme.classes.insert(
                    concat!("rss-", $name).to_string(),
                    ThemeEntry {
                        scope,
                        scope_data: String::new(),
                    },
                );
            }
        };
    }

    theme_field!("main-button", read_button);
    theme_field!("read-button", read_button);

    println!("{:?}", theme);

    let bundle = tailwind.bundle().unwrap();
     */

    //let first_arg = std::env::args().take(2).last();
    //if let Some(dir) = first_arg {
    //let path = Path::new(&dir).join("css").join("main.scss");
    //println!("{:?}", path);
    //let sass = grass::from_path(path.to_str().unwrap(), &grass::Options::default()).unwrap();
    //println!("{:?}", std::env::args().take(1));
    //println!("{:?}", sass);
    //} else {
    //println!("{:?}", first_arg);
    //}
    //
    //
    //println!("{}", bundle);
    Ok(())
    //Err(anyhow::anyhow!("TODO"))
}
