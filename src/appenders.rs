use std::path::Path;

use tracing_appender::rolling;

/// ファイルを切り替えるタイミング
#[derive(Clone)]
pub enum Rotation {
    /// 日付が変わったときにファイルを切り替え
    Daily,
    /// 指定されたサイズ書き込んだときにファイルを切り替え
    /// 列挙しの値は、切り替えるファイルのサイズをバイトで指定
    Size(u64),
}

pub struct LRFAppender {
    appender: rolling::RollingFileAppender,
    rotation: Rotation,
    max_count: u16,
}

impl LRFAppender {
    /// `LRFAppender`を作成する。
    ///
    /// # Arguments
    ///
    /// * rotation: ファイルを切り替えるタイミング。
    /// * directory: ファイルを作成するディレクトリ。
    /// * file_name_prefix: ファイル名の接頭語。
    /// * max_count: 残す最大ファイル数。
    ///
    /// # Returns
    ///
    /// `LRFAppender`インスタンス。
    pub fn new(
        rotation: Rotation,
        directory: impl AsRef<Path>,
        file_name_prefix: impl AsRef<Path>,
        max_count: u16,
    ) -> LRFAppender {
        Self {
            appender: create_appender(rotation.clone(), directory, file_name_prefix),
            rotation,
            max_count,
        }
    }
}

/// `RollingFileAppender`を作成する。
///
/// # 引数
///
/// * rotation: ファイルを切り替えるタイミング。
/// * directory: ファイルを作成するディレクトリ。
/// * file_name_prefix: ファイル名の接頭語。
///
/// # 戻り値
///
/// `LRFAppender`インスタンス。
fn create_appender(
    rotation: Rotation,
    directory: impl AsRef<Path>,
    file_name_prefix: impl AsRef<Path>,
) -> rolling::RollingFileAppender {
    return match rotation {
        Rotation::Daily => rolling::daily(directory, file_name_prefix),
        Rotation::Size(_) => rolling::never(directory, file_name_prefix),
    };
}
