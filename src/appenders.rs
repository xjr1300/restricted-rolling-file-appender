use std::{
    fmt::Debug,
    fs::{self, DirEntry, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};

use regex::Regex;
use time::{Date, OffsetDateTime};

use crate::sync::{RwLock, RwLockReadGuard};

/// `DailyFileAppender`
///
/// `DailyFileAppender`は、ログをファイルに記録するとともに、日をまたいだとき、ログを記録する
/// ファイルを別のファイルに切り替える。
/// また、別のファイルに切り替えたとき、ログファイルの数が保存するファイルの数より多くなった場合、
/// 最も古いファイルから削除する。
pub struct DailyRollingFileAppender {
    state: Inner,
    writer: RwLock<File>,
}

#[derive(Debug)]
pub struct RollingWriter<'a>(RwLockReadGuard<'a, File>);

struct Inner {
    current_date: Date,
    max_count: usize,
    directory: PathBuf,
    filename_prefix: String,
}

impl DailyRollingFileAppender {
    /// `DailyRollingFileAppender`を作成する。
    ///
    /// # Arguments
    ///
    /// * directory: ファイルを作成するディレクトリ。
    /// * file_name_prefix: ファイル名の接頭語。
    /// * max_count: 残す最大ファイル数。
    ///
    /// # Returns
    ///
    /// `LRFAppender`インスタンス。
    pub fn new(
        max_count: usize,
        directory: impl AsRef<Path>,
        filename_prefix: impl AsRef<Path>,
    ) -> Self {
        let today = today();
        let (state, writer) = Inner::new(today, max_count, directory, filename_prefix);

        Self { state, writer }
    }

    #[cfg(test)]
    fn new_test(
        max_count: usize,
        directory: impl AsRef<Path>,
        filename_prefix: impl AsRef<Path>,
        date: Date,
    ) -> Self {
        let (state, writer) = Inner::new(date, max_count, directory, filename_prefix);

        Self { state, writer }
    }
}

impl io::Write for DailyRollingFileAppender {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let writer = self.writer.get_mut();
        if let Some(today) = self.state.should_rollover() {
            self.state.refresh_writer(&today, writer);
        }

        writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.get_mut().flush()
    }
}

impl<'a> tracing_subscriber::fmt::writer::MakeWriter<'a> for DailyRollingFileAppender {
    type Writer = RollingWriter<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        // Should we try to roll over the log file?
        if let Some(today) = self.state.should_rollover() {
            self.state.refresh_writer(&today, &mut *self.writer.write());
        }
        RollingWriter(self.writer.read())
    }
}

impl io::Write for RollingWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self.0).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        (&*self.0).flush()
    }
}

impl Inner {
    fn new(
        today: Date,
        max_count: usize,
        directory: impl AsRef<Path>,
        filename_prefix: impl AsRef<Path>,
    ) -> (Self, RwLock<File>) {
        let directory = directory.as_ref().to_owned();
        let filename_prefix = filename_prefix.as_ref().to_str().unwrap().to_string();
        let writer = RwLock::new(
            create_writer(&directory, &filename_prefix, &today).expect("failed to create appender"),
        );

        let inner = Inner {
            directory,
            filename_prefix,
            current_date: today,
            max_count,
        };

        (inner, writer)
    }

    /// ファイルをローテーションする必要があるか確認する。
    ///
    /// # 戻り値
    ///
    /// ファイルをローテーションする必要がある場合は日付。必要ない場合はNone。
    fn should_rollover(&self) -> Option<Date> {
        let today = today();

        if self.current_date < today {
            Some(today)
        } else {
            None
        }
    }

    /// ログファイルを更新する。
    ///
    /// # 引数
    ///
    /// - today: ファイルの日付。
    /// - file: ファイル。
    fn refresh_writer(&self, today: &Date, file: &mut File) {
        if let Err(err) = file.flush() {
            eprintln!("Couldn't flush previous writer: {}", err);
        }
        let result = create_writer(&self.directory, &self.filename_prefix, today);
        match result {
            Ok(new_file) => {
                *file = new_file;
            }
            Err(err) => {
                eprintln!("Couldn't create writer for logs: {}", err);
            }
        }
        // 古いログファイルを削除
        self.remove_old_files();
    }

    /// 古いファイルを削除する。
    fn remove_old_files(&self) {
        let targets = fs::read_dir(&self.directory);
        if let Err(err) = targets {
            eprintln!("Couldn't find log files: {}", err);
            return;
        }
        let targets = targets.unwrap();
        let mut targets: Vec<DirEntry> = targets
            .filter_map(|entry| match entry {
                Ok(entry) => is_log_file(entry, &self.filename_prefix),
                Err(_) => None,
            })
            .collect();
        if self.max_count < targets.len() {
            targets.sort_by(|a, b| a.file_name().cmp(&b.file_name()).reverse());
            for target in &targets[..(targets.len() - self.max_count)] {
                if let Err(err) = std::fs::remove_file(target.path()) {
                    eprintln!("Couldn't remove log file: {}", err);
                }
            }
        }
    }
}

