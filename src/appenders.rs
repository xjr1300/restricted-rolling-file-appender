use std::{
    fmt::{Debug},
    fs::{self, File, OpenOptions},
    io::{self},
    path::Path,
};

use std::sync::{RwLock, RwLockReadGuard};
use time::{format_description, OffsetDateTime};

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
    rotation: Rotation,
    log_directory: String,
    log_filename_prefix: String,
    max_count: u16,
    writer: RwLock<File>,
}

#[derive(Debug)]
struct RollingWriter<'a>(RwLockReadGuard<'a, File>);

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
        filename_prefix: impl AsRef<Path>,
        max_count: u16,
    ) -> LRFAppender {
        let log_directory = directory.as_ref().to_str().unwrap().to_string();
        let log_filename_prefix = filename_prefix.as_ref().to_str().unwrap().to_string();
        let log_filepath = create_log_filepath(&rotation, &log_directory, &log_filename_prefix);
        let writer = RwLock::new(create_writer(&log_filepath).expect("failed to create appender"));

        Self {
            rotation,
            log_directory,
            log_filename_prefix,
            max_count,
            writer,
        }
    }
}

/// ログファイルパスを作成して返却する。
///
/// # 引数
///
/// - rotation: ファイルを切り替えるタイミング。
/// - directory: ファイルを作成するディレクトリ。
/// - filename_prefix: ファイル名の接頭語。
///
/// # 戻り値
///
/// ログファイル名。
fn create_log_filepath(rotation: &Rotation, directory: &str, filename_prefix: &str) -> String {
    let filename = match *rotation {
        Rotation::Daily => {
            let today = OffsetDateTime::now_local().expect("Unable to retrieve date of today");

            create_daily_log_filename(filename_prefix, &today)
        }
        Rotation::Size(_) => create_restricted_size_log_filename(filename_prefix),
    };

    Path::new(directory)
        .join(filename)
        .to_str()
        .unwrap()
        .to_string()
}

/// 日毎にローテーションするログファイルの名前を作成して、返却する。
///
/// ログファイル名は、`{filename_prefix}-<yyyymmdd>.log`となる。
///
/// # 引数
///
/// - filename_prefix: ファイル名の接頭語。
/// - today: ファイルの日付。
///
/// # 戻り値
///
/// ログファイル名。
fn create_daily_log_filename(filename_prefix: &str, today: &OffsetDateTime) -> String {
    let format = format_description::parse("[year][month][day]").expect(
        "Unable to create a date formatter; this is a bug in limited-rolling-file-appender",
    );
    let date = today
        .format(&format)
        .expect("Unable to format OffsetDateTime; this is a bug in limited-rolling-file-appender");

    format!("{}-{}.log", filename_prefix, date)
}

/// ファイルサイズを制限されたログファイルの名前を作成して、返却する。
///
/// ログファイル名は、`{filename_prefix}`となる。
///
/// # 引数
///
/// - directory: ファイルを作成するディレクトリ。
/// - filename_prefix: ファイル名の接頭語。
///
/// # 戻り値
///
/// ログファイル名。

fn create_restricted_size_log_filename(filename_prefix: &str) -> String {
    filename_prefix.to_string()
}

/// ライターを作成する。
///
/// # 引数
///
/// * path: ログファイルパス。
///
/// # 戻り値
///
/// `File`インスタンス。
fn create_writer(path: &str) -> io::Result<File> {
    let path = Path::new(path);
    let mut open_options = OpenOptions::new();
    open_options.append(true).create(true);

    let new_file = open_options.open(path);
    if new_file.is_err() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
            return open_options.open(path);
        }
    }

    new_file
}

#[cfg(test)]
mod tests {
    use time::{format_description, OffsetDateTime};

    use super::*;

    #[test]
    fn test_create_daily_log_filename() {
        let filename_prefix = "foo";
        let today = "20220526";
        let expected = format!("{}-{}.log", filename_prefix, today);

        let now = format!("{} 15:25:32 +09:00:00", today);
        let format = format_description::parse(
            "[year][month][day] [hour]:[minute]:[second] [offset_hour \
                sign:mandatory]:[offset_minute]:[offset_second]",
        )
        .unwrap();
        let date = OffsetDateTime::parse(&now, &format).unwrap();

        let path = create_daily_log_filename(filename_prefix, &date);
        assert_eq!(expected, path);
    }

    #[test]
    fn test_create_restricted_size_log_filename() {
        let filename_prefix = "foo.log";
        let expected = "foo.log";

        assert_eq!(
            expected,
            create_restricted_size_log_filename(filename_prefix)
        )
    }
}
