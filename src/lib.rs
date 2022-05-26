//! Limited Rolling File Appender
//!
//! ----------------------------------------------------------------------------
//!
//! このクレートには、`LRFAppender(Limited Rolling File Appender)`構造体が
//! 含まれており、この構造体は、ログを記録することを目的としている。
//! `LRFAppender`は、`tracing-appender`クレートの`RollingFileAppender`を内部にもち（合成）、
//! 多くの処理を`RollingFileAppender`に移譲している。
//!
//! `LRFAppender`は、以下いずれかの条件で、ログの記録を新しいファイルに切り替える。
//!
//! - 日付が変わったとき
//! - ファイルに指定されたサイズのログを出力したとき
//!
//! また、`LRFAppender`は、残しておく最大ファイル数を持つ。
//! `LRFAppender`は、新しいファイルに切り替えるとき、現在のファイルを含めて、ファイル数が
//! 最大ファイル数を超えた場合、最も古いファイルから削除する。

pub mod appenders;
