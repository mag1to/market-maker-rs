use once_cell::sync::Lazy;
use std::sync::{Arc, RwLock};

use log::LevelFilter;
use log4rs::append::console::ConsoleAppender;
use log4rs::config::{Appender, Config, Logger, Root};
use log4rs::encode::pattern::PatternEncoder;
use log4rs::Handle;

static HANDLE: Lazy<Arc<RwLock<Option<Handle>>>> = Lazy::new(|| Arc::new(RwLock::new(None)));

const PATTERN: &str = "[{d(%Y-%m-%dT%H:%M:%S.%fZ)(utc)} {h({l:5.5})} {M}] {m}{n}";

fn parse_level(level: &str) -> LevelFilter {
    match level {
        "trace" => LevelFilter::Trace,
        "debug" => LevelFilter::Debug,
        "info" => LevelFilter::Info,
        "warn" => LevelFilter::Warn,
        "error" => LevelFilter::Error,
        _ => panic!("invalid level"),
    }
}

fn parse_filter(filter: &str) -> (LevelFilter, Vec<(&str, LevelFilter)>) {
    let mut root_level: LevelFilter = LevelFilter::Off;
    let mut pairs: Vec<(&str, LevelFilter)> = Vec::new();

    if filter.is_empty() {
        return (root_level, pairs);
    }

    for (index, word) in filter.split(',').enumerate() {
        if !word.contains('=') {
            if index == 0 {
                root_level = parse_level(word);
                continue;
            } else {
                panic!("invalid filter: not contains `=` but not first word");
            }
        }

        let word_split: Vec<&str> = word.split('=').collect();
        if word_split.len() == 2 {
            let name = word_split[0];
            let level = parse_level(word_split[1]);
            pairs.push((name, level));
        } else {
            panic!("invalid filter: contains multiple `=` in a word");
        }
    }

    (root_level, pairs)
}

fn set_config(config: Config) {
    let mut guard = HANDLE.write().unwrap();
    if let Some(handle) = guard.as_ref() {
        handle.set_config(config);
    } else {
        *guard = Some(log4rs::init_config(config).unwrap());
    }
}

pub fn setup_with(filter: &str) {
    let (root_level, pairs) = parse_filter(filter);

    let console_appender = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new(PATTERN)))
        .build();

    let mut builder = Config::builder()
        .appender(Appender::builder().build("console", Box::new(console_appender)));

    for (name, level) in pairs {
        builder = builder.logger(Logger::builder().build(name, level));
    }

    let config = builder
        .build(Root::builder().appender("console").build(root_level))
        .unwrap();

    set_config(config);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logger_parse_filter() {
        assert_eq!((LevelFilter::Off, vec![]), parse_filter(""));
        assert_eq!((LevelFilter::Info, vec![]), parse_filter("info"));
        assert_eq!(
            (LevelFilter::Off, vec![("foo", LevelFilter::Warn)]),
            parse_filter("foo=warn")
        );
        assert_eq!(
            (LevelFilter::Info, vec![("foo", LevelFilter::Warn)]),
            parse_filter("info,foo=warn")
        );
        assert_eq!(
            (
                LevelFilter::Info,
                vec![("foo", LevelFilter::Warn), ("bar", LevelFilter::Error)]
            ),
            parse_filter("info,foo=warn,bar=error")
        );
    }

    #[test]
    fn test_logger_parse_level() {
        assert_eq!(LevelFilter::Trace, parse_level("trace"));
        assert_eq!(LevelFilter::Debug, parse_level("debug"));
        assert_eq!(LevelFilter::Info, parse_level("info"));
        assert_eq!(LevelFilter::Warn, parse_level("warn"));
        assert_eq!(LevelFilter::Error, parse_level("error"));
    }
}
