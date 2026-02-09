use fs2::FileExt;
use std::collections::HashMap;
use std::env;
use std::fs::OpenOptions;
use std::io::{Read, Seek, SeekFrom, Write};
use std::process;

fn main() {
    // 引数を取得
    let args: Vec<String> = env::args().collect();

    // 引数の数をチェック
    if args.len() != 4 {
        eprintln!(
            "使用法: {} <ファイル名> \"(word1 word2)\" \"(word2 word1)\"",
            args[0]
        );
        eprintln!(
            "例: {} text.txt \"(dog cat rabbit)\" \"(rabbit dog cat)",
            args[0]
        );
        process::exit(-1);
    }

    // 引数を整理
    let filename = &args[1];
    let source_words = parse_alternating_group_string(&args[2]);
    let target_words = parse_alternating_group_string(&args[3]);

    // バリデーション（要素数、構成単語の一致、重複チェック）
    validate_alternating_group(&source_words, &target_words);

    // 置換マップ作成 (String -> String)
    let substitution_map: HashMap<String, String> = source_words
        .iter()
        .cloned()
        .zip(target_words.iter().cloned())
        .collect();

    // ファイルを読み書きモードでオープン
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(filename)
        .unwrap_or_else(|e| {
            eprintln!("ファイルオープンエラー: {}", e);
            process::exit(1);
        });

    // 排他ロック
    file.lock_exclusive()
        .expect("ファイルロックの取得に失敗しました");

    // 読み込み
    let mut content = String::new();
    file.read_to_string(&mut content).expect("読み込み失敗");

    // 文字列置換の実行
    // 単純な replace だと「置換した結果をさらに置換」してしまう可能性があるため、変換対象の単語を一度にスキャンして置換
    let new_content = replace_multiple(&content, &substitution_map, &source_words);

    // ファイルポインターを先頭に戻し、サイズをゼロにする
    file.seek(SeekFrom::Start(0)).expect("シーク失敗");
    file.set_len(0).expect("ファイルサイズ変更失敗");

    // 書き出し
    file.write_all(new_content.as_bytes())
        .expect("書き出し失敗");

    // 何もしなくても自動で行われるが、明示的にフラッシュとロックを行う
    file.flush().expect("フラッシュ失敗");
    file.unlock().expect("ロック解除失敗");

    println!("置換が完了しました。");
}

/// 文字列をスペースで分割して Vec<String> を返す
fn parse_alternating_group_string(input: &str) -> Vec<String> {
    input
        .trim_matches(|c| c == '(' || c == ')')
        .split_whitespace()
        .map(|s| s.to_string())
        .collect()
}

/// 複数の文字列を一度の走査で置換する
fn replace_multiple(text: &str, map: &HashMap<String, String>, keys: &[String]) -> String {
    let mut result = String::new();

    // 効率と正確性のた、長い単語から順にマッチングを試みる
    // 長い単語を優先してマッチさせるためにソートする
    let mut sorted_keys = keys.to_vec();
    sorted_keys.sort_by_key(|b| std::cmp::Reverse(b.len()));

    // ここでは単純かつ確実な「現在の位置からキーのいずれかが開始するか」で走査
    let mut i = 0;
    while i < text.len() {
        let mut matched = false;

        // 置換対象のキーの中に、現在の位置から始まるものがあるか確認
        for key in sorted_keys.clone() {
            if text[i..].starts_with(&key) {
                result.push_str(map.get(&key).unwrap());
                i += key.len();
                matched = true;
                break;
            }
        }

        if !matched {
            // マッチしなければ1文字進める
            let c = text[i..].chars().next().unwrap();
            result.push(c);
            i += c.len_utf8();
        }
    }
    result
}

// 置換群になっているか検証
fn validate_alternating_group(s_words: &[String], t_words: &[String]) {
    if s_words.len() != t_words.len() {
        eprintln!("エラー: 要素数が一致しません。");
        process::exit(1);
    }

    let mut s_sorted = s_words.to_vec();
    let mut t_sorted = t_words.to_vec();
    s_sorted.sort();
    t_sorted.sort();

    if s_sorted != t_sorted {
        eprintln!("エラー: 置換元と置換先の単語セットが一致しません。");
        process::exit(1);
    }

    for i in 0..s_sorted.len().saturating_sub(1) {
        if s_sorted[i] == s_sorted[i + 1] {
            eprintln!(
                "エラー: 置換元に重複する単語 '{}' があります。",
                s_sorted[i]
            );
            process::exit(1);
        }
    }
}
