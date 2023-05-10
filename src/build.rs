/* Copyright (C) 2018 Olivier Goffart <ogoffart@woboq.com>
Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense,
and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so,
subject to the following conditions:
The above copyright notice and this permission notice shall be included in all copies or substantial
portions of the Software.
THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES
OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
*/
extern crate cpp_build;
use std::env;
use std::fs;
use std::fs::DirEntry;
use std::path::PathBuf;
use std::process::Command;
//use cmake;

fn qmake_query(qmake: &str, args: &str, var: &str) -> String {
    let mut qmake_cmd_list: Vec<&str> = qmake.split(' ').collect();
    qmake_cmd_list.push("-query");
    qmake_cmd_list.push(var);
    if !args.is_empty() {
        let mut qmake_args_list: Vec<&str> = args.split(' ').collect();
        qmake_cmd_list.append(&mut qmake_args_list);
    }
    String::from_utf8(
        Command::new(qmake_cmd_list[0])
            .args(&qmake_cmd_list[1..])
            .output()
            .expect("Failed to execute qmake. Make sure 'qmake' is in your path")
            .stdout,
    )
    .expect("UTF-8 conversion failed")
}

/// Gets environment variables by name
fn env_var(key: &str) -> Result<String, env::VarError> {
    Ok(String::from(env::var(key)?))
}

/// Generates the shell command to call for qmake
fn qmake_call() -> String {
    env_var("QMAKE").unwrap_or(String::from("qmake"))
}

/// Generates the arguments for the call to qmake
fn qmake_args() -> String {
    env_var("QMAKE_ARGS").unwrap_or(String::new())
}

/// Generate gettext translation files
fn gettext() {
    std::fs::create_dir_all("po").unwrap();
    let pot_file = "po/utwallet.ulrichard.pot";
    let source_files = source_files();

    let mut child = Command::new("xgettext")
        .args(&[
            &format!("--output={}", pot_file),
            "--language=javascript",
            "--qt",
            "--keyword=tr",
            "--keyword=tr:1,2",
            "--add-comments=i18n",
            "--from-code=UTF-8",
        ])
        .args(&source_files)
        .spawn()
        .unwrap();

    let exit_status = child.wait().unwrap();
    assert!(exit_status.code() == Some(0));

    for po_file in po_files() {
        let mut child = Command::new("msgmerge")
            .args(&["--update", &po_file.to_str().unwrap(), pot_file])
            .spawn()
            .unwrap();

        let exit_status = child.wait().unwrap();
        assert!(exit_status.code() == Some(0));

        let install_dir = env_var("INSTALL_DIR").unwrap();

        let mo_dir = format!(
            "{}/share/locale/{}/LC_MESSAGES",
            install_dir,
            po_file.file_stem().unwrap().to_str().unwrap()
        );
        fs::create_dir_all(&mo_dir).unwrap();

        let mo_file = format!("{}/utwallet.ulrichard.mo", mo_dir);

        let mut child = Command::new("msgfmt")
            .args(&[&po_file.to_str().unwrap(), "-o", &mo_file])
            .spawn()
            .unwrap();

        let exit_status = child.wait().unwrap();
        assert!(exit_status.code() == Some(0));
    }
}

/// Obtains a list of all QML files
fn source_files() -> Vec<PathBuf> {
    // Directory in which to search for QML files and file extension
    walk_dir(PathBuf::from("qml"), "qml")
}

/// Recursively searches for files in a directory and
/// returns a list of paths to the files
fn walk_dir<T>(dir: PathBuf, ext: T) -> Vec<PathBuf>
where
    T: AsRef<str>,
{
    let extension = ext.as_ref().to_string();
    let mut files: Vec<PathBuf> = Vec::new();
    let entries: Vec<DirEntry> = fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).collect();
    for entry in entries.iter() {
        if entry.file_type().unwrap().is_dir() {
            files.append(&mut walk_dir(entry.path(), &extension));
        } else {
            match entry.path().extension() {
                Some(file_ext) => {
                    if file_ext.to_str().unwrap().to_string() == extension {
                        files.push(entry.path())
                    }
                }
                None => {}
            }
        }
    }
    files
}

/// Obtains a list of all translation files
fn po_files() -> Vec<PathBuf> {
    // Directory in which to search for translation files
    fs::read_dir("po")
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|ext| ext == "po").unwrap_or(false))
        .collect()
}

fn main() {
    gettext();

    let qmake_cmd = qmake_call();
    let args = qmake_args();

    let qt_include_path = qmake_query(&qmake_cmd, &args, "QT_INSTALL_HEADERS");
    let qt_library_path = qmake_query(&qmake_cmd, &args, "QT_INSTALL_LIBS");

    cpp_build::Config::new()
        .include(qt_include_path.trim())
        .build("src/main.rs");

    let macos_lib_search = if cfg!(target_os = "macos") {
        "=framework"
    } else {
        ""
    };
    let macos_lib_framework = if cfg!(target_os = "macos") { "" } else { "5" };

    println!(
        "cargo:rustc-link-search{}={}",
        macos_lib_search,
        qt_library_path.trim()
    );
    println!(
        "cargo:rustc-link-lib{}=Qt{}Widgets",
        macos_lib_search, macos_lib_framework
    );
    println!(
        "cargo:rustc-link-lib{}=Qt{}Gui",
        macos_lib_search, macos_lib_framework
    );
    println!(
        "cargo:rustc-link-lib{}=Qt{}Core",
        macos_lib_search, macos_lib_framework
    );
    println!(
        "cargo:rustc-link-lib{}=Qt{}Quick",
        macos_lib_search, macos_lib_framework
    );
    println!(
        "cargo:rustc-link-lib{}=Qt{}Qml",
        macos_lib_search, macos_lib_framework
    );
    println!(
        "cargo:rustc-link-lib{}=Qt{}QuickControls2",
        macos_lib_search, macos_lib_framework
    );

    // Builds the project in the directory located in `qzxing`, installing it
    // into $OUT_DIR
    //    let dst = cmake::build("qzxing/src");

    //    println!("cargo:rustc-link-search=native={}", dst.display());
    //    println!("cargo:rustc-link-lib=static=qzxing");
}
