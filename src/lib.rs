//! Limited Rolling File Appender
//!
//! このクレートには、ログを記録することを目的とする構造体が含まれている。
//! ログを記録することを目的とした構造体は、`tracing-appender`クレートの`RollingFileAppender`
//! を内部にもち（合成）、! 多くの処理を`RollingFileAppender`に移譲している。
pub mod appenders;
mod sync;