/// ディレクトリエントリがログファイルであるか確認する。
///
/// # 引数
///
/// - entry: ディレクトリエントリ。
/// - prefix: ログファイルの接頭語。
///
/// # 戻り値
///
/// ログファイルの場合はそのディレクトリエントリ。ログファイルでない場合はNone。
fn is_log_file(entry: DirEntry, prefix: &str) -> Option<DirEntry> {
    if entry.file_type().is_err() {
        return None;
    }
    if !entry.file_type().unwrap().is_file() {
        return None;
    }

    let pattern = format!(r"^{}-\d{{8}}.log$", prefix);
    let re = Regex::new(&pattern).unwrap();

    let file_name: String = entry.file_name().to_string_lossy().into();
    match re.is_match(&file_name) {
        true => Some(entry),
        false => None,
    }
}

/// 本日の日付を取得して、返却する。
///
/// # 戻り値
///
/// 本日の日付（時刻はすべて0）。
fn today() -> Date {
    let now = SystemTime::now();
    let now = OffsetDateTime::from(now);

    now.date()
}

/// 日毎にローテーションするログファイルの名前を作成して、返却する。
///
/// ログファイル名は、`{filename_prefix}-<yyyymmdd>.log`となる。
///
/// # 引数
///
/// - filename_prefix: ファイル名の接頭語。
/// - date: ファイルの日付。
///
/// # 戻り値
///
/// ログファイル名。
fn create_daily_log_filename(filename_prefix: &str, date: &Date) -> String {
    let month: u8 = date.month().into();

    format!(
        "{}-{:04}{:02}{:02}.log",
        filename_prefix,
        date.year(),
        month,
        date.day()
    )
}

/// ログファイルのパスを生成して、返却する。
///
/// # 引数
///
/// - directory: ログファイルディレクトリ。
/// - filename: ログファイル名。
///
/// # 戻り値
///
/// ログファイルパスを返却する。
fn create_daily_log_path(directory: &Path, filename: &str) -> String {
    directory.join(filename).to_str().unwrap().to_string()
}

/// ライターを作成する。
///
/// # 引数
///
/// - path: ログファイルディレクトリのパス。
/// - filename_prefix: ログファイルの接頭語。
/// - date: ログファイルの日付。
///
/// # 戻り値
///
/// `File`インスタンス。
fn create_writer(directory: &Path, filename_prefix: &str, date: &Date) -> io::Result<File> {
    let filename = create_daily_log_filename(filename_prefix, date);
    let path = create_daily_log_path(directory, &filename);
    let path = Path::new(&path);
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
    use super::*;
    use time::format_description;

    #[test]
    fn test_create_daily_log_filename() {
        let filename_prefix = "foo";
        let today = "20220526";
        let expected = format!("{}-{}.log", filename_prefix, today);

        let format = format_description::parse("[year][month][day]").unwrap();
        let date = Date::parse(&today, &format).unwrap();

        let path = create_daily_log_filename(filename_prefix, &date);
        assert_eq!(expected, path);
    }

    fn write_to_log(appender: &mut DailyRollingFileAppender, msg: &str) {
        appender
            .write_all(msg.as_bytes())
            .expect("Failed to write to appender");
        appender.flush().expect("Failed to flush!");
    }

    fn find_str_in_log_files(dir_path: &Path, expected_value: &str) -> bool {
        let dir_contents = fs::read_dir(dir_path).expect("Failed to read directory");

        for entry in dir_contents {
            let path = entry.expect("Expected dir entry").path();
            let file = fs::read_to_string(&path).expect("Failed to read file");
            println!("path={}\nfile={:?}", path.display(), file);

            if file.as_str() == expected_value {
                return true;
            }
        }

        false
    }

    fn find_str_in_log_file(path: &Path, expected_value: &str) -> bool {
        let file = fs::read_to_string(&path).expect("Failed to read file");
        println!("path={}\nfile={:?}", path.display(), file);

        file.as_str() == expected_value
    }

    #[test]
    fn test_write_log() {
        let directory = tempfile::tempdir().expect("failed to create temp dir");
        let mut appender = DailyRollingFileAppender::new(3, directory.path(), "foo");

        let expected_value = "Hello";
        write_to_log(&mut appender, expected_value);
        assert!(find_str_in_log_files(directory.path(), expected_value));

        directory
            .close()
            .expect("Failed to explicitly close TempDir. TempDir should delete once out of scope.")
    }

    #[test]
    fn test_rolling_file() {
        // 昨日の日付でアペンダーを作成
        let directory = tempfile::tempdir().expect("failed to create temp dir");
        let filename_prefix = "foo";
        let today = today();
        let yesterday = today.previous_day().unwrap();
        let mut appender =
            DailyRollingFileAppender::new_test(3, directory.path(), filename_prefix, yesterday);

        // ログを出力
        let expected_value = "Hello";
        write_to_log(&mut appender, expected_value);

        // 昨日のログファイルにはログが記録されていないはず
        let yesterday_name = create_daily_log_filename(filename_prefix, &yesterday);
        let yesterday_path = create_daily_log_path(directory.path(), &yesterday_name);
        assert!(find_str_in_log_file(Path::new(&yesterday_path), ""));

        // 今日のログファイルにはログが記録されているはず
        let today_name = create_daily_log_filename(filename_prefix, &today);
        let today_path = create_daily_log_path(directory.path(), &today_name);
        assert!(find_str_in_log_file(Path::new(&today_path), expected_value));

        directory
            .close()
            .expect("Failed to explicitly close TempDir. TempDir should delete once out of scope.")
    }
}
