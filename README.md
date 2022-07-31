# neosvr-inventory-management
## Notes to foreign users
* This readme is written in Japanese. Please use external tool to translate this document into your language. I'm sorry for inconvenience.
* If this document is translated into a language other than Japanese, the Japanese version will prevail to the extent that there is any conflict.

## これはなに？
[NeosVR](https://neos.com)用のインベントリ整理支援ツールです。現在はベータ版となっています。

## なぜ？
私が知る限りでは、NeosはWindowsの「エクスプローラー」にあるようなディレクトリ間の移動ができません。
話を聞いていく中で、「大晦日にインベントリを整理するイベントがある」「アイテムを全部だして選り分ける必要がある」という事例を聞きました。
また、開発者である私も、ゲーム内でインベントリにアイテムをセーブする中で、インベントリ直下のアイテムがすでに8列ほどとなっていました。このペースで保存していくと必然的に直下が大変なカオスを招くことになるだろうと予想したため、今回ツールの作成に踏み切りました。

## ダウンロード
ダウンロードの前に[下記](#ライセンス)をお読みください。

以下のOS向けに予めコンパイルされたバイナリが配布されています。[Releases](https://github.com/KisaragiEffective/neosvr-inventory-management/releases)からダウンロードしてください。古いリリースにはバグが含まれている可能性があるので、基本的には最新のリリースを使用してください。
* Windows (`*-x86_64-pc-windows-gnu.zip`)
* Linux (`*-x86_64-unknown-linux-musl.tar.gz`)
* macOS (`*-x86_64-apple-darwin.zip`)

もし対応していないなど、何らかの事情でセルフコンパイルが必要な場合、`git clone`からこのレポジトリをクローンしてソースコードを入手してください。

## 使用
並行ログインに対応しました :tada:

### ご注意
* `cargo run`経由で動かす場合、`cargo`のフラグと本プロダクトのフラグを`--`で区切ってください。
  * 例: `cargo run --release -- -e kisaragi.marine@gmail.com -p ************** list Inventory`
* ログは標準エラー出力**及び**カレントディレクトリの`output.log`に出力されます。

### 認証
* `-e` or `--email`: Eメール
* `-p` or `--password`: パスワード
* `-t` or `--totp`: 二要素認証のトークン (任意)
* `-u` or `--user-id`: ユーザーID
* `--read-token-from-stdin`: 標準入力からトークンを読み込む

#### 認証方法
1. (`-e` または `-u`) と `-p` (と `-t`) を指定する
2. `-u` と `--read-token-from-stdin` を指定する

* 認証情報を提供しない場合はログインしません。その場合、`isPublic`が`true`のレコードのみ見ることができます。
* ログインするべきアカウントが特定できない場合はエラーになります。

### 例
#### 例1
`U-kisaragi-marine`の`Inventory\\Public`フォルダを見る

##### 入力1
```shell
neosvr-inventory-management --log-level none list -u U-kisaragi-marine Inventory Public
```

##### 出力1
(インベントリの各アイテムごとのJSON、1行に1アイテム)

##### 注意1
* `--log-level none` でログの出力を抑制しています。

#### 例2
`U-kisaragi-marine`の`R-65e927ba-d3cf-4d82-b5ec-ef5b1d34e143`を`Inventory\\Work`に移動する

##### 入力2
```shell
neosvr-inventory-management -e kisaragi.marine@gmail.com -p 1234567890 move -r R-65e927ba-d3cf-4d82-b5ec-ef5b1d34e143 -u U-kisaragi-marine --to Inventory --to Work 2>/dev/null
```

##### 出力2
(なし)

##### 注意
* `neosvr-inventory-management -e kisaragi.marine@gmail.com -p 1234567890`で認証を行っています。

## コントリビューション
バグ報告、デバッグ、パッチの送信、ドキュメントの誤字修正など、いかなる形でもコントリビューションをいただければ幸いです。
このツールはRustで書かれています。
パッチを送っていただいた際、別途の表明がない限りは、下記[ライセンス](#ライセンス)においてパッチを取り扱わせていただきます。

## ライセンス
* `src`ディレクトリ以下のソースコード、ソースコードに付随するドキュメント、及び配布されるバイナリはRust本体のライセンスに合わせて、MITライセンスとApache License, Version 2.0 (SPDX: `MIT OR Apache-2.0`) とします。
  * このライセンスはデュアルライセンスであり、あなたはどちらかのライセンスを選ぶことができます。
* README.md、Cargo.lock、Cargo.tomlはCC0とします。
* ファイルの中に合理的な方法でライセンスが表示されている場合は、そのファイルはそのライセンスによってライセンスされています。
* その他のファイルについては著作権を留保します。

### 免責事項
* MIT License及びApache License, Version 2.0 §7に指定されている通り、成果物は現状のまま提供されるものとし、開発者およびコントリビューターは当人が提供する部分についていかなる保証も提供しません。
  * 特に、このツールを使用して目的が達成されることをいかなる形態でも保障しません。
  * 特に、このツールを使用してユーザーにアイテム消失などの損害が起きないことを保障しません。
  * 特に、内部APIの破壊的変更に追従することを保障しません。

## 開発者
* KisaragiEffective (IGN: `kisaragi marine`)

## スペシャルサンクス
(敬称略)
* kazu0617
